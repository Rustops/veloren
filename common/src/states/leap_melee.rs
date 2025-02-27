use crate::{
    combat::{Attack, AttackDamage, AttackEffect, CombatBuff, CombatEffect, CombatRequirement},
    comp::{tool::ToolKind, CharacterState, Melee, StateUpdate},
    states::{
        behavior::{CharacterBehavior, JoinData},
        utils::{StageSection, *},
    },
    Damage, DamageKind, DamageSource, GroupTarget, Knockback, KnockbackDir,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Separated out to condense update portions of character state
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StaticData {
    /// How long the state is moving
    pub movement_duration: Duration,
    /// How long until state should deal damage
    pub buildup_duration: Duration,
    /// How long the weapon swings
    pub swing_duration: Duration,
    /// How long the state has until exiting
    pub recover_duration: Duration,
    /// Base damage
    pub base_damage: f32,
    /// Base poise damage
    pub base_poise_damage: f32,
    /// Knockback
    pub knockback: f32,
    /// Max range
    pub range: f32,
    /// Max angle (45.0 will give you a 90.0 angle window)
    pub max_angle: f32,
    /// Affects how far forward the player leaps
    pub forward_leap_strength: f32,
    /// Affects how high the player leaps
    pub vertical_leap_strength: f32,
    /// What key is used to press ability
    pub ability_info: AbilityInfo,
    /// What kind of damage the attack does
    pub damage_kind: DamageKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Data {
    /// Struct containing data that does not change over the course of the
    /// character state
    pub static_data: StaticData,
    /// Timer for each stage
    pub timer: Duration,
    /// What section the character stage is in
    pub stage_section: StageSection,
    /// Whether the attack can deal more damage
    pub exhausted: bool,
}

impl CharacterBehavior for Data {
    fn behavior(&self, data: &JoinData) -> StateUpdate {
        let mut update = StateUpdate::from(data);

        handle_orientation(data, &mut update, 1.0);
        handle_move(data, &mut update, 0.3);
        handle_jump(data, &mut update, 1.0);

        match self.stage_section {
            // Delay before leaping into the air
            StageSection::Buildup => {
                // Wait for `buildup_duration` to expire
                if self.timer < self.static_data.buildup_duration {
                    update.character = CharacterState::LeapMelee(Data {
                        timer: tick_attack_or_default(data, self.timer, None),
                        ..*self
                    });
                } else {
                    // Transitions to leap portion of state after buildup delay
                    update.character = CharacterState::LeapMelee(Data {
                        timer: Duration::default(),
                        stage_section: StageSection::Movement,
                        ..*self
                    });
                }
            },
            StageSection::Movement => {
                if self.timer < self.static_data.movement_duration {
                    // Apply jumping force
                    let progress = 1.0
                        - self.timer.as_secs_f32()
                            / self.static_data.movement_duration.as_secs_f32();
                    handle_forced_movement(data, &mut update, ForcedMovement::Leap {
                        vertical: self.static_data.vertical_leap_strength,
                        forward: self.static_data.forward_leap_strength,
                        progress,
                        direction: MovementDirection::Look,
                    });

                    // Increment duration
                    // If we were to set a timeout for state, this would be
                    // outside if block and have else check for > movement
                    // duration * some multiplier
                    update.character = CharacterState::LeapMelee(Data {
                        timer: tick_attack_or_default(data, self.timer, None),
                        ..*self
                    });
                } else if data.physics.on_ground.is_some() {
                    // Transitions to swing portion of state upon hitting ground
                    update.character = CharacterState::LeapMelee(Data {
                        timer: Duration::default(),
                        stage_section: StageSection::Swing,
                        ..*self
                    });
                }
            },
            StageSection::Swing => {
                if self.timer < self.static_data.swing_duration {
                    // Swings weapons
                    update.character = CharacterState::LeapMelee(Data {
                        timer: tick_attack_or_default(data, self.timer, None),
                        ..*self
                    });
                } else {
                    // Transitions to recover portion
                    update.character = CharacterState::LeapMelee(Data {
                        timer: Duration::default(),
                        stage_section: StageSection::Recover,
                        ..*self
                    });
                }
            },
            StageSection::Recover => {
                if !self.exhausted {
                    let poise = AttackEffect::new(
                        Some(GroupTarget::OutOfGroup),
                        CombatEffect::Poise(self.static_data.base_poise_damage as f32),
                    )
                    .with_requirement(CombatRequirement::AnyDamage);
                    let knockback = AttackEffect::new(
                        Some(GroupTarget::OutOfGroup),
                        CombatEffect::Knockback(Knockback {
                            strength: self.static_data.knockback,
                            direction: KnockbackDir::Away,
                        }),
                    )
                    .with_requirement(CombatRequirement::AnyDamage);
                    let buff = CombatEffect::Buff(CombatBuff::default_physical());
                    let damage = AttackDamage::new(
                        Damage {
                            source: DamageSource::Melee,
                            kind: self.static_data.damage_kind,
                            value: self.static_data.base_damage as f32,
                        },
                        Some(GroupTarget::OutOfGroup),
                    )
                    .with_effect(buff);
                    let (crit_chance, crit_mult) =
                        get_crit_data(data, self.static_data.ability_info);
                    let attack = Attack::default()
                        .with_damage(damage)
                        .with_crit(crit_chance, crit_mult)
                        .with_effect(poise)
                        .with_effect(knockback)
                        .with_combo_increment();

                    // Hit attempt, when animation plays
                    data.updater.insert(data.entity, Melee {
                        attack,
                        range: self.static_data.range,
                        max_angle: self.static_data.max_angle.to_radians(),
                        applied: false,
                        hit_count: 0,
                        break_block: data
                            .inputs
                            .select_pos
                            .map(|p| {
                                (
                                    p.map(|e| e.floor() as i32),
                                    self.static_data.ability_info.tool,
                                )
                            })
                            .filter(|(_, tool)| tool == &Some(ToolKind::Pick)),
                    });

                    update.character = CharacterState::LeapMelee(Data {
                        timer: tick_attack_or_default(data, self.timer, None),
                        exhausted: true,
                        ..*self
                    });
                } else if self.timer < self.static_data.recover_duration {
                    // Complete recovery delay before finishing state
                    update.character = CharacterState::LeapMelee(Data {
                        timer: tick_attack_or_default(data, self.timer, None),
                        ..*self
                    });
                } else {
                    // Done
                    update.character = CharacterState::Wielding;
                    // Make sure attack component is removed
                    data.updater.remove::<Melee>(data.entity);
                }
            },
            _ => {
                // If it somehow ends up in an incorrect stage section
                update.character = CharacterState::Wielding;
                // Make sure attack component is removed
                data.updater.remove::<Melee>(data.entity);
            },
        }

        // At end of state logic so an interrupt isn't overwritten
        if !input_is_pressed(data, self.static_data.ability_info.input) {
            handle_state_interrupt(data, &mut update, false);
        }

        update
    }
}
