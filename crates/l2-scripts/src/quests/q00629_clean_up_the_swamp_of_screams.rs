//! Quest 629: Clean Up the Swamp of Screams (Pierce in the Gludio/Dion area; the Stakato monsters
//! of the Swamp of Screams drop talons, 100 of them buy 20 Golden Ram Coins).
// Java: dist/game/data/scripts/quests/Q00629_CleanUpTheSwampOfScreams/Q00629_CleanUpTheSwampOfScreams.java
//
// Pattern: a repeatable kill count with a drop chance per monster (a fraction, `giveItemRandomly`
// with no limit), the kill goes to a random started party member (`getRandomPartyMemberState`),
// the exchange of 100 items, and the quit of the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PIERCE: i32 = 31553;

// Items
const TALON_OF_STAKATO: i32 = 7250;
const GOLDEN_RAM_COIN: i32 = 7251;

// Misc
const REQUIRED_TALON_COUNT: i64 = 100;
const MIN_LEVEL: i32 = 66;

/// Java `MOBS_DROP_CHANCES`: the monster and its talon drop chance.
const MOBS_DROP_CHANCES: [(i32, f64); 10] = [
    (21508, 0.599), // splinter_stakato
    (21509, 0.524), // splinter_stakato_worker
    (21510, 0.640), // splinter_stakato_soldier
    (21511, 0.830), // splinter_stakato_drone
    (21512, 0.970), // splinter_stakato_drone_a
    (21513, 0.682), // needle_stakato
    (21514, 0.595), // needle_stakato_worker
    (21515, 0.727), // needle_stakato_soldier
    (21516, 0.879), // needle_stakato_drone
    (21517, 0.999), // needle_stakato_drone_a
];

/// Java `Q00629_CleanUpTheSwampOfScreams`.
pub struct Q00629CleanUpTheSwampOfScreams;

impl Script for Q00629CleanUpTheSwampOfScreams {
    fn name(&self) -> &'static str {
        "Q00629_CleanUpTheSwampOfScreams"
    }

    fn path(&self) -> &'static str {
        "quests/Q00629_CleanUpTheSwampOfScreams"
    }

    fn id(&self) -> i32 {
        629
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PIERCE]);
        s.add_talk_id(&[PIERCE]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[TALON_OF_STAKATO, GOLDEN_RAM_COIN]);
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
            "31553-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31553-04.html" | "31553-06.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31553-07.html" => {
                if qs.is_started(ctx)
                    && ctx.get_quest_items_count(player, TALON_OF_STAKATO) >= REQUIRED_TALON_COUNT
                {
                    ctx.reward_items(player, GOLDEN_RAM_COIN, 20);
                    ctx.take_items(player, TALON_OF_STAKATO, 100);
                    htmltext = Some(event.to_owned());
                } else {
                    // NOTE(java-quirk): the "not enough" page is also shown to a character that
                    // is not started (Java sets it outside the `isStarted` check).
                    htmltext = Some("31553-08.html".to_owned());
                }
            }
            "31553-09.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 2, npc)`: the killer counts twice.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 2, Some(npc)) else {
            return;
        };
        let Some(&(_, chance)) = MOBS_DROP_CHANCES.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        ctx.give_item_randomly(qs.player(), Some(npc), TALON_OF_STAKATO, 1, 1, 0, chance, true);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31553-01.htm".to_owned()
            } else {
                "31553-02.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext = if ctx.get_quest_items_count(player, TALON_OF_STAKATO) >= REQUIRED_TALON_COUNT {
                "31553-04.html".to_owned()
            } else {
                "31553-05.html".to_owned()
            };
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

    const NAME: &str = "Q00629_CleanUpTheSwampOfScreams";

    const NPCS: &str = r#"
        <npc id="31553" type="Folk" level="70" name="Pierce"/>
        <npc id="21517" type="Monster" level="70" name="Needle Stakato Drone"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7250" type="EtcItem" name="Talon of Stakato"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7251" type="EtcItem" name="Golden Ram Coin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00629CleanUpTheSwampOfScreams);
        let pages = Pages::of(&Q00629CleanUpTheSwampOfScreams)
            .page("31553-01.htm", &["31553-03.htm"])
            .plain(&["31553-02.htm", "31553-03.htm", "31553-05.html", "31553-08.html"])
            .page("31553-04.html", &["31553-07.html"])
            .page(
                "31553-07.html",
                &["31553-06.html", "31553-07.html", "31553-09.html"],
            )
            .plain(&["31553-06.html", "31553-09.html"])
            .npcs(&[PIERCE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let pierce = t.spawn(PIERCE, 20);
        (t, pierce)
    }

    /// A needle stakato drone (drop chance 0.999) dies next to the player.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(21517, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_66_is_sent_away() {
        let (mut t, pierce) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, pierce).contains("31553-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_talons_buy_the_coins_and_the_quest_ends() {
        let (mut t, pierce) = setup();
        t.enter(1, 66, vec![]);
        assert!(t.quest_page(1, pierce).contains("31553-01.htm"));
        assert!(t.event(1, NAME, "31553-03.htm").contains("31553-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Talons come from the kills (the killer counts as a started party member).
        let mut kills = 0;
        while t.count(1, TALON_OF_STAKATO) < REQUIRED_TALON_COUNT {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 150, "no talons dropped");
        }
        assert!(t.take_writes().is_empty());

        // 100 talons: the coins, the talons taken, the quest left.
        assert!(t.quest_page(1, pierce).contains("31553-04.html"));
        assert!(t.event(1, NAME, "31553-07.html").contains("31553-07.html"));
        assert_eq!(t.count(1, GOLDEN_RAM_COIN), 20);
        assert_eq!(t.count(1, TALON_OF_STAKATO), 0);
        assert!(t.event(1, NAME, "31553-09.html").contains("31553-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_exchange_refuses_fewer_than_100_talons() {
        let (mut t, pierce) = setup();
        t.enter(1, 66, vec![]);
        t.quest_page(1, pierce);
        t.event(1, NAME, "31553-03.htm");
        t.take_writes();
        while t.count(1, TALON_OF_STAKATO) < REQUIRED_TALON_COUNT {
            kill_one(&mut t);
        }
        assert!(t.quest_page(1, pierce).contains("31553-04.html"));
        // Two talons leave the inventory after the page was shown: the exchange says no.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), TALON_OF_STAKATO, 1);
        });
        assert!(t.event(1, NAME, "31553-07.html").contains("31553-08.html"));
        assert_eq!(t.count(1, GOLDEN_RAM_COIN), 0);
        assert!(t.take_writes().is_empty());
    }
}
