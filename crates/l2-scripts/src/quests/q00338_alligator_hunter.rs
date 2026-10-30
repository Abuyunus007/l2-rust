//! Quest 338: Alligator Hunter (Enverun in Heine, alligators on Alligator Island).
// Java: dist/game/data/scripts/quests/Q00338_AlligatorHunter/Q00338_AlligatorHunter.java
//
// Pattern: a kill-drop quest with a second pelt chance; the trader pays per pelt and the quest
// stays open (repeatable) until the player quits.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ENVERUN: i32 = 30892;

// Monster
const ALLIGATOR: i32 = 20135;

// Item
const ALLIGATOR_LEATHER: i32 = 4337;

// Misc
const MIN_LEVEL: i32 = 40;
const SECOND_CHANCE: i32 = 19;

/// Java `Q00338_AlligatorHunter`.
pub struct Q00338AlligatorHunter;

impl Script for Q00338AlligatorHunter {
    fn name(&self) -> &'static str {
        "Q00338_AlligatorHunter"
    }

    fn path(&self) -> &'static str {
        "quests/Q00338_AlligatorHunter"
    }

    fn id(&self) -> i32 {
        338
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ENVERUN]);
        s.add_talk_id(&[ENVERUN]);
        s.add_kill_id(&[ALLIGATOR]);
        s.register_quest_items(&[ALLIGATOR_LEATHER]);
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
            "30892-03.htm" => {
                qs.start_quest(ctx);
            }
            "30892-06.html" => {
                if !ctx.has_quest_items(player, ALLIGATOR_LEATHER) {
                    return Some("30892-05.html".to_owned());
                }

                let count = ctx.get_quest_items_count(player, ALLIGATOR_LEATHER);
                let mut amount = if count >= 10 { 3430 } else { 0 };
                amount += 60 * count;
                ctx.give_adena(player, amount, true);
                ctx.take_items(player, ALLIGATOR_LEATHER, -1);
            }
            "30892-10.html" => {
                qs.exit_quest(ctx, true, true);
            }
            "30892-07.html" | "30892-08.html" | "30892-09.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() {
            ctx.give_items(killer, ALLIGATOR_LEATHER, 1);
            if ctx.get_random(100) < SECOND_CHANCE {
                ctx.give_items(killer, ALLIGATOR_LEATHER, 1);
            }
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30892-02.htm"
                } else {
                    "30892-01.htm"
                }
                .to_owned();
            }
            STARTED => htmltext = "30892-04.html".to_owned(),
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00338_AlligatorHunter";

    const NPCS: &str = r#"
        <npc id="30892" type="Folk" level="40" name="Enverun"/>
        <npc id="20135" type="Monster" level="40" name="Alligator"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4337" type="EtcItem" name="Alligator Leather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00338AlligatorHunter);
        let pages = Pages::of(&Q00338AlligatorHunter)
            .page("30892-02.htm", &["30892-03.htm"])
            .plain(&["30892-01.htm", "30892-03.htm", "30892-05.html", "30892-10.html"])
            .page("30892-04.html", &["30892-06.html", "30892-07.html"])
            .page("30892-06.html", &["30892-08.html", "30892-09.html"])
            .page("30892-07.html", &[])
            .page("30892-08.html", &[])
            .page("30892-09.html", &["30892-08.html", "30892-10.html"])
            .npcs(&[ENVERUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let enverun = t.spawn(ENVERUN, 20);
        (t, enverun)
    }

    /// Kills alligators until the player has `pelts` pelts; returns the number of kills.
    fn hunt(t: &mut ScriptTest, pelts: i64) -> usize {
        for kills in 1..400 {
            let alligator = t.spawn(ALLIGATOR, 60);
            t.kill(1, alligator);
            if t.count(1, ALLIGATOR_LEATHER) >= pelts {
                return kills;
            }
        }
        panic!("no {pelts} pelts after 400 kills");
    }

    #[test]
    fn a_character_below_level_forty_is_sent_away() {
        let (mut t, enverun) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, enverun).contains("30892-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hunt_starts_at_level_forty() {
        let (mut t, enverun) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, enverun).contains("30892-02.htm"));
        assert!(t.event(1, NAME, "30892-03.htm").contains("30892-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, enverun).contains("30892-04.html"));
    }

    #[test]
    fn pelts_are_sold_for_sixty_adena_each_and_the_quest_goes_on() {
        let (mut t, enverun) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, enverun);
        t.event(1, NAME, "30892-03.htm");
        t.take_writes();

        // Nothing to sell: the trader asks again.
        assert!(t.quest_page(1, enverun).contains("30892-04.html"));
        assert!(t.event(1, NAME, "30892-06.html").contains("30892-05.html"));
        assert_eq!(t.count(1, 57), 0);

        // Five pelts from the hunt; the sale gives 60 each and takes them.
        hunt(&mut t, 5);
        let pelts = t.count(1, ALLIGATOR_LEATHER);
        t.take();
        assert!(t.quest_page(1, enverun).contains("30892-04.html"));
        assert!(t.event(1, NAME, "30892-06.html").contains("30892-06.html"));
        assert_eq!(t.count(1, 57), 60 * pelts);
        assert_eq!(t.count(1, ALLIGATOR_LEATHER), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_pelts_or_more_also_pay_the_bonus() {
        let (mut t, enverun) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, enverun);
        t.event(1, NAME, "30892-03.htm");
        hunt(&mut t, 10);
        let pelts = t.count(1, ALLIGATOR_LEATHER);
        assert!(pelts >= 10);
        t.quest_page(1, enverun);
        t.event(1, NAME, "30892-06.html");
        assert_eq!(t.count(1, 57), 3430 + 60 * pelts);
    }

    #[test]
    fn quitting_the_hunt_ends_the_repeatable_quest() {
        let (mut t, enverun) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, enverun);
        t.event(1, NAME, "30892-03.htm");
        hunt(&mut t, 1);
        t.take_writes();
        // Sell the pelt, then "Quit hunting" and "Quit" (the links of the pages in between).
        assert!(t.quest_page(1, enverun).contains("30892-04.html"));
        assert!(t.event(1, NAME, "30892-06.html").contains("30892-06.html"));
        assert!(t.event(1, NAME, "30892-09.html").contains("30892-09.html"));
        t.bypass(1, &format!("Script {NAME} 30892-10.html"));
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        // Repeatable: the trader offers the quest again.
        assert!(t.quest_page(1, enverun).contains("30892-02.htm"));
    }

    #[test]
    fn each_kill_gives_one_or_two_pelts() {
        let (mut t, enverun) = setup();
        t.enter(1, 40, vec![]);
        // Without the quest a kill gives nothing (`getQuestState(player, false)` is null).
        let alligator = t.spawn(ALLIGATOR, 60);
        t.kill(1, alligator);
        assert_eq!(t.count(1, ALLIGATOR_LEATHER), 0);

        t.quest_page(1, enverun);
        t.event(1, NAME, "30892-03.htm");
        hunt(&mut t, 30);
        // One pelt per kill, a second one with 19 % chance.
        let kills = t.count(1, ALLIGATOR_LEATHER);
        assert!((30..=60).contains(&kills), "{kills}");
    }
}
