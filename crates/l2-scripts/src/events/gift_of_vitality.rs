//! Gift of Vitality: Steve Shyagel casts the vitality gift and the newbie memories buffs.
// Java: dist/game/data/scripts/events/GiftOfVitality/GiftOfVitality.java
// Java: dist/game/data/scripts/events/GiftOfVitality/config.xml

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::hours_and_minutes;
use super::long_time_event::{self, EventConfig, SpawnSpec};

// NPC
const STEVE_SHYAGEL: i32 = 4306;

// Skills
const GIFT_OF_VITALITY: SkillKey = skill(23179, 1);
const JOY_OF_VITALITY: SkillKey = skill(23180, 1);

const fn skill(id: i32, level: i32) -> SkillKey {
    SkillKey { id, level }
}

const FIGHTER_SKILLS: [SkillKey; 7] = [
    skill(5627, 1), // Wind Walk
    skill(5628, 1), // Shield
    skill(5637, 1), // Magic Barrier
    skill(5629, 1), // Bless the Body
    skill(5630, 1), // Vampiric Rage
    skill(5631, 1), // Regeneration
    skill(5632, 1), // Haste
];

const MAGE_SKILLS: [SkillKey; 7] = [
    skill(5627, 1), // Wind Walk
    skill(5628, 1), // Shield
    skill(5637, 1), // Magic Barrier
    skill(5633, 1), // Bless the Soul
    skill(5634, 1), // Acumen
    skill(5635, 1), // Concentration
    skill(5636, 1), // Empower
];

const SERVITOR_SKILLS: [SkillKey; 11] = [
    skill(5627, 1), // Wind Walk
    skill(5628, 1), // Shield
    skill(5637, 1), // Magic Barrier
    skill(5629, 1), // Bless the Body
    skill(5633, 1), // Bless the Soul
    skill(5630, 1), // Vampiric Rage
    skill(5634, 1), // Acumen
    skill(5631, 1), // Regeneration
    skill(5635, 1), // Concentration
    skill(5632, 1), // Haste
    skill(5636, 1), // Empower
];

// Misc
const HOURS: i64 = 5; // Reuse between buffs
const MIN_LEVEL: i32 = 75;
/// Java `GiftOfVitality.class.getSimpleName() + "_reuse"`.
const REUSE: &str = "GiftOfVitality_reuse";

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Gift of Vitality",
    active: "27 02 2008-28 02 2008",
    drop_period: None,
    drops: &[],
    spawns: &[
        SpawnSpec::at(4306, 87116, -141332, -1336, 52193, 0),
        SpawnSpec::at(4306, -13869, 122063, -2984, 18270, 0),
        SpawnSpec::at(4306, 17203, 144949, -3024, 18166, 0),
        SpawnSpec::at(4306, 82766, 149438, -3464, 33865, 0),
        SpawnSpec::at(4306, -83161, 150915, -3120, 17311, 0),
        SpawnSpec::at(4306, -84037, 243194, -3728, 8992, 0),
        SpawnSpec::at(4306, 45402, 48355, -3056, 49153, 0),
        SpawnSpec::at(4306, 12084, 16576, -4584, 57345, 0),
        SpawnSpec::at(4306, 82286, 53291, -1488, 15250, 0),
        SpawnSpec::at(4306, 147060, 25943, -2008, 46171, 0),
        SpawnSpec::at(4306, 116868, 77309, -2688, 40353, 0),
        SpawnSpec::at(4306, -119690, 44583, 360, 29289, 0),
        SpawnSpec::at(4306, 111164, 221062, -3544, 2714, 0),
        SpawnSpec::at(4306, -44928, -113608, -192, 30212, 0),
        SpawnSpec::at(4306, 115616, -177941, -896, 30708, 0),
        SpawnSpec::at(4306, 148096, -55466, -2728, 40541, 0),
        SpawnSpec::at(4306, 43521, -47542, -792, 31655, 0),
    ],
    on_enter_msg: "Gift of Vitality: Event ongoing!",
    on_end_msg: "Gift of Vitality: Event end!",
    destroy_items_on_end: &[],
};

/// Java `player.getVariables().getLong(REUSE, 0)`: a `character_variables` value as a long.
fn reuse_of(ctx: &ScriptCtx<'_>, player: ObjectId) -> i64 {
    ctx.player_variable(player, REUSE)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// Java `events.GiftOfVitality.GiftOfVitality`.
pub struct GiftOfVitality;

impl Script for GiftOfVitality {
    fn name(&self) -> &'static str {
        "GiftOfVitality"
    }

    fn path(&self) -> &'static str {
        "events/GiftOfVitality"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STEVE_SHYAGEL]);
        s.add_first_talk_id(&[STEVE_SHYAGEL]);
        s.add_talk_id(&[STEVE_SHYAGEL]);
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
        match event {
            "vitality" => {
                let reuse = reuse_of(ctx, player);
                if reuse > ctx.now_ms() {
                    let remaining_time = reuse - ctx.now_ms();
                    let (hours, minutes) = hours_and_minutes(remaining_time);
                    let sm =
                        SystemMessage::new(SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS)
                            .with(SystemMessageParam::SkillName { id: 23179, level: 1 })
                            .with(SystemMessageParam::IntNumber(hours))
                            .with(SystemMessageParam::IntNumber(minutes));
                    ctx.world().send(player, &sm);
                    htmltext = Some("4306-notime.htm".to_owned());
                } else {
                    ctx.world_mut().do_cast(player, GIFT_OF_VITALITY);
                    ctx.world_mut().do_simultaneous_cast(player, JOY_OF_VITALITY);
                    let until = ctx.now_ms() + HOURS * 3_600_000;
                    ctx.set_player_variable(player, REUSE, &until.to_string());
                    htmltext = Some("4306-okvitality.htm".to_owned());
                }
            }
            "memories_player" => {
                if ctx.player_level(player) <= MIN_LEVEL {
                    htmltext = Some("4306-nolevel.htm".to_owned());
                } else {
                    let skills = if ctx.is_mage_class(player) {
                        &MAGE_SKILLS[..]
                    } else {
                        &FIGHTER_SKILLS[..]
                    };
                    let npc = npc?;
                    for &sk in skills {
                        ctx.cast_skill(npc, player, sk);
                    }

                    htmltext = Some("4306-okbuff.htm".to_owned());
                }
            }
            "memories_summon" => {
                if ctx.player_level(player) <= MIN_LEVEL {
                    htmltext = Some("4306-nolevel.htm".to_owned());
                } else if let Some(summon) = ctx.summon_of(player) {
                    let npc = npc?;
                    for &sk in &SERVITOR_SKILLS {
                        ctx.cast_skill(npc, summon, sk);
                    }

                    htmltext = Some("4306-okbuff.htm".to_owned());
                } else {
                    htmltext = Some("4306-nosummon.htm".to_owned());
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("4306.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="4306" type="Folk" level="70" name="Steve Shyagel"/>"#;
    /// 2008-02-27 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_204_113_600_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(GiftOfVitality);
        let pages = Pages::of(&GiftOfVitality)
            .page("4306.htm", &["vitality", "memories_player", "memories_summon"])
            .page(
                "4306-notime.htm",
                &["vitality", "memories_player", "memories_summon"],
            )
            .page(
                "4306-okvitality.htm",
                &["vitality", "memories_player", "memories_summon"],
            )
            .page(
                "4306-nolevel.htm",
                &["vitality", "memories_player", "memories_summon"],
            )
            .page(
                "4306-okbuff.htm",
                &["vitality", "memories_player", "memories_summon"],
            )
            .page(
                "4306-nosummon.htm",
                &["vitality", "memories_player", "memories_summon"],
            )
            .npcs(&[STEVE_SHYAGEL]);
        ScriptTest::with_pages(registry, NPCS, "", pages)
    }

    #[test]
    fn inside_the_dates_the_npcs_stand_and_outside_they_do_not() {
        let inside = setup(Some(INSIDE));
        assert_eq!(inside.npcs_of(STEVE_SHYAGEL).len(), 17);
        // The event has no droplist: Java still hands the (empty) list to the manager.
        assert_eq!(
            inside.world.event_drops_of("GiftOfVitality").map(<[_]>::len),
            Some(0)
        );
        let outside = setup(None);
        assert!(outside.npcs_of(STEVE_SHYAGEL).is_empty());
    }

    #[test]
    fn the_gift_is_given_once_every_five_hours() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 80, vec![]);
        let steve = t.spawn(STEVE_SHYAGEL, 20);
        t.talk(1, steve);
        assert!(
            t.event(1, "GiftOfVitality", "vitality")
                .contains("4306-okvitality.htm")
        );
        let until: i64 = t
            .world
            .player(ObjectId(1))
            .and_then(|p| p.data.profile.variables.get(REUSE).cloned())
            .unwrap()
            .parse()
            .unwrap();
        assert!(until > l2_util::time::now_millis() + 4 * 3_600_000);

        // Asking again inside the five hours: the page says no and the player is told when.
        t.talk(1, steve);
        let (page, names) = crate::events::click(&mut t, 1, "GiftOfVitality", "vitality");
        assert!(page.contains("4306-notime.htm"));
        assert!(
            names.contains(&"SYSTEM_MESSAGE".to_owned()),
            "the remaining time goes out as a system message"
        );
    }

    #[test]
    fn the_buffs_need_a_level_above_75_and_a_servitor_for_the_summon() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 75, vec![]);
        let steve = t.spawn(STEVE_SHYAGEL, 20);
        t.talk(1, steve);
        assert!(
            t.event(1, "GiftOfVitality", "memories_player")
                .contains("4306-nolevel.htm")
        );
        assert!(
            t.event(1, "GiftOfVitality", "memories_summon")
                .contains("4306-nolevel.htm")
        );

        t.enter(2, 76, vec![]);
        t.talk(2, steve);
        assert!(
            t.event(2, "GiftOfVitality", "memories_player")
                .contains("4306-okbuff.htm")
        );
        assert!(
            t.event(2, "GiftOfVitality", "memories_summon")
                .contains("4306-nosummon.htm")
        );
    }
}
