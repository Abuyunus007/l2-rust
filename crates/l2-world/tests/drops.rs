#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Drops, items on the ground and pickup (M4-DROP-01).
//!
//! The first half checks `NpcTemplate.calculateDrops` with a scripted random source: the order
//! and the bounds of the draws, the `float`/`double` quirks and the occurrence counter. The
//! second half runs whole scenes (a monster dies, the loot lies there, somebody picks it up) on
//! the real world with small synthetic templates, and checks the packets and their order.
//! Java: entity/actor/templates/NpcTemplate.java (calculateDrops), entity/actor/Attackable.java
//! (doItemDrop), entity/actor/Npc.java (dropItem), entity/item/instance/Item.java (dropMe,
//! pickupMe), entity/actor/Player.java (doPickupItem, addItem, addAdena),
//! taskmanagers/ItemsAutoDestroyTaskManager.java, managers/ItemsOnGroundManager.java.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use l2_data::npc::DropType;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{DeleteObject, DropItem, GetItem, InventoryUpdate, SpawnItem, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::formulas::Rnd;
use l2_world::rewards::drop::{DropKiller, DropVictim, calculate_drops};
use l2_world::*;

// ------------------------------------------------------------------ the scripted random source

/// A random source that hands out the numbers a test wrote down and records every draw, so the
/// test can check the order and the bounds like the Java vectors do.
#[derive(Default)]
struct Scripted {
    doubles: VecDeque<f64>,
    longs: VecDeque<i64>,
    log: Vec<String>,
}

impl Scripted {
    fn new(doubles: &[f64], longs: &[i64]) -> Self {
        Self {
            doubles: doubles.iter().copied().collect(),
            longs: longs.iter().copied().collect(),
            log: Vec::new(),
        }
    }

    /// Everything written down was used.
    fn finished(&self) -> bool {
        self.doubles.is_empty() && self.longs.is_empty()
    }
}

impl Rnd for Scripted {
    fn next_double(&mut self) -> f64 {
        self.log.push("double".into());
        self.doubles.pop_front().expect("the script ran out of doubles")
    }

    fn get_long_range(&mut self, origin: i64, bound: i64) -> i64 {
        // `Rnd.get(long, long)` draws nothing when the range is a single number.
        if origin >= bound {
            return origin;
        }
        self.log.push(format!("long {origin}..={bound}"));
        let value = self.longs.pop_front().expect("the script ran out of longs");
        assert!(
            (origin..=bound).contains(&value),
            "{value} is outside {origin}..={bound}"
        );
        value
    }

    fn next_boolean(&mut self) -> bool {
        unreachable!("drops draw doubles and longs only")
    }
    fn next_bytes(&mut self, _bytes: &mut [u8]) {
        unreachable!()
    }
    fn get_int(&mut self, _bound: i32) -> i32 {
        unreachable!()
    }
    fn get_int_range(&mut self, _origin: i32, _bound: i32) -> i32 {
        unreachable!()
    }
    fn next_int(&mut self) -> i32 {
        unreachable!()
    }
    fn get_long(&mut self, _bound: i64) -> i64 {
        unreachable!()
    }
    fn next_long(&mut self) -> i64 {
        unreachable!()
    }
    fn next_float(&mut self) -> f32 {
        unreachable!()
    }
    fn get_double(&mut self, _bound: f64) -> f64 {
        unreachable!()
    }
    fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
        unreachable!()
    }
    fn next_gaussian(&mut self) -> f64 {
        unreachable!()
    }
}

// ------------------------------------------------------------------ items and NPC templates

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="5" /></item>
<item id="2" type="Weapon" name="Bow"><set name="weapon_type" val="BOW" /><set name="bodypart" val="lrhand" /><set name="weight" val="5" /></item>
<item id="3" type="Armor" name="Plate"><set name="bodypart" val="chest" /><set name="weight" val="5" /></item>
<item id="17" type="EtcItem" name="Arrow"><set name="etcitem_type" val="ARROW" /><set name="bodypart" val="lhand" /><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1060" type="EtcItem" name="Potion"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="8600" type="EtcItem" name="Herb"><set name="is_stackable" val="true" /><set name="ex_immediate_effect" val="true" /></item>
<item id="6361" type="EtcItem" name="Seal Stone"><set name="is_stackable" val="true" /></item>
<item id="1000" type="EtcItem" name="Dagger Bit"><set name="weight" val="1" /></item>
<item id="1001" type="EtcItem" name="Bit B"><set name="weight" val="1" /></item>
<item id="1002" type="EtcItem" name="Bit C"><set name="weight" val="1" /></item>
<item id="1003" type="EtcItem" name="Bit D"><set name="weight" val="1" /></item>
<item id="1500" type="EtcItem" name="Fast Auto Destroy"><set name="weight" val="1" /><set name="auto_destroy_time" val="7" /></item>
</list>"#;

fn items() -> ItemTemplates {
    ItemTemplates::from_xml(ITEMS).unwrap()
}

/// One monster template per scenario, built from the drop list that scenario needs.
fn monster(drop_lists: &str) -> NpcTemplates {
    NpcTemplates::from_xml(
        &format!(r#"<list><npc id="1" type="Monster" level="10" name="M">{drop_lists}</npc></list>"#),
        &NpcLoadOptions::default(),
    )
    .unwrap()
}

fn raid_boss(drop_lists: &str) -> NpcTemplates {
    NpcTemplates::from_xml(
        &format!(r#"<list><npc id="1" type="RaidBoss" level="50" name="B">{drop_lists}</npc></list>"#),
        &NpcLoadOptions::default(),
    )
    .unwrap()
}

const VICTIM: DropVictim = DropVictim {
    level: 10,
    raid: false,
    champion: false,
    seeded: false,
};

/// The default rates with `f` applied.
fn rates(f: impl FnOnce(&mut DropSettings)) -> DropSettings {
    let mut s = DropSettings::default();
    f(&mut s);
    s
}

fn roll(
    settings: &DropSettings,
    npcs: &NpcTemplates,
    drop_type: DropType,
    victim: &DropVictim,
    killer: &DropKiller,
    rng: &mut Scripted,
) -> Vec<(i32, i64)> {
    calculate_drops(
        settings,
        &items(),
        npcs.get(1).unwrap(),
        drop_type,
        victim,
        killer,
        rng,
    )
    .into_iter()
    .map(|h| (h.id, h.count))
    .collect()
}

// ------------------------------------------------------------------ calculateDrops

/// A group item rolls its level gap first, its chance second and its count third, and a group
/// that rolled an item rolls no other (x1 rates). The chance is `100 * (70 / 100)`.
#[test]
fn a_group_item_draws_gap_then_chance_then_count() {
    let npcs = monster(
        r#"<dropLists><drop><group chance="70"><item id="57" min="30" max="42" chance="100"/></group></drop></dropLists>"#,
    );
    let settings = DropSettings::default();
    // Equal levels: the adena level gap chance is 100, so the first draw never rejects.
    let mut rng = Scripted::new(&[0.5, 0.69], &[35]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(57, 35)]);
    assert_eq!(rng.log, ["double", "double", "long 30..=42"]);
    assert!(rng.finished());

    // 71 is not below 70: nothing, and the count is not drawn.
    let mut rng = Scripted::new(&[0.5, 0.71], &[]);
    assert!(
        roll(
            &settings,
            &npcs,
            DropType::Drop,
            &VICTIM,
            &DropKiller::plain(10),
            &mut rng
        )
        .is_empty()
    );
    assert_eq!(rng.log, ["double", "double"]);
}

/// Java keeps `Rates.ini` multipliers as `float`: 0.1 meets the chance as 0.10000000149011612, so a
/// draw of 7.00000005 passes `100 * 0.7 * 0.1f` although it fails `100 * 0.7 * 0.1`.
#[test]
fn rates_are_floats_widened_to_doubles() {
    let npcs = monster(
        r#"<dropLists><drop><group chance="70"><item id="57" min="1" max="1" chance="100"/></group></drop></dropLists>"#,
    );
    let settings = rates(|s| {
        s.rate_drop_chance_by_id = BTreeMap::from([(57, 0.1_f32)]);
    });
    let mut rng = Scripted::new(&[0.5, 0.070_000_000_5], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(57, 1)]);
    // The exact `double` rate would not have let it through.
    let exact = 100.0_f64 * (70.0 / 100.0) * 0.1_f64;
    assert!(0.070_000_000_5 * 100.0 > exact);
}

/// The amount is `(long) (count * rateAmount)`: truncated, with the by-item-id rate winning over
/// the monster rate, and the adena bonus stat applied on top.
#[test]
fn the_amount_is_truncated_after_the_rate() {
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="57" min="10" max="10" chance="100"/>
            <item id="1060" min="3" max="3" chance="100"/>
        </drop></dropLists>"#,
    );
    let settings = rates(|s| {
        s.rate_death_drop_amount_multiplier = 1.5;
        s.rate_drop_amount_by_id = BTreeMap::from([(57, 2.5_f32)]);
    });
    let mut killer = DropKiller::plain(10);
    killer.bonus_drop_adena = 1.1;
    // Both items are certain (chance 100 against a draw below 1).
    let mut rng = Scripted::new(&[0.5, 0.5, 0.5, 0.5], &[]);
    let drops = roll(&settings, &npcs, DropType::Drop, &VICTIM, &killer, &mut rng);
    // 10 * (2.5 * 1.1) = 27.500000000000004 -> 27; 3 * 1.5 = 4.5 -> 4.
    assert_eq!(drops, [(57, 27), (1060, 4)]);
    // The list is sorted by chance and the stable sort keeps the file order of equal chances.
}

/// A monster far above the player drops everything; one far below only rarely (the level gap
/// chance), and adena has its own gap.
#[test]
fn the_level_gap_can_reject_a_drop() {
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="57" min="1" max="1" chance="100"/>
            <item id="1060" min="1" max="1" chance="100"/>
        </drop></dropLists>"#,
    );
    let settings = DropSettings::default();
    // A level 10 monster against a level 50 player: -40 clamps to the end of both ranges, so the
    // gap chances are the 10 percent minimums. 0.5 * 100 > 10: both rejected, no chance draw.
    let mut rng = Scripted::new(&[0.5, 0.5], &[]);
    assert!(
        roll(
            &settings,
            &npcs,
            DropType::Drop,
            &VICTIM,
            &DropKiller::plain(50),
            &mut rng
        )
        .is_empty()
    );
    assert_eq!(rng.log, ["double", "double"]);
    // A draw of 0.05 is inside the 10 percent: the items drop.
    let mut rng = Scripted::new(&[0.05, 0.5, 0.05, 0.5], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(50),
        &mut rng,
    );
    assert_eq!(drops, [(57, 1), (1060, 1)]);

    // Half way inside the range: adena -11.5 of [-15, -8]: gap = (3.5 * 90) / 7 + 10 = 55.
    // The monster is 11.5 levels below... use whole levels: monster 10 vs player 21 -> -11.
    // adena: ((-11 + 15) * 90) / 7 + 10 = 61.43; item: ((-11 + 10) clamps to -10 -> 10).
    let mut rng = Scripted::new(&[0.6, 0.5, 0.2], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(21),
        &mut rng,
    );
    // Adena: 60 <= 61.43 passes, then the chance draw 0.5 passes; the potion: 20 > 10 rejected.
    assert_eq!(drops, [(57, 1)]);
}

/// At most `DropMaxOccurrencesNormal` (2) items under 100 percent drop; a third one replaces the
/// one with the highest chance.
#[test]
fn the_occurrence_counter_limits_the_drops_and_the_replaced_item_comes_back_when_nothing_replaces_it() {
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="1000" min="1" max="1" chance="50"/>
            <item id="1001" min="1" max="1" chance="40"/>
            <item id="1002" min="1" max="1" chance="30"/>
        </drop></dropLists>"#,
    );
    let settings = DropSettings::default();
    // All three succeed: 1000 and 1001 fill the counter, 1002 first removes 1000 (the highest
    // chance of the random drops) and then takes its place.
    let mut rng = Scripted::new(&[0.5, 0.1, 0.5, 0.1, 0.5, 0.1], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1001, 1), (1002, 1)]);

    // The third fails its chance: the removed item is put back at the end.
    let mut rng = Scripted::new(&[0.5, 0.1, 0.5, 0.1, 0.5, 0.9], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1001, 1), (1000, 1)]);

    // A 100 percent item is not an occurrence: it drops next to two others.
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="1000" min="1" max="1" chance="100"/>
            <item id="1001" min="1" max="1" chance="40"/>
            <item id="1002" min="1" max="1" chance="30"/>
        </drop></dropLists>"#,
    );
    let mut rng = Scripted::new(&[0.5, 0.1, 0.5, 0.1, 0.5, 0.1], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1000, 1), (1001, 1), (1002, 1)]);
}

/// A raid boss allows seven occurrences and uses the raid rates.
#[test]
fn a_raid_boss_uses_the_raid_rates_and_seven_occurrences() {
    let npcs =
        raid_boss(r#"<dropLists><drop><item id="1060" min="2" max="2" chance="50"/></drop></dropLists>"#);
    let settings = rates(|s| {
        s.rate_raid_drop_chance_multiplier = 2.0;
        s.rate_raid_drop_amount_multiplier = 3.0;
        s.rate_death_drop_amount_multiplier = 100.0;
    });
    let victim = DropVictim {
        level: 50,
        raid: true,
        ..VICTIM
    };
    // 50 * 2 = 100 percent: even a draw of 0.99 passes. Amount 2 * 3.
    let mut rng = Scripted::new(&[0.5, 0.99], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &victim,
        &DropKiller::plain(50),
        &mut rng,
    );
    assert_eq!(drops, [(1060, 6)]);
}

/// Spoil drops use their own rates and the bonus spoil stat; groups never apply.
#[test]
fn a_spoil_list_uses_the_spoil_rates() {
    let npcs =
        monster(r#"<dropLists><spoil><item id="1060" min="4" max="6" chance="20"/></spoil></dropLists>"#);
    let settings = rates(|s| {
        s.rate_spoil_drop_chance_multiplier = 2.0;
        s.rate_spoil_drop_amount_multiplier = 1.5;
    });
    let mut killer = DropKiller::plain(10);
    killer.bonus_spoil_rate = 1.25;
    // 20 * (2 * 1.25) = 50.
    let mut rng = Scripted::new(&[0.5, 0.49], &[5]);
    let drops = roll(&settings, &npcs, DropType::Spoil, &VICTIM, &killer, &mut rng);
    assert_eq!(drops, [(1060, 7)]);
    // The death list of such a monster is empty and draws nothing.
    let mut rng = Scripted::new(&[], &[]);
    assert!(
        roll(
            &settings,
            &npcs,
            DropType::Drop,
            &VICTIM,
            &DropKiller::plain(10),
            &mut rng
        )
        .is_empty()
    );
    assert!(rng.log.is_empty());
}

/// `NpcData` drops entries for items that do not exist; the roll does not draw for them.
#[test]
fn unknown_items_are_not_rolled() {
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="99999" min="1" max="1" chance="100"/>
            <item id="1060" min="1" max="1" chance="100"/>
        </drop></dropLists>"#,
    );
    let mut rng = Scripted::new(&[0.5, 0.5], &[]);
    let drops = roll(
        &DropSettings::default(),
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1060, 1)]);
    assert!(rng.finished());
}

/// With custom rates a group item rolls on its own chance, not on the running total of the group,
/// and every item of the group can drop.
#[test]
fn custom_rates_break_the_group_logic() {
    let npcs = monster(
        r#"<dropLists><drop><group chance="100">
            <item id="1000" min="1" max="1" chance="40"/>
            <item id="1001" min="1" max="1" chance="60"/>
        </group></drop></dropLists>"#,
    );
    // x1: the cumulative chance 40, then 100, and the first success ends the group.
    let mut rng = Scripted::new(&[0.5, 0.39], &[]);
    let drops = roll(
        &DropSettings::default(),
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1000, 1)]);
    let mut rng = Scripted::new(&[0.5, 0.41, 0.5, 0.99], &[]);
    let drops = roll(
        &DropSettings::default(),
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1001, 1)]);

    // Rate 2: each item has chance * 2, both can drop, and neither counts as an occurrence.
    let settings = rates(|s| {
        s.rate_death_drop_chance_multiplier = 2.0;
    });
    let mut rng = Scripted::new(&[0.5, 0.79, 0.5, 0.99], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &VICTIM,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(1000, 1), (1001, 1)]);
}

/// A seeded monster only drops adena and the seal stones.
#[test]
fn a_seeded_monster_keeps_adena_and_seal_stones() {
    let npcs = monster(
        r#"<dropLists><drop>
            <item id="57" min="1" max="1" chance="100"/>
            <item id="6361" min="1" max="1" chance="100"/>
            <item id="1060" min="1" max="1" chance="100"/>
        </drop></dropLists>"#,
    );
    let victim = DropVictim {
        seeded: true,
        ..VICTIM
    };
    let mut rng = Scripted::new(&[0.5; 6], &[]);
    let drops = roll(
        &DropSettings::default(),
        &npcs,
        DropType::Drop,
        &victim,
        &DropKiller::plain(10),
        &mut rng,
    );
    assert_eq!(drops, [(57, 1), (6361, 1)]);
}

/// `BossDropEnable` adds the configured items to raid bosses inside the level range, sorted in
/// with the others.
#[test]
fn the_boss_drop_list_joins_raid_bosses_in_range() {
    let npcs =
        raid_boss(r#"<dropLists><drop><item id="1060" min="1" max="1" chance="10"/></drop></dropLists>"#);
    let settings = rates(|s| {
        s.boss_drop_enabled = true;
        s.boss_drop_list = vec![l2_data::npc::DropEntry {
            drop_type: DropType::Drop,
            item_id: 1001,
            min: 2,
            max: 2,
            chance: 100.0,
        }];
    });
    let victim = DropVictim {
        level: 50,
        raid: true,
        ..VICTIM
    };
    // The boss item has the higher chance, so it rolls first.
    let mut rng = Scripted::new(&[0.5, 0.5, 0.5, 0.5], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &victim,
        &DropKiller::plain(50),
        &mut rng,
    );
    assert_eq!(drops, [(1001, 2)]);
    // A boss outside the level range gets nothing extra.
    let settings = DropSettings {
        boss_drop_min_level: 60,
        ..settings
    };
    let mut rng = Scripted::new(&[0.5, 0.5], &[]);
    let drops = roll(
        &settings,
        &npcs,
        DropType::Drop,
        &victim,
        &DropKiller::plain(50),
        &mut rng,
    );
    assert!(drops.is_empty(), "{drops:?}");
}

// ====================================================================================================
// Scenes on the real world
// ====================================================================================================

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

/// What reached the persistence task, in order.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }

    /// The `SaveGroundItems` commands.
    fn ground_saves(&self) -> Vec<Vec<GroundItemRow>> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                PersistCommand::SaveGroundItems(rows) => Some(rows),
                _ => None,
            })
            .collect()
    }

    /// `(upserted object ids, removed object ids)` of every `ItemChanges`.
    fn item_changes(&self) -> Vec<(Vec<i32>, Vec<i32>)> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                PersistCommand::ItemChanges { upserts, removed, .. } => Some((
                    upserts.iter().map(|i| i.object_id.0).collect(),
                    removed.iter().map(|i| i.0).collect(),
                )),
                _ => None,
            })
            .collect()
    }
}

struct RecOutbox(Persisted);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn of(sent: &Sent, to: i32, opcode: ServerOpcode) -> Vec<&[u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == opcode.name())
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The ids of the system messages one client got.
fn messages(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    of(sent, to, ServerOpcode::SystemMessage)
        .into_iter()
        .map(|b| decode::<SystemMessage>(b).id)
        .collect()
}

fn has_in_order(names: &[String], want: &[&str]) -> bool {
    let mut it = names.iter();
    want.iter().all(|w| it.any(|n| n == w))
}

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

/// The monster of the scenes: 300 HP, certain drops (an adena group and two single items).
const LOOT_MONSTER: &str = r#"<npc id="9001" type="Monster" level="1" name="Loot Monster">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <dropLists><drop>
      <group chance="100"><item id="57" min="50" max="50" chance="100"/></group>
      <item id="1" min="1" max="1" chance="100"/>
      <item id="1060" min="3" max="3" chance="100"/>
    </drop></dropLists>
  </npc>"#;

/// A raid boss that drops potions.
const RAID: &str = r#"<npc id="9002" type="RaidBoss" level="1" name="Big Boss">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <dropLists><drop><item id="1060" min="2" max="2" chance="100"/></drop></dropLists>
  </npc>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;
const OTHER: i32 = 0x2000_0002;
const THIRD: i32 = 0x2000_0003;
/// The object ids of the items the players start with.
const MY_SWORD: i32 = 0x2000_0101;
const MY_POTIONS: i32 = 0x2000_0102;

fn player_data(id: i32, name: &str, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y,
        z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn item(object_id: i32, item_id: i32, loc: &str, slot: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: loc.into(),
        loc_data: slot,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    ids: Arc<IdFactory>,
}

impl Scene {
    fn new(seed: u64) -> Self {
        Self::with_settings(seed, WorldSettings::default())
    }

    fn with_settings(seed: u64, settings: WorldSettings) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(seed), Box::new(RecOutbox(persisted.clone())));
        world.set_settings(settings);
        let stats = StatData {
            items: Arc::new(items()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::default()),
        };
        world.set_static_data(Box::new(Data(stats)));
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &format!("<list>{LOOT_MONSTER}{RAID}</list>"),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        world.set_npc_data(npcs, Arc::new(SpawnData::default()));
        Self {
            world,
            log,
            persisted,
            ids,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.enter_data(player_data(id, name, pos, items));
    }

    fn enter_data(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn click(&mut self, id: i32, target: ObjectId) {
        self.input(
            id,
            ClientInput::Action {
                object_id: target.0,
                action_id: 0,
            },
        );
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// Ticks until `done`, collecting the packets; panics after `limit` ticks.
    fn run_until(&mut self, limit: u32, mut done: impl FnMut(&World) -> bool) -> Sent {
        let mut sent = Vec::new();
        for _ in 0..limit {
            self.world.tick();
            sent.extend(self.log.take());
            if done(&self.world) {
                return sent;
            }
        }
        panic!("not done after {limit} ticks");
    }

    /// `who` clicks the ground item and walks to it until it is gone; the packets of the walk and
    /// of the 300 ms after it (the delayed `InventoryUpdate`).
    fn pick_up(&mut self, who: i32, item: ObjectId) -> Sent {
        self.click(who, item);
        let mut sent = self.run_until(400, |w| w.ground_item(item).is_none());
        self.ticks(6);
        sent.extend(self.log.take());
        sent
    }

    fn spawn_monster(&mut self, npc_id: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X, Y, Z, 0).unwrap()
    }

    /// `killer` hits the monster for `damage`.
    fn hit(&mut self, monster: ObjectId, killer: i32, damage: f64) {
        self.world
            .reduce_current_hp(monster, damage, Some(ObjectId(killer)));
    }

    /// The ground item with this template id.
    fn ground_of(&self, item_id: i32) -> ObjectId {
        self.world
            .ground_item_ids()
            .into_iter()
            .find(|id| self.world.ground_item(*id).unwrap().data.item_id == item_id)
            .unwrap_or_else(|| panic!("no item {item_id} on the ground"))
    }

    fn inventory(&self, who: i32) -> Vec<(i32, i64, String)> {
        self.world
            .player(ObjectId(who))
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.item_id, i.count, i.loc.clone()))
            .collect()
    }
}

fn settings(f: impl FnOnce(&mut WorldSettings)) -> WorldSettings {
    let mut s = WorldSettings::default();
    f(&mut s);
    s
}

// ------------------------------------------------------------------ the monster dies

/// Everybody who sees the monster gets `DropItem` for each drop before the `Die` packet (the
/// rewards are calculated before the AI is told); the loot lies within 70 of the monster, 20 above
/// it, and is owned and protected for the killer.
#[test]
fn a_monster_drops_its_loot_around_itself_for_everybody_who_sees_it() {
    let mut s = Scene::new(5);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.enter(OTHER, "Friend", (X + 30, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.log.take();

    s.hit(monster, ME, 1_000_000.0);
    let sent = s.log.take();

    let ground = s.world.ground_item_ids();
    assert_eq!(ground.len(), 3);
    let by_item = |id: i32| s.world.ground_item(s.ground_of(id)).unwrap().clone();
    assert_eq!(by_item(57).data.count, 50);
    assert_eq!(by_item(1).data.count, 1);
    assert_eq!(by_item(1060).data.count, 3);
    for id in &ground {
        let item = s.world.ground_item(*id).unwrap();
        assert!(
            (X - 70..=X + 70).contains(&item.x) && (Y - 70..=Y + 70).contains(&item.y),
            "{item:?}"
        );
        assert_eq!(item.z, Z + 20);
        assert_eq!(
            item.owner,
            Some(ObjectId(ME)),
            "the killer has the loot privilege"
        );
        assert_eq!(item.drop_protection.map(|p| p.owner), Some(ObjectId(ME)));
        assert!(!item.protected);
        assert_eq!(item.data.loc, "VOID");
        assert!(
            s.world.is_in_auto_destroy(*id),
            "AutoDestroyDroppedItemAfter = 600"
        );
        // The object ids are the Rust range's (D-39).
        assert!((RUST_FIRST_OBJECT_ID..=RUST_LAST_OBJECT_ID).contains(&id.0));
    }

    for who in [ME, OTHER] {
        let drops = of(&sent, who, ServerOpcode::DropItem);
        assert_eq!(drops.len(), 3, "player {who:x}");
        for raw in drops {
            let packet: DropItem = decode(raw);
            let item = s.world.ground_item(ObjectId(packet.object_id)).unwrap();
            assert_eq!(packet.player_id, monster.0, "the monster is the dropper");
            assert_eq!(
                (packet.x, packet.y, packet.z, packet.count),
                (item.x, item.y, item.z, item.data.count)
            );
            assert_eq!(packet.stackable, item.data.item_id != 1);
        }
        assert!(
            has_in_order(
                &names(&sent, who),
                &[ServerOpcode::DropItem.name(), ServerOpcode::Die.name()]
            ),
            "the drop comes before the Die packet"
        );
    }
}

/// Somebody who comes into view later gets `SpawnItem`, not `DropItem`.
#[test]
fn a_player_who_arrives_later_gets_spawn_item() {
    let mut s = Scene::new(6);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();

    s.enter(THIRD, "Late", (X + 10, Y, Z), vec![]);
    let sent = s.log.take();
    let spawns = of(&sent, THIRD, ServerOpcode::SpawnItem);
    assert_eq!(spawns.len(), 3);
    assert!(of(&sent, THIRD, ServerOpcode::DropItem).is_empty());
    for raw in spawns {
        let packet: SpawnItem = decode(raw);
        let item = s.world.ground_item(ObjectId(packet.object_id)).unwrap();
        assert_eq!((packet.x, packet.y, packet.z), (item.x, item.y, item.z));
        assert_eq!(packet.count, item.data.count);
    }
}

/// A monster nobody damaged (an empty aggro list) drops nothing: Java returns before `doItemDrop`.
#[test]
fn a_monster_without_an_aggro_list_drops_nothing() {
    let mut s = Scene::new(7);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.world.do_die(monster, Some(ObjectId(ME)));
    assert!(s.world.is_dead(monster));
    assert!(s.world.ground_item_ids().is_empty());
}

/// The loot goes to the player who did the most damage, not to the one who struck last; a dealer
/// farther than `AltPartyRange` does not count.
#[test]
fn the_loot_belongs_to_the_biggest_damage_dealer_within_range() {
    let mut s = Scene::new(8);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.enter(OTHER, "Friend", (X + 20, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    // The monster has 300 HP: OTHER does 200, ME the last 100.
    s.hit(monster, OTHER, 200.0);
    s.hit(monster, ME, 100.0);
    assert!(s.world.is_dead(monster));
    let item = s.world.ground_item(s.ground_of(57)).unwrap();
    assert_eq!(
        item.owner,
        Some(ObjectId(OTHER)),
        "not the killer: the one with the most damage"
    );

    // Now ME stands too far away: only OTHER counts, although ME struck the killing blow.
    let mut s = Scene::with_settings(8, settings(|w| w.drops.alt_party_range = 100));
    s.enter(ME, "Hero", (X + 500, Y, Z), vec![]);
    s.enter(OTHER, "Friend", (X + 20, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, OTHER, 200.0);
    s.hit(monster, ME, 1_000_000.0);
    let item = s.world.ground_item(s.ground_of(57)).unwrap();
    assert_eq!(item.owner, Some(ObjectId(OTHER)));
}

/// A raid boss tells everybody around what it dropped, and nobody owns its loot without a command
/// channel.
#[test]
fn a_raid_boss_announces_its_drop() {
    let mut s = Scene::new(9);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let boss = s.spawn_monster(9002);
    s.log.take();
    s.hit(boss, ME, 1_000_000.0);
    let sent = s.log.take();
    assert!(messages(&sent, ME).contains(&SystemMessageId::C1DiedAndDroppedS3S2));
    let item = s.world.ground_item(s.ground_of(1060)).unwrap();
    assert_eq!(item.owner, None);
}

// ------------------------------------------------------------------ pickup

/// A click on an item walks to it and picks it up: the packets of Java's `doPickupItem` in order,
/// the item in the inventory, gone from the ground for everybody.
#[test]
fn a_player_walks_to_an_item_and_picks_it_up() {
    let mut s = Scene::new(10);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.enter(OTHER, "Friend", (X + 30, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();

    let sword = s.ground_of(1);
    let sent = s.pick_up(ME, sword);

    // The picker: StopMove, the GetItem broadcast, the item vanishing, the message, the update.
    let mine = names(&sent, ME);
    assert!(
        has_in_order(
            &mine,
            &[
                ServerOpcode::StopMove.name(),
                ServerOpcode::GetItem.name(),
                ServerOpcode::DeleteObject.name(),
                ServerOpcode::SystemMessage.name(),
                ServerOpcode::InventoryUpdate.name(),
            ]
        ),
        "{mine:?}"
    );
    let get: GetItem = decode(of(&sent, ME, ServerOpcode::GetItem)[0]);
    assert_eq!((get.player_id, get.object_id), (ME, sword.0));
    // The neighbour sees the same: GetItem, then the item vanishes.
    let theirs = names(&sent, OTHER);
    assert!(has_in_order(
        &theirs,
        &[ServerOpcode::GetItem.name(), ServerOpcode::DeleteObject.name()]
    ));
    assert_eq!(s.inventory(ME), [(1, 1, "INVENTORY".to_string())]);
    assert!(messages(&sent, ME).contains(&SystemMessageId::YouHaveObtainedS1));
    // The weapon was picked up: everybody around hears who got it.
    assert!(messages(&sent, OTHER).contains(&SystemMessageId::AttentionC1HasPickedUpS2));
    assert!(!s.world.is_in_auto_destroy(sword));
    assert!(s.world.object(sword).is_none());
    // The row is written by the next full store (Java writes an added item then too).
    assert!(s.persisted.item_changes().is_empty());
}

/// A stackable item joins the stack in the inventory: the stack is the one `InventoryUpdate`
/// entry, flagged as modified, and the ground item's id goes back to the factory.
#[test]
fn a_picked_up_stack_merges_with_the_inventory() {
    let mut s = Scene::new(11);
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![item(MY_POTIONS, 1060, "INVENTORY", 0, 2)],
    );
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();

    let potions = s.ground_of(1060);
    assert!(s.ids.is_allocated(potions));
    let sent = s.pick_up(ME, potions);

    assert_eq!(s.inventory(ME), [(1060, 5, "INVENTORY".to_string())]);
    assert!(
        !s.ids.is_allocated(potions),
        "the merged item is destroyed and its id released"
    );
    let update: InventoryUpdate = decode(of(&sent, ME, ServerOpcode::InventoryUpdate)[0]);
    assert_eq!(update.updates.len(), 1);
    assert_eq!(update.updates[0].change, InventoryUpdate::MODIFIED);
    assert_eq!(update.updates[0].item.object_id, MY_POTIONS);
    assert_eq!(update.updates[0].item.count, 5);
    // "You have obtained 3 Potion" (count above one).
    assert!(messages(&sent, ME).contains(&SystemMessageId::YouHaveObtainedS2S1));
}

/// Adena is added with `addAdena`: its own message, and the adena item is created when the player
/// has none (then the whole item list is resent).
#[test]
fn picking_up_adena_creates_the_adena_item() {
    let mut s = Scene::new(12);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();
    let adena = s.ground_of(57);
    let sent = s.pick_up(ME, adena);
    assert_eq!(s.inventory(ME), [(57, 50, "INVENTORY".to_string())]);
    assert_eq!(s.world.adena_of(ObjectId(ME)), 50);
    // The first adena goes through `addItem(PICKUP, item)`: "You have obtained 50 Adena".
    let ids = messages(&sent, ME);
    assert!(ids.contains(&SystemMessageId::YouHaveObtainedS2S1), "{ids:?}");

    // Picking up more adena later uses `addAdena`: its own message, the item modified.
    // (`getAdenaInstance() != null` is what picks that branch.)
    let mut s = Scene::new(12);
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![item(MY_POTIONS, 57, "INVENTORY", 0, 1000)],
    );
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();
    let adena = s.ground_of(57);
    let sent = s.pick_up(ME, adena);
    assert_eq!(s.world.adena_of(ObjectId(ME)), 1050);
    assert!(messages(&sent, ME).contains(&SystemMessageId::YouHaveEarnedS1Adena));
    let update: InventoryUpdate = decode(of(&sent, ME, ServerOpcode::InventoryUpdate)[0]);
    assert_eq!(update.updates[0].change, InventoryUpdate::MODIFIED);
}

/// The killer is protected for 15 seconds: a stranger gets "failed to pick up" and nothing else.
/// When the time is over, the stranger may take it.
#[test]
fn the_killer_owns_the_loot_for_fifteen_seconds() {
    let mut s = Scene::new(13);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.enter(OTHER, "Thief", (X + 20, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();

    let potions = s.ground_of(1060);
    s.click(OTHER, potions);
    s.ticks(100);
    let sent = s.log.take();
    assert!(s.world.ground_item(potions).is_some());
    assert!(messages(&sent, OTHER).contains(&SystemMessageId::YouHaveFailedToPickUpS1));
    assert!(names(&sent, OTHER).contains(&ServerOpcode::ActionFail.name().to_string()));
    assert!(s.inventory(OTHER).is_empty());

    // 15 seconds after the drop both the drop protection and the loot privilege are over.
    s.ticks(300);
    assert!(s.world.ground_item(potions).unwrap().drop_protection.is_none());
    assert!(s.world.ground_item(potions).unwrap().owner.is_none());
    s.pick_up(OTHER, potions);
    assert_eq!(s.inventory(OTHER), [(1060, 3, "INVENTORY".to_string())]);
}

/// A full inventory refuses the pickup with `ActionFailed` and "your inventory is full".
#[test]
fn a_full_inventory_cannot_pick_up() {
    let mut s = Scene::with_settings(14, settings(|w| w.inventory_maximum_no_dwarf = 1));
    s.enter(ME, "Hero", (X, Y, Z), vec![item(MY_SWORD, 3, "INVENTORY", 0, 1)]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.log.take();
    let sword = s.ground_of(1);
    s.click(ME, sword);
    s.ticks(100);
    let sent = s.log.take();
    assert!(messages(&sent, ME).contains(&SystemMessageId::YourInventoryIsFull));
    assert!(s.world.ground_item(sword).is_some());
}

/// A bow and arrows: the arrows on the ground go to the left hand.
#[test]
fn arrows_are_equipped_when_a_bow_is_wielded() {
    let mut s = Scene::new(15);
    let paperdoll_rhand = i32::try_from(l2_protocol_game::server::charscreen::paperdoll::RHAND).unwrap();
    s.enter(
        ME,
        "Archer",
        (X, Y, Z),
        vec![item(MY_SWORD, 2, "PAPERDOLL", paperdoll_rhand, 1)],
    );
    // A neighbour drops arrows.
    s.enter(
        OTHER,
        "Friend",
        (X + 10, Y, Z),
        vec![item(MY_POTIONS, 17, "INVENTORY", 0, 30)],
    );
    s.input(
        OTHER,
        ClientInput::DropItem {
            object_id: MY_POTIONS,
            count: 30,
            x: X + 20,
            y: Y,
            z: Z,
        },
    );
    let arrows = s.ground_of(17);
    s.log.take();
    s.pick_up(ME, arrows);
    let lhand = s.world.paperdoll_object(
        ObjectId(ME),
        l2_protocol_game::server::charscreen::paperdoll::LHAND,
    );
    assert!(lhand.is_some(), "the arrows are in the left hand");
    assert_eq!(s.inventory(ME).len(), 2);
}

// ------------------------------------------------------------------ auto destroy

/// `ItemsAutoDestroyTaskManager`: every 5 seconds, an item older than the limit (strictly) is
/// destroyed, for everybody to see; its id goes back to the factory.
#[test]
fn dropped_items_are_destroyed_after_the_configured_time() {
    let mut s = Scene::with_settings(16, settings(|w| w.ground.autodestroy_item_after = 10));
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    let ground = s.world.ground_item_ids();
    assert_eq!(ground.len(), 3);
    s.log.take();

    // Checks run at 5, 10 and 15 seconds; 10 000 ms is not "more than" 10 seconds.
    s.ticks(250);
    assert_eq!(s.world.ground_item_ids().len(), 3);
    // (the monster's corpse decays in the meantime: only the items count)
    let deleted = |sent: &Sent| -> Vec<i32> {
        of(sent, ME, ServerOpcode::DeleteObject)
            .into_iter()
            .map(|b| decode::<DeleteObject>(b).object_id)
            .filter(|id| ground.iter().any(|g| g.0 == *id))
            .collect()
    };
    assert!(deleted(&s.log.take()).is_empty());
    s.ticks(60);
    let sent = s.log.take();
    assert!(s.world.ground_item_ids().is_empty());
    assert_eq!(deleted(&sent).len(), 3);
    for id in ground {
        assert!(!s.ids.is_allocated(id));
        assert!(!s.world.is_in_auto_destroy(id));
        assert!(s.world.object(id).is_none());
    }
}

/// `AutoDestroyDroppedItemAfter = 0` means one hour; an item's own `auto_destroy_time` wins over the
/// setting; a protected item id never goes.
#[test]
fn the_destroy_time_comes_from_the_item_the_setting_or_one_hour() {
    // The item of the scene with its own 7 second limit (player drops it; the setting is 600).
    let mut s = Scene::with_settings(17, settings(|w| w.ground.destroy_dropped_player_item = true));
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![item(MY_POTIONS, 1500, "INVENTORY", 0, 1)],
    );
    s.input(
        ME,
        ClientInput::DropItem {
            object_id: MY_POTIONS,
            count: 1,
            x: X + 10,
            y: Y,
            z: Z,
        },
    );
    assert!(s.world.is_in_auto_destroy(ObjectId(MY_POTIONS)));
    s.ticks(100);
    assert!(
        s.world.ground_item(ObjectId(MY_POTIONS)).is_some(),
        "5 000 ms is under 7 000"
    );
    s.ticks(100);
    assert!(
        s.world.ground_item(ObjectId(MY_POTIONS)).is_none(),
        "10 000 ms is over 7 000"
    );

    // Zero is an hour.
    let mut s = Scene::with_settings(18, settings(|w| w.ground.autodestroy_item_after = 0));
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    // `AUTODESTROY_ITEM_AFTER > 0` is the condition for the monster's drops to join the manager.
    assert!(
        s.world
            .ground_item_ids()
            .iter()
            .all(|id| !s.world.is_in_auto_destroy(*id))
    );
    s.ticks(100);
    assert_eq!(s.world.ground_item_ids().len(), 3);

    // A protected item id (Adena here) never joins the list.
    let mut s = Scene::with_settings(19, settings(|w| w.ground.list_protected_items = vec![57]));
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    assert!(!s.world.is_in_auto_destroy(s.ground_of(57)));
    assert!(s.world.is_in_auto_destroy(s.ground_of(1)));
}

// ------------------------------------------------------------------ auto loot

/// With `AutoLoot` the killer gets the drops at once and nothing lies on the ground.
#[test]
fn auto_loot_gives_the_drops_to_the_killer() {
    let mut s = Scene::with_settings(20, settings(|w| w.ground.auto_loot = true));
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.log.take();
    s.hit(monster, ME, 1_000_000.0);
    s.ticks(6);
    let sent = s.log.take();
    assert!(s.world.ground_item_ids().is_empty());
    assert_eq!(
        s.inventory(ME),
        [
            (57, 50, "INVENTORY".to_string()),
            (1, 1, "INVENTORY".to_string()),
            (1060, 3, "INVENTORY".to_string())
        ]
    );
    let ids = messages(&sent, ME);
    assert_eq!(
        ids.iter()
            .filter(|m| {
                matches!(
                    m,
                    SystemMessageId::YouHaveEarnedS1Adena
                        | SystemMessageId::YouHaveObtainedS1
                        | SystemMessageId::YouHaveObtainedS2S1
                )
            })
            .count(),
        3,
        "{ids:?}"
    );
    assert!(of(&sent, ME, ServerOpcode::DropItem).is_empty());
    // The items got Rust ids and the inventory update lists all three (one new, then the rest).
    let update: InventoryUpdate = decode(of(&sent, ME, ServerOpcode::InventoryUpdate)[0]);
    assert!(update.updates.iter().all(|u| u.change == InventoryUpdate::ADDED));
}

/// `AutoLootItemIds` loots one item and leaves the rest on the ground.
#[test]
fn auto_loot_item_ids_loot_single_items() {
    let mut s = Scene::with_settings(21, settings(|w| w.ground.auto_loot_item_ids = vec![1060]));
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    assert_eq!(s.inventory(ME), [(1060, 3, "INVENTORY".to_string())]);
    assert_eq!(s.world.ground_item_ids().len(), 2);
    // An auto looted item has no loot privilege on the ground... and the others have it.
    assert!(
        s.world
            .ground_item_ids()
            .iter()
            .all(|id| s.world.ground_item(*id).unwrap().owner == Some(ObjectId(ME)))
    );
}

/// An item that does not fit after auto loot is dropped at the player's feet.
#[test]
fn what_does_not_fit_is_dropped_again() {
    let mut s = Scene::with_settings(
        22,
        settings(|w| {
            w.ground.auto_loot = true;
            w.inventory_maximum_no_dwarf = 1;
            w.inventory.auto_loot_slot_limit = true;
        }),
    );
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.log.take();
    s.hit(monster, ME, 1_000_000.0);
    s.ticks(6);
    let sent = s.log.take();
    // The adena took the only slot; the sword and the potions were dropped again.
    assert_eq!(s.inventory(ME), [(57, 50, "INVENTORY".to_string())]);
    let ground: Vec<i32> = s
        .world
        .ground_item_ids()
        .iter()
        .map(|id| s.world.ground_item(*id).unwrap().data.item_id)
        .collect();
    assert_eq!(ground.len(), 2);
    assert!(ground.contains(&1) && ground.contains(&1060));
    assert_eq!(
        messages(&sent, ME)
            .iter()
            .filter(|m| **m == SystemMessageId::YouHaveDroppedS1)
            .count(),
        2
    );
    // A dropped player item stays (`DestroyPlayerDroppedItem` is off: it is protected).
    for id in s.world.ground_item_ids() {
        let item = s.world.ground_item(id).unwrap();
        assert!(item.protected);
        assert!(!s.world.is_in_auto_destroy(id));
        // Within 25 of the player (`Rnd.get(50) - 25`), 20 above.
        assert!((X - 25..=X + 25).contains(&item.x) && (Y - 25..=Y + 25).contains(&item.y));
        assert_eq!(item.z, Z + 20);
    }
}

// ------------------------------------------------------------------ RequestDropItem

/// A player drops part of a stack and a whole item: the ground items, the packets the neighbours
/// get, the rows, the protection against the auto destroy, and the pickup by somebody else.
#[test]
fn a_player_drops_items_and_a_neighbour_picks_them_up() {
    let mut s = Scene::new(23);
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![
            item(MY_SWORD, 1, "INVENTORY", 0, 1),
            item(MY_POTIONS, 1060, "INVENTORY", 0, 10),
        ],
    );
    s.enter(OTHER, "Friend", (X + 30, Y, Z), vec![]);
    s.log.take();
    s.persisted.take();

    s.input(
        ME,
        ClientInput::DropItem {
            object_id: MY_POTIONS,
            count: 4,
            x: X + 50,
            y: Y,
            z: Z,
        },
    );
    let sent = s.log.take();
    // The split-off part is a new item with a Rust id; the stack in the inventory is smaller.
    assert_eq!(s.inventory(ME)[1], (1060, 6, "INVENTORY".to_string()));
    let dropped = s.ground_of(1060);
    assert_ne!(dropped.0, MY_POTIONS);
    assert!(s.ids.is_allocated(dropped));
    let ground = s.world.ground_item(dropped).unwrap();
    assert_eq!(
        (ground.data.count, ground.x, ground.y, ground.z),
        (4, X + 50, Y, Z)
    );
    // `DestroyPlayerDroppedItem` is off: protected from the auto destroy, not on its list,
    // without a loot owner or drop protection.
    assert!(ground.protected && ground.owner.is_none() && ground.drop_protection.is_none());
    assert!(!s.world.is_in_auto_destroy(dropped));
    // The neighbour sees a `DropItem` with the player as the dropper.
    let packet: DropItem = decode(of(&sent, OTHER, ServerOpcode::DropItem)[0]);
    assert_eq!(
        (packet.player_id, packet.object_id, packet.count),
        (ME, dropped.0, 4)
    );
    // The dropper hears it too, then the load; the inventory update comes 100 ms later.
    let mine = names(&sent, ME);
    assert!(has_in_order(
        &mine,
        &[ServerOpcode::DropItem.name(), ServerOpcode::StatusUpdate.name()]
    ));
    assert_eq!(s.persisted.item_changes(), [(vec![MY_POTIONS], vec![])]);

    // The whole sword: it keeps its id, its row is deleted.
    s.input(
        ME,
        ClientInput::DropItem {
            object_id: MY_SWORD,
            count: 1,
            x: X - 40,
            y: Y,
            z: Z,
        },
    );
    assert!(s.world.ground_item(ObjectId(MY_SWORD)).is_some());
    assert_eq!(s.persisted.item_changes(), [(vec![], vec![MY_SWORD])]);
    s.ticks(6);
    let sent = s.log.take();
    let updates: Vec<InventoryUpdate> = of(&sent, ME, ServerOpcode::InventoryUpdate)
        .into_iter()
        .map(decode)
        .collect();
    assert!(!updates.is_empty());

    // Nobody protects a player's drop: the neighbour takes the potions at once.
    s.pick_up(OTHER, dropped);
    assert_eq!(s.inventory(OTHER), [(1060, 4, "INVENTORY".to_string())]);
}

/// With `DestroyPlayerDroppedItem` a dropped plain item joins the auto destroy list and is not
/// protected; equipment stays unless `DestroyEquipableItem` is on too.
#[test]
fn dropped_player_items_follow_the_destroy_settings() {
    for (equipable_too, sword_destroyed) in [(false, false), (true, true)] {
        let mut s = Scene::with_settings(
            24,
            settings(|w| {
                w.ground.destroy_dropped_player_item = true;
                w.ground.destroy_equipable_player_item = equipable_too;
            }),
        );
        s.enter(
            ME,
            "Hero",
            (X, Y, Z),
            vec![
                item(MY_SWORD, 1, "INVENTORY", 0, 1),
                item(MY_POTIONS, 1060, "INVENTORY", 0, 10),
            ],
        );
        for id in [MY_SWORD, MY_POTIONS] {
            s.input(
                ME,
                ClientInput::DropItem {
                    object_id: id,
                    count: if id == MY_SWORD { 1 } else { 10 },
                    x: X + 10,
                    y: Y,
                    z: Z,
                },
            );
        }
        let potions = s.world.ground_item(ObjectId(MY_POTIONS)).unwrap();
        assert!(!potions.protected);
        assert!(s.world.is_in_auto_destroy(ObjectId(MY_POTIONS)));
        let sword = s.world.ground_item(ObjectId(MY_SWORD)).unwrap();
        assert_eq!(s.world.is_in_auto_destroy(ObjectId(MY_SWORD)), sword_destroyed);
        assert_eq!(sword.protected, !sword_destroyed);
    }
}

/// A worn item is taken off first: the rows are written in Java's order and the player gets the
/// user info and the item list.
#[test]
fn dropping_a_worn_weapon_takes_it_off_first() {
    let mut s = Scene::new(25);
    let rhand = i32::try_from(l2_protocol_game::server::charscreen::paperdoll::RHAND).unwrap();
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![item(MY_SWORD, 1, "PAPERDOLL", rhand, 1)],
    );
    s.log.take();
    s.input(
        ME,
        ClientInput::DropItem {
            object_id: MY_SWORD,
            count: 1,
            x: X + 10,
            y: Y,
            z: Z,
        },
    );
    s.ticks(8);
    let sent = s.log.take();
    let mine = names(&sent, ME);
    assert!(
        mine.contains(&ServerOpcode::UserInfo.name().to_string()),
        "{mine:?}"
    );
    assert!(
        mine.contains(&ServerOpcode::ItemList.name().to_string()),
        "{mine:?}"
    );
    assert!(s.world.ground_item(ObjectId(MY_SWORD)).is_some());
    assert!(s.world.player(ObjectId(ME)).unwrap().data.items.is_empty());
}

// ------------------------------------------------------------------ SaveDroppedItem

fn saving() -> WorldSettings {
    settings(|w| {
        w.ground.save_dropped_item = true;
        w.ground.save_dropped_item_interval_ms = 60_000;
    })
}

/// With the default `SaveDroppedItem = False` the table is never written, at any time.
#[test]
fn nothing_is_saved_by_default() {
    let mut s = Scene::new(26);
    s.world.load_ground_items(vec![]);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.persisted.take();
    s.world.save_ground_items();
    s.ticks(1300);
    assert!(s.persisted.ground_saves().is_empty());
    let (tx, _rx) = std::sync::mpsc::channel::<()>();
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(move || {
            let _ = tx.send(());
        }),
    });
    assert!(s.persisted.ground_saves().is_empty());
}

/// `ItemsOnGroundManager.run()`: every item on the ground is a row, protected items with
/// `drop_time` -1, and the equipable flag; the timer repeats it and the shutdown writes it once
/// more.
#[test]
fn the_items_on_the_ground_are_saved_like_java_saves_them() {
    let mut s = Scene::with_settings(27, saving());
    s.world.load_ground_items(vec![]);
    s.enter(ME, "Hero", (X, Y, Z), vec![item(MY_SWORD, 1, "INVENTORY", 0, 1)]);
    let monster = s.spawn_monster(9001);
    s.hit(monster, ME, 1_000_000.0);
    s.input(
        ME,
        ClientInput::DropItem {
            object_id: MY_SWORD,
            count: 1,
            x: X + 10,
            y: Y,
            z: Z,
        },
    );
    s.persisted.take();

    s.world.save_ground_items();
    let saves = s.persisted.ground_saves();
    assert_eq!(saves.len(), 1);
    let rows = &saves[0];
    assert_eq!(rows.len(), 4);
    let row_of = |object_id: ObjectId| rows.iter().find(|r| r.object_id == object_id).unwrap();
    // The sword the player dropped: protected, equipable.
    let mine = row_of(ObjectId(MY_SWORD));
    assert_eq!(
        (mine.item_id, mine.count, mine.drop_time, mine.equipable),
        (1, 1, -1, true)
    );
    assert_eq!((mine.x, mine.y, mine.z), (X + 10, Y, Z));
    // The monster's adena: a real drop time (unix milliseconds), not equipable.
    let adena = row_of(s.ground_of(57));
    assert_eq!((adena.item_id, adena.count, adena.equipable), (57, 50, false));
    assert!(adena.drop_time > 1_000_000_000_000, "{}", adena.drop_time);
    // In object id order.
    assert!(rows.windows(2).all(|w| w[0].object_id.0 < w[1].object_id.0));

    // The timer: every `SaveDroppedItemInterval`.
    s.ticks(1199);
    assert!(s.persisted.ground_saves().is_empty());
    s.ticks(1);
    assert_eq!(s.persisted.ground_saves().len(), 1);
    s.ticks(1200);
    assert_eq!(s.persisted.ground_saves().len(), 1);

    // Shutdown ("Save items on ground before closing").
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    assert_eq!(s.persisted.ground_saves().len(), 1);
}

/// `ItemsOnGroundManager.load()`: the rows come back as items on the ground; protected ones
/// (`drop_time` -1) are kept out of the auto destroy list, the others join it with a fresh drop
/// time. A player who comes by sees them.
#[test]
fn saved_items_come_back_at_startup() {
    let mut s = Scene::with_settings(28, saving());
    let rows = vec![
        GroundItemRow {
            object_id: ObjectId(0x1000_0001),
            item_id: 1,
            count: 1,
            enchant_level: 3,
            x: X + 5,
            y: Y,
            z: Z,
            drop_time: -1,
            equipable: true,
        },
        GroundItemRow {
            object_id: ObjectId(0x1000_0002),
            item_id: 1060,
            count: 12,
            enchant_level: 0,
            x: X - 5,
            y: Y,
            z: Z,
            drop_time: 1_700_000_000_000,
            equipable: false,
        },
        // An item the datapack does not know is skipped.
        GroundItemRow {
            object_id: ObjectId(0x1000_0003),
            item_id: 987_654,
            count: 1,
            enchant_level: 0,
            x: X,
            y: Y,
            z: Z,
            drop_time: -1,
            equipable: false,
        },
    ];
    s.world.load_ground_items(rows);
    assert_eq!(
        s.world.ground_item_ids(),
        [ObjectId(0x1000_0001), ObjectId(0x1000_0002)]
    );
    let sword = s.world.ground_item(ObjectId(0x1000_0001)).unwrap();
    assert_eq!((sword.data.enchant_level, sword.protected), (3, true));
    assert!(!s.world.is_in_auto_destroy(ObjectId(0x1000_0001)));
    let potions = s.world.ground_item(ObjectId(0x1000_0002)).unwrap();
    assert_eq!((potions.data.count, potions.protected), (12, false));
    assert!(s.world.is_in_auto_destroy(ObjectId(0x1000_0002)));
    // The drop time was reset to "now" by `addItem`.
    assert_eq!(potions.drop_time_ms, Some(s.world.world_ms()));

    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let sent = s.log.take();
    assert_eq!(of(&sent, ME, ServerOpcode::SpawnItem).len(), 2);

    // And the next save writes them again, the protected one with -1.
    s.persisted.take();
    s.world.save_ground_items();
    let rows = &s.persisted.ground_saves()[0];
    assert_eq!(
        rows.iter().map(|r| r.drop_time == -1).collect::<Vec<_>>(),
        [true, false]
    );
}

// ------------------------------------------------------------------ the player dies

/// A player with karma drops items when a monster kills it (`Player.onDieDropItem`): at most
/// `KarmaDropLimit` of them, none that are on the non-droppable list.
#[test]
fn a_player_with_karma_drops_items_when_killed() {
    let mut s = Scene::with_settings(
        29,
        settings(|w| {
            w.drops.karma_rate_drop = 100;
            w.drops.karma_rate_drop_item = 100;
            w.drops.karma_rate_drop_equip = 100;
            w.drops.karma_rate_drop_equip_weapon = 100;
            w.drops.karma_drop_limit = 2;
            w.drops.karma_pk_limit = 5;
        }),
    );
    let mut data = player_data(
        ME,
        "Bandit",
        (X, Y, Z),
        vec![
            item(MY_SWORD, 1, "INVENTORY", 0, 1),
            item(MY_POTIONS, 1060, "INVENTORY", 0, 10),
            // Adena never drops.
            item(MY_POTIONS + 1, 57, "INVENTORY", 0, 1000),
            item(MY_POTIONS + 2, 3, "INVENTORY", 0, 1),
        ],
    );
    data.karma = 100;
    data.pk_kills = 6;
    s.enter_data(data);
    let monster = s.spawn_monster(9001);
    s.log.take();
    s.persisted.take();

    s.world
        .reduce_current_hp(ObjectId(ME), 1_000_000.0, Some(monster));
    assert!(s.world.is_dead(ObjectId(ME)));
    // Two of the three droppable items (the limit), in inventory order; the adena stays.
    let ground: Vec<i32> = s
        .world
        .ground_item_ids()
        .iter()
        .map(|id| s.world.ground_item(*id).unwrap().data.item_id)
        .collect();
    assert_eq!(ground.len(), 2);
    assert!(ground.contains(&1) && ground.contains(&1060));
    assert!(s.inventory(ME).iter().any(|(id, _, _)| *id == 57));
    assert!(s.inventory(ME).iter().any(|(id, _, _)| *id == 3));
    // The rows are deleted.
    let changes = s.persisted.item_changes();
    assert!(changes.contains(&(vec![], vec![MY_SWORD])), "{changes:?}");
    // A player without karma and below the PlayerRateDrop of 5 percent keeps everything in the
    // default settings; here a level 1 player is below the level 5 limit anyway.
}

/// A level 5 player killed by a monster uses the `PlayerRateDrop` rates.
#[test]
fn a_player_killed_by_a_monster_may_drop_by_the_player_rates() {
    let mut s = Scene::with_settings(
        30,
        settings(|w| {
            w.drops.player_rate_drop = 100;
            w.drops.player_rate_drop_item = 100;
            w.drops.player_drop_limit = 1;
        }),
    );
    let mut data = player_data(
        ME,
        "Unlucky",
        (X, Y, Z),
        vec![
            item(MY_SWORD, 1, "INVENTORY", 0, 1),
            item(MY_POTIONS, 1060, "INVENTORY", 0, 10),
        ],
    );
    data.level = 5;
    s.enter_data(data);
    let monster = s.spawn_monster(9001);
    s.world
        .reduce_current_hp(ObjectId(ME), 1_000_000.0, Some(monster));
    // Level 5 is above the limit of "level > 4"; one item (the limit) lies there.
    assert_eq!(s.world.ground_item_ids().len(), 1);
    assert_eq!(s.inventory(ME).len(), 1);
}
