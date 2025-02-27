use crate::{column::ColumnSample, sim::SimChunk, IndexRef, CONFIG};
use common::{
    comp::{
        biped_large, bird_large, bird_medium, fish_medium, fish_small, quadruped_low,
        quadruped_medium, quadruped_small, theropod, Alignment,
    },
    generation::{ChunkSupplement, EntityInfo},
    resources::TimeOfDay,
    terrain::Block,
    time::DayPeriod::{self, Evening, Morning, Night, Noon},
    vol::{BaseVol, ReadVol, RectSizedVol, WriteVol},
};
use rand::prelude::*;
use std::{f32, ops::Range};
use vek::*;

fn close(x: f32, tgt: f32, falloff: f32) -> f32 {
    (1.0 - (x - tgt).abs() / falloff).max(0.0).powf(0.125)
}

const BASE_DENSITY: f32 = 1.0e-5; // Base wildlife density

#[allow(clippy::eval_order_dependence)]
pub fn apply_wildlife_supplement<'a, R: Rng>(
    // NOTE: Used only for dynamic elements like chests and entities!
    dynamic_rng: &mut R,
    wpos2d: Vec2<i32>,
    mut get_column: impl FnMut(Vec2<i32>) -> Option<&'a ColumnSample<'a>>,
    vol: &(impl BaseVol<Vox = Block> + RectSizedVol + ReadVol + WriteVol),
    _index: IndexRef,
    chunk: &SimChunk,
    supplement: &mut ChunkSupplement,
    time: Option<TimeOfDay>,
) {
    struct Entry<R> {
        make_entity: fn(Vec3<f32>, &mut R) -> EntityInfo, // Entity
        group_size: Range<usize>,                         // Group size range
        is_underwater: bool,                              // Underwater?
        day_period: Vec<DayPeriod>,                       // Period of the day
        get_density: fn(&SimChunk, &ColumnSample) -> f32, // Density
    }

    let scatter: &[Entry<R>] = &[
        // Tundra snow pack ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Frostfang,
                        )
                        .into(),
                        1 => {
                            theropod::Body::random_with(rng, &theropod::Species::Snowraptor).into()
                        },
                        _ => quadruped_medium::Body {
                            species: quadruped_medium::Species::Roshwalr,
                            body_type: quadruped_medium::BodyType::Male,
                        }
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..4,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp, 0.3)
                    * BASE_DENSITY
                    * col.snow_cover as i32 as f32
                    * 1.0
            },
        },
        // Tundra solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..4) {
                        0 => {
                            theropod::Body::random_with(rng, &theropod::Species::Snowraptor).into()
                        },
                        1 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Snowleopard,
                        )
                        .into(),
                        2 => theropod::Body::random_with(rng, &theropod::Species::Yale).into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Grolgar,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp, 0.3) * col.tree_density * BASE_DENSITY * 1.4
            },
        },
        // Tundra rare solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        theropod::Body::random_with(rng, &theropod::Species::Snowraptor).into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| close(c.temp, CONFIG.snow_temp, 0.15) * BASE_DENSITY * 0.5,
        },
        // Tundra rarer solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..2) {
                        0 => biped_large::Body::random_with(rng, &biped_large::Species::Wendigo)
                            .into(),
                        _ => biped_large::Body::random_with(
                            rng,
                            &biped_large::Species::Mountaintroll,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| close(c.temp, CONFIG.snow_temp, 0.15) * BASE_DENSITY * 0.1,
        },
        // Tundra rock solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_low::Body::random_with(rng, &quadruped_low::Species::Rocksnapper)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp, 0.15) * BASE_DENSITY * col.rock * 1.0
            },
        },
        // Taiga rare solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..2) {
                        0 => biped_large::Body::random_with(rng, &biped_large::Species::Wendigo)
                            .into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Dreadhorn,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp + 0.2, 0.2) * col.tree_density * BASE_DENSITY * 0.4
            },
        },
        // Taiga pack ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_medium::Body::random_with(rng, &quadruped_medium::Species::Wolf)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 3..8,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp + 0.2, 0.6) * col.tree_density * BASE_DENSITY * 0.9
            },
        },
        // Taiga pack wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Mouflon,
                        )
                        .into(),
                        1 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Yak,
                        )
                        .into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Highland,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..4,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| close(c.temp, CONFIG.snow_temp + 0.2, 0.2) * BASE_DENSITY * 1.0,
        },
        // Taiga solitary wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..6) {
                        0 => {
                            bird_medium::Body::random_with(rng, &bird_medium::Species::Eagle).into()
                        },
                        1 => bird_medium::Body::random_with(rng, &bird_medium::Species::Owl).into(),
                        2 => quadruped_small::Body {
                            species: quadruped_small::Species::Fox,
                            body_type: quadruped_small::BodyType::Female,
                        }
                        .into(),
                        3 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Moose,
                        )
                        .into(),
                        4 => {
                            quadruped_small::Body {
                                species: quadruped_small::Species::Hare,
                                body_type: quadruped_small::BodyType::Female,
                            }
                        }
                        .into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Tuskram,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| close(c.temp, CONFIG.snow_temp + 0.2, 0.6) * BASE_DENSITY * 5.0,
        },
        // Temperate solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..5) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Tarasque,
                        )
                        .into(),
                        1 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Bear,
                        )
                        .into(),
                        2 => {
                            theropod::Body::random_with(rng, &theropod::Species::Woodraptor).into()
                        },
                        3 => {
                            quadruped_low::Body::random_with(rng, &quadruped_low::Species::Deadwood)
                                .into()
                        },
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Saber,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.temperate_temp + 0.1, 0.5)
                    * col.tree_density
                    * BASE_DENSITY
                    * 1.0
            },
        },
        // Temperate pack wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..12) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Deer,
                        )
                        .into(),
                        1 => {
                            quadruped_small::Body::random_with(rng, &quadruped_small::Species::Rat)
                                .into()
                        },
                        2 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Rabbit,
                        )
                        .into(),
                        3 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Jackalope,
                        )
                        .into(),
                        4 => {
                            quadruped_small::Body::random_with(rng, &quadruped_small::Species::Boar)
                                .into()
                        },
                        5 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Sheep,
                        )
                        .into(),
                        6 => {
                            quadruped_small::Body::random_with(rng, &quadruped_small::Species::Pig)
                                .into()
                        },
                        7 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Squirrel,
                        )
                        .into(),
                        8 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Horse,
                        )
                        .into(),
                        9 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Cattle,
                        )
                        .into(),
                        10 => {
                            quadruped_small::Body::random_with(rng, &quadruped_small::Species::Goat)
                                .into()
                        },
                        _ => bird_medium::Body::random_with(rng, &bird_medium::Species::Chicken)
                            .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..8,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.temperate_temp + 0.1, 0.6)
                    * close(c.humidity, CONFIG.forest_hum, 0.6)
                    * BASE_DENSITY
                    * 4.0
            },
        },
        // Temperate solitary wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..10) {
                        0 => quadruped_small::Body {
                            species: quadruped_small::Species::Fox,
                            body_type: quadruped_small::BodyType::Male,
                        }
                        .into(),
                        1 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Donkey,
                        )
                        .into(),
                        2 => {
                            bird_medium::Body::random_with(rng, &bird_medium::Species::Goose).into()
                        },
                        3 => bird_medium::Body::random_with(rng, &bird_medium::Species::Peacock)
                            .into(),
                        4 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Skunk,
                        )
                        .into(),
                        5 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Raccoon,
                        )
                        .into(),
                        6 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Catoblepas,
                        )
                        .into(),
                        7 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Turtle,
                        )
                        .into(),
                        8 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Hirdrasil,
                        )
                        .into(),
                        _ => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Truffler,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.temperate_temp + 0.1, 0.6)
                    * BASE_DENSITY
                    * close(c.humidity, CONFIG.forest_hum, 0.6)
                    * 8.0
            },
        },
        // Temperate solitary wild night
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_small::Body::random_with(rng, &quadruped_small::Species::Batfox)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night],
            get_density: |c, _col| {
                close(c.temp, CONFIG.temperate_temp + 0.1, 0.6)
                    * BASE_DENSITY
                    * close(c.humidity, CONFIG.forest_hum, 0.6)
                    * 0.8
            },
        },
        // Rare temperate solitary enemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => {
                            biped_large::Body::random_with(rng, &biped_large::Species::Ogre).into()
                        },
                        1 => biped_large::Body::random_with(rng, &biped_large::Species::Swamptroll)
                            .into(),
                        _ => biped_large::Body::random_with(rng, &biped_large::Species::Cyclops)
                            .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| close(c.temp, CONFIG.temperate_temp, 0.8) * BASE_DENSITY * 0.08,
        },
        // Temperate river wildlife
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..4) {
                        0 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Beaver,
                        )
                        .into(),
                        1 => quadruped_low::Body {
                            species: quadruped_low::Species::Salamander,
                            body_type: quadruped_low::BodyType::Female,
                        }
                        .into(),
                        2 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Kelpie,
                        )
                        .into(),
                        _ => {
                            bird_medium::Body::random_with(rng, &bird_medium::Species::Duck).into()
                        },
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.temperate_temp, 0.6)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.001
                    } else {
                        0.0
                    }
            },
        },
        // Temperate rare river ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Kelpie,
                        )
                        .into(),
                    )
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.temperate_temp, 0.6)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.00005
                    } else {
                        0.0
                    }
            },
        },
        // Temperate river ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_low::Body::random_with(rng, &quadruped_low::Species::Hakulaq)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.temperate_temp, 0.6)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.0001
                    } else {
                        0.0
                    }
            },
        },
        // Tropical rock solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Dodarock,
                        )
                        .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.tropical_temp + 0.1, 0.5) * col.rock * BASE_DENSITY * 5.0
            },
        },
        // Jungle solitary ennemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => {
                            quadruped_low::Body::random_with(rng, &quadruped_low::Species::Maneater)
                                .into()
                        },
                        1 => quadruped_low::Body::random_with(rng, &quadruped_low::Species::Asp)
                            .into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Tiger,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.2, 0.2)
                    * close(c.humidity, CONFIG.jungle_hum, 0.2)
                    * BASE_DENSITY
                    * 2.8
            },
        },
        // Jungle solitary ennemies day
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        theropod::Body::random_with(rng, &theropod::Species::Sunlizard).into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.2, 0.2)
                    * close(c.humidity, CONFIG.jungle_hum, 0.2)
                    * BASE_DENSITY
                    * 0.5
            },
        },
        // Jungle rare solitary wild day
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..5) {
                        0 => theropod::Body::random_with(rng, &theropod::Species::Odonto).into(),
                        1 => {
                            biped_large::Body::random_with(rng, &biped_large::Species::Mightysaurok)
                                .into()
                        },
                        2 => {
                            biped_large::Body::random_with(rng, &biped_large::Species::Occultsaurok)
                                .into()
                        },
                        3 => bird_large::Body::random_with(rng, &bird_large::Species::Cockatrice)
                            .into(),
                        _ => biped_large::Body::random_with(rng, &biped_large::Species::Slysaurok)
                            .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.2, 0.2)
                    * close(c.humidity, CONFIG.jungle_hum, 0.2)
                    * BASE_DENSITY
                    * 0.8
            },
        },
        // Jungle solitary wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => bird_medium::Body::random_with(rng, &bird_medium::Species::Parrot)
                            .into(),

                        1 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Quokka,
                        )
                        .into(),
                        _ => {
                            quadruped_low::Body::random_with(rng, &quadruped_low::Species::Tortoise)
                                .into()
                        },
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.2, 0.3)
                    * close(c.humidity, CONFIG.jungle_hum, 0.2)
                    * BASE_DENSITY
                    * 8.0
            },
        },
        // Jungle solitary wild day
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_low::Body::random_with(rng, &quadruped_low::Species::Monitor)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.2, 0.3)
                    * close(c.humidity, CONFIG.jungle_hum, 0.2)
                    * BASE_DENSITY
                    * 2.0
            },
        },
        // Tropical rare river enemy
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_low::Body::random_with(rng, &quadruped_low::Species::Alligator)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..3,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.tropical_temp + 0.2, 0.5)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.0001
                    } else {
                        0.0
                    }
            },
        },
        // Tropical rare river wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => {
                            quadruped_small::Body::random_with(rng, &quadruped_small::Species::Frog)
                                .into()
                        },
                        1 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Axolotl,
                        )
                        .into(),
                        _ => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Fungome,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..3,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.tropical_temp, 0.5)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.001
                    } else {
                        0.0
                    }
            },
        },
        // Tropical pack enemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..2) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Lion,
                        )
                        .into(),
                        _ => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Hyena,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..3,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.1, 0.4)
                    * close(c.humidity, CONFIG.desert_hum, 0.4)
                    * BASE_DENSITY
                    * 2.0
            },
        },
        // Desert pack wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..2) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Zebra,
                        )
                        .into(),
                        _ => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Antelope,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 3..7,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.tropical_temp + 0.1, 0.4)
                    * close(c.humidity, CONFIG.desert_hum, 0.4)
                    * BASE_DENSITY
                    * 0.8
            },
        },
        // Desert solitary enemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Bonerattler,
                        )
                        .into(),
                        1 => {
                            theropod::Body::random_with(rng, &theropod::Species::Sandraptor).into()
                        },
                        _ => quadruped_low::Body::random_with(
                            rng,
                            &quadruped_low::Species::Sandshark,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.desert_temp + 0.2, 0.3)
                    * close(c.humidity, CONFIG.desert_hum, 0.5)
                    * BASE_DENSITY
                    * 1.3
            },
        },
        // Desert rare solitary enemies
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => quadruped_low::Body::random_with(
                            rng,
                            &quadruped_low::Species::Lavadrake,
                        )
                        .into(),
                        1 => theropod::Body::random_with(rng, &theropod::Species::Ntouka).into(),
                        _ => theropod::Body::random_with(rng, &theropod::Species::Archaeos).into(),
                    })
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.desert_temp + 0.2, 0.3)
                    * close(c.humidity, CONFIG.desert_hum, 0.5)
                    * BASE_DENSITY
                    * 0.15
            },
        },
        // Desert river solitary enemy
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_low::Body::random_with(rng, &quadruped_low::Species::Crocodile)
                            .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..3,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |_c, col| {
                close(col.temp, CONFIG.desert_temp + 0.2, 0.3)
                    * if col.water_dist.map(|d| d < 10.0).unwrap_or(false) {
                        0.0001
                    } else {
                        0.0
                    }
            },
        },
        // Desert secret solitary enemy
        Entry {
            make_entity: |pos, _rng| {
                EntityInfo::at(pos)
                    .with_body(
                        quadruped_medium::Body {
                            species: quadruped_medium::Species::Roshwalr,
                            body_type: quadruped_medium::BodyType::Female,
                        }
                        .into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..3,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.desert_temp + 0.2, 0.3)
                    * close(c.humidity, CONFIG.desert_hum, 0.5)
                    * BASE_DENSITY
                    * 0.01
            },
        },
        // Desert solitary wild
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..4) {
                        0 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Holladon,
                        )
                        .into(),
                        1 => {
                            quadruped_low::Body::random_with(rng, &quadruped_low::Species::Pangolin)
                                .into()
                        },
                        2 => quadruped_medium::Body::random_with(
                            rng,
                            &quadruped_medium::Species::Camel,
                        )
                        .into(),
                        3 => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Porcupine,
                        )
                        .into(),
                        _ => quadruped_small::Body {
                            species: quadruped_small::Species::Hare,
                            body_type: quadruped_small::BodyType::Male,
                        }
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.desert_temp + 0.2, 0.3) * BASE_DENSITY * 3.8
            },
        },
        // Desert solitary wild day
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        1 => quadruped_low::Body {
                            species: quadruped_low::Species::Salamander,
                            body_type: quadruped_low::BodyType::Male,
                        }
                        .into(),
                        _ => quadruped_small::Body::random_with(
                            rng,
                            &quadruped_small::Species::Gecko,
                        )
                        .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 1..2,
            is_underwater: false,
            day_period: vec![Morning, Noon, Evening],
            get_density: |c, _col| {
                close(c.temp, CONFIG.desert_temp + 0.2, 0.3) * BASE_DENSITY * 1.0
            },
        },
        // Underwater temperate
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(match rng.gen_range(0..3) {
                        0 => fish_medium::Body::random_with(rng, &fish_medium::Species::Marlin)
                            .into(),
                        1 => {
                            fish_small::Body::random_with(rng, &fish_small::Species::Piranha).into()
                        },
                        _ => fish_small::Body::random_with(rng, &fish_small::Species::Clownfish)
                            .into(),
                    })
                    .with_alignment(Alignment::Wild)
            },
            group_size: 3..5,
            is_underwater: true,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.temperate_temp, 1.0) * col.tree_density * BASE_DENSITY * 5.0
            },
        },
        // Underwater taiga
        Entry {
            make_entity: |pos, rng| {
                EntityInfo::at(pos)
                    .with_body(
                        fish_medium::Body::random_with(rng, &fish_medium::Species::Icepike).into(),
                    )
                    .with_alignment(Alignment::Enemy)
            },
            group_size: 1..3,
            is_underwater: true,
            day_period: vec![Night, Morning, Noon, Evening],
            get_density: |c, col| {
                close(c.temp, CONFIG.snow_temp, 0.15) * col.tree_density * BASE_DENSITY * 5.0
            },
        },
    ];

    for y in 0..vol.size_xy().y as i32 {
        for x in 0..vol.size_xy().x as i32 {
            let offs = Vec2::new(x, y);

            let wpos2d = wpos2d + offs;

            // Sample terrain
            let col_sample = if let Some(col_sample) = get_column(offs) {
                col_sample
            } else {
                continue;
            };

            let underwater = col_sample.water_level > col_sample.alt;
            let current_day_period;
            if let Some(time) = time {
                current_day_period = DayPeriod::from(time.0)
            } else {
                current_day_period = Noon
            }

            let entity_group = scatter.iter().enumerate().find_map(
                |(
                    _i,
                    Entry {
                        make_entity,
                        group_size,
                        is_underwater,
                        day_period,
                        get_density,
                    },
                )| {
                    let density = get_density(chunk, col_sample);
                    if density > 0.0
                        && dynamic_rng.gen::<f32>() < density * col_sample.spawn_rate
                        && underwater == *is_underwater
                        && day_period.contains(&current_day_period)
                        && col_sample.gradient < Some(1.3)
                    {
                        Some((make_entity, group_size.clone()))
                    } else {
                        None
                    }
                },
            );

            let alt = col_sample.alt as i32;

            if let Some((make_entity, group_size)) = entity_group {
                let group_size = dynamic_rng.gen_range(group_size.start..group_size.end);
                let entity = make_entity(
                    (wpos2d.map(|e| e as f32) + 0.5).with_z(alt as f32),
                    dynamic_rng,
                );
                for e in 0..group_size {
                    // Choose a nearby position
                    let offs_wpos2d = (Vec2::new(
                        (e as f32 / group_size as f32 * 2.0 * f32::consts::PI).sin(),
                        (e as f32 / group_size as f32 * 2.0 * f32::consts::PI).cos(),
                    ) * (5.0 + dynamic_rng.gen::<f32>().powf(0.5) * 5.0))
                        .map(|e| e as i32);
                    // Clamp position to chunk
                    let offs_wpos2d = (offs + offs_wpos2d)
                        .clamped(Vec2::zero(), vol.size_xy().map(|e| e as i32) - 1)
                        - offs;

                    // Find the intersection between ground and air, if there is one near the
                    // surface
                    if let Some(solid_end) = (-8..8).find(|z| {
                        (0..2).all(|z2| {
                            vol.get(Vec3::new(offs.x, offs.y, alt) + offs_wpos2d.with_z(z + z2))
                                .map(|b| !b.is_solid())
                                .unwrap_or(true)
                        })
                    }) {
                        let mut entity = entity.clone();
                        entity.pos += offs_wpos2d.with_z(solid_end).map(|e| e as f32);
                        supplement.add_entity(entity.with_automatic_name());
                    }
                }
            }
        }
    }
}
