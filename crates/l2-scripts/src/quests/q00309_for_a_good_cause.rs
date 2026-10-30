//! Quest 309: For A Good Cause (Atra in the Hot Springs area trades the mucrokian hides for dynasty and Moirai items).
// Java: dist/game/data/scripts/quests/Q00309_ForAGoodCause/Q00309_ForAGoodCause.java
//
// Pattern: the same trade as Q00308 (`q00308_reed_field_maintenance.rs`) with its own hides and
// drop chances: the mucrokians drop hides for a random party member, the contaminated one a fallen
// hide and a second one through the quest rewards; Atra exchanges the hides for one item of a
// table. Players on Q00308_ReedFieldMaintenance are sent to its own NPC (`player_quest_state`).
//
// FOLLOW-UP(engine): the Java radar point of the start (`RadarControl(0, 2, ..)`) has no script
// helper yet; the page is shown, the radar marker is not sent.
// NOTE(java-quirk): the exchange answers are the pages `32646-16.htm` and `32646-15.htm` (the
// pages of Q00308), not this quest's own `32647-*` pages.

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ATRA: i32 = 32647;

// Mobs
const CORRUPTED_MUCROKIAN: i32 = 22654;
const AWAKENED_MUCROKIAN: i32 = 22655;

/// Java `MUCROKIANS`: the hide chance out of 1000 per mucrokian.
fn hide_chance(npc_id: i32) -> i32 {
    match npc_id {
        22650 => 218, // Mucrokian Fanatic
        22651 => 258, // Mucrokian Ascetic
        22652 => 248, // Mucrokian Savior
        22653 => 290, // Mucrokian Preacher
        22654 => 124, // Contaminated Mucrokian
        22655 => 220, // Awakened Mucrokian
        _ => 0,
    }
}

// Items
const MUCROKIAN_HIDE: i32 = 14873;
const FALLEN_MUCROKIAN_HIDE: i32 = 14874;

// Rewards
const REC_DYNASTY_EARRINGS_70: i32 = 9985;
const REC_DYNASTY_NECKLACE_70: i32 = 9986;
const REC_DYNASTY_RING_70: i32 = 9987;
const REC_DYNASTY_SIGIL_60: i32 = 10115;

const MOIRAI_RECIPES: [i32; 9] = [15777, 15780, 15783, 15786, 15789, 15790, 15814, 15813, 15812];
const MOIRAI_PIECES: [i32; 9] = [15647, 15650, 15653, 15656, 15659, 15692, 15772, 15773, 15774];

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00308_ReedFieldMaintenance.class.getSimpleName()`.
const REED_FIELD_MAINTENANCE: &str = "Q00308_ReedFieldMaintenance";
/// Java `Q00239_WontYouJoinUs.class.getSimpleName()`.
const WONT_YOU_JOIN_US: &str = "Q00239_WontYouJoinUs";

/// Java `canGiveItem(player, quanty)`: the awakened (fallen) hides first, two mucrokian hides for
/// one of them.
fn can_give_item(ctx: &mut ScriptCtx<'_>, player: ObjectId, quanty: i64) -> bool {
    let mucrokian = ctx.get_quest_items_count(player, MUCROKIAN_HIDE);
    let fallen = ctx.get_quest_items_count(player, FALLEN_MUCROKIAN_HIDE);
    if fallen > 0 {
        if fallen >= quanty / 2 {
            ctx.take_items(player, FALLEN_MUCROKIAN_HIDE, quanty / 2);
            return true;
        } else if mucrokian >= quanty - fallen * 2 {
            ctx.take_items(player, FALLEN_MUCROKIAN_HIDE, fallen);
            ctx.take_items(player, MUCROKIAN_HIDE, quanty - fallen * 2);
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
        // NOTE(java-quirk): the answer pages are the ones of Q00308 (see the file comment).
        "32646-16.htm".to_owned()
    } else {
        "32646-15.htm".to_owned()
    }
}

/// Java `Q00309_ForAGoodCause`.
pub struct Q00309ForAGoodCause;

impl Script for Q00309ForAGoodCause {
    fn name(&self) -> &'static str {
        "Q00309_ForAGoodCause"
    }

    fn path(&self) -> &'static str {
        "quests/Q00309_ForAGoodCause"
    }

    fn id(&self) -> i32 {
        309
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ATRA]);
        s.add_talk_id(&[ATRA]);
        s.add_kill_id(&[
            22650,
            22651,
            22652,
            22653,
            CORRUPTED_MUCROKIAN,
            AWAKENED_MUCROKIAN,
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
            "32647-02.htm" | "32647-03.htm" | "32647-04.htm" | "32647-08.html" | "32647-10.html"
            | "32647-12.html" | "32647-13.html" => {
                htmltext = Some(event.to_owned());
            }
            "32647-05.html" => {
                qs.start_quest(ctx);
                // FOLLOW-UP(engine): Java sends `RadarControl(0, 2, 77325, 205773, -3432)` here.
                htmltext = Some(event.to_owned());
            }
            "claimreward" => {
                let completed = ctx
                    .player_quest_state(player, WONT_YOU_JOIN_US)
                    .is_some_and(|q239| q239.is_completed());
                htmltext = Some(
                    if completed {
                        "32647-11.html"
                    } else {
                        "32647-09.html"
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
            "32647-14.html" | "32647-07.html" => {
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
            if npc.id == CORRUPTED_MUCROKIAN {
                ctx.give_items(member, FALLEN_MUCROKIAN_HIDE, 1);
                ctx.reward_items(member, FALLEN_MUCROKIAN_HIDE, 1);
            } else {
                ctx.give_items(member, MUCROKIAN_HIDE, 1);
            }
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let on_reed_field = ctx
            .player_quest_state(talker, REED_FIELD_MAINTENANCE)
            .is_some_and(|q308| q308.is_started());
        let htmltext = if on_reed_field {
            "32647-17.html".to_owned()
        } else if qs.state(ctx) == STARTED {
            if ctx.has_at_least_one_quest_item(talker, &[MUCROKIAN_HIDE, FALLEN_MUCROKIAN_HIDE]) {
                "32647-08.html".to_owned()
            } else {
                "32647-06.html".to_owned()
            }
        } else if ctx.player_level(talker) >= MIN_LEVEL {
            "32647-01.htm".to_owned()
        } else {
            "32647-00.html".to_owned()
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

    const NAME: &str = "Q00309_ForAGoodCause";

    /// The stand-in of Q00308: an NPC of its own, and the quest started when talked to.
    const REED_NPC: i32 = 32646;

    const NPCS: &str = r#"
        <npc id="32647" type="Folk" level="85" name="Atra"/>
        <npc id="32646" type="Folk" level="70" name="Reed Stand-in"/>
        <npc id="22654" type="Monster" level="80" name="Contaminated Mucrokian"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14873" type="EtcItem" name="Mucrokian Hide"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14874" type="EtcItem" name="Fallen Mucrokian Hide"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15647" type="EtcItem" name="Moirai Piece"><set name="is_stackable" val="true"/></item>
        <item id="15650" type="EtcItem" name="Moirai Piece 2"><set name="is_stackable" val="true"/></item>
        <item id="15653" type="EtcItem" name="Moirai Piece 3"><set name="is_stackable" val="true"/></item>
        <item id="15656" type="EtcItem" name="Moirai Piece 4"><set name="is_stackable" val="true"/></item>
        <item id="15659" type="EtcItem" name="Moirai Piece 5"><set name="is_stackable" val="true"/></item>
        <item id="15692" type="EtcItem" name="Moirai Piece 6"><set name="is_stackable" val="true"/></item>
        <item id="15772" type="EtcItem" name="Moirai Piece 7"><set name="is_stackable" val="true"/></item>
        <item id="15773" type="EtcItem" name="Moirai Piece 8"><set name="is_stackable" val="true"/></item>
        <item id="15774" type="EtcItem" name="Moirai Piece 9"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00308_ReedFieldMaintenance` as far as this quest needs it: its NPC starts it.
    struct ReedFieldMaintenance;

    impl Script for ReedFieldMaintenance {
        fn name(&self) -> &'static str {
            REED_FIELD_MAINTENANCE
        }
        fn path(&self) -> &'static str {
            "quests/Q00308_ReedFieldMaintenance"
        }
        fn id(&self) -> i32 {
            308
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[REED_NPC]);
            s.add_talk_id(&[REED_NPC]);
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
        registry.add(ReedFieldMaintenance);
        registry.add(Q00309ForAGoodCause);
        let pages = Pages::of(&Q00309ForAGoodCause)
            .page("32647-01.htm", &["32647-02.htm"])
            .page("32647-02.htm", &["32647-03.htm", "32647-04.htm"])
            .page("32647-03.htm", &["32647-04.htm", "32647-07.html"])
            .page("32647-04.htm", &["32647-05.html"])
            .plain(&[
                "32647-00.html",
                "32647-05.html",
                "32647-06.html",
                "32647-07.html",
                "32647-15.htm",
                "32647-17.html",
            ])
            .page(
                "32647-08.html",
                &["claimreward", "32647-12.html", "32647-13.html"],
            )
            .page("32647-09.html", &["32647-10.html", "32647-12.html"])
            .page("32647-10.html", &["120", "216", "32647-08.html"])
            .page("32647-13.html", &["claimreward", "32647-14.html"])
            .plain(&["32647-11.html", "32647-12.html", "32647-14.html", "32647-16.htm"])
            .npcs(&[ATRA, REED_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let atra = t.spawn(ATRA, 20);
        let stand_in = t.spawn(REED_NPC, 40);
        (t, atra, stand_in)
    }

    #[test]
    fn a_level_eighty_one_character_is_sent_away_and_a_reed_field_player_is_sent_on() {
        let (mut t, atra, stand_in) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, atra).contains("32647-00.html"));
        t.enter_as(2, 85, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(2, stand_in);
        t.take_writes();
        assert!(t.quest_page(2, atra).contains("32647-17.html"));
    }

    #[test]
    fn hides_for_a_moirai_piece_then_the_quest_ends_from_the_hide_page() {
        let (mut t, atra, _stand_in) = setup();
        t.enter_as(
            1,
            82,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, MUCROKIAN_HIDE, 150)],
        );
        assert!(t.quest_page(1, atra).contains("32647-01.htm"));
        assert!(t.event(1, NAME, "32647-02.htm").contains("32647-02.htm"));
        assert!(t.event(1, NAME, "32647-04.htm").contains("32647-04.htm"));
        assert!(t.event(1, NAME, "32647-05.html").contains("32647-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The hides: the hide page; the claim has no Q00239 state, so the "not yet" page.
        assert!(t.quest_page(1, atra).contains("32647-08.html"));
        assert!(t.event(1, NAME, "claimreward").contains("32647-09.html"));
        assert!(t.event(1, NAME, "32647-10.html").contains("32647-10.html"));

        // 120 hides for one Moirai piece (the answer pages are not in this quest's folder, see the
        // file comment: only the items are checked).
        t.event(1, NAME, "120");
        assert_eq!(t.count(1, MUCROKIAN_HIDE), 30);
        let pieces: i64 = MOIRAI_PIECES.iter().map(|&item| t.count(1, item)).sum();
        assert!((1..=4).contains(&pieces), "{pieces}");

        // Not enough for 216: nothing taken.
        assert!(t.quest_page(1, atra).contains("32647-08.html"));
        assert!(t.event(1, NAME, "claimreward").contains("32647-09.html"));
        assert!(t.event(1, NAME, "32647-10.html").contains("32647-10.html"));
        t.event(1, NAME, "216");
        assert_eq!(t.count(1, MUCROKIAN_HIDE), 30);
        assert!(t.take_writes().is_empty());

        // The last page ends the quest (`exitQuest(true, true)`).
        assert!(t.quest_page(1, atra).contains("32647-08.html"));
        assert!(t.event(1, NAME, "32647-13.html").contains("32647-13.html"));
        assert!(t.event(1, NAME, "32647-14.html").contains("32647-14.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_contaminated_mucrokian_gives_two_fallen_hides_per_drop() {
        let (mut t, atra, _stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, atra).contains("32647-01.htm"));
        assert!(t.event(1, NAME, "32647-02.htm").contains("32647-02.htm"));
        assert!(t.event(1, NAME, "32647-04.htm").contains("32647-04.htm"));
        assert!(t.event(1, NAME, "32647-05.html").contains("32647-05.html"));
        for _ in 0..500 {
            if t.count(1, FALLEN_MUCROKIAN_HIDE) >= 2 {
                break;
            }
            let contaminated = t.spawn(CORRUPTED_MUCROKIAN, 60);
            t.kill(1, contaminated);
        }
        let fallen = t.count(1, FALLEN_MUCROKIAN_HIDE);
        assert!(fallen >= 2 && fallen % 2 == 0, "{fallen}");
        assert!(t.quest_page(1, atra).contains("32647-08.html"));
    }
}
