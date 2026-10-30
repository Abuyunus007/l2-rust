//! The Valentine Event: Miss Queen hands out the cake recipe.
// Java: dist/game/data/scripts/events/TheValentineEvent/TheValentineEvent.java
// Java: dist/game/data/scripts/events/TheValentineEvent/config.xml
//
// The reference port of a seasonal event: a `LongTimeEvent` (`long_time_event.rs`) whose
// `config.xml` is the `CONFIG` constant and whose script is plain NPC talk.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

// NPC
const NPC: i32 = 4301;

// Item
const RECIPE: i32 = 20191;

// Misc
/// Java `TheValentineEvent.class.getSimpleName() + "_completed"`.
const COMPLETED: &str = "TheValentineEvent_completed";

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Valentine Event",
    active: "15 12 2001-16 12 2001",
    drop_period: None,
    drops: &[
        DropSpec {
            item: 20192,
            min: 1,
            max: 1,
            chance: 5.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 20193,
            min: 1,
            max: 1,
            chance: 5.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 20194,
            min: 1,
            max: 1,
            chance: 5.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
    ],
    spawns: &[
        SpawnSpec::at(4301, 87792, -142240, -1343, 44000, 0),
        SpawnSpec::at(4301, 87616, -140688, -1542, 16500, 0),
        SpawnSpec::at(4301, 114733, -178691, -821, 0, 0),
        SpawnSpec::at(4301, 115708, -182362, -1449, 0, 0),
        SpawnSpec::at(4301, -44337, -113669, -224, 0, 0),
        SpawnSpec::at(4301, -44628, -115409, -240, 22500, 0),
        SpawnSpec::at(4301, -13073, 122801, -3117, 0, 0),
        SpawnSpec::at(4301, -13949, 121934, -2988, 32768, 0),
        SpawnSpec::at(4301, -14822, 123708, -3117, 8192, 0),
        SpawnSpec::at(4301, -80762, 151118, -3043, 28672, 0),
        SpawnSpec::at(4301, -84049, 150176, -3129, 4096, 0),
        SpawnSpec::at(4301, -82623, 151666, -3129, 49152, 0),
        SpawnSpec::at(4301, -84516, 242971, -3730, 34000, 0),
        SpawnSpec::at(4301, -86003, 243205, -3730, 60000, 0),
        SpawnSpec::at(4301, 11281, 15652, -4584, 25000, 0),
        SpawnSpec::at(4301, 11303, 17732, -4574, 57344, 0),
        SpawnSpec::at(4301, 47151, 49436, -3059, 32000, 0),
        SpawnSpec::at(4301, 79806, 55570, -1560, 0, 0),
        SpawnSpec::at(4301, 83328, 55824, -1525, 32768, 0),
        SpawnSpec::at(4301, 80986, 54504, -1525, 32768, 0),
        SpawnSpec::at(4301, 18178, 145149, -3054, 7400, 0),
        SpawnSpec::at(4301, 19208, 144380, -3097, 32768, 0),
        SpawnSpec::at(4301, 19508, 145775, -3086, 48000, 0),
        SpawnSpec::at(4301, 17396, 170259, -3507, 30000, 0),
        SpawnSpec::at(4301, 83332, 149160, -3405, 49152, 0),
        SpawnSpec::at(4301, 82277, 148598, -3467, 0, 0),
        SpawnSpec::at(4301, 81621, 148725, -3467, 32768, 0),
        SpawnSpec::at(4301, 81680, 145656, -3533, 32768, 0),
        SpawnSpec::at(4301, 117498, 76630, -2695, 38000, 0),
        SpawnSpec::at(4301, 115914, 76449, -2711, 59000, 0),
        SpawnSpec::at(4301, 119536, 76988, -2275, 40960, 0),
        SpawnSpec::at(4301, 147120, 27312, -2192, 40960, 0),
        SpawnSpec::at(4301, 147920, 25664, -2000, 16384, 0),
        SpawnSpec::at(4301, 111776, 221104, -3543, 16384, 0),
        SpawnSpec::at(4301, 107904, 218096, -3675, 0, 0),
        SpawnSpec::at(4301, 114920, 220020, -3632, 32768, 0),
        SpawnSpec::at(4301, 147888, -58048, -2979, 49000, 0),
        SpawnSpec::at(4301, 147285, -56461, -2776, 11500, 0),
        SpawnSpec::at(4301, 44176, -48732, -800, 33000, 0),
        SpawnSpec::at(4301, 44294, -47642, -792, 50000, 0),
        SpawnSpec::at(4301, -116677, 46824, 360, 34828, 0),
    ],
    on_enter_msg: "Valentine's Event is currently active. Collect all items and make your Valentine Cake!",
    on_end_msg: "The Valentine Event end!",
    destroy_items_on_end: &[],
};

/// Java `events.TheValentineEvent.TheValentineEvent`.
pub struct TheValentineEvent;

impl Script for TheValentineEvent {
    fn name(&self) -> &'static str {
        "TheValentineEvent"
    }

    fn path(&self) -> &'static str {
        "events/TheValentineEvent"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NPC]);
        s.add_first_talk_id(&[NPC]);
        s.add_talk_id(&[NPC]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        long_time_event::load(ctx, &CONFIG);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if long_time_event::on_event(ctx, &CONFIG, event, npc) {
            return None;
        }
        let player = player?;
        let mut htmltext = Some(event.to_owned());
        if event.eq_ignore_ascii_case("4301-3.htm") {
            // NOTE(java-quirk): nothing ever sets `COMPLETED`, so the recipe can be asked for again
            // and again.
            if ctx
                .player_variable(player, COMPLETED)
                .is_some_and(|v| v.eq_ignore_ascii_case("true"))
            {
                htmltext = Some("4301-4.htm".to_owned());
            } else {
                ctx.give_items(player, RECIPE, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.htm", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="4301" type="Folk" level="70" name="Miss Queen"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="20191" type="EtcItem" name="Valentine Cake Recipe"/>
        <item id="20192" type="EtcItem" name="Valentine Cake Ingredient A"/>
        <item id="20193" type="EtcItem" name="Valentine Cake Ingredient B"/>
        <item id="20194" type="EtcItem" name="Valentine Cake Ingredient C"/>"#;
    /// 2001-12-15 10:00 UTC, a Saturday inside the event.
    const INSIDE: i64 = 1_008_410_400_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(TheValentineEvent);
        let pages = Pages::of(&TheValentineEvent)
            .page("4301.htm", &["4301-1.htm"])
            .page("4301-1.htm", &["4301-2.htm"])
            .page("4301-2.htm", &["4301-3.htm"])
            .plain(&["4301-3.htm", "4301-4.htm"])
            .npcs(&[NPC]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    #[test]
    fn inside_the_dates_the_event_drops_spawns_and_announces() {
        let mut t = setup(Some(INSIDE));
        assert_eq!(
            t.world.event_drops_of("TheValentineEvent").map(<[_]>::len),
            Some(3)
        );
        // The config has 41 spawn points of Miss Queen.
        assert_eq!(t.npcs_of(NPC).len(), 41);
        assert_eq!(t.world.announcement_count(), 1);
        // The enter message goes to the players that enter later as an announcement.
        t.enter(1, 20, vec![]);
        t.take();
    }

    #[test]
    fn outside_the_dates_nothing_starts() {
        // The shipped dates are in 2001.
        let t = setup(None);
        assert!(t.world.event_drops_of("TheValentineEvent").is_none());
        assert!(t.npcs_of(NPC).is_empty());
        assert_eq!(t.world.announcement_count(), 0);
    }

    #[test]
    fn the_recipe_is_given_every_time() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 20, vec![]);
        let queen = t.spawn(NPC, 20);
        t.talk(1, queen);
        t.take();
        assert!(
            t.event(1, "TheValentineEvent", "4301-1.htm")
                .contains("4301-1.htm")
        );
        assert!(
            t.event(1, "TheValentineEvent", "4301-2.htm")
                .contains("4301-2.htm")
        );
        assert!(
            t.event(1, "TheValentineEvent", "4301-3.htm")
                .contains("4301-3.htm")
        );
        assert_eq!(t.count(1, 20191), 1);
        // The recipe again: nothing sets the completed flag.
        t.world.run_as_script("TheValentineEvent", |ctx| {
            let _ = TheValentineEvent.on_event(ctx, "4301-3.htm", None, Some(l2_util::ObjectId(1)));
        });
        assert_eq!(t.count(1, 20191), 2);
    }

    #[test]
    fn a_completed_player_is_sent_to_the_other_page() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 20, vec![]);
        t.world.run_as_script("TheValentineEvent", |ctx| {
            ctx.set_player_variable(l2_util::ObjectId(1), COMPLETED, "true");
            let page = TheValentineEvent.on_event(ctx, "4301-3.htm", None, Some(l2_util::ObjectId(1)));
            assert_eq!(page.as_deref(), Some("4301-4.htm"));
        });
        assert_eq!(t.count(1, 20191), 0);
    }
}
