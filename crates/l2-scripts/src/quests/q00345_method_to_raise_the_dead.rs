//! Quest 345: Method to Raise the Dead (Dorothy in Gludio, Xenovia, Orpheus and the medium jar).
// Java: dist/game/data/scripts/quests/Q00345_MethodToRaiseTheDead/Q00345_MethodToRaiseTheDead.java
//
// Pattern: a kill drop of five bone kinds (a repeat of a kind becomes a useless piece), a
// one-time payment, and a memo state per step. The medium jar rolls the ending (`memoStateEx 1`
// to 3); Dorothy pays for it. Q00351 reads this quest's state (`player.getQuestState`).

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const XENOVIA: i32 = 30912;
const DOROTHY: i32 = 30970;
const ORPHEUS: i32 = 30971;
const MEDIUM_JAR: i32 = 30973;

// Items
const IMPERIAL_DIAMOND: i32 = 3456;
const VICTIMS_ARM_BONE: i32 = 4274;
const VICTIMS_THIGH_BONE: i32 = 4275;
const VICTIMS_SKULL: i32 = 4276;
const VICTIMS_RIB_BONE: i32 = 4277;
const VICTIMS_SPINE: i32 = 4278;
const USELESS_BONE_PIECES: i32 = 4280;
const POWDER_TO_SUMMON_DEAD_SOULS: i32 = 4281;
const BILL_OF_IASON_HEINE: i32 = 4407;

// Misc
const MIN_LEVEL: i32 = 35;

// Monsters
const CROKIAN: i32 = 20789;
const CROKIAN_WARRIOR: i32 = 20791;

/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `Q00345_MethodToRaiseTheDead`.
pub struct Q00345MethodToRaiseTheDead;

/// The victim's bones of a kill: a bone the player has is a useless piece instead.
fn drop_bone(ctx: &mut ScriptCtx<'_>, member: ObjectId, bone: i32) {
    if !ctx.has_quest_items(member, bone) {
        ctx.give_items(member, bone, 1);
    } else {
        ctx.give_items(member, USELESS_BONE_PIECES, 1);
    }
    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
}

impl Script for Q00345MethodToRaiseTheDead {
    fn name(&self) -> &'static str {
        "Q00345_MethodToRaiseTheDead"
    }

    fn path(&self) -> &'static str {
        "quests/Q00345_MethodToRaiseTheDead"
    }

    fn id(&self) -> i32 {
        345
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DOROTHY]);
        s.add_talk_id(&[DOROTHY, ORPHEUS, MEDIUM_JAR, XENOVIA]);
        s.add_kill_id(&[CROKIAN, CROKIAN_WARRIOR]);
        s.register_quest_items(&[
            VICTIMS_ARM_BONE,
            VICTIMS_THIGH_BONE,
            VICTIMS_SKULL,
            VICTIMS_RIB_BONE,
            VICTIMS_SPINE,
            USELESS_BONE_PIECES,
            POWDER_TO_SUMMON_DEAD_SOULS,
        ]);
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
            "30970-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30970-03.html" => {
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30970-07.html" => {
                if ctx.has_all_quest_items(
                    player,
                    &[
                        VICTIMS_ARM_BONE,
                        VICTIMS_THIGH_BONE,
                        VICTIMS_SKULL,
                        VICTIMS_RIB_BONE,
                        VICTIMS_SPINE,
                    ],
                ) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30971-02.html" | "30912-05.html" => {
                htmltext = Some(event.to_owned());
            }
            "30971-03.html" => {
                let useless_bone_pieces_count = ctx.get_quest_items_count(player, USELESS_BONE_PIECES);
                if useless_bone_pieces_count > 0 {
                    ctx.give_adena(player, useless_bone_pieces_count * 104, true);
                    ctx.take_items(player, USELESS_BONE_PIECES, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30973-02.html" => {
                let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                if memo_state_ex == 1 {
                    htmltext = Some(event.to_owned());
                } else if memo_state_ex == 2 {
                    htmltext = Some("30973-04.html".to_owned());
                } else if memo_state_ex == 3 {
                    htmltext = Some("30973-06.html".to_owned());
                }
            }
            "30973-03.html" => {
                if qs.is_memo_state(ctx, 7) && qs.get_memo_state_ex(ctx, 1) == 1 {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30973-05.html" => {
                if qs.is_memo_state(ctx, 7) && qs.get_memo_state_ex(ctx, 1) == 2 {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30973-07.html" => {
                if qs.is_memo_state(ctx, 7) && qs.get_memo_state_ex(ctx, 1) == 3 {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30912-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30912-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    if ctx.adena_of(player) >= 1000 {
                        ctx.give_items(player, POWDER_TO_SUMMON_DEAD_SOULS, 1);
                        ctx.take_items(player, ADENA_ID, 1000);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 3, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30912-04.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        let member = qs.player();
        let random = ctx.get_random(100);
        if random <= 5 {
            drop_bone(ctx, member, VICTIMS_ARM_BONE);
        } else if random <= 11 {
            drop_bone(ctx, member, VICTIMS_THIGH_BONE);
        } else if random <= 17 {
            drop_bone(ctx, member, VICTIMS_SKULL);
        } else if random <= 23 {
            drop_bone(ctx, member, VICTIMS_RIB_BONE);
        } else if random <= 29 {
            drop_bone(ctx, member, VICTIMS_SPINE);
        } else if random <= 60 {
            ctx.give_items(member, USELESS_BONE_PIECES, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30970-01.htm"
            } else {
                "30970-04.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match npc.id {
                DOROTHY => match qs.get_memo_state(ctx) {
                    0 => {
                        htmltext = "30970-03.html".to_owned();
                        qs.set_memo_state(ctx, 1);
                    }
                    1 => {
                        htmltext = if !ctx.has_all_quest_items(
                            player,
                            &[
                                VICTIMS_ARM_BONE,
                                VICTIMS_THIGH_BONE,
                                VICTIMS_SKULL,
                                VICTIMS_RIB_BONE,
                                VICTIMS_SPINE,
                            ],
                        ) {
                            "30970-05.html"
                        } else {
                            "30970-06.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "30970-08.html".to_owned(),
                    3 => htmltext = "30970-09.html".to_owned(),
                    7 => htmltext = "30970-10.html".to_owned(),
                    8 => {
                        let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                        let useless_bone_pieces_count =
                            ctx.get_quest_items_count(player, USELESS_BONE_PIECES);
                        if memo_state_ex == 1 || memo_state_ex == 2 {
                            ctx.give_items(player, BILL_OF_IASON_HEINE, 3);
                            ctx.give_adena(player, 5390 + 70 * useless_bone_pieces_count, true);
                            htmltext = "30970-11.html".to_owned();
                        } else if memo_state_ex == 3 {
                            if ctx.get_random(100) <= 92 {
                                ctx.give_items(player, BILL_OF_IASON_HEINE, 5);
                            } else {
                                ctx.give_items(player, IMPERIAL_DIAMOND, 1);
                            }

                            ctx.give_adena(player, 3040 + 70 * useless_bone_pieces_count, true);
                            htmltext = "30970-12.html".to_owned();
                        }

                        qs.exit_quest(ctx, true, true);
                    }
                    _ => {}
                },
                ORPHEUS => {
                    if ctx.has_quest_items(player, USELESS_BONE_PIECES) {
                        htmltext = "30971-01.html".to_owned();
                    }
                }
                MEDIUM_JAR => match qs.get_memo_state(ctx) {
                    3 => {
                        ctx.take_items_many(
                            player,
                            -1,
                            &[
                                POWDER_TO_SUMMON_DEAD_SOULS,
                                VICTIMS_ARM_BONE,
                                VICTIMS_THIGH_BONE,
                                VICTIMS_SKULL,
                                VICTIMS_RIB_BONE,
                                VICTIMS_SPINE,
                            ],
                        );
                        qs.set_memo_state(ctx, 7);

                        let random = ctx.get_random(100);
                        if random <= 39 {
                            qs.set_memo_state_ex(ctx, 1, 1);
                        } else if random <= 79 {
                            qs.set_memo_state_ex(ctx, 1, 2);
                        } else {
                            qs.set_memo_state_ex(ctx, 1, 3);
                        }

                        htmltext = "30973-01.html".to_owned();
                    }
                    7 => {
                        let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                        if memo_state_ex == 1 {
                            htmltext = "30973-08.html".to_owned();
                        } else if memo_state_ex == 2 {
                            htmltext = "30973-09.html".to_owned();
                        } else if memo_state_ex == 3 {
                            htmltext = "30973-10.html".to_owned();
                        }
                    }
                    8 => htmltext = "30973-11.html".to_owned(),
                    _ => {}
                },
                XENOVIA => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "30912-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 7)
                        || qs.is_memo_state(ctx, 8)
                        || ctx.has_quest_items(player, POWDER_TO_SUMMON_DEAD_SOULS)
                    {
                        htmltext = "30912-06.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00345_MethodToRaiseTheDead";

    const NPCS: &str = r#"
        <npc id="30912" type="Folk" level="50" name="Xenovia"/>
        <npc id="30970" type="Folk" level="50" name="Dorothy"/>
        <npc id="30971" type="Folk" level="50" name="Orpheus"/>
        <npc id="30973" type="Folk" level="50" name="Medium Jar"/>
        <npc id="20789" type="Monster" level="30" name="Crokian"/>
        <npc id="20791" type="Monster" level="30" name="Crokian Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3456" type="EtcItem" name="Imperial Diamond"/>
        <item id="4274" type="EtcItem" name="Victim's Arm Bone"><set name="is_questitem" val="true"/></item>
        <item id="4275" type="EtcItem" name="Victim's Thigh Bone"><set name="is_questitem" val="true"/></item>
        <item id="4276" type="EtcItem" name="Victim's Skull"><set name="is_questitem" val="true"/></item>
        <item id="4277" type="EtcItem" name="Victim's Rib Bone"><set name="is_questitem" val="true"/></item>
        <item id="4278" type="EtcItem" name="Victim's Spine"><set name="is_questitem" val="true"/></item>
        <item id="4280" type="EtcItem" name="Useless Bone Pieces"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4281" type="EtcItem" name="Powder to Summon Dead Souls"><set name="is_questitem" val="true"/></item>
        <item id="4407" type="EtcItem" name="Bill of Iason Heine"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        dorothy: Oid,
        xenovia: Oid,
        orpheus: Oid,
        jar: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00345MethodToRaiseTheDead);
        let pages = Pages::of(&Q00345MethodToRaiseTheDead)
            .page("30970-01.htm", &["30970-02.htm"])
            .page("30970-02.htm", &["30970-03.html"])
            .plain(&["30970-03.html", "30970-04.htm", "30970-05.html"])
            .page("30970-06.html", &["30970-07.html"])
            .plain(&["30970-07.html", "30970-08.html", "30970-09.html", "30970-10.html"])
            .plain(&["30970-11.html", "30970-12.html"])
            .page("30971-01.html", &["30971-02.html"])
            .page("30971-02.html", &["30971-03.html"])
            .plain(&["30971-03.html"])
            .page("30973-01.html", &["30973-02.html"])
            .page("30973-02.html", &["30973-03.html"])
            .plain(&["30973-03.html", "30973-04.html", "30973-06.html"])
            .page("30973-04.html", &["30973-05.html"])
            .plain(&["30973-05.html"])
            .page("30973-06.html", &["30973-07.html"])
            .plain(&["30973-07.html"])
            .page("30973-08.html", &["30973-03.html"])
            .page("30973-09.html", &["30973-05.html"])
            .page("30973-10.html", &["30973-07.html"])
            .plain(&["30973-11.html"])
            .page("30912-01.html", &["30912-02.html"])
            .page("30912-02.html", &["30912-03.html", "30912-05.html"])
            .plain(&["30912-03.html", "30912-04.html", "30912-05.html", "30912-06.html"])
            .npcs(&[DOROTHY, ORPHEUS, MEDIUM_JAR, XENOVIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            dorothy: t.spawn(DOROTHY, 20),
            xenovia: t.spawn(XENOVIA, 40),
            orpheus: t.spawn(ORPHEUS, 60),
            jar: t.spawn(MEDIUM_JAR, 80),
        };
        (t, npcs)
    }

    /// Dorothy's accept and the memo state 1.
    fn start(t: &mut ScriptTest, dorothy: Oid) {
        assert!(t.quest_page(1, dorothy).contains("30970-01.htm"));
        assert!(t.event(1, NAME, "30970-02.htm").contains("30970-02.htm"));
        assert!(t.event(1, NAME, "30970-03.html").contains("30970-03.html"));
    }

    /// Kills the Crokians until the player has the five bones.
    fn collect_bones(t: &mut ScriptTest) {
        let ids = [CROKIAN, CROKIAN_WARRIOR];
        for kill in 0..800 {
            let all = [
                VICTIMS_ARM_BONE,
                VICTIMS_THIGH_BONE,
                VICTIMS_SKULL,
                VICTIMS_RIB_BONE,
                VICTIMS_SPINE,
            ]
            .iter()
            .all(|bone| t.count(1, *bone) >= 1);
            if all {
                return;
            }
            let crokian = t.spawn(ids[kill % ids.len()], 60);
            t.kill(1, crokian);
        }
        panic!("no five bones after 800 kills");
    }

    /// The memo state ex 1 the medium jar rolled, read from its write.
    fn rolled_ex(writes: &[QuestWrite]) -> i32 {
        writes
            .iter()
            .find_map(|w| match w {
                QuestWrite::InsertVar { var, value, .. } if var == "memoStateEx1" => {
                    Some(value.parse::<i32>().unwrap())
                }
                _ => None,
            })
            .expect("the jar rolls a memoStateEx1")
    }

    #[test]
    fn a_character_below_level_thirty_five_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 34, vec![]);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn dorothy_starts_the_quest_with_memo_state_one() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-01.htm"));
        assert!(t.event(1, NAME, "30970-02.htm").contains("30970-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.event(1, NAME, "30970-03.html").contains("30970-03.html"));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "memoState", "1")]);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-05.html"));
    }

    #[test]
    fn the_five_bones_are_asked_for_by_dorothy_then_the_quest_goes_on() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        start(&mut t, npcs.dorothy);
        t.take_writes();
        collect_bones(&mut t);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-06.html"));
        assert!(t.event(1, NAME, "30970-07.html").contains("30970-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
            ]
        );
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-08.html"));
    }

    #[test]
    fn useless_pieces_are_sold_to_orpheus_for_104_each() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![item(100, USELESS_BONE_PIECES, 3)]);
        start(&mut t, npcs.dorothy);
        t.take_writes();
        assert!(t.quest_page(1, npcs.orpheus).contains("30971-01.html"));
        assert!(t.event(1, NAME, "30971-02.html").contains("30971-02.html"));
        assert!(t.event(1, NAME, "30971-03.html").contains("30971-03.html"));
        assert_eq!(t.count(1, 57), 3 * 104);
        assert_eq!(t.count(1, USELESS_BONE_PIECES), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn xenovia_wants_a_thousand_adena_for_the_powder() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![item(100, ADENA_ID, 999)]);
        start(&mut t, npcs.dorothy);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 2);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.xenovia).contains("30912-01.html"));
        assert!(t.event(1, NAME, "30912-02.html").contains("30912-02.html"));
        assert!(t.event(1, NAME, "30912-03.html").contains("30912-04.html"));
        assert_eq!(t.count(1, POWDER_TO_SUMMON_DEAD_SOULS), 0);
        assert_eq!(t.count(1, ADENA_ID), 999);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_the_bones_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![item(100, ADENA_ID, 1000)]);
        start(&mut t, npcs.dorothy);
        collect_bones(&mut t);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-06.html"));
        assert!(t.event(1, NAME, "30970-07.html").contains("30970-07.html"));
        t.take_writes();

        // Xenovia: the powder for 1000 adena, memo 3 and cond 3.
        assert!(t.quest_page(1, npcs.xenovia).contains("30912-01.html"));
        assert!(t.event(1, NAME, "30912-02.html").contains("30912-02.html"));
        assert!(t.event(1, NAME, "30912-03.html").contains("30912-03.html"));
        assert_eq!(t.count(1, POWDER_TO_SUMMON_DEAD_SOULS), 1);
        assert_eq!(t.count(1, ADENA_ID), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3"),
            ]
        );

        // The medium jar: the bones and the powder go, the ending is rolled (memo 7).
        assert!(t.quest_page(1, npcs.jar).contains("30973-01.html"));
        assert_eq!(t.count(1, POWDER_TO_SUMMON_DEAD_SOULS), 0);
        assert_eq!(t.count(1, VICTIMS_ARM_BONE), 0);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::update(NAME, "memoState", "7"));
        let ex = rolled_ex(&writes);
        assert!((1..=3).contains(&ex));

        // The jar's page of the ending gives the link to the decision.
        let (page, event) = match ex {
            1 => ("30973-08.html", "30973-03.html"),
            2 => ("30973-09.html", "30973-05.html"),
            _ => ("30973-10.html", "30973-07.html"),
        };
        assert!(t.quest_page(1, npcs.jar).contains(page));
        assert!(t.event(1, NAME, event).contains(event));
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "memoState", "8")));
        let cond = if ex == 3 { "7" } else { "6" };
        assert!(writes.contains(&rows::update(NAME, "cond", cond)), "{writes:?}");

        // Dorothy pays the reward and ends the quest (repeatable).
        let useless = t.count(1, USELESS_BONE_PIECES);
        let page = if ex == 3 { "30970-12.html" } else { "30970-11.html" };
        assert!(t.quest_page(1, npcs.dorothy).contains(page));
        let bills = t.count(1, BILL_OF_IASON_HEINE);
        if ex == 3 {
            assert!(bills == 5 || t.count(1, IMPERIAL_DIAMOND) == 1);
            assert_eq!(t.count(1, 57), 3040 + 70 * useless);
        } else {
            assert_eq!(bills, 3);
            assert_eq!(t.count(1, 57), 5390 + 70 * useless);
        }
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_third_ending_pays_the_diamond_or_the_bills_of_the_ending() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        start(&mut t, npcs.dorothy);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 8);
            qs.set_memo_state_ex(ctx, 1, 3);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-12.html"));
        assert!(t.count(1, BILL_OF_IASON_HEINE) == 5 || t.count(1, IMPERIAL_DIAMOND) == 1);
        assert_eq!(t.count(1, 57), 3040);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_out_of_range_gives_no_bones() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        start(&mut t, npcs.dorothy);
        let far = t
            .world
            .spawn_npc_at(
                CROKIAN,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, VICTIMS_ARM_BONE), 0);
        assert_eq!(t.count(1, USELESS_BONE_PIECES), 0);
    }
}
