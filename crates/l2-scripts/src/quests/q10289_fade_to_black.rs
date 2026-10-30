//! Quest 10289: Fade to Black (Greymore in the Gludio area, Anays).
// Java: dist/game/data/scripts/quests/Q10289_FadeToBlack/Q10289_FadeToBlack.java
//
// Pattern: a party kill with one lucky member (`docs/design/scripts.md`, "Kills, drops and
// parties"): the killing blow goes to a random started member (`random_party_member_state`,
// `check_party_member_state` = Java `checkPartyMember`, cond below 3). In a party one member
// gets the Mark of Splendor, the others the Mark of Darkness. The Mark of Splendor is traded
// for a Moirai armour piece (and the Marks of Darkness add exp).

use l2_util::ObjectId;
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GREYMORE: i32 = 32757;

// Items
const MARK_OF_SPLENDOR: i32 = 15527;
const MARK_OF_DARKNESS: i32 = 15528;

// Monster
const ANAYS: i32 = 25701;

// Misc
/// Java `Q10288_SecretMission.class.getSimpleName()`.
const SECRET_MISSION: &str = "Q10288_SecretMission";

/// Java `Q10289_FadeToBlack`.
pub struct Q10289FadeToBlack;

/// Java's `switch (event)` of "11".."30" in `onEvent`: the Moirai armour piece and the adena
/// with it (`None` for "14", which gives no adena). `None` for any other event.
fn moirai_reward(choice: &str) -> Option<(i32, Option<i64>)> {
    match choice {
        "11" => Some((15775, Some(420_920))),
        "12" => Some((15776, Some(420_920))),
        "13" => Some((15777, Some(420_920))),
        "14" => Some((15778, None)),
        "15" => Some((15779, Some(168_360))),
        "16" => Some((15780, Some(168_360))),
        "17" => Some((15781, Some(252_540))),
        "18" => Some((15782, Some(357_780))),
        "19" => Some((15783, Some(357_780))),
        "20" => Some((15784, Some(505_100))),
        "21" => Some((15785, Some(505_100))),
        "22" => Some((15786, Some(505_100))),
        "23" => Some((15787, Some(505_100))),
        // NOTE(java-quirk): Java's "24" gives 15787 again (the piece of "23"), not 15788.
        "24" => Some((15787, Some(505_100))),
        "25" => Some((15789, Some(505_100))),
        "26" => Some((15790, Some(496_680))),
        "27" => Some((15791, Some(496_680))),
        "28" => Some((15792, Some(563_860))),
        "29" => Some((15793, Some(509_040))),
        "30" => Some((15794, Some(454_240))),
        _ => None,
    }
}

/// Java `rewardPlayer(qs, isLucky)` (the `qs` may be null: a member without the quest).
fn reward_player(ctx: &mut ScriptCtx<'_>, qs: Option<Qs>, is_lucky: bool) {
    if let Some(qs) = qs
        && qs.is_cond(ctx, 1)
    {
        let player = qs.player();
        ctx.give_items(
            player,
            if is_lucky {
                MARK_OF_SPLENDOR
            } else {
                MARK_OF_DARKNESS
            },
            1,
        );
        qs.set_cond(ctx, if is_lucky { 3 } else { 2 }, true);
    }
}

impl Script for Q10289FadeToBlack {
    fn name(&self) -> &'static str {
        "Q10289_FadeToBlack"
    }

    fn path(&self) -> &'static str {
        "quests/Q10289_FadeToBlack"
    }

    fn id(&self) -> i32 {
        10289
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GREYMORE]);
        s.add_talk_id(&[GREYMORE]);
        s.add_kill_id(&[ANAYS]);
        s.register_quest_items(&[MARK_OF_SPLENDOR, MARK_OF_DARKNESS]);
    }

    fn check_party_member_state(&self, ctx: &mut ScriptCtx<'_>, qs: Qs, _npc: Option<ScriptNpc>) -> bool {
        qs.get_cond(ctx) < 3
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (not the no-quest page) when there is no state.
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32757-02.htm" => htmltext = Some(event.to_owned()),
            "32757-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32757-06.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MARK_OF_DARKNESS) {
                    htmltext = Some("32757-07.html".to_owned());
                } else if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, MARK_OF_SPLENDOR) {
                    htmltext = Some("32757-08.html".to_owned());
                } else {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {
                // "11" .. "30": the Moirai armour pieces (see `moirai_reward`).
                if let Some((item, adena)) = moirai_reward(event)
                    && qs.is_cond(ctx, 3)
                    && ctx.has_quest_items(player, MARK_OF_SPLENDOR)
                {
                    ctx.reward_items(player, item, 1);
                    if let Some(adena) = adena {
                        ctx.give_adena(player, adena, true);
                    }

                    let marks_of_darkness = ctx.get_quest_items_count(player, MARK_OF_DARKNESS);
                    if marks_of_darkness > 0 {
                        ctx.add_exp_and_sp(
                            player,
                            55_983 * marks_of_darkness,
                            136_500 * marks_of_darkness as i32,
                        );
                    }

                    qs.exit_quest(ctx, false, true);
                    htmltext = Some("32757-09.html".to_owned());
                }
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if ctx.is_in_party(qs.player()) {
            // if player is in party, reward all members
            let members = ctx.world().party_members_of(qs.player());
            let rnd = ctx.get_random(members.len() as i32);
            for (idx, member) in members.into_iter().enumerate() {
                // only one lucky player will get the good item, the rest will get the bad one
                let member_qs = ctx.get_quest_state(member, false);
                reward_player(ctx, member_qs, idx as i32 == rnd);
            }
        } else {
            // if no party, the winner gets it all
            reward_player(ctx, Some(qs), true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            let secret_mission_done = ctx
                .player_quest_state(player, SECRET_MISSION)
                .is_some_and(|q| q.is_completed());
            htmltext = if ctx.player_level(player) < 82 || !secret_mission_done {
                "32757-00.htm"
            } else {
                "32757-01.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match qs.get_cond(ctx) {
                1 => htmltext = "32757-04.html".to_owned(),
                2 | 3 => htmltext = "32757-05.html".to_owned(),
                _ => {}
            }
        } else {
            htmltext = "32757-10.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q10289_FadeToBlack";

    const NPCS: &str = r#"
        <npc id="32757" type="Folk" level="80" name="Greymore"/>
        <npc id="25701" type="RaidBoss" level="80" name="Anays"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15527" type="EtcItem" name="Mark of Splendor"><set name="is_questitem" val="true"/></item>
        <item id="15528" type="EtcItem" name="Mark of Darkness"><set name="is_questitem" val="true"/></item>
        <item id="15775" type="Armor" name="Moirai Robe"><set name="bodypart" val="chest"/></item>
        <item id="15787" type="Armor" name="Moirai Gloves"><set name="bodypart" val="gloves"/></item>
        <item id="15788" type="Armor" name="Moirai Boots"><set name="bodypart" val="feet"/></item>"#;

    /// Java `Q10288` completed: the condition of the start page.
    fn secret_mission_done() -> QuestLoad {
        QuestLoad {
            states: vec![(SECRET_MISSION.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        }
    }

    /// Stands in for Q10288 (its state is read through `player_quest_state` only).
    struct SecretMissionStandIn;

    impl Script for SecretMissionStandIn {
        fn name(&self) -> &'static str {
            SECRET_MISSION
        }

        fn path(&self) -> &'static str {
            "quests/Q10288_SecretMission"
        }

        fn id(&self) -> i32 {
            10288
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10289FadeToBlack);
        registry.add(SecretMissionStandIn);
        let mut choices = vec![];
        for n in 11..=30 {
            choices.push(n.to_string());
        }
        let choices: Vec<&str> = choices.iter().map(String::as_str).collect();
        let pages = Pages::of(&Q10289FadeToBlack)
            .page("32757-01.htm", &["32757-02.htm"])
            .page("32757-02.htm", &["32757-03.htm"])
            .plain(&["32757-00.htm", "32757-03.htm", "32757-04.html"])
            .page("32757-05.html", &["32757-06.html"])
            .plain(&["32757-06.html", "32757-07.html"])
            .page("32757-08.html", &choices)
            .plain(&["32757-09.html", "32757-10.html"])
            .npcs(&[GREYMORE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let greymore = t.spawn(GREYMORE, 20);
        (t, greymore)
    }

    /// A character of `level` that has finished Secret Mission, with `items`.
    fn finished_secret_mission(t: &mut ScriptTest, id: i32, level: i32, items: Vec<l2_world::ItemData>) {
        let mut data = player_data(id, level, items);
        data.profile.quests = secret_mission_done();
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_level_82_and_secret_mission() {
        let (mut t, greymore) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, greymore).contains("32757-00.htm"));
        finished_secret_mission(&mut t, 2, 81, vec![]);
        assert!(t.quest_page(2, greymore).contains("32757-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_then_the_mark_from_a_solo_kill() {
        let (mut t, greymore) = setup();
        finished_secret_mission(&mut t, 1, 82, vec![]);
        assert!(t.quest_page(1, greymore).contains("32757-01.htm"));
        assert!(t.event(1, NAME, "32757-02.htm").contains("32757-02.htm"));
        assert!(t.event(1, NAME, "32757-03.htm").contains("32757-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // A solo killer gets the Mark of Splendor and cond 3.
        let anays = t.spawn(ANAYS, 60);
        t.kill(1, anays);
        assert_eq!(t.count(1, MARK_OF_SPLENDOR), 1);
        // cond 1 -> 3 skips step 2: Java also records the skipped steps (`__compltdStateFlags`).
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643")
            ]
        );
        assert!(t.quest_page(1, greymore).contains("32757-05.html"));
        assert!(t.event(1, NAME, "32757-06.html").contains("32757-08.html"));

        // The Moirai robe for 420920 adena; the quest ends.
        assert!(t.event(1, NAME, "11").contains("32757-09.html"));
        assert_eq!((t.count(1, 15775), t.count(1, 57)), (1, 420_920));
        assert_eq!(t.count(1, MARK_OF_SPLENDOR), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, greymore).contains("32757-10.html"));
    }

    #[test]
    fn a_party_kill_gives_one_splendor_and_the_rest_darkness() {
        let (mut t, greymore) = setup();
        finished_secret_mission(&mut t, 1, 82, vec![]);
        finished_secret_mission(&mut t, 2, 82, vec![]);
        for player in [1, 2] {
            assert!(t.quest_page(player, greymore).contains("32757-01.htm"));
            t.event(player, NAME, "32757-02.htm");
            t.event(player, NAME, "32757-03.htm");
        }
        t.take_writes();
        t.party(1, 2);

        let anays = t.spawn(ANAYS, 60);
        t.kill(1, anays);
        // Exactly one of the two gets the splendor (cond 3), the other the darkness (cond 2).
        let lucky = if t.count(1, MARK_OF_SPLENDOR) == 1 { 1 } else { 2 };
        let other = 3 - lucky;
        assert_eq!(t.count(lucky, MARK_OF_SPLENDOR), 1);
        assert_eq!(t.count(other, MARK_OF_SPLENDOR), 0);
        assert_eq!(t.count(other, MARK_OF_DARKNESS), 1);
        assert_eq!(t.count(lucky, MARK_OF_DARKNESS), 0);
        let writes = t.take_writes();
        assert_eq!(writes.len(), 3);
        assert!(writes.contains(&rows::of(lucky, rows::update(NAME, "cond", "3"))));
        assert!(writes.contains(&rows::of(
            lucky,
            rows::insert(NAME, "__compltdStateFlags", "-2147483643")
        )));
        assert!(writes.contains(&rows::of(other, rows::update(NAME, "cond", "2"))));

        // The darkness holder gets the darkness page at cond 2 and no reward.
        assert!(t.quest_page(other, greymore).contains("32757-05.html"));
        assert!(t.event(other, NAME, "32757-06.html").contains("32757-07.html"));
        assert_eq!(t.count(other, 15775), 0);
        assert!(t.quest_page(lucky, greymore).contains("32757-05.html"));
    }

    #[test]
    fn the_marks_of_darkness_come_back_with_the_reward_and_the_24_quirk() {
        let (mut t, greymore) = setup();
        // A character at cond 3 with the Mark of Splendor and two Marks of Darkness.
        let mut data = player_data(
            1,
            82,
            vec![item(100, MARK_OF_SPLENDOR, 1), item(101, MARK_OF_DARKNESS, 2)],
        );
        data.profile.quests = QuestLoad {
            states: vec![
                (SECRET_MISSION.to_owned(), Some("Completed".to_owned())),
                (NAME.to_owned(), Some("Started".to_owned())),
            ],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("3".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, greymore).contains("32757-05.html"));
        assert!(t.event(1, NAME, "32757-06.html").contains("32757-08.html"));

        // "24" gives the same piece as "23" (Java's quirk), 505100 adena.
        assert!(t.event(1, NAME, "24").contains("32757-09.html"));
        assert_eq!((t.count(1, 15787), t.count(1, 15788)), (1, 0));
        assert_eq!(t.count(1, 57), 505_100);
        assert_eq!(t.count(1, MARK_OF_DARKNESS), 0);
        assert_eq!(t.count(1, MARK_OF_SPLENDOR), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
