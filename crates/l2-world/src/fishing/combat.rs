//! The fight with the fish: `Fishing.java`, and the `Pumping` and `Reeling` skill effects.
// Java: mechanics/fishing/Fishing.java
// Java: dist/game/data/scripts/handlers/skill/effects/{Pumping,Reeling}.java
// Java: entity/actor/Player.java (startFishCombat)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ExFishingHpRegen, ExFishingStartCombat, PlaySound, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use super::cast::is_night_lure;
use super::{FISHING_EXPERTISE_SKILL, FishCombat, FishingTimer, FixedRate};
use crate::inventory::handlers::shots::ShotType;
use crate::stats::Stat;
use crate::timer::TimerKind;
use crate::world::World;

/// Which of the two skills is being used. They are the same code in Java apart from the messages
/// and the mode that counts as a good use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FishAction {
    Pumping,
    Reeling,
}

impl FishAction {
    /// Java `_anim`: `1` for `usePumping`, `2` for `useReeling`.
    fn anim(self) -> u8 {
        match self {
            Self::Pumping => 1,
            Self::Reeling => 2,
        }
    }

    /// Whether the fish's state makes this use work. Pumping works against a resting fish
    /// (`mode == 0`), reeling against a fighting one (`mode == 1`); the deceptive mode reverses
    /// both.
    fn works(self, mode: u8, deceptive_mode: u8) -> bool {
        let right_mode = match self {
            Self::Pumping => mode == 0,
            Self::Reeling => mode == 1,
        };
        right_mode == (deceptive_mode == 0)
    }

    fn success_message(self) -> SystemMessageId {
        match self {
            Self::Pumping => SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage,
            Self::Reeling => SystemMessageId::YouReelThatFishInCloserAndCauseS1Damage,
        }
    }

    fn penalty_message(self) -> SystemMessageId {
        match self {
            Self::Pumping => SystemMessageId::YourPumpingWasSuccessfulMasteryPenaltyS1,
            Self::Reeling => SystemMessageId::YourReelingWasSuccessfulMasteryPenaltyS1,
        }
    }

    fn failure_message(self) -> SystemMessageId {
        match self {
            Self::Pumping => SystemMessageId::YouFailedToDoAnythingWithTheFishAndItRegainsS1Hp,
            Self::Reeling => SystemMessageId::YouFailedToReelThatFishInFurtherAndItRegainsS1Hp,
        }
    }

    fn only_while_fishing_message(self) -> SystemMessageId {
        match self {
            Self::Pumping => SystemMessageId::YouMayOnlyUseThePumpingSkillWhileYouAreFishing,
            Self::Reeling => SystemMessageId::YouMayOnlyUseTheReelingSkillWhileYouAreFishing,
        }
    }
}

impl World {
    /// Java `Player.startFishCombat(isNoob, isUpperGrade)`: `new Fishing(...)`.
    pub(crate) fn begin_fish_combat(&mut self, id: ObjectId, is_noob: bool, is_upper_grade: bool) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (Some(fish), Some(lure_id)) = (player.fishing.fish.clone(), player.fishing.lure_item_id) else {
            return;
        };
        // Every night lure fights like a night catch, the fish turns deceptive and reverses
        // reeling and pumping.
        let is_upper_grade = is_upper_grade || is_night_lure(lure_id);

        let (deceptive_mode, lure_type) = if is_upper_grade {
            (u8::from(self.rng().below(100) >= 90), 2)
        } else {
            (0, u8::from(!is_noob))
        };
        let mode = u8::from(self.rng().below(100) >= 80);
        let combat = FishCombat {
            time: fish.combat_duration,
            stop: 0,
            good_use: 0,
            anim: 0,
            mode,
            deceptive_mode,
            timer: None,
            rate: FixedRate::new(self.tick, 1000, 1000),
            fish_id: fish.item_id,
            fish_max_hp: fish.fish_hp,
            fish_cur_hp: fish.fish_hp,
            regen_hp: fish.hp_regen,
            is_upper_grade,
            lure_id,
        };

        self.broadcast_creature_packet(
            id,
            &ExFishingStartCombat {
                object_id: id.0,
                time: combat.time,
                hp: combat.fish_max_hp,
                mode,
                lure_type,
                deceptive_mode,
            },
        );
        self.send(id, &PlaySound::with_unknown1(1, "SF_S_01"));

        // Succeeded in getting a bite.
        self.send(id, &SystemMessage::new(SystemMessageId::YouVeGotABite));

        let mut combat = combat;
        let due = combat.rate.next_due(self.tick);
        combat.timer = Some(
            self.timers
                .schedule_at(due, TimerKind::Fishing(FishingTimer::Combat(id))),
        );
        if let Some(player) = self.players.get_mut(&id) {
            player.fishing.combat = Some(combat);
        }
    }

    /// Java `Fishing.run`, once a second.
    pub(crate) fn on_fish_combat_timer(&mut self, id: ObjectId) {
        let tick = self.tick;
        let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) else {
            return;
        };
        // The task repeats at a fixed rate until the fight ends and cancels it.
        let due = combat.rate.next_due(tick);
        let (cur_hp, max_hp, time) = (combat.fish_cur_hp, combat.fish_max_hp, combat.time);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Fishing(FishingTimer::Combat(id)));
        if let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) {
            combat.timer = Some(timer);
        }

        if cur_hp >= max_hp.saturating_mul(2) {
            // The fish got away.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourBaitWasStolenByThatFish),
            );
            self.fish_do_die(id, false);
        } else if time <= 0 {
            // Time is up, so that fish got away.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThatFishIsMoreDeterminedThanYouAreItSpitTheHook),
            );
            self.fish_do_die(id, false);
        } else {
            self.fish_ai_task(id);
        }
    }

    /// Java `Fishing.aiTask`.
    fn fish_ai_task(&mut self, id: ObjectId) {
        let (flip_mode, flip_deceptive) = {
            // The draws come in Java's order: the mode check first, then the deceptive one.
            let Some(combat) = self.players.get(&id).and_then(|p| p.fishing.combat.as_ref()) else {
                return;
            };
            let (stop, upper) = (combat.stop, combat.is_upper_grade);
            if stop == 0 {
                let flip_mode = self.rng().below(100) >= 70;
                let flip_deceptive = upper && self.rng().below(100) >= 90;
                (Some(flip_mode), flip_deceptive)
            } else {
                (None, false)
            }
        };

        let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) else {
            return;
        };
        combat.time -= 1;
        // The fish state shown to the player must follow the mode alone. Including the deceptive
        // mode here would match the reversed actions and hide the deception.
        if combat.mode == 1 {
            // `(int) _regenHp`
            combat.fish_cur_hp = combat.fish_cur_hp.wrapping_add(combat.regen_hp as i32);
        }
        match flip_mode {
            Some(flip) => {
                combat.stop = 1;
                if flip {
                    combat.mode = u8::from(combat.mode == 0);
                }
                if flip_deceptive {
                    combat.deceptive_mode = u8::from(combat.deceptive_mode == 0);
                }
            }
            None => combat.stop -= 1,
        }

        let packet = ExFishingHpRegen {
            object_id: id.0,
            time: combat.time,
            fish_hp: combat.fish_cur_hp,
            hp_mode: combat.mode,
            good_use: 0,
            anim: combat.anim,
            penalty: 0,
            hp_bar_color: combat.deceptive_mode,
        };
        let broadcast = combat.anim != 0;
        if broadcast {
            self.broadcast_creature_packet(id, &packet);
        } else {
            self.send(id, &packet);
        }
    }

    /// Java `Fishing.changeHp(hp, pen)`.
    fn fish_change_hp(&mut self, id: ObjectId, hp: i32, pen: i32) {
        let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) else {
            return;
        };
        combat.fish_cur_hp = combat.fish_cur_hp.saturating_sub(hp).max(0);
        let packet = ExFishingHpRegen {
            object_id: id.0,
            time: combat.time,
            fish_hp: combat.fish_cur_hp,
            hp_mode: combat.mode,
            good_use: combat.good_use,
            anim: combat.anim,
            penalty: pen,
            hp_bar_color: combat.deceptive_mode,
        };
        combat.anim = 0;
        let limit = combat.fish_max_hp.saturating_mul(2);
        let outcome = if combat.fish_cur_hp > limit {
            combat.fish_cur_hp = limit;
            Some(false)
        } else if combat.fish_cur_hp == 0 {
            Some(true)
        } else {
            None
        };
        self.broadcast_creature_packet(id, &packet);
        if let Some(win) = outcome {
            self.fish_do_die(id, win);
        }
    }

    /// Java `Fishing.doDie(win)`: the fight is over.
    pub(crate) fn fish_do_die(&mut self, id: ObjectId, win: bool) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        // Java cancels the task even when `_fisher` is already null; a second call finds no fight.
        let Some(combat) = player.fishing.combat.as_mut() else {
            return;
        };
        let timer = combat.timer.take();
        let (fish_id, lure_id) = (combat.fish_id, combat.lure_id);
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }

        if win {
            self.fish_caught(id, fish_id, lure_id);
        }

        self.end_fishing(id, win);
    }

    /// The `if (win)` block of `Fishing.doDie`.
    fn fish_caught(&mut self, id: ObjectId, fish_id: i32, lure_id: i32) {
        let level = self.players.get(&id).map_or(0, |p| p.core.level);
        // NOTE(java-quirk): a level without a fishing monster (below 6 or above 85 in the shipped
        // file) gets neither the fish nor a message.
        let Some(monster) = self
            .fishing
            .monsters
            .as_ref()
            .and_then(|m| m.get_fishing_monster(level).cloned())
        else {
            return;
        };
        if self.rng().below(100) <= monster.probability {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCaughtSomethingSmellyAndScaryMaybeYouShouldThrowItBack,
                ),
            );
            // `Quest.addSpawn(npcId, fisher)`: a temporary spawn at the fisher.
            if let Some((x, y, z)) = self.position_of(id) {
                let heading = self.players.get(&id).map_or(0, |p| p.core.heading);
                if let Some(npc) = self.spawn_npc_at(monster.fishing_monster_id, x, y, z, heading) {
                    self.set_creature_target(npc, Some(id));
                }
            }
        } else {
            self.send(id, &SystemMessage::new(SystemMessageId::YouCaughtSomething));
            self.add_item(id, fish_id, 1, true);
            self.championship_new_fish(id, lure_id);
        }
    }

    /// Java `Fishing.usePumping(dmg, pen)` and `Fishing.useReeling(dmg, pen)`.
    fn fish_use(&mut self, id: ObjectId, action: FishAction, dmg: i32, pen: i32) {
        if let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) {
            combat.anim = action.anim();
        } else {
            return;
        }
        if self.rng().below(100) > 90 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheFishHasResistedYourAttemptToBringItIn),
            );
            self.set_good_use(id, 0);
            self.fish_change_hp(id, 0, pen);
            return;
        }

        let Some((mode, deceptive_mode)) = self
            .players
            .get(&id)
            .and_then(|p| p.fishing.combat.as_ref())
            .map(|c| (c.mode, c.deceptive_mode))
        else {
            return;
        };
        if action.works(mode, deceptive_mode) {
            // Pumping or reeling is successful, damage: $s1.
            self.send(
                id,
                &SystemMessage::new(action.success_message()).with(SystemMessageParam::IntNumber(dmg)),
            );
            if pen > 0 {
                self.send(
                    id,
                    &SystemMessage::new(action.penalty_message()).with(SystemMessageParam::IntNumber(pen)),
                );
            }
            self.set_good_use(id, 1);
            self.fish_change_hp(id, dmg, pen);
        } else {
            // Pumping or reeling failed, the fish regains $s1.
            self.send(
                id,
                &SystemMessage::new(action.failure_message()).with(SystemMessageParam::IntNumber(dmg)),
            );
            self.set_good_use(id, 2);
            self.fish_change_hp(id, dmg.wrapping_neg(), pen);
        }
    }

    fn set_good_use(&mut self, id: ObjectId, value: u8) {
        if let Some(combat) = self.players.get_mut(&id).and_then(|p| p.fishing.combat.as_mut()) {
            combat.good_use = value;
        }
    }

    /// Java `Pumping.onStart` and `Reeling.onStart`: the skill landed. `power` is the effect's
    /// `power` parameter and `skill_level` the level of the skill being used.
    pub(crate) fn pumping_reeling_effect_start(
        &mut self,
        id: ObjectId,
        action: FishAction,
        power: f64,
        skill_level: i32,
    ) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.has_fish_combat(id) {
            // Pumping and reeling skills are available only while fishing.
            self.send(id, &SystemMessage::new(action.only_while_fishing_message()));
            self.send(id, &ActionFailed);
            return;
        }

        let weapon_id = self.stats(id).and_then(|v| v.active_weapon_item()).map(|t| t.id);
        let has_weapon_instance = self
            .paperdoll_object(id, l2_protocol_game::server::charscreen::paperdoll::RHAND)
            .is_some();
        let (Some(weapon_id), true) = (weapon_id, has_weapon_instance) else {
            return;
        };

        let mut ss = 1;
        let mut pen = 0;
        if self.is_charged_shot(id, ShotType::FishSoulshots) {
            ss = 2;
        }

        let Some(rod) = self.fishing.rods.as_ref().and_then(|r| r.get(weapon_id).cloned()) else {
            // Java dereferences the missing rod (a NullPointerException); the skill needs a rod.
            tracing::warn!("Fishing: item {weapon_id} is not a fishing rod.");
            return;
        };
        // TODO(java): "Check this formula (is guessed)".
        let grade_bonus = f64::from(rod.fishing_rod_level) * 0.1;
        let expertise = self
            .stats(id)
            .map_or(1.0, |v| v.calc_stat(Stat::FishingExpertise, 1.0, None, None));
        let mut dmg = ((rod.fishing_rod_damage + expertise + power) * grade_bonus * f64::from(ss)) as i32;

        // Penalty 5% less damage dealt.
        let expertise_level = self
            .stats(id)
            .map_or(0, |v| v.skill_level(FISHING_EXPERTISE_SKILL));
        if expertise_level < skill_level - 2 {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::DueToYourReelingAndOrPumpingSkillBeingThreeOrMoreLevelsHigherThanYourFishingSkillA50DamagePenaltyWillBeApplied,
                ),
            );
            pen = (f64::from(dmg) * 0.05) as i32;
            dmg -= pen;
        }

        if ss > 1 {
            self.set_charged_shot(id, ShotType::FishSoulshots, false);
        }

        self.fish_use(id, action, dmg, pen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pumping_works_on_a_resting_fish_and_reeling_on_a_fighting_one() {
        // mode 0 = resting, 1 = fighting; deceptive 0 = no.
        assert!(FishAction::Pumping.works(0, 0));
        assert!(!FishAction::Pumping.works(1, 0));
        assert!(FishAction::Reeling.works(1, 0));
        assert!(!FishAction::Reeling.works(0, 0));
    }

    #[test]
    fn the_deceptive_mode_reverses_both() {
        assert!(!FishAction::Pumping.works(0, 1));
        assert!(FishAction::Pumping.works(1, 1));
        assert!(!FishAction::Reeling.works(1, 1));
        assert!(FishAction::Reeling.works(0, 1));
    }

    #[test]
    fn anim_is_one_for_pumping_and_two_for_reeling() {
        assert_eq!(FishAction::Pumping.anim(), 1);
        assert_eq!(FishAction::Reeling.anim(), 2);
    }
}
