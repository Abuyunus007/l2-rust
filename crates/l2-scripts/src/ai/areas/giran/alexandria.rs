//! Alexandria, the Giran agathion seller: a pile of items for a random Little Angel or Devil.
// Java: dist/game/data/scripts/ai/areas/Giran/Alexandria/Alexandria.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALEXANDRIA: i32 = 30098;
// Items: (item id, count)
const REQUIRED_ITEMS: [(i32, i64); 7] = [
    (57, 7500000),
    (5094, 50),
    (6471, 25),
    (9814, 4),
    (9815, 3),
    (9816, 5),
    (9817, 5),
];

/// Java `ItemChanceHolder` (count 1) and `AdditionalItemChanceHolder` (the extra item, 0 for none).
struct Agathion {
    id: i32,
    chance: i32,
    additional_id: i32,
}

// Agathions
const LITTLE_DEVILS: [Agathion; 6] = [
    Agathion {
        id: 10321,
        chance: 600,
        additional_id: 10408,
    },
    Agathion {
        id: 10322,
        chance: 10,
        additional_id: 0,
    },
    Agathion {
        id: 10323,
        chance: 10,
        additional_id: 0,
    },
    Agathion {
        id: 10324,
        chance: 5,
        additional_id: 0,
    },
    Agathion {
        id: 10325,
        chance: 5,
        additional_id: 0,
    },
    Agathion {
        id: 10326,
        chance: 370,
        additional_id: 0,
    },
];
const LITTLE_ANGELS: [Agathion; 6] = [
    Agathion {
        id: 10315,
        chance: 600,
        additional_id: 10408,
    },
    Agathion {
        id: 10316,
        chance: 10,
        additional_id: 0,
    },
    Agathion {
        id: 10317,
        chance: 10,
        additional_id: 0,
    },
    Agathion {
        id: 10318,
        chance: 5,
        additional_id: 0,
    },
    Agathion {
        id: 10319,
        chance: 5,
        additional_id: 0,
    },
    Agathion {
        id: 10320,
        chance: 370,
        additional_id: 0,
    },
];

/// Java `ai.areas.Giran.Alexandria.Alexandria`.
pub struct Alexandria;

/// Java `takeAllItems(player, REQUIRED_ITEMS)`: all or nothing.
fn take_all_items(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    // First check if the player has all items to avoid taking half the items from the list.
    if !REQUIRED_ITEMS
        .iter()
        .all(|(id, count)| ctx.get_quest_items_count(player, *id) >= *count)
    {
        return false;
    }
    for (id, count) in REQUIRED_ITEMS {
        ctx.take_items(player, id, count);
    }
    true
}

impl Script for Alexandria {
    fn name(&self) -> &'static str {
        "Alexandria"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Giran/Alexandria"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALEXANDRIA]);
        s.add_talk_id(&[ALEXANDRIA]);
        s.add_first_talk_id(&[ALEXANDRIA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        let player = player?;
        let agathions: Option<&[Agathion; 6]> = match event {
            "littleAngel" => Some(&LITTLE_ANGELS),
            "littleDevil" => Some(&LITTLE_DEVILS),
            _ => None,
        };
        if event == "30098-02.html" {
            htmltext = Some(event.to_owned());
        } else if let Some(agathions) = agathions {
            let chance = ctx.get_random(1000);
            let mut chance2 = 0;
            let mut chance3 = 0;
            for agathion in agathions {
                chance3 += agathion.chance;
                if chance2 <= chance && chance < chance3 {
                    if take_all_items(ctx, player) {
                        ctx.give_items(player, agathion.id, 1);
                        htmltext = Some("30098-03.html".to_owned());
                        if agathion.additional_id != 0 {
                            ctx.give_items(player, agathion.additional_id, 1);
                            htmltext = Some("30098-03a.html".to_owned());
                        }
                    } else {
                        htmltext = Some("30098-04.html".to_owned());
                    }
                    break;
                }
                chance2 += agathion.chance;
            }
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
    use crate::testing::{Pages, ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="30098" type="Folk" level="70" name="Alexandria"/>"#;

    fn items() -> String {
        let mut xml = String::new();
        for (id, _) in REQUIRED_ITEMS {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="i{id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        for id in (10315..=10326).chain([10408]) {
            xml.push_str(&format!(r#"<item id="{id}" type="EtcItem" name="a{id}"/>"#));
        }
        xml
    }

    #[test]
    fn the_pile_buys_an_agathion_and_a_missing_item_refuses() {
        let mut registry = ScriptRegistry::new();
        registry.add(Alexandria);
        let pages = Pages::of(&Alexandria)
            .page("30098.html", &["30098-02.html", "littleAngel", "littleDevil"])
            .plain(&[
                "30098-02.html",
                "30098-03.html",
                "30098-03a.html",
                "30098-04.html",
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let alexandria = t.spawn(ALEXANDRIA, 20);
        let pile = |base: i32| -> Vec<_> {
            REQUIRED_ITEMS
                .iter()
                .enumerate()
                .map(|(i, (id, count))| item(base + i as i32, *id, *count))
                .collect()
        };
        // One item short.
        let mut short = pile(500);
        short[3].count = 3;
        t.enter(1, 70, short);
        t.talk(1, alexandria);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("30098.html"));
        assert!(
            t.event(1, "Alexandria", "30098-02.html")
                .contains("30098-02.html")
        );
        // The page offers the choices only on the first page; talk again.
        t.talk(1, alexandria);
        t.take();
        assert!(t.event(1, "Alexandria", "littleAngel").contains("30098-04.html"));
        assert_eq!(t.count(1, 57), 7_500_000);

        // The whole pile: an agathion comes, the pile goes.
        t.enter(2, 70, pile(600));
        t.talk(2, alexandria);
        t.take();
        let page = t.event(2, "Alexandria", "littleDevil");
        assert!(page.contains("30098-03"), "{page}");
        assert_eq!(t.count(2, 57), 0);
        let agathions: i64 = (10321..=10326).map(|id| t.count(2, id)).sum();
        assert_eq!(agathions, 1);
    }
}
