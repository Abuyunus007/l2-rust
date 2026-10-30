//! Quest 266: Pleas of Pixies (Pixy Murika in the Elven Forest collects predator fangs for elves).
// Java: dist/game/data/scripts/quests/Q00266_PleasOfPixies/Q00266_PleasOfPixies.java
//
// Pattern: a kill-count quest with a graded drop (`docs/design/scripts.md`, "Quest patterns"): the
// keltirs and wolves drop predator fangs by a roll of ten (1 to 3 fangs per kill, up to 100); the
// 100th sets cond 2. Murika gives one of four reward packs by a roll of 100 and ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PIXY_MURIKA: i32 = 31852;

// Items
const PREDATORS_FANG: i32 = 1334;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `MONSTERS`: per monster, `(fangs, threshold)` pairs. The first pair whose threshold is
/// above the roll of ten gives its fangs.
fn fang_drops(npc_id: i32) -> &'static [(i64, i32)] {
    match npc_id {
        20537 => &[(2, 10)],         // Elder Red Keltir
        20525 => &[(2, 5), (3, 10)], // Gray Wolf
        20534 => &[(1, 6)],          // Red Keltir
        20530 => &[(1, 8)],          // Young Red Keltir
        _ => &[],
    }
}

/// Java `REWARDS`: four packs, `0` the jackpot.
fn reward_pack(reward: usize) -> &'static [(i32, i64)] {
    match reward {
        0 => &[(1337, 1), (3032, 1)], // Emerald, Recipe: Spiritshot D
        1 => &[(2176, 1), (1338, 1)], // Recipe: Leather Boots, Blue Onyx
        2 => &[(1339, 1), (1061, 1)], // Onyx, Greater Healing Potion
        _ => &[(1336, 1), (1060, 1)], // Glass Shard, Lesser Healing Potion
    }
}

/// Java `Q00266_PleasOfPixies`.
pub struct Q00266PleasOfPixies;

impl Script for Q00266PleasOfPixies {
    fn name(&self) -> &'static str {
        "Q00266_PleasOfPixies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00266_PleasOfPixies"
    }

    fn id(&self) -> i32 {
        266
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PIXY_MURIKA]);
        s.add_talk_id(&[PIXY_MURIKA]);
        s.add_kill_id(&[20537, 20525, 20534, 20530]);
        s.register_quest_items(&[PREDATORS_FANG]);
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
        if event == "31852-04.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            let chance = ctx.get_random(10);
            for &(fangs, threshold) in fang_drops(npc.id) {
                if chance < threshold {
                    // Java's 7-argument overload: `fangs` at a time, limit 100, always dropped.
                    if ctx.give_item_randomly(killer, Some(npc), PREDATORS_FANG, fangs, fangs, 100, 1.0, true)
                    {
                        // `setCond(2)`: no sound of its own.
                        qs.set_cond(ctx, 2, false);
                    }
                    break;
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) != race::ELF {
                    "31852-01.htm"
                } else if ctx.player_level(player) < MIN_LEVEL {
                    "31852-02.htm"
                } else {
                    "31852-03.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "31852-05.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, PREDATORS_FANG) >= 100 {
                        let chance = ctx.get_random(100);
                        let reward = if chance < 2 {
                            ctx.play_sound(player, QuestSound::ItemsoundQuestJackpot);
                            0
                        } else if chance < 20 {
                            1
                        } else if chance < 45 {
                            2
                        } else {
                            3
                        };
                        for &(item, count) in reward_pack(reward) {
                            ctx.reward_items(player, item, count);
                        }
                        qs.exit_quest(ctx, true, true);
                        htmltext = "31852-06.html".to_owned();
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

    const NAME: &str = "Q00266_PleasOfPixies";

    const NPCS: &str = r#"
        <npc id="31852" type="Folk" level="70" name="Pixy Murika"/>
        <npc id="20534" type="Monster" level="6" name="Red Keltir"/>"#;

    const ITEMS: &str = r#"
        <item id="1334" type="EtcItem" name="Predator's Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1337" type="EtcItem" name="Emerald"/>
        <item id="3032" type="EtcItem" name="Recipe: Spiritshot D"/>
        <item id="2176" type="EtcItem" name="Recipe: Leather Boots"/>
        <item id="1338" type="EtcItem" name="Blue Onyx"/>
        <item id="1339" type="EtcItem" name="Onyx"/>
        <item id="1061" type="EtcItem" name="Greater Healing Potion" />
        <item id="1336" type="EtcItem" name="Glass Shard"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00266PleasOfPixies);
        let pages = Pages::of(&Q00266PleasOfPixies)
            .page("31852-03.htm", &["31852-04.htm"])
            .plain(&[
                "31852-01.htm",
                "31852-02.htm",
                "31852-04.htm",
                "31852-05.html",
                "31852-06.html",
            ])
            .npcs(&[PIXY_MURIKA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let murika = t.spawn(PIXY_MURIKA, 20);
        (t, murika)
    }

    #[test]
    fn only_elves_of_level_three_can_take_it() {
        let (mut t, murika) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, murika).contains("31852-01.htm"));
        t.enter_as(2, 2, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, murika).contains("31852-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_fangs_then_one_of_the_four_packs() {
        let (mut t, murika) = setup();
        t.enter_as(1, 3, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, murika).contains("31852-03.htm"));
        assert!(t.event(1, NAME, "31852-04.htm").contains("31852-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, murika).contains("31852-05.html"));

        // The 100th fang sets cond 2 (`setCond(2)`, the limit clamps the last drop).
        for _ in 0..1000 {
            if t.count(1, PREDATORS_FANG) >= 100 {
                break;
            }
            let keltir = t.spawn(20534, 60);
            t.kill(1, keltir);
        }
        assert_eq!(t.count(1, PREDATORS_FANG), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // One pack: 1337 + 3032, 2176 + 1338, 1339 + 1061 or 1336 + 1060.
        assert!(t.quest_page(1, murika).contains("31852-06.html"));
        let packs = [
            [(1337, 1), (3032, 1)],
            [(2176, 1), (1338, 1)],
            [(1339, 1), (1061, 1)],
            [(1336, 1), (1060, 1)],
        ];
        let matches: Vec<usize> = packs
            .iter()
            .enumerate()
            .filter(|(_, pack)| pack.iter().all(|&(item, n)| t.count(1, item) == n))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!(t.count(1, PREDATORS_FANG), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
