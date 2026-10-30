//! Quest 246: Possessor of a Precious Soul 3 (Caradine sends a sub-class character to Ossian, who
//! needs the Waterbinder and the Evergreen from the Splendor monsters, the Rain Song from
//! Barakiel and the fragments for the relic box, which Ladd takes back to Caradine).
// Java: dist/game/data/scripts/quests/Q00246_PossessorOfAPreciousSoul3/Q00246_PossessorOfAPreciousSoul3.java
//
// Pattern: a sub-class quest (`no_sub.html`), per-quest variables that pick a random party member
// (`awaitsWaterbinder`, `awaitsEvergreen`), a party kill with a distance check, and a long chain of
// conds (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LADD: i32 = 30721;
const CARADINE: i32 = 31740;
const OSSIAN: i32 = 31741;
const PILGRIM_OF_SPLENDOR: i32 = 21541;
const JUDGE_OF_SPLENDOR: i32 = 21544;
const BARAKIEL: i32 = 25325;
// Signet, Crown, Fang, Fang, Wailing, Wailing of Splendor
const MOBS: [i32; 6] = [21535, 21536, 21537, 21538, 21539, 21540];

// Items
const CARADINE_LETTER: i32 = 7678;
const CARADINE_LETTER_LAST: i32 = 7679;
const WATERBINDER: i32 = 7591;
const EVERGREEN: i32 = 7592;
const RAIN_SONG: i32 = 7593;
const RELIC_BOX: i32 = 7594;
const FRAGMENTS: i32 = 21725;

// Rewards
const CHANCE_FOR_DROP: i32 = 30;
const CHANCE_FOR_DROP_FRAGMENTS: i32 = 60;

/// Java `Q00246_PossessorOfAPreciousSoul3`.
pub struct Q00246PossessorOfAPreciousSoul3;

impl Script for Q00246PossessorOfAPreciousSoul3 {
    fn name(&self) -> &'static str {
        "Q00246_PossessorOfAPreciousSoul3"
    }

    fn path(&self) -> &'static str {
        "quests/Q00246_PossessorOfAPreciousSoul3"
    }

    fn id(&self) -> i32 {
        246
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CARADINE]);
        s.add_talk_id(&[LADD, CARADINE, OSSIAN]);
        s.add_kill_id(&[PILGRIM_OF_SPLENDOR, JUDGE_OF_SPLENDOR, BARAKIEL]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[WATERBINDER, EVERGREEN, FRAGMENTS, RAIN_SONG, RELIC_BOX]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(st) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        if !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }
        match event {
            "31740-4.html" => {
                if st.is_created(ctx) {
                    ctx.take_items(player, CARADINE_LETTER, -1);
                    st.start_quest(ctx);
                }
            }
            "31741-2.html" => {
                if st.is_started(ctx) && st.is_cond(ctx, 1) {
                    st.set(ctx, "awaitsWaterbinder", "1");
                    st.set(ctx, "awaitsEvergreen", "1");
                    st.set_cond(ctx, 2, true);
                }
            }
            "31741-5.html" => {
                if st.is_cond(ctx, 3)
                    && ctx.has_quest_items(player, WATERBINDER)
                    && ctx.has_quest_items(player, EVERGREEN)
                {
                    ctx.take_items(player, WATERBINDER, 1);
                    ctx.take_items(player, EVERGREEN, 1);
                    st.set_cond(ctx, 4, true);
                }
            }
            "31741-9.html" => {
                if st.is_cond(ctx, 5)
                    && (ctx.has_quest_items(player, RAIN_SONG)
                        || ctx.get_quest_items_count(player, FRAGMENTS) >= 100)
                {
                    ctx.take_items(player, RAIN_SONG, -1);
                    ctx.take_items(player, FRAGMENTS, -1);
                    ctx.give_items(player, RELIC_BOX, 1);
                    st.set_cond(ctx, 6, true);
                } else {
                    return Some("31741-8.html".to_owned());
                }
            }
            "30721-2.html" => {
                if st.is_cond(ctx, 6) && ctx.has_quest_items(player, RELIC_BOX) {
                    ctx.take_items(player, RELIC_BOX, -1);
                    ctx.give_items(player, CARADINE_LETTER_LAST, 1);
                    ctx.add_exp_and_sp(player, 719843, 0);
                    st.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            PILGRIM_OF_SPLENDOR => {
                if let Some(member) = ctx.get_random_party_member_var(killer, Some("awaitsWaterbinder"), "1")
                {
                    let Some(st) = ctx.get_quest_state(member, false) else {
                        return;
                    };
                    let chance = ctx.get_random(100);
                    if st.is_cond(ctx, 2)
                        && !ctx.has_quest_items(member, WATERBINDER)
                        && chance < CHANCE_FOR_DROP
                    {
                        ctx.give_items(member, WATERBINDER, 1);
                        st.unset(ctx, "awaitsWaterbinder");
                        if ctx.has_quest_items(member, EVERGREEN) {
                            st.set_cond(ctx, 3, true);
                        } else {
                            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
            }
            JUDGE_OF_SPLENDOR => {
                if let Some(member) = ctx.get_random_party_member_var(killer, Some("awaitsEvergreen"), "1") {
                    let Some(st) = ctx.get_quest_state(member, false) else {
                        return;
                    };
                    let chance = ctx.get_random(100);
                    if st.is_cond(ctx, 2)
                        && !ctx.has_quest_items(member, EVERGREEN)
                        && chance < CHANCE_FOR_DROP
                    {
                        ctx.give_items(member, EVERGREEN, 1);
                        st.unset(ctx, "awaitsEvergreen");
                        if ctx.has_quest_items(member, WATERBINDER) {
                            st.set_cond(ctx, 3, true);
                        } else {
                            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
            }
            // The Rain Song goes to every member near Barakiel (the party's own distance check is
            // in `action_for_each_player`).
            BARAKIEL => ctx.execute_for_each_player(killer, npc, false, true, false),
            _ => {
                let Some(st) = ctx.get_quest_state(killer, false) else {
                    return;
                };
                if MOBS.contains(&npc.id)
                    && ctx.get_quest_items_count(killer, FRAGMENTS) < 100
                    && st.is_cond(ctx, 4)
                    && ctx.get_random(100) < CHANCE_FOR_DROP_FRAGMENTS
                {
                    ctx.give_items(killer, FRAGMENTS, 1);
                    if ctx.get_quest_items_count(killer, FRAGMENTS) < 100 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    } else {
                        st.set_cond(ctx, 5, true);
                    }
                }
            }
        }
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        // Java: a party member farther than `AltPartyRange` from Barakiel gets nothing (the solo
        // killer is not checked).
        if ctx.is_in_party(player)
            && ctx
                .distance_3d(player, npc.object_id)
                .is_some_and(|d| d > f64::from(ctx.alt_party_range()))
        {
            return;
        }
        if let Some(pst) = ctx.get_quest_state(player, false)
            && pst.is_cond(ctx, 4)
            && !ctx.has_quest_items(player, RAIN_SONG)
        {
            ctx.give_items(player, RAIN_SONG, 1);
            pst.set_cond(ctx, 5, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        if st.is_started(ctx) && !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            CARADINE => match st.state(ctx) {
                CREATED => {
                    let fore_done = ctx
                        .player_quest_state(player, "Q00242_PossessorOfAPreciousSoul2")
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= 65 && fore_done {
                        "31740-1.htm"
                    } else {
                        "31740-2.html"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31740-5.html".to_owned(),
                _ => {}
            },
            OSSIAN => {
                if st.is_started(ctx) {
                    match st.get_cond(ctx) {
                        1 => htmltext = "31741-1.html".to_owned(),
                        2 => htmltext = "31741-4.html".to_owned(),
                        3 => {
                            if ctx.has_quest_items(player, WATERBINDER)
                                && ctx.has_quest_items(player, EVERGREEN)
                            {
                                htmltext = "31741-3.html".to_owned();
                            }
                        }
                        4 => htmltext = "31741-8.html".to_owned(),
                        5 => {
                            htmltext = if ctx.has_quest_items(player, RAIN_SONG)
                                || ctx.get_quest_items_count(player, FRAGMENTS) >= 100
                            {
                                "31741-7.html"
                            } else {
                                "31741-8.html"
                            }
                            .to_owned();
                        }
                        6 => {
                            if ctx.get_quest_items_count(player, RELIC_BOX) == 1 {
                                htmltext = "31741-11.html".to_owned();
                            }
                        }
                        _ => {}
                    }
                }
            }
            LADD => match st.state(ctx) {
                STARTED => {
                    if st.is_cond(ctx, 6) {
                        htmltext = "30721-1.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
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
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00246_PossessorOfAPreciousSoul3";
    const FORE: &str = "Q00242_PossessorOfAPreciousSoul2";

    const NPCS: &str = r#"
        <npc id="30721" type="Folk" level="70" name="Ladd"/>
        <npc id="31740" type="Folk" level="70" name="Caradine"/>
        <npc id="31741" type="Folk" level="70" name="Ossian"/>
        <npc id="21541" type="Monster" level="80" name="Pilgrim of Splendor"/>
        <npc id="21544" type="Monster" level="80" name="Judge of Splendor"/>
        <npc id="25325" type="RaidBoss" level="80" name="Barakiel"/>"#;

    const ITEMS: &str = r#"
        <item id="7678" type="EtcItem" name="Caradine's Letter"/>
        <item id="7679" type="EtcItem" name="Caradine's Letter - Last"/>
        <item id="7591" type="EtcItem" name="Waterbinder"><set name="is_questitem" val="true"/></item>
        <item id="7592" type="EtcItem" name="Evergreen"><set name="is_questitem" val="true"/></item>
        <item id="7593" type="EtcItem" name="Rain Song"><set name="is_questitem" val="true"/></item>
        <item id="7594" type="EtcItem" name="Relic Box"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00242_PossessorOfAPreciousSoul2`: a stand-in whose state a test completes.
    struct Fore;

    impl Script for Fore {
        fn name(&self) -> &'static str {
            FORE
        }
        fn path(&self) -> &'static str {
            "quests/Q00242_PossessorOfAPreciousSoul2"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Fore);
        registry.add(Q00246PossessorOfAPreciousSoul3);
        let pages = Pages::of(&Q00246PossessorOfAPreciousSoul3)
            .page("31740-1.htm", &["31740-4.html"])
            .plain(&["31740-2.html", "31740-4.html", "31740-5.html", "no_sub.html"])
            .page("31741-1.html", &["31741-2.html"])
            .plain(&["31741-2.html", "31741-4.html", "31741-8.html", "31741-11.html"])
            .page("31741-3.html", &["31741-5.html"])
            .plain(&["31741-5.html"])
            .page("31741-7.html", &["31741-9.html"])
            .plain(&["31741-9.html"])
            .page("30721-1.html", &["30721-2.html"])
            .plain(&["30721-2.html"])
            .npcs(&[LADD, CARADINE, OSSIAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let caradine = t.spawn(CARADINE, 20);
        let ossian = t.spawn(OSSIAN, 40);
        let ladd = t.spawn(LADD, 60);
        (t, caradine, ossian, ladd)
    }

    /// A level-70 character with an active sub class, who finished the second soul quest.
    fn ready(t: &mut ScriptTest, id: i32) {
        let mut data = player_data(id, 70, vec![item(10 * id, CARADINE_LETTER, 1)]);
        data.profile.class_index = 1;
        t.enter_with(data);
    }

    #[test]
    fn the_quest_is_offered_after_the_second_soul_and_level_65() {
        let (mut t, caradine, _, _) = setup();
        ready(&mut t, 1);
        t.world.run_as_script(FORE, |ctx| {
            if let Some(qs) = ctx.quest_state_of(FORE, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, caradine).contains("31740-1.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_character_without_a_sub_class_is_sent_away() {
        let (mut t, caradine, _, _) = setup();
        t.enter_with(player_data(1, 70, vec![]));
        assert!(t.quest_page(1, caradine).contains("31740-2.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_from_caradine_through_ossian_to_ladd() {
        let (mut t, caradine, ossian, ladd) = setup();
        ready(&mut t, 1);
        t.world.run_as_script(FORE, |ctx| {
            if let Some(qs) = ctx.quest_state_of(FORE, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();

        // Caradine: the letter is taken and the quest starts.
        assert!(t.quest_page(1, caradine).contains("31740-1.htm"));
        assert!(t.event(1, NAME, "31740-4.html").contains("31740-4.html"));
        assert_eq!(t.count(1, CARADINE_LETTER), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Ossian sends the player out for the Waterbinder and the Evergreen (cond 2).
        assert!(t.quest_page(1, ossian).contains("31741-1.html"));
        assert!(t.event(1, NAME, "31741-2.html").contains("31741-2.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "awaitsWaterbinder", "1"),
                rows::insert(NAME, "awaitsEvergreen", "1"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // The Pilgrim gives the Waterbinder; the variable is removed.
        let pilgrim_done = |t: &ScriptTest| t.count(1, WATERBINDER) >= 1;
        for _ in 0..200 {
            if pilgrim_done(&t) {
                break;
            }
            let p = t.spawn(PILGRIM_OF_SPLENDOR, 20);
            t.kill(1, p);
        }
        assert_eq!(t.count(1, WATERBINDER), 1);
        assert_eq!(t.take_writes(), [rows::delete_var(NAME, "awaitsWaterbinder")]);

        // The Judge gives the Evergreen; with the Waterbinder the quest goes to cond 3.
        for _ in 0..200 {
            if t.count(1, EVERGREEN) >= 1 {
                break;
            }
            let j = t.spawn(JUDGE_OF_SPLENDOR, 20);
            t.kill(1, j);
        }
        assert_eq!(t.count(1, EVERGREEN), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete_var(NAME, "awaitsEvergreen"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // Ossian takes both and sends the player to Barakiel (cond 4).
        assert!(t.quest_page(1, ossian).contains("31741-3.html"));
        assert!(t.event(1, NAME, "31741-5.html").contains("31741-5.html"));
        assert_eq!((t.count(1, WATERBINDER), t.count(1, EVERGREEN)), (0, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Barakiel gives the Rain Song (cond 5).
        let b = t.spawn(BARAKIEL, 20);
        t.kill(1, b);
        assert_eq!(t.count(1, RAIN_SONG), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Ossian makes the relic box from the Rain Song (cond 6).
        assert!(t.quest_page(1, ossian).contains("31741-7.html"));
        assert!(t.event(1, NAME, "31741-9.html").contains("31741-9.html"));
        assert_eq!((t.count(1, RAIN_SONG), t.count(1, RELIC_BOX)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Ladd takes the box back to Caradine's letter; the quest ends.
        assert!(t.quest_page(1, ladd).contains("30721-1.html"));
        assert!(t.event(1, NAME, "30721-2.html").contains("30721-2.html"));
        assert_eq!(t.count(1, CARADINE_LETTER_LAST), 1);
        assert_eq!(t.count(1, RELIC_BOX), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
