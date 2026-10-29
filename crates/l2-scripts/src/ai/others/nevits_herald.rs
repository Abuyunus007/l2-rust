//! Nevit's Herald: after Antharas or Valakas die, heralds appear in the towns and offer a buff.
// Java: dist/game/data/scripts/ai/others/NevitsHerald/NevitsHerald.java

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::effects::EffectType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const NEVITS_HERALD: i32 = 4326;
const NEVITS_HERALD_LOC: [(i32, i32, i32, i32); 11] = [
    (86971, -142772, -1336, 20480), // Town of Schuttgart
    (44165, -48494, -792, 32768),   // Rune Township
    (148017, -55264, -2728, 49152), // Town of Goddard
    (147919, 26631, -2200, 16384),  // Town of Aden
    (82325, 53278, -1488, 16384),   // Town of Oren
    (81925, 148302, -3464, 49152),  // Town of Giran
    (111678, 219197, -3536, 49152), // Heine
    (16254, 142808, -2696, 16384),  // Town of Dion
    (-13865, 122081, -2984, 32768), // Town of Gludio
    (-83248, 150832, -3136, 32768), // Gludin Village
    (116899, 77256, -2688, 49152),  // Hunters Village
];
const ANTHARAS: i32 = 29068; // Antharas Strong (85)
const VALAKAS: i32 = 29028; // Valakas (85)

// Skill
const FALL_OF_THE_DRAGON: SkillKey = SkillKey { id: 23312, level: 1 };

/// Java `SPAWNS`.
#[derive(Default)]
struct Spawns(Vec<ScriptNpc>);

/// Java `ai.others.NevitsHerald.NevitsHerald`.
pub struct NevitsHerald;

impl Script for NevitsHerald {
    fn name(&self) -> &'static str {
        "NevitsHerald"
    }

    fn path(&self) -> &'static str {
        "ai/others/NevitsHerald"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[NEVITS_HERALD]);
        s.add_start_npc(&[NEVITS_HERALD]);
        s.add_talk_id(&[NEVITS_HERALD]);
        s.add_kill_id(&[ANTHARAS, VALAKAS]);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("4326.htm".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): Java reads `npc.getId()` first. The "despawn" timer is started without
        // an NPC, so it throws a NullPointerException and the heralds are never removed (nor
        // `SPAWNS` cleared), and the "text_spam" timer of a herald only enters the branch of the
        // herald, which answers "buff" alone: the heralds never shout.
        let npc = npc?;
        if npc.id == NEVITS_HERALD && event.eq_ignore_ascii_case("buff") {
            let player = player?;
            let has_hourglass = ctx
                .world()
                .effect_list(player)
                .is_some_and(|list| list.first_effect(EffectType::NevitsHourglass).is_some());
            if has_hourglass {
                return Some("4326-1.htm".to_owned());
            }

            ctx.cast_skill(npc, player, FALL_OF_THE_DRAGON);
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let message = if npc.id == VALAKAS {
            NpcStringId::TheEvilFireDragonValakasHasBeenDefeated
        } else {
            NpcStringId::TheEvilLandDragonAntharasHasBeenDefeated
        };

        let players: Vec<ObjectId> = ctx.world().players().map(|p| p.data.object_id).collect();
        for online_player in players {
            ctx.show_on_screen_npc_string(online_player, message, 2, 10000, &[]);
        }

        if ctx.state::<Spawns>().0.is_empty() {
            for (x, y, z, heading) in NEVITS_HERALD_LOC {
                if let Some(herald) = ctx.add_spawn(NEVITS_HERALD, x, y, z, heading, false, 0, 0) {
                    ctx.start_quest_timer("text_spam", 3000, Some(herald), None, false);
                    ctx.state::<Spawns>().0.push(herald);
                }
            }

            ctx.start_quest_timer("despawn", 14400000, None, None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="4326" type="Folk" level="1" name="Nevit's Herald"/>
        <npc id="29028" type="Monster" level="85" name="Valakas"><stats><vitals hp="10" mp="10"/></stats></npc>"#;

    #[test]
    fn a_dead_dragon_brings_eleven_heralds_and_a_message_that_never_ends_them() {
        let mut registry = ScriptRegistry::new();
        registry.add(NevitsHerald);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let valakas = t.spawn(VALAKAS, 20);
        t.enter(1, 85, vec![]);
        t.take();
        t.kill(1, valakas);
        assert!(names_to(&t.take(), 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        let heralds = t.npcs_of(NEVITS_HERALD);
        assert_eq!(heralds.len(), 11);
        // The timers run into Java's quirks: the heralds stay after four hours.
        t.wait_ms(3_500);
        assert_eq!(t.npcs_of(NEVITS_HERALD).len(), 11);
        for h in heralds {
            t.world.delete_npc(h);
        }
    }
}
