//! Quest 347: Go Get the Calculator (Brunon in Gludin, the gemstone beasts, Silvera and Balanki).
// Java: dist/game/data/scripts/quests/Q00347_GoGetTheCalculator/Q00347_GoGetTheCalculator.java
//
// Pattern: a delivery chain with a party kill step (`getRandomPartyMemberState(killer, 4, 3, npc)`
// at cond 4 gives the gemstones); the conditions are set by talks and by the kill, the reward is a
// choice on the last page (item or adena).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BRUNON: i32 = 30526;
const SILVERA: i32 = 30527;
const SPIRON: i32 = 30532;
const BALANKI: i32 = 30533;

// Items
const STOLEN_CALCULATOR: i32 = 4285;
const GEMSTONE: i32 = 4286;

// Monster
const GEMSTONE_BEAST: i32 = 20540;

// Reward
const CALCULATOR: i32 = 4393;
const ADENA: i64 = 1500;

// Misc
const MIN_LEVEL: i32 = 12;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `Q00347_GoGetTheCalculator`.
pub struct Q00347GoGetTheCalculator;

impl Script for Q00347GoGetTheCalculator {
    fn name(&self) -> &'static str {
        "Q00347_GoGetTheCalculator"
    }

    fn path(&self) -> &'static str {
        "quests/Q00347_GoGetTheCalculator"
    }

    fn id(&self) -> i32 {
        347
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BRUNON]);
        s.add_talk_id(&[BRUNON, SILVERA, SPIRON, BALANKI]);
        s.add_kill_id(&[GEMSTONE_BEAST]);
        s.register_quest_items(&[STOLEN_CALCULATOR, GEMSTONE]);
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
            "30526-03.htm" | "30526-04.htm" | "30526-05.htm" | "30526-06.htm" | "30526-07.htm"
            | "30532-03.html" | "30532-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "30526-08.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30526-10.html" => {
                if qs.is_cond(ctx, 6) {
                    ctx.take_items(player, STOLEN_CALCULATOR, -1);
                    ctx.reward_items(player, CALCULATOR, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30526-09.html".to_owned());
                }
            }
            "30526-11.html" => {
                if qs.is_cond(ctx, 6) {
                    ctx.take_items(player, STOLEN_CALCULATOR, -1);
                    ctx.give_adena(player, ADENA, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30532-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30533-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.adena_of(player) > 100 {
                    ctx.take_items(player, ADENA_ID, 100);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == BRUNON {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30526-01.htm"
                    } else {
                        "30526-02.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                BRUNON => {
                    if ctx.has_quest_items(talker, CALCULATOR) {
                        qs.set_cond(ctx, 6, false);
                    }

                    match qs.get_cond(ctx) {
                        1 | 2 => htmltext = "30526-13.html".to_owned(),
                        3 | 4 => htmltext = "30526-14.html".to_owned(),
                        5 => htmltext = "30526-15.html".to_owned(),
                        6 => htmltext = "30526-09.html".to_owned(),
                        _ => {}
                    }
                }
                SPIRON => {
                    htmltext = if qs.is_cond(ctx, 1) {
                        "30532-01.html"
                    } else {
                        "30532-05.html"
                    }
                    .to_owned();
                }
                BALANKI => {
                    htmltext = if qs.is_cond(ctx, 2) {
                        "30533-01.html"
                    } else if qs.get_cond(ctx) > 2 {
                        "30533-04.html"
                    } else {
                        "30533-03.html"
                    }
                    .to_owned();
                }
                SILVERA => match qs.get_cond(ctx) {
                    1 | 2 => htmltext = "30527-01.html".to_owned(),
                    3 => {
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30527-02.html".to_owned();
                    }
                    4 => htmltext = "30527-04.html".to_owned(),
                    5 => {
                        ctx.take_items(talker, GEMSTONE, -1);
                        ctx.give_items(talker, STOLEN_CALCULATOR, 1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30527-03.html".to_owned();
                    }
                    6 => htmltext = "30527-05.html".to_owned(),
                    _ => {}
                },
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, 4, 3, Some(npc)) else {
            return;
        };
        if ctx.give_item_randomly(qs.player(), Some(npc), GEMSTONE, 1, 10, 0, 0.4, true) {
            qs.set_cond(ctx, 5, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00347_GoGetTheCalculator";

    const NPCS: &str = r#"
        <npc id="30526" type="Folk" level="30" name="Brunon"/>
        <npc id="30527" type="Folk" level="30" name="Silvera"/>
        <npc id="30532" type="Folk" level="30" name="Spiron"/>
        <npc id="30533" type="Folk" level="30" name="Balanki"/>
        <npc id="20540" type="Monster" level="20" name="Gemstone Beast"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4285" type="EtcItem" name="Stolen Calculator"><set name="is_questitem" val="true"/></item>
        <item id="4286" type="EtcItem" name="Gemstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4393" type="Armor" name="Calculator"><set name="bodypart" val="none"/></item>"#;

    struct Npcs {
        brunon: Oid,
        spiron: Oid,
        balanki: Oid,
        silvera: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00347GoGetTheCalculator);
        let pages = Pages::of(&Q00347GoGetTheCalculator)
            .page("30526-01.htm", &["30526-03.htm"])
            .page("30526-03.htm", &["30526-04.htm", "30526-05.htm", "30526-06.htm"])
            .plain(&["30526-02.html", "30526-04.htm", "30526-05.htm"])
            .page("30526-06.htm", &["30526-07.htm"])
            .page("30526-07.htm", &["30526-08.htm"])
            .plain(&["30526-08.htm", "30526-10.html", "30526-11.html", "30526-13.html"])
            .plain(&["30526-14.html", "30526-15.html"])
            .page("30526-09.html", &["30526-10.html", "30526-11.html"])
            .page(
                "30532-01.html",
                &["30532-02.html", "30532-03.html", "30532-04.html"],
            )
            .plain(&["30532-02.html", "30532-03.html", "30532-04.html", "30532-05.html"])
            .page("30533-01.html", &["30533-02.html"])
            .plain(&["30533-02.html", "30533-03.html", "30533-04.html"])
            .plain(&[
                "30527-01.html",
                "30527-02.html",
                "30527-03.html",
                "30527-04.html",
                "30527-05.html",
            ])
            .npcs(&[BRUNON, SILVERA, SPIRON, BALANKI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            brunon: t.spawn(BRUNON, 20),
            spiron: t.spawn(SPIRON, 40),
            balanki: t.spawn(BALANKI, 60),
            silvera: t.spawn(SILVERA, 80),
        };
        (t, npcs)
    }

    /// Brunon's accept: the quest is started (cond 1).
    fn accept(t: &mut ScriptTest, brunon: Oid) {
        assert!(t.quest_page(1, brunon).contains("30526-01.htm"));
        assert!(t.event(1, NAME, "30526-03.htm").contains("30526-03.htm"));
        assert!(t.event(1, NAME, "30526-06.htm").contains("30526-06.htm"));
        assert!(t.event(1, NAME, "30526-07.htm").contains("30526-07.htm"));
        t.take_writes();
        assert!(t.event(1, NAME, "30526-08.htm").contains("30526-08.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_character_below_level_twelve_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 11, vec![]);
        assert!(t.quest_page(1, npcs.brunon).contains("30526-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn brunon_starts_the_quest_at_level_twelve() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![]);
        accept(&mut t, npcs.brunon);
        assert!(t.quest_page(1, npcs.brunon).contains("30526-13.html"));
    }

    #[test]
    fn balanki_needs_more_than_a_hundred_adena() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![item(100, 57, 100)]);
        accept(&mut t, npcs.brunon);
        assert!(t.quest_page(1, npcs.spiron).contains("30532-01.html"));
        assert!(t.event(1, NAME, "30532-02.html").contains("30532-02.html"));
        t.take_writes();
        // Balanki asks for the adena; with exactly 100 nothing happens.
        assert!(t.quest_page(1, npcs.balanki).contains("30533-01.html"));
        assert_eq!(t.event(1, NAME, "30533-02.html"), "");
        assert_eq!(t.count(1, 57), 100);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_steps_to_the_gemstones_and_the_calculator() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![item(100, 57, 200)]);
        accept(&mut t, npcs.brunon);

        // Spiron: cond 2 with the middle sound.
        assert!(t.quest_page(1, npcs.spiron).contains("30532-01.html"));
        assert!(t.event(1, NAME, "30532-02.html").contains("30532-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Balanki takes 100 adena: cond 3.
        assert!(t.quest_page(1, npcs.balanki).contains("30533-01.html"));
        assert!(t.event(1, NAME, "30533-02.html").contains("30533-02.html"));
        assert_eq!(t.count(1, 57), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Silvera: cond 4 (the gemstone hunt).
        assert!(t.quest_page(1, npcs.silvera).contains("30527-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.silvera).contains("30527-04.html"));
        assert!(t.quest_page(1, npcs.balanki).contains("30533-04.html"));

        // The gemstone beasts: the first gemstone sets cond 5.
        for _ in 0..200 {
            if t.count(1, GEMSTONE) > 0 {
                break;
            }
            let beast = t.spawn(GEMSTONE_BEAST, 60);
            t.kill(1, beast);
        }
        assert!(t.count(1, GEMSTONE) > 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "5")));

        // Silvera takes the gemstones and gives the stolen calculator: cond 6.
        assert!(t.quest_page(1, npcs.silvera).contains("30527-03.html"));
        assert_eq!(t.count(1, GEMSTONE), 0);
        assert_eq!(t.count(1, STOLEN_CALCULATOR), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.silvera).contains("30527-05.html"));
    }

    #[test]
    fn brunon_gives_the_calculator_or_the_adena_at_cond_six() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![item(100, STOLEN_CALCULATOR, 1)]);
        accept(&mut t, npcs.brunon);
        // The stolen calculator is in the bag, but no calculator yet: cond stays 1.
        assert!(t.quest_page(1, npcs.brunon).contains("30526-13.html"));
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 6, false);
        });
        t.take_writes();

        // Brunon: the stolen calculator is still there; the reward page is 10 (the calculator).
        assert!(t.quest_page(1, npcs.brunon).contains("30526-09.html"));
        assert!(t.event(1, NAME, "30526-10.html").contains("30526-10.html"));
        assert_eq!(t.count(1, CALCULATOR), 1);
        assert_eq!(t.count(1, STOLEN_CALCULATOR), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, npcs.brunon).contains("30526-01.htm"));
    }

    #[test]
    fn brunon_gives_the_adena_instead_of_the_calculator() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![]);
        accept(&mut t, npcs.brunon);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 6, false);
            ctx.give_items(Oid(1), STOLEN_CALCULATOR, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.brunon).contains("30526-09.html"));
        assert!(t.event(1, NAME, "30526-11.html").contains("30526-11.html"));
        assert_eq!(t.count(1, 57), ADENA);
        assert_eq!(t.count(1, STOLEN_CALCULATOR), 0);
        assert_eq!(t.count(1, CALCULATOR), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_calculator_in_the_bag_moves_brunon_to_cond_six() {
        let (mut t, npcs) = setup();
        t.enter(1, 12, vec![]);
        accept(&mut t, npcs.brunon);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), CALCULATOR, 1);
        });
        // `setCond(6)` with no sound, then the cond 6 page. Going from cond 1 to 6 skips steps,
        // so Java's `setCond(cond, old)` also writes the completed state flags.
        assert!(t.quest_page(1, npcs.brunon).contains("30526-09.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "6"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483615"),
            ]
        );
    }
}
