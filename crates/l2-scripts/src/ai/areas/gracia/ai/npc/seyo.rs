//! Seyo, the gambler of the Seed of Annihilation: Spirit Stone Fragments for luck.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/Seyo/Seyo.java

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SEYO: i32 = 32737;

// Item
const STONE_FRAGMENT: i32 = 15486; // Spirit Stone Fragment

// Misc
const TEXT: [NpcStringId; 5] = [
    NpcStringId::NoOneElseDonTWorryIDonTBiteHaha,
    NpcStringId::OkMasterOfLuckThatSYouHahaWellAnyoneCanComeAfterAll,
    NpcStringId::SheddingBloodIsAGivenOnTheBattlefieldAtLeastItSSafeHere,
    NpcStringId::OkWhoSNextItAllDependsOnYourFateAndLuckRightAtLeastComeAndTakeALook,
    NpcStringId::ThereWasSomeoneWhoWon10000FromMeAWarriorShouldnTJustBeGoodAtFightingRightYouVeGottaBeGoodInEverything,
];

/// The script values of Seyo (Java `npc.getScriptValue()`), by NPC object.
#[derive(Default)]
struct SeyoState {
    script_value: HashMap<ObjectId, i32>,
}

/// Java `ai.areas.Gracia.AI.NPC.Seyo.Seyo`.
pub struct Seyo;

impl Script for Seyo {
    fn name(&self) -> &'static str {
        "Seyo"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/Seyo"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SEYO]);
        s.add_talk_id(&[SEYO]);
        s.add_first_talk_id(&[SEYO]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        let npc = npc?;
        let busy = ctx.state::<SeyoState>().script_value.get(&npc.object_id).copied() == Some(1);

        match event {
            "TRICKERY_TIMER" => {
                if busy {
                    ctx.state::<SeyoState>().script_value.insert(npc.object_id, 0);
                    let text = ctx.get_random_entry(&TEXT)?;
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
                }
            }
            "give1" => {
                let player = player?;
                if busy {
                    htmltext = Some("32737-04.html".to_owned());
                } else if !ctx.has_quest_items(player, STONE_FRAGMENT) {
                    htmltext = Some("32737-01.html".to_owned());
                } else {
                    ctx.state::<SeyoState>().script_value.insert(npc.object_id, 1);
                    ctx.take_items(player, STONE_FRAGMENT, 1);
                    let name = ctx.player_name(player);
                    if ctx.get_random(100) == 0 {
                        ctx.give_items(player, STONE_FRAGMENT, 100);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::AmazingS1Took100OfTheseSoulStoneFragmentsWhatACompleteSwindler,
                            &[&name],
                        );
                    } else {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::HmmHeyDidYouGiveS1SomethingButItWasJust1Haha,
                            &[&name],
                        );
                    }

                    ctx.start_quest_timer("TRICKERY_TIMER", 5000, Some(npc), None, false);
                }
            }
            "give5" => {
                let player = player?;
                if busy {
                    htmltext = Some("32737-04.html".to_owned());
                } else if ctx.get_quest_items_count(player, STONE_FRAGMENT) < 5 {
                    htmltext = Some("32737-02.html".to_owned());
                } else {
                    ctx.state::<SeyoState>().script_value.insert(npc.object_id, 1);
                    ctx.take_items(player, STONE_FRAGMENT, 5);
                    let name = ctx.player_name(player);
                    let chance = ctx.get_random(100);
                    if chance < 20 {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::AhemS1HasNoLuckAtAllTryPraying,
                            &[&name],
                        );
                    } else if chance < 80 {
                        ctx.give_items(player, STONE_FRAGMENT, 1);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::ItSBetterThanLosingItAllRightOrDoesThisFeelWorse,
                            &[],
                        );
                    } else {
                        let item_count = ctx.get_random_range(10, 16);
                        ctx.give_items(player, STONE_FRAGMENT, i64::from(item_count));
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::S1PulledOneWithS2DigitsLuckyNotBad,
                            &[&name, &item_count.to_string()],
                        );
                    }

                    ctx.start_quest_timer("TRICKERY_TIMER", 5000, Some(npc), None, false);
                }
            }
            "give20" => {
                let player = player?;
                if busy {
                    htmltext = Some("32737-04.html".to_owned());
                } else if ctx.get_quest_items_count(player, STONE_FRAGMENT) < 20 {
                    htmltext = Some("32737-03.html".to_owned());
                } else {
                    ctx.state::<SeyoState>().script_value.insert(npc.object_id, 1);
                    ctx.take_items(player, STONE_FRAGMENT, 20);
                    let name = ctx.player_name(player);
                    let chance = ctx.get_random(10000);
                    if chance == 0 {
                        ctx.give_items(player, STONE_FRAGMENT, 10000);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::AhItSOverWhatKindOfGuyIsThatDamnFineYouS1TakeItAndGetOuttaHere,
                            &[&name],
                        );
                    } else if chance < 10 {
                        ctx.give_items(player, STONE_FRAGMENT, 1);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::YouDonTFeelBadRightAreYouSadButDonTCry,
                            &[],
                        );
                    } else {
                        let count = ctx.get_random_range(1, 100);
                        ctx.give_items(player, STONE_FRAGMENT, i64::from(count));
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::ABigPieceIsMadeUpOfLittlePiecesSoHereSALittlePiece,
                            &[],
                        );
                    }

                    ctx.start_quest_timer("TRICKERY_TIMER", 5000, Some(npc), None, false);
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32737" type="Folk" level="70" name="Seyo"/>"#;
    const ITEMS: &str = r#"<item id="15486" type="EtcItem" name="Spirit Stone Fragment"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/Seyo";

    fn setup(fragments: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Seyo);
        let links: String = ["give1", "give5", "give20"]
            .iter()
            .map(|e| format!(r#"<a action="bypass -h Script Seyo {e}">x</a>"#))
            .collect();
        let mut pages = vec![(
            format!("{DIR}/32737.html"),
            format!("<html><body>32737 {links}</body></html>"),
        )];
        for n in 1..=4 {
            pages.push((
                format!("{DIR}/32737-0{n}.html"),
                format!("<html><body>32737-0{n} {links}</body></html>"),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(SEYO, 20);
        let items = if fragments > 0 {
            vec![item(1000, STONE_FRAGMENT, fragments)]
        } else {
            vec![]
        };
        t.enter(1, 80, items);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32737 "));
        (t, npc)
    }

    #[test]
    fn too_few_fragments_get_the_refusal_pages() {
        let (mut t, _) = setup(0);
        t.bypass(1, "Script Seyo give1");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32737-01"));
        t.bypass(1, "Script Seyo give5");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32737-02"));
        t.bypass(1, "Script Seyo give20");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32737-03"));
    }

    #[test]
    fn a_bet_takes_the_stake_and_the_gambler_rests_five_seconds() {
        let (mut t, _) = setup(20);
        t.bypass(1, "Script Seyo give20");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"NPC_SAY".to_owned()));
        // The stake is gone; the prize is 0 (lost), 1, 1..100 or 10000 back.
        let left = t.count(1, STONE_FRAGMENT);
        assert!((0..=10_000).contains(&left), "{left}");
        // While Seyo rests, no new bet.
        t.bypass(1, "Script Seyo give1");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32737-04"));
        // After 5 seconds (100 ticks of 50 ms) the gambler talks again and takes bets.
        t.ticks(110);
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"NPC_SAY".to_owned()));
    }
}
