//! Quest 170: Dangerous Seduction (Velior in the Dark Elven Village, Merkenis kills).
// Java: dist/game/data/scripts/quests/Q00170_DangerousSeduction/Q00170_DangerousSeduction.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VELIOR: i32 = 30305;

// Monster
const MERKENIS: i32 = 27022;

// Item
const NIGHTMARE_CRYSTAL: i32 = 1046;

// Misc
const MIN_LEVEL: i32 = 21;

/// Java `Q00170_DangerousSeduction`.
pub struct Q00170DangerousSeduction;

impl Script for Q00170DangerousSeduction {
    fn name(&self) -> &'static str {
        "Q00170_DangerousSeduction"
    }

    fn path(&self) -> &'static str {
        "quests/Q00170_DangerousSeduction"
    }

    fn id(&self) -> i32 {
        170
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VELIOR]);
        s.add_talk_id(&[VELIOR]);
        s.add_kill_id(&[MERKENIS]);
        s.register_quest_items(&[NIGHTMARE_CRYSTAL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        if event.eq_ignore_ascii_case("30305-04.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_cond(ctx, 1)
        {
            qs.set_cond(ctx, 2, true);
            ctx.give_items(killer, NIGHTMARE_CRYSTAL, 1);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::SendMySoulToLichKingIcarus,
                &[],
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30305-01.htm"
                    } else {
                        "30305-02.htm"
                    }
                } else {
                    "30305-03.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30305-05.html".to_owned();
                } else {
                    ctx.give_adena(player, 102_680, true);
                    ctx.add_exp_and_sp(player, 38_607, 4018);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30305-06.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00170_DangerousSeduction";

    const NPCS: &str = r#"
        <npc id="30305" type="Folk" level="70" name="Velior"/>
        <npc id="27022" type="Monster" level="28" name="Merkenis"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1046" type="EtcItem" name="Nightmare Crystal"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00170DangerousSeduction);
        let pages = Pages::of(&Q00170DangerousSeduction)
            .page("30305-01.htm", &["30305-04.htm"])
            .plain(&[
                "30305-02.htm",
                "30305-03.htm",
                "30305-04.htm",
                "30305-05.html",
                "30305-06.html",
            ])
            .npcs(&[VELIOR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let velior = t.spawn(VELIOR, 20);
        (t, velior)
    }

    #[test]
    fn only_a_dark_elf_of_level_21_is_offered_the_quest() {
        let (mut t, velior) = setup();
        t.enter_as(1, 30, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, velior).contains("30305-03.htm"));
        assert!(t.take_writes().is_empty());

        t.enter_as(2, 20, race::DARK_ELF, 31, vec![]);
        assert!(t.quest_page(2, velior).contains("30305-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_crystal_then_the_reward() {
        let (mut t, velior) = setup();
        t.enter_as(1, 21, race::DARK_ELF, 31, vec![]);
        assert!(t.quest_page(1, velior).contains("30305-01.htm"));
        assert!(t.event(1, NAME, "30305-04.htm").contains("30305-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, velior).contains("30305-05.html"));

        // Merkenis drops the crystal and sets cond 2 (`setCond(2, true)`).
        let mob = t.spawn(MERKENIS, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, NIGHTMARE_CRYSTAL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: adena, exp and sp, then `exitQuest(false, true)`.
        assert!(t.quest_page(1, velior).contains("30305-06.html"));
        assert_eq!(t.count(1, 57), 102_680);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, velior).contains("already been completed"));
    }
}
