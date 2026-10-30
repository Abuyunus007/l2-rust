//! Love Your Gatekeeper: 10,000 Adena buy the gatekeeper transformation stick once a day.
// Java: dist/game/data/scripts/events/LoveYourGatekeeper/LoveYourGatekeeper.java
// Java: dist/game/data/scripts/events/LoveYourGatekeeper/config.xml

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::hours_and_minutes;
use super::long_time_event::{self, EventConfig, SpawnSpec};

// NPC
const GATEKEEPER: i32 = 32477;

// Item
const GATEKEEPER_TRANSFORMATION_STICK: i32 = 12814;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Skills
const TELEPORTER_TRANSFORM: SkillKey = SkillKey { id: 5655, level: 1 };

// Misc
const HOURS: i64 = 24;
const PRICE: i64 = 10000;
/// Java `LoveYourGatekeeper.class.getSimpleName() + "_reuse"`.
const REUSE: &str = "LoveYourGatekeeper_reuse";

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Love Your Gatekeeper",
    active: "27 02 2008-28 02 2008",
    drop_period: None,
    drops: &[],
    spawns: &[
        SpawnSpec::at(32477, -80762, 151118, -3043, 28672, 0),
        SpawnSpec::at(32477, -84046, 150193, -3129, 4096, 0),
        SpawnSpec::at(32477, -82675, 151652, -3129, 46943, 0),
        SpawnSpec::at(32477, -12992, 122818, -3117, 0, 0),
        SpawnSpec::at(32477, -13964, 121947, -2988, 32768, 0),
        SpawnSpec::at(32477, -14823, 123752, -3117, 8192, 0),
        SpawnSpec::at(32477, 18178, 145149, -3054, 7400, 0),
        SpawnSpec::at(32477, 19185, 144377, -3097, 32768, 0),
        SpawnSpec::at(32477, 19508, 145753, -3086, 47999, 0),
        SpawnSpec::at(32477, 17396, 170259, -3507, 30000, 0),
        SpawnSpec::at(32477, 44150, -48708, -800, 32999, 0),
        SpawnSpec::at(32477, 44280, -47664, -792, 49167, 0),
        SpawnSpec::at(32477, 79806, 55570, -1560, 0, 0),
        SpawnSpec::at(32477, 83328, 55824, -1525, 32768, 0),
        SpawnSpec::at(32477, 80986, 54504, -1525, 32768, 0),
        SpawnSpec::at(32477, 83358, 149223, -3400, 32768, 0),
        SpawnSpec::at(32477, 82277, 148598, -3467, 0, 0),
        SpawnSpec::at(32477, 81621, 148725, -3467, 32768, 0),
        SpawnSpec::at(32477, 81680, 145656, -3533, 32768, 0),
        SpawnSpec::at(32477, 117498, 76630, -2695, 38000, 0),
        SpawnSpec::at(32477, 119536, 76988, -2275, 40960, 0),
        SpawnSpec::at(32477, 111585, 221011, -3544, 16384, 0),
        SpawnSpec::at(32477, 107922, 218094, -3675, 0, 0),
        SpawnSpec::at(32477, 114920, 220020, -3632, 32768, 0),
        SpawnSpec::at(32477, 147888, -58048, -2979, 49000, 0),
        SpawnSpec::at(32477, 147285, -56461, -2776, 11500, 0),
        SpawnSpec::at(32477, 147120, 27312, -2192, 40960, 0),
        SpawnSpec::at(32477, 147959, 25695, -2000, 16384, 0),
        SpawnSpec::at(32477, 87792, -142240, -1343, 44000, 0),
        SpawnSpec::at(32477, 87557, -140657, -1542, 20476, 0),
        SpawnSpec::at(32477, 115933, 76482, -2711, 58999, 0),
    ],
    on_enter_msg: "Love Your Gatekeeper: Event ongoing!",
    on_end_msg: "Love Your Gatekeeper: Event end!",
    destroy_items_on_end: &[],
};

/// Java `events.LoveYourGatekeeper.LoveYourGatekeeper`.
pub struct LoveYourGatekeeper;

impl Script for LoveYourGatekeeper {
    fn name(&self) -> &'static str {
        "LoveYourGatekeeper"
    }

    fn path(&self) -> &'static str {
        "events/LoveYourGatekeeper"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GATEKEEPER]);
        s.add_first_talk_id(&[GATEKEEPER]);
        s.add_talk_id(&[GATEKEEPER]);
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
        match event {
            "transform_stick" => {
                if ctx.adena_of(player) >= PRICE {
                    let reuse: i64 = ctx
                        .player_variable(player, REUSE)
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if reuse > ctx.now_ms() {
                        let remaining_time = reuse - ctx.now_ms();
                        let (hours, minutes) = hours_and_minutes(remaining_time);
                        let sm = SystemMessage::new(
                            SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS,
                        )
                        .with(ctx.world().item_message_param(GATEKEEPER_TRANSFORMATION_STICK))
                        .with(SystemMessageParam::IntNumber(hours))
                        .with(SystemMessageParam::IntNumber(minutes));
                        ctx.world().send(player, &sm);
                    } else {
                        ctx.take_items(player, ADENA_ID, PRICE);
                        ctx.give_items(player, GATEKEEPER_TRANSFORMATION_STICK, 1);
                        let until = ctx.now_ms() + HOURS * 3_600_000;
                        ctx.set_player_variable(player, REUSE, &until.to_string());
                    }
                } else {
                    return Some("32477-3.htm".to_owned());
                }

                None
            }
            "transform" => {
                if !ctx.world().is_transformed(player) {
                    ctx.world_mut().do_cast(player, TELEPORTER_TRANSFORM);
                }

                None
            }
            _ => Some(event.to_owned()),
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32477.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32477" type="Folk" level="70" name="Gatekeeper"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="12814" type="EtcItem" name="Gatekeeper Transformation Stick"/>"#;
    /// 2008-02-27 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_204_113_600_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(LoveYourGatekeeper);
        let pages = Pages::of(&LoveYourGatekeeper)
            .page("32477.htm", &["transform_stick", "transform", "32477-1.htm"])
            .plain(&["32477-1.htm", "32477-3.htm"])
            .npcs(&[GATEKEEPER]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    #[test]
    fn inside_the_dates_the_gatekeepers_stand_and_outside_they_do_not() {
        assert_eq!(setup(Some(INSIDE)).npcs_of(GATEKEEPER).len(), 31);
        assert!(setup(None).npcs_of(GATEKEEPER).is_empty());
    }

    #[test]
    fn the_stick_costs_10000_adena_once_a_day() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 25_000)]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        t.talk(1, gatekeeper);
        // Buying: the page stays (null), the stick comes and the reuse is stored.
        t.event(1, "LoveYourGatekeeper", "transform_stick");
        assert_eq!(t.count(1, GATEKEEPER_TRANSFORMATION_STICK), 1);
        assert_eq!(t.count(1, ADENA_ID), 15_000);
        assert!(
            t.world
                .player(ObjectId(1))
                .and_then(|p| p.data.profile.variables.get(REUSE).cloned())
                .is_some()
        );

        // Again the same day: told the time, nothing is taken.
        t.talk(1, gatekeeper);
        t.take();
        let (_, names) = crate::events::click(&mut t, 1, "LoveYourGatekeeper", "transform_stick");
        assert_eq!(t.count(1, GATEKEEPER_TRANSFORMATION_STICK), 1);
        assert_eq!(t.count(1, ADENA_ID), 15_000);
        assert!(names.contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn without_the_adena_the_poor_page_comes() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 9_999)]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        t.talk(1, gatekeeper);
        assert!(
            t.event(1, "LoveYourGatekeeper", "transform_stick")
                .contains("32477-3.htm")
        );
        assert_eq!(t.count(1, GATEKEEPER_TRANSFORMATION_STICK), 0);
    }

    #[test]
    fn other_events_are_echoed() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        t.talk(1, gatekeeper);
        assert!(
            t.event(1, "LoveYourGatekeeper", "32477-1.htm")
                .contains("32477-1.htm")
        );
    }
}
