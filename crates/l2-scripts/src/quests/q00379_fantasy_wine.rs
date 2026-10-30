//! Quest 379: Fantasy Wine (Guard Harlan in the Town of Oren: eucalyptus leaves and stones of chill).
// Java: dist/game/data/scripts/quests/Q00379_FantasyWine/Q00379_FantasyWine.java
//
// Pattern: two drops with a limit (`docs/design/scripts.md`, "Quest patterns"): the leaves come
// from the Enku Orc Champion, the stones from the Enku Orc Shaman, each up to its count; the kill
// that completes both sets cond 2. The reward is one of three wines, picked at random.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HARLAN: i32 = 30074;

// Items (Java `ItemHolder`: id and count)
const LEAF_OF_EUCALYPTUS: (i32, i64) = (5893, 80);
const STONE_OF_CHILL: (i32, i64) = (5894, 100);
const OLD_WINE_15_YEAR: i32 = 5956;
const OLD_WINE_30_YEAR: i32 = 5957;
const OLD_WINE_60_YEAR: i32 = 5958;

// Monsters
const ENKU_ORC_CHAMPION: i32 = 20291;
const ENKU_ORC_SHAMAN: i32 = 20292;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `hasItem(player, ItemHolder)` (checkCount): the player has at least the holder's count.
fn has_item(ctx: &ScriptCtx<'_>, player: ObjectId, holder: (i32, i64)) -> bool {
    ctx.get_quest_items_count(player, holder.0) >= holder.1
}

/// Java `hasAllItems(player, true, holders...)`.
fn has_all_items(ctx: &ScriptCtx<'_>, player: ObjectId, holders: &[(i32, i64)]) -> bool {
    holders.iter().all(|holder| has_item(ctx, player, *holder))
}

/// Java `takeAllItems(player, holders...)`: each holder's count goes.
fn take_all_items(ctx: &mut ScriptCtx<'_>, player: ObjectId, holders: &[(i32, i64)]) {
    for (id, count) in holders {
        ctx.take_items(player, *id, *count);
    }
}

/// Java `Q00379_FantasyWine`.
pub struct Q00379FantasyWine;

impl Script for Q00379FantasyWine {
    fn name(&self) -> &'static str {
        "Q00379_FantasyWine"
    }

    fn path(&self) -> &'static str {
        "quests/Q00379_FantasyWine"
    }

    fn id(&self) -> i32 {
        379
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HARLAN]);
        s.add_talk_id(&[HARLAN]);
        s.add_kill_id(&[ENKU_ORC_CHAMPION, ENKU_ORC_SHAMAN]);
        s.register_quest_items(&[LEAF_OF_EUCALYPTUS.0, STONE_OF_CHILL.0]);
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
            "30074-02.htm" | "30074-03.htm" | "30074-05.html" => {
                htmltext = Some(event.to_owned());
            }
            "30074-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30074-11.html" => {
                if has_all_items(ctx, player, &[LEAF_OF_EUCALYPTUS, STONE_OF_CHILL]) {
                    let random = ctx.get_random(10);
                    let (item, page) = if random < 3 {
                        (OLD_WINE_15_YEAR, event)
                    } else if random < 9 {
                        (OLD_WINE_30_YEAR, "30074-12.html")
                    } else {
                        (OLD_WINE_60_YEAR, "30074-13.html")
                    };
                    htmltext = Some(page.to_owned());
                    ctx.give_items(player, item, 1);
                    take_all_items(ctx, player, &[LEAF_OF_EUCALYPTUS, STONE_OF_CHILL]);
                    qs.exit_quest(ctx, true, true);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        let drop = if npc.id == ENKU_ORC_CHAMPION {
            LEAF_OF_EUCALYPTUS
        } else {
            STONE_OF_CHILL
        };
        if ctx.give_item_randomly(killer, Some(npc), drop.0, 1, 1, drop.1, 1.0, true)
            && has_all_items(ctx, killer, &[LEAF_OF_EUCALYPTUS, STONE_OF_CHILL])
        {
            qs.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30074-01.htm"
            } else {
                "30074-06.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let has_leaf_of_eucalyptus = has_item(ctx, player, LEAF_OF_EUCALYPTUS);
            let has_stone_of_chill = has_item(ctx, player, STONE_OF_CHILL);
            htmltext = if !has_leaf_of_eucalyptus && !has_stone_of_chill {
                "30074-07.html"
            } else if has_leaf_of_eucalyptus && !has_stone_of_chill {
                "30074-08.html"
            } else if !has_leaf_of_eucalyptus {
                "30074-09.html"
            } else {
                "30074-10.html"
            }
            .to_owned();
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

    const NAME: &str = "Q00379_FantasyWine";

    const NPCS: &str = r#"
        <npc id="30074" type="Folk" level="70" name="Guard Harlan"/>
        <npc id="20291" type="Monster" level="28" name="Enku Orc Champion"/>
        <npc id="20292" type="Monster" level="28" name="Enku Orc Shaman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5893" type="EtcItem" name="Leaf of Eucalyptus"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5894" type="EtcItem" name="Stone of Chill"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5956" type="EtcItem" name="Dion Wine (15 years)"/>
        <item id="5957" type="EtcItem" name="Dion Wine (30 years)"/>
        <item id="5958" type="EtcItem" name="Dion Wine (60 years)"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00379FantasyWine);
        let pages = Pages::of(&Q00379FantasyWine)
            .page("30074-01.htm", &["30074-02.htm"])
            .page("30074-02.htm", &["30074-03.htm", "30074-05.html"])
            .page("30074-03.htm", &["30074-04.htm", "30074-05.html"])
            .plain(&[
                "30074-04.htm",
                "30074-05.html",
                "30074-06.html",
                "30074-07.html",
                "30074-08.html",
                "30074-09.html",
            ])
            .page("30074-10.html", &["30074-11.html"])
            .plain(&["30074-11.html", "30074-12.html", "30074-13.html"])
            .npcs(&[HARLAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let harlan = t.spawn(HARLAN, 20);
        (t, harlan)
    }

    /// Kills monsters of `npc_id` until the player has `count` of `item_id`.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) {
        for _ in 0..500 {
            if t.count(1, item_id) >= count {
                return;
            }
            let mob = t.spawn(npc_id, 60);
            t.kill(1, mob);
        }
        panic!("no {count} of {item_id} after 500 kills");
    }

    #[test]
    fn a_level_19_character_is_sent_away() {
        let (mut t, harlan) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, harlan).contains("30074-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_ingredients_then_one_wine() {
        let (mut t, harlan) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, harlan).contains("30074-01.htm"));
        assert!(t.event(1, NAME, "30074-02.htm").contains("30074-02.htm"));
        assert!(t.event(1, NAME, "30074-03.htm").contains("30074-03.htm"));
        assert!(t.event(1, NAME, "30074-04.htm").contains("30074-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, harlan).contains("30074-07.html"));

        // The leaves first: no cond until the stones are there too.
        hunt(
            &mut t,
            ENKU_ORC_CHAMPION,
            LEAF_OF_EUCALYPTUS.0,
            LEAF_OF_EUCALYPTUS.1,
        );
        assert_eq!(t.count(1, LEAF_OF_EUCALYPTUS.0), 80);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, harlan).contains("30074-08.html"));

        // The stones: the one that completes both sets the cond.
        hunt(&mut t, ENKU_ORC_SHAMAN, STONE_OF_CHILL.0, STONE_OF_CHILL.1);
        assert_eq!(t.count(1, STONE_OF_CHILL.0), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, harlan).contains("30074-10.html"));

        // One wine, picked at random; the ingredients go and the quest ends repeatable.
        let page = t.event(1, NAME, "30074-11.html");
        assert!(
            ["30074-11.html", "30074-12.html", "30074-13.html"]
                .iter()
                .any(|p| page.contains(p)),
            "{page}"
        );
        let wines: i64 = [OLD_WINE_15_YEAR, OLD_WINE_30_YEAR, OLD_WINE_60_YEAR]
            .iter()
            .map(|id| t.count(1, *id))
            .sum();
        assert_eq!(wines, 1);
        assert_eq!(t.count(1, LEAF_OF_EUCALYPTUS.0), 0);
        assert_eq!(t.count(1, STONE_OF_CHILL.0), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
