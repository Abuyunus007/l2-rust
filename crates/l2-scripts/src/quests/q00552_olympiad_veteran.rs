//! Quest 552: Olympiad Veteran (five matches of each Olympiad kind, daily).
// Java: dist/game/data/scripts/quests/Q00552_OlympiadVeteran/Q00552_OlympiadVeteran.java

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestType};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MANAGER: i32 = 31688;

// Items
const TEAM_EVENT_CERTIFICATE: i32 = 17241;
const CLASS_FREE_BATTLE_CERTIFICATE: i32 = 17242;
const CLASS_BATTLE_CERTIFICATE: i32 = 17243;
const OLY_CHEST: i32 = 17169;

// `CompetitionType` ordinals.
const CLASSED: i32 = 0;
const NON_CLASSED: i32 = 1;
const TEAMS: i32 = 2;

/// Java `Q00552_OlympiadVeteran`.
pub struct Q00552OlympiadVeteran;

fn certificates(ctx: &ScriptCtx<'_>, player: ObjectId) -> i64 {
    ctx.get_quest_items_count(player, TEAM_EVENT_CERTIFICATE)
        + ctx.get_quest_items_count(player, CLASS_FREE_BATTLE_CERTIFICATE)
        + ctx.get_quest_items_count(player, CLASS_BATTLE_CERTIFICATE)
}

/// One finished match of `player`; `only_once` is the winner's `!hasQuestItems(..)` check (the
/// loser's branch has none).
fn count_match(ctx: &mut ScriptCtx<'_>, player: ObjectId, competition_type: i32, only_once: bool) {
    let Some(qs) = ctx.get_quest_state(player, false) else {
        return;
    };
    if !qs.is_started(ctx) {
        return;
    }
    let (variable, item) = match competition_type {
        CLASSED => ("classed", CLASS_BATTLE_CERTIFICATE),
        NON_CLASSED => ("nonclassed", CLASS_FREE_BATTLE_CERTIFICATE),
        TEAMS => ("teams", TEAM_EVENT_CERTIFICATE),
        _ => return,
    };
    let matches = qs.get_int(ctx, variable) + 1;
    qs.set(ctx, variable, &matches.to_string());
    if matches == 5 && (!only_once || !ctx.has_quest_items(player, item)) {
        ctx.give_items(player, item, 1);
    }
}

impl Script for Q00552OlympiadVeteran {
    fn name(&self) -> &'static str {
        "Q00552_OlympiadVeteran"
    }

    fn path(&self) -> &'static str {
        "quests/Q00552_OlympiadVeteran"
    }

    fn id(&self) -> i32 {
        552
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MANAGER]);
        s.add_talk_id(&[MANAGER]);
        s.register_quest_items(&[
            TEAM_EVENT_CERTIFICATE,
            CLASS_FREE_BATTLE_CERTIFICATE,
            CLASS_BATTLE_CERTIFICATE,
        ]);
        s.set_on_olympiad_match_result();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        let mut htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("31688-03.html") {
            qs.start_quest(ctx);
        } else if event.eq_ignore_ascii_case("31688-04.html") {
            let count = certificates(ctx, player);
            if count > 0 {
                ctx.give_items(player, OLY_CHEST, count);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            } else {
                htmltext = ctx.get_no_quest_msg(player);
            }
        }
        Some(htmltext)
    }

    fn on_olympiad_match_result(
        &self,
        ctx: &mut ScriptCtx<'_>,
        winner: Option<i32>,
        loser: i32,
        competition_type: i32,
    ) {
        // NOTE(java-quirk): an offline winner ends the method, so the loser is not counted then.
        if let Some(winner) = winner {
            let player = ObjectId(winner);
            if ctx.world().player(player).is_none() {
                return;
            }
            count_match(ctx, player, competition_type, true);
        }
        let player = ObjectId(loser);
        if ctx.world().player(player).is_none() {
            return;
        }
        count_match(ctx, player, competition_type, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if ctx.player_level(player) < 75 || !ctx.world().is_noble(player) {
            htmltext = "31688-00.htm".to_owned();
        } else if qs.is_created(ctx) {
            htmltext = "31688-01.htm".to_owned();
        } else if qs.is_completed(ctx) {
            if qs.is_now_available(ctx) {
                qs.set_state(ctx, CREATED);
                // The gate was passed above, so this is always the first page.
                htmltext = "31688-01.htm".to_owned();
            } else {
                htmltext = "31688-05.html".to_owned();
            }
        } else if qs.is_started(ctx) {
            let count = certificates(ctx, player);
            if count == 3 {
                htmltext = "31688-04.html".to_owned();
                ctx.give_items(player, OLY_CHEST, 4);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            } else {
                htmltext = format!("31688-s{count}.html");
            }
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

    const NAME: &str = "Q00552_OlympiadVeteran";

    const NPCS: &str = r#"<npc id="31688" type="Folk" level="70" name="Manager"/>"#;

    const ITEMS: &str = r#"
        <item id="17169" type="EtcItem" name="Olympiad Chest"><set name="is_stackable" val="true"/></item>
        <item id="17241" type="EtcItem" name="Team Cert"><set name="is_questitem" val="true"/></item>
        <item id="17242" type="EtcItem" name="Free Cert"><set name="is_questitem" val="true"/></item>
        <item id="17243" type="EtcItem" name="Class Cert"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00552OlympiadVeteran);
        let pages = Pages::of(&Q00552OlympiadVeteran)
            .page("31688-01.htm", &["31688-03.html", "31688-02.htm"])
            .page("31688-02.htm", &["31688-03.html"])
            .page("31688-s2.html", &["31688-04.html"])
            .plain(&[
                "31688-00.htm",
                "31688-03.html",
                "31688-04.html",
                "31688-05.html",
                "31688-s0.html",
                "31688-s1.html",
            ])
            .file(
                "data/html/olympiad/noble_main.htm",
                r#"<html><body>noble_main <a action="bypass Script">Quest</a></body></html>"#,
            )
            .npcs(&[MANAGER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let manager = t.spawn(MANAGER, 20);
        (t, manager)
    }

    fn finish(t: &mut ScriptTest, winner: Option<i32>, loser: i32, kind: i32) {
        t.world.run_as_script(NAME, |ctx| {
            Q00552OlympiadVeteran.on_olympiad_match_result(ctx, winner, loser, kind);
        });
    }

    fn noble(t: &mut ScriptTest, id: i32, level: i32) {
        t.enter(id, level, vec![]);
        t.world.set_noble(Oid(id), true);
    }

    #[test]
    fn only_nobles_of_level_75_start() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, manager).contains("31688-00.htm"));
        noble(&mut t, 2, 74);
        assert!(t.quest_page(2, manager).contains("31688-00.htm"));
        noble(&mut t, 3, 75);
        assert!(t.quest_page(3, manager).contains("31688-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_matches_of_each_kind_then_the_reward() {
        let (mut t, manager) = setup();
        noble(&mut t, 1, 80);
        noble(&mut t, 2, 80);
        assert!(t.quest_page(1, manager).contains("31688-01.htm"));
        assert!(t.event(1, NAME, "31688-03.html").contains("31688-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, manager).contains("31688-s0.html"));

        // Player 1 wins five classed matches, loses five non-classed ones and wins five team ones.
        for _ in 0..5 {
            finish(&mut t, Some(1), 2, CLASSED);
        }
        assert_eq!(t.count(1, CLASS_BATTLE_CERTIFICATE), 1);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::insert(NAME, "classed", "1"));
        assert_eq!(writes[1], rows::update(NAME, "classed", "2"));
        for _ in 0..5 {
            finish(&mut t, Some(2), 1, NON_CLASSED);
        }
        assert_eq!(t.count(1, CLASS_FREE_BATTLE_CERTIFICATE), 1);
        assert!(t.quest_page(1, manager).contains("31688-s2.html"));
        for _ in 0..5 {
            finish(&mut t, Some(1), 2, TEAMS);
        }
        assert_eq!(t.count(1, TEAM_EVENT_CERTIFICATE), 1);
        // Player 2 never started the quest: nothing was counted.
        assert_eq!(t.count(2, CLASS_BATTLE_CERTIFICATE), 0);

        t.take_writes();
        assert!(t.quest_page(1, manager).contains("31688-04.html"));
        assert_eq!(t.count(1, OLY_CHEST), 4);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::delete(NAME, false));
        assert!(t.quest_page(1, manager).contains("31688-05.html"));
    }

    #[test]
    fn the_incomplete_reward_is_one_chest_per_certificate() {
        let (mut t, manager) = setup();
        noble(&mut t, 1, 80);
        t.quest_page(1, manager);
        t.event(1, NAME, "31688-03.html");
        // Asked for before any certificate: the "no quest" message.
        assert!(!t.event(1, NAME, "31688-04.html").contains("31688-04.html"));
        for _ in 0..5 {
            finish(&mut t, None, 1, TEAMS);
        }
        for _ in 0..5 {
            finish(&mut t, None, 1, NON_CLASSED);
        }
        assert!(t.quest_page(1, manager).contains("31688-s2.html"));
        assert!(t.event(1, NAME, "31688-04.html").contains("31688-04.html"));
        assert_eq!(t.count(1, OLY_CHEST), 2);
    }
}
