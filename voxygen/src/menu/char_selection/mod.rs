mod ui;

use crate::{
    render::Renderer,
    scene::simple::{self as scene, Scene},
    session::SessionState,
    settings::Settings,
    window::Event as WinEvent,
    Direction, GlobalState, PlayState, PlayStateResult,
};
use client::{self, Client};
use common::{comp, resources::DeltaTime};
use common_base::span;
use specs::WorldExt;
use std::{cell::RefCell, mem, rc::Rc};
use tracing::error;
use ui::CharSelectionUi;

pub struct CharSelectionState {
    char_selection_ui: CharSelectionUi,
    client: Rc<RefCell<Client>>,
    scene: Scene,
    need_shadow_clear: bool,
}

impl CharSelectionState {
    /// Create a new `CharSelectionState`.
    pub fn new(global_state: &mut GlobalState, client: Rc<RefCell<Client>>) -> Self {
        let scene = Scene::new(
            global_state.window.renderer_mut(),
            Some("fixture.selection_bg"),
            &*client.borrow(),
        );
        let char_selection_ui = CharSelectionUi::new(global_state, &*client.borrow());

        Self {
            char_selection_ui,
            client,
            scene,
            need_shadow_clear: false,
        }
    }

    fn get_humanoid_body_inventory<'a>(
        char_selection_ui: &'a CharSelectionUi,
        client: &'a Client,
    ) -> (
        Option<comp::humanoid::Body>,
        Option<&'a comp::inventory::Inventory>,
    ) {
        char_selection_ui
            .display_body_inventory(&client.character_list().characters)
            .map(|(body, inventory)| {
                (
                    match body {
                        comp::Body::Humanoid(body) => Some(body),
                        _ => None,
                    },
                    Some(inventory),
                )
            })
            .unwrap_or_default()
    }
}

impl PlayState for CharSelectionState {
    fn enter(&mut self, global_state: &mut GlobalState, _: Direction) {
        // Load the player's character list
        self.client.borrow_mut().load_character_list();

        // Updated localization in case the selected language was changed
        self.char_selection_ui.update_language(global_state.i18n);
        // Set scale mode in case it was change
        self.char_selection_ui
            .set_scale_mode(global_state.settings.interface.ui_scale);

        // Clear shadow textures since we don't render to them here
        self.need_shadow_clear = true;
    }

    fn tick(&mut self, global_state: &mut GlobalState, events: Vec<WinEvent>) -> PlayStateResult {
        span!(_guard, "tick", "<CharSelectionState as PlayState>::tick");
        let (client_presence, client_registered) = {
            let client = self.client.borrow();
            (client.presence(), client.registered())
        };
        if client_presence.is_none() && client_registered {
            // Handle window events
            for event in events {
                if self.char_selection_ui.handle_event(event.clone()) {
                    continue;
                }
                match event {
                    WinEvent::Close => {
                        return PlayStateResult::Shutdown;
                    },
                    // Pass all other events to the scene
                    event => {
                        self.scene.handle_input_event(event);
                    }, // TODO: Do something if the event wasn't handled?
                }
            }

            // Maintain the UI.
            let events = self
                .char_selection_ui
                .maintain(global_state, &self.client.borrow());

            for event in events {
                match event {
                    ui::Event::Logout => {
                        return PlayStateResult::Pop;
                    },
                    ui::Event::AddCharacter { alias, tool, body } => {
                        self.client
                            .borrow_mut()
                            .create_character(alias, Some(tool), body);
                    },
                    ui::Event::DeleteCharacter(character_id) => {
                        self.client.borrow_mut().delete_character(character_id);
                    },
                    ui::Event::Play(character_id) => {
                        {
                            let mut c = self.client.borrow_mut();
                            c.request_character(character_id);
                            //Send our ViewDistance
                            c.set_view_distance(global_state.settings.graphics.view_distance);
                        }
                        return PlayStateResult::Switch(Box::new(SessionState::new(
                            global_state,
                            Rc::clone(&self.client),
                        )));
                    },
                    ui::Event::ClearCharacterListError => {
                        self.char_selection_ui.error = None;
                    },
                    ui::Event::SelectCharacter(selected) => {
                        let client = self.client.borrow();
                        let server_name = &client.server_info().name;
                        // Select newly created character
                        global_state
                            .profile
                            .set_selected_character(server_name, selected);
                        global_state.profile.save_to_file_warn();
                    },
                }
            }

            // Maintain the scene.
            {
                let client = self.client.borrow();
                let (humanoid_body, loadout) =
                    Self::get_humanoid_body_inventory(&self.char_selection_ui, &client);

                // Maintain the scene.
                let scene_data = scene::SceneData {
                    time: client.state().get_time(),
                    delta_time: client.state().ecs().read_resource::<DeltaTime>().0,
                    tick: client.get_tick(),
                    slow_job_pool: &client.state().slow_job_pool(),
                    body: humanoid_body,
                    gamma: global_state.settings.graphics.gamma,
                    exposure: global_state.settings.graphics.exposure,
                    ambiance: global_state.settings.graphics.ambiance,
                    mouse_smoothing: global_state.settings.gameplay.smooth_pan_enable,
                    figure_lod_render_distance: global_state
                        .settings
                        .graphics
                        .figure_lod_render_distance
                        as f32,
                };

                self.scene
                    .maintain(global_state.window.renderer_mut(), scene_data, loadout);
            }

            // Tick the client (currently only to keep the connection alive).
            let localized_strings = &global_state.i18n.read();

            match self.client.borrow_mut().tick(
                comp::ControllerInputs::default(),
                global_state.clock.dt(),
                |_| {},
            ) {
                Ok(events) => {
                    for event in events {
                        match event {
                            client::Event::SetViewDistance(vd) => {
                                global_state.settings.graphics.view_distance = vd;
                                global_state.settings.save_to_file_warn();
                            },
                            client::Event::Disconnect => {
                                global_state.info_message = Some(
                                    localized_strings
                                        .get("main.login.server_shut_down")
                                        .to_owned(),
                                );
                                return PlayStateResult::Pop;
                            },
                            client::Event::CharacterCreated(character_id) => {
                                self.char_selection_ui.select_character(character_id);
                            },
                            client::Event::CharacterError(error) => {
                                global_state.client_error = Some(error);
                            },
                            _ => {},
                        }
                    }
                },
                Err(err) => {
                    global_state.info_message =
                        Some(localized_strings.get("common.connection_lost").to_owned());
                    error!(?err, "[char_selection] Failed to tick the client");
                    return PlayStateResult::Pop;
                },
            }

            if let Some(error) = mem::take(&mut global_state.client_error) {
                self.char_selection_ui.display_error(error);
            }

            // TODO: make sure rendering is not relying on cleaned up stuff
            self.client.borrow_mut().cleanup();

            PlayStateResult::Continue
        } else {
            error!("Client not in pending or registered state. Popping char selection play state");
            // TODO set global_state.info_message
            PlayStateResult::Pop
        }
    }

    fn name(&self) -> &'static str { "Character Selection" }

    fn capped_fps(&self) -> bool { true }

    fn render(&mut self, renderer: &mut Renderer, _: &Settings) {
        let mut drawer = match renderer
            .start_recording_frame(self.scene.global_bind_group())
            .expect("Unrecoverable render error when starting a new frame!")
        {
            Some(d) => d,
            // Couldn't get swap chain texture this fime
            None => return,
        };

        if self.need_shadow_clear {
            drawer.clear_shadows();
            self.need_shadow_clear = false;
        }

        let client = self.client.borrow();
        let (humanoid_body, loadout) =
            Self::get_humanoid_body_inventory(&self.char_selection_ui, &client);

        if let Some(mut first_pass) = drawer.first_pass() {
            self.scene
                .render(&mut first_pass, client.get_tick(), humanoid_body, loadout);
        }

        // Clouds
        if let Some(mut second_pass) = drawer.second_pass() {
            second_pass.draw_clouds();
        }
        // PostProcess and UI
        let mut third_pass = drawer.third_pass();
        third_pass.draw_postprocess();
        // Draw the UI to the screen.
        if let Some(mut ui_drawer) = third_pass.draw_ui() {
            self.char_selection_ui.render(&mut ui_drawer);
        };
    }
}
