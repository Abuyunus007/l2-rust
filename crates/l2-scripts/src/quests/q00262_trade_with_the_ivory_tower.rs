//! Quest 262: Trade With The Ivory Tower (Vollodos in the Town of Aden sends the player after the fungi).
// Java: dist/game/data/scripts/quests/Q00262_TradeWithTheIvoryTower/Q00262_TradeWithTheIvoryTower.java
//
// Pattern: a party kill quest (`docs/design/scripts.md`, "Quest patterns"): a random party member
// in cond 1 is drawn on each fungus kill and the spore sac roll is made for the killer. The 10th
// sac sets cond 2 and Vollodos pays 3000 adena and ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VOLLODOS: i32 = 30137;

// Item
const SPORE_SAC: i32 = 707;

// Misc
const MIN_LEVEL: i32 = 8;
const REQUIRED_ITEM_COUNT: i64 = 10;

/// Java `MOBS_SAC`: the fungi and the roll of ten a sac drops on (`getRandom(10) < n`).
fn sac_chance(npc_id: i32) -> i32 {
    match npc_id {
        20007 => 3, // Green Fungus
        20400 => 4, // Blood Fungus
        _ => 0,
    }
}

/// Java `Q00262_TradeWithTheIvoryTower`.
pub struct Q00262TradeWithTheIvoryTower;

impl Script for Q00262TradeWithTheIvoryTower {
    fn name(&self) -> &'static str {
        "Q00262_TradeWithTheIvoryTower"
    }

    fn path(&self) -> &'static str {
        "quests/Q00262_TradeWithTheIvoryTower"
    }

    fn id(&self) -> i32 {
        262
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VOLLODOS]);
        s.add_talk_id(&[VOLLODOS]);
        s.add_kill_id(&[20007, 20400]);
        s.register_quest_items(&[SPORE_SAC]);
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
        if event.eq_ignore_ascii_case("30137-03.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 1) else {
            return;
        };
        let qs = ctx.get_quest_state(party_member, false);
        if ctx.get_random(10) < sac_chance(npc.id) {
            // NOTE(java-quirk): the sac goes to the killer, the cond 2 goes to the party member
            // drawn above (Java passes `player` to `rewardItems` and `partyMember` to `setCond`).
            ctx.reward_items(player, SPORE_SAC, 1);
            if ctx.get_quest_items_count(player, SPORE_SAC) >= REQUIRED_ITEM_COUNT {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 2, true);
                }
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30137-02.htm"
                } else {
                    "30137-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    if ctx.get_quest_items_count(player, SPORE_SAC) < REQUIRED_ITEM_COUNT {
                        htmltext = "30137-04.html".to_owned();
                    }
                }
                2 => {
                    if ctx.get_quest_items_count(player, SPORE_SAC) >= REQUIRED_ITEM_COUNT {
                        htmltext = "30137-05.html".to_owned();
                        ctx.give_adena(player, 3000, true);
                        qs.exit_quest(ctx, true, true);
                    }
                }
                _ => {}
            },
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

    const NAME: &str = "Q00262_TradeWithTheIvoryTower";

    const NPCS: &str = r#"
        <npc id="30137" type="Folk" level="70" name="Vollodos"/>
        <npc id="20007" type="Monster" level="3" name="Green Fungus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="707" type="EtcItem" name="Spore Sac"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00262TradeWithTheIvoryTower);
        let pages = Pages::of(&Q00262TradeWithTheIvoryTower)
            .page("30137-02.htm", &["30137-03.htm"])
            .plain(&["30137-01.htm", "30137-03.htm", "30137-04.html", "30137-05.html"])
            .npcs(&[VOLLODOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let vollodos = t.spawn(VOLLODOS, 20);
        (t, vollodos)
    }

    /// Kills Green Fungi with `killer` until `player` has `sacs` spore sacs; returns the kills.
    fn hunt(t: &mut ScriptTest, killer: i32, player: i32, sacs: i64) -> usize {
        for kills in 1..400 {
            if t.count(player, SPORE_SAC) >= sacs {
                return kills;
            }
            let fungus = t.spawn(20007, 60);
            t.kill(killer, fungus);
        }
        panic!("no {sacs} sacs after 400 kills");
    }

    #[test]
    fn a_level_seven_character_is_sent_away() {
        let (mut t, vollodos) = setup();
        t.enter_as(1, 7, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, vollodos).contains("30137-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_sacs_then_three_thousand_adena() {
        let (mut t, vollodos) = setup();
        t.enter_as(1, 8, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, vollodos).contains("30137-02.htm"));
        assert!(t.event(1, NAME, "30137-03.htm").contains("30137-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, vollodos).contains("30137-04.html"));

        // The 10th sac sets cond 2 (`setCond(2, true)`).
        hunt(&mut t, 1, 1, REQUIRED_ITEM_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Vollodos pays 3000 adena; `exitQuest(true, true)`.
        assert!(t.quest_page(1, vollodos).contains("30137-05.html"));
        assert_eq!(t.count(1, 57), 3000);
        assert_eq!(t.count(1, SPORE_SAC), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_sac_goes_to_the_killer_not_the_party_member() {
        let (mut t, _vollodos) = setup();
        t.enter_as(1, 8, race::HUMAN, player_class::FIGHTER, vec![]);
        t.enter_as(2, 8, race::HUMAN, player_class::FIGHTER, vec![]);
        t.party(1, 2);
        // Player 1 has the quest; player 2 kills the fungi.
        let vollodos = t.npcs_of(VOLLODOS)[0];
        t.talk(1, vollodos);
        t.bypass(1, "Script");
        t.take();
        assert!(t.event(1, NAME, "30137-03.htm").contains("30137-03.htm"));
        t.take_writes();
        hunt(&mut t, 2, 2, 1);
        assert_eq!(t.count(1, SPORE_SAC), 0);
        assert!(t.count(2, SPORE_SAC) >= 1);
    }
}
