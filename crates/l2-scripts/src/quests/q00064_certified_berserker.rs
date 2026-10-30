//! Quest 64: Certified Berserker (Master Orkurus, Entiens, Tenain, Caravaner Gort, Harkilgamed).
// Java: dist/game/data/scripts/quests/Q00064_CertifiedBerserker/Q00064_CertifiedBerserker.java
//
// Pattern: a kill and delivery chain with the memo state as the step counter
// (`docs/design/scripts.md`, "Quest patterns"), sibling of `q00062_path_of_the_trooper.rs`; the
// diamond reward is paid once per character (`2ND_CLASS_DIAMOND_REWARD`, like Q00222).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_ENTIENS: i32 = 32200;
const MASTER_ORKURUS: i32 = 32207;
const MASTER_TENAIN: i32 = 32215;
const CARAVANER_GORT: i32 = 32252;
const HARKILGAMED: i32 = 32253;

// Items
const BREKA_ORC_HEAD: i32 = 9754;
const MESSAGE_PLATE: i32 = 9755;
const REPORT_EAST: i32 = 9756;
const REPORT_NORTH: i32 = 9757;
const HARKILGAMEDS_LETTER: i32 = 9758;
const TENAINS_RECOMMENDATION: i32 = 9759;

// Reward
const DIMENSIONAL_DIAMOND: i32 = 7562;
const ORKURUS_RECOMMENDATION: i32 = 9760;

// Monster
const DEAD_SEEKER: i32 = 20202;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC: i32 = 20267;
const BREKA_ORC_ARCHER: i32 = 20268;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const BREKA_ORC_WARRIOR: i32 = 20271;
const ROAD_SCAVENGER: i32 = 20551;

// Quest Monster
const DIVINE_EMISSARY: i32 = 27323;

// Misc
const MIN_LEVEL: i32 = 39;
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00064_CertifiedBerserker`.
pub struct Q00064CertifiedBerserker;

impl Script for Q00064CertifiedBerserker {
    fn name(&self) -> &'static str {
        "Q00064_CertifiedBerserker"
    }

    fn path(&self) -> &'static str {
        "quests/Q00064_CertifiedBerserker"
    }

    fn id(&self) -> i32 {
        64
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_ORKURUS]);
        s.add_talk_id(&[
            MASTER_ORKURUS,
            MASTER_ENTIENS,
            MASTER_TENAIN,
            CARAVANER_GORT,
            HARKILGAMED,
        ]);
        s.add_kill_id(&[
            DEAD_SEEKER,
            MARSH_STAKATO_DRONE,
            BREKA_ORC,
            BREKA_ORC_ARCHER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            BREKA_ORC_WARRIOR,
            ROAD_SCAVENGER,
            DIVINE_EMISSARY,
        ]);
        s.register_quest_items(&[
            BREKA_ORC_HEAD,
            MESSAGE_PLATE,
            REPORT_EAST,
            REPORT_NORTH,
            HARKILGAMEDS_LETTER,
            TENAINS_RECOMMENDATION,
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 48);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("32207-06.htm".to_owned());
                    } else {
                        htmltext = Some("32207-06a.htm".to_owned());
                    }
                }
            }
            "32207-10.html" => {
                if qs.is_memo_state(ctx, 11) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32207-11.html" => {
                if qs.is_memo_state(ctx, 11) {
                    ctx.give_adena(player, 63_104, true);
                    ctx.give_items(player, ORKURUS_RECOMMENDATION, 1);
                    ctx.add_exp_and_sp(player, 349_006, 23_948);
                    qs.exit_quest(ctx, false, true);
                    // `SocialAction(3)` goes to this player only.
                    ctx.send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "32215-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32215-07.html" | "32215-08.html" | "32215-09.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32215-10.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.take_items(player, MESSAGE_PLATE, 1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32215-15.html" => {
                if qs.is_memo_state(ctx, 10) {
                    ctx.take_items(player, HARKILGAMEDS_LETTER, 1);
                    ctx.give_items(player, TENAINS_RECOMMENDATION, 1);
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32252-02.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32253-02.html" => {
                if qs.is_memo_state(ctx, 9) {
                    ctx.give_items(player, HARKILGAMEDS_LETTER, 1);
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 13, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            DEAD_SEEKER => {
                if qs.is_memo_state(ctx, 7)
                    && !ctx.has_quest_items(killer, REPORT_EAST)
                    && ctx.get_random(100) < 20
                {
                    ctx.give_items(killer, REPORT_EAST, 1);
                    if ctx.has_quest_items(killer, REPORT_NORTH) {
                        qs.set_cond(ctx, 10, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MARSH_STAKATO_DRONE => {
                if qs.is_memo_state(ctx, 7)
                    && !ctx.has_quest_items(killer, REPORT_NORTH)
                    && ctx.get_random(100) < 20
                {
                    ctx.give_items(killer, REPORT_NORTH, 1);
                    if ctx.has_quest_items(killer, REPORT_EAST) {
                        qs.set_cond(ctx, 10, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD | BREKA_ORC_WARRIOR => {
                if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, BREKA_ORC_HEAD) < 20 {
                    if ctx.get_quest_items_count(killer, BREKA_ORC_HEAD) >= 19 {
                        ctx.give_items(killer, BREKA_ORC_HEAD, 1);
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.give_items(killer, BREKA_ORC_HEAD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            ROAD_SCAVENGER => {
                if qs.is_memo_state(ctx, 4)
                    && !ctx.has_quest_items(killer, MESSAGE_PLATE)
                    && ctx.get_random(100) < 20
                {
                    ctx.give_items(killer, MESSAGE_PLATE, 1);
                    qs.set_cond(ctx, 6, true);
                }
            }
            DIVINE_EMISSARY => {
                if qs.is_memo_state(ctx, 9) && ctx.get_random(100) < 20 {
                    if let Some(kamael) = ctx.add_spawn_at(HARKILGAMED, npc.object_id, true, 60_000) {
                        let name = ctx.player_name(killer);
                        ctx.broadcast_npc_say(
                            kamael,
                            ChatType::NpcGeneral,
                            NpcStringId::S1DidYouComeToHelpMe,
                            &[&name],
                        );
                    }
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == MASTER_ORKURUS {
                    if ctx.player_race(player) == race::KAMAEL {
                        if ctx.player_class_id(player) == player_class::TROOPER {
                            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                                "32207-01.htm"
                            } else {
                                "32207-02.html"
                            }
                            .to_owned();
                        } else {
                            htmltext = "32207-03.html".to_owned();
                        }
                    } else {
                        htmltext = "32207-04.html".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                MASTER_ORKURUS => {
                    if memo_state == 1 {
                        htmltext = "32207-07.html".to_owned();
                    } else if (2..11).contains(&memo_state) {
                        htmltext = "32207-08.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "32207-09.html".to_owned();
                    }
                }
                MASTER_ENTIENS => {
                    if memo_state == 6 {
                        qs.set_memo_state(ctx, 7);
                        qs.set_cond(ctx, 9, true);
                        ctx.add_radar(player, 27956, 106003, -3831);
                        ctx.add_radar(player, 50568, 152408, -2656);
                        htmltext = "32200-01.html".to_owned();
                    } else if memo_state == 7 {
                        if !ctx.has_all_quest_items(player, &[REPORT_EAST, REPORT_NORTH]) {
                            htmltext = "32200-02.html".to_owned();
                        } else {
                            ctx.take_items(player, REPORT_EAST, 1);
                            ctx.take_items(player, REPORT_NORTH, 1);
                            qs.set_memo_state(ctx, 8);
                            qs.set_cond(ctx, 11, true);
                            htmltext = "32200-03.html".to_owned();
                        }
                    } else if memo_state == 8 {
                        htmltext = "32200-04.html".to_owned();
                    }
                }
                MASTER_TENAIN => {
                    if memo_state == 1 {
                        htmltext = "32215-01.html".to_owned();
                    } else if memo_state == 2 {
                        if ctx.get_quest_items_count(player, BREKA_ORC_HEAD) < 20 {
                            htmltext = "32215-03.html".to_owned();
                        } else {
                            ctx.take_items(player, BREKA_ORC_HEAD, -1);
                            qs.set_memo_state(ctx, 3);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "32215-04.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        htmltext = "32215-05.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "32215-06.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "32215-11.html".to_owned();
                    } else if memo_state == 8 {
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 12, true);
                        htmltext = "32215-12.html".to_owned();
                    } else if memo_state == 9 {
                        htmltext = "32215-13.html".to_owned();
                    } else if memo_state == 10 {
                        htmltext = "32215-14.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "32215-16.html".to_owned();
                    }
                }
                CARAVANER_GORT => {
                    if memo_state == 3 {
                        htmltext = "32252-01.html".to_owned();
                    } else if memo_state == 4 {
                        if !ctx.has_quest_items(player, MESSAGE_PLATE) {
                            htmltext = "32252-03.html".to_owned();
                        } else {
                            qs.set_memo_state(ctx, 5);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "32252-04.html".to_owned();
                        }
                    } else if memo_state == 5 {
                        htmltext = "32252-05.html".to_owned();
                    }
                }
                HARKILGAMED => {
                    if memo_state == 9 {
                        htmltext = "32253-01.html".to_owned();
                    } else if memo_state == 10 {
                        htmltext = "32253-03.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == MASTER_ORKURUS {
                    htmltext = "32207-05.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00064_CertifiedBerserker";

    const NPCS: &str = r#"
        <npc id="32200" type="Folk" level="70" name="Master Entiens"/>
        <npc id="32207" type="Folk" level="70" name="Master Orkurus"/>
        <npc id="32215" type="Folk" level="70" name="Master Tenain"/>
        <npc id="32252" type="Folk" level="70" name="Caravaner Gort"/>
        <npc id="32253" type="Folk" level="70" name="Harkilgamed"/>
        <npc id="20267" type="Monster" level="40" name="Breka Orc"/>
        <npc id="20551" type="Monster" level="40" name="Road Scavenger"/>
        <npc id="27323" type="Monster" level="40" name="Divine Emissary"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="9754" type="EtcItem" name="Breka Orc Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9755" type="EtcItem" name="Message Plate"><set name="is_questitem" val="true"/></item>
        <item id="9760" type="EtcItem" name="Orkurus' Recommendation"/>"#;

    struct Npcs {
        orkurus: Oid,
        tenain: Oid,
        gort: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00064CertifiedBerserker);
        let pages = Pages::of(&Q00064CertifiedBerserker)
            .page("32207-01.htm", &["ACCEPT"])
            .plain(&[
                "32207-02.html",
                "32207-03.html",
                "32207-04.html",
                "32207-05.html",
                "32207-06.htm",
                "32207-06a.htm",
                "32207-07.html",
                "32207-09.html",
            ])
            .npcs(&[
                MASTER_ORKURUS,
                MASTER_ENTIENS,
                MASTER_TENAIN,
                CARAVANER_GORT,
                HARKILGAMED,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            orkurus: t.spawn(MASTER_ORKURUS, 20),
            tenain: t.spawn(MASTER_TENAIN, 40),
            gort: t.spawn(CARAVANER_GORT, 60),
        };
        (t, npcs)
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00064CertifiedBerserker.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00064CertifiedBerserker.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_kamael_trooper_of_level_39_starts_it() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::KAMAEL, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.orkurus).contains("32207-02.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, npcs.orkurus).contains("32207-03.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.orkurus).contains("32207-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_pays_the_diamonds_once_and_the_heads_and_the_plate() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.orkurus).contains("32207-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("32207-06.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 48);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Tenain sends the player to the Breka orcs: memo 2, cond 2; twenty heads: cond 3.
        assert_eq!(talk(&mut t, npcs.tenain).as_deref(), Some("32215-01.html"));
        assert_eq!(ev(&mut t, "32215-02.html").as_deref(), Some("32215-02.html"));
        for _ in 0..20 {
            let mob = t.spawn(BREKA_ORC, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, BREKA_ORC_HEAD), 20);
        assert_eq!(talk(&mut t, npcs.tenain).as_deref(), Some("32215-04.html"));
        assert_eq!(t.count(1, BREKA_ORC_HEAD), 0);

        // Gort sends to the road scavengers (memo 4); the plate drops with a 20% roll.
        assert_eq!(ev(&mut t, "32252-02.html").as_deref(), Some("32252-02.html"));
        for _ in 0..200 {
            if t.count(1, MESSAGE_PLATE) > 0 {
                break;
            }
            let mob = t.spawn(ROAD_SCAVENGER, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, MESSAGE_PLATE), 1);
        assert_eq!(talk(&mut t, npcs.gort).as_deref(), Some("32252-04.html"));
    }

    #[test]
    fn the_recommendation_pays_and_the_diamonds_are_not_paid_twice() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::TROOPER, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.set_player_variable(Oid(1), DIAMOND_REWARD_VAR, "1")
        });
        t.quest_page(1, npcs.orkurus);
        assert!(t.event(1, NAME, "ACCEPT").contains("32207-06a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 0);
        t.take_writes();

        memo(&mut t, 11);
        assert_eq!(talk(&mut t, npcs.orkurus).as_deref(), Some("32207-09.html"));
        assert_eq!(ev(&mut t, "32207-11.html").as_deref(), Some("32207-11.html"));
        assert_eq!(t.count(1, ORKURUS_RECOMMENDATION), 1);
        assert_eq!(t.count(1, 57), 63_104);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert_eq!(
            w,
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert_eq!(talk(&mut t, npcs.orkurus).as_deref(), Some("32207-05.html"));
    }
}
