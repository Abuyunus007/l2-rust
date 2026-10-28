//! Quest 113: Status of the Beacon Tower (Seer Moira sends the box, Torrant at the Den of Evil).
// Java: dist/game/data/scripts/quests/Q00113_StatusOfTheBeaconTower/Q00113_StatusOfTheBeaconTower.java
//
// Pattern: a delivery with a reward that depends on an item the player holds (the Fire box of a
// previous quest gives the smaller reward). `on_event` answers only the events it knows: Java's
// `default: htmltext = null`, and a player without the quest gets null, not the no-quest page.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MOIRA: i32 = 31979;
const TORRANT: i32 = 32016;

// Items
const FLAME_BOX: i32 = 14860;
const FIRE_BOX: i32 = 8086;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00113_StatusOfTheBeaconTower`.
pub struct Q00113StatusOfTheBeaconTower;

impl Script for Q00113StatusOfTheBeaconTower {
    fn name(&self) -> &'static str {
        "Q00113_StatusOfTheBeaconTower"
    }

    fn path(&self) -> &'static str {
        "quests/Q00113_StatusOfTheBeaconTower"
    }

    fn id(&self) -> i32 {
        113
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOIRA]);
        s.add_talk_id(&[MOIRA, TORRANT]);
        s.register_quest_items(&[FIRE_BOX, FLAME_BOX]);
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

        match event {
            "31979-02.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, FLAME_BOX, 1);
            }
            "32016-02.html" => {
                if ctx.has_quest_items(player, FIRE_BOX) {
                    ctx.give_adena(player, 21_578, true);
                    ctx.add_exp_and_sp(player, 76_665, 5333);
                } else {
                    ctx.give_adena(player, 154_800, true);
                    ctx.add_exp_and_sp(player, 619_300, 44_200);
                }
                qs.exit_quest(ctx, false, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MOIRA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31979-01.htm"
                    } else {
                        "31979-00.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31979-03.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            TORRANT => {
                if qs.is_started(ctx) {
                    htmltext = "32016-01.html".to_owned();
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"
        <npc id="31979" type="Folk" level="80" name="Seer Moira"/>
        <npc id="32016" type="Folk" level="80" name="Beacon Tower Manager Torrant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14860" type="EtcItem" name="Flame Box"><set name="is_questitem" val="true"/></item>
        <item id="8086" type="EtcItem" name="Fire Box"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00113_StatusOfTheBeaconTower";

    struct Npcs {
        moira: Oid,
        torrant: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00113StatusOfTheBeaconTower);
        let pages = Pages::of(&Q00113StatusOfTheBeaconTower)
            .page("31979-00.htm", &[])
            .page("31979-01.htm", &["31979-02.htm"])
            .plain(&["31979-02.htm", "31979-03.html"])
            .page("32016-01.html", &["32016-02.html"])
            .plain(&["32016-02.html"])
            .npcs(&[MOIRA, TORRANT])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            moira: t.spawn(MOIRA, 20),
            torrant: t.spawn(TORRANT, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, npcs.moira).contains("31979-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn torrant_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, npcs.torrant).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_box_to_torrant_and_the_big_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, npcs.moira).contains("31979-01.htm"));

        // Accept: `startQuest()`, then the flame box.
        assert!(t.event(1, NAME, "31979-02.htm").contains("31979-02.htm"));
        assert_eq!(t.count(1, FLAME_BOX), 1);
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        assert!(t.quest_page(1, npcs.moira).contains("31979-03.html"));
        assert!(t.quest_page(1, npcs.torrant).contains("32016-01.html"));

        // No fire box: the bigger reward, then `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32016-02.html").contains("32016-02.html"));
        assert_eq!(t.count(1, 57), 154_800);
        assert_eq!(t.count(1, FLAME_BOX), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_fire_box_in_the_bag_gives_the_smaller_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![item(100, FIRE_BOX, 1)]);
        t.quest_page(1, npcs.moira);
        t.event(1, NAME, "31979-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.torrant).contains("32016-01.html"));

        assert!(t.event(1, NAME, "32016-02.html").contains("32016-02.html"));
        assert_eq!(t.count(1, 57), 21_578);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }

    #[test]
    fn a_finished_character_gets_the_completed_page() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, npcs.moira);
        t.event(1, NAME, "31979-02.htm");
        t.quest_page(1, npcs.torrant);
        t.event(1, NAME, "32016-02.html");
        t.take_writes();
        assert!(t.quest_page(1, npcs.moira).contains("This quest has already been completed."));
    }
}
