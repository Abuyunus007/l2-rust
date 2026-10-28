//! Quest 38: Dragon Fangs (Guard Luis, Iris and Magister Rohmer; the lizardmen of the Talking
//! Island and Gludio hunting grounds).
// Java: dist/game/data/scripts/quests/Q00038_DragonFangs/Q00038_DragonFangs.java
//
// Pattern: a delivery chain with party kills (`docs/design/scripts.md`, "Quest patterns"): the
// sentinels and subleaders give feathers at cond 1, the leaders and shamans give the second
// tooth at cond 6 (`getRandomPartyMemberState`), and the quest ends with a random reward.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const IRIS: i32 = 30034;
const MAGISTER_ROHMER: i32 = 30344;
const GUARD_LUIS: i32 = 30386;

// Monsters
const LIZARDMAN_SENTINEL: i32 = 21100;
const LIZARDMAN_SHAMAN: i32 = 21101;
const LIZARDMAN_LEADER: i32 = 20356;
const LIZARDMAN_SUB_LEADER: i32 = 20357;

// Items: `ItemHolder(id, count)` of Java as an id and a count
const FEATHER: i32 = 7173;
const FEATHER_COUNT: i64 = 100;
const TOTEM_TOOTH_1ST: i32 = 7174;
const TOTEM_TOOTH_2ND: i32 = 7175;
const TOTEM_TOOTH_2ND_COUNT: i64 = 50;
const LETTER_1ST: i32 = 7176;
const LETTER_2ND: i32 = 7177;

// Rewards
const BONE_HELMET: i32 = 45;
const LEATHER_GAUNTLET: i32 = 605;
const ASPIS: i32 = 627;
const BLUE_BUCKSKIN_BOOTS: i32 = 1123;

// Misc
const MIN_LEVEL: i32 = 19;

/// Java `Q00038_DragonFangs`.
pub struct Q00038DragonFangs;

impl Script for Q00038DragonFangs {
    fn name(&self) -> &'static str {
        "Q00038_DragonFangs"
    }

    fn path(&self) -> &'static str {
        "quests/Q00038_DragonFangs"
    }

    fn id(&self) -> i32 {
        38
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_LUIS]);
        s.add_talk_id(&[GUARD_LUIS, IRIS, MAGISTER_ROHMER]);
        s.add_kill_id(&[
            LIZARDMAN_SENTINEL,
            LIZARDMAN_SHAMAN,
            LIZARDMAN_LEADER,
            LIZARDMAN_SUB_LEADER,
        ]);
        s.register_quest_items(&[FEATHER, TOTEM_TOOTH_1ST, TOTEM_TOOTH_2ND, LETTER_1ST, LETTER_2ND]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java sets `htmltext = null` before the state check: no state, no page.
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30386-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30386-06.html" => {
                if qs.is_cond(ctx, 2) {
                    if ctx.get_quest_items_count(player, FEATHER) >= FEATHER_COUNT {
                        qs.set_cond(ctx, 3, true);
                        ctx.take_items(player, FEATHER, FEATHER_COUNT);
                        ctx.give_items(player, TOTEM_TOOTH_1ST, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30386-07.html".to_owned());
                    }
                }
            }
            "30034-02.html" => {
                if qs.is_cond(ctx, 3) {
                    if ctx.has_quest_items(player, TOTEM_TOOTH_1ST) {
                        qs.set_cond(ctx, 4, true);
                        ctx.take_items(player, TOTEM_TOOTH_1ST, 1);
                        ctx.give_items(player, LETTER_1ST, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30034-03.html".to_owned());
                    }
                }
            }
            "30034-06.html" => {
                if qs.is_cond(ctx, 5) {
                    if ctx.has_quest_items(player, LETTER_2ND) {
                        qs.set_cond(ctx, 6, true);
                        ctx.take_items(player, LETTER_2ND, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30034-07.html".to_owned());
                    }
                }
            }
            "30034-10.html" => {
                if qs.is_cond(ctx, 7) {
                    if ctx.get_quest_items_count(player, TOTEM_TOOTH_2ND) >= TOTEM_TOOTH_2ND_COUNT {
                        ctx.add_exp_and_sp(player, 435_117, 23_977);
                        let chance = ctx.get_random(1000);
                        if chance < 250 {
                            ctx.reward_items(player, BONE_HELMET, 1);
                            ctx.give_adena(player, 5200, true);
                        } else if chance < 500 {
                            ctx.reward_items(player, ASPIS, 1);
                            ctx.give_adena(player, 1500, true);
                        } else if chance < 750 {
                            ctx.reward_items(player, BLUE_BUCKSKIN_BOOTS, 1);
                            ctx.give_adena(player, 3200, true);
                        } else if chance < 1000 {
                            ctx.reward_items(player, LEATHER_GAUNTLET, 1);
                            ctx.give_adena(player, 3200, true);
                        }

                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30034-11.html".to_owned());
                    }
                }
            }
            "30344-02.html" => {
                if qs.is_cond(ctx, 4) {
                    if ctx.has_quest_items(player, LETTER_1ST) {
                        qs.set_cond(ctx, 5, true);
                        ctx.take_items(player, LETTER_1ST, 1);
                        ctx.give_items(player, LETTER_2ND, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30344-03.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            IRIS => match qs.get_cond(ctx) {
                3 => htmltext = "30034-01.html".to_owned(),
                4 => htmltext = "30034-04.html".to_owned(),
                5 => htmltext = "30034-05.html".to_owned(),
                6 => htmltext = "30034-09.html".to_owned(),
                7 => {
                    if ctx.get_quest_items_count(talker, TOTEM_TOOTH_2ND) >= TOTEM_TOOTH_2ND_COUNT {
                        htmltext = "30034-08.html".to_owned();
                    }
                }
                _ => {}
            },
            MAGISTER_ROHMER => {
                if qs.is_cond(ctx, 4) {
                    htmltext = "30344-01.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "30344-04.html".to_owned();
                }
            }
            GUARD_LUIS => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30386-01.htm"
                    } else {
                        "30386-02.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30386-05.html".to_owned(),
                        2 => {
                            if ctx.get_quest_items_count(talker, FEATHER) >= FEATHER_COUNT {
                                htmltext = "30386-04.html".to_owned();
                            }
                        }
                        3 => htmltext = "30386-08.html".to_owned(),
                        _ => {}
                    }
                } else if qs.is_completed(ctx) {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            LIZARDMAN_SUB_LEADER | LIZARDMAN_SENTINEL => {
                if let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc))
                    && ctx.give_item_randomly(qs.player(), Some(npc), FEATHER, 1, 1, FEATHER_COUNT, 1.0, true)
                {
                    qs.set_cond(ctx, 2, false);
                }
            }
            LIZARDMAN_LEADER | LIZARDMAN_SHAMAN => {
                if let Some(qs) = ctx.random_party_member_state(killer, 6, 3, Some(npc))
                    && ctx.give_item_randomly(
                        qs.player(),
                        Some(npc),
                        TOTEM_TOOTH_2ND,
                        1,
                        1,
                        TOTEM_TOOTH_2ND_COUNT,
                        0.5,
                        true,
                    )
                {
                    qs.set_cond(ctx, 7, false);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00038_DragonFangs";

    const NPCS: &str = r#"
        <npc id="30034" type="Folk" level="70" name="Iris"/>
        <npc id="30344" type="Folk" level="70" name="Magister Rohmer"/>
        <npc id="30386" type="Folk" level="70" name="Guard Luis"/>
        <npc id="21100" type="Monster" level="25" name="Lizardman Sentinel"/>
        <npc id="20356" type="Monster" level="33" name="Lizardman Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="45" type="Armor" name="Bone Helmet"><set name="bodypart" val="head"/></item>
        <item id="605" type="Armor" name="Leather Gauntlet"><set name="bodypart" val="gloves"/></item>
        <item id="627" type="Armor" name="Aspis"><set name="bodypart" val="lhand"/></item>
        <item id="1123" type="Armor" name="Blue Buckskin Boots"><set name="bodypart" val="feet"/></item>
        <item id="7173" type="EtcItem" name="Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7174" type="EtcItem" name="Totem Tooth 1st"><set name="is_questitem" val="true"/></item>
        <item id="7175" type="EtcItem" name="Totem Tooth 2nd"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7176" type="EtcItem" name="Letter 1st"><set name="is_questitem" val="true"/></item>
        <item id="7177" type="EtcItem" name="Letter 2nd"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        luis: Oid,
        iris: Oid,
        rohmer: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00038DragonFangs);
        let pages = Pages::of(&Q00038DragonFangs)
            .page("30386-01.htm", &["30386-03.htm"])
            .page("30386-04.html", &["30386-06.html"])
            .page("30034-01.html", &["30034-02.html"])
            .page("30034-05.html", &["30034-06.html"])
            .page("30034-08.html", &["30034-10.html"])
            .page("30344-01.html", &["30344-02.html"])
            .plain(&[
                "30386-02.htm",
                "30386-03.htm",
                "30386-05.html",
                "30386-06.html",
                "30386-07.html",
                "30386-08.html",
                "30034-02.html",
                "30034-03.html",
                "30034-04.html",
                "30034-06.html",
                "30034-07.html",
                "30034-09.html",
                "30034-10.html",
                "30034-11.html",
                "30344-02.html",
                "30344-03.html",
                "30344-04.html",
            ])
            .npcs(&[GUARD_LUIS, IRIS, MAGISTER_ROHMER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            luis: t.spawn(GUARD_LUIS, 20),
            iris: t.spawn(IRIS, 40),
            rohmer: t.spawn(MAGISTER_ROHMER, 60),
        };
        (t, npcs)
    }

    /// Sets the player's cond directly (for the branches reached through a long hunt).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, cond, false);
            }
        });
        t.take_writes();
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00038DragonFangs.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills sentinels until the player has 100 feathers; returns the number of kills.
    fn hunt_feathers(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let sentinel = t.spawn(LIZARDMAN_SENTINEL, 80);
            t.kill(1, sentinel);
            if t.count(1, FEATHER) >= FEATHER_COUNT {
                return kills;
            }
        }
        panic!("no 100 feathers after 400 kills");
    }

    /// Kills leaders until the player has the second tooth; returns the number of kills.
    fn hunt_teeth(t: &mut ScriptTest) -> usize {
        for kills in 1..1000 {
            let leader = t.spawn(LIZARDMAN_LEADER, 80);
            t.kill(1, leader);
            if t.count(1, TOTEM_TOOTH_2ND) >= TOTEM_TOOTH_2ND_COUNT {
                return kills;
            }
        }
        panic!("no 50 teeth after 1000 kills");
    }

    #[test]
    fn a_level_18_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 18, vec![]);
        assert!(t.quest_page(1, npcs.luis).contains("30386-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_the_feathers_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, npcs.luis).contains("30386-01.htm"));
        assert!(t.event(1, NAME, "30386-03.htm").contains("30386-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.luis).contains("30386-05.html"));

        // The sentinels give the feathers to the killer; the 100th sets cond 2.
        hunt_feathers(&mut t);
        assert_eq!(t.count(1, FEATHER), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Luis takes the feathers and gives the first tooth: cond 3.
        assert!(t.quest_page(1, npcs.luis).contains("30386-04.html"));
        assert!(t.event(1, NAME, "30386-06.html").contains("30386-06.html"));
        assert_eq!((t.count(1, FEATHER), t.count(1, TOTEM_TOOTH_1ST)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Iris takes the first tooth and gives the first letter: cond 4.
        assert!(t.quest_page(1, npcs.iris).contains("30034-01.html"));
        assert!(t.event(1, NAME, "30034-02.html").contains("30034-02.html"));
        assert_eq!((t.count(1, TOTEM_TOOTH_1ST), t.count(1, LETTER_1ST)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Rohmer takes the first letter and gives the second: cond 5.
        assert!(t.quest_page(1, npcs.rohmer).contains("30344-01.html"));
        assert!(t.event(1, NAME, "30344-02.html").contains("30344-02.html"));
        assert_eq!((t.count(1, LETTER_1ST), t.count(1, LETTER_2ND)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Iris takes the second letter: cond 6. Then the leaders give the second tooth.
        assert!(t.quest_page(1, npcs.iris).contains("30034-05.html"));
        assert!(t.event(1, NAME, "30034-06.html").contains("30034-06.html"));
        assert_eq!(t.count(1, LETTER_2ND), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.iris).contains("30034-09.html"));

        hunt_teeth(&mut t);
        assert_eq!(t.count(1, TOTEM_TOOTH_2ND), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Iris takes the 50 teeth and gives the reward: one of the four, the adena for it.
        assert!(t.quest_page(1, npcs.iris).contains("30034-08.html"));
        assert!(t.event(1, NAME, "30034-10.html").contains("30034-10.html"));
        // `exitQuest` takes the registered quest items (the teeth) away.
        assert_eq!(t.count(1, TOTEM_TOOTH_2ND), 0);
        let rewards = [
            (BONE_HELMET, 5200),
            (ASPIS, 1500),
            (BLUE_BUCKSKIN_BOOTS, 3200),
            (LEATHER_GAUNTLET, 3200),
        ];
        let given: Vec<(i32, i64)> = rewards
            .into_iter()
            .filter(|(id, _)| t.count(1, *id) == 1)
            .collect();
        assert_eq!(given.len(), 1);
        assert_eq!(t.count(1, 57), given[0].1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        // Iris only talks about the teeth (her `switch` has no state check): nothing left to
        // say; Luis has the completed page.
        assert!(t.quest_page(1, npcs.iris).contains("not on a quest"));
        assert!(
            t.quest_page(1, npcs.luis)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn each_step_refuses_without_its_item() {
        let (mut t, npcs) = setup();
        t.enter(1, 19, vec![item(1001, FEATHER, 20)]);
        t.quest_page(1, npcs.luis);
        t.event(1, NAME, "30386-03.htm");
        t.take_writes();

        // Luis at cond 2 with too few feathers: the no-quest page; the event refuses too.
        set_cond(&mut t, 2);
        assert!(t.quest_page(1, npcs.luis).contains("not on a quest"));
        assert_eq!(direct(&mut t, "30386-06.html").as_deref(), Some("30386-07.html"));
        assert_eq!(t.count(1, FEATHER), 20);
        assert!(t.take_writes().is_empty());

        // Iris, Rohmer and the second letter, with nothing in hand.
        set_cond(&mut t, 3);
        assert_eq!(direct(&mut t, "30034-02.html").as_deref(), Some("30034-03.html"));
        set_cond(&mut t, 5);
        assert_eq!(direct(&mut t, "30034-06.html").as_deref(), Some("30034-07.html"));
        set_cond(&mut t, 4);
        assert_eq!(direct(&mut t, "30344-02.html").as_deref(), Some("30344-03.html"));
        set_cond(&mut t, 7);
        assert_eq!(direct(&mut t, "30034-10.html").as_deref(), Some("30034-11.html"));
        assert_eq!(t.count(1, 57), 0);
        assert!(t.take_writes().is_empty());
    }
}
