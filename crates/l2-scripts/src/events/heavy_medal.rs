//! Heavy Medals: Cat Roy and Cat Winnie swap glittering medals for badges on a coin toss.
// Java: dist/game/data/scripts/events/HeavyMedal/HeavyMedal.java
// Java: dist/game/data/scripts/events/HeavyMedal/config.xml

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

const CAT_ROY: i32 = 31228;
const CAT_WINNIE: i32 = 31229;
const GLITTERING_MEDAL: i32 = 6393;

const WIN_CHANCE: i32 = 50;

const MEDALS: [i64; 4] = [5, 10, 20, 40];
const BADGES: [i32; 4] = [6399, 6400, 6401, 6402];

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Heavy Medal",
    active: "27 02 2007-28 02 2007",
    drop_period: Some("27 02 2007-20 02 2007"),
    drops: &[
        DropSpec {
            item: 6392,
            min: 1,
            max: 1,
            chance: 45.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 6393,
            min: 1,
            max: 1,
            chance: 5.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
    ],
    spawns: &[
        SpawnSpec::at(31228, -44337, -113669, -224, 0, 0),
        SpawnSpec::at(31228, -44628, -115409, -240, 22500, 0),
        SpawnSpec::at(31228, -13073, 122801, -3117, 0, 0),
        SpawnSpec::at(31228, -13949, 121934, -2988, 32768, 0),
        SpawnSpec::at(31228, -14786, 123686, -3117, 8192, 0),
        SpawnSpec::at(31228, 11281, 15652, -4584, 25000, 0),
        SpawnSpec::at(31228, 11303, 17732, -4574, 57344, 0),
        SpawnSpec::at(31228, 18178, 145149, -3054, 7400, 0),
        SpawnSpec::at(31228, 19208, 144380, -3097, 32768, 0),
        SpawnSpec::at(31228, 19508, 145775, -3086, 48000, 0),
        SpawnSpec::at(31228, 17396, 170259, -3507, 36000, 0),
        SpawnSpec::at(31228, 47151, 49436, -3059, 32000, 0),
        SpawnSpec::at(31228, 44122, 50784, -3059, 57344, 0),
        SpawnSpec::at(31228, 79806, 55570, -1560, 0, 0),
        SpawnSpec::at(31228, 83328, 55824, -1525, 32768, 0),
        SpawnSpec::at(31228, 80986, 54504, -1525, 32768, 0),
        SpawnSpec::at(31228, 83332, 149160, -3405, 49152, 0),
        SpawnSpec::at(31228, 82277, 148598, -3467, 0, 0),
        SpawnSpec::at(31228, 81621, 148725, -3467, 32768, 0),
        SpawnSpec::at(31228, 81680, 145656, -3467, 32768, 0),
        SpawnSpec::at(31228, 114733, -178691, -821, 0, 0),
        SpawnSpec::at(31228, 115708, -182362, -1449, 0, 0),
        SpawnSpec::at(31228, -80789, 151073, -3043, 28672, 0),
        SpawnSpec::at(31228, -84049, 150176, -3129, 4096, 0),
        SpawnSpec::at(31228, -82623, 151666, -3129, 49152, 0),
        SpawnSpec::at(31228, 117498, 76630, -2695, 38000, 0),
        SpawnSpec::at(31228, 115914, 76449, -2711, 59000, 0),
        SpawnSpec::at(31228, 119536, 76988, -2275, 40960, 0),
        SpawnSpec::at(31228, -84516, 242971, -3730, 34000, 0),
        SpawnSpec::at(31228, -86003, 243205, -3730, 60000, 0),
        SpawnSpec::at(31228, 147184, 27405, -2192, 17000, 0),
        SpawnSpec::at(31228, 147920, 25664, -2000, 16384, 0),
        SpawnSpec::at(31228, 111776, 221104, -3543, 16384, 0),
        SpawnSpec::at(31228, 107904, 218096, -3675, 0, 0),
        SpawnSpec::at(31228, 114920, 220020, -3632, 32768, 0),
        SpawnSpec::at(31228, 147888, -58048, -2979, 49000, 0),
        SpawnSpec::at(31228, 147262, -56450, -2776, 33000, 0),
        SpawnSpec::at(31228, 44176, -48732, -800, 33000, 0),
        SpawnSpec::at(31228, 44319, -47640, -792, 50000, 0),
        SpawnSpec::at(31229, -44342, -113726, -240, 0, 0),
        SpawnSpec::at(31229, -44671, -115437, -240, 22500, 0),
        SpawnSpec::at(31229, -13073, 122841, -3117, 0, 0),
        SpawnSpec::at(31229, -13972, 121893, -2988, 32768, 0),
        SpawnSpec::at(31229, -14843, 123710, -3117, 8192, 0),
        SpawnSpec::at(31229, 11327, 15682, -4584, 25000, 0),
        SpawnSpec::at(31229, 11243, 17712, -4574, 57344, 0),
        SpawnSpec::at(31229, 18154, 145192, -3054, 7400, 0),
        SpawnSpec::at(31229, 19214, 144327, -3097, 32768, 0),
        SpawnSpec::at(31229, 19459, 145775, -3086, 48000, 0),
        SpawnSpec::at(31229, 17418, 170217, -3507, 36000, 0),
        SpawnSpec::at(31229, 47146, 49382, -3059, 32000, 0),
        SpawnSpec::at(31229, 44157, 50827, -3059, 57344, 0),
        SpawnSpec::at(31229, 79798, 55629, -1560, 0, 0),
        SpawnSpec::at(31229, 83328, 55769, -1525, 32768, 0),
        SpawnSpec::at(31229, 80986, 54452, -1525, 32768, 0),
        SpawnSpec::at(31229, 83329, 149095, -3405, 49152, 0),
        SpawnSpec::at(31229, 82277, 148564, -3467, 0, 0),
        SpawnSpec::at(31229, 81620, 148689, -3464, 32768, 0),
        SpawnSpec::at(31229, 81691, 145610, -3467, 32768, 0),
        SpawnSpec::at(31229, 114719, -178742, -821, 0, 0),
        SpawnSpec::at(31229, 115708, -182422, -1449, 0, 0),
        SpawnSpec::at(31229, -80731, 151152, -3043, 28672, 0),
        SpawnSpec::at(31229, -84097, 150171, -3129, 4096, 0),
        SpawnSpec::at(31229, -82678, 151666, -3129, 49152, 0),
        SpawnSpec::at(31229, 117459, 76664, -2695, 38000, 0),
        SpawnSpec::at(31229, 115936, 76488, -2711, 59000, 0),
        SpawnSpec::at(31229, 119576, 76940, -2275, 40960, 0),
        SpawnSpec::at(31229, -84516, 243015, -3730, 34000, 0),
        SpawnSpec::at(31229, -86031, 243153, -3730, 60000, 0),
        SpawnSpec::at(31229, 147124, 27401, -2192, 40960, 0),
        SpawnSpec::at(31229, 147985, 25664, -2000, 16384, 0),
        SpawnSpec::at(31229, 111724, 221111, -3543, 16384, 0),
        SpawnSpec::at(31229, 107899, 218149, -3675, 0, 0),
        SpawnSpec::at(31229, 114920, 220080, -3632, 32768, 0),
        SpawnSpec::at(31229, 147924, -58052, -2979, 49000, 0),
        SpawnSpec::at(31229, 147285, -56461, -2776, 33000, 0),
        SpawnSpec::at(31229, 44176, -48688, -800, 33000, 0),
        SpawnSpec::at(31229, 44294, -47642, -792, 50000, 0),
    ],
    on_enter_msg: "Heavy Medal Event: Collect Medals and trade them for prizes at Event Cats in every town! Hurry up, since the event will end at Feb 28th!",
    on_end_msg: "Heavy Medal: Event end!",
    destroy_items_on_end: &[],
};

/// Java `checkLevel(player)`: the best badge the player holds.
fn check_level(ctx: &ScriptCtx<'_>, player: ObjectId) -> usize {
    let mut level = 0;
    if ctx.has_quest_items(player, 6402) {
        level = 4;
    } else if ctx.has_quest_items(player, 6401) {
        level = 3;
    } else if ctx.has_quest_items(player, 6400) {
        level = 2;
    } else if ctx.has_quest_items(player, 6399) {
        level = 1;
    }
    level
}

/// Java `events.HeavyMedal.HeavyMedal`.
pub struct HeavyMedal;

impl Script for HeavyMedal {
    fn name(&self) -> &'static str {
        "HeavyMedal"
    }

    fn path(&self) -> &'static str {
        "events/HeavyMedal"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CAT_ROY]);
        s.add_start_npc(&[CAT_WINNIE]);
        s.add_talk_id(&[CAT_ROY]);
        s.add_talk_id(&[CAT_WINNIE]);
        s.add_first_talk_id(&[CAT_ROY]);
        s.add_first_talk_id(&[CAT_WINNIE]);
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
        let mut level = check_level(ctx, player);
        if event.eq_ignore_ascii_case("game") {
            htmltext = Some(
                if ctx.get_quest_items_count(player, GLITTERING_MEDAL) < MEDALS[level] {
                    "31229-no.htm"
                } else {
                    "31229-game.htm"
                }
                .to_owned(),
            );
        } else if event.eq_ignore_ascii_case("heads") || event.eq_ignore_ascii_case("tails") {
            let side = event.to_lowercase();
            if ctx.get_quest_items_count(player, GLITTERING_MEDAL) < MEDALS[level] {
                htmltext = Some(format!("31229-{side}-10.htm"));
            } else {
                ctx.take_items(player, GLITTERING_MEDAL, MEDALS[level]);
                if ctx.get_random(100) > WIN_CHANCE {
                    level = 0;
                } else {
                    if level > 0 {
                        ctx.take_items(player, BADGES[level - 1], -1);
                    }

                    ctx.give_items(player, BADGES[level], 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    level += 1;
                }

                htmltext = Some(format!("31229-{side}-{level}.htm"));
            }
        } else if event.eq_ignore_ascii_case("talk") {
            htmltext = Some(format!("{}-lvl-{level}.htm", npc?.id));
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.player_quest_state(player, "HeavyMedal").is_none() {
            // `newQuestState(player)`
            ctx.get_quest_state(player, true);
        }

        Some(format!("{}.htm", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="31228" type="Folk" level="70" name="Cat Roy"/>
        <npc id="31229" type="Folk" level="70" name="Cat Winnie"/>"#;
    const ITEMS: &str = r#"
        <item id="6392" type="EtcItem" name="Medal of Glory"><set name="is_stackable" val="true"/></item>
        <item id="6393" type="EtcItem" name="Glittering Medal"><set name="is_stackable" val="true"/></item>
        <item id="6399" type="EtcItem" name="Badge 1"/>
        <item id="6400" type="EtcItem" name="Badge 2"/>
        <item id="6401" type="EtcItem" name="Badge 3"/>
        <item id="6402" type="EtcItem" name="Badge 4"/>"#;
    /// 2007-02-27 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_172_577_600_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(HeavyMedal);
        let pages = Pages::of(&HeavyMedal)
            .page("31229.htm", &["game", "talk"])
            .page("31229-game.htm", &["heads", "tails"])
            .page("31229-heads-0.htm", &["game"])
            .page("31229-heads-1.htm", &["game"])
            .page("31229-heads-10.htm", &["game"])
            .page("31229-tails-0.htm", &["game"])
            .page("31229-tails-1.htm", &["game"])
            .page("31229-tails-2.htm", &["game"])
            .plain(&["31229-no.htm", "31229-lvl-0.htm", "31229-lvl-2.htm"])
            .npcs(&[CAT_ROY, CAT_WINNIE]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    #[test]
    fn inside_the_dates_the_medals_drop_and_the_cats_stand() {
        let t = setup(Some(INSIDE));
        let drops = t.world.event_drops_of("HeavyMedal");
        // Java quirk: the shipped drop period ends (20 02) before it starts (27 02), and the
        // drops start only when now is inside the drop period, which is never.
        assert!(drops.is_none());
        assert_eq!(t.npcs_of(CAT_ROY).len() + t.npcs_of(CAT_WINNIE).len(), 78);
    }

    #[test]
    fn outside_the_dates_nothing_starts() {
        let t = setup(None);
        assert!(t.npcs_of(CAT_ROY).is_empty() && t.npcs_of(CAT_WINNIE).is_empty());
        assert!(t.world.event_drops_of("HeavyMedal").is_none());
    }

    #[test]
    fn a_toss_costs_the_medals_and_gives_a_badge_or_nothing() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, GLITTERING_MEDAL, 5 * 40)]);
        let winnie = t.spawn(CAT_WINNIE, 20);
        t.talk(1, winnie);
        // The first click made the quest state of the script.
        assert!(
            t.world
                .player_quests(ObjectId(1))
                .is_some_and(|q| q.contains("HeavyMedal"))
        );
        assert!(t.event(1, "HeavyMedal", "game").contains("31229-game.htm"));
        // Toss until the seeded roll wins once: the badge 1 arrives and 5 medals went each time.
        let mut tosses = 0;
        while t.count(1, BADGES[0]) == 0 && tosses < 40 {
            let page = t.event(1, "HeavyMedal", "heads");
            assert!(page.contains("31229-heads-"));
            // The toss is over; go back to the game page for the next one.
            t.bypass(1, "Script HeavyMedal game");
            tosses += 1;
        }
        assert_eq!(t.count(1, BADGES[0]), 1);
        assert_eq!(t.count(1, GLITTERING_MEDAL), 5 * (40 - tosses));
    }

    #[test]
    fn without_enough_medals_the_game_says_no() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, GLITTERING_MEDAL, 4)]);
        let winnie = t.spawn(CAT_WINNIE, 20);
        t.talk(1, winnie);
        assert!(t.event(1, "HeavyMedal", "game").contains("31229-no.htm"));
    }

    #[test]
    fn the_next_badge_replaces_the_old_one() {
        let mut t = setup(Some(INSIDE));
        t.enter(
            1,
            40,
            vec![item(1000, GLITTERING_MEDAL, 10 * 30), item(1001, BADGES[0], 1)],
        );
        let winnie = t.spawn(CAT_WINNIE, 20);
        t.talk(1, winnie);
        t.world.run_as_script("HeavyMedal", |ctx| {
            assert_eq!(check_level(ctx, ObjectId(1)), 1);
        });
        let mut tosses = 0;
        while t.count(1, BADGES[1]) == 0 && tosses < 30 {
            t.bypass(1, "Script HeavyMedal game");
            t.bypass(1, "Script HeavyMedal tails");
            tosses += 1;
        }
        assert_eq!(t.count(1, BADGES[1]), 1);
        assert_eq!(t.count(1, BADGES[0]), 0);
    }

    #[test]
    fn the_talk_page_depends_on_the_badge_level() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, BADGES[1], 1)]);
        let winnie = t.spawn(CAT_WINNIE, 20);
        t.talk(1, winnie);
        assert!(t.event(1, "HeavyMedal", "talk").contains("31229-lvl-2.htm"));
    }
}
