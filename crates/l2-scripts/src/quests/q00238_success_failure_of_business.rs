//! Quest 238: Success / Failure of Business (Helvetica in the Hot Springs Village breaks the
//! Brazier of Purity and the Evil and Guardian Spirits of the Ruins of Agony for the Fos
//! vicinity).
// Java: dist/game/data/scripts/quests/Q00238_SuccessFailureOfBusiness/Q00238_SuccessFailureOfBusiness.java
//
// Pattern: two kill counts in a row with a party draw at a different cond each (`docs/design/
// scripts.md`, "Kills, drops and parties"). The items of both counts go to the killer, the cond
// to the member the draw picked: Java's, kept as it is.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HELVETICA: i32 = 32641;

// Mobs
const BRAZIER_OF_PURITY: i32 = 18806;
const EVIL_SPIRITS: i32 = 22658;
const GUARDIAN_SPIRITS: i32 = 22659;

// Items
const VICINITY_OF_FOS: i32 = 14865;
const BROKEN_PIECE_OF_MAGIC_FORCE: i32 = 14867;
const GUARDIAN_SPIRIT_FRAGMENT: i32 = 14868;

// Misc
const BROKEN_PIECE_OF_MAGIC_FORCE_NEEDED: i64 = 10;
const GUARDIAN_SPIRIT_FRAGMENT_NEEDED: i64 = 20;
const CHANCE_FOR_FRAGMENT: i32 = 80;
const MIN_LEVEL: i32 = 82;

/// Java `Q00238_SuccessFailureOfBusiness`.
pub struct Q00238SuccessFailureOfBusiness;

impl Script for Q00238SuccessFailureOfBusiness {
    fn name(&self) -> &'static str {
        "Q00238_SuccessFailureOfBusiness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00238_SuccessFailureOfBusiness"
    }

    fn id(&self) -> i32 {
        238
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HELVETICA]);
        s.add_talk_id(&[HELVETICA]);
        s.add_kill_id(&[BRAZIER_OF_PURITY, EVIL_SPIRITS, GUARDIAN_SPIRITS]);
        s.register_quest_items(&[BROKEN_PIECE_OF_MAGIC_FORCE, GUARDIAN_SPIRIT_FRAGMENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "32641-02.htm" => Some(event.to_owned()),
            "32641-03.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "32641-06.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == BRAZIER_OF_PURITY {
            // NOTE(java-quirk): the piece goes to the killer, the cond change to the party member
            // at cond 1 that the draw picked (the same for the fragments below).
            if let Some(member) = ctx.get_random_party_member_cond(killer, 1) {
                let Some(qs) = ctx.get_quest_state(member, false) else {
                    return;
                };
                if ctx.get_quest_items_count(killer, BROKEN_PIECE_OF_MAGIC_FORCE)
                    < BROKEN_PIECE_OF_MAGIC_FORCE_NEEDED
                {
                    ctx.give_items(killer, BROKEN_PIECE_OF_MAGIC_FORCE, 1);
                }
                if ctx.get_quest_items_count(killer, BROKEN_PIECE_OF_MAGIC_FORCE)
                    == BROKEN_PIECE_OF_MAGIC_FORCE_NEEDED
                {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        } else if let Some(member) = ctx.get_random_party_member_cond(killer, 3)
            && ctx.get_random(100) < CHANCE_FOR_FRAGMENT
        {
            let Some(qs) = ctx.get_quest_state(member, false) else {
                return;
            };
            if ctx.get_quest_items_count(killer, GUARDIAN_SPIRIT_FRAGMENT) < GUARDIAN_SPIRIT_FRAGMENT_NEEDED {
                ctx.give_items(killer, GUARDIAN_SPIRIT_FRAGMENT, 1);
            }
            if ctx.get_quest_items_count(killer, GUARDIAN_SPIRIT_FRAGMENT) == GUARDIAN_SPIRIT_FRAGMENT_NEEDED
            {
                qs.set_cond(ctx, 4, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = "32641-09.html".to_owned(),
            CREATED => {
                let q237_done = ctx
                    .player_quest_state(player, "Q00237_WindsOfChange")
                    .is_some_and(|q| q.is_completed());
                let q239_done = ctx
                    .player_quest_state(player, "Q00239_WontYouJoinUs")
                    .is_some_and(|q| q.is_completed());
                if q239_done {
                    htmltext = "32641-10.html".to_owned();
                } else if q237_done
                    && ctx.player_level(player) >= MIN_LEVEL
                    && ctx.has_quest_items(player, VICINITY_OF_FOS)
                {
                    htmltext = "32641-01.htm".to_owned();
                } else {
                    htmltext = "32641-00.html".to_owned();
                }
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "32641-04.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, BROKEN_PIECE_OF_MAGIC_FORCE)
                        == BROKEN_PIECE_OF_MAGIC_FORCE_NEEDED
                    {
                        htmltext = "32641-05.html".to_owned();
                        ctx.take_items(player, BROKEN_PIECE_OF_MAGIC_FORCE, -1);
                    }
                }
                3 => htmltext = "32641-07.html".to_owned(),
                4 => {
                    if ctx.get_quest_items_count(player, GUARDIAN_SPIRIT_FRAGMENT)
                        == GUARDIAN_SPIRIT_FRAGMENT_NEEDED
                    {
                        htmltext = "32641-08.html".to_owned();
                        ctx.give_adena(player, 283346, true);
                        ctx.take_items(player, VICINITY_OF_FOS, 1);
                        ctx.add_exp_and_sp(player, 1319736, 103553);
                        qs.exit_quest(ctx, false, true);
                    }
                }
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

    const NAME: &str = "Q00238_SuccessFailureOfBusiness";
    const Q237: &str = "Q00237_WindsOfChange";
    const Q239: &str = "Q00239_WontYouJoinUs";

    const NPCS: &str = r#"
        <npc id="32641" type="Folk" level="70" name="Helvetica"/>
        <npc id="18806" type="Monster" level="80" name="Brazier of Purity"/>
        <npc id="22658" type="Monster" level="80" name="Evil Spirit"/>
        <npc id="22659" type="Monster" level="80" name="Guardian Spirit"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14865" type="EtcItem" name="Vicinity of Fos"><set name="is_questitem" val="true"/></item>
        <item id="14867" type="EtcItem" name="Broken Piece of Magic Force"><set name="is_questitem" val="true"/></item>
        <item id="14868" type="EtcItem" name="Guardian Spirit Fragment"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00237_WindsOfChange` and `Q00239_WontYouJoinUs`: stand-ins whose state a test
    /// completes (the states Helvetica reads).
    struct Stand237;
    struct Stand239;

    impl Script for Stand237 {
        fn name(&self) -> &'static str {
            Q237
        }
        fn path(&self) -> &'static str {
            "quests/Q00237_WindsOfChange"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    impl Script for Stand239 {
        fn name(&self) -> &'static str {
            Q239
        }
        fn path(&self) -> &'static str {
            "quests/Q00239_WontYouJoinUs"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Stand237);
        registry.add(Stand239);
        registry.add(Q00238SuccessFailureOfBusiness);
        let pages = Pages::of(&Q00238SuccessFailureOfBusiness)
            .page("32641-01.htm", &["32641-02.htm"])
            .page("32641-02.htm", &["32641-03.html"])
            .plain(&[
                "32641-00.html",
                "32641-03.html",
                "32641-04.html",
                "32641-07.html",
                "32641-08.html",
                "32641-09.html",
                "32641-10.html",
            ])
            .page("32641-05.html", &["32641-06.html"])
            .plain(&["32641-06.html"])
            .npcs(&[HELVETICA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let helvetica = t.spawn(HELVETICA, 20);
        (t, helvetica)
    }

    /// Completes the quest `name` of `player` (a stand-in script's state).
    fn complete(t: &mut ScriptTest, name: &'static str, player: i32) {
        t.world.run_as_script(name, |ctx| {
            if let Some(qs) = ctx.quest_state_of(name, Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    /// A level-82 character with the Vicinity of Fos, who finished Winds of Change.
    fn ready(t: &mut ScriptTest) {
        t.enter_with(player_data(1, 82, vec![item(10, VICINITY_OF_FOS, 1)]));
        complete(t, Q237, 1);
    }

    #[test]
    fn the_quest_is_offered_after_winds_of_change() {
        let (mut t, helvetica) = setup();
        t.enter_with(player_data(1, 82, vec![item(10, VICINITY_OF_FOS, 1)]));
        // Winds of Change not done yet.
        assert!(t.quest_page(1, helvetica).contains("32641-00.html"));
        complete(&mut t, Q237, 1);
        assert!(t.quest_page(1, helvetica).contains("32641-01.htm"));
    }

    #[test]
    fn a_character_below_level_82_or_without_the_vicinity_is_sent_away() {
        let (mut t, helvetica) = setup();
        t.enter_with(player_data(1, 81, vec![item(10, VICINITY_OF_FOS, 1)]));
        complete(&mut t, Q237, 1);
        assert!(t.quest_page(1, helvetica).contains("32641-00.html"));
        t.enter_with(player_data(2, 82, vec![]));
        complete(&mut t, Q237, 2);
        assert!(t.quest_page(2, helvetica).contains("32641-00.html"));
    }

    #[test]
    fn the_brazier_and_the_spirits_bring_the_pieces_and_the_reward() {
        let (mut t, helvetica) = setup();
        ready(&mut t);
        assert!(t.quest_page(1, helvetica).contains("32641-01.htm"));
        assert!(t.event(1, NAME, "32641-02.htm").contains("32641-02.htm"));
        assert!(t.event(1, NAME, "32641-03.html").contains("32641-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, helvetica).contains("32641-04.html"));

        // Ten pieces from the braziers; the tenth sets cond 2.
        for _ in 0..9 {
            let brazier = t.spawn(BRAZIER_OF_PURITY, 20);
            t.kill(1, brazier);
        }
        assert_eq!(t.count(1, BROKEN_PIECE_OF_MAGIC_FORCE), 9);
        assert!(t.take_writes().is_empty());
        let brazier = t.spawn(BRAZIER_OF_PURITY, 20);
        t.kill(1, brazier);
        assert_eq!(t.count(1, BROKEN_PIECE_OF_MAGIC_FORCE), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Helvetica takes the pieces and sends the player to the spirits (cond 3).
        assert!(t.quest_page(1, helvetica).contains("32641-05.html"));
        assert_eq!(t.count(1, BROKEN_PIECE_OF_MAGIC_FORCE), 0);
        assert!(t.event(1, NAME, "32641-06.html").contains("32641-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, helvetica).contains("32641-07.html"));

        // Twenty fragments from the spirits (cond 4 on the twentieth, a roll of 80% each).
        for _ in 0..200 {
            if t.count(1, GUARDIAN_SPIRIT_FRAGMENT) >= GUARDIAN_SPIRIT_FRAGMENT_NEEDED {
                break;
            }
            let spirit = t.spawn(EVIL_SPIRITS, 20);
            t.kill(1, spirit);
        }
        assert_eq!(
            t.count(1, GUARDIAN_SPIRIT_FRAGMENT),
            GUARDIAN_SPIRIT_FRAGMENT_NEEDED
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Helvetica pays and the quest ends; the vicinity is taken.
        assert!(t.quest_page(1, helvetica).contains("32641-08.html"));
        assert_eq!(t.count(1, 57), 283346);
        assert_eq!(
            (t.count(1, VICINITY_OF_FOS), t.count(1, GUARDIAN_SPIRIT_FRAGMENT)),
            (0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, helvetica).contains("32641-09.html"));
    }

    #[test]
    fn the_spirits_of_the_guardians_do_not_count_for_the_brazier() {
        let (mut t, helvetica) = setup();
        ready(&mut t);
        t.quest_page(1, helvetica);
        t.event(1, NAME, "32641-02.htm");
        t.event(1, NAME, "32641-03.html");
        t.take_writes();
        // A spirit at cond 1 gives nothing (the spirits need cond 3).
        let spirit = t.spawn(GUARDIAN_SPIRITS, 20);
        t.kill(1, spirit);
        assert_eq!(t.count(1, GUARDIAN_SPIRIT_FRAGMENT), 0);
        assert!(t.take_writes().is_empty());
    }
}
