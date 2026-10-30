//! Quest 146: The Zero Hour (Kahman at the Hot Springs sends the player after Queen Shyeed).
// Java: dist/game/data/scripts/quests/Q00146_TheZeroHour/Q00146_TheZeroHour.java
//
// Pattern: a one-time kill quest. A random party member of the killer gets the Fang at cond 2
// (`get_random_party_member_cond`); the start NPC hands out the supply box and the reward when
// the Fang is in. The level gate and the prerequisite quest are read from the other quest's
// state (`player_quest_state`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KAHMAN: i32 = 31554;

// Monster
const QUEEN_SHYEED: i32 = 25671;

// Items
const KAHMANS_SUPPLY_BOX: i32 = 14849;
const FANG: i32 = 14859;

// Misc
const MIN_LEVEL: i32 = 81;
const PREVIOUS_QUEST: &str = "Q00109_InSearchOfTheNest";
const REWARD_EXP: i64 = 154_616;
const REWARD_SP: i32 = 12_500;

/// Java `Q00146_TheZeroHour`.
pub struct Q00146TheZeroHour;

impl Script for Q00146TheZeroHour {
    fn name(&self) -> &'static str {
        "Q00146_TheZeroHour"
    }

    fn path(&self) -> &'static str {
        "quests/Q00146_TheZeroHour"
    }

    fn id(&self) -> i32 {
        146
    }

    /// NOTE(java-quirk): Java's `getNpcStringId()` returns 640 (the quest's name in the quest
    /// list), not the quest id.
    fn npc_string_id(&self) -> i32 {
        640
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KAHMAN]);
        s.add_talk_id(&[KAHMAN]);
        s.add_kill_id(&[QUEEN_SHYEED]);
        s.register_quest_items(&[FANG]);
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
        if event.eq_ignore_ascii_case("31554-03.htm") {
            qs.start_quest(ctx);
        }
        // NOTE(java-quirk): Java returns the event for every event, so any link of this
        // script is shown as a page (`showResult`).
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if !ctx.has_quest_items(member, FANG) {
            ctx.give_items(member, FANG, 1);
            if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "31554-02.htm".to_owned()
                } else {
                    let previous_done = ctx
                        .player_quest_state(player, PREVIOUS_QUEST)
                        .is_some_and(|prev| prev.is_completed());
                    if previous_done {
                        "31554-01a.htm".to_owned()
                    } else {
                        "31554-04.html".to_owned()
                    }
                };
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "31554-06.html".to_owned();
                } else {
                    ctx.give_items(player, KAHMANS_SUPPLY_BOX, 1);
                    ctx.add_exp_and_sp(player, REWARD_EXP, REWARD_SP);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "31554-05.html".to_owned();
                }
            }
            COMPLETED => htmltext = "31554-01b.htm".to_owned(),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{PlayerData, QuestLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q00146_TheZeroHour";

    /// Stand-in for Q00109 (not ported yet): the world only installs the rows of a known script.
    struct InSearchOfTheNestStandIn;

    impl Script for InSearchOfTheNestStandIn {
        fn name(&self) -> &'static str {
            PREVIOUS_QUEST
        }

        fn path(&self) -> &'static str {
            "quests/Q00109_InSearchOfTheNest"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    const NPCS: &str = r#"
        <npc id="31554" type="Folk" level="1" name="Kahman"/>
        <npc id="25671" type="RaidBoss" level="81" name="Queen Shyeed"/>"#;

    const ITEMS: &str = r#"
        <item id="14849" type="EtcItem" name="Kahman's Supply Box"/>
        <item id="14859" type="EtcItem" name="Fang"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00146TheZeroHour);
        registry.add(InSearchOfTheNestStandIn);
        let pages = Pages::of(&Q00146TheZeroHour)
            .page("31554-01a.htm", &["31554-03.htm"])
            .plain(&[
                "31554-01b.htm",
                "31554-02.htm",
                "31554-03.htm",
                "31554-04.html",
                "31554-05.html",
                "31554-06.html",
            ])
            .npcs(&[KAHMAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kahman = t.spawn(KAHMAN, 20);
        let queen = t.spawn(QUEEN_SHYEED, 40);
        (t, kahman, queen)
    }

    fn with_previous_completed(id: i32, level: i32) -> PlayerData {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(PREVIOUS_QUEST.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        data
    }

    #[test]
    fn a_character_below_level_81_is_sent_away() {
        let (mut t, kahman, _) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, kahman).contains("31554-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_previous_quest_the_character_is_sent_away() {
        let (mut t, kahman, _) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, kahman).contains("31554-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fang_from_queen_shyeed_and_the_reward() {
        let (mut t, kahman, queen) = setup();
        t.enter_with(with_previous_completed(1, 81));
        assert!(t.quest_page(1, kahman).contains("31554-01a.htm"));
        assert!(t.event(1, NAME, "31554-03.htm").contains("31554-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Still cond 1: Kahman has nothing to give yet.
        assert!(t.quest_page(1, kahman).contains("31554-06.html"));
        assert!(t.take_writes().is_empty());

        // Queen Shyeed drops the Fang and moves the quest to cond 2.
        t.kill(1, queen);
        assert_eq!(t.count(1, FANG), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Kahman: the box, the reward, `exitQuest(false, true)`.
        assert!(t.quest_page(1, kahman).contains("31554-05.html"));
        assert_eq!(t.count(1, KAHMANS_SUPPLY_BOX), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, kahman).contains("31554-01b.htm"));
    }
}
