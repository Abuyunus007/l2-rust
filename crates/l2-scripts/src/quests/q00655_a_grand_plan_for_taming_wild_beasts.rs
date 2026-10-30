//! Quest 655: A Grand Plan for Taming Wild Beasts (the Beast Farm messenger, the clan hall siege qualification).
// Java: dist/game/data/scripts/quests/Q00655_AGrandPlanForTamingWildBeasts/Q00655_AGrandPlanForTamingWildBeasts.java
//
// Pattern: a one-time quest that gathers items from another script's kills
// (`check_crystal_of_purity`, called by the beast farm AI), then pays a license for the clan hall
// siege. The messenger refuses while five clans are already in the siege of the Beast Farm.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{ClanHallId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MESSENGER: i32 = 35627;

// Items
const STONE: i32 = 8084;
const TRAINER_LICENSE: i32 = 8293;

// Misc
/// Java `CHSiegeManager.getSiegableHall(63)`: the Beast Farm clan hall.
const BEAST_STRONGHOLD: ClanHallId = ClanHallId(63);
/// Java `BEAST_STRONGHOLD.getSiege().getAttackers().size() >= 5`.
const MAX_ATTACKERS: usize = 5;

/// Java `Q00655_AGrandPlanForTamingWildBeasts`.
pub struct Q00655AGrandPlanForTamingWildBeasts;

/// Java `Q00655_AGrandPlanForTamingWildBeasts.checkCrystalofPurity(player)`: a player at cond 1
/// who has fewer than 10 Crystals of Purity gets one more (called by the beast farm kills).
///
/// Called from the last growth step of the beast farm (`BeastFarm`, `FeedableBeasts`).
pub fn check_crystal_of_purity(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let at_cond_one = ctx
        .player_quest_state(player, "Q00655_AGrandPlanForTamingWildBeasts")
        .is_some_and(|qs| qs.is_cond(1));
    if at_cond_one && ctx.get_quest_items_count(player, STONE) < 10 {
        ctx.give_items(player, STONE, 1);
    }
}

impl Script for Q00655AGrandPlanForTamingWildBeasts {
    fn name(&self) -> &'static str {
        "Q00655_AGrandPlanForTamingWildBeasts"
    }

    fn path(&self) -> &'static str {
        "quests/Q00655_AGrandPlanForTamingWildBeasts"
    }

    fn id(&self) -> i32 {
        655
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MESSENGER]);
        s.add_talk_id(&[MESSENGER]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let Some(qs) = ctx.get_quest_state(player, true) else {
            return Some(htmltext);
        };

        match qs.state(ctx) {
            CREATED => {
                if ctx.world().hall_siege_attacker_clans(BEAST_STRONGHOLD).len() >= MAX_ATTACKERS {
                    htmltext = "35627-00.htm".to_owned();
                } else {
                    htmltext = "35627-01.htm".to_owned();
                    qs.start_quest(ctx);
                }
            }
            STARTED => {
                if ctx.get_quest_items_count(player, STONE) < 10 {
                    htmltext = "35627-02.htm".to_owned();
                } else {
                    ctx.take_items(player, STONE, 10);
                    ctx.give_items(player, TRAINER_LICENSE, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "35627-03.htm".to_owned();
                }
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00655_AGrandPlanForTamingWildBeasts";

    const NPCS: &str = r#"
        <npc id="35627" type="Folk" level="1" name="Messenger"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8084" type="EtcItem" name="Crystal of Purity"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8293" type="EtcItem" name="Trainer's License"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00655AGrandPlanForTamingWildBeasts);
        let pages = Pages::of(&Q00655AGrandPlanForTamingWildBeasts)
            .plain(&["35627-00.htm", "35627-01.htm", "35627-02.htm", "35627-03.htm"])
            .npcs(&[MESSENGER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let messenger = t.spawn(MESSENGER, 20);
        (t, messenger)
    }

    #[test]
    fn start_then_the_stones_then_the_license() {
        let (mut t, messenger) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, messenger).contains("35627-01.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nine stones are not enough.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), STONE, 9);
        });
        assert!(t.quest_page(1, messenger).contains("35627-02.htm"));
        assert!(t.take_writes().is_empty());

        // The tenth one: the license, the stones are gone and the quest is over.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), STONE, 1);
        });
        assert!(t.quest_page(1, messenger).contains("35627-03.htm"));
        assert_eq!((t.count(1, STONE), t.count(1, TRAINER_LICENSE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_beast_farm_gives_a_stone_at_cond_one_only() {
        let (mut t, messenger) = setup();
        t.enter(1, 20, vec![]);
        // Not started: nothing.
        t.world
            .run_as_script(NAME, |ctx| check_crystal_of_purity(ctx, Oid(1)));
        assert_eq!(t.count(1, STONE), 0);

        t.quest_page(1, messenger);
        t.take_writes();
        for expected in 1..=10 {
            t.world
                .run_as_script(NAME, |ctx| check_crystal_of_purity(ctx, Oid(1)));
            assert_eq!(t.count(1, STONE), expected);
        }
        // The tenth stone is the last one: no more are given.
        t.world
            .run_as_script(NAME, |ctx| check_crystal_of_purity(ctx, Oid(1)));
        assert_eq!(t.count(1, STONE), 10);
    }
}
