//! Quest 287: Figuring It Out! (Laki in the Rune area trades the tanta lizardmen's vials for Moirai and Icarus gear).
// Java: dist/game/data/scripts/quests/Q00287_FiguringItOut/Q00287_FiguringItOut.java
//
// Pattern: a repeatable collect quest with a party drop (`docs/design/scripts.md`, "Quest
// patterns"): the tanta lizardmen drop vials of tanta blood with a chance per monster; Laki gives
// one random item of a table for 100 vials (Moirai) or 500 vials (Icarus). The quest needs the
// quest Q00250_WatchWhatYouEat completed, which this server reads through `player_quest_state`.
// Both item choices come from bypasses of the last page, not from the NPC's talk.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LAKI: i32 = 32742;

// Item
const VIAL_OF_TANTA_BLOOD: i32 = 15499;

/// Java `MONSTERS`: the tanta lizardmen and their drop chance out of 1000.
fn vial_chance(npc_id: i32) -> i32 {
    match npc_id {
        22768 => 509, // Tanta Lizardman Scout
        22769 => 689, // Tanta Lizardman Warrior
        22770 => 123, // Tanta Lizardman Soldier
        22771 => 159, // Tanta Lizardman Berserker
        22772 => 739, // Tanta Lizardman Archer
        22773 => 737, // Tanta Lizardman Magician
        22774 => 261, // Tanta Lizardman Summoner
        _ => 0,
    }
}

/// Java `MOIRAI`: the Moirai rewards, `(item, count)`.
const MOIRAI: [(i32, i64); 16] = [
    (15776, 1),
    (15779, 1),
    (15782, 1),
    (15785, 1),
    (15788, 1),
    (15812, 1),
    (15813, 1),
    (15814, 1),
    (15646, 5),
    (15649, 5),
    (15652, 5),
    (15655, 5),
    (15658, 5),
    (15772, 1),
    (15773, 1),
    (15774, 1),
];

/// Java `ICARUS`: the Icarus rewards, `(item, count)`.
const ICARUS: [(i32, i64); 5] = [(10381, 1), (10405, 1), (10405, 4), (10405, 4), (10405, 6)];

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00250_WatchWhatYouEat.class.getSimpleName()`: the quest that must be completed.
const WATCH_WHAT_YOU_EAT: &str = "Q00250_WatchWhatYouEat";

/// Java `Q00287_FiguringItOut`.
pub struct Q00287FiguringItOut;

impl Script for Q00287FiguringItOut {
    fn name(&self) -> &'static str {
        "Q00287_FiguringItOut"
    }

    fn path(&self) -> &'static str {
        "quests/Q00287_FiguringItOut"
    }

    fn id(&self) -> i32 {
        287
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LAKI]);
        s.add_talk_id(&[LAKI]);
        s.add_kill_id(&[22768, 22769, 22770, 22771, 22772, 22773, 22774]);
        s.register_quest_items(&[VIAL_OF_TANTA_BLOOD]);
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
        let mut htmltext = Some(event.to_owned());
        match event {
            "32742-03.htm" => qs.start_quest(ctx),
            "Icarus" => {
                if ctx.get_quest_items_count(player, VIAL_OF_TANTA_BLOOD) >= 500 {
                    if let Some((item, count)) = ctx.get_random_entry(&ICARUS) {
                        ctx.give_items(player, item, count);
                    }
                    ctx.take_items(player, VIAL_OF_TANTA_BLOOD, 500);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
                    htmltext = Some("32742-06.html".to_owned());
                } else {
                    htmltext = Some("32742-07.html".to_owned());
                }
            }
            "Moirai" => {
                if ctx.get_quest_items_count(player, VIAL_OF_TANTA_BLOOD) >= 100 {
                    if let Some((item, count)) = ctx.get_random_entry(&MOIRAI) {
                        ctx.give_items(player, item, count);
                    }
                    ctx.take_items(player, VIAL_OF_TANTA_BLOOD, 100);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
                    htmltext = Some("32742-08.html".to_owned());
                } else {
                    htmltext = Some("32742-09.html".to_owned());
                }
            }
            "32742-11.html" => {
                if !ctx.has_quest_items(player, VIAL_OF_TANTA_BLOOD) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("32742-12.html".to_owned());
                }
            }
            "32742-13.html" => qs.exit_quest(ctx, true, true),
            "32742-02.htm" | "32742-10.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): Java only checks that some party member is at cond 1 (`partyMember`
        // is never used again); the vial goes to the killer.
        if ctx.get_random_party_member_cond(killer, 1).is_none() {
            return;
        }
        if ctx.get_random(1000) < vial_chance(npc.id) {
            ctx.give_items(killer, VIAL_OF_TANTA_BLOOD, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let prev_completed = ctx
            .player_quest_state(player, WATCH_WHAT_YOU_EAT)
            .is_some_and(|prev| prev.is_completed());
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL && prev_completed {
                    "32742-01.htm"
                } else {
                    "32742-14.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.get_quest_items_count(player, VIAL_OF_TANTA_BLOOD) < 100 {
                    "32742-04.html"
                } else {
                    "32742-05.html"
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00287_FiguringItOut";

    /// The stand-in of Q00250: an NPC of its own, and the quest completed when talked to.
    const WATCH_NPC: i32 = 30001;

    const NPCS: &str = r#"
        <npc id="32742" type="Folk" level="85" name="Laki"/>
        <npc id="30001" type="Folk" level="70" name="Watch Stand-in"/>
        <npc id="22768" type="Monster" level="80" name="Tanta Lizardman Scout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15499" type="EtcItem" name="Vial of Tanta Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15776" type="EtcItem" name="Moirai A"/>
        <item id="15779" type="EtcItem" name="Moirai A2"/>
        <item id="15782" type="EtcItem" name="Moirai A3"/>
        <item id="15785" type="EtcItem" name="Moirai A4"/>
        <item id="15788" type="EtcItem" name="Moirai A5"/>
        <item id="15812" type="EtcItem" name="Moirai A6"/>
        <item id="15813" type="EtcItem" name="Moirai A7"/>
        <item id="15814" type="EtcItem" name="Moirai A8"/>
        <item id="15772" type="EtcItem" name="Moirai A9"/>
        <item id="15773" type="EtcItem" name="Moirai A10"/>
        <item id="15774" type="EtcItem" name="Moirai A11"/>
        <item id="15646" type="EtcItem" name="Moirai B"><set name="is_stackable" val="true"/></item>
        <item id="15649" type="EtcItem" name="Moirai B2"><set name="is_stackable" val="true"/></item>
        <item id="15652" type="EtcItem" name="Moirai B3"><set name="is_stackable" val="true"/></item>
        <item id="15655" type="EtcItem" name="Moirai B4"><set name="is_stackable" val="true"/></item>
        <item id="15658" type="EtcItem" name="Moirai B5"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00250_WatchWhatYouEat` as far as this quest needs it: its NPC completes it.
    struct WatchWhatYouEat;

    impl Script for WatchWhatYouEat {
        fn name(&self) -> &'static str {
            WATCH_WHAT_YOU_EAT
        }
        fn path(&self) -> &'static str {
            "quests/Q00250_WatchWhatYouEat"
        }
        fn id(&self) -> i32 {
            250
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[WATCH_NPC]);
            s.add_talk_id(&[WATCH_NPC]);
        }
        fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
            if let Some(qs) = ctx.get_quest_state(player, true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
            None
        }
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(WatchWhatYouEat);
        registry.add(Q00287FiguringItOut);
        let pages = Pages::of(&Q00287FiguringItOut)
            .page("32742-01.htm", &["32742-02.htm"])
            .page("32742-02.htm", &["32742-03.htm"])
            .plain(&[
                "32742-03.htm",
                "32742-04.html",
                "32742-07.html",
                "32742-09.html",
                "32742-12.html",
                "32742-14.htm",
            ])
            .page("32742-05.html", &["Icarus", "Moirai"])
            .page("32742-08.html", &["32742-10.html", "32742-11.html"])
            .page("32742-11.html", &["32742-10.html", "32742-13.html"])
            .plain(&["32742-10.html", "32742-13.html"])
            .npcs(&[LAKI, WATCH_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let laki = t.spawn(LAKI, 20);
        let watch = t.spawn(WATCH_NPC, 30);
        (t, laki, watch)
    }

    /// Kills tanta scouts until `vials` vials are in the inventory.
    fn hunt(t: &mut ScriptTest, vials: i64) {
        for _ in 0..3000 {
            if t.count(1, VIAL_OF_TANTA_BLOOD) >= vials {
                return;
            }
            let scout = t.spawn(22768, 60);
            t.kill(1, scout);
        }
        panic!("no {vials} vials after 3000 kills");
    }

    #[test]
    fn without_the_watch_quest_laki_says_no() {
        let (mut t, laki, _watch) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, laki).contains("32742-14.htm"));
        t.enter_as(2, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, laki).contains("32742-14.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn vials_for_moirai_then_the_quest_ends_on_the_last_page() {
        let (mut t, laki, watch) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        // Complete Q00250 (the stand-in), then Laki's quest is open.
        t.quest_page(1, watch);
        // The stand-in's own rows (started, then completed) are not what this test checks.
        t.take_writes();
        assert!(t.quest_page(1, laki).contains("32742-01.htm"));
        assert!(t.event(1, NAME, "32742-02.htm").contains("32742-02.htm"));
        assert!(t.event(1, NAME, "32742-03.htm").contains("32742-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Vials drop from the tanta lizardmen; no condition changes.
        // 101 vials: one is left after Moirai, so the last page does not end the quest.
        hunt(&mut t, 101);
        assert!(t.take_writes().is_empty());
        let vials = t.count(1, VIAL_OF_TANTA_BLOOD);

        // Moirai: one random item, 100 vials taken, the sound; the page stays open.
        assert!(t.quest_page(1, laki).contains("32742-05.html"));
        assert!(t.event(1, NAME, "Moirai").contains("32742-08.html"));
        assert_eq!(t.count(1, VIAL_OF_TANTA_BLOOD), vials - 100);
        let moirai: i64 = MOIRAI.iter().map(|&(item, _)| t.count(1, item)).sum();
        assert!(moirai >= 1);
        assert!(t.event(1, NAME, "32742-11.html").contains("32742-11.html"));
        assert!(t.take_writes().is_empty());

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32742-13.html").contains("32742-13.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
