//! Quest 278: Home Security (Tunatun in the Gludin area sends level 82 players after the sel mahum manes).
// Java: dist/game/data/scripts/quests/Q00278_HomeSecurity/Q00278_HomeSecurity.java
//
// Pattern: a party collect quest with a count target (`docs/design/scripts.md`, "Quest patterns"):
// the Farm Ravager and the bandits and beasts drop sel mahum manes for a random party member in
// cond 1, the 300th sets cond 2; Tunatun gives one random reward out of a table and ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TUNATUN: i32 = 31537;

// Monsters
const FARM_RAVAGER: i32 = 18905;
const FARM_BANDIT: i32 = 18906;
const BEAST_DEVOURER: i32 = 18907;

// Item
const SEL_MAHUM_MANE: i32 = 15531;

// Misc
const SEL_MAHUM_MANE_COUNT: i64 = 300;

/// Java `onEvent` "31537-07.html": the reward of a roll of 100, as `(item, count)`.
fn reward(roll: i32) -> (i32, i64) {
    match roll {
        r if r < 10 => (960, 1),
        r if r < 19 => (960, 2),
        r if r < 27 => (960, 3),
        r if r < 34 => (960, 4),
        r if r < 40 => (960, 5),
        r if r < 45 => (960, 6),
        r if r < 49 => (960, 7),
        r if r < 52 => (960, 8),
        r if r < 54 => (960, 9),
        r if r < 55 => (960, 10),
        r if r < 75 => (9553, 1),
        r if r < 90 => (9553, 2),
        _ => (959, 1),
    }
}

/// Java `Q00278_HomeSecurity`.
pub struct Q00278HomeSecurity;

impl Script for Q00278HomeSecurity {
    fn name(&self) -> &'static str {
        "Q00278_HomeSecurity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00278_HomeSecurity"
    }

    fn id(&self) -> i32 {
        278
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TUNATUN]);
        s.add_talk_id(&[TUNATUN]);
        s.add_kill_id(&[FARM_RAVAGER, FARM_BANDIT, BEAST_DEVOURER]);
        s.register_quest_items(&[SEL_MAHUM_MANE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // NOTE(java-quirk): without a quest state Java returns the event itself as the text.
        let mut htmltext = Some(event.to_owned());
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return htmltext;
        };
        match event {
            "31537-02.htm" => {
                htmltext = Some(
                    if ctx.player_level(player) >= 82 {
                        "31537-02.htm"
                    } else {
                        "31537-03.html"
                    }
                    .to_owned(),
                );
            }
            "31537-04.htm" => qs.start_quest(ctx),
            "31537-07.html" => {
                let (item, count) = reward(ctx.get_random(100));
                ctx.give_items(player, item, count);
                qs.exit_quest(ctx, true, true);
                htmltext = Some("31537-07.html".to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.random_party_member_state(player, 1, 3, Some(npc)) else {
            return;
        };
        let member_player = member.player();
        match npc.id {
            FARM_RAVAGER => {
                // Java evaluates the first roll before the amount is chosen.
                let amount = i64::from(if ctx.get_random(1000) < 486 {
                    ctx.get_random(6) + 1
                } else {
                    ctx.get_random(5) + 1
                });
                if ctx.give_item_randomly(
                    member_player,
                    Some(npc),
                    SEL_MAHUM_MANE,
                    amount,
                    amount,
                    SEL_MAHUM_MANE_COUNT,
                    1.0,
                    true,
                ) {
                    // `setCond(2, true)` on the member drawn.
                    member.set_cond(ctx, 2, true);
                }
            }
            FARM_BANDIT | BEAST_DEVOURER => {
                if ctx.give_item_randomly(
                    member_player,
                    Some(npc),
                    SEL_MAHUM_MANE,
                    1,
                    1,
                    SEL_MAHUM_MANE_COUNT,
                    0.85,
                    true,
                ) {
                    member.set_cond(ctx, 2, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            htmltext = "31537-01.htm".to_owned();
        } else if qs.state(ctx) == STARTED {
            let manes = ctx.get_quest_items_count(player, SEL_MAHUM_MANE);
            if qs.is_cond(ctx, 1) || manes < SEL_MAHUM_MANE_COUNT {
                htmltext = "31537-06.html".to_owned();
            } else if qs.is_cond(ctx, 2) && manes >= SEL_MAHUM_MANE_COUNT {
                htmltext = "31537-05.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00278_HomeSecurity";

    const NPCS: &str = r#"
        <npc id="31537" type="Folk" level="70" name="Tunatun"/>
        <npc id="18905" type="Monster" level="50" name="Farm Ravager"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15531" type="EtcItem" name="Sel Mahum Mane"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="959" type="EtcItem" name="Reward C"/>
        <item id="960" type="EtcItem" name="Reward D"><set name="is_stackable" val="true"/></item>
        <item id="9553" type="EtcItem" name="Reward E"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00278HomeSecurity);
        let pages = Pages::of(&Q00278HomeSecurity)
            .page("31537-01.htm", &["31537-02.htm"])
            .page("31537-02.htm", &["31537-04.htm"])
            .plain(&["31537-03.html", "31537-04.htm", "31537-05.html", "31537-06.html"])
            .page("31537-05.html", &["31537-07.html"])
            .plain(&["31537-07.html"])
            .npcs(&[TUNATUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tunatun = t.spawn(TUNATUN, 20);
        (t, tunatun)
    }

    #[test]
    fn a_character_below_level_82_is_turned_away() {
        let (mut t, tunatun) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-01.htm"));
        assert!(t.event(1, NAME, "31537-02.htm").contains("31537-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_hundred_manes_then_one_reward() {
        let (mut t, tunatun) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-01.htm"));
        assert!(t.event(1, NAME, "31537-02.htm").contains("31537-02.htm"));
        assert!(t.event(1, NAME, "31537-04.htm").contains("31537-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, tunatun).contains("31537-06.html"));

        // The 300th mane sets cond 2 (`setCond(2, true)`); the limit clamps the last drop.
        let mut kills = 0;
        while t.take_writes().is_empty() {
            kills += 1;
            assert!(kills < 1000, "no 300 manes after {kills} kills");
            let ravager = t.spawn(FARM_RAVAGER, 60);
            t.kill(1, ravager);
        }
        assert_eq!(t.count(1, SEL_MAHUM_MANE), SEL_MAHUM_MANE_COUNT);

        // Tunatun: one reward from the table and `exitQuest(true, true)`.
        assert!(t.quest_page(1, tunatun).contains("31537-05.html"));
        assert!(t.event(1, NAME, "31537-07.html").contains("31537-07.html"));
        assert_eq!(t.count(1, SEL_MAHUM_MANE), 0);
        let total: i64 = [959, 960, 9553].iter().map(|&item| t.count(1, item)).sum();
        assert!(total >= 1, "{total}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
