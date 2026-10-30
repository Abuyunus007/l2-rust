//! Quest 356: Dig Up the Sea of Spores (Gauen in the Rune Township, Rotting Trees and Spore Zombies).
// Java: dist/game/data/scripts/quests/Q00356_DigUpTheSeaOfSpores/Q00356_DigUpTheSeaOfSpores.java
//
// Pattern: two drops with a limit of 50 each (`docs/design/scripts.md`, "Quest patterns"): the
// herbivorous spores from the rotting trees, the carnivorous from the spore zombies, the cond moves
// to 2 at the first 50 and to 3 when both are complete. The reward is picked on the page: exp
// (and sp) for the spores, or a random adena amount at the end.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GAUEN: i32 = 30717;

// Items
const CARNIVORE_SPORE: i32 = 5865;
const HERBIVOROUS_SPORE: i32 = 5866;

// Misc
const MIN_LEVEL: i32 = 43;

// Monsters (drop chance)
const ROTTING_TREE: i32 = 20558;
const SPORE_ZOMBIE: i32 = 20562;
const MONSTER_DROP_CHANCES: [(i32, f64); 2] = [(ROTTING_TREE, 0.73), (SPORE_ZOMBIE, 0.94)];

/// Java `Q00356_DigUpTheSeaOfSpores`.
pub struct Q00356DigUpTheSeaOfSpores;

impl Script for Q00356DigUpTheSeaOfSpores {
    fn name(&self) -> &'static str {
        "Q00356_DigUpTheSeaOfSpores"
    }

    fn path(&self) -> &'static str {
        "quests/Q00356_DigUpTheSeaOfSpores"
    }

    fn id(&self) -> i32 {
        356
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GAUEN]);
        s.add_talk_id(&[GAUEN]);
        s.add_kill_id(&[ROTTING_TREE, SPORE_ZOMBIE]);
        s.register_quest_items(&[HERBIVOROUS_SPORE, CARNIVORE_SPORE]);
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
            "30717-02.htm" | "30717-03.htm" | "30717-04.htm" | "30717-10.html" | "30717-18.html" => {
                htmltext = Some(event.to_owned());
            }
            "30717-05.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30717-09.html" => {
                ctx.add_exp_and_sp(player, 31850, 0);
                ctx.take_items(player, CARNIVORE_SPORE, -1);
                ctx.take_items(player, HERBIVOROUS_SPORE, -1);
                htmltext = Some(event.to_owned());
            }
            "30717-11.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30717-14.html" => {
                ctx.add_exp_and_sp(player, 45500, 2600);
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "FINISH" => {
                let value = ctx.get_random(100);
                let (adena, page) = if value < 20 {
                    (44000, "30717-15.html")
                } else if value < 70 {
                    (20950, "30717-16.html")
                } else {
                    (10400, "30717-17.html")
                };
                ctx.give_adena(player, adena, true);
                qs.exit_quest(ctx, true, true);
                htmltext = Some(page.to_owned());
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
        let drop_item = if npc.id == ROTTING_TREE {
            HERBIVOROUS_SPORE
        } else {
            CARNIVORE_SPORE
        };
        let other_item = if drop_item == HERBIVOROUS_SPORE {
            CARNIVORE_SPORE
        } else {
            HERBIVOROUS_SPORE
        };
        let chance = MONSTER_DROP_CHANCES
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0.0, |(_, c)| *c);
        if ctx.give_item_randomly(qs.player(), Some(npc), drop_item, 1, 1, 50, chance, true) {
            if ctx.get_quest_items_count(killer, other_item) >= 50 {
                qs.set_cond(ctx, 3, false);
            } else {
                qs.set_cond(ctx, 2, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30717-01.htm"
            } else {
                "30717-06.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let has_all_herb_spores = ctx.get_quest_items_count(player, HERBIVOROUS_SPORE) >= 50;
            let has_all_carn_spores = ctx.get_quest_items_count(player, CARNIVORE_SPORE) >= 50;
            htmltext = if has_all_herb_spores && has_all_carn_spores {
                "30717-13.html"
            } else if has_all_carn_spores {
                "30717-12.html"
            } else if has_all_herb_spores {
                "30717-08.html"
            } else {
                "30717-07.html"
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00356_DigUpTheSeaOfSpores";

    const NPCS: &str = r#"
        <npc id="30717" type="Folk" level="70" name="Gauen"/>
        <npc id="20558" type="Monster" level="40" name="Rotting Tree"/>
        <npc id="20562" type="Monster" level="40" name="Spore Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5865" type="EtcItem" name="Carnivore Spore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5866" type="EtcItem" name="Herbivorous Spore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00356DigUpTheSeaOfSpores);
        let pages = Pages::of(&Q00356DigUpTheSeaOfSpores)
            .page("30717-01.htm", &["30717-02.htm"])
            .page("30717-02.htm", &["30717-03.htm"])
            .page("30717-03.htm", &["30717-04.htm"])
            .page("30717-04.htm", &["30717-05.htm"])
            .plain(&["30717-05.htm", "30717-06.htm", "30717-07.html"])
            .page("30717-08.html", &["30717-09.html", "30717-10.html"])
            .page("30717-09.html", &["30717-10.html", "30717-11.html"])
            .plain(&[
                "30717-10.html",
                "30717-11.html",
                "30717-14.html",
                "30717-15.html",
                "30717-16.html",
                "30717-17.html",
            ])
            .page("30717-12.html", &["30717-09.html", "30717-10.html"])
            .page("30717-13.html", &["30717-14.html", "30717-18.html"])
            .page("30717-18.html", &["FINISH"])
            .npcs(&[GAUEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gauen = t.spawn(GAUEN, 20);
        (t, gauen)
    }

    /// Accepts the quest: 01 -> 02 -> 03 -> 04 -> 05.
    fn accept(t: &mut ScriptTest, gauen: Oid) {
        assert!(t.quest_page(1, gauen).contains("30717-01.htm"));
        for page in ["30717-02.htm", "30717-03.htm", "30717-04.htm", "30717-05.htm"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        t.take_writes();
    }

    #[test]
    fn a_level_42_character_is_sent_away() {
        let (mut t, gauen) = setup();
        t.enter(1, 42, vec![]);
        assert!(t.quest_page(1, gauen).contains("30717-06.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_spores_then_the_finish_pays_adena() {
        let (mut t, gauen) = setup();
        t.enter(1, 43, vec![]);
        assert!(t.quest_page(1, gauen).contains("30717-01.htm"));
        for page in ["30717-02.htm", "30717-03.htm", "30717-04.htm"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert!(t.event(1, NAME, "30717-05.htm").contains("30717-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // 50 herbivorous spores (rotting trees): cond 2 at the 50th.
        for _ in 0..400 {
            if t.count(1, HERBIVOROUS_SPORE) >= 50 {
                break;
            }
            let tree = t.spawn(ROTTING_TREE, 60);
            t.kill(1, tree);
        }
        assert_eq!(t.count(1, HERBIVOROUS_SPORE), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, gauen).contains("30717-08.html"));

        // 50 carnivorous spores (spore zombies): cond 3 at the 50th.
        for _ in 0..400 {
            if t.count(1, CARNIVORE_SPORE) >= 50 {
                break;
            }
            let zombie = t.spawn(SPORE_ZOMBIE, 60);
            t.kill(1, zombie);
        }
        assert_eq!(t.count(1, CARNIVORE_SPORE), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Both: the NPC's page 13 offers the exp reward and "FINISH" (18).
        assert!(t.quest_page(1, gauen).contains("30717-13.html"));
        assert!(t.event(1, NAME, "30717-18.html").contains("30717-18.html"));
        let adena = t.count(1, 57);
        let page = t.event(1, NAME, "FINISH");
        assert!(
            ["30717-15.html", "30717-16.html", "30717-17.html"]
                .iter()
                .any(|p| page.contains(p)),
            "{page}"
        );
        let paid = t.count(1, 57) - adena;
        assert!([44000, 20950, 10400].contains(&paid), "{paid}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_spores_given_up_for_exp() {
        let (mut t, gauen) = setup();
        t.enter(1, 43, vec![item(100, HERBIVOROUS_SPORE, 50)]);
        accept(&mut t, gauen);
        assert!(t.quest_page(1, gauen).contains("30717-08.html"));
        assert!(t.event(1, NAME, "30717-09.html").contains("30717-09.html"));
        assert_eq!(t.count(1, HERBIVOROUS_SPORE), 0);
        assert!(t.take_writes().is_empty());
        assert!(t.event(1, NAME, "30717-11.html").contains("30717-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
