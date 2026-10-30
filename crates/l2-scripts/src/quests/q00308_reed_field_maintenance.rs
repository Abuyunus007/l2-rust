//! Quest 308: Reed Field Maintenance (Katensa in the Hot Springs area trades the mucrokian hides for dynasty and Moirai items).
// Java: dist/game/data/scripts/quests/Q00308_ReedFieldMaintenance/Q00308_ReedFieldMaintenance.java
//
// Pattern: a repeatable trade (`docs/design/scripts.md`, "Quest patterns"): the mucrokians drop
// hides (awakened ones their own kind) for a random party member; Katensa exchanges hides for one
// item of a table (Moirai pieces get 1 to 4 of them), the exchange takes the awakened hides first.
// Players who are on Q00309_ForAGoodCause are sent to its own NPC (`player_quest_state`).
//
// NOTE(java-quirk): the reward check reads Q00238 with the full class name (`Class.getName()`),
// which never matches a quest state (states are keyed by the simple name), so the page is always
// the one for "not completed".

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KATENSA: i32 = 32646;

// Mobs
const AWAKENED_MUCROKIAN: i32 = 22655;

/// Java `MUCROKIAN`: the hide chance out of 1000 per mucrokian.
fn hide_chance(npc_id: i32) -> i32 {
    match npc_id {
        22650 => 218, // Mucrokian Fanatic
        22651 => 258, // Mucrokian Ascetic
        22652 => 248, // Mucrokian Savior
        22653 => 290, // Mucrokian Preacher
        22654 => 220, // Contaminated Mucrokian
        22655 => 124, // Awakened Mucrokian
        _ => 0,
    }
}

// Items
const MUCROKIAN_HIDE: i32 = 14871;
const AWAKENED_MUCROKIAN_HIDE: i32 = 14872;

// Rewards
const REC_DYNASTY_EARRINGS_70: i32 = 9985;
const REC_DYNASTY_NECKLACE_70: i32 = 9986;
const REC_DYNASTY_RING_70: i32 = 9987;
const REC_DYNASTY_SIGIL_60: i32 = 10115;

const MOIRAI_RECIPES: [i32; 9] = [15777, 15780, 15783, 15786, 15789, 15790, 15814, 15813, 15812];
const MOIRAI_PIECES: [i32; 9] = [15647, 15650, 15653, 15656, 15659, 15692, 15772, 15773, 15774];

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00309_ForAGoodCause.class.getSimpleName()`.
const FOR_A_GOOD_CAUSE: &str = "Q00309_ForAGoodCause";
/// Java `Q00238_SuccessFailureOfBusiness.class.getName()`: the full name, not a quest state key.
const SUCCESS_FAILURE_OF_BUSINESS_CLASS_NAME: &str =
    "quests.Q00238_SuccessFailureOfBusiness.Q00238_SuccessFailureOfBusiness";

/// Java `canGiveItem(player, quanty)`: takes the hides of an exchange of `quanty` hides, the
/// awakened ones first (two mucrokian hides count for one awakened one).
fn can_give_item(ctx: &mut ScriptCtx<'_>, player: ObjectId, quanty: i64) -> bool {
    let mucrokian = ctx.get_quest_items_count(player, MUCROKIAN_HIDE);
    let awakened = ctx.get_quest_items_count(player, AWAKENED_MUCROKIAN_HIDE);
    if awakened > 0 {
        if awakened >= quanty / 2 {
            ctx.take_items(player, AWAKENED_MUCROKIAN_HIDE, quanty / 2);
            return true;
        } else if mucrokian >= quanty - awakened * 2 {
            ctx.take_items(player, AWAKENED_MUCROKIAN_HIDE, awakened);
            ctx.take_items(player, MUCROKIAN_HIDE, quanty - awakened * 2);
            return true;
        }
    } else if mucrokian >= quanty {
        ctx.take_items(player, MUCROKIAN_HIDE, quanty);
        return true;
    }
    false
}

/// Java `onItemExchangeRequest(player, item, quanty)`.
fn item_exchange_request(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: i32, quanty: i64) -> String {
    if can_give_item(ctx, player, quanty) {
        if MOIRAI_PIECES.contains(&item) {
            let count = i64::from(ctx.get_random_range(1, 4));
            ctx.give_items(player, item, count);
        } else {
            ctx.give_items(player, item, 1);
        }
        ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
        "32646-14.html".to_owned()
    } else {
        "32646-13.html".to_owned()
    }
}

/// Java `Q00308_ReedFieldMaintenance`.
pub struct Q00308ReedFieldMaintenance;

impl Script for Q00308ReedFieldMaintenance {
    fn name(&self) -> &'static str {
        "Q00308_ReedFieldMaintenance"
    }

    fn path(&self) -> &'static str {
        "quests/Q00308_ReedFieldMaintenance"
    }

    fn id(&self) -> i32 {
        308
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KATENSA]);
        s.add_talk_id(&[KATENSA]);
        s.add_kill_id(&[22650, 22651, 22652, 22653, 22654, AWAKENED_MUCROKIAN]);
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
            "32646-02.htm" | "32646-03.htm" | "32646-06.html" | "32646-07.html" | "32646-08.html"
            | "32646-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "32646-04.html" => {
                qs.start_quest(ctx);
                ctx.send_radar_control(player, 0, 2, 77_325, 205_773, -3_432);
                htmltext = Some(event.to_owned());
            }
            "claimreward" => {
                let completed = ctx
                    .player_quest_state(player, SUCCESS_FAILURE_OF_BUSINESS_CLASS_NAME)
                    .is_some_and(|q238| q238.is_completed());
                htmltext = Some(
                    if completed {
                        "32646-09.html"
                    } else {
                        "32646-12.html"
                    }
                    .to_owned(),
                );
            }
            "100" | "120" => {
                let item = MOIRAI_PIECES[ctx.get_random(8) as usize];
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    item,
                    event.parse().unwrap_or(0),
                ));
            }
            "192" | "230" => {
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    REC_DYNASTY_EARRINGS_70,
                    event.parse().unwrap_or(0),
                ));
            }
            "256" | "308" => {
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    REC_DYNASTY_NECKLACE_70,
                    event.parse().unwrap_or(0),
                ));
            }
            "128" | "154" => {
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    REC_DYNASTY_RING_70,
                    event.parse().unwrap_or(0),
                ));
            }
            "206" | "246" => {
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    REC_DYNASTY_SIGIL_60,
                    event.parse().unwrap_or(0),
                ));
            }
            "180" | "216" => {
                let item = MOIRAI_RECIPES[ctx.get_random(8) as usize];
                htmltext = Some(item_exchange_request(
                    ctx,
                    player,
                    item,
                    event.parse().unwrap_or(0),
                ));
            }
            "32646-11.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(member) = ctx.get_random_party_member_cond(killer, 1)
            && ctx.get_random(1000) < hide_chance(npc.id)
        {
            let hide = if npc.id == AWAKENED_MUCROKIAN {
                AWAKENED_MUCROKIAN_HIDE
            } else {
                MUCROKIAN_HIDE
            };
            ctx.give_items(member, hide, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let on_good_cause = ctx
            .player_quest_state(talker, FOR_A_GOOD_CAUSE)
            .is_some_and(|q309| q309.is_started());
        let htmltext = if on_good_cause {
            "32646-15.html".to_owned()
        } else if qs.state(ctx) == STARTED {
            if ctx.has_at_least_one_quest_item(talker, &[MUCROKIAN_HIDE, AWAKENED_MUCROKIAN_HIDE]) {
                "32646-06.html".to_owned()
            } else {
                "32646-05.html".to_owned()
            }
        } else if ctx.player_level(talker) >= MIN_LEVEL {
            "32646-01.htm".to_owned()
        } else {
            "32646-00.html".to_owned()
        };
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

    const NAME: &str = "Q00308_ReedFieldMaintenance";

    /// The stand-in of Q00309: an NPC of its own, and the quest started when talked to.
    const FOR_A_GOOD_CAUSE_NPC: i32 = 32647;

    const NPCS: &str = r#"
        <npc id="32646" type="Folk" level="85" name="Katensa"/>
        <npc id="32647" type="Folk" level="70" name="Good Cause Stand-in"/>
        <npc id="22650" type="Monster" level="80" name="Mucrokian Fanatic"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14871" type="EtcItem" name="Mucrokian Hide"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14872" type="EtcItem" name="Awakened Mucrokian Hide"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15647" type="EtcItem" name="Moirai Piece"><set name="is_stackable" val="true"/></item>
        <item id="15650" type="EtcItem" name="Moirai Piece 2"><set name="is_stackable" val="true"/></item>
        <item id="15653" type="EtcItem" name="Moirai Piece 3"><set name="is_stackable" val="true"/></item>
        <item id="15656" type="EtcItem" name="Moirai Piece 4"><set name="is_stackable" val="true"/></item>
        <item id="15659" type="EtcItem" name="Moirai Piece 5"><set name="is_stackable" val="true"/></item>
        <item id="15692" type="EtcItem" name="Moirai Piece 6"><set name="is_stackable" val="true"/></item>
        <item id="15772" type="EtcItem" name="Moirai Piece 7"><set name="is_stackable" val="true"/></item>
        <item id="15773" type="EtcItem" name="Moirai Piece 8"><set name="is_stackable" val="true"/></item>
        <item id="15774" type="EtcItem" name="Moirai Piece 9"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00309_ForAGoodCause` as far as this quest needs it: its NPC starts it.
    struct ForAGoodCause;

    impl Script for ForAGoodCause {
        fn name(&self) -> &'static str {
            FOR_A_GOOD_CAUSE
        }
        fn path(&self) -> &'static str {
            "quests/Q00309_ForAGoodCause"
        }
        fn id(&self) -> i32 {
            309
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[FOR_A_GOOD_CAUSE_NPC]);
            s.add_talk_id(&[FOR_A_GOOD_CAUSE_NPC]);
        }
        fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
            if let Some(qs) = ctx.get_quest_state(player, true) {
                qs.start_quest(ctx);
            }
            None
        }
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ForAGoodCause);
        registry.add(Q00308ReedFieldMaintenance);
        let pages = Pages::of(&Q00308ReedFieldMaintenance)
            .page("32646-01.htm", &["32646-02.htm"])
            .page("32646-02.htm", &["32646-03.htm"])
            .page("32646-03.htm", &["32646-04.html"])
            .plain(&[
                "32646-00.html",
                "32646-04.html",
                "32646-05.html",
                "32646-07.html",
                "32646-13.html",
                "32646-14.html",
                "32646-15.html",
            ])
            .page(
                "32646-06.html",
                &["claimreward", "32646-07.html", "32646-10.html"],
            )
            .page("32646-12.html", &["32646-08.html", "32646-07.html"])
            .page(
                "32646-08.html",
                &["230", "308", "154", "246", "216", "120", "32646-06.html"],
            )
            .page("32646-10.html", &["claimreward", "32646-11.html"])
            .plain(&["32646-09.html", "32646-11.html"])
            .npcs(&[KATENSA, FOR_A_GOOD_CAUSE_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let katensa = t.spawn(KATENSA, 20);
        let stand_in = t.spawn(FOR_A_GOOD_CAUSE_NPC, 40);
        (t, katensa, stand_in)
    }

    #[test]
    fn a_level_eighty_one_character_is_sent_away_and_a_good_cause_player_too() {
        let (mut t, katensa, stand_in) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, katensa).contains("32646-00.html"));
        // A player on Q00309 is sent to that quest's NPC.
        t.enter_as(2, 85, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(2, stand_in);
        t.take_writes();
        assert!(t.quest_page(2, katensa).contains("32646-15.html"));
    }

    #[test]
    fn hides_for_a_moirai_piece_then_the_quest_ends_on_the_last_page() {
        let (mut t, katensa, _stand_in) = setup();
        t.enter_as(
            1,
            82,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, MUCROKIAN_HIDE, 150)],
        );
        assert!(t.quest_page(1, katensa).contains("32646-01.htm"));
        assert!(t.event(1, NAME, "32646-02.htm").contains("32646-02.htm"));
        assert!(t.event(1, NAME, "32646-03.htm").contains("32646-03.htm"));
        assert!(t.event(1, NAME, "32646-04.html").contains("32646-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Hides in the bag: the hide page; the claim is always the "not completed" page (see the
        // NOTE on the file).
        assert!(t.quest_page(1, katensa).contains("32646-06.html"));
        assert!(t.event(1, NAME, "claimreward").contains("32646-12.html"));
        assert!(t.event(1, NAME, "32646-08.html").contains("32646-08.html"));

        // 120 hides for one Moirai piece (one to four of them): the hides go, the item comes.
        assert!(t.event(1, NAME, "120").contains("32646-14.html"));
        assert_eq!(t.count(1, MUCROKIAN_HIDE), 30);
        let pieces: i64 = MOIRAI_PIECES.iter().map(|&item| t.count(1, item)).sum();
        assert!((1..=4).contains(&pieces), "{pieces}");

        // 216 hides are not there: the refusal page, nothing taken (page 14 has no exchange link,
        // so the exchange page 08 is opened again first).
        assert!(t.quest_page(1, katensa).contains("32646-06.html"));
        assert!(t.event(1, NAME, "claimreward").contains("32646-12.html"));
        assert!(t.event(1, NAME, "32646-08.html").contains("32646-08.html"));
        assert!(t.event(1, NAME, "216").contains("32646-13.html"));
        assert_eq!(t.count(1, MUCROKIAN_HIDE), 30);
        assert!(t.take_writes().is_empty());

        // Page 10 ends the quest (`exitQuest(true, true)`).
        assert!(t.quest_page(1, katensa).contains("32646-06.html"));
        assert!(t.event(1, NAME, "32646-10.html").contains("32646-10.html"));
        assert!(t.event(1, NAME, "32646-11.html").contains("32646-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn awakened_hides_count_double_and_go_first() {
        let (mut t, katensa, _stand_in) = setup();
        t.enter_as(
            1,
            82,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, AWAKENED_MUCROKIAN_HIDE, 60)],
        );
        assert!(t.quest_page(1, katensa).contains("32646-01.htm"));
        assert!(t.event(1, NAME, "32646-02.htm").contains("32646-02.htm"));
        assert!(t.event(1, NAME, "32646-03.htm").contains("32646-03.htm"));
        assert!(t.event(1, NAME, "32646-04.html").contains("32646-04.html"));
        t.take_writes();

        // 60 awakened hides cover a 120 exchange on their own.
        assert!(t.quest_page(1, katensa).contains("32646-06.html"));
        assert!(t.event(1, NAME, "claimreward").contains("32646-12.html"));
        assert!(t.event(1, NAME, "32646-08.html").contains("32646-08.html"));
        assert!(t.event(1, NAME, "120").contains("32646-14.html"));
        assert_eq!(t.count(1, AWAKENED_MUCROKIAN_HIDE), 0);
        let pieces: i64 = MOIRAI_PIECES.iter().map(|&item| t.count(1, item)).sum();
        assert!((1..=4).contains(&pieces), "{pieces}");
    }

    #[test]
    fn mucrokians_drop_hides_for_the_member() {
        let (mut t, katensa, _stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, katensa).contains("32646-01.htm"));
        assert!(t.event(1, NAME, "32646-02.htm").contains("32646-02.htm"));
        assert!(t.event(1, NAME, "32646-03.htm").contains("32646-03.htm"));
        assert!(t.event(1, NAME, "32646-04.html").contains("32646-04.html"));
        for _ in 0..200 {
            if t.count(1, MUCROKIAN_HIDE) >= 3 {
                break;
            }
            let fanatic = t.spawn(22650, 60);
            t.kill(1, fanatic);
        }
        assert!(t.count(1, MUCROKIAN_HIDE) >= 3);
        assert!(t.quest_page(1, katensa).contains("32646-06.html"));
    }
}
