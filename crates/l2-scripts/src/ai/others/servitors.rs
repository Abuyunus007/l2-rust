//! The Test of the Summoner: a servitor killed by one of the quest monsters turns the crystals
//! of the master from starting/in progress to defeat.
// Java: dist/game/data/scripts/ai/others/Servitors/Servitors.java

use l2_util::ObjectId;
use l2_world::{QuestSound, Script, ScriptCtx, ScriptSetup};

/// Java `Q00230_TestOfTheSummoner.class.getSimpleName()`.
const Q00230_TEST_OF_THE_SUMMONER: &str = "Q00230_TestOfTheSummoner";

// Quest Monster
const PAKO_THE_CAT: i32 = 27102;
const UNICORN_RACER: i32 = 27103;
const SHADOW_TUREN: i32 = 27104;
const MIMI_THE_CAT: i32 = 27105;
const UNICORN_PHANTASM: i32 = 27106;
const SILHOUETTE_TILFO: i32 = 27107;

// Items
const CRYSTAL_OF_INPROGRESS_3RD: i32 = 3371;

/// Java `MONSTERS.get(npcId)`: (starting, in progress, defeat) crystal.
fn crystals(monster: i32) -> Option<(i32, i32, i32)> {
    match monster {
        PAKO_THE_CAT => Some((3360, 3361, 3363)),
        UNICORN_RACER => Some((3370, 3371, 3373)),
        SHADOW_TUREN => Some((3380, 3381, 3383)),
        MIMI_THE_CAT => Some((3365, 3366, 3368)),
        UNICORN_PHANTASM => Some((3375, 3376, 3378)),
        SILHOUETTE_TILFO => Some((3385, 3386, 3388)),
        _ => None,
    }
}

/// The servitors that carry the listener (Java `@Id`).
const SERVITORS: [i32; 24] = [
    // Kat the Cat
    14111, 14112, 14113, 14114, // Mew the Cat
    14159, 14160, 14161, 14162, // Boxer the Unicorn
    14295, 14296, 14297, 14298, // Mirage the Unicorn
    14343, 14344, 14345, 14346, // Shadow
    14479, 14480, 14481, 14482, // Silhouette
    14527, 14528, 14529, 14530,
];

/// Java `ai.others.Servitors.Servitors`.
pub struct Servitors;

impl Script for Servitors {
    fn name(&self) -> &'static str {
        "Servitors"
    }

    fn path(&self) -> &'static str {
        "ai/others/Servitors"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_creature_death_id(&SERVITORS);
    }

    fn on_creature_death(&self, ctx: &mut ScriptCtx<'_>, killer: Option<ObjectId>, victim: ObjectId) {
        // `event.getAttacker().isNpc() && event.getTarget().isServitor()`
        let Some(killer) = killer.and_then(|k| ctx.script_npc(k)) else {
            return;
        };
        if ctx.world().is_servitor(victim) && ctx.check_if_in_range(1500, killer.object_id, victim, true) {
            let Some(master) = ctx.world().summon_owner(victim) else {
                return;
            };
            if ctx
                .player_quest_state(master, Q00230_TEST_OF_THE_SUMMONER)
                .is_some()
                && ctx.has_quest_items(master, CRYSTAL_OF_INPROGRESS_3RD)
            {
                // NOTE(java-quirk): `MONSTERS.get(killer.getId())` is null for any other NPC and
                // Java throws; nothing happens here.
                let Some((starting, in_progress, defeat)) = crystals(killer.id) else {
                    return;
                };
                ctx.give_items(master, defeat, 1); // Crystal of Defeat
                ctx.play_sound(master, QuestSound::ItemsoundQuestItemget);
                ctx.take_items(master, in_progress, -1); // Crystal of Inprogress
                ctx.take_items(master, starting, -1); // Crystal of Starting
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crystal_table_is_javas() {
        assert_eq!(crystals(UNICORN_RACER), Some((3370, 3371, 3373)));
        assert_eq!(crystals(SILHOUETTE_TILFO), Some((3385, 3386, 3388)));
        assert_eq!(crystals(1), None);
        assert_eq!(SERVITORS.len(), 24);
    }
}
