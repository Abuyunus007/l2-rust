//! L2 Day: collect the letters, make LINEAGE II, NCSOFT or CHRONICLE, win a prize.
// Java: dist/game/data/scripts/events/L2Day/L2Day.java
// Java: dist/game/data/scripts/events/L2Day/config.xml

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

// NPCs: the manager and the item he hands out on the 5% roll (Java `MANAGERS`).
const MANAGERS: [(i32, i32); 5] = [
    (31854, 7117), // Talking Island Village
    (31855, 7118), // Elven Village
    (31856, 7119), // Dark Elven Village
    (31857, 7121), // Dwarven Village
    (31858, 7120), // Orc Village
];

// Items
const A: i32 = 3875;
const C: i32 = 3876;
const E: i32 = 3877;
const F: i32 = 3878;
const G: i32 = 3879;
const H: i32 = 3880;
const I: i32 = 3881;
const L: i32 = 3882;
const N: i32 = 3883;
const O: i32 = 3884;
const R: i32 = 3885;
const S: i32 = 3886;
const T: i32 = 3887;
const II: i32 = 3888;

/// Java `ItemChanceHolder(id, chance, count)`.
type Reward = (i32, i32, i64);

// Rewards
const L2_REWARDS: [Reward; 25] = [
    (10260, 90, 3), // Alacrity Juice
    (10261, 85, 3), // Accuracy Juice
    (10262, 80, 3), // Critical Hit Juice
    (10263, 75, 3), // Critical Rate Juice
    (10264, 70, 3), // Casting Spd. Juice
    (10265, 65, 3), // Evasion Juice
    (10266, 60, 3), // M. Atk. Juice
    (10267, 55, 3), // Power Juice
    (10268, 50, 3), // Speed Juice
    (10269, 45, 3), // Defense Juice
    (10270, 40, 3), // MP Consumption Juice
    (9546, 37, 2),  // Fire Stone
    (9547, 34, 2),  // Water Stone
    (9548, 31, 2),  // Earth Stone
    (9549, 28, 2),  // Wind Stone
    (9550, 25, 2),  // Dark Stone
    (9551, 22, 2),  // Holy Stone
    (8947, 19, 1),  // L2day - Rabbit Ears
    (8948, 16, 1),  // L2day - Little Angel Wings
    (8949, 13, 1),  // L2day - Fairy Antennae
    (3959, 10, 2),  // Blessed Scroll of Resurrection (Event)
    (3958, 7, 2),   // Blessed Scroll of Escape (Event)
    (8752, 4, 2),   // High-Grade Life Stone - Level 76
    (8762, 1, 1),   // Top-Grade Life Stone - Level 76
    (6660, 0, 1),   // Ring of Queen Ant
];
const NC_REWARDS: [Reward; 25] = [
    (10260, 90, 2), // Alacrity Juice
    (10261, 85, 2), // Accuracy Juice
    (10262, 80, 2), // Critical Hit Juice
    (10263, 75, 2), // Critical Rate Juice
    (10264, 70, 2), // Casting Spd. Juice
    (10265, 65, 2), // Evasion Juice
    (10266, 60, 2), // M. Atk. Juice
    (10267, 55, 2), // Power Juice
    (10268, 50, 2), // Speed Juice
    (10269, 45, 2), // Defense Juice
    (10270, 40, 2), // MP Consumption Juice
    (9546, 37, 1),  // Fire Stone
    (9547, 34, 1),  // Water Stone
    (9548, 31, 1),  // Earth Stone
    (9549, 28, 1),  // Wind Stone
    (9550, 25, 1),  // Dark Stone
    (9551, 22, 1),  // Holy Stone
    (8948, 19, 1),  // L2day - Little Angel Wings
    (8949, 16, 1),  // L2day - Fairy Antennae
    (8950, 13, 1),  // L2day - Feathered Hat
    (3959, 10, 1),  // Blessed Scroll of Resurrection (Event)
    (3958, 7, 1),   // Blessed Scroll of Escape (Event)
    (8742, 4, 2),   // Mid-Grade Life Stone - Level 76
    (8752, 1, 1),   // High-Grade Life Stone - Level 76
    (6661, 0, 1),   // Earring of Orfen
];
const CH_REWARDS: [Reward; 25] = [
    (10260, 90, 1), // Alacrity Juice
    (10261, 85, 1), // Accuracy Juice
    (10262, 80, 1), // Critical Hit Juice
    (10263, 75, 1), // Critical Rate Juice
    (10264, 70, 1), // Casting Spd. Juice
    (10265, 65, 1), // Evasion Juice
    (10266, 60, 1), // M. Atk. Juice
    (10267, 55, 1), // Power Juice
    (10268, 50, 1), // Speed Juice
    (10269, 45, 1), // Defense Juice
    (10270, 40, 1), // MP Consumption Juice
    (9546, 37, 1),  // Fire Stone
    (9547, 34, 1),  // Water Stone
    (9548, 31, 1),  // Earth Stone
    (9549, 28, 1),  // Wind Stone
    (9550, 25, 1),  // Dark Stone
    (9551, 22, 1),  // Holy Stone
    (8949, 19, 1),  // L2day - Fairy Antennae
    (8950, 16, 1),  // L2day - Feathered Hat
    (8951, 13, 1),  // L2day - Artisan's Goggles
    (3959, 10, 1),  // Blessed Scroll of Resurrection (Event)
    (3958, 7, 1),   // Blessed Scroll of Escape (Event)
    (8742, 4, 1),   // Mid-Grade Life Stone - Level 76
    (8752, 1, 1),   // High-Grade Life Stone - Level 76
    (6662, 0, 1),   // Ring of Core
];

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "L2 Day",
    active: "23 03 2006-28 03 2016",
    drop_period: None,
    drops: &[
        DropSpec {
            item: 3875,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3876,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3877,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3878,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3879,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3880,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3881,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3882,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3883,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3884,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3885,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3886,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3887,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
        DropSpec {
            item: 3888,
            min: 1,
            max: 1,
            chance: 1.0,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        },
    ],
    spawns: &[
        SpawnSpec::at(31854, -84128, 243258, -3735, 9707, 0),
        SpawnSpec::at(31855, 45510, 48364, -3065, 49859, 0),
        SpawnSpec::at(31856, 11861, 16173, -4568, 18289, 0),
        SpawnSpec::at(31857, 115639, -178066, -921, 27485, 0),
        SpawnSpec::at(31858, -44810, -113388, -192, 15527, 0),
    ],
    on_enter_msg: "L2 Day Event: Collect letters from monsters. Create the words NCSOFT, CHRONICLE and LINEAGE II. Turn these letters to the Event Manager in the villages and win great prizes!",
    on_end_msg: "L2 Day: Event end!",
    destroy_items_on_end: &[],
};

/// Java `MANAGERS.get(npcId)`.
fn manager_item(npc_id: i32) -> Option<i32> {
    MANAGERS
        .iter()
        .find(|(id, _)| *id == npc_id)
        .map(|(_, item)| *item)
}

/// Java `for (ItemChanceHolder holder : rewards) if (random >= holder.getChance()) { giveItems(player,
/// holder); break; }`.
fn give_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, rewards: &[Reward], random: i32) {
    for &(item_id, chance, count) in rewards {
        if random >= chance {
            ctx.give_items(player, item_id, count);
            break;
        }
    }
}

/// Java `events.L2Day.L2Day`.
pub struct L2Day;

impl Script for L2Day {
    fn name(&self) -> &'static str {
        "L2Day"
    }

    fn path(&self) -> &'static str {
        "events/L2Day"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let managers: Vec<i32> = MANAGERS.iter().map(|(id, _)| *id).collect();
        s.add_start_npc(&managers);
        s.add_first_talk_id(&managers);
        s.add_talk_id(&managers);
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
            "collect_l2" => {
                if ctx.has_all_quest_items(player, &[L, I, N, E, A, G, II])
                    && ctx.get_quest_items_count(player, E) > 1
                {
                    ctx.take_items_many(player, 1, &[L, I, N, E, A, G, E, II]);
                    let random = ctx.get_random(100);
                    if random >= 95 {
                        ctx.give_items(player, manager_item(npc?.id)?, 2);
                    } else {
                        give_reward(ctx, player, &L2_REWARDS, random);
                    }

                    htmltext = Some("manager-1.htm".to_owned());
                } else {
                    htmltext = Some("manager-no.htm".to_owned());
                }
            }
            "collect_nc" => {
                if ctx.has_all_quest_items(player, &[N, C, S, O, F, T]) {
                    ctx.take_items_many(player, 1, &[N, C, S, O, F, T]);
                    let random = ctx.get_random(100);
                    if random >= 95 {
                        ctx.give_items(player, manager_item(npc?.id)?, 1);
                    } else {
                        give_reward(ctx, player, &NC_REWARDS, random);
                    }

                    htmltext = Some("manager-1.htm".to_owned());
                } else {
                    htmltext = Some("manager-no.htm".to_owned());
                }
            }
            "collect_ch" => {
                if ctx.has_all_quest_items(player, &[C, H, R, O, N, I, L, E])
                    && ctx.get_quest_items_count(player, C) > 1
                {
                    ctx.take_items_many(player, 1, &[C, H, R, O, N, I, C, L, E]);
                    let random = ctx.get_random(100);
                    if random >= 95 {
                        ctx.give_items(player, manager_item(npc?.id)?, 1);
                    } else {
                        give_reward(ctx, player, &CH_REWARDS, random);
                    }

                    htmltext = Some("manager-1.htm".to_owned());
                } else {
                    htmltext = Some("manager-no.htm".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("manager-1.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="31854" type="Folk" level="70" name="Event Manager"/>
        <npc id="31855" type="Folk" level="70" name="Event Manager"/>
        <npc id="31856" type="Folk" level="70" name="Event Manager"/>
        <npc id="31857" type="Folk" level="70" name="Event Manager"/>
        <npc id="31858" type="Folk" level="70" name="Event Manager"/>"#;
    const MANAGER: i32 = 31854;
    /// 2010-06-01 12:00 UTC, inside the event (23 03 2006 to 28 03 2016).
    const INSIDE: i64 = 1_275_393_600_000;

    /// An item template for every id the script gives or takes.
    fn items_xml() -> String {
        let mut ids: Vec<i32> = vec![
            3875, 3876, 3877, 3878, 3879, 3880, 3881, 3882, 3883, 3884, 3885, 3886, 3887, 3888,
        ];
        ids.extend([7117, 7118, 7119, 7120, 7121]);
        for table in [&L2_REWARDS, &NC_REWARDS, &CH_REWARDS] {
            ids.extend(table.iter().map(|r| r.0));
        }
        ids.sort_unstable();
        ids.dedup();
        ids.iter()
            .map(|id| format!(r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_stackable" val="true"/></item>"#))
            .collect()
    }

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(L2Day);
        let pages = Pages::of(&L2Day)
            .page("manager-1.htm", &["collect_l2", "collect_nc", "collect_ch"])
            .plain(&["manager-2.htm", "manager-no.htm"])
            .npcs(&[MANAGER]);
        ScriptTest::with_pages(registry, NPCS, &items_xml(), pages)
    }

    fn letters(ids: &[i32]) -> Vec<l2_world::ItemData> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| item(1000 + i as i32, *id, 1))
            .collect()
    }

    #[test]
    fn inside_the_dates_the_letters_drop_and_the_managers_stand() {
        let t = setup(Some(INSIDE));
        let drops = t.world.event_drops_of("L2Day").unwrap();
        assert_eq!(drops.len(), 14);
        assert!(drops.iter().all(|d| d.chance == 1.0 && d.min == 1 && d.max == 1));
        for manager in [31854, 31855, 31856, 31857, 31858] {
            assert_eq!(t.npcs_of(manager).len(), 1);
        }
    }

    #[test]
    fn outside_the_dates_there_are_no_drops_and_no_managers() {
        // The event ended in 2016.
        let t = setup(None);
        assert!(t.world.event_drops_of("L2Day").is_none());
        assert!(t.npcs_of(MANAGER).is_empty());
    }

    #[test]
    fn the_end_of_the_event_takes_the_drops_back() {
        let mut t = setup(Some(INSIDE));
        assert!(t.world.event_drops_of("L2Day").is_some());
        // Java's `ScheduleEnd` runs `stopEvent`.
        t.world.run_as_script("L2Day", |ctx| {
            assert!(long_time_event::on_event(
                ctx,
                &CONFIG,
                "LongTimeEvent_ScheduleEnd",
                None
            ));
            assert!(!long_time_event::is_event_period(ctx));
        });
        assert!(t.world.event_drops_of("L2Day").is_none());
    }

    #[test]
    fn a_player_without_the_letters_gets_the_no_page() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, letters(&[N, C, S]));
        let manager = t.spawn(MANAGER, 20);
        t.talk(1, manager);
        assert!(t.event(1, "L2Day", "collect_nc").contains("manager-no.htm"));
        assert_eq!(t.count(1, N), 1);
    }

    #[test]
    fn the_nc_letters_are_swapped_for_a_prize() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, letters(&[N, C, S, O, F, T]));
        let manager = t.spawn(MANAGER, 20);
        t.talk(1, manager);
        assert!(t.event(1, "L2Day", "collect_nc").contains("manager-1.htm"));
        for letter in [N, C, S, O, F, T] {
            assert_eq!(t.count(1, letter), 0);
        }
        // Seeded roll: some prize (a juice, a stone, ..., or the manager's item) arrived.
        let prizes: i64 = NC_REWARDS
            .iter()
            .map(|r| r.0)
            .chain([7117])
            .map(|id| t.count(1, id))
            .sum();
        assert!(prizes >= 1);
    }

    #[test]
    fn lineage_needs_two_e() {
        let mut t = setup(Some(INSIDE));
        // L I N E A G E II with a single E: the second E is missing.
        t.enter(1, 40, letters(&[L, I, N, E, A, G, II]));
        let manager = t.spawn(MANAGER, 20);
        t.talk(1, manager);
        assert!(t.event(1, "L2Day", "collect_l2").contains("manager-no.htm"));
        assert_eq!(t.count(1, L), 1);
    }

    #[test]
    fn chronicle_takes_the_two_c_and_the_rest() {
        let mut t = setup(Some(INSIDE));
        let mut items = letters(&[H, R, O, N, I, L, E]);
        items.push(item(2000, C, 2));
        t.enter(1, 40, items);
        let manager = t.spawn(MANAGER, 20);
        t.talk(1, manager);
        assert!(t.event(1, "L2Day", "collect_ch").contains("manager-1.htm"));
        assert_eq!(t.count(1, C), 0);
        assert_eq!(t.count(1, H), 0);
    }

    #[test]
    fn the_reward_tables_end_with_a_zero_chance_item() {
        // A roll of 0 to 94 always finds an entry: the last one has chance 0.
        for table in [&L2_REWARDS, &NC_REWARDS, &CH_REWARDS] {
            assert_eq!(table.last().unwrap().1, 0);
        }
        assert_eq!(manager_item(31857), Some(7121));
        assert_eq!(manager_item(1), None);
    }
}
