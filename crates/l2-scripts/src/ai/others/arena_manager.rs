//! The Arena Manager and Director: recovery and buffs for adena.
// Java: dist/game/data/scripts/ai/others/ArenaManager/ArenaManager.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const ARENA_MANAGER: [i32; 2] = [
    31226, // Arena Director (MDT)
    31225, // Arena Manager (Coliseum)
];

// Skill
const BUFFS: [SkillKey; 6] = [
    SkillKey { id: 6805, level: 1 }, // Arena Empower
    SkillKey { id: 6806, level: 1 }, // Arena Acumen
    SkillKey { id: 6807, level: 1 }, // Arena Concentration
    SkillKey { id: 6808, level: 1 }, // Arena Might
    SkillKey { id: 6804, level: 1 }, // Arena Wind Walk
    SkillKey { id: 6812, level: 1 }, // Arena Berserker Spirit
];
const CP_RECOVERY: SkillKey = SkillKey { id: 4380, level: 1 }; // Arena: CP Recovery
const HP_RECOVERY: SkillKey = SkillKey { id: 6817, level: 1 }; // Arena HP Recovery
// Misc
const CP_COST: i64 = 1000;
const HP_COST: i64 = 1000;
const BUFF_COST: i64 = 2000;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `ai.others.ArenaManager.ArenaManager`.
pub struct ArenaManager;

impl Script for ArenaManager {
    fn name(&self) -> &'static str {
        "ArenaManager"
    }

    fn path(&self) -> &'static str {
        "ai/others/ArenaManager"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&ARENA_MANAGER);
        s.add_talk_id(&ARENA_MANAGER);
        s.add_first_talk_id(&ARENA_MANAGER);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "CPrecovery" => {
                let player = player?;
                if ctx.adena_of(player) >= CP_COST {
                    ctx.take_items(player, ADENA_ID, CP_COST);
                    ctx.start_quest_timer("CPrecovery_delay", 2000, npc, Some(player), false);
                } else {
                    ctx.send_system_message(player, SystemMessageId::YouDoNotHaveEnoughAdena);
                }
            }
            "CPrecovery_delay" => {
                if let (Some(player), Some(npc)) = (player, npc) {
                    if !ctx.is_inside_pvp_zone(player) {
                        ctx.cast_skill(npc, player, CP_RECOVERY);
                    }
                }
            }
            "HPrecovery" => {
                let player = player?;
                if ctx.adena_of(player) >= HP_COST {
                    ctx.take_items(player, ADENA_ID, HP_COST);
                    ctx.start_quest_timer("HPrecovery_delay", 2000, npc, Some(player), false);
                } else {
                    ctx.send_system_message(player, SystemMessageId::YouDoNotHaveEnoughAdena);
                }
            }
            "HPrecovery_delay" => {
                if let (Some(player), Some(npc)) = (player, npc) {
                    if !ctx.is_inside_pvp_zone(player) {
                        ctx.cast_skill(npc, player, HP_RECOVERY);
                    }
                }
            }
            "Buff" => {
                let player = player?;
                if ctx.adena_of(player) >= BUFF_COST {
                    ctx.take_items(player, ADENA_ID, BUFF_COST);
                    if let Some(npc) = npc {
                        for skill in BUFFS {
                            ctx.cast_skill(npc, player, skill);
                        }
                    }
                } else {
                    ctx.send_system_message(player, SystemMessageId::YouDoNotHaveEnoughAdena);
                }
            }
            _ => {}
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="31225" type="Folk" level="70" name="Arena Manager"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/others/ArenaManager";

    fn setup(adena: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ArenaManager);
        let main = r#"<html><body>main
<a action="bypass -h Script ArenaManager CPrecovery">c</a>
<a action="bypass -h Script ArenaManager HPrecovery">h</a>
<a action="bypass -h Script ArenaManager Buff">b</a></body></html>"#;
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[(&format!("{DIR}/31225.html"), main)]);
        let npc = t.spawn(31225, 20);
        t.enter(1, 75, vec![item(900, ADENA_ID, adena)]);
        t.talk(1, npc);
        t.take();
        (t, npc)
    }

    #[test]
    fn the_first_page_is_the_npc_id() {
        let (mut t, npc) = setup(0);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("main"));
    }

    #[test]
    fn recovery_takes_adena_and_casts_after_two_seconds() {
        let (mut t, _) = setup(5000);
        t.bypass(1, "Script ArenaManager CPrecovery");
        assert_eq!(t.count(1, ADENA_ID), 4000);
        t.bypass(1, "Script ArenaManager HPrecovery");
        assert_eq!(t.count(1, ADENA_ID), 3000);
        // 2 s = 40 ticks of 50 ms; the timers fire without a skill table (nothing to cast).
        t.ticks(45);
        assert_eq!(t.world.quest_timer_count(), 0);
    }

    #[test]
    fn buff_costs_two_thousand() {
        let (mut t, _) = setup(5000);
        t.bypass(1, "Script ArenaManager Buff");
        assert_eq!(t.count(1, ADENA_ID), 3000);
    }

    #[test]
    fn without_adena_the_player_is_told() {
        let (mut t, _) = setup(999);
        for event in ["CPrecovery", "HPrecovery", "Buff"] {
            t.bypass(1, &format!("Script ArenaManager {event}"));
            let out = t.take();
            assert!(
                names_to(&out, 1).contains(&"SYSTEM_MESSAGE".to_owned()),
                "{event}"
            );
        }
        assert_eq!(t.count(1, ADENA_ID), 999);
        assert_eq!(t.world.quest_timer_count(), 0);
    }
}
