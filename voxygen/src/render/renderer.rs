mod binding;
pub(super) mod drawer;
// Consts and bind groups for post-process and clouds
mod locals;
mod pipeline_creation;
mod screenshot;
mod shaders;
mod shadow_map;

use locals::Locals;
use pipeline_creation::{
    IngameAndShadowPipelines, InterfacePipelines, PipelineCreation, Pipelines, ShadowPipelines,
};
use shaders::Shaders;
use shadow_map::{ShadowMap, ShadowMapRenderer};

use super::{
    buffer::Buffer,
    consts::Consts,
    instances::Instances,
    mesh::Mesh,
    model::{DynamicModel, Model},
    pipelines::{
        blit, clouds, debug, figure, postprocess, shadow, sprite, terrain, ui, GlobalsBindGroup,
        GlobalsLayouts, ShadowTexturesBindGroup,
    },
    texture::Texture,
    AaMode, AddressMode, FilterMode, RenderError, RenderMode, ShadowMapMode, ShadowMode, Vertex,
};
use common::assets::{self, AssetExt, AssetHandle};
use common_base::span;
use core::convert::TryFrom;
use std::sync::Arc;
use tracing::{error, info, warn};
use vek::*;

// TODO: yeet this somewhere else
/// A type representing data that can be converted to an immutable texture map
/// of ColLight data (used for texture atlases created during greedy meshing).
// TODO: revert to u16
pub type ColLightInfo = (Vec<[u8; 4]>, Vec2<u16>);

const QUAD_INDEX_BUFFER_U16_START_VERT_LEN: u16 = 3000;
const QUAD_INDEX_BUFFER_U32_START_VERT_LEN: u32 = 3000;

/// A type that stores all the layouts associated with this renderer.
struct Layouts {
    global: GlobalsLayouts,

    clouds: clouds::CloudsLayout,
    debug: debug::DebugLayout,
    figure: figure::FigureLayout,
    postprocess: postprocess::PostProcessLayout,
    shadow: shadow::ShadowLayout,
    sprite: sprite::SpriteLayout,
    terrain: terrain::TerrainLayout,
    ui: ui::UiLayout,
    blit: blit::BlitLayout,
}

/// Render target views
struct Views {
    // NOTE: unused for now, maybe... we will want it for something
    _win_depth: wgpu::TextureView,

    tgt_color: wgpu::TextureView,
    tgt_depth: wgpu::TextureView,
    // TODO: rename
    tgt_color_pp: wgpu::TextureView,
}

/// Shadow rendering textures, layouts, pipelines, and bind groups
struct Shadow {
    map: ShadowMap,
    bind: ShadowTexturesBindGroup,
}

/// Represent two states of the renderer:
/// 1. Only interface pipelines created
/// 2. All of the pipelines have been created
#[allow(clippy::large_enum_variant)] // They are both pretty large
enum State {
    // NOTE: this is used as a transient placeholder for moving things out of State temporarily
    Nothing,
    Interface {
        pipelines: InterfacePipelines,
        shadow_views: Option<(Texture, Texture)>,
        // In progress creation of the remaining pipelines in the background
        creating: PipelineCreation<IngameAndShadowPipelines>,
    },
    Complete {
        pipelines: Pipelines,
        shadow: Shadow,
        recreating: Option<PipelineCreation<Result<(Pipelines, ShadowPipelines), RenderError>>>,
    },
}

/// A type that encapsulates rendering state. `Renderer` is central to Voxygen's
/// rendering subsystem and contains any state necessary to interact with the
/// GPU, along with pipeline state objects (PSOs) needed to renderer different
/// kinds of models to the screen.
pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    sc_desc: wgpu::SwapChainDescriptor,

    sampler: wgpu::Sampler,
    depth_sampler: wgpu::Sampler,

    state: State,
    // true if there is a pending need to recreate the pipelines (e.g. RenderMode change or shader
    // hotloading)
    recreation_pending: bool,

    layouts: Arc<Layouts>,
    // Note: we keep these here since their bind groups need to be updated if we resize the
    // color/depth textures
    locals: Locals,
    views: Views,
    noise_tex: Texture,

    quad_index_buffer_u16: Buffer<u16>,
    quad_index_buffer_u32: Buffer<u32>,

    shaders: AssetHandle<Shaders>,

    mode: RenderMode,
    resolution: Vec2<u32>,

    // If this is Some then a screenshot will be taken and passed to the handler here
    take_screenshot: Option<screenshot::ScreenshotFn>,

    profiler: wgpu_profiler::GpuProfiler,
    profile_times: Vec<wgpu_profiler::GpuTimerScopeResult>,
    profiler_features_enabled: bool,

    // This checks is added because windows resizes the window to 0,0 when
    // minimizing and this causes a bunch of validation errors
    is_minimized: bool,

    // To remember the backend info after initialization for debug purposes
    graphics_backend: String,
}

impl Renderer {
    /// Create a new `Renderer` from a variety of backend-specific components
    /// and the window targets.
    pub fn new(window: &winit::window::Window, mut mode: RenderMode) -> Result<Self, RenderError> {
        // Enable seamless cubemaps globally, where available--they are essentially a
        // strict improvement on regular cube maps.
        //
        // Note that since we only have to enable this once globally, there is no point
        // in doing this on rerender.
        // Self::enable_seamless_cube_maps(&mut device);

        // TODO: fix panic on wayland with opengl?
        // TODO: fix backend defaulting to opengl on wayland.
        let backend_bit = std::env::var("WGPU_BACKEND")
            .ok()
            .and_then(|backend| match backend.to_lowercase().as_str() {
                "vulkan" => Some(wgpu::BackendBit::VULKAN),
                "metal" => Some(wgpu::BackendBit::METAL),
                "dx12" => Some(wgpu::BackendBit::DX12),
                "primary" => Some(wgpu::BackendBit::PRIMARY),
                "opengl" | "gl" => Some(wgpu::BackendBit::GL),
                "dx11" => Some(wgpu::BackendBit::DX11),
                "secondary" => Some(wgpu::BackendBit::SECONDARY),
                "all" => Some(wgpu::BackendBit::all()),
                _ => None,
            })
            .unwrap_or(
                (wgpu::BackendBit::PRIMARY | wgpu::BackendBit::SECONDARY) & !wgpu::BackendBit::GL,
            );

        let instance = wgpu::Instance::new(backend_bit);

        let dims = window.inner_size();

        // This is unsafe because the window handle must be valid, if you find a way to
        // have an invalid winit::Window then you have bigger issues
        #[allow(unsafe_code)]
        let surface = unsafe { instance.create_surface(window) };

        let adapter = futures_executor::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            },
        ))
        .ok_or(RenderError::CouldNotFindAdapter)?;

        let info = adapter.get_info();
        info!(
            ?info.name,
            ?info.vendor,
            ?info.backend,
            ?info.device,
            ?info.device_type,
            "selected graphics device"
        );
        let graphics_backend = format!("{:?}", &info.backend);

        let limits = wgpu::Limits {
            max_push_constant_size: 64,
            ..Default::default()
        };

        let (device, queue) = futures_executor::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                // TODO
                label: None,
                features: wgpu::Features::DEPTH_CLAMPING
                    | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                    | wgpu::Features::PUSH_CONSTANTS
                    | (adapter.features() & wgpu_profiler::GpuProfiler::REQUIRED_WGPU_FEATURES),
                limits,
            },
            std::env::var_os("WGPU_TRACE_DIR").as_ref().map(|v| {
                let path = std::path::Path::new(v);
                // We don't want to continue if we can't actually collect the api trace
                if !path.exists() {
                    panic!(
                        "WGPU_TRACE_DIR is set to the path \"{}\" which doesn't exist",
                        path.display()
                    );
                }
                if !path.is_dir() {
                    panic!(
                        "WGPU_TRACE_DIR is set to the path \"{}\" which is not a directory",
                        path.display()
                    );
                }
                if path
                    .read_dir()
                    .expect("Could not read the directory that is specified by WGPU_TRACE_DIR")
                    .next()
                    .is_some()
                {
                    panic!(
                        "WGPU_TRACE_DIR is set to the path \"{}\" which already contains other \
                         files",
                        path.display()
                    );
                }

                path
            }),
        ))?;

        // Set error handler for wgpu errors
        // This is better for use than their default because it includes the error in
        // the panic message
        device.on_uncaptured_error(move |error| {
            error!("{}", &error);
            panic!(
                "wgpu error (handling all wgpu errors as fatal):\n{:?}\n{:?}",
                &error, &info,
            );
        });

        let profiler_features_enabled = device
            .features()
            .contains(wgpu_profiler::GpuProfiler::REQUIRED_WGPU_FEATURES);
        if !profiler_features_enabled {
            info!(
                "The features for GPU profiling (timestamp queries) are not available on this \
                 adapter"
            );
        }

        let format = adapter
            .get_swap_chain_preferred_format(&surface)
            .expect("No supported swap chain format found");
        info!("Using {:?} as the swapchain format", format);

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format,
            width: dims.width,
            height: dims.height,
            present_mode: mode.present_mode.into(),
        };

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let shadow_views = ShadowMap::create_shadow_views(
            &device,
            (dims.width, dims.height),
            &ShadowMapMode::try_from(mode.shadow).unwrap_or_default(),
        )
        .map_err(|err| {
            warn!("Could not create shadow map views: {:?}", err);
        })
        .ok();

        let shaders = Shaders::load_expect("");

        let layouts = {
            let global = GlobalsLayouts::new(&device);

            let clouds = clouds::CloudsLayout::new(&device);
            let debug = debug::DebugLayout::new(&device);
            let figure = figure::FigureLayout::new(&device);
            let postprocess = postprocess::PostProcessLayout::new(&device);
            let shadow = shadow::ShadowLayout::new(&device);
            let sprite = sprite::SpriteLayout::new(&device);
            let terrain = terrain::TerrainLayout::new(&device);
            let ui = ui::UiLayout::new(&device);
            let blit = blit::BlitLayout::new(&device);

            Layouts {
                global,

                clouds,
                debug,
                figure,
                postprocess,
                shadow,
                sprite,
                terrain,
                ui,
                blit,
            }
        };

        // Arcify the device and layouts
        let device = Arc::new(device);
        let layouts = Arc::new(layouts);

        let (interface_pipelines, creating) = pipeline_creation::initial_create_pipelines(
            // TODO: combine Arcs?
            Arc::clone(&device),
            Arc::clone(&layouts),
            shaders.read().clone(),
            mode.clone(),
            sc_desc.clone(), // Note: cheap clone
            shadow_views.is_some(),
        )?;

        let state = State::Interface {
            pipelines: interface_pipelines,
            shadow_views,
            creating,
        };

        let views = Self::create_rt_views(&device, (dims.width, dims.height), &mode)?;

        let create_sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: None,
                ..Default::default()
            })
        };

        let sampler = create_sampler(wgpu::FilterMode::Linear);
        let depth_sampler = create_sampler(wgpu::FilterMode::Nearest);

        let noise_tex = Texture::new(
            &device,
            &queue,
            &assets::Image::load_expect("voxygen.texture.noise").read().0,
            Some(wgpu::FilterMode::Linear),
            Some(wgpu::AddressMode::Repeat),
        )?;

        let clouds_locals =
            Self::create_consts_inner(&device, &queue, &[clouds::Locals::default()]);
        let postprocess_locals =
            Self::create_consts_inner(&device, &queue, &[postprocess::Locals::default()]);

        let locals = Locals::new(
            &device,
            &layouts,
            clouds_locals,
            postprocess_locals,
            &views.tgt_color,
            &views.tgt_depth,
            &views.tgt_color_pp,
            &sampler,
            &depth_sampler,
        );

        let quad_index_buffer_u16 =
            create_quad_index_buffer_u16(&device, QUAD_INDEX_BUFFER_U16_START_VERT_LEN as usize);
        let quad_index_buffer_u32 =
            create_quad_index_buffer_u32(&device, QUAD_INDEX_BUFFER_U32_START_VERT_LEN as usize);
        let mut profiler = wgpu_profiler::GpuProfiler::new(4, queue.get_timestamp_period());
        mode.profiler_enabled &= profiler_features_enabled;
        profiler.enable_timer = mode.profiler_enabled;
        profiler.enable_debug_marker = mode.profiler_enabled;

        Ok(Self {
            device,
            queue,
            surface,
            swap_chain,
            sc_desc,

            state,
            recreation_pending: false,

            layouts,
            locals,
            views,

            sampler,
            depth_sampler,
            noise_tex,

            quad_index_buffer_u16,
            quad_index_buffer_u32,

            shaders,

            mode,
            resolution: Vec2::new(dims.width, dims.height),

            take_screenshot: None,

            profiler,
            profile_times: Vec::new(),
            profiler_features_enabled,

            is_minimized: false,

            graphics_backend,
        })
    }

    /// Get the graphics backend being used
    pub fn graphics_backend(&self) -> &str { &self.graphics_backend }

    /// Check the status of the intial pipeline creation
    /// Returns `None` if complete
    /// Returns `Some((total, complete))` if in progress
    pub fn pipeline_creation_status(&self) -> Option<(usize, usize)> {
        if let State::Interface { creating, .. } = &self.state {
            Some(creating.status())
        } else {
            None
        }
    }

    /// Check the status the pipeline recreation
    /// Returns `None` if pipelines are currently not being recreated
    /// Returns `Some((total, complete))` if in progress
    pub fn pipeline_recreation_status(&self) -> Option<(usize, usize)> {
        if let State::Complete { recreating, .. } = &self.state {
            recreating.as_ref().map(|r| r.status())
        } else {
            None
        }
    }

    /// Change the render mode.
    pub fn set_render_mode(&mut self, mode: RenderMode) -> Result<(), RenderError> {
        // TODO: are there actually any issues with the current mode not matching the
        // pipelines (since we could previously have inconsistencies from
        // pipelines failing to build due to shader editing)?
        // TODO: FIXME: defer mode changing until pipelines are rebuilt to prevent
        // incompatibilities as pipelines are now rebuilt in a deferred mannder in the
        // background TODO: consider separating changes that don't require
        // rebuilding pipelines
        self.mode = mode;
        self.sc_desc.present_mode = self.mode.present_mode.into();

        // Only enable profiling if the wgpu features are enabled
        self.mode.profiler_enabled &= self.profiler_features_enabled;
        // Enable/disable profiler
        if !self.mode.profiler_enabled {
            // Clear the times if disabled
            core::mem::take(&mut self.profile_times);
        }
        self.profiler.enable_timer = self.mode.profiler_enabled;
        self.profiler.enable_debug_marker = self.mode.profiler_enabled;

        // Recreate render target
        self.on_resize(self.resolution)?;

        // Recreate pipelines with the new AA mode
        self.recreate_pipelines();

        Ok(())
    }

    /// Get the render mode.
    pub fn render_mode(&self) -> &RenderMode { &self.mode }

    /// Get the current profiling times
    /// Nested timings immediately follow their parent
    /// Returns Vec<(how nested this timing is, label, length in seconds)>
    pub fn timings(&self) -> Vec<(u8, &str, f64)> {
        use wgpu_profiler::GpuTimerScopeResult;
        fn recursive_collect<'a>(
            vec: &mut Vec<(u8, &'a str, f64)>,
            result: &'a GpuTimerScopeResult,
            nest_level: u8,
        ) {
            vec.push((
                nest_level,
                &result.label,
                result.time.end - result.time.start,
            ));
            result
                .nested_scopes
                .iter()
                .for_each(|child| recursive_collect(vec, child, nest_level + 1));
        }
        let mut vec = Vec::new();
        self.profile_times
            .iter()
            .for_each(|child| recursive_collect(&mut vec, child, 0));
        vec
    }

    /// Resize internal render targets to match window render target dimensions.
    pub fn on_resize(&mut self, dims: Vec2<u32>) -> Result<(), RenderError> {
        // Avoid panics when creating texture with w,h of 0,0.
        if dims.x != 0 && dims.y != 0 {
            self.is_minimized = false;
            // Resize swap chain
            self.resolution = dims;
            self.sc_desc.width = dims.x;
            self.sc_desc.height = dims.y;
            self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);

            // Resize other render targets
            self.views = Self::create_rt_views(&self.device, (dims.x, dims.y), &self.mode)?;
            // Rebind views to clouds/postprocess bind groups
            self.locals.rebind(
                &self.device,
                &self.layouts,
                &self.views.tgt_color,
                &self.views.tgt_depth,
                &self.views.tgt_color_pp,
                &self.sampler,
                &self.depth_sampler,
            );

            // Get mutable reference to shadow views out of the current state
            let shadow_views = match &mut self.state {
                State::Interface { shadow_views, .. } => {
                    shadow_views.as_mut().map(|s| (&mut s.0, &mut s.1))
                },
                State::Complete {
                    shadow:
                        Shadow {
                            map: ShadowMap::Enabled(shadow_map),
                            ..
                        },
                    ..
                } => Some((&mut shadow_map.point_depth, &mut shadow_map.directed_depth)),
                State::Complete { .. } => None,
                State::Nothing => None, // Should never hit this
            };

            if let (Some((point_depth, directed_depth)), ShadowMode::Map(mode)) =
                (shadow_views, self.mode.shadow)
            {
                match ShadowMap::create_shadow_views(&self.device, (dims.x, dims.y), &mode) {
                    Ok((new_point_depth, new_directed_depth)) => {
                        *point_depth = new_point_depth;
                        *directed_depth = new_directed_depth;
                        // Recreate the shadow bind group if needed
                        if let State::Complete {
                            shadow:
                                Shadow {
                                    bind,
                                    map: ShadowMap::Enabled(shadow_map),
                                    ..
                                },
                            ..
                        } = &mut self.state
                        {
                            *bind = self.layouts.global.bind_shadow_textures(
                                &self.device,
                                &shadow_map.point_depth,
                                &shadow_map.directed_depth,
                            );
                        }
                    },
                    Err(err) => {
                        warn!("Could not create shadow map views: {:?}", err);
                    },
                }
            }
        } else {
            self.is_minimized = true;
        }

        Ok(())
    }

    pub fn maintain(&self) {
        if self.is_minimized {
            self.queue.submit(std::iter::empty());
        }

        self.device.poll(wgpu::Maintain::Poll)
    }

    /// Create render target views
    fn create_rt_views(
        device: &wgpu::Device,
        size: (u32, u32),
        mode: &RenderMode,
    ) -> Result<Views, RenderError> {
        let upscaled = Vec2::<u32>::from(size)
            .map(|e| (e as f32 * mode.upscale_mode.factor) as u32)
            .into_tuple();
        let (width, height, sample_count) = match mode.aa {
            AaMode::None | AaMode::Fxaa => (upscaled.0, upscaled.1, 1),
            AaMode::MsaaX4 => (upscaled.0, upscaled.1, 4),
            AaMode::MsaaX8 => (upscaled.0, upscaled.1, 8),
            AaMode::MsaaX16 => (upscaled.0, upscaled.1, 16),
        };
        let levels = 1;

        let color_view = || {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::RENDER_ATTACHMENT,
            });

            tex.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(wgpu::TextureFormat::Rgba16Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                // TODO: why is this not Color?
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
        };

        let tgt_color_view = color_view();
        let tgt_color_pp_view = color_view();

        let tgt_depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::RENDER_ATTACHMENT,
        });
        let tgt_depth_view = tgt_depth_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let win_depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        });
        // TODO: Consider no depth buffer for the final draw to the window?
        let win_depth_view = win_depth_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        Ok(Views {
            tgt_color: tgt_color_view,
            tgt_depth: tgt_depth_view,
            tgt_color_pp: tgt_color_pp_view,
            _win_depth: win_depth_view,
        })
    }

    /// Get the resolution of the render target.
    pub fn resolution(&self) -> Vec2<u32> { self.resolution }

    /// Get the resolution of the shadow render target.
    pub fn get_shadow_resolution(&self) -> (Vec2<u32>, Vec2<u32>) {
        match &self.state {
            State::Interface { shadow_views, .. } => shadow_views.as_ref().map(|s| (&s.0, &s.1)),
            State::Complete {
                shadow:
                    Shadow {
                        map: ShadowMap::Enabled(shadow_map),
                        ..
                    },
                ..
            } => Some((&shadow_map.point_depth, &shadow_map.directed_depth)),
            State::Complete { .. } | State::Nothing => None,
        }
        .map(|(point, directed)| (point.get_dimensions().xy(), directed.get_dimensions().xy()))
        .unwrap_or_else(|| (Vec2::new(1, 1), Vec2::new(1, 1)))
    }

    // TODO: Seamless is potentially the default with wgpu but we need further
    // investigation into whether this is actually turned on for the OpenGL
    // backend
    //
    /// NOTE: Supported by Vulkan (by default), DirectX 10+ (it seems--it's hard
    /// to find proof of this, but Direct3D 10 apparently does it by
    /// default, and 11 definitely does, so I assume it's natively supported
    /// by DirectX itself), OpenGL 3.2+, and Metal (done by default).  While
    /// there may be some GPUs that don't quite support it correctly, the
    /// impact is relatively small, so there is no reason not to enable it where
    /// available.
    //fn enable_seamless_cube_maps() {
    //todo!()
    // unsafe {
    //     // NOTE: Currently just fail silently rather than complain if the
    // computer is on     // a version lower than 3.2, where
    // seamless cubemaps were introduced.     if !device.get_info().
    // is_version_supported(3, 2) {         return;
    //     }

    //     // NOTE: Safe because GL_TEXTURE_CUBE_MAP_SEAMLESS is supported
    // by OpenGL 3.2+     // (see https://www.khronos.org/opengl/wiki/Cubemap_Texture#Seamless_cubemap);
    //     // enabling seamless cube maps should always be safe regardless
    // of the state of     // the OpenGL context, so no further
    // checks are needed.     device.with_gl(|gl| {
    //         gl.Enable(gfx_gl::TEXTURE_CUBE_MAP_SEAMLESS);
    //     });
    // }
    //}

    /// Start recording the frame
    /// When the returned `Drawer` is dropped the recorded draw calls will be
    /// submitted to the queue
    /// If there is an intermittent issue with the swap chain then Ok(None) will
    /// be returned
    pub fn start_recording_frame<'a>(
        &'a mut self,
        globals: &'a GlobalsBindGroup,
    ) -> Result<Option<drawer::Drawer<'a>>, RenderError> {
        span!(
            _guard,
            "start_recording_frame",
            "Renderer::start_recording_frame"
        );

        if self.is_minimized {
            return Ok(None);
        }

        // Try to get the latest profiling results
        if self.mode.profiler_enabled {
            // Note: this lags a few frames behind
            if let Some(profile_times) = self.profiler.process_finished_frame() {
                self.profile_times = profile_times;
            }
        }

        // Handle polling background pipeline creation/recreation
        // Temporarily set to nothing and then replace in the statement below
        let state = core::mem::replace(&mut self.state, State::Nothing);
        // If still creating initial pipelines, check if complete
        self.state = if let State::Interface {
            pipelines: interface,
            shadow_views,
            creating,
        } = state
        {
            match creating.try_complete() {
                Ok(pipelines) => {
                    let IngameAndShadowPipelines { ingame, shadow } = pipelines;

                    let pipelines = Pipelines::consolidate(interface, ingame);

                    let shadow_map = ShadowMap::new(
                        &self.device,
                        &self.queue,
                        shadow.point,
                        shadow.directed,
                        shadow.figure,
                        shadow_views,
                    );

                    let shadow_bind = {
                        let (point, directed) = shadow_map.textures();
                        self.layouts
                            .global
                            .bind_shadow_textures(&self.device, point, directed)
                    };

                    let shadow = Shadow {
                        map: shadow_map,
                        bind: shadow_bind,
                    };

                    State::Complete {
                        pipelines,
                        shadow,
                        recreating: None,
                    }
                },
                // Not complete
                Err(creating) => State::Interface {
                    pipelines: interface,
                    shadow_views,
                    creating,
                },
            }
        // If recreating the pipelines, check if that is complete
        } else if let State::Complete {
            pipelines,
            mut shadow,
            recreating: Some(recreating),
        } = state
        {
            match recreating.try_complete() {
                Ok(Ok((pipelines, shadow_pipelines))) => {
                    if let (
                        Some(point_pipeline),
                        Some(terrain_directed_pipeline),
                        Some(figure_directed_pipeline),
                        ShadowMap::Enabled(shadow_map),
                    ) = (
                        shadow_pipelines.point,
                        shadow_pipelines.directed,
                        shadow_pipelines.figure,
                        &mut shadow.map,
                    ) {
                        shadow_map.point_pipeline = point_pipeline;
                        shadow_map.terrain_directed_pipeline = terrain_directed_pipeline;
                        shadow_map.figure_directed_pipeline = figure_directed_pipeline;
                    }
                    State::Complete {
                        pipelines,
                        shadow,
                        recreating: None,
                    }
                },
                Ok(Err(e)) => {
                    error!(?e, "Could not recreate shaders from assets due to an error");
                    State::Complete {
                        pipelines,
                        shadow,
                        recreating: None,
                    }
                },
                // Not complete
                Err(recreating) => State::Complete {
                    pipelines,
                    shadow,
                    recreating: Some(recreating),
                },
            }
        } else {
            state
        };

        // If the shaders files were changed attempt to recreate the shaders
        if self.shaders.reloaded() {
            self.recreate_pipelines();
        }

        // Or if we have a recreation pending
        if self.recreation_pending
            && matches!(&self.state, State::Complete { recreating, .. } if recreating.is_none())
        {
            self.recreation_pending = false;
            self.recreate_pipelines();
        }

        let tex = match self.swap_chain.get_current_frame() {
            Ok(frame) => frame.output,
            // If lost recreate the swap chain
            Err(err @ wgpu::SwapChainError::Lost) => {
                warn!("{}. Recreating swap chain. A frame will be missed", err);
                return self.on_resize(self.resolution).map(|()| None);
            },
            Err(wgpu::SwapChainError::Timeout) => {
                // This will probably be resolved on the next frame
                // NOTE: we don't log this because it happens very frequently with
                // PresentMode::Fifo and unlimited FPS on certain machines
                return Ok(None);
            },
            Err(err @ wgpu::SwapChainError::Outdated) => {
                warn!("{}. Recreating the swapchain", err);
                self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
                return Ok(None);
            },
            Err(err @ wgpu::SwapChainError::OutOfMemory) => return Err(err.into()),
        };
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("A render encoder"),
            });

        Ok(Some(drawer::Drawer::new(encoder, self, tex, globals)))
    }

    /// Recreate the pipelines
    fn recreate_pipelines(&mut self) {
        match &mut self.state {
            State::Complete { recreating, .. } if recreating.is_some() => {
                // Defer recreation so that we are not building multiple sets of pipelines in
                // the background at once
                self.recreation_pending = true;
            },
            State::Complete {
                recreating, shadow, ..
            } => {
                *recreating = Some(pipeline_creation::recreate_pipelines(
                    Arc::clone(&self.device),
                    Arc::clone(&self.layouts),
                    self.shaders.read().clone(),
                    self.mode.clone(),
                    self.sc_desc.clone(), // Note: cheap clone
                    shadow.map.is_enabled(),
                ));
            },
            State::Interface { .. } => {
                // Defer recreation so that we are not building multiple sets of pipelines in
                // the background at once
                self.recreation_pending = true;
            },
            State::Nothing => {},
        }
    }

    /// Create a new set of constants with the provided values.
    pub fn create_consts<T: Copy + bytemuck::Pod>(&mut self, vals: &[T]) -> Consts<T> {
        Self::create_consts_inner(&self.device, &self.queue, vals)
    }

    pub fn create_consts_inner<T: Copy + bytemuck::Pod>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vals: &[T],
    ) -> Consts<T> {
        let mut consts = Consts::new(device, vals.len());
        consts.update(queue, vals, 0);
        consts
    }

    /// Update a set of constants with the provided values.
    pub fn update_consts<T: Copy + bytemuck::Pod>(&self, consts: &mut Consts<T>, vals: &[T]) {
        consts.update(&self.queue, vals, 0)
    }

    pub fn update_clouds_locals(&mut self, new_val: clouds::Locals) {
        self.locals.clouds.update(&self.queue, &[new_val], 0)
    }

    pub fn update_postprocess_locals(&mut self, new_val: postprocess::Locals) {
        self.locals.postprocess.update(&self.queue, &[new_val], 0)
    }

    /// Create a new set of instances with the provided values.
    pub fn create_instances<T: Copy + bytemuck::Pod>(
        &mut self,
        vals: &[T],
    ) -> Result<Instances<T>, RenderError> {
        let mut instances = Instances::new(&self.device, vals.len());
        instances.update(&self.queue, vals, 0);
        Ok(instances)
    }

    /// Ensure that the quad index buffer is large enough for a quad vertex
    /// buffer with this many vertices
    pub(super) fn ensure_sufficient_index_length<V: Vertex>(
        &mut self,
        // Length of the vert buffer with 4 verts per quad
        vert_length: usize,
    ) {
        let quad_index_length = vert_length / 4 * 6;

        match V::QUADS_INDEX {
            Some(wgpu::IndexFormat::Uint16) => {
                // Make sure the global quad index buffer is large enough
                if self.quad_index_buffer_u16.len() < quad_index_length {
                    // Make sure we aren't over the max
                    if vert_length > u16::MAX as usize {
                        panic!(
                            "Vertex type: {} needs to use a larger index type, length: {}",
                            core::any::type_name::<V>(),
                            vert_length
                        );
                    }
                    self.quad_index_buffer_u16 =
                        create_quad_index_buffer_u16(&self.device, vert_length);
                }
            },
            Some(wgpu::IndexFormat::Uint32) => {
                // Make sure the global quad index buffer is large enough
                if self.quad_index_buffer_u32.len() < quad_index_length {
                    // Make sure we aren't over the max
                    if vert_length > u32::MAX as usize {
                        panic!(
                            "More than u32::MAX({}) verts({}) for type({}) using an index buffer!",
                            u32::MAX,
                            vert_length,
                            core::any::type_name::<V>()
                        );
                    }
                    self.quad_index_buffer_u32 =
                        create_quad_index_buffer_u32(&self.device, vert_length);
                }
            },
            None => {},
        }
    }

    pub fn create_sprite_verts(&mut self, mesh: Mesh<sprite::Vertex>) -> sprite::SpriteVerts {
        self.ensure_sufficient_index_length::<sprite::Vertex>(sprite::VERT_PAGE_SIZE as usize);
        sprite::create_verts_buffer(&self.device, mesh)
    }

    /// Create a new model from the provided mesh.
    /// If the provided mesh is empty this returns None
    pub fn create_model<V: Vertex>(&mut self, mesh: &Mesh<V>) -> Option<Model<V>> {
        self.ensure_sufficient_index_length::<V>(mesh.vertices().len());
        Model::new(&self.device, mesh)
    }

    /// Create a new dynamic model with the specified size.
    pub fn create_dynamic_model<V: Vertex>(&mut self, size: usize) -> DynamicModel<V> {
        DynamicModel::new(&self.device, size)
    }

    /// Update a dynamic model with a mesh and a offset.
    pub fn update_model<V: Vertex>(&self, model: &DynamicModel<V>, mesh: &Mesh<V>, offset: usize) {
        model.update(&self.queue, mesh, offset)
    }

    /// Return the maximum supported texture size.
    pub fn max_texture_size(&self) -> u32 { Self::max_texture_size_raw(&self.device) }

    /// Return the maximum supported texture size from the factory.
    fn max_texture_size_raw(_device: &wgpu::Device) -> u32 {
        // This value is temporary as there are plans to include a way to get this in
        // wgpu this is just a sane standard for now
        8192
    }

    /// Create a new immutable texture from the provided image.
    /// # Panics
    /// If the provided data doesn't completely fill the texture this function
    /// will panic.
    pub fn create_texture_with_data_raw(
        &mut self,
        texture_info: &wgpu::TextureDescriptor,
        view_info: &wgpu::TextureViewDescriptor,
        sampler_info: &wgpu::SamplerDescriptor,
        data: &[u8],
    ) -> Texture {
        let tex = Texture::new_raw(&self.device, &texture_info, &view_info, &sampler_info);

        let size = texture_info.size;
        let block_size = texture_info.format.describe().block_size;
        assert_eq!(
            size.width as usize
                * size.height as usize
                * size.depth_or_array_layers as usize
                * block_size as usize,
            data.len(),
            "Provided data length {} does not fill the provided texture size {:?}",
            data.len(),
            size,
        );

        tex.update(
            &self.queue,
            [0; 2],
            [texture_info.size.width, texture_info.size.height],
            data,
        );

        tex
    }

    /// Create a new raw texture.
    pub fn create_texture_raw(
        &mut self,
        texture_info: &wgpu::TextureDescriptor,
        view_info: &wgpu::TextureViewDescriptor,
        sampler_info: &wgpu::SamplerDescriptor,
    ) -> Texture {
        let texture = Texture::new_raw(&self.device, texture_info, view_info, sampler_info);
        texture.clear(&self.queue); // Needs to be fully initialized for partial writes to work on Dx12 AMD
        texture
    }

    /// Create a new texture from the provided image.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn create_texture(
        &mut self,
        image: &image::DynamicImage,
        filter_method: Option<FilterMode>,
        address_mode: Option<AddressMode>,
    ) -> Result<Texture, RenderError> {
        Texture::new(
            &self.device,
            &self.queue,
            image,
            filter_method,
            address_mode,
        )
    }

    /// Create a new dynamic texture with the
    /// specified dimensions.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn create_dynamic_texture(&mut self, dims: Vec2<u32>) -> Texture {
        Texture::new_dynamic(&self.device, &self.queue, dims.x, dims.y)
    }

    /// Update a texture with the provided offset, size, and data.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn update_texture(
        &mut self,
        texture: &Texture, /* <T> */
        offset: [u32; 2],
        size: [u32; 2],
        // TODO: be generic over pixel type
        data: &[[u8; 4]],
    ) {
        texture.update(&self.queue, offset, size, bytemuck::cast_slice(data))
    }

    /// Queue to obtain a screenshot on the next frame render
    pub fn create_screenshot(
        &mut self,
        screenshot_handler: impl FnOnce(image::DynamicImage) + Send + 'static,
    ) {
        // Queue screenshot
        self.take_screenshot = Some(Box::new(screenshot_handler));
        // Take profiler snapshot
        if self.mode.profiler_enabled {
            let file_name = format!(
                "frame-trace_{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            );

            if let Err(err) = wgpu_profiler::chrometrace::write_chrometrace(
                std::path::Path::new(&file_name),
                &self.profile_times,
            ) {
                error!(?err, "Failed to save GPU timing snapshot");
            } else {
                info!("Saved GPU timing snapshot as: {}", file_name);
            }
        }
    }

    // Consider reenabling at some time
    //
    // /// Queue the rendering of the player silhouette in the upcoming frame.
    // pub fn render_player_shadow(
    //     &mut self,
    //     _model: &figure::FigureModel,
    //     _col_lights: &Texture<ColLightFmt>,
    //     _global: &GlobalModel,
    //     _bones: &Consts<figure::BoneData>,
    //     _lod: &lod_terrain::LodData,
    //     _locals: &Consts<shadow::Locals>,
    // ) {
    //     // FIXME: Consider reenabling at some point.
    //     /* let (point_shadow_maps, directed_shadow_maps) =
    //         if let Some(shadow_map) = &mut self.shadow_map {
    //             (
    //                 (
    //                     shadow_map.point_res.clone(),
    //                     shadow_map.point_sampler.clone(),
    //                 ),
    //                 (
    //                     shadow_map.directed_res.clone(),
    //                     shadow_map.directed_sampler.clone(),
    //                 ),
    //             )
    //         } else {
    //             (
    //                 (self.noise_tex.srv.clone(), self.noise_tex.sampler.clone()),
    //                 (self.noise_tex.srv.clone(), self.noise_tex.sampler.clone()),
    //             )
    //         };
    //     let model = &model.opaque;

    //     self.encoder.draw(
    //         &gfx::Slice {
    //             start: model.vertex_range().start,
    //             end: model.vertex_range().end,
    //             base_vertex: 0,
    //             instances: None,
    //             buffer: gfx::IndexBuffer::Auto,
    //         },
    //         &self.player_shadow_pipeline.pso,
    //         &figure::pipe::Data {
    //             vbuf: model.vbuf.clone(),
    //             col_lights: (col_lights.srv.clone(), col_lights.sampler.clone()),
    //             locals: locals.buf.clone(),
    //             globals: global.globals.buf.clone(),
    //             bones: bones.buf.clone(),
    //             lights: global.lights.buf.clone(),
    //             shadows: global.shadows.buf.clone(),
    //             light_shadows: global.shadow_mats.buf.clone(),
    //             point_shadow_maps,
    //             directed_shadow_maps,
    //             noise: (self.noise_tex.srv.clone(),
    // self.noise_tex.sampler.clone()),             alt: (lod.alt.srv.clone(),
    // lod.alt.sampler.clone()),             horizon: (lod.horizon.srv.clone(),
    // lod.horizon.sampler.clone()),             tgt_color:
    // self.tgt_color_view.clone(),             tgt_depth:
    // (self.tgt_depth_view.clone()/* , (0, 0) */),         },
    //     ); */
    // }
}

fn create_quad_index_buffer_u16(device: &wgpu::Device, vert_length: usize) -> Buffer<u16> {
    assert!(vert_length <= u16::MAX as usize);
    let indices = [0, 1, 2, 2, 1, 3]
        .iter()
        .cycle()
        .copied()
        .take(vert_length / 4 * 6)
        .enumerate()
        .map(|(i, b)| (i / 6 * 4 + b) as u16)
        .collect::<Vec<_>>();

    Buffer::new(device, wgpu::BufferUsage::INDEX, &indices)
}

fn create_quad_index_buffer_u32(device: &wgpu::Device, vert_length: usize) -> Buffer<u32> {
    assert!(vert_length <= u32::MAX as usize);
    let indices = [0, 1, 2, 2, 1, 3]
        .iter()
        .cycle()
        .copied()
        .take(vert_length / 4 * 6)
        .enumerate()
        .map(|(i, b)| (i / 6 * 4 + b) as u32)
        .collect::<Vec<_>>();

    Buffer::new(device, wgpu::BufferUsage::INDEX, &indices)
}
