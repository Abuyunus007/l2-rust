//! Quest 310: Only What Remains (Kintaijin in the Rune town, the Mutated Creatures).
// Java: dist/game/data/scripts/quests/Q00310_OnlyWhatRemains/Q00310_OnlyWhatRemains.java
//
// Pattern: a party kill, a random member at a state (`docs/design/scripts.md`, "Quest patterns"):
// `get_random_party_member_cond(player, 1)` draws the member and the bead goes to that member.
// The start needs another quest completed (Q00240) through its state: a read only
// `player_quest_state`. The NPC trades 500 beads for two items and the quest goes on until the
// player quits.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KINTAIJIN: i32 = 32640;

// Items
const GROW_ACCELERATOR: i32 = 14832;
const MULTI_COLORED_JEWEL: i32 = 14835;
const DIRTY_BEAD: i32 = 14880;

// Monsters (id, drop chance per mille)
const MOBS: [(i32, i32); 17] = [
    (22617, 646),
    (22618, 646),
    (22619, 646),
    (22620, 666),
    (22621, 630),
    (22622, 940),
    (22623, 622),
    (22624, 630),
    (22625, 678),
    (22626, 940),
    (22627, 646),
    (22628, 646),
    (22629, 646),
    (22630, 638),
    (22631, 880),
    (22632, 722),
    (22633, 638),
];

// Misc
const MIN_LEVEL: i32 = 81;
const REQUIRED_BEADS: i64 = 500;
/// Java `Q00240_ImTheOnlyOneYouCanTrust.class.getSimpleName()`: the quest that must be done.
const IM_THE_ONLY_ONE_YOU_CAN_TRUST: &str = "Q00240_ImTheOnlyOneYouCanTrust";

/// Java `Q00310_OnlyWhatRemains`.
pub struct Q00310OnlyWhatRemains;

impl Script for Q00310OnlyWhatRemains {
    fn name(&self) -> &'static str {
        "Q00310_OnlyWhatRemains"
    }

    fn path(&self) -> &'static str {
        "quests/Q00310_OnlyWhatRemains"
    }

    fn id(&self) -> i32 {
        310
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KINTAIJIN]);
        s.add_talk_id(&[KINTAIJIN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[DIRTY_BEAD]);
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
            "32640-04.htm" => {
                qs.start_quest(ctx);
            }
            "32640-quit.html" => {
                qs.exit_quest(ctx, true, true);
            }
            "32640-02.htm" | "32640-03.htm" | "32640-05.html" | "32640-06.html" | "32640-07.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 1) else {
            return;
        };
        // Only the registered monsters reach here; any other id never drops.
        let chance = MOBS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if ctx.get_random(1000) < chance {
            ctx.give_items(party_member, DIRTY_BEAD, 1);
            ctx.play_sound(party_member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            let prev_completed = ctx
                .player_quest_state(player, IM_THE_ONLY_ONE_YOU_CAN_TRUST)
                .is_some_and(|prev| prev.is_completed());
            htmltext = if ctx.player_level(player) >= MIN_LEVEL && prev_completed {
                "32640-01.htm"
            } else {
                "32640-00.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if !ctx.has_quest_items(player, DIRTY_BEAD) {
                htmltext = "32640-08.html".to_owned();
            } else if ctx.get_quest_items_count(player, DIRTY_BEAD) < REQUIRED_BEADS {
                htmltext = "32640-09.html".to_owned();
            } else {
                ctx.take_items(player, DIRTY_BEAD, REQUIRED_BEADS);
                ctx.give_items(player, GROW_ACCELERATOR, 1);
                ctx.give_items(player, MULTI_COLORED_JEWEL, 1);
                htmltext = "32640-10.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00310_OnlyWhatRemains";

    const NPCS: &str = r#"
        <npc id="32640" type="Folk" level="1" name="Kintaijin"/>
        <npc id="22617" type="Monster" level="80" name="Mutated Creature"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14832" type="EtcItem" name="Grow Accelerator"><set name="is_stackable" val="true"/></item>
        <item id="14835" type="EtcItem" name="Multi-Colored Jewel"><set name="is_stackable" val="true"/></item>
        <item id="14880" type="EtcItem" name="Dirty Bead"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00240_ImTheOnlyOneYouCanTrust`: a script of that name, the quest is made completed
    /// through its own exit path (`exitQuest(false)`) for the player.
    struct ImTheOnlyOneYouCanTrust;

    impl Script for ImTheOnlyOneYouCanTrust {
        fn name(&self) -> &'static str {
            "Q00240_ImTheOnlyOneYouCanTrust"
        }
        fn path(&self) -> &'static str {
            "quests/Q00240_ImTheOnlyOneYouCanTrust"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn complete_prerequisite(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(IM_THE_ONLY_ONE_YOU_CAN_TRUST, |ctx| {
            if let Some(qs) = ctx.get_quest_state(ObjectId(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ImTheOnlyOneYouCanTrust);
        registry.add(Q00310OnlyWhatRemains);
        let pages = Pages::of(&Q00310OnlyWhatRemains)
            .page("32640-01.htm", &["32640-02.htm"])
            .page("32640-02.htm", &["32640-03.htm"])
            .page("32640-03.htm", &["32640-04.htm"])
            .page("32640-04.htm", &["32640-06.html", "32640-05.html"])
            .page("32640-06.html", &["32640-07.html"])
            .page(
                "32640-08.html",
                &["32640-06.html", "32640-05.html", "32640-quit.html"],
            )
            .page(
                "32640-10.html",
                &["32640-06.html", "32640-05.html", "32640-quit.html"],
            )
            .plain(&[
                "32640-00.htm",
                "32640-05.html",
                "32640-07.html",
                "32640-09.html",
                "32640-quit.html",
            ])
            .npcs(&[KINTAIJIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kintaijin = t.spawn(KINTAIJIN, 20);
        (t, kintaijin)
    }

    /// Kills Mutated Creatures until a bead drops; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let creature = t.spawn(22617, 60);
            t.kill(1, creature);
            if t.count(1, DIRTY_BEAD) >= 1 {
                return kills;
            }
        }
        panic!("no bead after 400 kills");
    }

    #[test]
    fn the_prerequisite_and_the_level_are_checked() {
        let (mut t, kintaijin) = setup();
        t.enter_as(1, 80, race::HUMAN, player_class::FIGHTER, vec![]);
        complete_prerequisite(&mut t, 1);
        assert!(t.quest_page(1, kintaijin).contains("32640-00.htm"));
        t.enter_as(2, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, kintaijin).contains("32640-00.htm"));
        t.take_writes();
    }

    #[test]
    fn the_start_then_a_bead_drops() {
        let (mut t, kintaijin) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        complete_prerequisite(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, kintaijin).contains("32640-01.htm"));
        assert!(t.event(1, NAME, "32640-02.htm").contains("32640-02.htm"));
        assert!(t.event(1, NAME, "32640-03.htm").contains("32640-03.htm"));
        assert!(t.event(1, NAME, "32640-04.htm").contains("32640-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No bead yet.
        assert!(t.quest_page(1, kintaijin).contains("32640-08.html"));

        // The drawn member (the killer) gets a bead; with fewer than 500 the NPC says so.
        let kills = hunt(&mut t);
        assert!(kills >= 1, "{kills}");
        assert!(t.quest_page(1, kintaijin).contains("32640-09.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_hundred_beads_give_the_two_items() {
        let (mut t, kintaijin) = setup();
        t.enter_as(
            1,
            81,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 14880, 500)],
        );
        complete_prerequisite(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, kintaijin).contains("32640-01.htm"));
        assert!(t.event(1, NAME, "32640-02.htm").contains("32640-02.htm"));
        assert!(t.event(1, NAME, "32640-03.htm").contains("32640-03.htm"));
        assert!(t.event(1, NAME, "32640-04.htm").contains("32640-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Talk: the 500 beads become the accelerator and the jewel, the quest goes on.
        assert!(t.quest_page(1, kintaijin).contains("32640-10.html"));
        assert_eq!(t.count(1, DIRTY_BEAD), 0);
        assert_eq!(t.count(1, GROW_ACCELERATOR), 1);
        assert_eq!(t.count(1, MULTI_COLORED_JEWEL), 1);
        assert!(t.take_writes().is_empty());

        // Quit: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 32640-quit.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32640-quit.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
