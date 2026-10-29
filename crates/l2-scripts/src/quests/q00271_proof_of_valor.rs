//! Quest 271: Proof of Valor (Rukain in the Orc Village sends the orcs after the kasha wolves).
// Java: dist/game/data/scripts/quests/Q00271_ProofOfValor/Q00271_ProofOfValor.java
//
// Pattern: a kill-count quest with a double drop (`docs/design/scripts.md`, "Quest patterns"):
// every kasha wolf gives one fang, sometimes two, until 50 are in; the 50th sets cond 2 and
// Rukain gives a necklace of valor (with 10 healing potions, 13% of the time) or of courage.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RUKAIN: i32 = 30577;

// Items
const KASHA_WOLF_FANG: i32 = 1473;

// Monster
const KASHA_WOLF: i32 = 20475;

// Rewards
const HEALING_POTION: i32 = 1061;
const NECKLACE_OF_COURAGE: i32 = 1506;
const NECKLACE_OF_VALOR: i32 = 1507;

// Misc
const MIN_LEVEL: i32 = 4;

/// Java `Q00271_ProofOfValor`.
pub struct Q00271ProofOfValor;

impl Script for Q00271ProofOfValor {
    fn name(&self) -> &'static str {
        "Q00271_ProofOfValor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00271_ProofOfValor"
    }

    fn id(&self) -> i32 {
        271
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RUKAIN]);
        s.add_talk_id(&[RUKAIN]);
        s.add_kill_id(&[KASHA_WOLF]);
        s.register_quest_items(&[KASHA_WOLF_FANG]);
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
        if event.eq_ignore_ascii_case("30577-04.htm") {
            qs.start_quest(ctx);
            return Some(
                if ctx.has_at_least_one_quest_item(player, &[NECKLACE_OF_VALOR, NECKLACE_OF_COURAGE]) {
                    "30577-08.html".to_owned()
                } else {
                    event.to_owned()
                },
            );
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            let count = ctx.get_quest_items_count(killer, KASHA_WOLF_FANG);
            // Java rolls before it looks at the count (both operands of `&&` are read in order).
            let double = ctx.get_random(100) < 25;
            let amount = if double && count < 49 { 2 } else { 1 };
            ctx.give_items(killer, KASHA_WOLF_FANG, amount);
            if count + amount >= 50 {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ORC {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_at_least_one_quest_item(player, &[NECKLACE_OF_VALOR, NECKLACE_OF_COURAGE])
                        {
                            "30577-07.htm"
                        } else {
                            "30577-03.htm"
                        }
                    } else {
                        "30577-02.htm"
                    }
                } else {
                    "30577-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30577-05.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, KASHA_WOLF_FANG) >= 50 {
                        if ctx.get_random(100) <= 13 {
                            ctx.reward_items(player, NECKLACE_OF_VALOR, 1);
                            ctx.give_items(player, HEALING_POTION, 10);
                        } else {
                            ctx.reward_items(player, NECKLACE_OF_COURAGE, 1);
                        }
                        ctx.take_items(player, KASHA_WOLF_FANG, -1);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30577-06.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00271_ProofOfValor";

    const NPCS: &str = r#"
        <npc id="30577" type="Folk" level="70" name="Rukain"/>
        <npc id="20475" type="Monster" level="6" name="Kasha Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1473" type="EtcItem" name="Kasha Wolf Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1061" type="EtcItem" name="Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="1506" type="Armor" name="Necklace of Courage"><set name="bodypart" val="neck"/></item>
        <item id="1507" type="Armor" name="Necklace of Valor"><set name="bodypart" val="neck"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00271ProofOfValor);
        let pages = Pages::of(&Q00271ProofOfValor)
            .page("30577-03.htm", &["30577-04.htm"])
            .page("30577-07.htm", &["30577-04.htm"])
            .plain(&[
                "30577-01.htm",
                "30577-02.htm",
                "30577-04.htm",
                "30577-05.html",
                "30577-06.html",
                "30577-08.html",
            ])
            .npcs(&[RUKAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rukain = t.spawn(RUKAIN, 20);
        (t, rukain)
    }

    /// Kills kasha wolves until the quest is at cond 2 (50 fangs); returns the kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let wolf = t.spawn(KASHA_WOLF, 60);
            t.kill(1, wolf);
            if t.count(1, KASHA_WOLF_FANG) >= 50 {
                return kills;
            }
        }
        panic!("no 50 fangs after 400 kills");
    }

    #[test]
    fn orcs_of_level_four_only() {
        let (mut t, rukain) = setup();
        t.enter_as(1, 4, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, rukain).contains("30577-01.htm"));
        t.enter_as(2, 3, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, rukain).contains("30577-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_fangs_then_a_necklace() {
        let (mut t, rukain) = setup();
        t.enter_as(1, 4, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, rukain).contains("30577-03.htm"));
        assert!(t.event(1, NAME, "30577-04.htm").contains("30577-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, rukain).contains("30577-05.html"));

        // Cond 2 comes with the 50th fang (`setCond(2, true)`).
        hunt(&mut t);
        assert!(t.count(1, KASHA_WOLF_FANG) >= 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // One necklace, valor with 10 potions or courage; the fangs go, `exitQuest(true, true)`.
        assert!(t.quest_page(1, rukain).contains("30577-06.html"));
        let valor = t.count(1, NECKLACE_OF_VALOR);
        let courage = t.count(1, NECKLACE_OF_COURAGE);
        assert_eq!(valor + courage, 1);
        assert_eq!(t.count(1, HEALING_POTION), valor * 10);
        assert_eq!(t.count(1, KASHA_WOLF_FANG), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn with_a_necklace_already_the_quest_starts_on_the_last_page() {
        let (mut t, rukain) = setup();
        t.enter_as(
            1,
            4,
            race::ORC,
            player_class::ORC_FIGHTER,
            vec![item(900, NECKLACE_OF_COURAGE, 1)],
        );
        assert!(t.quest_page(1, rukain).contains("30577-07.htm"));
        // The start still happens; the page offered is 08.
        assert!(t.event(1, NAME, "30577-04.htm").contains("30577-08.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }
}
