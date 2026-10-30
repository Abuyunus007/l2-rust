//! Quest 410: Path of the Palus Knight (Master Virgil and Kalinta).
// Java: dist/game/data/scripts/quests/Q00410_PathOfThePalusKnight/Q00410_PathOfThePalusKnight.java
//
// Pattern: a kill chain with the items taken at each step (`docs/design/scripts.md`, "Quest
// patterns"): thirteen lycanthrope skulls, one venomous spider carapace and five tracker silks,
// then Kalinta's coffin and Virgil's reward.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_VIRGIL: i32 = 30329;
const KALINTA: i32 = 30422;

// Items
const PALLUS_TALISMAN: i32 = 1237;
const LYCANTHROPE_SKULL: i32 = 1238;
const VIRGILS_LETTER: i32 = 1239;
const MORTE_TALISMAN: i32 = 1240;
const VENOMOUS_SPIDERS_CARAPACE: i32 = 1241;
const ARACHNID_TRACKER_SILK: i32 = 1242;
const COFFIN_OF_ETERNAL_REST: i32 = 1243;

// Reward
const GAZE_OF_ABYSS: i32 = 1244;

// Monsters
const VENOMOUS_SPIDER: i32 = 20038;
const ARACHNID_TRACKER: i32 = 20043;
const LYCANTHROPE: i32 = 20049;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00410_PathOfThePalusKnight`.
pub struct Q00410PathOfThePalusKnight;

impl Script for Q00410PathOfThePalusKnight {
    fn name(&self) -> &'static str {
        "Q00410_PathOfThePalusKnight"
    }

    fn path(&self) -> &'static str {
        "quests/Q00410_PathOfThePalusKnight"
    }

    fn id(&self) -> i32 {
        410
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_VIRGIL]);
        s.add_talk_id(&[MASTER_VIRGIL, KALINTA]);
        s.add_kill_id(&[VENOMOUS_SPIDER, ARACHNID_TRACKER, LYCANTHROPE]);
        s.register_quest_items(&[
            PALLUS_TALISMAN,
            LYCANTHROPE_SKULL,
            VIRGILS_LETTER,
            MORTE_TALISMAN,
            VENOMOUS_SPIDERS_CARAPACE,
            ARACHNID_TRACKER_SILK,
            COFFIN_OF_ETERNAL_REST,
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
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::DARK_FIGHTER {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, GAZE_OF_ABYSS) {
                                "30329-04.htm"
                            } else {
                                "30329-05.htm"
                            }
                        } else {
                            "30329-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::PALUS_KNIGHT {
                        "30329-02a.htm"
                    } else {
                        "30329-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30329-06.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, PALLUS_TALISMAN, 1);
                htmltext = Some(event.to_owned());
            }
            "30329-10.html" => {
                if ctx.has_all_quest_items(player, &[PALLUS_TALISMAN, LYCANTHROPE_SKULL]) {
                    ctx.take_items(player, PALLUS_TALISMAN, 1);
                    ctx.take_items(player, LYCANTHROPE_SKULL, -1);
                    ctx.give_items(player, VIRGILS_LETTER, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30422-02.html" => {
                if ctx.has_quest_items(player, VIRGILS_LETTER) {
                    ctx.take_items(player, VIRGILS_LETTER, 1);
                    ctx.give_items(player, MORTE_TALISMAN, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30422-06.html" => {
                if ctx.has_all_quest_items(
                    player,
                    &[MORTE_TALISMAN, ARACHNID_TRACKER_SILK, VENOMOUS_SPIDERS_CARAPACE],
                ) {
                    ctx.take_items(player, MORTE_TALISMAN, 1);
                    ctx.take_items(player, VENOMOUS_SPIDERS_CARAPACE, 1);
                    ctx.take_items(player, ARACHNID_TRACKER_SILK, -1);
                    ctx.give_items(player, COFFIN_OF_ETERNAL_REST, 1);
                    qs.set_cond(ctx, 6, true);
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
            VENOMOUS_SPIDER => {
                if ctx.has_quest_items(killer, MORTE_TALISMAN)
                    && ctx.get_quest_items_count(killer, VENOMOUS_SPIDERS_CARAPACE) < 1
                {
                    ctx.give_items(killer, VENOMOUS_SPIDERS_CARAPACE, 1);
                    if ctx.get_quest_items_count(killer, ARACHNID_TRACKER_SILK) >= 5 {
                        qs.set_cond(ctx, 5, true);
                    }
                }
            }
            ARACHNID_TRACKER => {
                if ctx.has_quest_items(killer, MORTE_TALISMAN)
                    && ctx.get_quest_items_count(killer, ARACHNID_TRACKER_SILK) < 5
                {
                    ctx.give_items(killer, ARACHNID_TRACKER_SILK, 1);
                    if ctx.get_quest_items_count(killer, ARACHNID_TRACKER_SILK) == 5 {
                        if ctx.get_quest_items_count(killer, ARACHNID_TRACKER_SILK) >= 4
                            && ctx.has_quest_items(killer, VENOMOUS_SPIDERS_CARAPACE)
                        {
                            qs.set_cond(ctx, 5, true);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            LYCANTHROPE => {
                if ctx.has_quest_items(killer, PALLUS_TALISMAN)
                    && ctx.get_quest_items_count(killer, LYCANTHROPE_SKULL) < 13
                {
                    ctx.give_items(killer, LYCANTHROPE_SKULL, 1);
                    if ctx.get_quest_items_count(killer, LYCANTHROPE_SKULL) == 13 {
                        qs.set_cond(ctx, 2, true);
                    } else {
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
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == MASTER_VIRGIL {
                htmltext = "30329-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                MASTER_VIRGIL => {
                    if ctx.has_quest_items(player, PALLUS_TALISMAN) {
                        if !ctx.has_quest_items(player, LYCANTHROPE_SKULL) {
                            htmltext = "30329-07.html".to_owned();
                        } else if ctx.get_quest_items_count(player, LYCANTHROPE_SKULL) < 13 {
                            htmltext = "30329-08.html".to_owned();
                        } else {
                            htmltext = "30329-09.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COFFIN_OF_ETERNAL_REST) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, GAZE_OF_ABYSS, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 26_212);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 32_910);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 39_608);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30329-11.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[VIRGILS_LETTER, MORTE_TALISMAN]) {
                        htmltext = "30329-12.html".to_owned();
                    }
                }
                KALINTA => {
                    if ctx.has_quest_items(player, VIRGILS_LETTER) {
                        htmltext = "30422-01.html".to_owned();
                    } else if ctx.has_quest_items(player, MORTE_TALISMAN) {
                        if !ctx
                            .has_all_quest_items(player, &[ARACHNID_TRACKER_SILK, VENOMOUS_SPIDERS_CARAPACE])
                        {
                            htmltext = "30422-03.html".to_owned();
                        } else if !ctx.has_quest_items(player, ARACHNID_TRACKER_SILK)
                            && ctx.has_quest_items(player, VENOMOUS_SPIDERS_CARAPACE)
                        {
                            // NOTE(java-quirk): unreachable, the branch above already took
                            // "not both" (carapace without silk); Java has it too.
                            htmltext = "30422-04.html".to_owned();
                        } else if ctx.get_quest_items_count(player, ARACHNID_TRACKER_SILK) >= 5
                            && ctx.has_quest_items(player, VENOMOUS_SPIDERS_CARAPACE)
                        {
                            htmltext = "30422-05.html".to_owned();
                        } else if ctx
                            .has_all_quest_items(player, &[ARACHNID_TRACKER_SILK, VENOMOUS_SPIDERS_CARAPACE])
                        {
                            htmltext = "30422-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COFFIN_OF_ETERNAL_REST) {
                        htmltext = "30422-06.html".to_owned();
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
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00410_PathOfThePalusKnight";

    const NPCS: &str = r#"
        <npc id="30329" type="Folk" level="70" name="Master Virgil"/>
        <npc id="30422" type="Folk" level="70" name="Kalinta"/>
        <npc id="20038" type="Monster" level="20" name="Venomous Spider"/>
        <npc id="20043" type="Monster" level="20" name="Arachnid Tracker"/>
        <npc id="20049" type="Monster" level="20" name="Lycanthrope"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1237" type="EtcItem" name="Pallus Talisman"><set name="is_questitem" val="true"/></item>
        <item id="1238" type="EtcItem" name="Lycanthrope Skull"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1239" type="EtcItem" name="Virgil's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1240" type="EtcItem" name="Morte Talisman"><set name="is_questitem" val="true"/></item>
        <item id="1241" type="EtcItem" name="Venomous Spider's Carapace"><set name="is_questitem" val="true"/></item>
        <item id="1242" type="EtcItem" name="Arachnid Tracker Silk"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1243" type="EtcItem" name="Coffin of Eternal Rest"><set name="is_questitem" val="true"/></item>
        <item id="1244" type="EtcItem" name="Gaze of Abyss"/>"#;

    struct Npcs {
        virgil: Oid,
        kalinta: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00410PathOfThePalusKnight)
            .page("30329-01.htm", &["ACCEPT"])
            .page("30329-05.htm", &["30329-06.htm"])
            .page("30329-09.html", &["30329-10.html"])
            .page("30422-01.html", &["30422-02.html"])
            .page("30422-05.html", &["30422-06.html"])
            .plain(&[
                "30329-02.htm",
                "30329-02a.htm",
                "30329-03.htm",
                "30329-04.htm",
                "30329-06.htm",
                "30329-07.html",
                "30329-08.html",
                "30329-10.html",
                "30329-11.html",
                "30329-12.html",
                "30422-02.html",
                "30422-03.html",
                "30422-04.html",
                "30422-06.html",
            ])
            .npcs(&[MASTER_VIRGIL, KALINTA])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00410PathOfThePalusKnight);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            virgil: t.spawn(MASTER_VIRGIL, 20),
            kalinta: t.spawn(KALINTA, 40),
        };
        (t, npcs)
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_gaze() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.virgil).contains("30329-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-02.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        t.quest_page(1, npcs.virgil);
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-05.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        t.quest_page(1, npcs.virgil);
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        t.quest_page(1, npcs.virgil);
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::DARK_ELF,
            player_class::DARK_FIGHTER,
            vec![item(5000, 1244, 1)],
        );
        t.quest_page(1, npcs.virgil);
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_skulls_to_the_gaze() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.virgil).contains("30329-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30329-05.htm"));
        assert!(t.event(1, NAME, "30329-06.htm").contains("30329-06.htm"));
        assert_eq!(t.count(1, PALLUS_TALISMAN), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No skulls yet.
        assert!(t.quest_page(1, npcs.virgil).contains("30329-07.html"));

        // Thirteen lycanthrope skulls; the thirteenth sets cond 2.
        hunt(&mut t, LYCANTHROPE, LYCANTHROPE_SKULL, 13);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.virgil).contains("30329-09.html"));

        // Virgil takes the talisman and the skulls and gives his letter: cond 3.
        assert!(t.event(1, NAME, "30329-10.html").contains("30329-10.html"));
        assert_eq!(
            (
                t.count(1, PALLUS_TALISMAN),
                t.count(1, LYCANTHROPE_SKULL),
                t.count(1, VIRGILS_LETTER)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Kalinta takes the letter and gives the morte talisman: cond 4.
        assert!(t.quest_page(1, npcs.virgil).contains("30329-12.html"));
        assert!(t.quest_page(1, npcs.kalinta).contains("30422-01.html"));
        assert!(t.event(1, NAME, "30422-02.html").contains("30422-02.html"));
        assert_eq!((t.count(1, VIRGILS_LETTER), t.count(1, MORTE_TALISMAN)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // One venomous spider carapace; Kalinta has no silk yet.
        hunt(&mut t, VENOMOUS_SPIDER, VENOMOUS_SPIDERS_CARAPACE, 1);
        assert!(t.quest_page(1, npcs.kalinta).contains("30422-03.html"));
        assert!(t.take_writes().is_empty());

        // Five arachnid silks; the fifth sets cond 5 (the carapace is in).
        hunt(&mut t, ARACHNID_TRACKER, ARACHNID_TRACKER_SILK, 5);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.kalinta).contains("30422-05.html"));

        // Kalinta takes the morte talisman, the carapace and the silk: cond 6, the coffin.
        assert!(t.event(1, NAME, "30422-06.html").contains("30422-06.html"));
        assert_eq!(
            (
                t.count(1, MORTE_TALISMAN),
                t.count(1, VENOMOUS_SPIDERS_CARAPACE),
                t.count(1, ARACHNID_TRACKER_SILK),
                t.count(1, COFFIN_OF_ETERNAL_REST)
            ),
            (0, 0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Virgil's reward: adena, the gaze, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.kalinta).contains("30422-06.html"));
        assert!(t.quest_page(1, npcs.virgil).contains("30329-11.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, GAZE_OF_ABYSS), 1);
        assert_eq!(t.count(1, COFFIN_OF_ETERNAL_REST), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Virgil shows his first page again.
        assert!(t.quest_page(1, npcs.virgil).contains("30329-01.htm"));
    }
}
