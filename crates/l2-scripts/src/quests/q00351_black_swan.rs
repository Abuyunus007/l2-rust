//! Quest 351: Black Swan (Captain Gosta and Iason Heine in Innadril, Roman the blacksmith).
// Java: dist/game/data/scripts/quests/Q00351_BlackSwan/Q00351_BlackSwan.java
//
// Pattern: a party kill drop (a random member in the started state gets the fangs and the barrels),
// the NPC pays the fangs (a bonus from ten of them), the barrels turn into bills that Roman
// exchanges. Reads the state of Q00345 (`player.getQuestState`), read only.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROMAN: i32 = 30897;
const GOSTA: i32 = 30916;
const IASON_HEINE: i32 = 30969;

// Items
const ORDER_OF_GOSTA: i32 = 4296;
const LIZARD_FANG: i32 = 4297;
const BARREL_OF_LEAGUE: i32 = 4298;
const BILL_OF_IASON_HEINE: i32 = 4407;

// Misc
const MIN_LEVEL: i32 = 32;

// Monsters
const TASABA_LIZARDMAN1: i32 = 20784;
const TASABA_LIZARDMAN_SHAMAN1: i32 = 20785;
const TASABA_LIZARDMAN2: i32 = 21639;
const TASABA_LIZARDMAN_SHAMAN2: i32 = 21640;

/// Java `MONSTER_DROP_CHANCES`.
const MONSTER_DROP_CHANCES: [(i32, i32); 4] = [
    (TASABA_LIZARDMAN1, 4),
    (TASABA_LIZARDMAN_SHAMAN1, 3),
    (TASABA_LIZARDMAN2, 4),
    (TASABA_LIZARDMAN_SHAMAN2, 3),
];

/// Java `Q00345_MethodToRaiseTheDead.class.getSimpleName()`.
const METHOD_TO_RAISE_THE_DEAD: &str = "Q00345_MethodToRaiseTheDead";

/// Java `Q00351_BlackSwan`.
pub struct Q00351BlackSwan;

impl Script for Q00351BlackSwan {
    fn name(&self) -> &'static str {
        "Q00351_BlackSwan"
    }

    fn path(&self) -> &'static str {
        "quests/Q00351_BlackSwan"
    }

    fn id(&self) -> i32 {
        351
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GOSTA, ROMAN]);
        s.add_talk_id(&[GOSTA, IASON_HEINE, ROMAN]);
        let ids: Vec<i32> = MONSTER_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[ORDER_OF_GOSTA, LIZARD_FANG, BARREL_OF_LEAGUE]);
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
            "30916-02.htm" | "30969-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30916-03.htm" => {
                ctx.give_items(player, ORDER_OF_GOSTA, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30969-02.html" => {
                let lizard_fang_count = ctx.get_quest_items_count(player, LIZARD_FANG);
                if lizard_fang_count == 0 {
                    htmltext = Some(event.to_owned());
                } else {
                    let adena_bonus = if lizard_fang_count >= 10 { 3880 } else { 0 };
                    ctx.give_adena(player, adena_bonus + 20 * lizard_fang_count, true);
                    ctx.take_items(player, LIZARD_FANG, -1);
                    htmltext = Some("30969-04.html".to_owned());
                }
            }
            "30969-05.html" => {
                let barrel_of_league_count = ctx.get_quest_items_count(player, BARREL_OF_LEAGUE);
                if barrel_of_league_count == 0 {
                    htmltext = Some(event.to_owned());
                } else {
                    ctx.give_items(player, BILL_OF_IASON_HEINE, barrel_of_league_count);
                    ctx.give_adena(player, 3880, true);
                    ctx.take_items(player, BARREL_OF_LEAGUE, -1);
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some("30969-06.html".to_owned());
                }
            }
            "30969-07.html" => {
                htmltext = Some(
                    if !ctx.has_all_quest_items(player, &[BARREL_OF_LEAGUE, LIZARD_FANG]) {
                        event
                    } else {
                        "30969-08.html"
                    }
                    .to_owned(),
                );
            }
            "30969-09.html" => {
                htmltext = Some(event.to_owned());
                qs.exit_quest(ctx, true, true);
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        // The drops go to the member the quest state was drawn for (Java `qs.getPlayer()`).
        let member = qs.player();
        let random = ctx.get_random(20);
        if random < 10 {
            ctx.give_item_randomly(member, Some(npc), LIZARD_FANG, 1, 1, 0, 1.0, true);
            if ctx.get_random(20) == 0 {
                ctx.give_item_randomly(member, Some(npc), BARREL_OF_LEAGUE, 1, 1, 0, 1.0, false);
            }
        } else if random < 15 {
            ctx.give_item_randomly(member, Some(npc), LIZARD_FANG, 2, 2, 0, 1.0, true);
            if ctx.get_random(20) == 0 {
                ctx.give_item_randomly(member, Some(npc), BARREL_OF_LEAGUE, 1, 1, 0, 1.0, false);
            }
        } else {
            let chance = MONSTER_DROP_CHANCES
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0, |(_, chance)| *chance);
            if ctx.get_random(100) < chance {
                ctx.give_item_randomly(member, Some(npc), BARREL_OF_LEAGUE, 1, 1, 0, 1.0, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let qs2_started = ctx
            .player_quest_state(player, METHOD_TO_RAISE_THE_DEAD)
            .is_some_and(|q| q.is_started());
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GOSTA => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30916-01.htm"
                    } else {
                        "30916-04.html"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    htmltext = "30916-05.html".to_owned();
                }
            }
            IASON_HEINE => {
                if qs.is_started(ctx) {
                    htmltext = "30969-01.html".to_owned();
                }
            }
            ROMAN => {
                if qs.is_started(ctx) || qs2_started {
                    htmltext = if ctx.has_quest_items(player, BILL_OF_IASON_HEINE) {
                        "30897-01.html"
                    } else {
                        "30897-02.html"
                    }
                    .to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, rows};

    const NAME: &str = "Q00351_BlackSwan";

    const NPCS: &str = r#"
        <npc id="30897" type="Folk" level="50" name="Roman"/>
        <npc id="30916" type="Folk" level="50" name="Gosta"/>
        <npc id="30969" type="Folk" level="50" name="Iason Heine"/>
        <npc id="20784" type="Monster" level="30" name="Tasaba Lizardman"/>
        <npc id="20785" type="Monster" level="30" name="Tasaba Lizardman Shaman"/>
        <npc id="21639" type="Monster" level="30" name="Tasaba Lizardman"/>
        <npc id="21640" type="Monster" level="30" name="Tasaba Lizardman Shaman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4296" type="EtcItem" name="Order of Gosta"><set name="is_questitem" val="true"/></item>
        <item id="4297" type="EtcItem" name="Lizard Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4298" type="EtcItem" name="Barrel of League"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4407" type="EtcItem" name="Bill of Iason Heine"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00345_MethodToRaiseTheDead` as far as this quest reads it: a script of that name.
    struct MethodToRaiseTheDead;

    impl Script for MethodToRaiseTheDead {
        fn name(&self) -> &'static str {
            METHOD_TO_RAISE_THE_DEAD
        }
        fn path(&self) -> &'static str {
            "quests/Q00345_MethodToRaiseTheDead"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup(with_345: bool) -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        if with_345 {
            registry.add(MethodToRaiseTheDead);
        }
        registry.add(Q00351BlackSwan);
        let pages = Pages::of(&Q00351BlackSwan)
            .page("30916-01.htm", &["30916-02.htm"])
            .page("30916-02.htm", &["30916-03.htm"])
            .plain(&["30916-03.htm", "30916-04.html", "30916-05.html"])
            .plain(&["30897-01.html", "30897-02.html"])
            .page("30969-01.html", &["30969-02.html", "30969-05.html"])
            .page("30969-02.html", &["30969-03.html"])
            .page(
                "30969-03.html",
                &["30969-02.html", "30969-05.html", "30969-07.html"],
            )
            .page("30969-04.html", &["30969-03.html"])
            .page("30969-05.html", &["30969-03.html"])
            .page("30969-06.html", &["30969-03.html"])
            .page("30969-07.html", &["30969-03.html", "30969-09.html"])
            .page("30969-08.html", &["30969-03.html"])
            .plain(&["30969-09.html"])
            .npcs(&[GOSTA, ROMAN, IASON_HEINE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gosta = t.spawn(GOSTA, 20);
        let roman = t.spawn(ROMAN, 40);
        let iason = t.spawn(IASON_HEINE, 60);
        (t, gosta, roman, iason)
    }

    /// Kills Tasaba lizardmen until the player has `fangs` lizard fangs.
    fn hunt(t: &mut ScriptTest, fangs: i64) {
        for _ in 0..400 {
            if t.count(1, LIZARD_FANG) >= fangs {
                return;
            }
            let lizardman = t.spawn(TASABA_LIZARDMAN1, 60);
            t.kill(1, lizardman);
        }
        panic!("no {fangs} fangs after 400 kills");
    }

    /// Gosta's accept: the order and the quest started (`getQuestState` on a level 32 character).
    fn accept(t: &mut ScriptTest, gosta: Oid) {
        assert!(t.quest_page(1, gosta).contains("30916-01.htm"));
        assert!(t.event(1, NAME, "30916-02.htm").contains("30916-02.htm"));
        assert!(t.event(1, NAME, "30916-03.htm").contains("30916-03.htm"));
    }

    #[test]
    fn a_character_below_level_thirty_two_is_sent_away() {
        let (mut t, gosta, _roman, _iason) = setup(false);
        t.enter(1, 31, vec![]);
        assert!(t.quest_page(1, gosta).contains("30916-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn gosta_hands_out_the_order_and_starts_the_quest() {
        let (mut t, gosta, _roman, _iason) = setup(false);
        t.enter(1, 32, vec![]);
        t.quest_page(1, gosta);
        t.event(1, NAME, "30916-02.htm");
        t.take_writes();
        assert!(t.event(1, NAME, "30916-03.htm").contains("30916-03.htm"));
        assert_eq!(t.count(1, ORDER_OF_GOSTA), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, gosta).contains("30916-05.html"));
    }

    #[test]
    fn the_fangs_are_paid_at_twenty_adena_each_and_a_bonus_from_ten() {
        let (mut t, gosta, _roman, iason) = setup(false);
        t.enter(1, 32, vec![]);
        accept(&mut t, gosta);
        t.take_writes();

        // No fangs yet: Iason asks for them.
        assert!(t.quest_page(1, iason).contains("30969-01.html"));
        assert!(t.event(1, NAME, "30969-02.html").contains("30969-02.html"));
        assert!(t.take_writes().is_empty());

        // Fangs: 20 adena each, +3880 from ten of them, the fangs go.
        hunt(&mut t, 10);
        let fangs = t.count(1, LIZARD_FANG);
        assert!(fangs >= 10);
        t.quest_page(1, iason);
        assert!(t.event(1, NAME, "30969-02.html").contains("30969-04.html"));
        assert_eq!(t.count(1, 57), 20 * fangs + 3880);
        assert_eq!(t.count(1, LIZARD_FANG), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fewer_than_ten_fangs_get_no_bonus() {
        let (mut t, gosta, _roman, iason) = setup(false);
        t.enter(1, 32, vec![item(100, LIZARD_FANG, 3)]);
        accept(&mut t, gosta);
        t.quest_page(1, iason);
        assert!(t.event(1, NAME, "30969-02.html").contains("30969-04.html"));
        assert_eq!(t.count(1, 57), 60);
        assert_eq!(t.count(1, LIZARD_FANG), 0);
    }

    #[test]
    fn the_barrels_become_bills_and_set_cond_two() {
        let (mut t, gosta, roman, iason) = setup(false);
        t.enter(1, 32, vec![item(100, BARREL_OF_LEAGUE, 2)]);
        accept(&mut t, gosta);
        t.take_writes();

        t.quest_page(1, iason);
        assert!(t.event(1, NAME, "30969-05.html").contains("30969-06.html"));
        assert_eq!(t.count(1, BILL_OF_IASON_HEINE), 2);
        assert_eq!(t.count(1, BARREL_OF_LEAGUE), 0);
        assert_eq!(t.count(1, 57), 3880);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Roman exchanges the bills (the page says so).
        assert!(t.quest_page(1, roman).contains("30897-01.html"));
    }

    #[test]
    fn roman_without_a_bill_sends_the_player_on() {
        let (mut t, gosta, roman, _iason) = setup(false);
        t.enter(1, 32, vec![]);
        // Not started: Roman has no quest page (`getQuestState` and the other quest are not there).
        let page = t.quest_page(1, roman);
        assert!(!page.contains("30897-"), "{page}");
        accept(&mut t, gosta);
        assert!(t.quest_page(1, roman).contains("30897-02.html"));
    }

    #[test]
    fn roman_talks_to_the_player_of_black_swan_or_method_to_raise_the_dead() {
        let (mut t, _gosta, roman, _iason) = setup(true);
        t.enter(1, 32, vec![]);
        // Quest 345 started, Black Swan not: Roman answers with the bill page.
        t.world.run_as_script(METHOD_TO_RAISE_THE_DEAD, |ctx| {
            let qs = ctx
                .quest_state_of(METHOD_TO_RAISE_THE_DEAD, Oid(1), true)
                .unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
        assert!(t.quest_page(1, roman).contains("30897-02.html"));
    }

    #[test]
    fn quitting_takes_the_order_and_ends_the_repeatable_quest() {
        let (mut t, gosta, _roman, iason) = setup(false);
        t.enter(1, 32, vec![]);
        accept(&mut t, gosta);
        t.take_writes();

        // "Show the wooden barrels" with none gives the page back, which links to the quit task.
        t.quest_page(1, iason);
        assert!(t.event(1, NAME, "30969-05.html").contains("30969-05.html"));
        assert!(t.event(1, NAME, "30969-03.html").contains("30969-03.html"));
        assert!(t.event(1, NAME, "30969-07.html").contains("30969-07.html"));
        t.bypass(1, &format!("Script {NAME} 30969-09.html"));
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, ORDER_OF_GOSTA), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, gosta).contains("30916-01.htm"));
    }

    #[test]
    fn quitting_with_fangs_and_barrels_is_refused() {
        let (mut t, gosta, _roman, iason) = setup(false);
        t.enter(1, 32, vec![item(100, LIZARD_FANG, 1)]);
        accept(&mut t, gosta);
        t.take_writes();
        t.quest_page(1, iason);
        // "Show the barrels" with none keeps the page (no barrel is handed in yet).
        assert!(t.event(1, NAME, "30969-05.html").contains("30969-05.html"));
        assert!(t.event(1, NAME, "30969-03.html").contains("30969-03.html"));
        // A barrel found meanwhile: the quit is refused.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), BARREL_OF_LEAGUE, 1));
        assert!(t.event(1, NAME, "30969-07.html").contains("30969-08.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn quitting_with_only_fangs_is_allowed() {
        let (mut t, gosta, _roman, iason) = setup(false);
        t.enter(1, 32, vec![item(100, LIZARD_FANG, 1)]);
        accept(&mut t, gosta);
        t.quest_page(1, iason);
        assert!(t.event(1, NAME, "30969-05.html").contains("30969-05.html"));
        assert!(t.event(1, NAME, "30969-03.html").contains("30969-03.html"));
        assert!(t.event(1, NAME, "30969-07.html").contains("30969-07.html"));
    }

    #[test]
    fn a_kill_out_of_range_gives_nothing() {
        let (mut t, gosta, _roman, _iason) = setup(false);
        t.enter(1, 32, vec![]);
        accept(&mut t, gosta);
        let far = t
            .world
            .spawn_npc_at(
                TASABA_LIZARDMAN1,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, LIZARD_FANG), 0);
        assert_eq!(t.count(1, BARREL_OF_LEAGUE), 0);
    }
}
