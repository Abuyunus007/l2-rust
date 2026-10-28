//! Quest 3: Will the Seal be Broken? (Talloth in Dark Elven Village sends a dark elf out to hunt monsters).
// Java: dist/game/data/scripts/quests/Q00003_WillTheSealBeBroken/Q00003_WillTheSealBeBroken.java
//
// Pattern: a kill chain with one item per monster kind (`docs/design/scripts.md`, "Quest
// patterns"): the drawn party member gets the item for the kill, and their quest goes to cond 2
// when all three kinds are in. Then the start NPC gives the reward (one-time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TALLOTH: i32 = 30141;

// Monsters
const OMEN_BEAST: i32 = 20031;
const TAINTED_ZOMBIE: i32 = 20041;
const STINK_ZOMBIE: i32 = 20046;
const LESSER_SUCCUBUS: i32 = 20048;
const LESSER_SUCCUBUS_TUREN: i32 = 20052;
const LESSER_SUCCUBUS_TILFO: i32 = 20057;

// Items
const OMEN_BEAST_EYE: i32 = 1081;
const TAINT_STONE: i32 = 1082;
const SUCCUBUS_BLOOD: i32 = 1083;
const ENCHANT: i32 = 956;

// Misc
const MIN_LEVEL: i32 = 16;

/// Java `Q00003_WillTheSealBeBroken`.
pub struct Q00003WillTheSealBeBroken;

impl Q00003WillTheSealBeBroken {
    /// Java `giveItem(player, qs, item, getRegisteredItemIds())`: the member gets the item once,
    /// and the quest moves to cond 2 when all the registered items are in.
    fn give_item(ctx: &mut ScriptCtx<'_>, member: ObjectId, item: i32) {
        if ctx.has_quest_items(member, item) {
            return;
        }
        ctx.give_items(member, item, 1);
        ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        let registered = [OMEN_BEAST_EYE, TAINT_STONE, SUCCUBUS_BLOOD];
        if ctx.has_all_quest_items(member, &registered) {
            // Java calls `qs.setCond` on the member's state; a member without one has nothing to move.
            if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }
}

impl Script for Q00003WillTheSealBeBroken {
    fn name(&self) -> &'static str {
        "Q00003_WillTheSealBeBroken"
    }

    fn path(&self) -> &'static str {
        "quests/Q00003_WillTheSealBeBroken"
    }

    fn id(&self) -> i32 {
        3
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TALLOTH]);
        s.add_talk_id(&[TALLOTH]);
        s.add_kill_id(&[
            OMEN_BEAST,
            TAINTED_ZOMBIE,
            STINK_ZOMBIE,
            LESSER_SUCCUBUS,
            LESSER_SUCCUBUS_TILFO,
            LESSER_SUCCUBUS_TUREN,
        ]);
        s.register_quest_items(&[OMEN_BEAST_EYE, TAINT_STONE, SUCCUBUS_BLOOD]);
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

        match event {
            "30141-03.htm" => qs.start_quest(ctx),
            "30141-05.html" => {}
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 1) else {
            return;
        };
        match npc.id {
            OMEN_BEAST => Self::give_item(ctx, member, OMEN_BEAST_EYE),
            STINK_ZOMBIE | TAINTED_ZOMBIE => Self::give_item(ctx, member, TAINT_STONE),
            LESSER_SUCCUBUS | LESSER_SUCCUBUS_TILFO | LESSER_SUCCUBUS_TUREN => {
                Self::give_item(ctx, member, SUCCUBUS_BLOOD);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) != race::DARK_ELF {
                    "30141-00.htm"
                } else if ctx.player_level(player) >= MIN_LEVEL {
                    "30141-02.htm"
                } else {
                    "30141-01.html"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30141-04.html".to_owned();
                } else {
                    ctx.give_items(player, ENCHANT, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30141-06.html".to_owned();
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

    const NAME: &str = "Q00003_WillTheSealBeBroken";

    const NPCS: &str = r#"
        <npc id="30141" type="Folk" level="70" name="Talloth"/>
        <npc id="20031" type="Monster" level="12" name="Omen Beast"/>
        <npc id="20041" type="Monster" level="12" name="Tainted Zombie"/>
        <npc id="20046" type="Monster" level="12" name="Stink Zombie"/>
        <npc id="20048" type="Monster" level="12" name="Lesser Succubus"/>
        <npc id="20052" type="Monster" level="12" name="Lesser Succubus Turen"/>
        <npc id="20057" type="Monster" level="12" name="Lesser Succubus Tilfo"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1081" type="EtcItem" name="Omen Beast's Eye"><set name="is_questitem" val="true"/></item>
        <item id="1082" type="EtcItem" name="Taint Stone"><set name="is_questitem" val="true"/></item>
        <item id="1083" type="EtcItem" name="Succubus Blood"><set name="is_questitem" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (Grade D)"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        talloth: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00003WillTheSealBeBroken);
        let pages = Pages::of(&Q00003WillTheSealBeBroken)
            .page("30141-02.htm", &["30141-03.htm"])
            .page("30141-04.html", &["30141-05.html"])
            .plain(&[
                "30141-00.htm",
                "30141-01.html",
                "30141-03.htm",
                "30141-05.html",
                "30141-06.html",
            ])
            .npcs(&[TALLOTH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            talloth: t.spawn(TALLOTH, 20),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_dark_elf_of_level_sixteen_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 20, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.talloth).contains("30141-00.htm"));
        t.enter_as(2, 10, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.talloth).contains("30141-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_monsters_and_the_enchant_scroll() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 16, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.talloth).contains("30141-02.htm"));
        assert!(t.event(1, NAME, "30141-03.htm").contains("30141-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // One item per monster kind; the second of a kind gives nothing more.
        let omen = t.spawn(OMEN_BEAST, 60);
        t.kill(1, omen);
        assert_eq!(t.count(1, OMEN_BEAST_EYE), 1);
        let zombie = t.spawn(STINK_ZOMBIE, 60);
        t.kill(1, zombie);
        assert_eq!(t.count(1, TAINT_STONE), 1);
        assert!(t.take_writes().is_empty());
        let succubus = t.spawn(LESSER_SUCCUBUS_TILFO, 60);
        t.kill(1, succubus);
        assert_eq!(t.count(1, SUCCUBUS_BLOOD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Talloth: the reward and `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.talloth).contains("30141-06.html"));
        assert_eq!(t.count(1, ENCHANT), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.talloth)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn talloth_waits_until_the_monsters_are_done() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 16, race::DARK_ELF, 0, vec![]);
        t.quest_page(1, npcs.talloth);
        t.event(1, NAME, "30141-03.htm");
        let zombie = t.spawn(TAINTED_ZOMBIE, 60);
        t.kill(1, zombie);
        assert_eq!(t.count(1, TAINT_STONE), 1);
        t.take_writes();
        assert!(t.quest_page(1, npcs.talloth).contains("30141-04.html"));
        assert!(t.event(1, NAME, "30141-05.html").contains("30141-05.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, ENCHANT), 0);
    }
}
