//! Quest 418: Path of the Artisan (Silvera starts it for the dwarven fighters; the Boogle Ratmen).
// Java: dist/game/data/scripts/quests/Q00418_PathOfTheArtisan/Q00418_PathOfTheArtisan.java
//
// Pattern: a class-restricted quest (`player_class_id`), kill counts of two monsters with a drop
// chance and a limit, a memo state per branch of the story (`memoState`), and a reward that is
// given by the last NPC of the branch (`exitQuest(false, true)`, one-time).

use l2_util::ObjectId;
use l2_world::script::{QuestSound, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BLACKSMITH_SILVERA: i32 = 30527;
const BLACKSMITH_PINTER: i32 = 30298;
const BLACKSMITH_KLUTO: i32 = 30317;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const WAREHOUSE_KEEPER_RYDEL: i32 = 31956;
const MINERAL_TRADER_HITCHI: i32 = 31963;
const RAILROAD_WORKER_OBI: i32 = 32052;

// Items
const SILVERYS_RING: i32 = 1632;
const PASS_1ST_CERTIFICATE: i32 = 1633;
const PASS_2ND_CERTIFICATE: i32 = 1634;
const BOOGLE_RATMAN_TOOTH: i32 = 1636;
const BOOGLE_RATMAN_LEADERS_TOOTH: i32 = 1637;
const KLUTOS_LETTER: i32 = 1638;
const FOOTPRINT_OF_THIEF: i32 = 1639;
const STOLEN_SECRET_BOX: i32 = 1640;
const SECRET_BOX: i32 = 1641;

// Reward
const FINAL_PASS_CERTIFICATE: i32 = 1635;

// Monsters
const VUKU_ORC_FIGHTER: i32 = 20017;
const BOOGLE_RATMAN: i32 = 20389;
const BOOGLE_RATMAN_LEADER: i32 = 20390;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00418_PathOfTheArtisan`.
pub struct Q00418PathOfTheArtisan;

/// The end of the quest for one of the branches: the adena, the certificate, the exp and sp by
/// level (`>= 20`, `19`, below), then `exitQuest(false, true)` and the social action 3.
fn finish(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::script::Qs,
    player: ObjectId,
    adena: i64,
    exp_sp: [(i64, i32); 3],
) {
    ctx.give_adena(player, adena, true);
    ctx.give_items(player, FINAL_PASS_CERTIFICATE, 1);
    let level = ctx.player_level(player);
    let (exp, sp) = if level >= 20 {
        exp_sp[0]
    } else if level == 19 {
        exp_sp[1]
    } else {
        exp_sp[2]
    };
    ctx.add_exp_and_sp(player, exp, sp);
    qs.exit_quest(ctx, false, true);
    ctx.send_social_action(player, 3);
}

/// The reward of the Dwarven Artisan's path at the Kluto's (`30317-10`).
const KLUTO_REWARD: (i64, [(i64, i32); 3]) =
    (163_800, [(320_534, 32_452), (456_128, 30_150), (591_724, 36_848)]);
/// The reward of the other ends of the path.
const OTHER_REWARD: (i64, [(i64, i32); 3]) =
    (81_900, [(160_267, 11_726), (228_064, 15_075), (295_862, 18_424)]);

impl Script for Q00418PathOfTheArtisan {
    fn name(&self) -> &'static str {
        "Q00418_PathOfTheArtisan"
    }

    fn path(&self) -> &'static str {
        "quests/Q00418_PathOfTheArtisan"
    }

    fn id(&self) -> i32 {
        418
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACKSMITH_SILVERA]);
        s.add_talk_id(&[
            BLACKSMITH_SILVERA,
            BLACKSMITH_PINTER,
            BLACKSMITH_KLUTO,
            IRON_GATES_LOCKIRIN,
            WAREHOUSE_KEEPER_RYDEL,
            MINERAL_TRADER_HITCHI,
            RAILROAD_WORKER_OBI,
        ]);
        s.add_kill_id(&[VUKU_ORC_FIGHTER, BOOGLE_RATMAN, BOOGLE_RATMAN_LEADER]);
        s.register_quest_items(&[
            SILVERYS_RING,
            PASS_1ST_CERTIFICATE,
            PASS_2ND_CERTIFICATE,
            BOOGLE_RATMAN_TOOTH,
            BOOGLE_RATMAN_LEADERS_TOOTH,
            KLUTOS_LETTER,
            FOOTPRINT_OF_THIEF,
            STOLEN_SECRET_BOX,
            SECRET_BOX,
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
        let memo = qs.get_memo_state(ctx);

        match event {
            "ACCEPT" => {
                let class = ctx.player_class_id(player);
                let page = if class == player_class::DWARVEN_FIGHTER {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, FINAL_PASS_CERTIFICATE) {
                            "30527-04.htm"
                        } else {
                            "30527-05.htm"
                        }
                    } else {
                        "30527-03.htm"
                    }
                } else if class == player_class::ARTISAN {
                    "30527-02a.htm"
                } else {
                    "30527-02.htm"
                };
                Some(page.to_owned())
            }
            "30527-06.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, SILVERYS_RING, 1);
                Some(event.to_owned())
            }
            "30527-08b.html" => {
                ctx.take_items(player, SILVERYS_RING, 1);
                ctx.take_items(player, BOOGLE_RATMAN_TOOTH, -1);
                ctx.take_items(player, BOOGLE_RATMAN_LEADERS_TOOTH, -1);
                ctx.give_items(player, PASS_1ST_CERTIFICATE, 1);
                qs.set_cond(ctx, 3, true);
                Some(event.to_owned())
            }
            "30527-08c.html" => {
                ctx.take_items(player, SILVERYS_RING, 1);
                ctx.take_items(player, BOOGLE_RATMAN_TOOTH, -1);
                ctx.take_items(player, BOOGLE_RATMAN_LEADERS_TOOTH, -1);
                qs.set_memo_state(ctx, 10);
                qs.set_cond(ctx, 8, true);
                Some(event.to_owned())
            }
            "30298-02.html" | "30317-02.html" | "30317-03.html" | "30317-05.html" | "30317-06.html"
            | "30317-11.html" | "30531-02.html" | "30531-03.html" | "30531-04.html" | "31956-02.html"
            | "31956-03.html" | "32052-02.html" | "32052-03.html" | "32052-04.html" | "32052-05.html"
            | "32052-06.html" | "32052-10.html" | "32052-11.html" | "32052-12.html" => Some(event.to_owned()),
            "30298-03.html" => {
                if ctx.has_quest_items(player, KLUTOS_LETTER) {
                    ctx.take_items(player, KLUTOS_LETTER, 1);
                    ctx.give_items(player, FOOTPRINT_OF_THIEF, 1);
                    qs.set_cond(ctx, 5, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30298-06.html" => {
                if ctx.has_all_quest_items(player, &[FOOTPRINT_OF_THIEF, STOLEN_SECRET_BOX]) {
                    ctx.give_items(player, PASS_2ND_CERTIFICATE, 1);
                    ctx.take_items(player, FOOTPRINT_OF_THIEF, 1);
                    ctx.take_items(player, STOLEN_SECRET_BOX, 1);
                    ctx.give_items(player, SECRET_BOX, 1);
                    qs.set_cond(ctx, 7, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30317-04.html" => {
                ctx.give_items(player, KLUTOS_LETTER, 1);
                qs.set_cond(ctx, 4, true);
                Some(event.to_owned())
            }
            "30317-07.html" => {
                ctx.give_items(player, KLUTOS_LETTER, 1);
                qs.set_cond(ctx, 4, false);
                Some(event.to_owned())
            }
            "30317-10.html" => {
                if ctx.has_all_quest_items(player, &[PASS_2ND_CERTIFICATE, SECRET_BOX]) {
                    finish(ctx, qs, player, KLUTO_REWARD.0, KLUTO_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30317-12.html" => {
                if ctx.has_all_quest_items(player, &[PASS_2ND_CERTIFICATE, SECRET_BOX]) {
                    finish(ctx, qs, player, OTHER_REWARD.0, OTHER_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30531-05.html" => {
                if qs.is_memo_state(ctx, 101) {
                    finish(ctx, qs, player, OTHER_REWARD.0, OTHER_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31956-04.html" => {
                if qs.is_memo_state(ctx, 201) {
                    finish(ctx, qs, player, OTHER_REWARD.0, OTHER_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-02.html" | "31963-06.html" => {
                if memo == 100 {
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-03.html" => {
                if memo == 100 {
                    qs.set_memo_state(ctx, 101);
                    qs.set_cond(ctx, 10, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-05.html" => {
                if memo == 100 {
                    qs.set_memo_state(ctx, 102);
                    qs.set_cond(ctx, 11, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-07.html" => {
                if memo == 100 {
                    qs.set_memo_state(ctx, 201);
                    qs.set_cond(ctx, 12, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-09.html" => {
                if memo == 100 {
                    qs.set_memo_state(ctx, 202);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "31963-10.html" => {
                if memo == 202 {
                    finish(ctx, qs, player, OTHER_REWARD.0, OTHER_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "32052-07.html" => {
                if memo == 10 {
                    qs.set_memo_state(ctx, 100);
                    qs.set_cond(ctx, 9, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "32052-13.html" => {
                if qs.is_memo_state(ctx, 102) {
                    finish(ctx, qs, player, OTHER_REWARD.0, OTHER_REWARD.1);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        match npc.id {
            VUKU_ORC_FIGHTER => {
                if ctx.has_quest_items(killer, FOOTPRINT_OF_THIEF)
                    && !ctx.has_quest_items(killer, STOLEN_SECRET_BOX)
                    && ctx.get_random(10) < 2
                {
                    ctx.give_items(killer, STOLEN_SECRET_BOX, 1);
                    qs.set_cond(ctx, 6, true);
                }
            }
            BOOGLE_RATMAN => {
                if ctx.has_quest_items(killer, SILVERYS_RING)
                    && ctx.get_quest_items_count(killer, BOOGLE_RATMAN_TOOTH) < 10
                    && ctx.get_random(10) < 7
                {
                    if ctx.get_quest_items_count(killer, BOOGLE_RATMAN_TOOTH) == 9 {
                        ctx.give_items(killer, BOOGLE_RATMAN_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.get_quest_items_count(killer, BOOGLE_RATMAN_LEADERS_TOOTH) >= 2 {
                            qs.set_cond(ctx, 2, false);
                        }
                    } else {
                        ctx.give_items(killer, BOOGLE_RATMAN_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            BOOGLE_RATMAN_LEADER => {
                if ctx.has_quest_items(killer, SILVERYS_RING)
                    && ctx.get_quest_items_count(killer, BOOGLE_RATMAN_LEADERS_TOOTH) < 2
                {
                    if ctx.get_random(10) < 5 {
                        if ctx.get_quest_items_count(killer, BOOGLE_RATMAN_LEADERS_TOOTH) == 1 {
                            ctx.give_items(killer, BOOGLE_RATMAN_LEADERS_TOOTH, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, BOOGLE_RATMAN_TOOTH) >= 10 {
                                qs.set_cond(ctx, 2, false);
                            }
                        }
                    } else {
                        ctx.give_items(killer, BOOGLE_RATMAN_LEADERS_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let memo = qs.get_memo_state(ctx);

        if qs.is_created(ctx) || qs.is_completed(ctx) {
            if npc.id == BLACKSMITH_SILVERA {
                htmltext = "30527-01.htm".to_owned();
            }
        } else if qs.is_started(ctx) {
            htmltext = started_page(ctx, npc.id, player, memo).unwrap_or(htmltext);
        }
        Some(htmltext)
    }
}

/// The page an NPC of the path shows to a started character (Java's `onTalk` switch); `None`
/// keeps the no-quest page.
fn started_page(ctx: &mut ScriptCtx<'_>, npc: i32, player: ObjectId, memo: i32) -> Option<String> {
    let page = match npc {
        BLACKSMITH_SILVERA => {
            let teeth = ctx.get_quest_items_count(player, BOOGLE_RATMAN_TOOTH);
            let leaders = ctx.get_quest_items_count(player, BOOGLE_RATMAN_LEADERS_TOOTH);
            if ctx.has_quest_items(player, SILVERYS_RING) && teeth + leaders < 12 {
                "30527-07.html"
            } else if ctx.has_quest_items(player, SILVERYS_RING) && teeth >= 10 && leaders >= 2 {
                "30527-08a.html"
            } else if ctx.has_quest_items(player, PASS_1ST_CERTIFICATE) {
                "30527-09.html"
            } else if !ctx.has_quest_items(player, PASS_1ST_CERTIFICATE) && memo == 10 {
                "30527-09a.html"
            } else {
                return None;
            }
        }
        BLACKSMITH_PINTER => {
            if ctx.has_all_quest_items(player, &[PASS_1ST_CERTIFICATE, KLUTOS_LETTER]) {
                "30298-01.html"
            } else if ctx.has_all_quest_items(player, &[PASS_1ST_CERTIFICATE, FOOTPRINT_OF_THIEF])
                && !ctx.has_quest_items(player, STOLEN_SECRET_BOX)
            {
                "30298-04.html"
            } else if ctx.has_all_quest_items(
                player,
                &[PASS_1ST_CERTIFICATE, FOOTPRINT_OF_THIEF, STOLEN_SECRET_BOX],
            ) {
                "30298-05.html"
            } else if ctx
                .has_all_quest_items(player, &[PASS_1ST_CERTIFICATE, PASS_2ND_CERTIFICATE, SECRET_BOX])
            {
                "30298-07.html"
            } else {
                return None;
            }
        }
        BLACKSMITH_KLUTO => {
            if ctx.has_quest_items(player, PASS_1ST_CERTIFICATE)
                && !ctx.has_at_least_one_quest_item(
                    player,
                    &[
                        FOOTPRINT_OF_THIEF,
                        KLUTOS_LETTER,
                        PASS_2ND_CERTIFICATE,
                        SECRET_BOX,
                    ],
                )
            {
                "30317-01.html"
            } else if ctx.has_quest_items(player, PASS_1ST_CERTIFICATE)
                && ctx.has_at_least_one_quest_item(player, &[KLUTOS_LETTER, FOOTPRINT_OF_THIEF])
            {
                "30317-08.html"
            } else if ctx
                .has_all_quest_items(player, &[PASS_1ST_CERTIFICATE, PASS_2ND_CERTIFICATE, SECRET_BOX])
            {
                "30317-09.html"
            } else {
                return None;
            }
        }
        IRON_GATES_LOCKIRIN if memo == 101 => "30531-01.html",
        WAREHOUSE_KEEPER_RYDEL if memo == 201 => "31956-01.html",
        MINERAL_TRADER_HITCHI => match memo {
            100 => "31963-01.html",
            101 => "31963-04.html",
            102 => "31963-06a.html",
            201 => "31963-08.html",
            202 => "31963-11.html",
            _ => return None,
        },
        RAILROAD_WORKER_OBI => match memo {
            10 => "32052-01.html",
            100 => "32052-08.html",
            102 => "32052-09.html",
            _ => return None,
        },
        _ => return None,
    };
    Some(page.to_owned())
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00418_PathOfTheArtisan";

    const NPCS: &str = r#"
        <npc id="30527" type="Folk" level="70" name="Silvera"/>
        <npc id="30317" type="Folk" level="70" name="Kluto"/>
        <npc id="20389" type="Monster" level="10" name="Boogle Ratman"/>
        <npc id="20390" type="Monster" level="12" name="Boogle Ratman Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1632" type="EtcItem" name="Silvery's Ring"><set name="is_questitem" val="true"/></item>
        <item id="1633" type="EtcItem" name="1st Certificate"><set name="is_questitem" val="true"/></item>
        <item id="1634" type="EtcItem" name="2nd Certificate"><set name="is_questitem" val="true"/></item>
        <item id="1635" type="EtcItem" name="Final Pass Certificate"/>
        <item id="1636" type="EtcItem" name="Boogle Ratman Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1637" type="EtcItem" name="Boogle Ratman Leader's Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1641" type="EtcItem" name="Secret Box"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00418PathOfTheArtisan);
        let pages = Pages::of(&Q00418PathOfTheArtisan)
            .page("30527-01.htm", &["ACCEPT"])
            .plain(&[
                "30527-03.htm",
                "30527-04.htm",
                "30527-05.htm",
                "30527-02.htm",
                "30527-02a.htm",
            ])
            .page("30527-05.htm", &["30527-06.htm"])
            .plain(&["30527-06.htm", "30527-07.html", "30527-09.html"])
            .page("30527-08a.html", &["30527-08b.html", "30527-08c.html"])
            .plain(&["30527-08b.html", "30527-08c.html"])
            .plain(&["30317-01.html", "30317-08.html", "30317-10.html"])
            .page("30317-09.html", &["30317-10.html", "30317-11.html"])
            .npcs(&[BLACKSMITH_SILVERA, BLACKSMITH_KLUTO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let silvera = t.spawn(BLACKSMITH_SILVERA, 20);
        (t, silvera)
    }

    /// A dwarven fighter (class 53) of `level`, the one the quest is for.
    fn enter_dwarf(t: &mut ScriptTest, level: i32) {
        t.enter_as(
            1,
            level,
            l2_world::script::race::DWARF,
            player_class::DWARVEN_FIGHTER,
            vec![],
        );
    }

    #[test]
    fn a_dwarven_fighter_below_level_18_is_sent_back() {
        let (mut t, silvera) = setup();
        enter_dwarf(&mut t, 17);
        assert!(t.quest_page(1, silvera).contains("30527-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30527-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_ring_then_the_teeth_then_the_first_certificate() {
        let (mut t, silvera) = setup();
        enter_dwarf(&mut t, 18);
        assert!(t.quest_page(1, silvera).contains("30527-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30527-05.htm"));
        assert!(t.event(1, NAME, "30527-06.htm").contains("30527-06.htm"));
        assert_eq!(t.count(1, SILVERYS_RING), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Ten teeth and two leaders' teeth (a kill in range each): the last one sets cond 2.
        for _ in 0..1_000 {
            if t.count(1, BOOGLE_RATMAN_TOOTH) >= 10 && t.count(1, BOOGLE_RATMAN_LEADERS_TOOTH) >= 2 {
                break;
            }
            let ratman = t.spawn(BOOGLE_RATMAN, 60);
            t.kill(1, ratman);
            let leader = t.spawn(BOOGLE_RATMAN_LEADER, 60);
            t.kill(1, leader);
        }
        assert_eq!(t.count(1, BOOGLE_RATMAN_TOOTH), 10);
        assert_eq!(t.count(1, BOOGLE_RATMAN_LEADERS_TOOTH), 2);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));

        // Silvera gives the first certificate and takes the ring and the teeth.
        assert!(t.quest_page(1, silvera).contains("30527-08a.html"));
        assert!(t.event(1, NAME, "30527-08b.html").contains("30527-08b.html"));
        assert_eq!(t.count(1, PASS_1ST_CERTIFICATE), 1);
        assert_eq!(t.count(1, SILVERYS_RING), 0);
        assert_eq!(t.count(1, BOOGLE_RATMAN_TOOTH), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));
    }

    #[test]
    fn the_second_certificate_and_the_secret_box_end_the_quest_with_the_reward() {
        // A started character with both certificates and the secret box: Kluto pays (level 18,
        // so the third exp row).
        let (mut t, _silvera) = setup();
        let mut data = player_data(
            1,
            18,
            vec![
                item(901, PASS_1ST_CERTIFICATE, 1),
                item(902, PASS_2ND_CERTIFICATE, 1),
                item(903, SECRET_BOX, 1),
            ],
        );
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        let kluto = t.spawn(BLACKSMITH_KLUTO, 40);
        assert!(t.quest_page(1, kluto).contains("30317-09.html"));
        t.take_writes();
        let adena = t.count(1, 57);
        assert!(t.event(1, NAME, "30317-10.html").contains("30317-10.html"));
        assert_eq!(t.count(1, 57) - adena, 163_800);
        assert_eq!(t.count(1, FINAL_PASS_CERTIFICATE), 1);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::delete(NAME, false)), "{writes:?}");
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
