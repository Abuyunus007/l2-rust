//! Quest 270: The One Who Ends Silence (the fake Greymore in the Hot Springs area; divinity and Solina monsters drop tattered monk clothes).
// Java: dist/game/data/scripts/quests/Q00270_TheOneWhoEndsSilence/Q00270_TheOneWhoEndsSilence.java
//
// Pattern: a collect quest with a party drop and a rewards table (`docs/design/scripts.md`,
// "Quest patterns"): the Solina and divinity monsters drop tattered monk clothes for a random party
// member in the range (with a chance, some of them always one); the fake Greymore trades 100, 200,
// 300, 400 or 500 clothes for one or more of the divinity and scroll rewards, and the player can
// leave the quest with `exitQuest`. The quest needs Q10288_SecretMission completed, which this
// server reads through `player_quest_state`.
//
// NOTE(java-quirk): the drop chances of the Solina and divinity monsters are the item ids of the
// Java table (57, 55, 59, 698, 735, 903, 811, 884, 893, 953 out of a thousand), exactly as Java
// passes them to `giveItem`; the rewards of the scroll and the divinity items are Java's too.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const FAKE_GREYMORE: i32 = 32757;

// Monsters
const SEEKER_SOLINA: i32 = 22790;
const SAVIOR_SOLINA: i32 = 22791;
const ASCETIC_SOLINA: i32 = 22793;
const DIVINITY_JUDGE: i32 = 22794;
const DIVINITY_MANAGER: i32 = 22795;
const DIVINITY_SUPERVISOR: i32 = 22796;
const DIVINITY_WORSHIPPER: i32 = 22797;
const DIVINITY_PROTECTOR: i32 = 22798;
const DIVINITY_FIGHTER: i32 = 22799;
const DIVINITY_MAGUS: i32 = 22800;

// Items
const TATTERED_MONK_CLOTHES: i32 = 15526;

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q10288_SecretMission.class.getSimpleName()`: the quest that must be completed.
const SECRET_MISSION: &str = "Q10288_SecretMission";

/// Java `rewardScroll(player, type)`.
fn reward_scroll(ctx: &mut ScriptCtx<'_>, player: ObjectId, kind: i32) {
    let scroll_id = match kind {
        1 => {
            let chance = ctx.get_random(100);
            if chance < 1 {
                5593
            } else if chance < 28 {
                5594
            } else if chance < 61 {
                5595
            } else {
                9898
            }
        }
        2 => {
            let chance = ctx.get_random(100);
            if chance < 20 {
                5593
            } else if chance < 40 {
                5594
            } else if chance < 70 {
                5595
            } else {
                9898
            }
        }
        _ => {
            let chance = ctx.get_random(1000);
            if chance < 242 {
                5593
            } else if chance < 486 {
                5594
            } else if chance < 742 {
                5595
            } else {
                9898
            }
        }
    };
    ctx.give_items(player, scroll_id, 1);
}

/// Java `giveItem(player, npc, chance, atLeastOne)`: one tattered monk clothes for a random party
/// member in range, with `chance` out of a thousand (and always one more with `atLeastOne`).
fn give_item(
    ctx: &mut ScriptCtx<'_>,
    member: Option<ObjectId>,
    npc: ScriptNpc,
    chance: i32,
    at_least_one: bool,
) {
    if let Some(player) = member
        && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
    {
        let count = i64::from(ctx.get_random(1000) < chance) + i64::from(at_least_one);
        if count > 0 {
            ctx.give_items(player, TATTERED_MONK_CLOTHES, count);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// A random divinity or Solina item of the rewards: `10373 + random(9)` or `10397 + random(9)`.
fn random_divinity_item(ctx: &mut ScriptCtx<'_>, first: i32) -> i32 {
    first + ctx.get_random(9)
}

/// Java `Q00270_TheOneWhoEndsSilence`.
pub struct Q00270TheOneWhoEndsSilence;

impl Script for Q00270TheOneWhoEndsSilence {
    fn name(&self) -> &'static str {
        "Q00270_TheOneWhoEndsSilence"
    }

    fn path(&self) -> &'static str {
        "quests/Q00270_TheOneWhoEndsSilence"
    }

    fn id(&self) -> i32 {
        270
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FAKE_GREYMORE]);
        s.add_talk_id(&[FAKE_GREYMORE]);
        s.add_kill_id(&[
            SEEKER_SOLINA,
            SAVIOR_SOLINA,
            ASCETIC_SOLINA,
            DIVINITY_JUDGE,
            DIVINITY_MANAGER,
            DIVINITY_SUPERVISOR,
            DIVINITY_WORSHIPPER,
            DIVINITY_PROTECTOR,
            DIVINITY_FIGHTER,
            DIVINITY_MAGUS,
        ]);
        s.register_quest_items(&[TATTERED_MONK_CLOTHES]);
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
        let rags = ctx.get_quest_items_count(player, TATTERED_MONK_CLOTHES);
        let secret_mission_done = |ctx: &ScriptCtx<'_>| {
            ctx.player_level(player) >= MIN_LEVEL
                && ctx
                    .player_quest_state(player, SECRET_MISSION)
                    .is_some_and(|qst| qst.is_completed())
        };
        match event {
            "32757-02.htm" => {
                if secret_mission_done(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32757-04.html" => {
                if secret_mission_done(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32757-08.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(
                        if rags == 0 {
                            "32757-06.html"
                        } else if rags < 100 {
                            "32757-07.html"
                        } else {
                            event
                        }
                        .to_owned(),
                    );
                }
            }
            "rags100" => {
                if rags >= 100 {
                    if ctx.get_random(10) < 5 {
                        if ctx.get_random(1000) < 438 {
                            let item = random_divinity_item(ctx, 10373);
                            ctx.give_items(player, item, 1);
                        } else {
                            let item = random_divinity_item(ctx, 10397);
                            ctx.give_items(player, item, 1);
                        }
                    } else {
                        reward_scroll(ctx, player, 1);
                    }
                    ctx.take_items(player, TATTERED_MONK_CLOTHES, 100);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32757-09.html".to_owned());
                } else {
                    htmltext = Some("32757-10.html".to_owned());
                }
            }
            "rags200" => {
                if rags >= 200 {
                    if ctx.get_random(1000) < 549 {
                        let item = random_divinity_item(ctx, 10373);
                        ctx.give_items(player, item, 1);
                    } else {
                        let item = random_divinity_item(ctx, 10397);
                        ctx.give_items(player, item, 1);
                    }
                    reward_scroll(ctx, player, 2);
                    ctx.take_items(player, TATTERED_MONK_CLOTHES, 200);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32757-09.html".to_owned());
                } else {
                    htmltext = Some("32757-10.html".to_owned());
                }
            }
            "rags300" => {
                if rags >= 300 {
                    let item = random_divinity_item(ctx, 10373);
                    ctx.give_items(player, item, 1);
                    let item = random_divinity_item(ctx, 10397);
                    ctx.give_items(player, item, 1);
                    reward_scroll(ctx, player, 3);
                    ctx.take_items(player, TATTERED_MONK_CLOTHES, 300);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32757-09.html".to_owned());
                } else {
                    htmltext = Some("32757-10.html".to_owned());
                }
            }
            "rags400" => {
                if rags >= 400 {
                    let item = random_divinity_item(ctx, 10373);
                    ctx.give_items(player, item, 1);
                    let item = random_divinity_item(ctx, 10397);
                    ctx.give_items(player, item, 1);
                    reward_scroll(ctx, player, 3);
                    if ctx.get_random(10) < 5 {
                        if ctx.get_random(1000) < 438 {
                            let item = random_divinity_item(ctx, 10373);
                            ctx.give_items(player, item, 1);
                        } else {
                            let item = random_divinity_item(ctx, 10397);
                            ctx.give_items(player, item, 1);
                        }
                    } else {
                        reward_scroll(ctx, player, 1);
                    }
                    ctx.take_items(player, TATTERED_MONK_CLOTHES, 400);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32757-09.html".to_owned());
                } else {
                    htmltext = Some("32757-10.html".to_owned());
                }
            }
            "rags500" => {
                if rags >= 500 {
                    let item = random_divinity_item(ctx, 10373);
                    ctx.give_items(player, item, 1);
                    let item = random_divinity_item(ctx, 10397);
                    ctx.give_items(player, item, 1);
                    reward_scroll(ctx, player, 3);
                    if ctx.get_random(1000) < 549 {
                        let item = random_divinity_item(ctx, 10373);
                        ctx.give_items(player, item, 1);
                    } else {
                        let item = random_divinity_item(ctx, 10397);
                        ctx.give_items(player, item, 1);
                    }
                    reward_scroll(ctx, player, 2);
                    ctx.take_items(player, TATTERED_MONK_CLOTHES, 500);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32757-09.html".to_owned());
                } else {
                    htmltext = Some("32757-10.html".to_owned());
                }
            }
            "exit1" => {
                if qs.is_cond(ctx, 1) {
                    if rags >= 1 {
                        htmltext = Some("32757-12.html".to_owned());
                    } else {
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some("32757-13.html".to_owned());
                    }
                }
            }
            "exit2" => {
                if qs.is_cond(ctx, 1) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("32757-13.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let member = ctx.get_random_party_member_cond(killer, 1);
        // NOTE(java-quirk): the chance argument is Java's item id (see the file comment).
        match npc.id {
            SEEKER_SOLINA => give_item(ctx, member, npc, 57, false),
            SAVIOR_SOLINA => give_item(ctx, member, npc, 55, false),
            ASCETIC_SOLINA => give_item(ctx, member, npc, 59, false),
            DIVINITY_JUDGE => give_item(ctx, member, npc, 698, false),
            DIVINITY_MANAGER => give_item(ctx, member, npc, 735, false),
            DIVINITY_SUPERVISOR => give_item(ctx, member, npc, 903, false),
            DIVINITY_WORSHIPPER => give_item(ctx, member, npc, 811, false),
            DIVINITY_PROTECTOR => give_item(ctx, member, npc, 884, true),
            DIVINITY_FIGHTER => give_item(ctx, member, npc, 893, true),
            DIVINITY_MAGUS => give_item(ctx, member, npc, 953, true),
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let done = ctx.player_level(player) >= MIN_LEVEL
                    && ctx
                        .player_quest_state(player, SECRET_MISSION)
                        .is_some_and(|qst| qst.is_completed());
                htmltext = if done { "32757-01.htm" } else { "32757-03.html" }.to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "32757-05.html".to_owned();
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
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00270_TheOneWhoEndsSilence";

    /// The stand-in of Q10288: an NPC of its own, and the quest completed when talked to.
    const STAND_IN_NPC: i32 = 32758;

    const NPCS: &str = r#"
        <npc id="32757" type="Folk" level="85" name="Fake Greymore"/>
        <npc id="32758" type="Folk" level="70" name="Stand-in"/>
        <npc id="22798" type="Monster" level="80" name="Divinity Protector"/>"#;

    /// The item templates: the clothes, the scrolls and every divinity and Solina item of the
    /// rewards (`10373..=10381` and `10397..=10405`).
    fn items_xml() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
            <item id="15526" type="EtcItem" name="Tattered Monk Clothes"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
            <item id="5593" type="EtcItem" name="Scroll A"><set name="is_stackable" val="true"/></item>
            <item id="5594" type="EtcItem" name="Scroll B"><set name="is_stackable" val="true"/></item>
            <item id="5595" type="EtcItem" name="Scroll C"><set name="is_stackable" val="true"/></item>
            <item id="9898" type="EtcItem" name="Scroll D"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in (10373..=10381).chain(10397..=10405) {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Divinity {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        xml
    }

    /// Java `Q10288_SecretMission` as far as this quest needs it: its NPC completes it.
    struct SecretMission;

    impl Script for SecretMission {
        fn name(&self) -> &'static str {
            SECRET_MISSION
        }
        fn path(&self) -> &'static str {
            "quests/Q10288_SecretMission"
        }
        fn id(&self) -> i32 {
            10288
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[STAND_IN_NPC]);
            s.add_talk_id(&[STAND_IN_NPC]);
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
        registry.add(SecretMission);
        registry.add(Q00270TheOneWhoEndsSilence);
        let pages = Pages::of(&Q00270TheOneWhoEndsSilence)
            .page("32757-01.htm", &["32757-02.htm"])
            .page("32757-02.htm", &["32757-04.html"])
            .plain(&[
                "32757-03.html",
                "32757-04.html",
                "32757-06.html",
                "32757-07.html",
                "32757-10.html",
                "32757-12.html",
                "32757-13.html",
            ])
            .page("32757-05.html", &["32757-08.html"])
            .page(
                "32757-08.html",
                &["rags100", "rags200", "rags300", "rags400", "rags500"],
            )
            .page("32757-09.html", &["32757-08.html", "32757-04.html", "exit1"])
            .page("32757-12.html", &["32757-08.html", "32757-04.html", "exit2"])
            .npcs(&[FAKE_GREYMORE, STAND_IN_NPC]);
        let xml = items_xml();
        let mut t = ScriptTest::with_pages(registry, NPCS, &xml, pages);
        let greymore = t.spawn(FAKE_GREYMORE, 20);
        let stand_in = t.spawn(STAND_IN_NPC, 40);
        (t, greymore, stand_in)
    }

    #[test]
    fn without_the_secret_mission_or_below_level_82_the_fake_greymore_says_no() {
        let (mut t, greymore, stand_in) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, stand_in);
        t.take_writes();
        assert!(t.quest_page(1, greymore).contains("32757-03.html"));
        t.enter_as(2, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, greymore).contains("32757-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn clothes_buy_rewards_in_steps_and_the_quest_ends_on_the_exit_page() {
        let (mut t, greymore, stand_in) = setup();
        t.enter_as(
            1,
            82,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, TATTERED_MONK_CLOTHES, 500)],
        );
        t.quest_page(1, stand_in);
        t.take_writes();

        assert!(t.quest_page(1, greymore).contains("32757-01.htm"));
        assert!(t.event(1, NAME, "32757-02.htm").contains("32757-02.htm"));
        assert!(t.event(1, NAME, "32757-04.html").contains("32757-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, greymore).contains("32757-05.html"));

        // 100 clothes for one reward; the page goes back to the list.
        assert!(t.event(1, NAME, "32757-08.html").contains("32757-08.html"));
        assert!(t.event(1, NAME, "rags100").contains("32757-09.html"));
        assert_eq!(t.count(1, TATTERED_MONK_CLOTHES), 400);
        assert!(t.take_writes().is_empty());

        // 200 clothes more, then 300 is refused (only 200 are left).
        assert!(t.event(1, NAME, "32757-08.html").contains("32757-08.html"));
        assert!(t.event(1, NAME, "rags200").contains("32757-09.html"));
        assert_eq!(t.count(1, TATTERED_MONK_CLOTHES), 200);
        assert!(t.event(1, NAME, "32757-08.html").contains("32757-08.html"));
        assert!(t.event(1, NAME, "rags300").contains("32757-10.html"));
        assert_eq!(t.count(1, TATTERED_MONK_CLOTHES), 200);

        // Leaving with clothes left goes to the page that asks again; the second exit ends it.
        assert!(t.quest_page(1, greymore).contains("32757-05.html"));
        assert!(t.event(1, NAME, "32757-08.html").contains("32757-08.html"));
        assert!(t.event(1, NAME, "rags100").contains("32757-09.html"));
        assert!(t.event(1, NAME, "exit1").contains("32757-12.html"));
        assert!(t.event(1, NAME, "exit2").contains("32757-13.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_divinity_protector_always_gives_at_least_one_clothes() {
        let (mut t, greymore, stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, stand_in);
        t.take_writes();
        assert!(t.quest_page(1, greymore).contains("32757-01.htm"));
        assert!(t.event(1, NAME, "32757-02.htm").contains("32757-02.htm"));
        assert!(t.event(1, NAME, "32757-04.html").contains("32757-04.html"));
        t.take_writes();

        // "At least one" with a chance on top: each protector gives one or two clothes.
        for _ in 0..3 {
            let protector = t.spawn(DIVINITY_PROTECTOR, 60);
            t.kill(1, protector);
        }
        let clothes = t.count(1, TATTERED_MONK_CLOTHES);
        assert!((3..=6).contains(&clothes), "{clothes}");
        assert!(t.take_writes().is_empty());
    }
}
