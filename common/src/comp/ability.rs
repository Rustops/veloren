use crate::{
    assets::{self, Asset},
    combat::{self, CombatEffect, DamageKind, Knockback},
    comp::{
        self, aura, beam, buff, inventory::item::tool::ToolKind, projectile::ProjectileConstructor,
        skills, Body, CharacterState, EnergySource, LightEmitter, StateUpdate,
    },
    states::{
        behavior::JoinData,
        utils::{AbilityInfo, StageSection},
        *,
    },
    terrain::SpriteKind,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum CharacterAbilityType {
    BasicMelee,
    BasicRanged,
    Boost,
    ChargedMelee(StageSection),
    ChargedRanged,
    DashMelee(StageSection),
    BasicBlock,
    ComboMelee(StageSection, u32),
    LeapMelee(StageSection),
    SpinMelee(StageSection),
    Shockwave,
    BasicBeam,
    RepeaterRanged,
    BasicAura,
    SelfBuff,
}

impl From<&CharacterState> for CharacterAbilityType {
    fn from(state: &CharacterState) -> Self {
        match state {
            CharacterState::BasicMelee(_) => Self::BasicMelee,
            CharacterState::BasicRanged(_) => Self::BasicRanged,
            CharacterState::Boost(_) => Self::Boost,
            CharacterState::DashMelee(data) => Self::DashMelee(data.stage_section),
            CharacterState::BasicBlock(_) => Self::BasicBlock,
            CharacterState::LeapMelee(data) => Self::LeapMelee(data.stage_section),
            CharacterState::ComboMelee(data) => Self::ComboMelee(data.stage_section, data.stage),
            CharacterState::SpinMelee(data) => Self::SpinMelee(data.stage_section),
            CharacterState::ChargedMelee(data) => Self::ChargedMelee(data.stage_section),
            CharacterState::ChargedRanged(_) => Self::ChargedRanged,
            CharacterState::Shockwave(_) => Self::Shockwave,
            CharacterState::BasicBeam(_) => Self::BasicBeam,
            CharacterState::RepeaterRanged(_) => Self::RepeaterRanged,
            CharacterState::BasicAura(_) => Self::BasicAura,
            CharacterState::SelfBuff(_) => Self::SelfBuff,
            _ => Self::BasicMelee,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CharacterAbility {
    BasicMelee {
        energy_cost: f32,
        buildup_duration: f32,
        swing_duration: f32,
        recover_duration: f32,
        base_damage: f32,
        base_poise_damage: f32,
        knockback: Knockback,
        range: f32,
        max_angle: f32,
        damage_effect: Option<CombatEffect>,
        damage_kind: DamageKind,
    },
    BasicRanged {
        energy_cost: f32,
        buildup_duration: f32,
        recover_duration: f32,
        projectile: ProjectileConstructor,
        projectile_body: Body,
        projectile_light: Option<LightEmitter>,
        projectile_speed: f32,
        num_projectiles: u32,
        projectile_spread: f32,
    },
    RepeaterRanged {
        energy_cost: f32,
        buildup_duration: f32,
        shoot_duration: f32,
        recover_duration: f32,
        max_speed: f32,
        half_speed_at: u32,
        projectile: ProjectileConstructor,
        projectile_body: Body,
        projectile_light: Option<LightEmitter>,
        projectile_speed: f32,
    },
    Boost {
        movement_duration: f32,
        only_up: bool,
        speed: f32,
        max_exit_velocity: f32,
    },
    DashMelee {
        energy_cost: f32,
        base_damage: f32,
        scaled_damage: f32,
        base_poise_damage: f32,
        scaled_poise_damage: f32,
        base_knockback: f32,
        scaled_knockback: f32,
        range: f32,
        angle: f32,
        energy_drain: f32,
        forward_speed: f32,
        buildup_duration: f32,
        charge_duration: f32,
        swing_duration: f32,
        recover_duration: f32,
        charge_through: bool,
        is_interruptible: bool,
        damage_kind: DamageKind,
    },
    BasicBlock {
        buildup_duration: f32,
        recover_duration: f32,
        max_angle: f32,
        block_strength: f32,
        energy_cost: f32,
    },
    Roll {
        energy_cost: f32,
        buildup_duration: f32,
        movement_duration: f32,
        recover_duration: f32,
        roll_strength: f32,
        immune_melee: bool,
    },
    ComboMelee {
        stage_data: Vec<combo_melee::Stage<f32>>,
        initial_energy_gain: f32,
        max_energy_gain: f32,
        energy_increase: f32,
        speed_increase: f32,
        max_speed_increase: f32,
        scales_from_combo: u32,
        is_interruptible: bool,
        ori_modifier: f32,
    },
    LeapMelee {
        energy_cost: f32,
        buildup_duration: f32,
        movement_duration: f32,
        swing_duration: f32,
        recover_duration: f32,
        base_damage: f32,
        base_poise_damage: f32,
        range: f32,
        max_angle: f32,
        knockback: f32,
        forward_leap_strength: f32,
        vertical_leap_strength: f32,
        damage_kind: DamageKind,
    },
    SpinMelee {
        buildup_duration: f32,
        swing_duration: f32,
        recover_duration: f32,
        base_damage: f32,
        base_poise_damage: f32,
        knockback: Knockback,
        range: f32,
        damage_effect: Option<CombatEffect>,
        energy_cost: f32,
        is_infinite: bool,
        movement_behavior: spin_melee::MovementBehavior,
        is_interruptible: bool,
        forward_speed: f32,
        num_spins: u32,
        specifier: Option<spin_melee::FrontendSpecifier>,
        target: Option<combat::GroupTarget>,
        damage_kind: DamageKind,
    },
    ChargedMelee {
        energy_cost: f32,
        energy_drain: f32,
        initial_damage: f32,
        scaled_damage: f32,
        initial_poise_damage: f32,
        scaled_poise_damage: f32,
        initial_knockback: f32,
        scaled_knockback: f32,
        range: f32,
        max_angle: f32,
        speed: f32,
        charge_duration: f32,
        swing_duration: f32,
        hit_timing: f32,
        recover_duration: f32,
        specifier: Option<charged_melee::FrontendSpecifier>,
        damage_kind: DamageKind,
    },
    ChargedRanged {
        energy_cost: f32,
        energy_drain: f32,
        initial_regen: f32,
        scaled_regen: f32,
        initial_damage: f32,
        scaled_damage: f32,
        initial_knockback: f32,
        scaled_knockback: f32,
        speed: f32,
        buildup_duration: f32,
        charge_duration: f32,
        recover_duration: f32,
        projectile_body: Body,
        projectile_light: Option<LightEmitter>,
        initial_projectile_speed: f32,
        scaled_projectile_speed: f32,
        move_speed: f32,
    },
    Shockwave {
        energy_cost: f32,
        buildup_duration: f32,
        swing_duration: f32,
        recover_duration: f32,
        damage: f32,
        poise_damage: f32,
        knockback: Knockback,
        shockwave_angle: f32,
        shockwave_vertical_angle: f32,
        shockwave_speed: f32,
        shockwave_duration: f32,
        requires_ground: bool,
        move_efficiency: f32,
        damage_kind: DamageKind,
        specifier: comp::shockwave::FrontendSpecifier,
    },
    BasicBeam {
        buildup_duration: f32,
        recover_duration: f32,
        beam_duration: f32,
        damage: f32,
        tick_rate: f32,
        range: f32,
        max_angle: f32,
        damage_effect: Option<CombatEffect>,
        energy_regen: f32,
        energy_drain: f32,
        orientation_behavior: basic_beam::OrientationBehavior,
        ori_rate: f32,
        specifier: beam::FrontendSpecifier,
    },
    BasicAura {
        buildup_duration: f32,
        cast_duration: f32,
        recover_duration: f32,
        targets: combat::GroupTarget,
        aura: aura::AuraBuffConstructor,
        aura_duration: f32,
        range: f32,
        energy_cost: f32,
    },
    HealingBeam {
        buildup_duration: f32,
        recover_duration: f32,
        beam_duration: f32,
        heal: f32,
        tick_rate: f32,
        range: f32,
        max_angle: f32,
        energy_cost: f32,
        specifier: beam::FrontendSpecifier,
    },
    Blink {
        buildup_duration: f32,
        recover_duration: f32,
        max_range: f32,
    },
    BasicSummon {
        buildup_duration: f32,
        cast_duration: f32,
        recover_duration: f32,
        summon_amount: u32,
        summon_distance: (f32, f32),
        summon_info: basic_summon::SummonInfo,
        duration: Option<Duration>,
    },
    SelfBuff {
        buildup_duration: f32,
        cast_duration: f32,
        recover_duration: f32,
        buff_kind: buff::BuffKind,
        buff_strength: f32,
        buff_duration: Option<f32>,
        energy_cost: f32,
    },
    SpriteSummon {
        buildup_duration: f32,
        cast_duration: f32,
        recover_duration: f32,
        sprite: SpriteKind,
        summon_distance: (f32, f32),
        sparseness: f64,
    },
}

impl Default for CharacterAbility {
    fn default() -> Self {
        CharacterAbility::BasicMelee {
            energy_cost: 0.0,
            buildup_duration: 0.25,
            swing_duration: 0.25,
            recover_duration: 0.5,
            base_damage: 10.0,
            base_poise_damage: 0.0,
            knockback: Knockback {
                strength: 0.0,
                direction: combat::KnockbackDir::Away,
            },
            range: 3.5,
            max_angle: 15.0,
            damage_effect: None,
            damage_kind: DamageKind::Crushing,
        }
    }
}

impl Asset for CharacterAbility {
    type Loader = assets::RonLoader;

    const EXTENSION: &'static str = "ron";
}

impl CharacterAbility {
    /// Attempts to fulfill requirements, mutating `update` (taking energy) if
    /// applicable.
    pub fn requirements_paid(&self, data: &JoinData, update: &mut StateUpdate) -> bool {
        match self {
            CharacterAbility::Roll { energy_cost, .. } => {
                data.physics.on_ground.is_some()
                    && data.vel.0.xy().magnitude_squared() > 0.5
                    && update
                        .energy
                        .try_change_by(-(*energy_cost as i32), EnergySource::Ability)
                        .is_ok()
            },
            CharacterAbility::DashMelee { energy_cost, .. }
            | CharacterAbility::BasicMelee { energy_cost, .. }
            | CharacterAbility::BasicRanged { energy_cost, .. }
            | CharacterAbility::SpinMelee { energy_cost, .. }
            | CharacterAbility::ChargedRanged { energy_cost, .. }
            | CharacterAbility::ChargedMelee { energy_cost, .. }
            | CharacterAbility::Shockwave { energy_cost, .. }
            | CharacterAbility::BasicAura { energy_cost, .. }
            | CharacterAbility::BasicBlock { energy_cost, .. }
            | CharacterAbility::SelfBuff { energy_cost, .. } => update
                .energy
                .try_change_by(-(*energy_cost as i32), EnergySource::Ability)
                .is_ok(),
            // Consumes energy within state, so value only checked before entering state
            CharacterAbility::RepeaterRanged { energy_cost, .. } => {
                update.energy.current() as f32 >= *energy_cost
            },
            CharacterAbility::LeapMelee { energy_cost, .. } => {
                update.vel.0.z >= 0.0
                    && update
                        .energy
                        .try_change_by(-(*energy_cost as i32), EnergySource::Ability)
                        .is_ok()
            },
            CharacterAbility::HealingBeam { .. } => data.combo.counter() > 0,
            CharacterAbility::ComboMelee { .. }
            | CharacterAbility::Boost { .. }
            | CharacterAbility::BasicBeam { .. }
            | CharacterAbility::Blink { .. }
            | CharacterAbility::BasicSummon { .. }
            | CharacterAbility::SpriteSummon { .. } => true,
        }
    }

    pub fn default_roll() -> CharacterAbility {
        CharacterAbility::Roll {
            energy_cost: 120.0,
            buildup_duration: 0.05,
            movement_duration: 0.33,
            recover_duration: 0.125,
            roll_strength: 2.0,
            immune_melee: true,
        }
    }

    pub fn default_block() -> CharacterAbility {
        CharacterAbility::BasicBlock {
            buildup_duration: 0.35,
            recover_duration: 0.3,
            max_angle: 60.0,
            block_strength: 0.5,
            energy_cost: 50.0,
        }
    }

    pub fn adjusted_by_stats(mut self, power: f32, poise_strength: f32, speed: f32) -> Self {
        use CharacterAbility::*;
        match self {
            BasicMelee {
                ref mut buildup_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ref mut base_damage,
                ref mut base_poise_damage,
                ..
            } => {
                *buildup_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
                *base_damage *= power;
                *base_poise_damage *= poise_strength;
            },
            BasicRanged {
                ref mut buildup_duration,
                ref mut recover_duration,
                ref mut projectile,
                ..
            } => {
                *buildup_duration /= speed;
                *recover_duration /= speed;
                *projectile = projectile.modified_projectile(power, 1_f32, 1_f32);
            },
            RepeaterRanged {
                ref mut buildup_duration,
                ref mut shoot_duration,
                ref mut recover_duration,
                ref mut projectile,
                ..
            } => {
                *buildup_duration /= speed;
                *shoot_duration /= speed;
                *recover_duration /= speed;
                *projectile = projectile.modified_projectile(power, 1_f32, 1_f32);
            },
            Boost {
                ref mut movement_duration,
                speed: ref mut boost_speed,
                ..
            } => {
                *movement_duration /= speed;
                *boost_speed *= power;
            },
            DashMelee {
                ref mut base_damage,
                ref mut scaled_damage,
                ref mut base_poise_damage,
                ref mut scaled_poise_damage,
                ref mut buildup_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ..
            } => {
                *base_damage *= power;
                *scaled_damage *= power;
                *base_poise_damage *= poise_strength;
                *scaled_poise_damage *= poise_strength;
                *buildup_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
            },
            BasicBlock {
                ref mut buildup_duration,
                ref mut recover_duration,
                // Block strength explicitly not modified by power, that will be a separate stat
                ..
            } => {
                *buildup_duration /= speed;
                *recover_duration /= speed;
            },
            Roll {
                ref mut buildup_duration,
                ref mut movement_duration,
                ref mut recover_duration,
                ..
            } => {
                *buildup_duration /= speed;
                *movement_duration /= speed;
                *recover_duration /= speed;
            },
            ComboMelee {
                ref mut stage_data, ..
            } => {
                *stage_data = stage_data
                    .iter_mut()
                    .map(|s| s.adjusted_by_stats(power, poise_strength, speed))
                    .collect();
            },
            LeapMelee {
                ref mut buildup_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ref mut base_damage,
                ref mut base_poise_damage,
                ..
            } => {
                *buildup_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
                *base_damage *= power;
                *base_poise_damage *= poise_strength;
            },
            SpinMelee {
                ref mut buildup_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ref mut base_damage,
                ref mut base_poise_damage,
                ..
            } => {
                *buildup_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
                *base_damage *= power;
                *base_poise_damage *= poise_strength;
            },
            ChargedMelee {
                ref mut initial_damage,
                ref mut scaled_damage,
                ref mut initial_poise_damage,
                ref mut scaled_poise_damage,
                speed: ref mut ability_speed,
                ref mut charge_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ..
            } => {
                *initial_damage *= power;
                *scaled_damage *= power;
                *initial_poise_damage *= poise_strength;
                *scaled_poise_damage *= poise_strength;
                *ability_speed *= speed;
                *charge_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
            },
            ChargedRanged {
                ref mut initial_damage,
                ref mut scaled_damage,
                speed: ref mut ability_speed,
                ref mut buildup_duration,
                ref mut charge_duration,
                ref mut recover_duration,
                ..
            } => {
                *initial_damage *= power;
                *scaled_damage *= power;
                *ability_speed *= speed;
                *buildup_duration /= speed;
                *charge_duration /= speed;
                *recover_duration /= speed;
            },
            Shockwave {
                ref mut buildup_duration,
                ref mut swing_duration,
                ref mut recover_duration,
                ref mut damage,
                ref mut poise_damage,
                ..
            } => {
                *buildup_duration /= speed;
                *swing_duration /= speed;
                *recover_duration /= speed;
                *damage *= power;
                *poise_damage *= poise_strength;
            },
            BasicBeam {
                ref mut buildup_duration,
                ref mut recover_duration,
                ref mut damage,
                ref mut tick_rate,
                ..
            } => {
                *buildup_duration /= speed;
                *recover_duration /= speed;
                *damage *= power;
                *tick_rate *= speed;
            },
            BasicAura {
                ref mut buildup_duration,
                ref mut cast_duration,
                ref mut recover_duration,
                ref mut aura,
                ..
            } => {
                *buildup_duration /= speed;
                *cast_duration /= speed;
                *recover_duration /= speed;
                aura.strength *= power;
            },
            HealingBeam {
                ref mut buildup_duration,
                ref mut recover_duration,
                ref mut heal,
                ref mut tick_rate,
                ..
            } => {
                *buildup_duration /= speed;
                *recover_duration /= speed;
                *heal *= power;
                *tick_rate *= speed;
            },
            Blink {
                ref mut buildup_duration,
                ref mut recover_duration,
                ..
            } => {
                *buildup_duration /= speed;
                *recover_duration /= speed;
            },
            BasicSummon {
                ref mut buildup_duration,
                ref mut cast_duration,
                ref mut recover_duration,
                ..
            } => {
                // TODO: Figure out how/if power should affect this
                *buildup_duration /= speed;
                *cast_duration /= speed;
                *recover_duration /= speed;
            },
            SelfBuff {
                ref mut buff_strength,
                ref mut buildup_duration,
                ref mut cast_duration,
                ref mut recover_duration,
                ..
            } => {
                *buff_strength *= power;
                *buildup_duration /= speed;
                *cast_duration /= speed;
                *recover_duration /= speed;
            },
            SpriteSummon {
                ref mut buildup_duration,
                ref mut cast_duration,
                ref mut recover_duration,
                ..
            } => {
                // TODO: Figure out how/if power should affect this
                *buildup_duration /= speed;
                *cast_duration /= speed;
                *recover_duration /= speed;
            },
        }
        self
    }

    pub fn get_energy_cost(&self) -> u32 {
        use CharacterAbility::*;
        match self {
            BasicMelee { energy_cost, .. }
            | BasicRanged { energy_cost, .. }
            | RepeaterRanged { energy_cost, .. }
            | DashMelee { energy_cost, .. }
            | Roll { energy_cost, .. }
            | LeapMelee { energy_cost, .. }
            | SpinMelee { energy_cost, .. }
            | ChargedMelee { energy_cost, .. }
            | ChargedRanged { energy_cost, .. }
            | Shockwave { energy_cost, .. }
            | HealingBeam { energy_cost, .. }
            | BasicAura { energy_cost, .. }
            | BasicBlock { energy_cost, .. }
            | SelfBuff { energy_cost, .. } => *energy_cost as u32,
            BasicBeam { energy_drain, .. } => {
                if *energy_drain > f32::EPSILON {
                    1
                } else {
                    0
                }
            },
            Boost { .. }
            | ComboMelee { .. }
            | Blink { .. }
            | BasicSummon { .. }
            | SpriteSummon { .. } => 0,
        }
    }

    pub fn adjusted_by_skills(
        mut self,
        skillset: &skills::SkillSet,
        tool: Option<ToolKind>,
    ) -> Self {
        use skills::Skill::{self, *};
        use CharacterAbility::*;
        match tool {
            Some(ToolKind::Sword) => {
                use skills::SwordSkill::*;
                match self {
                    ComboMelee {
                        ref mut is_interruptible,
                        ref mut speed_increase,
                        ref mut max_speed_increase,
                        ref stage_data,
                        ref mut max_energy_gain,
                        ref mut scales_from_combo,
                        ..
                    } => {
                        *is_interruptible = skillset.has_skill(Sword(InterruptingAttacks));
                        let speed_segments = Sword(TsSpeed).max_level().map_or(1, |l| l + 1) as f32;
                        let speed_level = if skillset.has_skill(Sword(TsCombo)) {
                            skillset
                                .skill_level(Sword(TsSpeed))
                                .unwrap_or(None)
                                .map_or(1, |l| l + 1) as f32
                        } else {
                            0.0
                        };
                        {
                            *speed_increase *= speed_level / speed_segments;
                            *max_speed_increase *= speed_level / speed_segments;
                        }
                        let energy_level =
                            if let Ok(Some(level)) = skillset.skill_level(Sword(TsRegen)) {
                                level
                            } else {
                                0
                            };
                        *max_energy_gain = *max_energy_gain
                            * ((energy_level + 1) * stage_data.len() as u16 - 1) as f32
                            / (Sword(TsRegen).max_level().unwrap() + 1) as f32
                            * (stage_data.len() - 1) as f32;
                        *scales_from_combo = skillset
                            .skill_level(Sword(TsDamage))
                            .unwrap_or(None)
                            .unwrap_or(0)
                            .into();
                    },
                    DashMelee {
                        ref mut is_interruptible,
                        ref mut energy_cost,
                        ref mut energy_drain,
                        ref mut base_damage,
                        ref mut scaled_damage,
                        ref mut forward_speed,
                        ref mut charge_through,
                        ..
                    } => {
                        *is_interruptible = skillset.has_skill(Sword(InterruptingAttacks));
                        if let Ok(Some(level)) = skillset.skill_level(Sword(DCost)) {
                            *energy_cost *= 0.75_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sword(DDrain)) {
                            *energy_drain *= 0.75_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sword(DDamage)) {
                            *base_damage *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sword(DScaling)) {
                            *scaled_damage *= 1.2_f32.powi(level.into());
                        }
                        if skillset.has_skill(Sword(DSpeed)) {
                            *forward_speed *= 1.15;
                        }
                        *charge_through = skillset.has_skill(Sword(DInfinite));
                    },
                    SpinMelee {
                        ref mut is_interruptible,
                        ref mut base_damage,
                        ref mut swing_duration,
                        ref mut energy_cost,
                        ref mut num_spins,
                        ..
                    } => {
                        *is_interruptible = skillset.has_skill(Sword(InterruptingAttacks));
                        if let Ok(Some(level)) = skillset.skill_level(Sword(SDamage)) {
                            *base_damage *= 1.4_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sword(SSpeed)) {
                            *swing_duration *= 0.8_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sword(SCost)) {
                            *energy_cost *= 0.75_f32.powi(level.into());
                        }
                        *num_spins = skillset
                            .skill_level(Sword(SSpins))
                            .unwrap_or(None)
                            .unwrap_or(0) as u32
                            + 1;
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Axe) => {
                use skills::AxeSkill::*;
                match self {
                    ComboMelee {
                        ref mut speed_increase,
                        ref mut max_speed_increase,
                        ref mut stage_data,
                        ref mut max_energy_gain,
                        ref mut scales_from_combo,
                        ..
                    } => {
                        if !skillset.has_skill(Axe(DsCombo)) {
                            stage_data.pop();
                        }
                        let speed_segments = Axe(DsSpeed).max_level().unwrap_or(1) as f32;
                        let speed_level = skillset
                            .skill_level(Axe(DsSpeed))
                            .unwrap_or(None)
                            .unwrap_or(0) as f32;
                        {
                            *speed_increase *= speed_level / speed_segments;
                            *max_speed_increase *= speed_level / speed_segments;
                        }
                        let energy_level =
                            if let Ok(Some(level)) = skillset.skill_level(Axe(DsRegen)) {
                                level
                            } else {
                                0
                            };
                        *max_energy_gain = *max_energy_gain
                            * ((energy_level + 1) * stage_data.len() as u16 - 1) as f32
                            / (Axe(DsRegen).max_level().unwrap() + 1) as f32
                            * (stage_data.len() - 1) as f32;
                        *scales_from_combo = skillset
                            .skill_level(Axe(DsDamage))
                            .unwrap_or(None)
                            .unwrap_or(0)
                            .into();
                    },
                    SpinMelee {
                        ref mut base_damage,
                        ref mut swing_duration,
                        ref mut energy_cost,
                        ref mut is_infinite,
                        ref mut movement_behavior,
                        ..
                    } => {
                        *is_infinite = skillset.has_skill(Axe(SInfinite));
                        *movement_behavior = if skillset.has_skill(Axe(SHelicopter)) {
                            spin_melee::MovementBehavior::AxeHover
                        } else {
                            spin_melee::MovementBehavior::ForwardGround
                        };
                        if let Ok(Some(level)) = skillset.skill_level(Axe(SDamage)) {
                            *base_damage *= 1.3_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Axe(SSpeed)) {
                            *swing_duration *= 0.8_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Axe(SCost)) {
                            *energy_cost *= 0.75_f32.powi(level.into());
                        }
                    },
                    LeapMelee {
                        ref mut base_damage,
                        ref mut knockback,
                        ref mut energy_cost,
                        ref mut forward_leap_strength,
                        ref mut vertical_leap_strength,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Axe(LDamage)) {
                            *base_damage *= 1.35_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Axe(LKnockback)) {
                            *knockback *= 1.4_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Axe(LCost)) {
                            *energy_cost *= 0.75_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Axe(LDistance)) {
                            *forward_leap_strength *= 1.2_f32.powi(level.into());
                            *vertical_leap_strength *= 1.2_f32.powi(level.into());
                        }
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Hammer) => {
                use skills::HammerSkill::*;
                match self {
                    ComboMelee {
                        ref mut speed_increase,
                        ref mut max_speed_increase,
                        ref mut stage_data,
                        ref mut max_energy_gain,
                        ref mut scales_from_combo,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(SsKnockback)) {
                            *stage_data = (*stage_data)
                                .iter()
                                .map(|s| s.modify_strike(1.5_f32.powi(level.into())))
                                .collect::<Vec<combo_melee::Stage<f32>>>();
                        }
                        let speed_segments = Hammer(SsSpeed).max_level().unwrap_or(1) as f32;
                        let speed_level = skillset
                            .skill_level(Hammer(SsSpeed))
                            .unwrap_or(None)
                            .unwrap_or(0) as f32;
                        {
                            *speed_increase *= speed_level / speed_segments;
                            *max_speed_increase *= speed_level / speed_segments;
                        }
                        let energy_level =
                            if let Ok(Some(level)) = skillset.skill_level(Hammer(SsRegen)) {
                                level
                            } else {
                                0
                            };
                        *max_energy_gain = *max_energy_gain
                            * ((energy_level + 1) * stage_data.len() as u16) as f32
                            / ((Hammer(SsRegen).max_level().unwrap() + 1) * stage_data.len() as u16)
                                as f32;
                        *scales_from_combo = skillset
                            .skill_level(Hammer(SsDamage))
                            .unwrap_or(None)
                            .unwrap_or(0)
                            .into();
                    },
                    ChargedMelee {
                        ref mut scaled_damage,
                        ref mut scaled_knockback,
                        ref mut energy_drain,
                        ref mut speed,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(CDamage)) {
                            *scaled_damage *= 1.25_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(CKnockback)) {
                            *scaled_knockback *= 1.5_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(CDrain)) {
                            *energy_drain *= 0.75_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(CSpeed)) {
                            *speed *= 1.25_f32.powi(level.into());
                        }
                    },
                    LeapMelee {
                        ref mut base_damage,
                        ref mut knockback,
                        ref mut energy_cost,
                        ref mut forward_leap_strength,
                        ref mut vertical_leap_strength,
                        ref mut range,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(LDamage)) {
                            *base_damage *= 1.4_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(LKnockback)) {
                            *knockback *= 1.5_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(LCost)) {
                            *energy_cost *= 0.75_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(LDistance)) {
                            *forward_leap_strength *= 1.25_f32.powi(level.into());
                            *vertical_leap_strength *= 1.25_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Hammer(LRange)) {
                            *range += 1.0 * level as f32;
                        }
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Bow) => {
                use skills::BowSkill::*;
                match self {
                    ChargedRanged {
                        ref mut initial_damage,
                        ref mut scaled_damage,
                        ref mut initial_regen,
                        ref mut scaled_regen,
                        ref mut initial_knockback,
                        ref mut scaled_knockback,
                        ref mut speed,
                        ref mut move_speed,
                        ref mut initial_projectile_speed,
                        ref mut scaled_projectile_speed,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Bow(ProjSpeed)) {
                            let projectile_speed_scaling = 1.2_f32.powi(level.into());
                            *initial_projectile_speed *= projectile_speed_scaling;
                            *scaled_projectile_speed *= projectile_speed_scaling;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(CDamage)) {
                            let damage_scaling = 1.2_f32.powi(level.into());
                            *initial_damage *= damage_scaling;
                            *scaled_damage *= damage_scaling;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(CRegen)) {
                            let regen_scaling = 1.2_f32.powi(level.into());
                            *initial_regen *= regen_scaling;
                            *scaled_regen *= regen_scaling;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(CKnockback)) {
                            let knockback_scaling = 1.2_f32.powi(level.into());
                            *initial_knockback *= knockback_scaling;
                            *scaled_knockback *= knockback_scaling;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(CSpeed)) {
                            *speed *= 1.1_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(CMove)) {
                            *move_speed *= 1.1_f32.powi(level.into());
                        }
                    },
                    RepeaterRanged {
                        ref mut energy_cost,
                        ref mut projectile,
                        ref mut max_speed,
                        ref mut projectile_speed,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Bow(ProjSpeed)) {
                            *projectile_speed *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(RDamage)) {
                            let power = 1.2_f32.powi(level.into());
                            *projectile = projectile.modified_projectile(power, 1_f32, 1_f32);
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(RCost)) {
                            *energy_cost *= 0.8_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(RSpeed)) {
                            *max_speed *= 1.2_f32.powi(level.into());
                        }
                    },
                    BasicRanged {
                        ref mut projectile,
                        ref mut energy_cost,
                        ref mut num_projectiles,
                        ref mut projectile_spread,
                        ref mut projectile_speed,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Bow(ProjSpeed)) {
                            *projectile_speed *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(SDamage)) {
                            let power = 1.2_f32.powi(level.into());
                            *projectile = projectile.modified_projectile(power, 1_f32, 1_f32);
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(SCost)) {
                            *energy_cost *= 0.8_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(SArrows)) {
                            *num_projectiles += level as u32;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Bow(SSpread)) {
                            *projectile_spread *= 0.8_f32.powi(level.into());
                        }
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Staff) => {
                use skills::StaffSkill::*;
                match self {
                    BasicRanged {
                        ref mut projectile, ..
                    } => {
                        let damage_level = skillset
                            .skill_level(Staff(BDamage))
                            .unwrap_or(None)
                            .unwrap_or(0);
                        let regen_level = skillset
                            .skill_level(Staff(BRegen))
                            .unwrap_or(None)
                            .unwrap_or(0);
                        let range_level = skillset
                            .skill_level(Staff(BRadius))
                            .unwrap_or(None)
                            .unwrap_or(0);
                        let power = 1.2_f32.powi(damage_level.into());
                        let regen = 1.2_f32.powi(regen_level.into());
                        let range = 1.15_f32.powi(range_level.into());
                        *projectile = projectile.modified_projectile(power, regen, range);
                    },
                    BasicBeam {
                        ref mut damage,
                        ref mut range,
                        ref mut energy_drain,
                        ref mut beam_duration,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Staff(FDamage)) {
                            *damage *= 1.3_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(FRange)) {
                            let range_mod = 1.25_f32.powi(level.into());
                            *range *= range_mod;
                            // Duration modified to keep velocity constant
                            *beam_duration *= range_mod;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(FDrain)) {
                            *energy_drain *= 0.8_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(FVelocity)) {
                            let velocity_increase = 1.25_f32.powi(level.into());
                            let duration_mod = 1.0 / (1.0 + velocity_increase);
                            *beam_duration *= duration_mod;
                        }
                    },
                    Shockwave {
                        ref mut damage,
                        ref mut knockback,
                        ref mut shockwave_duration,
                        ref mut energy_cost,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Staff(SDamage)) {
                            *damage *= 1.3_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(SKnockback)) {
                            *knockback = knockback.modify_strength(1.3_f32.powi(level.into()));
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(SRange)) {
                            *shockwave_duration *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Staff(SCost)) {
                            *energy_cost *= 0.8_f32.powi(level.into());
                        }
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Sceptre) => {
                use skills::SceptreSkill::*;
                match self {
                    BasicBeam {
                        ref mut damage,
                        ref mut range,
                        ref mut beam_duration,
                        ref mut damage_effect,
                        ref mut energy_regen,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(LDamage)) {
                            *damage *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(LRange)) {
                            let range_mod = 1.2_f32.powi(level.into());
                            *range *= range_mod;
                            // Duration modified to keep velocity constant
                            *beam_duration *= range_mod;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(LRegen)) {
                            *energy_regen *= 1.2_f32.powi(level.into());
                        }
                        if let (Ok(Some(level)), Some(CombatEffect::Lifesteal(ref mut lifesteal))) =
                            (skillset.skill_level(Sceptre(LLifesteal)), damage_effect)
                        {
                            *lifesteal *= 1.15_f32.powi(level.into());
                        }
                    },
                    HealingBeam {
                        ref mut heal,
                        ref mut energy_cost,
                        ref mut range,
                        ref mut beam_duration,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(HHeal)) {
                            *heal *= 1.2_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(HRange)) {
                            let range_mod = 1.2_f32.powi(level.into());
                            *range *= range_mod;
                            // Duration modified to keep velocity constant
                            *beam_duration *= range_mod;
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(HCost)) {
                            *energy_cost *= 0.8_f32.powi(level.into());
                        }
                    },
                    BasicAura {
                        ref mut aura,
                        ref mut range,
                        ref mut energy_cost,
                        ..
                    } => {
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(AStrength)) {
                            aura.strength *= 1.15_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(ADuration)) {
                            aura.duration.map(|dur| dur * 1.2_f32.powi(level.into()));
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(ARange)) {
                            *range *= 1.25_f32.powi(level.into());
                        }
                        if let Ok(Some(level)) = skillset.skill_level(Sceptre(ACost)) {
                            *energy_cost *= 0.85_f32.powi(level.into());
                        }
                    },
                    _ => {},
                }
            },
            Some(ToolKind::Pick) => {
                use skills::MiningSkill::*;
                if let BasicMelee {
                    ref mut buildup_duration,
                    ref mut swing_duration,
                    ref mut recover_duration,
                    ..
                } = self
                {
                    if let Ok(Some(level)) = skillset.skill_level(Pick(Speed)) {
                        let speed = 1.1_f32.powi(level.into());
                        *buildup_duration /= speed;
                        *swing_duration /= speed;
                        *recover_duration /= speed;
                    }
                }
            },
            None => {
                if let CharacterAbility::Roll {
                    ref mut energy_cost,
                    ref mut roll_strength,
                    ref mut movement_duration,
                    ..
                } = self
                {
                    use skills::RollSkill::*;
                    if let Ok(Some(level)) = skillset.skill_level(Skill::Roll(Cost)) {
                        *energy_cost *= 0.9_f32.powi(level.into());
                    }
                    if let Ok(Some(level)) = skillset.skill_level(Skill::Roll(Strength)) {
                        *roll_strength *= 1.1_f32.powi(level.into());
                    }
                    if let Ok(Some(level)) = skillset.skill_level(Skill::Roll(Duration)) {
                        *movement_duration *= 1.1_f32.powi(level.into());
                    }
                }
            },
            Some(_) => {},
        }
        self
    }
}

impl From<(&CharacterAbility, AbilityInfo)> for CharacterState {
    fn from((ability, ability_info): (&CharacterAbility, AbilityInfo)) -> Self {
        match ability {
            CharacterAbility::BasicMelee {
                buildup_duration,
                swing_duration,
                recover_duration,
                base_damage,
                base_poise_damage,
                knockback,
                range,
                max_angle,
                damage_effect,
                energy_cost: _,
                damage_kind,
            } => CharacterState::BasicMelee(basic_melee::Data {
                static_data: basic_melee::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    base_damage: *base_damage,
                    base_poise_damage: *base_poise_damage,
                    knockback: *knockback,
                    range: *range,
                    max_angle: *max_angle,
                    damage_effect: *damage_effect,
                    ability_info,
                    damage_kind: *damage_kind,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::BasicRanged {
                buildup_duration,
                recover_duration,
                projectile,
                projectile_body,
                projectile_light,
                projectile_speed,
                energy_cost: _,
                num_projectiles,
                projectile_spread,
            } => CharacterState::BasicRanged(basic_ranged::Data {
                static_data: basic_ranged::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    projectile: *projectile,
                    projectile_body: *projectile_body,
                    projectile_light: *projectile_light,
                    projectile_speed: *projectile_speed,
                    num_projectiles: *num_projectiles,
                    projectile_spread: *projectile_spread,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::Boost {
                movement_duration,
                only_up,
                speed,
                max_exit_velocity,
            } => CharacterState::Boost(boost::Data {
                static_data: boost::StaticData {
                    movement_duration: Duration::from_secs_f32(*movement_duration),
                    only_up: *only_up,
                    speed: *speed,
                    max_exit_velocity: *max_exit_velocity,
                    ability_info,
                },
                timer: Duration::default(),
            }),
            CharacterAbility::DashMelee {
                energy_cost: _,
                base_damage,
                scaled_damage,
                base_poise_damage,
                scaled_poise_damage,
                base_knockback,
                scaled_knockback,
                range,
                angle,
                energy_drain,
                forward_speed,
                buildup_duration,
                charge_duration,
                swing_duration,
                recover_duration,
                charge_through,
                is_interruptible,
                damage_kind,
            } => CharacterState::DashMelee(dash_melee::Data {
                static_data: dash_melee::StaticData {
                    base_damage: *base_damage,
                    scaled_damage: *scaled_damage,
                    base_poise_damage: *base_poise_damage,
                    scaled_poise_damage: *scaled_poise_damage,
                    base_knockback: *base_knockback,
                    scaled_knockback: *scaled_knockback,
                    range: *range,
                    angle: *angle,
                    energy_drain: *energy_drain,
                    forward_speed: *forward_speed,
                    charge_through: *charge_through,
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    charge_duration: Duration::from_secs_f32(*charge_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    is_interruptible: *is_interruptible,
                    ability_info,
                    damage_kind: *damage_kind,
                },
                auto_charge: false,
                timer: Duration::default(),
                charge_end_timer: Duration::from_secs_f32(*charge_duration),
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::BasicBlock {
                buildup_duration,
                recover_duration,
                max_angle,
                block_strength,
                energy_cost: _,
            } => CharacterState::BasicBlock(basic_block::Data {
                static_data: basic_block::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    max_angle: *max_angle,
                    block_strength: *block_strength,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::Roll {
                energy_cost: _,
                buildup_duration,
                movement_duration,
                recover_duration,
                roll_strength,
                immune_melee,
            } => CharacterState::Roll(roll::Data {
                static_data: roll::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    movement_duration: Duration::from_secs_f32(*movement_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    roll_strength: *roll_strength,
                    immune_melee: *immune_melee,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                was_wielded: false, // false by default. utils might set it to true
                was_sneak: false,
                was_combo: None,
            }),
            CharacterAbility::ComboMelee {
                stage_data,
                initial_energy_gain,
                max_energy_gain,
                energy_increase,
                speed_increase,
                max_speed_increase,
                scales_from_combo,
                is_interruptible,
                ori_modifier,
            } => CharacterState::ComboMelee(combo_melee::Data {
                static_data: combo_melee::StaticData {
                    num_stages: stage_data.len() as u32,
                    stage_data: stage_data.iter().map(|stage| stage.to_duration()).collect(),
                    initial_energy_gain: *initial_energy_gain,
                    max_energy_gain: *max_energy_gain,
                    energy_increase: *energy_increase,
                    speed_increase: 1.0 - *speed_increase,
                    max_speed_increase: *max_speed_increase,
                    scales_from_combo: *scales_from_combo,
                    is_interruptible: *is_interruptible,
                    ori_modifier: *ori_modifier as f32,
                    ability_info,
                },
                exhausted: false,
                stage: 1,
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::LeapMelee {
                energy_cost: _,
                buildup_duration,
                movement_duration,
                swing_duration,
                recover_duration,
                base_damage,
                base_poise_damage,
                knockback,
                range,
                max_angle,
                forward_leap_strength,
                vertical_leap_strength,
                damage_kind,
            } => CharacterState::LeapMelee(leap_melee::Data {
                static_data: leap_melee::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    movement_duration: Duration::from_secs_f32(*movement_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    base_damage: *base_damage,
                    base_poise_damage: *base_poise_damage,
                    knockback: *knockback,
                    range: *range,
                    max_angle: *max_angle,
                    forward_leap_strength: *forward_leap_strength,
                    vertical_leap_strength: *vertical_leap_strength,
                    ability_info,
                    damage_kind: *damage_kind,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::SpinMelee {
                buildup_duration,
                swing_duration,
                recover_duration,
                base_damage,
                base_poise_damage,
                knockback,
                range,
                damage_effect,
                energy_cost,
                is_infinite,
                movement_behavior,
                is_interruptible,
                forward_speed,
                num_spins,
                specifier,
                target,
                damage_kind,
            } => CharacterState::SpinMelee(spin_melee::Data {
                static_data: spin_melee::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    base_damage: *base_damage,
                    base_poise_damage: *base_poise_damage,
                    knockback: *knockback,
                    range: *range,
                    damage_effect: *damage_effect,
                    energy_cost: *energy_cost,
                    is_infinite: *is_infinite,
                    movement_behavior: *movement_behavior,
                    is_interruptible: *is_interruptible,
                    forward_speed: *forward_speed,
                    num_spins: *num_spins,
                    target: *target,
                    ability_info,
                    specifier: *specifier,
                    damage_kind: *damage_kind,
                },
                timer: Duration::default(),
                consecutive_spins: 1,
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::ChargedMelee {
                energy_cost,
                energy_drain,
                initial_damage,
                scaled_damage,
                initial_poise_damage,
                scaled_poise_damage,
                initial_knockback,
                scaled_knockback,
                speed,
                charge_duration,
                swing_duration,
                hit_timing,
                recover_duration,
                range,
                max_angle,
                specifier,
                damage_kind,
            } => CharacterState::ChargedMelee(charged_melee::Data {
                static_data: charged_melee::StaticData {
                    energy_cost: *energy_cost,
                    energy_drain: *energy_drain,
                    initial_damage: *initial_damage,
                    scaled_damage: *scaled_damage,
                    initial_poise_damage: *initial_poise_damage,
                    scaled_poise_damage: *scaled_poise_damage,
                    initial_knockback: *initial_knockback,
                    scaled_knockback: *scaled_knockback,
                    speed: *speed,
                    range: *range,
                    max_angle: *max_angle,
                    charge_duration: Duration::from_secs_f32(*charge_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    hit_timing: *hit_timing,
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    ability_info,
                    specifier: *specifier,
                    damage_kind: *damage_kind,
                },
                stage_section: StageSection::Charge,
                timer: Duration::default(),
                exhausted: false,
                charge_amount: 0.0,
            }),
            CharacterAbility::ChargedRanged {
                energy_cost: _,
                energy_drain,
                initial_regen,
                scaled_regen,
                initial_damage,
                scaled_damage,
                initial_knockback,
                scaled_knockback,
                speed,
                buildup_duration,
                charge_duration,
                recover_duration,
                projectile_body,
                projectile_light,
                initial_projectile_speed,
                scaled_projectile_speed,
                move_speed,
            } => CharacterState::ChargedRanged(charged_ranged::Data {
                static_data: charged_ranged::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    charge_duration: Duration::from_secs_f32(*charge_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    energy_drain: *energy_drain,
                    initial_regen: *initial_regen,
                    scaled_regen: *scaled_regen,
                    initial_damage: *initial_damage,
                    scaled_damage: *scaled_damage,
                    speed: *speed,
                    initial_knockback: *initial_knockback,
                    scaled_knockback: *scaled_knockback,
                    projectile_body: *projectile_body,
                    projectile_light: *projectile_light,
                    initial_projectile_speed: *initial_projectile_speed,
                    scaled_projectile_speed: *scaled_projectile_speed,
                    move_speed: *move_speed,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                exhausted: false,
            }),
            CharacterAbility::RepeaterRanged {
                energy_cost,
                buildup_duration,
                shoot_duration,
                recover_duration,
                max_speed,
                half_speed_at,
                projectile,
                projectile_body,
                projectile_light,
                projectile_speed,
            } => CharacterState::RepeaterRanged(repeater_ranged::Data {
                static_data: repeater_ranged::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    shoot_duration: Duration::from_secs_f32(*shoot_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    energy_cost: *energy_cost,
                    // 1.0 is subtracted as 1.0 is added in state file
                    max_speed: *max_speed - 1.0,
                    half_speed_at: *half_speed_at,
                    projectile: *projectile,
                    projectile_body: *projectile_body,
                    projectile_light: *projectile_light,
                    projectile_speed: *projectile_speed,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                projectiles_fired: 0,
                speed: 1.0,
            }),
            CharacterAbility::Shockwave {
                energy_cost: _,
                buildup_duration,
                swing_duration,
                recover_duration,
                damage,
                poise_damage,
                knockback,
                shockwave_angle,
                shockwave_vertical_angle,
                shockwave_speed,
                shockwave_duration,
                requires_ground,
                move_efficiency,
                damage_kind,
                specifier,
            } => CharacterState::Shockwave(shockwave::Data {
                static_data: shockwave::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    swing_duration: Duration::from_secs_f32(*swing_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    damage: *damage,
                    poise_damage: *poise_damage,
                    knockback: *knockback,
                    shockwave_angle: *shockwave_angle,
                    shockwave_vertical_angle: *shockwave_vertical_angle,
                    shockwave_speed: *shockwave_speed,
                    shockwave_duration: Duration::from_secs_f32(*shockwave_duration),
                    requires_ground: *requires_ground,
                    move_efficiency: *move_efficiency,
                    ability_info,
                    damage_kind: *damage_kind,
                    specifier: *specifier,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::BasicBeam {
                buildup_duration,
                recover_duration,
                beam_duration,
                damage,
                tick_rate,
                range,
                max_angle,
                damage_effect,
                energy_regen,
                energy_drain,
                orientation_behavior,
                ori_rate,
                specifier,
            } => CharacterState::BasicBeam(basic_beam::Data {
                static_data: basic_beam::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    beam_duration: Duration::from_secs_f32(*beam_duration),
                    damage: *damage,
                    tick_rate: *tick_rate,
                    range: *range,
                    max_angle: *max_angle,
                    damage_effect: *damage_effect,
                    energy_regen: *energy_regen,
                    energy_drain: *energy_drain,
                    ability_info,
                    orientation_behavior: *orientation_behavior,
                    ori_rate: *ori_rate,
                    specifier: *specifier,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::BasicAura {
                buildup_duration,
                cast_duration,
                recover_duration,
                targets,
                aura,
                aura_duration,
                range,
                energy_cost: _,
            } => CharacterState::BasicAura(basic_aura::Data {
                static_data: basic_aura::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    cast_duration: Duration::from_secs_f32(*cast_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    targets: *targets,
                    aura: *aura,
                    aura_duration: Duration::from_secs_f32(*aura_duration),
                    range: *range,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::HealingBeam {
                buildup_duration,
                recover_duration,
                beam_duration,
                heal,
                tick_rate,
                range,
                max_angle,
                energy_cost,
                specifier,
            } => CharacterState::HealingBeam(healing_beam::Data {
                static_data: healing_beam::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    beam_duration: Duration::from_secs_f32(*beam_duration),
                    heal: *heal,
                    tick_rate: *tick_rate,
                    range: *range,
                    max_angle: *max_angle,
                    energy_cost: *energy_cost,
                    ability_info,
                    specifier: *specifier,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::Blink {
                buildup_duration,
                recover_duration,
                max_range,
            } => CharacterState::Blink(blink::Data {
                static_data: blink::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    max_range: *max_range,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::BasicSummon {
                buildup_duration,
                cast_duration,
                recover_duration,
                summon_amount,
                summon_distance,
                summon_info,
                duration,
            } => CharacterState::BasicSummon(basic_summon::Data {
                static_data: basic_summon::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    cast_duration: Duration::from_secs_f32(*cast_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    summon_amount: *summon_amount,
                    summon_distance: *summon_distance,
                    summon_info: *summon_info,
                    ability_info,
                    duration: *duration,
                },
                summon_count: 0,
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::SelfBuff {
                buildup_duration,
                cast_duration,
                recover_duration,
                buff_kind,
                buff_strength,
                buff_duration,
                energy_cost: _,
            } => CharacterState::SelfBuff(self_buff::Data {
                static_data: self_buff::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    cast_duration: Duration::from_secs_f32(*cast_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    buff_kind: *buff_kind,
                    buff_strength: *buff_strength,
                    buff_duration: buff_duration.map(Duration::from_secs_f32),
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
            }),
            CharacterAbility::SpriteSummon {
                buildup_duration,
                cast_duration,
                recover_duration,
                sprite,
                summon_distance,
                sparseness,
            } => CharacterState::SpriteSummon(sprite_summon::Data {
                static_data: sprite_summon::StaticData {
                    buildup_duration: Duration::from_secs_f32(*buildup_duration),
                    cast_duration: Duration::from_secs_f32(*cast_duration),
                    recover_duration: Duration::from_secs_f32(*recover_duration),
                    sprite: *sprite,
                    summon_distance: *summon_distance,
                    sparseness: *sparseness,
                    ability_info,
                },
                timer: Duration::default(),
                stage_section: StageSection::Buildup,
                achieved_radius: summon_distance.0.floor() as i32 - 1,
            }),
        }
    }
}
