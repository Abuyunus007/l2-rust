//! The drops of a dead monster: `NpcTemplate.calculateDrops` (the drop lists, the groups, the
//! rates and the integer/double quirks of Java) and `Attackable.doItemDrop` (who gets the loot,
//! auto loot, items on the ground).
// Java: entity/actor/templates/NpcTemplate.java (calculateDrops, calculateGroupDrops,
//       calculateUngroupedDrops, calculateGroupDrop, calculateUngroupedDrop)
// Java: entity/actor/Attackable.java (calculateRewards: who the loot goes to; doItemDrop)
// Java: data/xml/NpcData.java (the drop list post-processing: unknown items dropped, ungrouped
//       lists sorted by chance high to low, groups low to high, `BossDrop*`)
// Java: config/RatesConfig.java
// Java: util/MathUtil.java (scaleToRange)
//
// # How the numbers are computed
// All of it is done like Java does it, so a seeded run draws in the same order:
// - the multipliers of `Rates.ini` are `float`s, widened to `double` when they meet a chance
//   (`0.1f` is `0.10000000149011612`), so they are `f32` here and `f64::from` at the use;
// - a drop's level gap check draws first (`Rnd.nextDouble() * 100`), the chance draws second, the
//   count (`Rnd.get(min, max)` with longs, which draws nothing when `min >= max`) third;
// - the count is `(long) (Rnd.get(min, max) * rateAmount)`, a saturating cast;
// - `rateChance == 1` is an exact `double` comparison and decides if a group rolls its items one
//   after the other (the x1 case) or each on its own (any custom rate).
//
// # Not modelled, with what brings it
// - Spoil: the state (`Attackable._spoilerObjectId`, `_sweepItems`) and its checks are at the end
//   of this file (M5-SPOIL-01); the `Spoil` / `Sweeper` effects are in `effects/handlers/`.
//   `isInLooterParty` is `party/mod.rs` (M6-PARTY-01).
// - Champions (`ChampionMonsters.ini`, off by default; nobody is a champion) and the premium
//   system (`PremiumSystem.ini`, off by default; there is no premium status). A seeded monster
//   (`manor/farming.rs`, M7-MANOR-01) keeps only adena and the seal stones.
// - `EventDropManager.doEventDrop` (event drops, no event runs), fake players that drop items.
// - Parties: the leader of the party that did the most damage is the one the loot belongs to
//   when its members together beat the single biggest dealer; auto loot is shared by
//   `Party.distributeItem` (`party/loot.rs`, M6-PARTY-01). The raid boss loot right of a command
//   channel is `party/command_channel.rs` and `ground_item.rs`.

use std::collections::BTreeMap;

use l2_data::ItemTemplates;
use l2_data::NpcTemplate;
use l2_data::Stat;
use l2_data::item::ADENA_ID;
use l2_data::npc::{DropEntry, DropGroup, DropType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::formulas::Rnd;
use crate::target::distance_3d;
use crate::world::World;

/// Java `ItemHolder`: an item id and a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemHolder {
    pub id: i32,
    pub count: i64,
}

/// The values of `Rates.ini` that drops read, plus `AltPartyRange` (the damage dealer must be
/// within it). `Default` is the Java default of each key (not the shipped `Rates.ini`: the two
/// per-item maps are empty in the code).
#[derive(Debug, Clone, PartialEq)]
pub struct DropSettings {
    /// `RATE_DEATH_DROP_AMOUNT_MULTIPLIER` (`DeathDropAmountMultiplier`), a Java `float`.
    pub rate_death_drop_amount_multiplier: f32,
    /// `RATE_SPOIL_DROP_AMOUNT_MULTIPLIER` (`SpoilDropAmountMultiplier`).
    pub rate_spoil_drop_amount_multiplier: f32,
    /// `RATE_HERB_DROP_AMOUNT_MULTIPLIER` (`HerbDropAmountMultiplier`).
    pub rate_herb_drop_amount_multiplier: f32,
    /// `RATE_RAID_DROP_AMOUNT_MULTIPLIER` (`RaidDropAmountMultiplier`).
    pub rate_raid_drop_amount_multiplier: f32,
    /// `RATE_DEATH_DROP_CHANCE_MULTIPLIER` (`DeathDropChanceMultiplier`).
    pub rate_death_drop_chance_multiplier: f32,
    /// `RATE_SPOIL_DROP_CHANCE_MULTIPLIER` (`SpoilDropChanceMultiplier`).
    pub rate_spoil_drop_chance_multiplier: f32,
    /// `RATE_HERB_DROP_CHANCE_MULTIPLIER` (`HerbDropChanceMultiplier`).
    pub rate_herb_drop_chance_multiplier: f32,
    /// `RATE_RAID_DROP_CHANCE_MULTIPLIER` (`RaidDropChanceMultiplier`).
    pub rate_raid_drop_chance_multiplier: f32,
    /// `RATE_DROP_AMOUNT_BY_ID` (`DropAmountMultiplierByItemId`).
    pub rate_drop_amount_by_id: BTreeMap<i32, f32>,
    /// `RATE_DROP_CHANCE_BY_ID` (`DropChanceMultiplierByItemId`).
    pub rate_drop_chance_by_id: BTreeMap<i32, f32>,
    /// `DROP_MAX_OCCURRENCES_NORMAL` (`DropMaxOccurrencesNormal`).
    pub drop_max_occurrences_normal: i32,
    /// `DROP_MAX_OCCURRENCES_RAIDBOSS` (`DropMaxOccurrencesRaidboss`).
    pub drop_max_occurrences_raidboss: i32,
    /// `DROP_ADENA_MIN_LEVEL_DIFFERENCE` (`DropAdenaMinLevelDifference`).
    pub drop_adena_min_level_difference: i32,
    /// `DROP_ADENA_MAX_LEVEL_DIFFERENCE` (`DropAdenaMaxLevelDifference`).
    pub drop_adena_max_level_difference: i32,
    /// `DROP_ADENA_MIN_LEVEL_GAP_CHANCE` (`DropAdenaMinLevelGapChance`).
    pub drop_adena_min_level_gap_chance: f64,
    /// `DROP_ITEM_MIN_LEVEL_DIFFERENCE` (`DropItemMinLevelDifference`).
    pub drop_item_min_level_difference: i32,
    /// `DROP_ITEM_MAX_LEVEL_DIFFERENCE` (`DropItemMaxLevelDifference`).
    pub drop_item_max_level_difference: i32,
    /// `DROP_ITEM_MIN_LEVEL_GAP_CHANCE` (`DropItemMinLevelGapChance`).
    pub drop_item_min_level_gap_chance: f64,
    /// `BOSS_DROP_ENABLED` (`BossDropEnable`).
    pub boss_drop_enabled: bool,
    /// `BOSS_DROP_MIN_LEVEL` (`BossDropMinLevel`).
    pub boss_drop_min_level: i32,
    /// `BOSS_DROP_MAX_LEVEL` (`BossDropMaxLevel`).
    pub boss_drop_max_level: i32,
    /// `BOSS_DROP_LIST` (`BossDropList`), all of type `DROP`.
    pub boss_drop_list: Vec<DropEntry>,
    /// `PlayerConfig.ALT_PARTY_RANGE` (`AltPartyRange`): a damage dealer farther from the monster
    /// than this gets no share of the loot.
    pub alt_party_range: i32,
    /// `RatesConfig.EVENT_ITEM_MAX_LEVEL_DIFFERENCE` (`EventItemMaxLevelDifference`): the event
    /// drops (`event_drop.rs`) go to a killer at most this many levels above the monster.
    pub event_item_max_level_difference: i32,
    /// `PLAYER_DROP_LIMIT`, `PLAYER_RATE_DROP`, `PLAYER_RATE_DROP_ITEM`, `PLAYER_RATE_DROP_EQUIP`,
    /// `PLAYER_RATE_DROP_EQUIP_WEAPON` (`PlayerDropLimit` ...): what a player killed by a monster
    /// drops.
    pub player_drop_limit: i32,
    pub player_rate_drop: i32,
    pub player_rate_drop_item: i32,
    pub player_rate_drop_equip: i32,
    pub player_rate_drop_equip_weapon: i32,
    /// `KARMA_DROP_LIMIT`, `KARMA_RATE_DROP`, ... (`KarmaDropLimit` ...): what a player with karma
    /// drops when killed.
    pub karma_drop_limit: i32,
    pub karma_rate_drop: i32,
    pub karma_rate_drop_item: i32,
    pub karma_rate_drop_equip: i32,
    pub karma_rate_drop_equip_weapon: i32,
    /// `PvpConfig.KARMA_DROP_GM` (`CanGMDropEquipment`).
    pub karma_drop_gm: bool,
    /// `PvpConfig.KARMA_PK_LIMIT` (`MinimumPKRequiredToDrop`).
    pub karma_pk_limit: i32,
    /// `PvpConfig.KARMA_LIST_NONDROPPABLE_ITEMS` (`ListOfNonDroppableItems`).
    pub karma_list_nondroppable_items: Vec<i32>,
}

impl Default for DropSettings {
    fn default() -> Self {
        Self {
            rate_death_drop_amount_multiplier: 1.0,
            rate_spoil_drop_amount_multiplier: 1.0,
            rate_herb_drop_amount_multiplier: 1.0,
            rate_raid_drop_amount_multiplier: 1.0,
            rate_death_drop_chance_multiplier: 1.0,
            rate_spoil_drop_chance_multiplier: 1.0,
            rate_herb_drop_chance_multiplier: 1.0,
            rate_raid_drop_chance_multiplier: 1.0,
            rate_drop_amount_by_id: BTreeMap::new(),
            rate_drop_chance_by_id: BTreeMap::new(),
            drop_max_occurrences_normal: 2,
            drop_max_occurrences_raidboss: 7,
            drop_adena_min_level_difference: 8,
            drop_adena_max_level_difference: 15,
            drop_adena_min_level_gap_chance: 10.0,
            drop_item_min_level_difference: 5,
            drop_item_max_level_difference: 10,
            drop_item_min_level_gap_chance: 10.0,
            boss_drop_enabled: false,
            boss_drop_min_level: 40,
            boss_drop_max_level: 999,
            boss_drop_list: Vec::new(),
            alt_party_range: 1500,
            event_item_max_level_difference: 9,
            player_drop_limit: 3,
            player_rate_drop: 5,
            player_rate_drop_item: 70,
            player_rate_drop_equip: 25,
            player_rate_drop_equip_weapon: 5,
            karma_drop_limit: 10,
            karma_rate_drop: 70,
            karma_rate_drop_item: 50,
            karma_rate_drop_equip: 40,
            karma_rate_drop_equip_weapon: 10,
            karma_drop_gm: false,
            karma_pk_limit: 5,
            karma_list_nondroppable_items: vec![
                57, 1147, 425, 1146, 461, 10, 2368, 7, 6, 2370, 2369, 6842, 6611, 6612, 6613, 6614, 6615,
                6616, 6617, 6618, 6619, 6620, 6621, 7694, 8181, 5575, 7694, 9388, 9389, 9390,
            ],
        }
    }
}

/// The monster side of a drop roll (Java `victim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropVictim {
    /// `getLevel()`.
    pub level: i32,
    /// `isRaid()`.
    pub raid: bool,
    /// `isChampion()`: nobody is one yet.
    pub champion: bool,
    /// `isMonster() && isSeeded()`: a seeded monster only drops adena and seal stones (the manor,
    /// M7-MANOR-01).
    pub seeded: bool,
}

/// The player side of a drop roll (Java `killer.asPlayer()`): its level and the multipliers of
/// its bonus stats (`1 + calcStat(BONUS_DROP_RATE, 0) / 100` and so on).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropKiller {
    pub level: i32,
    pub bonus_drop_rate: f64,
    pub bonus_drop_amount: f64,
    pub bonus_drop_adena: f64,
    pub bonus_spoil_rate: f64,
}

impl DropKiller {
    /// A killer without any bonus.
    pub fn plain(level: i32) -> Self {
        Self {
            level,
            bonus_drop_rate: 1.0,
            bonus_drop_amount: 1.0,
            bonus_drop_adena: 1.0,
            bonus_spoil_rate: 1.0,
        }
    }
}

/// The template id `Chest.doItemDrop` swaps in for a chest nobody unlocked (`id += 3536` for the
/// first range, a fixed id for each pair after it).
pub(crate) fn locked_chest_drop_id(id: i32) -> i32 {
    match id {
        18265..=18286 => id + 3536,
        18287 | 18288 => 21671,
        18289 | 18290 => 21694,
        18291 | 18292 => 21717,
        18293 | 18294 => 21740,
        18295 | 18296 => 21763,
        18297 | 18298 => 21786,
        _ => id,
    }
}

/// Java `MathUtil.scaleToRange(double, double, double, double, double)`.
fn scale_to_range(value: f64, source_min: f64, source_max: f64, target_min: f64, target_max: f64) -> f64 {
    let clamped = value.min(source_max).max(source_min);
    (((clamped - source_min) * (target_max - target_min)) / (source_max - source_min)) + target_min
}

/// A drop that was rolled, tagged with its position so the `remove(Object)` calls of Java (which
/// compare by identity) can find it again.
#[derive(Debug, Clone, Copy)]
struct Rolled {
    seq: usize,
    holder: ItemHolder,
}

struct Gaps {
    adena: f64,
    item: f64,
}

impl Gaps {
    fn of(s: &DropSettings, victim: &DropVictim, killer: &DropKiller) -> Self {
        let level_difference = f64::from(victim.level - killer.level);
        Self {
            adena: scale_to_range(
                level_difference,
                f64::from(-s.drop_adena_max_level_difference),
                f64::from(-s.drop_adena_min_level_difference),
                s.drop_adena_min_level_gap_chance,
                100.0,
            ),
            item: scale_to_range(
                level_difference,
                f64::from(-s.drop_item_max_level_difference),
                f64::from(-s.drop_item_min_level_difference),
                s.drop_item_min_level_gap_chance,
                100.0,
            ),
        }
    }

    /// `dropItem.getItemId() == Inventory.ADENA_ID ? adena : item`.
    fn of_item(&self, item_id: i32) -> f64 {
        if item_id == ADENA_ID {
            self.adena
        } else {
            self.item
        }
    }
}

/// The chance multiplier of one `DROP` item (the first block of `calculateGroupDrops` and
/// `calculateUngroupedDrop`).
fn rate_chance(
    s: &DropSettings,
    items: &ItemTemplates,
    item_id: i32,
    victim: &DropVictim,
    killer: &DropKiller,
) -> f64 {
    let ex_immediate = items.get(item_id).is_some_and(|t| t.ex_immediate_effect);
    let mut rate = 1.0_f64;
    if let Some(by_id) = s.rate_drop_chance_by_id.get(&item_id) {
        rate *= f64::from(*by_id);
        // Champion adena chance: `ChampionMonsters.ini`, off.
        if item_id == ADENA_ID && rate > 100.0 {
            rate = 100.0;
        }
    } else if ex_immediate {
        rate *= f64::from(s.rate_herb_drop_chance_multiplier);
    } else if victim.raid {
        rate *= f64::from(s.rate_raid_drop_chance_multiplier);
    } else {
        // Champion rewards chance: off.
        rate *= f64::from(s.rate_death_drop_chance_multiplier);
    }
    // Premium chance: `PremiumSystem.ini` is off. Bonus drop rate effect:
    rate *= killer.bonus_drop_rate;
    rate
}

/// The amount multiplier of one `DROP` item.
fn rate_amount(
    s: &DropSettings,
    items: &ItemTemplates,
    item_id: i32,
    victim: &DropVictim,
    killer: &DropKiller,
) -> f64 {
    let ex_immediate = items.get(item_id).is_some_and(|t| t.ex_immediate_effect);
    let mut rate = 1.0_f64;
    if let Some(by_id) = s.rate_drop_amount_by_id.get(&item_id) {
        rate *= f64::from(*by_id);
    } else if ex_immediate {
        rate *= f64::from(s.rate_herb_drop_amount_multiplier);
    } else if victim.raid {
        rate *= f64::from(s.rate_raid_drop_amount_multiplier);
    } else {
        rate *= f64::from(s.rate_death_drop_amount_multiplier);
    }
    // Bonus drop amount effect.
    rate *= killer.bonus_drop_amount;
    if item_id == ADENA_ID {
        rate *= killer.bonus_drop_adena;
    }
    rate
}

/// `(long) (Rnd.get(min, max) * rateAmount)`.
fn amount(rng: &mut impl Rnd, entry: &DropEntry, rate: f64) -> i64 {
    (rng.get_long_range(entry.min, entry.max) as f64 * rate) as i64
}

/// The ungrouped drop list of one type, as `NpcData` leaves it: items that do not exist dropped,
/// sorted by chance from high to low (a stable sort), the boss drops of `Rates.ini` added before
/// sorting.
fn ungrouped_list(
    s: &DropSettings,
    items: &ItemTemplates,
    template: &NpcTemplate,
    drop_type: DropType,
) -> Vec<DropEntry> {
    let source = match drop_type {
        DropType::Drop => &template.drops,
        DropType::Spoil => &template.spoils,
    };
    // Java checks the item of a drop when it loads the NPC ("Could not find drop with id").
    let mut list: Vec<DropEntry> = source
        .iter()
        .filter(|e| items.get(e.item_id).is_some())
        .copied()
        .collect();
    if drop_type == DropType::Drop
        && s.boss_drop_enabled
        && template.npc_type.contains("RaidBoss")
        && i32::from(template.level) >= s.boss_drop_min_level
        && i32::from(template.level) <= s.boss_drop_max_level
    {
        // The configurable boss drops are added unchecked.
        list.extend(s.boss_drop_list.iter().copied());
    }
    // Drops are sorted by chance (high to low); `Double.compare`.
    list.sort_by(|a, b| b.chance.total_cmp(&a.chance));
    list
}

/// The drop groups of the template, as `NpcData` leaves them: items that do not exist dropped, each
/// group sorted by chance from low to high.
fn group_list(items: &ItemTemplates, template: &NpcTemplate) -> Vec<DropGroup> {
    template
        .drop_groups
        .iter()
        .map(|group| {
            let mut entries: Vec<DropEntry> = group
                .items
                .iter()
                .filter(|e| items.get(e.item_id).is_some())
                .copied()
                .collect();
            // Group drops are sorted by chance (low to high).
            entries.sort_by(|a, b| a.chance.total_cmp(&b.chance));
            DropGroup {
                chance: group.chance,
                items: entries,
            }
        })
        .collect()
}

/// Java `NpcTemplate.calculateDrops(dropType, victim, killer)`: the items the monster drops.
///
/// `rng` draws in Java's order. An empty vector is Java's `null` (nothing dropped).
pub fn calculate_drops(
    settings: &DropSettings,
    items: &ItemTemplates,
    template: &NpcTemplate,
    drop_type: DropType,
    victim: &DropVictim,
    killer: &DropKiller,
    rng: &mut impl Rnd,
) -> Vec<ItemHolder> {
    match drop_type {
        DropType::Drop => {
            // Calculate group drops.
            let mut group_drops = Vec::new();
            if !template.drop_groups.is_empty() {
                group_drops = calculate_group_drops(settings, items, template, victim, killer, rng);
                if victim.seeded {
                    group_drops.retain(|i| is_seal_stone_or_adena(i.id));
                }
            }

            // Calculate ungrouped drops.
            let mut ungrouped_drops = Vec::new();
            let list = ungrouped_list(settings, items, template, DropType::Drop);
            if !list.is_empty() {
                ungrouped_drops = calculate_ungrouped_drops(settings, items, &list, victim, killer, rng);
                if victim.seeded {
                    ungrouped_drops.retain(|i| is_seal_stone_or_adena(i.id));
                }
            }

            // Return results.
            group_drops.extend(ungrouped_drops);
            group_drops
        }
        DropType::Spoil => {
            let list = ungrouped_list(settings, items, template, DropType::Spoil);
            if list.is_empty() {
                return Vec::new();
            }
            calculate_ungrouped_drops(settings, items, &list, victim, killer, rng)
        }
    }
}

/// `i.getId() != 57 && != 6361 && != 6362 && != 6360` kept out of a seeded monster's drops.
fn is_seal_stone_or_adena(item_id: i32) -> bool {
    matches!(item_id, 57 | 6361 | 6362 | 6360)
}

/// Java `calculateGroupDrops`.
fn calculate_group_drops(
    s: &DropSettings,
    items: &ItemTemplates,
    template: &NpcTemplate,
    victim: &DropVictim,
    killer: &DropKiller,
    rng: &mut impl Rnd,
) -> Vec<ItemHolder> {
    let gaps = Gaps::of(s, victim, killer);
    let mut calculated: Option<Vec<Rolled>> = None;
    let mut counter = if victim.raid {
        s.drop_max_occurrences_raidboss
    } else {
        s.drop_max_occurrences_normal
    };
    let mut seq = 0usize;
    if counter > 0 {
        let mut random: Option<Vec<usize>> = None;
        let mut cached: Option<Rolled> = None;
        for group in group_list(items, template) {
            // Total group chance is 100.
            let mut total_chance = 0.0_f64;
            for drop_item in &group.items {
                let item_id = drop_item.item_id;
                let rate_chance = rate_chance(s, items, item_id, victim, killer);

                // Only use total chance on x1, custom rates break this logic because total chance
                // is more than 100%.
                if rate_chance == 1.0 {
                    total_chance += drop_item.chance;
                } else {
                    total_chance = drop_item.chance;
                }

                let group_item_chance = total_chance * (group.chance / 100.0) * rate_chance;

                // Check if maximum drop occurrences have been reached. Items that have 100% drop
                // chance without server rate multipliers drop normally.
                if counter == 0
                    && group_item_chance < 100.0
                    && let (Some(random), Some(calculated)) = (random.as_mut(), calculated.as_mut())
                {
                    // Custom rates break this logic because total chance is more than 100%.
                    if rate_chance == 1.0 && !random.is_empty() {
                        // Remove highest chance item (temporarily if no other item replaces it).
                        let first = random.remove(0);
                        if let Some(at) = calculated.iter().position(|r| r.seq == first) {
                            cached = Some(calculated.remove(at));
                        }
                    }
                    counter = 1;
                }

                // Check level gap that may prevent to drop item.
                if rng.next_double() * 100.0 > gaps.of_item(item_id) {
                    continue;
                }

                // Calculate chances.
                let Some(holder) =
                    calculate_group_drop(s, items, drop_item, victim, killer, group_item_chance, rng)
                else {
                    continue;
                };
                let rolled = Rolled { seq, holder };
                seq += 1;

                // Finally.
                let random = random.get_or_insert_with(Vec::new);
                let calculated = calculated.get_or_insert_with(Vec::new);
                let counts = match s.rate_drop_chance_by_id.get(&item_id) {
                    Some(by_id) => group_item_chance * f64::from(*by_id) < 100.0,
                    None => group_item_chance < 100.0,
                };
                if counts {
                    counter -= 1;
                    // Custom rates break this logic because total chance is more than 100%.
                    if rate_chance == 1.0 {
                        random.push(rolled.seq);
                    }
                }
                calculated.push(rolled);

                // No more drops from this group, only use on x1, custom rates break this logic
                // because total chance is more than 100%.
                if rate_chance == 1.0 {
                    break;
                }
            }
        }

        // Add temporarily removed item when not replaced.
        if counter > 0
            && let (Some(cached), Some(calculated)) = (cached, calculated.as_mut())
        {
            calculated.push(cached);
        }
        // Champion extra drop: `ChampionMonsters.ini` is off.
    }
    calculated
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.holder)
        .collect()
}

/// Java `calculateGroupDrop`.
fn calculate_group_drop(
    s: &DropSettings,
    items: &ItemTemplates,
    drop_item: &DropEntry,
    victim: &DropVictim,
    killer: &DropKiller,
    chance: f64,
    rng: &mut impl Rnd,
) -> Option<ItemHolder> {
    // Calculate if item will drop.
    if rng.next_double() * 100.0 < chance {
        // Amount is calculated after chance returned success.
        let rate = rate_amount(s, items, drop_item.item_id, victim, killer);
        return Some(ItemHolder {
            id: drop_item.item_id,
            count: amount(rng, drop_item, rate),
        });
    }
    None
}

/// Java `calculateUngroupedDrops` for a list that is already filtered and sorted.
fn calculate_ungrouped_drops(
    s: &DropSettings,
    items: &ItemTemplates,
    list: &[DropEntry],
    victim: &DropVictim,
    killer: &DropKiller,
    rng: &mut impl Rnd,
) -> Vec<ItemHolder> {
    let gaps = Gaps::of(s, victim, killer);
    let mut counter = if victim.raid {
        s.drop_max_occurrences_raidboss
    } else {
        s.drop_max_occurrences_normal
    };
    let mut calculated: Option<Vec<Rolled>> = None;
    let mut random: Option<Vec<usize>> = None;
    let mut cached: Option<Rolled> = None;
    let mut seq = 0usize;
    if counter > 0 {
        for drop_item in list {
            // Check if maximum drop occurrences have been reached. Items that have 100% drop chance
            // without server rate multipliers drop normally.
            if counter == 0
                && drop_item.chance < 100.0
                && let (Some(random), Some(calculated)) = (random.as_mut(), calculated.as_mut())
            {
                // Remove highest chance item (temporarily if no other item replaces it).
                // NOTE(java-quirk): `randomDrops.remove(0)` throws on an empty list; the counter
                // only reaches 0 after something was added, so the list is not empty here.
                if !random.is_empty() {
                    let first = random.remove(0);
                    if let Some(at) = calculated.iter().position(|r| r.seq == first) {
                        cached = Some(calculated.remove(at));
                    }
                }
                counter = 1;
            }

            // Check level gap that may prevent to drop item.
            if rng.next_double() * 100.0 > gaps.of_item(drop_item.item_id) {
                continue;
            }

            // Calculate chances.
            let Some(holder) = calculate_ungrouped_drop(s, items, drop_item, victim, killer, rng) else {
                continue;
            };
            let rolled = Rolled { seq, holder };
            seq += 1;

            // Finally.
            let random = random.get_or_insert_with(Vec::new);
            let calculated = calculated.get_or_insert_with(Vec::new);
            let counts = match s.rate_drop_chance_by_id.get(&drop_item.item_id) {
                Some(by_id) => drop_item.chance * f64::from(*by_id) < 100.0,
                None => drop_item.chance < 100.0,
            };
            if counts {
                counter -= 1;
                random.push(rolled.seq);
            }
            calculated.push(rolled);
        }
    }

    // Add temporarily removed item when not replaced.
    if counter > 0
        && let (Some(cached), Some(calculated)) = (cached, calculated.as_mut())
    {
        calculated.push(cached);
    }
    // Champion extra drop: `ChampionMonsters.ini` is off.
    calculated
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.holder)
        .collect()
}

/// Java `calculateUngroupedDrop`.
fn calculate_ungrouped_drop(
    s: &DropSettings,
    items: &ItemTemplates,
    drop_item: &DropEntry,
    victim: &DropVictim,
    killer: &DropKiller,
    rng: &mut impl Rnd,
) -> Option<ItemHolder> {
    match drop_item.drop_type {
        DropType::Drop => {
            let item_id = drop_item.item_id;
            let rate_chance = rate_chance(s, items, item_id, victim, killer);
            // Calculate if item will drop.
            if rng.next_double() * 100.0 < drop_item.chance * rate_chance {
                // Amount is calculated after chance returned success.
                let rate = rate_amount(s, items, item_id, victim, killer);
                return Some(ItemHolder {
                    id: item_id,
                    count: amount(rng, drop_item, rate),
                });
            }
            None
        }
        DropType::Spoil => {
            // Chance; bonus spoil rate effect.
            let rate_chance = f64::from(s.rate_spoil_drop_chance_multiplier) * killer.bonus_spoil_rate;
            // Calculate if item will be rewarded.
            if rng.next_double() * 100.0 < drop_item.chance * rate_chance {
                // Amount is calculated after chance returned success.
                let rate = f64::from(s.rate_spoil_drop_amount_multiplier);
                return Some(ItemHolder {
                    id: drop_item.item_id,
                    count: amount(rng, drop_item, rate),
                });
            }
            None
        }
    }
}

impl World {
    /// The drop half of Java `Attackable.calculateRewards(lastAttacker)`: who the loot belongs to
    /// (the player who did the most damage within `AltPartyRange`, else the last attacker), then
    /// `doItemDrop`.
    ///
    /// Java returns early when the aggro list is empty, which skips the exp as well; callers that
    /// add the exp half must do the same (`aggro_list(npc).is_empty()`).
    // NOTE(java-quirk): the biggest damage dealer is the first one over the running maximum, in the
    // order Java's `ConcurrentHashMap` iterates the aggro list (an identity hash order). Here it
    // is the order the attackers hit first; only an exact tie can differ.
    pub(crate) fn drop_rewards(&mut self, dying: ObjectId, last_attacker: Option<ObjectId>) {
        if !self.is_attackable_npc(dying) {
            return;
        }
        let aggro = self.aggro_list(dying).to_vec();
        if aggro.is_empty() {
            return;
        }
        let Some(victim_pos) = self.position_of(dying) else {
            return;
        };
        let range = f64::from(self.settings.drops.alt_party_range);
        let mut max_dealer: Option<ObjectId> = None;
        let mut max_damage: i64 = 0;
        let mut rewards: Vec<(ObjectId, i64)> = Vec::new();
        for info in &aggro {
            // Get the Creature corresponding to this attacker (`asPlayer()`: a summon's owner).
            let Some(attacker) = self.acting_player(info.attacker) else {
                continue;
            };
            // Get damages done by this attacker.
            let damage = info.damage;
            // Prevent unwanted behavior.
            if damage > 1 {
                // Check if damage dealer isn't too far from this (killed monster).
                let Some(pos) = self.position_of(attacker) else {
                    continue;
                };
                if distance_3d(victim_pos, pos) > range {
                    continue;
                }
                // Calculate real damages (summoners should get own damage plus summon's damage).
                let total = match rewards.iter_mut().find(|(a, _)| *a == attacker) {
                    Some(entry) => {
                        entry.1 = entry.1.saturating_add(damage);
                        entry.1
                    }
                    None => {
                        rewards.push((attacker, damage));
                        damage
                    }
                };
                if total > max_damage {
                    max_dealer = Some(attacker);
                    max_damage = total;
                }
            }
        }
        // The party that did the most damage: every attacker's party once, with what its members
        // did in all (the attackers out of range count here, too).
        let mut damaging_parties: Vec<(crate::party::PartyId, i64)> = Vec::new();
        for info in &aggro {
            let Some(party) = self.party_of(info.attacker) else {
                continue;
            };
            let mut total_member_damage: i64 = 0;
            for member in self.party_members(party) {
                if let Some(member_aggro) = aggro.iter().find(|a| a.attacker == member)
                    && member_aggro.damage > 1
                {
                    total_member_damage = total_member_damage.wrapping_add(member_aggro.damage);
                }
            }
            match damaging_parties.iter_mut().find(|(p, _)| *p == party) {
                Some(entry) => entry.1 = total_member_damage,
                None => damaging_parties.push((party, total_member_damage)),
            }
        }
        // `sort(comparingLong(c -> -c.damage))` is stable: equal parties stay in aggro order.
        damaging_parties.sort_by_key(|(_, damage)| std::cmp::Reverse(*damage));
        let most_damage_party = damaging_parties.first().copied();

        // Manage base, quests and sweep drops of the Attackable.
        if let Some((party, damage)) = most_damage_party
            && damage > max_damage
        {
            // The party's members together beat the single best dealer: its leader gets the loot.
            let leader = self.party_leader(party);
            self.do_item_drop(dying, leader);
            self.do_event_drop(leader, dying);
            return;
        }
        let dealer = match max_dealer {
            // `maxDealer.isOnline()`.
            Some(p) if self.players.contains_key(&p) => Some(p),
            _ => last_attacker,
        };
        self.do_item_drop(dying, dealer);
        // NOTE(java-quirk): the event drops go to the last attacker even when the damage dealer
        // got the normal drops.
        self.do_event_drop(last_attacker, dying);
    }

    /// Java `Player.onDieDropItem(killer)`: a player with karma, or one killed by a monster, may
    /// drop items on the ground.
    pub(crate) fn player_on_die_drop_item(&mut self, dying: ObjectId, killer: ObjectId) {
        // `isOnEvent()`: no events.
        let killer_is_player = self.players.contains_key(&killer);
        // A player without karma killed by a player whose clan declared war on its clan drops
        // nothing. NOTE(java-quirk): only this direction counts (the other one is commented out
        // in Java).
        let own_clan = self.clan_id_of(dying);
        if killer_is_player
            && self.players.get(&dying).is_some_and(|p| p.data.karma <= 0)
            && self.clan(own_clan).is_some()
            && self
                .clan_of(killer)
                .is_some_and(|pk_clan| pk_clan.is_at_war_with(own_clan))
        {
            return;
        }
        let s = self.settings.drops.clone();
        let inside_pvp = self.is_inside_zone(dying, crate::zones::ZoneId::Pvp);
        if !((!inside_pvp || !killer_is_player) && (!self.is_gm(dying) || s.karma_drop_gm)) {
            return;
        }
        let Some((karma, pk_kills, level)) = self
            .players
            .get(&dying)
            .map(|p| (p.data.karma, p.data.pk_kills, p.core.level))
        else {
            return;
        };
        let killer_is_npc = self.npcs.contains_key(&killer);
        let (mut drop_equip, mut drop_equip_weapon, mut drop_item, mut drop_limit, mut drop_percent) =
            (0, 0, 0, 0, 0);
        let mut is_karma_drop = false;
        if karma > 0 && pk_kills >= s.karma_pk_limit {
            is_karma_drop = true;
            drop_percent = s.karma_rate_drop;
            drop_equip = s.karma_rate_drop_equip;
            drop_equip_weapon = s.karma_rate_drop_equip_weapon;
            drop_item = s.karma_rate_drop_item;
            drop_limit = s.karma_drop_limit;
        // `!isFestivalParticipant()`: the festival is not modelled.
        } else if killer_is_npc && level > 4 {
            drop_percent = s.player_rate_drop;
            drop_equip = s.player_rate_drop_equip;
            drop_equip_weapon = s.player_rate_drop_equip_weapon;
            drop_item = s.player_rate_drop_item;
            drop_limit = s.player_drop_limit;
        }

        if !(drop_percent > 0 && self.rng().below(100) < drop_percent) {
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        let mut drop_count = 0;
        let mut item_drop_percent;
        // Java walks the live item container; the items are taken from a snapshot here, in the
        // order of the item list.
        let snapshot: Vec<crate::player::ItemData> = self
            .players
            .get(&dying)
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| i.loc == "INVENTORY" || i.loc == "PAPERDOLL")
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for item_drop in snapshot {
            let Some(template) = templates.get(item_drop.item_id) else {
                continue;
            };
            // Don't drop: shadow items, time limited items, items that cannot be dropped, adena,
            // quest items, the control item of the active pet (no pets), the non droppable items.
            // The pet item list needs pets (M5).
            if item_drop.mana_left >= 0
                || item_drop.time > 0
                || !template.dropable
                || item_drop.item_id == ADENA_ID
                || template.type2 == l2_data::item::TYPE2_QUEST
                || s.karma_list_nondroppable_items.contains(&item_drop.item_id)
            {
                continue;
            }
            let equipped = item_drop.loc == "PAPERDOLL";
            if equipped {
                // Set proper chance according to Item type of equipped Item.
                item_drop_percent = if template.type2 == l2_data::item::TYPE2_WEAPON {
                    drop_equip_weapon
                } else {
                    drop_equip
                };
            } else {
                // Item in inventory.
                item_drop_percent = drop_item;
            }

            // NOTE: Each time an item is dropped, the chance of another item being dropped gets
            // lesser (dropCount * 2) -- a comment of Java's that the code does not follow.
            if self.rng().below(100) < item_drop_percent {
                if equipped {
                    let slot = usize::try_from(item_drop.loc_data).unwrap_or(0);
                    let mut changed = Vec::new();
                    self.set_paperdoll_item(dying, slot, None, &mut changed);
                    self.refresh_expertise(dying);
                }
                self.player_drop_item_object(dying, item_drop.object_id, false, true);
                self.send_item_list(dying, false);

                let name = self
                    .players
                    .get(&dying)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                if is_karma_drop {
                    warn!(
                        "{name} has karma and dropped id = {}, count = {}",
                        item_drop.item_id, item_drop.count
                    );
                } else {
                    warn!(
                        "{name} dropped id = {}, count = {}",
                        item_drop.item_id, item_drop.count
                    );
                }

                drop_count += 1;
                if drop_count >= drop_limit {
                    break;
                }
            }
        }
    }

    /// The template whose drop list `doItemDrop` rolls: the NPC's own, except for a `Chest` nobody
    /// unlocked (`Chest.doItemDrop`), which drops as the template its id maps to.
    // NOTE: Java passes `NpcData.getTemplate(id)` on, which is `null` for an id without a
    // template (and then fails); the chest's own template is used in that case.
    fn chest_drop_template_id(&self, dying: ObjectId, npc_id: i32) -> i32 {
        let is_chest = self
            .npc_world
            .template(npc_id)
            .is_some_and(|t| t.is_type("Chest"));
        if !is_chest || self.npcs.get(&dying).is_some_and(|n| n.special_drop) {
            return npc_id;
        }
        let mapped = locked_chest_drop_id(npc_id);
        if self.npc_world.template(mapped).is_some() {
            mapped
        } else {
            npc_id
        }
    }

    /// Java `Attackable.doItemDrop(mainDamageDealer)`.
    pub fn do_item_drop(&mut self, dying: ObjectId, main_damage_dealer: Option<ObjectId>) {
        // `mainDamageDealer.asPlayer()`: a summon's owner.
        let Some(dealer) = main_damage_dealer.and_then(|d| self.acting_player(d)) else {
            // Don't drop anything if the last attacker or owner isn't Player (a fake player may,
            // when `FakePlayerCanDropItems` is on: fake players are not ported).
            return;
        };
        // `CursedWeaponsManager.checkDrop(this, player)`: cursed weapons are M5.

        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some((npc_id, victim_level, raid, raid_minion)) = self
            .npcs
            .get(&dying)
            .map(|n| (n.npc_id, n.core.level, n.class.is_raid(), n.leader.is_some()))
        else {
            return;
        };
        let Some(npc_name) = self.npc_world.template(npc_id).map(|t| t.name.clone()) else {
            return;
        };
        let Some(player_level) = self.players.get(&dealer).map(|p| p.core.level) else {
            return;
        };
        let killer = match self.stats(dealer) {
            Some(view) => DropKiller {
                level: player_level,
                bonus_drop_rate: 1.0 + view.calc(Stat::BonusDropRate, 0.0) / 100.0,
                bonus_drop_amount: 1.0 + view.calc(Stat::BonusDropAmount, 0.0) / 100.0,
                bonus_drop_adena: 1.0 + view.calc(Stat::BonusDropAdena, 0.0) / 100.0,
                bonus_spoil_rate: 1.0 + view.calc(Stat::BonusSpoilRate, 0.0) / 100.0,
            },
            None => DropKiller::plain(player_level),
        };
        let victim = DropVictim {
            level: victim_level,
            raid,
            champion: false,
            seeded: self.is_seeded(dying),
        };
        let settings = self.settings.drops.clone();
        let drop_template_id = self.chest_drop_template_id(dying, npc_id);
        // `if (isSpoiled()) _sweepItems.set(calculateDrops(SPOIL, this, player))`: the spoil
        // list rolls before the DROP list, so a seeded run draws in Java's order.
        if self.is_spoiled(dying) {
            let sweep_items = self.with_rng(|world, rng| {
                world.npc_world.template(npc_id).map(|npc_template| {
                    calculate_drops(
                        &settings,
                        &templates,
                        npc_template,
                        DropType::Spoil,
                        &victim,
                        &killer,
                        rng,
                    )
                })
            });
            // An empty list is Java's `null`: nothing to sweep, no blue glow.
            self.set_sweep_items(dying, sweep_items.filter(|items| !items.is_empty()));
        }
        let death_items = self.with_rng(|world, rng| {
            world.npc_world.template(drop_template_id).map(|npc_template| {
                calculate_drops(
                    &settings,
                    &templates,
                    npc_template,
                    DropType::Drop,
                    &victim,
                    &killer,
                    rng,
                )
            })
        });
        let Some(death_items) = death_items else {
            return;
        };
        let ground = self.settings.ground.clone();
        for drop in death_items {
            let Some(item) = templates.get(drop.id) else {
                warn!("{dying}: drop of unknown item {}", drop.id);
                continue;
            };
            // Check if the autoLoot mode is active.
            // `isFlying()`: a flying monster cannot drop things on the ground; none is modelled.
            let auto_loot = ground.auto_loot_item_ids.contains(&item.id)
                || (!item.ex_immediate_effect
                    && ((!raid && ground.auto_loot) || (raid && ground.auto_loot_raids)))
                || (item.ex_immediate_effect && ground.auto_loot_herbs);
            if auto_loot {
                // Give the item(s) to the Player that has killed the Attackable.
                self.do_auto_loot(dealer, dying, drop.id, drop.count);
            } else {
                // Drop the item on the ground.
                self.npc_drop_item(dying, Some(dealer), drop.id, drop.count);
            }

            // Broadcast message if RaidBoss was defeated.
            if raid && !raid_minion && drop.count > 0 {
                let message = SystemMessage::new(SystemMessageId::C1DiedAndDroppedS3S2)
                    .with(SystemMessageParam::Text(npc_name.clone()))
                    .with(SystemMessageParam::ItemName(drop.id))
                    .with(SystemMessageParam::LongNumber(drop.count));
                self.broadcast_packet(dying, &message);
            }
        }
    }
}

// ------------------------------------------------------------------ spoil and sweep

/// Java `Attackable._spoilerObjectId` and `_sweepItems` of one monster.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SpoilState {
    /// `_spoilerObjectId`: who cast Spoil (`0` in Java is `None`).
    pub spoiler: Option<ObjectId>,
    /// `_sweepItems`: the SPOIL list rolled when the monster died (`null` is `None`).
    pub sweep: Option<Vec<ItemHolder>>,
}

/// What `ConditionPlayerCanSweep.testImpl` found, and the messages it sent to the sweeper, in
/// Java's order. The condition code is read-only, so the caller sends the messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SweepCheck {
    pub can_sweep: bool,
    pub messages: Vec<SystemMessageId>,
}

impl World {
    /// Java `Attackable.isSpoiled()`: `_spoilerObjectId != 0`.
    pub fn is_spoiled(&self, id: ObjectId) -> bool {
        self.death.spoil.get(&id).is_some_and(|s| s.spoiler.is_some())
    }

    /// Java `Attackable.getSpoilerObjectId()` (`0` is `None`).
    pub fn spoiler_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.death.spoil.get(&id).and_then(|s| s.spoiler)
    }

    /// Java `Attackable.setSpoilerObjectId(id)`.
    pub(crate) fn set_spoiler(&mut self, id: ObjectId, spoiler: Option<ObjectId>) {
        self.death.spoil.entry(id).or_default().spoiler = spoiler;
        self.drop_empty_spoil_state(id);
    }

    /// `_sweepItems.set(items)`.
    pub(crate) fn set_sweep_items(&mut self, id: ObjectId, items: Option<Vec<ItemHolder>>) {
        self.death.spoil.entry(id).or_default().sweep = items;
        self.drop_empty_spoil_state(id);
    }

    fn drop_empty_spoil_state(&mut self, id: ObjectId) {
        if self
            .death
            .spoil
            .get(&id)
            .is_some_and(|s| *s == SpoilState::default())
        {
            self.death.spoil.remove(&id);
        }
    }

    /// Java `Attackable.isSweepActive()`: `_sweepItems.get() != null` (the blue glow of `Die`).
    pub fn is_sweep_active(&self, id: ObjectId) -> bool {
        self.death.spoil.get(&id).is_some_and(|s| s.sweep.is_some())
    }

    /// Java `Attackable.takeSweep()`: `_sweepItems.getAndSet(null)`.
    pub(crate) fn take_sweep(&mut self, id: ObjectId) -> Option<Vec<ItemHolder>> {
        let items = self.death.spoil.get_mut(&id)?.sweep.take();
        self.drop_empty_spoil_state(id);
        items
    }

    /// Java `Attackable.getSpoilLootItems()`, as the holders (the checks look their templates
    /// up).
    pub fn spoil_loot_items(&self, id: ObjectId) -> Vec<ItemHolder> {
        self.death
            .spoil
            .get(&id)
            .and_then(|s| s.sweep.clone())
            .unwrap_or_default()
    }

    /// Java `Attackable.checkSpoilOwner(sweeper, false)`: the sweeper cast the Spoil or is in
    /// the party of whoever did. The caller sends `THERE_ARE_NO_PRIORITY_RIGHTS_ON_A_SWEEPER`.
    pub(crate) fn check_spoil_owner(&self, id: ObjectId, sweeper: ObjectId) -> bool {
        let spoiler = self.death.spoil.get(&id).and_then(|s| s.spoiler);
        // `sweeper.getObjectId() == _spoilerObjectId || sweeper.isInLooterParty(_spoilerObjectId)`.
        spoiler == Some(sweeper) || spoiler.is_some_and(|s| self.is_in_looter_party(sweeper, s))
    }

    /// Java `Attackable.isOldCorpse(attacker, CorpseConsumeSkillAllowedTimeBeforeDecay, false)`:
    /// dead, and less than the allowed time left before it decays. The caller sends
    /// `THE_CORPSE_IS_TOO_OLD_THE_SKILL_CANNOT_BE_USED`.
    pub(crate) fn is_old_corpse(&self, id: ObjectId) -> bool {
        let allowed = i64::from(self.settings.death.corpse_consume_skill_allowed_time_before_decay);
        self.is_dead(id) && self.decay_remaining_ms(id) < allowed
    }

    /// Java `PlayerInventory.checkInventorySlotsAndWeight(itemList, false, false)`: the items fit
    /// the slots and the weight limit.
    // NOTE(java-quirk): the weight is each template's weight once, not times the count.
    pub(crate) fn check_inventory_slots_and_weight(&self, player: ObjectId, items: &[ItemHolder]) -> bool {
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let mut loot_weight: i32 = 0;
        let mut required_slots: i64 = 0;
        for holder in items {
            // Java would throw on an unknown template; the roll only gives known items.
            let Some(item) = templates.get(holder.id) else {
                continue;
            };
            // If the item is not stackable or is stackable and not present in inventory, will
            // need a slot.
            if !item.stackable || self.inventory_item_count(player, item.id) <= 0 {
                required_slots += 1;
            }
            loot_weight = loot_weight.wrapping_add(item.weight);
        }
        self.validate_capacity(player, required_slots, false)
            && self.validate_weight(player, i64::from(loot_weight))
    }

    /// Java `ConditionPlayerCanSweep.testImpl` for `sweeper` and `skill`: the targets of the
    /// skill are looked at one by one and the last dead one decides (a quirk of Java's loop).
    /// Pure: the messages Java sends from the condition are returned.
    pub(crate) fn can_sweep_check(&self, sweeper: ObjectId, skill: &l2_data::SkillTemplate) -> SweepCheck {
        let mut check = SweepCheck::default();
        if !self.is_player(sweeper) {
            return check;
        }
        // `skill.getTargetList(sweeper)`: the single-target handlers of the sweep skills draw
        // nothing, so a scratch RNG stands for the world's.
        let target = self.cast_selected_target(sweeper);
        let ctx = self.cast_target_context(sweeper);
        let mut scratch = l2_util::WorldRng::from_seed(0);
        let result = self.skill_target_list_with(&mut scratch, skill, sweeper, false, target, &ctx);
        for message in &result.messages {
            if let crate::skills::TargetMessage::System(id) = message {
                check.messages.push(*id);
            }
        }
        for &target in &result.targets {
            if !self.is_attackable_npc(target) || !self.is_dead(target) {
                continue;
            }
            if self.is_spoiled(target) {
                let owner = self.check_spoil_owner(target, sweeper);
                if !owner {
                    check
                        .messages
                        .push(SystemMessageId::ThereAreNoPriorityRightsOnASweeper);
                }
                // `&=` does not short-circuit: every check runs and may talk.
                let old = self.is_old_corpse(target);
                if old {
                    check
                        .messages
                        .push(SystemMessageId::TheCorpseIsTooOldTheSkillCannotBeUsed);
                }
                let fits = self.check_inventory_slots_and_weight(sweeper, &self.spoil_loot_items(target));
                if !fits {
                    check.messages.push(SystemMessageId::YourInventoryIsFull);
                    check.messages.push(
                        SystemMessageId::WeightAndVolumeLimitHaveBeenExceededThatSkillIsCurrentlyUnavailable,
                    );
                }
                check.can_sweep = owner && !old && fits;
            } else {
                check
                    .messages
                    .push(SystemMessageId::SweeperFailedTargetNotSpoiled);
            }
        }
        check
    }
}

#[cfg(test)]
mod chest_tests {
    use super::locked_chest_drop_id;

    /// The id table of `Chest.doItemDrop`.
    #[test]
    fn locked_chests_drop_as_the_template_their_id_maps_to() {
        assert_eq!(locked_chest_drop_id(18265), 21801);
        assert_eq!(locked_chest_drop_id(18286), 21822);
        assert_eq!(locked_chest_drop_id(18287), 21671);
        assert_eq!(locked_chest_drop_id(18288), 21671);
        assert_eq!(locked_chest_drop_id(18289), 21694);
        assert_eq!(locked_chest_drop_id(18292), 21717);
        assert_eq!(locked_chest_drop_id(18294), 21740);
        assert_eq!(locked_chest_drop_id(18296), 21763);
        assert_eq!(locked_chest_drop_id(18298), 21786);
        // Not a chest of the table: itself.
        assert_eq!(locked_chest_drop_id(18264), 18264);
        assert_eq!(locked_chest_drop_id(18299), 18299);
        assert_eq!(locked_chest_drop_id(9003), 9003);
    }
}
