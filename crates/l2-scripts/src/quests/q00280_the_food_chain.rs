//! Quest 280: The Food Chain (Bixon in the Town of Aden sells the keltir and wolf teeth).
// Java: dist/game/data/scripts/quests/Q00280_TheFoodChain/Q00280_TheFoodChain.java
//
// Pattern: a repeatable collect quest with a graded drop (`docs/design/scripts.md`, "Quest
// patterns"): the keltirs drop grey keltir teeth and the black wolves black wolf teeth, one to four
// a kill by a roll of a thousand. Bixon buys the teeth at 2 adena each, or, with 25 teeth, gives
// one of five reward items; the quest repeats.

use l2_util::ObjectId;
use l2_world::script::{CREATED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BIXON: i32 = 32175;

// Items
const GREY_KELTIR_TOOTH: i32 = 9809;
const BLACK_WOLF_TOOTH: i32 = 9810;

// Rewards
const REWARDS: [i32; 5] = [28, 35, 41, 48, 116];

// Misc
const MIN_LEVEL: i32 = 3;
const TEETH_COUNT: i64 = 25;

/// Java `MONSTER_ITEM`: the tooth each monster drops.
fn monster_item(npc_id: i32) -> Option<i32> {
    match npc_id {
        22229..=22231 => Some(GREY_KELTIR_TOOTH),
        22232 | 22233 => Some(BLACK_WOLF_TOOTH),
        _ => None,
    }
}

/// Java `MONSTER_CHANCE`: `(threshold out of 1000, teeth)` pairs, the first pair above the roll
/// gives its teeth.
fn monster_chance(npc_id: i32) -> &'static [(i32, i64)] {
    match npc_id {
        22229 => &[(1000, 1)],
        22230 => &[(500, 1), (1000, 2)],
        22231 => &[(1000, 2)],
        22232 => &[(1000, 3)],
        22233 => &[(500, 3), (1000, 4)],
        _ => &[],
    }
}

/// Java `Q00280_TheFoodChain`.
pub struct Q00280TheFoodChain;

impl Script for Q00280TheFoodChain {
    fn name(&self) -> &'static str {
        "Q00280_TheFoodChain"
    }

    fn path(&self) -> &'static str {
        "quests/Q00280_TheFoodChain"
    }

    fn id(&self) -> i32 {
        280
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BIXON]);
        s.add_talk_id(&[BIXON]);
        s.add_kill_id(&[22229, 22230, 22231, 22232, 22233]);
        s.register_quest_items(&[GREY_KELTIR_TOOTH, BLACK_WOLF_TOOTH]);
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
        let teeth = [GREY_KELTIR_TOOTH, BLACK_WOLF_TOOTH];
        let mut htmltext = None;
        match event {
            "32175-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32175-06.html" => {
                if ctx.has_at_least_one_quest_item(player, &teeth) {
                    let grey = ctx.get_quest_items_count(player, GREY_KELTIR_TOOTH);
                    let black = ctx.get_quest_items_count(player, BLACK_WOLF_TOOTH);
                    ctx.give_adena(player, 2 * (grey + black), true);
                    ctx.take_items_many(player, -1, &teeth);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32175-07.html".to_owned());
                }
            }
            "32175-08.html" => htmltext = Some(event.to_owned()),
            "32175-09.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "32175-11.html" => {
                let grey = ctx.get_quest_items_count(player, GREY_KELTIR_TOOTH);
                let black = ctx.get_quest_items_count(player, BLACK_WOLF_TOOTH);
                if grey + black >= TEETH_COUNT {
                    if grey >= TEETH_COUNT {
                        ctx.take_items(player, GREY_KELTIR_TOOTH, TEETH_COUNT);
                    } else {
                        ctx.take_items(player, GREY_KELTIR_TOOTH, grey);
                        ctx.take_items(player, BLACK_WOLF_TOOTH, TEETH_COUNT - grey);
                    }
                    let reward = REWARDS[ctx.get_random(5) as usize];
                    ctx.reward_items(player, reward, 1);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32175-10.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            let chance = ctx.get_random(1000);
            for &(threshold, teeth) in monster_chance(npc.id) {
                if chance < threshold {
                    if let Some(item) = monster_item(npc.id) {
                        // Java's 6-argument overload: no NPC, no limit, always dropped.
                        ctx.give_item_randomly(killer, None, item, teeth, teeth, 0, 1.0, true);
                    }
                    break;
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let teeth = [GREY_KELTIR_TOOTH, BLACK_WOLF_TOOTH];
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                    "32175-01.htm"
                } else {
                    "32175-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_at_least_one_quest_item(talker, &teeth) {
                    "32175-05.html"
                } else {
                    "32175-04.html"
                }
                .to_owned();
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
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00280_TheFoodChain";

    const NPCS: &str = r#"
        <npc id="32175" type="Folk" level="70" name="Bixon"/>
        <npc id="22229" type="Monster" level="30" name="Grey Keltir"/>
        <npc id="22232" type="Monster" level="30" name="Black Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9809" type="EtcItem" name="Grey Keltir Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9810" type="EtcItem" name="Black Wolf Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="28" type="EtcItem" name="Reward 28"/>
        <item id="35" type="EtcItem" name="Reward 35"/>
        <item id="41" type="EtcItem" name="Reward 41"/>
        <item id="48" type="EtcItem" name="Reward 48"/>
        <item id="116" type="EtcItem" name="Reward 116"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00280TheFoodChain);
        let pages = Pages::of(&Q00280TheFoodChain)
            .page("32175-01.htm", &["32175-03.htm"])
            .plain(&[
                "32175-02.htm",
                "32175-03.htm",
                "32175-04.html",
                "32175-07.html",
                "32175-10.html",
                "32175-08.html",
                "32175-09.html",
            ])
            .page("32175-05.html", &["32175-06.html", "32175-11.html"])
            .page("32175-06.html", &["32175-08.html", "32175-09.html"])
            .page("32175-11.html", &["32175-08.html", "32175-09.html"])
            .npcs(&[BIXON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let bixon = t.spawn(BIXON, 20);
        (t, bixon)
    }

    #[test]
    fn a_level_two_character_is_sent_away() {
        let (mut t, bixon) = setup();
        t.enter_as(1, 2, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, bixon).contains("32175-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn twenty_five_teeth_for_a_reward_then_teeth_for_adena() {
        let (mut t, bixon) = setup();
        t.enter_as(1, 3, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, bixon).contains("32175-01.htm"));
        assert!(t.event(1, NAME, "32175-03.htm").contains("32175-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, bixon).contains("32175-04.html"));

        // One grey tooth a kill; 25 of them buy one reward (the teeth go).
        for _ in 0..25 {
            let keltir = t.spawn(22229, 60);
            t.kill(1, keltir);
        }
        assert_eq!(t.count(1, GREY_KELTIR_TOOTH), 25);
        assert!(t.quest_page(1, bixon).contains("32175-05.html"));
        assert!(t.event(1, NAME, "32175-11.html").contains("32175-11.html"));
        assert_eq!(t.count(1, GREY_KELTIR_TOOTH), 0);
        let rewards: i64 = REWARDS.iter().map(|&item| t.count(1, item)).sum();
        assert_eq!(rewards, 1);
        assert!(t.take_writes().is_empty());

        // Black wolves: three teeth a kill, sold for 2 adena each.
        for _ in 0..5 {
            let wolf = t.spawn(22232, 60);
            t.kill(1, wolf);
        }
        assert_eq!(t.count(1, BLACK_WOLF_TOOTH), 15);
        assert!(t.quest_page(1, bixon).contains("32175-05.html"));
        assert!(t.event(1, NAME, "32175-06.html").contains("32175-06.html"));
        assert_eq!(t.count(1, 57), 30);
        assert_eq!(t.count(1, BLACK_WOLF_TOOTH), 0);

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32175-09.html").contains("32175-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
