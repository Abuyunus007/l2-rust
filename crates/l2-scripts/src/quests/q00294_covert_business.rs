//! Quest 294: Covert Business (Keef in the Dwarven Village sends the dwarves after the bat fangs).
// Java: dist/game/data/scripts/quests/Q00294_CovertBusiness/Q00294_CovertBusiness.java
//
// Pattern: a kill quest with a graded drop (`docs/design/scripts.md`, "Quest patterns"): a kill of
// a bat or a vampire bat gives 1 to 4 fangs by one roll of ten, up to 100 fangs; the 100th sets
// cond 2 and Keef pays a ring of raccoon (or adena, once the player has one) and ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KEEF: i32 = 30534;

// Item
const BAT_FANG: i32 = 1491;

// Reward
const RING_OF_RACCOON: i32 = 1508;

// Misc
const MIN_LEVEL: i32 = 10;

/// Java `MONSTER_DROP_CHANCE`: the thresholds of the roll of ten per monster; one more fang per
/// threshold the roll passes.
fn drop_thresholds(npc_id: i32) -> &'static [i32] {
    match npc_id {
        20370 => &[6, 3, 1, -1],
        20480 => &[5, 2, -1],
        _ => &[],
    }
}

/// Java `Q00294_CovertBusiness`.
pub struct Q00294CovertBusiness;

impl Script for Q00294CovertBusiness {
    fn name(&self) -> &'static str {
        "Q00294_CovertBusiness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00294_CovertBusiness"
    }

    fn id(&self) -> i32 {
        294
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEEF]);
        s.add_talk_id(&[KEEF]);
        s.add_kill_id(&[20370, 20480]);
        s.register_quest_items(&[BAT_FANG]);
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
        if qs.is_created(ctx) && event == "30534-03.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
        {
            let chance = ctx.get_random(10);
            let mut count = 0;
            for &i in drop_thresholds(npc.id) {
                count += 1;
                if chance > i {
                    if ctx.give_item_randomly(killer, Some(npc), BAT_FANG, count, count, 100, 1.0, true) {
                        // `setCond(2)`: no sound of its own.
                        qs.set_cond(ctx, 2, false);
                    }
                    break;
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);
        if qs.state(ctx) == CREATED {
            html = if ctx.player_race(talker) == race::DWARF {
                if ctx.player_level(talker) >= MIN_LEVEL {
                    "30534-02.htm"
                } else {
                    "30534-01.htm"
                }
            } else {
                "30534-00.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if qs.is_cond(ctx, 2) {
                if ctx.has_quest_items(talker, RING_OF_RACCOON) {
                    ctx.give_adena(talker, 2400, true);
                    html = "30534-06.html".to_owned();
                } else {
                    ctx.give_items(talker, RING_OF_RACCOON, 1);
                    html = "30534-05.html".to_owned();
                }
                ctx.add_exp_and_sp(talker, 0, 600);
                qs.exit_quest(ctx, true, true);
            } else {
                html = "30534-04.html".to_owned();
            }
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00294_CovertBusiness";

    const NPCS: &str = r#"
        <npc id="30534" type="Folk" level="70" name="Keef"/>
        <npc id="20370" type="Monster" level="12" name="Bat"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1491" type="EtcItem" name="Bat Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1508" type="Armor" name="Ring of Raccoon"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00294CovertBusiness);
        let pages = Pages::of(&Q00294CovertBusiness)
            .page("30534-02.htm", &["30534-03.htm"])
            .plain(&[
                "30534-00.htm",
                "30534-01.htm",
                "30534-03.htm",
                "30534-04.html",
                "30534-05.html",
                "30534-06.html",
            ])
            .npcs(&[KEEF]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let keef = t.spawn(KEEF, 20);
        (t, keef)
    }

    /// Kills bats with `player` until it has `fangs` fangs; returns the kills.
    fn hunt(t: &mut ScriptTest, player: i32, fangs: i64) -> usize {
        for kills in 1..1000 {
            let bat = t.spawn(20370, 60);
            t.kill(player, bat);
            if t.count(player, BAT_FANG) >= fangs {
                return kills;
            }
        }
        panic!("no {fangs} fangs after 1000 kills");
    }

    #[test]
    fn only_dwarves_of_level_ten_can_take_it() {
        let (mut t, keef) = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, keef).contains("30534-00.htm"));
        t.enter_as(2, 9, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, keef).contains("30534-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_fangs_then_the_ring_and_exp() {
        let (mut t, keef) = setup();
        t.enter_as(1, 10, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, keef).contains("30534-02.htm"));
        assert!(t.event(1, NAME, "30534-03.htm").contains("30534-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, keef).contains("30534-04.html"));

        // The 100th fang sets cond 2 (`giveItemRandomly` reports the limit).
        hunt(&mut t, 1, 100);
        assert_eq!(t.count(1, BAT_FANG), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Keef gives the ring (no ring yet) and ends the quest, repeatable.
        assert!(t.quest_page(1, keef).contains("30534-05.html"));
        assert_eq!(t.count(1, RING_OF_RACCOON), 1);
        assert_eq!(t.count(1, BAT_FANG), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn with_a_ring_already_keef_pays_adena() {
        let (mut t, keef) = setup();
        t.enter_as(
            1,
            10,
            race::DWARF,
            player_class::DWARVEN_FIGHTER,
            vec![item(900, RING_OF_RACCOON, 1)],
        );
        assert!(t.quest_page(1, keef).contains("30534-02.htm"));
        assert!(t.event(1, NAME, "30534-03.htm").contains("30534-03.htm"));
        t.take_writes();
        hunt(&mut t, 1, 100);
        t.take_writes();

        // The ring is there: 2400 adena instead of a second ring.
        assert!(t.quest_page(1, keef).contains("30534-06.html"));
        assert_eq!(t.count(1, RING_OF_RACCOON), 1);
        assert_eq!(t.count(1, 57), 2400);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
