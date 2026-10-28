//! The Sin Eater servitor: it complains about everything.
// Java: dist/game/data/scripts/ai/others/Servitors/SinEater.java
//
// Java registers `addSummonSpawnId` / `addSummonTalkId` and `@RegisterEvent` listeners for the
// summon. The summon is an NPC object of the world here, so its spawn is `on_spawn`, its death the
// per-id creature death, and the attack and talk listeners global ones that look at the id.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TerminateReturn};

// NPCs
const SIN_EATER: i32 = 12564;

/// Java `broadcastSummonSay(summon, npcStringId)`.
fn broadcast_summon_say(ctx: &mut ScriptCtx<'_>, summon: ObjectId, text: NpcStringId) {
    if let Some(npc) = ctx.script_npc(summon) {
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
    }
}

/// Java `ai.others.Servitors.SinEater`.
pub struct SinEater;

impl Script for SinEater {
    fn name(&self) -> &'static str {
        "SinEater"
    }

    fn path(&self) -> &'static str {
        "ai/others/Servitors"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[SIN_EATER]); // addSummonSpawnId
        s.set_on_player_summon_talk(); // addSummonTalkId
        s.add_creature_death_id(&[SIN_EATER]);
        s.set_on_creature_attacked();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "TALK" {
            if let Some(player) = player {
                if let Some(summon) = ctx.summon_of(player) {
                    if ctx.get_random(100) < 30 {
                        let random = ctx.get_random(100);
                        let text = if random < 20 {
                            NpcStringId::YawwwwnItSSoBoringHereWeShouldGoAndFindSomeAction
                        } else if random < 40 {
                            NpcStringId::HeyIfYouContinueToWasteTimeYouWillNeverFinishYourPenance
                        } else if random < 60 {
                            NpcStringId::IKnowYouDonTLikeMeTheFeelingIsMutual
                        } else if random < 80 {
                            NpcStringId::INeedADrink
                        } else {
                            NpcStringId::OhThisIsDraggingOnTooLongAtThisRateIWonTMakeItHomeBeforeTheSevenSealsAreBroken
                        };
                        broadcast_summon_say(ctx, summon, text);
                    }

                    ctx.start_quest_timer("TALK", 60000, None, Some(player), false);
                }
            }
        }

        None
    }

    fn on_creature_death(&self, ctx: &mut ScriptCtx<'_>, _killer: Option<ObjectId>, victim: ObjectId) {
        let random = ctx.get_random(100);
        let text = if random < 30 {
            NpcStringId::OhThisIsJustGreatWhatAreYouGoingToDoNow
        } else if random < 70 {
            NpcStringId::YouInconsiderateMoronCanTYouEvenTakeCareOfLittleOldMe
        } else {
            NpcStringId::OhNoTheManWhoEatsOneSSinsHasDiedPenitenceIsFurtherAway
        };
        broadcast_summon_say(ctx, victim, text);
    }

    fn on_creature_attacked(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _attacker: ObjectId,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        if ctx.script_npc(target).is_some_and(|n| n.id == SIN_EATER) && ctx.get_random(100) < 30 {
            let random = ctx.get_random(100);
            let text = if random < 35 {
                NpcStringId::OhThatSmarts
            } else if random < 70 {
                NpcStringId::HeyMasterPayAttentionIMDyingOverHere
            } else {
                NpcStringId::WhatHaveIDoneToDeserveThis
            };
            broadcast_summon_say(ctx, target, text);
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let text = if ctx.get_random_boolean() {
            NpcStringId::HeyItSeemsLikeYouNeedMyHelpDoesnTIt
        } else {
            NpcStringId::AlmostGotItOuchStopDamnTheseBloodyManacles
        };
        broadcast_summon_say(ctx, npc.object_id, text);
        if let Some(owner) = ctx.world().summon_owner(npc.object_id) {
            ctx.start_quest_timer("TALK", 60000, None, Some(owner), false);
        }
    }

    fn on_player_summon_talk(&self, ctx: &mut ScriptCtx<'_>, summon: ObjectId) {
        if !ctx.script_npc(summon).is_some_and(|n| n.id == SIN_EATER) {
            return;
        }
        if ctx.get_random(100) < 10 {
            let random = ctx.get_random(100);
            let text = if random < 25 {
                NpcStringId::UsingASpecialSkillHereCouldTriggerABloodbath
            } else if random < 50 {
                NpcStringId::HeyWhatDoYouExpectOfMe
            } else if random < 75 {
                NpcStringId::UggggghPushItSNotComingOut
            } else {
                NpcStringId::AhIMissedTheMark
            };
            broadcast_summon_say(ctx, summon, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPC_DEFS: &str = r#"<npc id="12564" type="Folk" level="1" name="Sin Eater"/>"#;

    #[test]
    fn the_sin_eater_complains_when_it_dies_and_is_attacked() {
        let mut registry = ScriptRegistry::new();
        registry.add(SinEater);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let eater = t.spawn(SIN_EATER, 20);
        t.enter(1, 60, vec![]);
        t.take();
        t.world.run_as_script("SinEater", |ctx| {
            SinEater.on_creature_death(ctx, None, eater);
        });
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));

        // 30 % of the attacks make it talk: within 60 attacks it has.
        t.world.run_as_script("SinEater", |ctx| {
            for _ in 0..60 {
                SinEater.on_creature_attacked(ctx, ObjectId(1), eater);
            }
        });
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
    }

    #[test]
    fn the_summon_talk_event_needs_a_summon() {
        let mut registry = ScriptRegistry::new();
        registry.add(SinEater);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        t.enter(1, 60, vec![]);
        let before = t.world.quest_timer_count();
        t.world.run_as_script("SinEater", |ctx| {
            SinEater.on_event(ctx, "TALK", None, Some(ObjectId(1)));
        });
        // `player.getSummon() != null` is false: no new timer.
        assert_eq!(t.world.quest_timer_count(), before);
    }
}
