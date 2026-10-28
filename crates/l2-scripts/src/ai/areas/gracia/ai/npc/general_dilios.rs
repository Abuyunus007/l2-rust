//! General Dilios and his guards at the Keucereus Alliance Base: the commands and the salute.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/GeneralDilios/GeneralDilios.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::util::broadcast_social_action;

const GENERAL_ID: i32 = 32549;
const GUARD_ID: i32 = 32619;

const DILIOS_TEXT: [NpcStringId; 3] = [
    NpcStringId::MessengerInformThePatronsOfTheKeucereusAllianceBaseWeReGatheringBraveAdventurersToAttackTiatSMountedTroopThatSRootedInTheSeedOfDestruction,
    NpcStringId::MessengerInformTheBrothersInKucereusClanOutpostBraveAdventurersWhoHaveChallengedTheSeedOfInfinityAreCurrentlyInfiltratingTheHallOfErosionThroughTheDefensivelyWeakHallOfSuffering,
    NpcStringId::MessengerInformThePatronsOfTheKeucereusAllianceBaseTheSeedOfInfinityIsCurrentlySecuredUnderTheFlagOfTheKeucereusAlliance,
];

/// Java's fields `_general` and `_guards`.
#[derive(Default)]
struct DiliosState {
    general: Option<ObjectId>,
    /// `Set<Spawn>`: the spawn index when the NPC has one (the set keeps one entry per spawn, and
    /// `getLastSpawn()` is the NPC that stands there now), and the NPC.
    guards: Vec<(Option<usize>, ObjectId)>,
}

/// Java `ai.areas.Gracia.AI.NPC.GeneralDilios.GeneralDilios`.
pub struct GeneralDilios;

impl Script for GeneralDilios {
    fn name(&self) -> &'static str {
        "GeneralDilios"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/GeneralDilios"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[GENERAL_ID, GUARD_ID]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(rest) = event.strip_prefix("command_") {
            // `Integer.parseInt(event.substring(8))`
            let mut value: i32 = rest.parse().ok()?;
            let general = ctx.state::<DiliosState>().general;
            let general = general.and_then(|g| ctx.script_npc(g));
            if value < 6 {
                if let Some(general) = general {
                    ctx.broadcast_npc_say(general, ChatType::NpcGeneral, NpcStringId::StabbingThreeTimes, &[]);
                }
                ctx.start_quest_timer("guard_animation_0", 3400, None, None, false);
            } else {
                value = -1;
                if let Some(general) = general {
                    let text = ctx.get_random_entry(&DILIOS_TEXT)?;
                    ctx.broadcast_npc_say(general, ChatType::NpcShout, text, &[]);
                }
            }

            ctx.start_quest_timer(&format!("command_{}", value + 1), 60000, None, None, false);
        } else if let Some(rest) = event.strip_prefix("guard_animation_") {
            let value: i32 = rest.parse().ok()?;
            let guards: Vec<ObjectId> = ctx.state::<DiliosState>().guards.iter().map(|(_, g)| *g).collect();
            for guard in guards {
                broadcast_social_action(ctx, guard, 4);
            }

            if value < 2 {
                ctx.start_quest_timer(&format!("guard_animation_{}", value + 1), 1500, None, None, false);
            }
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == GENERAL_ID {
            ctx.start_quest_timer("command_0", 60000, None, None, false);
            ctx.state::<DiliosState>().general = Some(npc.object_id);
        } else if npc.id == GUARD_ID {
            let spawn = ctx.world().npc(npc.object_id).and_then(|n| n.spawn);
            let guards = &mut ctx.state::<DiliosState>().guards;
            match spawn.and_then(|s| guards.iter().position(|(known, _)| *known == Some(s))) {
                Some(at) => guards[at].1 = npc.object_id,
                None => guards.push((spawn, npc.object_id)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="32549" type="Folk" level="70" name="Dilios"/>
        <npc id="32619" type="Folk" level="70" name="Guard"/>"#;

    #[test]
    fn the_general_stabs_three_times_and_the_guards_salute() {
        let mut registry = ScriptRegistry::new();
        registry.add(GeneralDilios);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.spawn(GENERAL_ID, 20);
        t.spawn(GUARD_ID, 30);
        t.ticks(1);
        t.take();

        // 60 s after the general appeared: "Stabbing three times!"
        t.ticks(1190);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "NPC_SAY"));
        t.ticks(15);
        let out = names_to(&t.take(), 1);
        assert_eq!(out.iter().filter(|n| *n == "NPC_SAY").count(), 1, "{out:?}");

        // 3.4 s later the guards salute (social action 4), three times 1.5 s apart.
        t.ticks(70);
        let out = names_to(&t.take(), 1);
        assert_eq!(out.iter().filter(|n| *n == "SOCIAL_ACTION").count(), 1, "{out:?}");
        t.ticks(100);
        let out = names_to(&t.take(), 1);
        assert_eq!(out.iter().filter(|n| *n == "SOCIAL_ACTION").count(), 2, "{out:?}");
    }

    #[test]
    fn the_sixth_command_is_a_shout() {
        let mut registry = ScriptRegistry::new();
        registry.add(GeneralDilios);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.spawn(GENERAL_ID, 20);
        t.ticks(1);
        t.take();
        // Commands 0 to 5 stab (one a minute, the first after a minute), command 6 shouts.
        t.ticks(1200 * 7 + 20);
        let out = names_to(&t.take(), 1);
        assert_eq!(out.iter().filter(|n| *n == "NPC_SAY").count(), 7, "{out:?}");
    }
}
