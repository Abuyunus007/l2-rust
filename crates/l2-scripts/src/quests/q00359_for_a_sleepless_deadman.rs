//! Quest 359: For a Sleepless Deadman (High Priest Orven, the doom monsters of the Devastated Castle).
// Java: dist/game/data/scripts/quests/Q00359_ForASleeplessDeadman/Q00359_ForASleeplessDeadman.java
//
// Pattern: a kill-count quest with a drop chance up to a limit and a party check at cond 1
// (`random_party_member_state`, like Q00295), then a memo state marks the stages: 1 = collecting,
// 2 = purified (`set_memo_state`). The reward is four random pieces.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ORVEN: i32 = 30857;

// Item
const REMAINS_OF_ADEN_RESIDENTS: i32 = 5869;

// Misc
const MIN_LEVEL: i32 = 60;
const REMAINS_COUNT: i64 = 60;

// Rewards
const REWARDS: [i32; 8] = [
    5494, // Sealed Dark Crystal Shield Fragment
    5495, // Sealed Shield of Nightmare Fragment
    6341, // Sealed Phoenix Earring Gemstone
    6342, // Sealed Majestic Earring Gemstone
    6343, // Sealed Phoenix Necklace Beads
    6344, // Sealed Majestic Necklace Beads
    6345, // Sealed Phoenix Ring Gemstone
    6346, // Sealed Majestic Ring Gemstone
];

// Mobs (drop chance)
const MOBS: [(i32, f64); 3] = [
    (21006, 0.365), // doom_servant
    (21007, 0.392), // doom_guard
    (21008, 0.503), // doom_archer
];

/// Java `Q00359_ForASleeplessDeadman`.
pub struct Q00359ForASleeplessDeadman;

impl Script for Q00359ForASleeplessDeadman {
    fn name(&self) -> &'static str {
        "Q00359_ForASleeplessDeadman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00359_ForASleeplessDeadman"
    }

    fn id(&self) -> i32 {
        359
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORVEN]);
        s.add_talk_id(&[ORVEN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[REMAINS_OF_ADEN_RESIDENTS]);
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
        let mut htmltext = None;
        match event {
            "30857-02.htm" | "30857-03.htm" | "30857-04.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30857-05.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30857-10.html" => {
                if let Some(reward) = ctx.get_random_entry(&REWARDS) {
                    ctx.reward_items(player, reward, 4);
                }
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, 1, 3, Some(npc));
        if let Some(qs) = qs {
            let chance = MOBS.iter().find(|(id, _)| *id == npc.id).map_or(0.0, |(_, c)| *c);
            if ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                REMAINS_OF_ADEN_RESIDENTS,
                1,
                1,
                REMAINS_COUNT,
                chance,
                true,
            ) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30857-01.htm"
            } else {
                "30857-06.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if qs.is_memo_state(ctx, 1) {
                if ctx.get_quest_items_count(player, REMAINS_OF_ADEN_RESIDENTS) < REMAINS_COUNT {
                    htmltext = "30857-07.html".to_owned();
                } else {
                    ctx.take_items(player, REMAINS_OF_ADEN_RESIDENTS, -1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 3, true);
                    htmltext = "30857-08.html".to_owned();
                }
            } else if qs.is_memo_state(ctx, 2) {
                htmltext = "30857-09.html".to_owned();
            }
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

    const NAME: &str = "Q00359_ForASleeplessDeadman";

    const NPCS: &str = r#"
        <npc id="30857" type="Folk" level="70" name="High Priest Orven"/>
        <npc id="21006" type="Monster" level="60" name="Doom Servant"/>
        <npc id="21007" type="Monster" level="60" name="Doom Guard"/>
        <npc id="21008" type="Monster" level="60" name="Doom Archer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5869" type="EtcItem" name="Remains of Aden Residents"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5494" type="EtcItem" name="Sealed Dark Crystal Shield Fragment"/>
        <item id="5495" type="EtcItem" name="Sealed Shield of Nightmare Fragment"/>
        <item id="6341" type="EtcItem" name="Sealed Phoenix Earring Gemstone"/>
        <item id="6342" type="EtcItem" name="Sealed Majestic Earring Gemstone"/>
        <item id="6343" type="EtcItem" name="Sealed Phoenix Necklace Beads"/>
        <item id="6344" type="EtcItem" name="Sealed Majestic Necklace Beads"/>
        <item id="6345" type="EtcItem" name="Sealed Phoenix Ring Gemstone"/>
        <item id="6346" type="EtcItem" name="Sealed Majestic Ring Gemstone"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00359ForASleeplessDeadman);
        let pages = Pages::of(&Q00359ForASleeplessDeadman)
            .page("30857-01.htm", &["30857-02.htm"])
            .page("30857-02.htm", &["30857-03.htm"])
            .page("30857-03.htm", &["30857-04.htm"])
            .page("30857-04.htm", &["30857-05.htm"])
            .plain(&["30857-05.htm", "30857-06.html", "30857-07.html", "30857-08.html"])
            .page("30857-09.html", &["30857-10.html"])
            .plain(&["30857-10.html"])
            .npcs(&[ORVEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let orven = t.spawn(ORVEN, 20);
        (t, orven)
    }

    /// Kills the doom monsters in turns until the player has `remains` remains.
    fn hunt(t: &mut ScriptTest, remains: i64) {
        for kills in 0..600 {
            if t.count(1, REMAINS_OF_ADEN_RESIDENTS) >= remains {
                return;
            }
            let mob = t.spawn(MOBS[kills % 3].0, 60);
            t.kill(1, mob);
        }
        panic!("no {remains} remains after 600 kills");
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, orven) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, orven).contains("30857-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn sixty_remains_then_the_purification_and_four_pieces() {
        let (mut t, orven) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, orven).contains("30857-01.htm"));
        for page in ["30857-02.htm", "30857-03.htm", "30857-04.htm"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert!(t.event(1, NAME, "30857-05.htm").contains("30857-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, orven).contains("30857-07.html"));

        // The 60th remains sets cond 2 (`giveItemRandomly` with the limit).
        hunt(&mut t, REMAINS_COUNT);
        assert_eq!(t.count(1, REMAINS_OF_ADEN_RESIDENTS), REMAINS_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The ashes are purified: the remains go, memo state 2 and cond 3.
        assert!(t.quest_page(1, orven).contains("30857-08.html"));
        assert_eq!(t.count(1, REMAINS_OF_ADEN_RESIDENTS), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "3"),
            ]
        );
        assert!(t.quest_page(1, orven).contains("30857-09.html"));

        // Four pieces of the random kind; the quest ends repeatable.
        assert!(t.event(1, NAME, "30857-10.html").contains("30857-10.html"));
        let pieces: i64 = REWARDS.iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(pieces, 4);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, orven).contains("30857-01.htm"));
    }
}
