//! The soul crystal leveling table: which crystal becomes which, and which monsters level them.
//!
//! Java: `gameserver/data/xml/LevelUpCrystalData.java`, `data/holders/SoulCrystal.java`,
//! `data/holders/LevelingSoulCrystalInfo.java`, `data/enums/AbsorbCrystalType.java`.
//! Data: `data/LevelUpCrystalData.xml` (`xsd/LevelUpCrystalData.xsd`).
//!
//! Read by quest 350 (`Q00350_EnhanceYourWeapon`): a kill of a listed monster levels the
//! crystal of the killer (or of party members) from `level` to `level + 1`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::info;

use crate::loader::{DataError, load_file};

/// Java `AbsorbCrystalType`: who of the killer's party may level a crystal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbsorbCrystalType {
    LastHit,
    FullParty,
    PartyOneRandom,
    PartyRandom,
}

impl AbsorbCrystalType {
    /// Java `Enum.valueOf(AbsorbCrystalType.class, name)`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "LAST_HIT" => Self::LastHit,
            "FULL_PARTY" => Self::FullParty,
            "PARTY_ONE_RANDOM" => Self::PartyOneRandom,
            "PARTY_RANDOM" => Self::PartyRandom,
            _ => return None,
        })
    }
}

/// Java `SoulCrystal`: a crystal item of `level` that becomes `leveled_item_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulCrystal {
    pub level: i32,
    pub item_id: i32,
    pub leveled_item_id: i32,
}

/// Java `LevelingSoulCrystalInfo`: how a monster levels a crystal of one level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelingSoulCrystalInfo {
    pub absorb_type: AbsorbCrystalType,
    /// `skill="true"`: the crystal must have been used on the monster (skill 2096) before the kill.
    pub skill_needed: bool,
    /// Percent, with `SoulCrystalChanceMultiplier` applied and capped at 100 (Java does it at load).
    pub chance: i32,
}

/// The whole table. Java `LevelUpCrystalData`.
#[derive(Debug, Clone, Default)]
pub struct LevelUpCrystalData {
    soul_crystals: BTreeMap<i32, SoulCrystal>,
    /// NPC id -> crystal level -> info.
    npc_leveling_info: BTreeMap<i32, BTreeMap<i32, LevelingSoulCrystalInfo>>,
}

impl LevelUpCrystalData {
    /// Loads `<root>/data/LevelUpCrystalData.xml`. `chance_multiplier` is `Player.ini`
    /// `SoulCrystalChanceMultiplier` (default 1).
    pub fn load(root: &Path, chance_multiplier: f64) -> Self {
        let mut data = Self::default();
        load_file(&root.join("data/LevelUpCrystalData.xml"), |doc: RawList| {
            data.add(doc, chance_multiplier);
            Ok(())
        });
        info!("Soul crystal data : Loaded {}", data.soul_crystals.len());
        info!(
            "Npc leveling crystal data : Loaded {}",
            data.npc_leveling_info.len()
        );
        data
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, chance_multiplier: f64) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut data = Self::default();
        data.add(doc, chance_multiplier);
        Ok(data)
    }

    fn add(&mut self, doc: RawList, chance_multiplier: f64) {
        for block in doc.crystals {
            for item in block.items {
                let Some(item_id) = item.item_id.as_deref().and_then(parse_int) else {
                    info!("Missing itemId in Crystal List, skipping!");
                    continue;
                };
                let Some(level) = item.level.as_deref().and_then(parse_int) else {
                    info!("Missing level in Crystal List item Id {item_id} skipping!");
                    continue;
                };
                let Some(leveled_item_id) = item.leveled_item_id.as_deref().and_then(parse_int) else {
                    info!("Missing leveledItemId in Crystal List item Id {item_id} skipping!");
                    continue;
                };
                self.soul_crystals.insert(
                    item_id,
                    SoulCrystal {
                        level,
                        item_id,
                        leveled_item_id,
                    },
                );
            }
        }
        for block in doc.npcs {
            for item in block.items {
                let Some(npc_id) = item.npc_id.as_deref().and_then(parse_int) else {
                    info!("Missing npc Id in NPC List, skipping!");
                    continue;
                };
                let mut temp = BTreeMap::new();
                for detail in item.details {
                    let mut absorb_type = AbsorbCrystalType::LastHit;
                    if let Some(name) = &detail.absorb_type {
                        // Java's `Enum.valueOf` throws on an unknown name and the rest of the file
                        // is lost; here only the detail is skipped.
                        let Some(t) = AbsorbCrystalType::from_name(name) else {
                            info!("Bad absorbType {name} in NPC List npc Id {npc_id} skipping!");
                            continue;
                        };
                        absorb_type = t;
                    }
                    let mut chance = detail.chance.as_deref().and_then(parse_int).unwrap_or(5);
                    // `Boolean.parseBoolean`.
                    let skill_needed = detail
                        .skill
                        .as_deref()
                        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
                    if detail.max_level.is_none() && detail.level_list.is_none() {
                        info!("Missing maxlevel/levelList in NPC List npc Id {npc_id} skipping!");
                        continue;
                    }
                    // `(int) (chance * SOUL_CRYSTAL_CHANCE_MULTIPLIER)`, then capped at 100.
                    chance = (f64::from(chance) * chance_multiplier) as i32;
                    chance = chance.min(100);
                    let info = LevelingSoulCrystalInfo {
                        absorb_type,
                        skill_needed,
                        chance,
                    };
                    if let Some(max_level) = &detail.max_level {
                        let max_level = parse_int(max_level).unwrap_or(-1);
                        for i in 0..=max_level {
                            temp.insert(i, info);
                        }
                    } else if let Some(list) = &detail.level_list {
                        for (i, token) in list.split(',').filter(|t| !t.trim().is_empty()).enumerate() {
                            let value = parse_int(token.trim()).unwrap_or_else(|| {
                                info!("Bad Level value!! npc Id {npc_id}, token {i}!");
                                0
                            });
                            temp.insert(value, info);
                        }
                    }
                }
                if temp.is_empty() {
                    info!("No leveling info for npc Id {npc_id} skipping!");
                    continue;
                }
                self.npc_leveling_info.insert(npc_id, temp);
            }
        }
    }

    /// Java `getSoulCrystal(itemId)`.
    pub fn soul_crystal(&self, item_id: i32) -> Option<&SoulCrystal> {
        self.soul_crystals.get(&item_id)
    }

    /// Java `getSoulCrystals().containsKey(itemId)`.
    pub fn is_soul_crystal(&self, item_id: i32) -> bool {
        self.soul_crystals.contains_key(&item_id)
    }

    /// Java `getNpcSoulInfo(npcId)`: crystal level -> info.
    pub fn npc_soul_info(&self, npc_id: i32) -> Option<&BTreeMap<i32, LevelingSoulCrystalInfo>> {
        self.npc_leveling_info.get(&npc_id)
    }

    /// Java `getNpcsSoulInfo().keySet()`, sorted.
    pub fn npc_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.npc_leveling_info.keys().copied()
    }

    /// Number of crystal items.
    pub fn soul_crystal_count(&self) -> usize {
        self.soul_crystals.len()
    }

    /// Number of NPCs with leveling info.
    pub fn npc_count(&self) -> usize {
        self.npc_leveling_info.len()
    }
}

/// Java `Integer.parseInt` / `Integer.decode` of a datapack value (decimal, `0x` hex, `#` hex).
fn parse_int(text: &str) -> Option<i32> {
    let t = text.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let value = if let Some(hex) = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .or_else(|| t.strip_prefix('#'))
    {
        i64::from_str_radix(hex, 16).ok()?
    } else if t.len() > 1 && t.starts_with('0') {
        i64::from_str_radix(&t[1..], 8).ok()?
    } else {
        t.parse::<i64>().ok()?
    };
    i32::try_from(if neg { -value } else { value }).ok()
}

// Raw XML shape (xsd/LevelUpCrystalData.xsd).
#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "crystal", default)]
    crystals: Vec<RawCrystalBlock>,
    #[serde(rename = "npc", default)]
    npcs: Vec<RawNpcBlock>,
}

#[derive(Deserialize)]
struct RawCrystalBlock {
    #[serde(rename = "item", default)]
    items: Vec<RawCrystal>,
}

#[derive(Deserialize)]
struct RawCrystal {
    #[serde(rename = "@itemId")]
    item_id: Option<String>,
    #[serde(rename = "@level")]
    level: Option<String>,
    #[serde(rename = "@leveledItemId")]
    leveled_item_id: Option<String>,
}

#[derive(Deserialize)]
struct RawNpcBlock {
    #[serde(rename = "item", default)]
    items: Vec<RawNpc>,
}

#[derive(Deserialize)]
struct RawNpc {
    #[serde(rename = "@npcId")]
    npc_id: Option<String>,
    #[serde(rename = "detail", default)]
    details: Vec<RawDetail>,
}

#[derive(Deserialize)]
struct RawDetail {
    #[serde(rename = "@absorbType")]
    absorb_type: Option<String>,
    #[serde(rename = "@chance")]
    chance: Option<String>,
    #[serde(rename = "@skill")]
    skill: Option<String>,
    #[serde(rename = "@maxLevel")]
    max_level: Option<String>,
    #[serde(rename = "@levelList")]
    level_list: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<crystal>
		<item itemId="4629" level="0" leveledItemId="4630" />
		<item itemId="4630" level="1" leveledItemId="4631" />
		<item itemId="9999" level="1" />
	</crystal>
	<npc>
		<item npcId="20583"> <!-- Timak Orc -->
			<detail chance="9" skill="true" maxLevel="1" />
		</item>
		<item npcId="25163">
			<detail chance="60" absorbType="FULL_PARTY" levelList="12, 13" />
			<detail chance="50" absorbType="PARTY_ONE_RANDOM" levelList="13, 0x0E" />
		</item>
		<item npcId="1">
			<detail chance="10" />
		</item>
		<item npcId="2">
			<detail maxLevel="0" />
		</item>
	</npc>
</list>"#;

    #[test]
    fn parses_crystals_and_npc_levels_like_java() {
        let data = LevelUpCrystalData::from_xml(XML, 1.0).unwrap();
        assert_eq!(data.soul_crystal_count(), 2);
        assert_eq!(
            data.soul_crystal(4629),
            Some(&SoulCrystal {
                level: 0,
                item_id: 4629,
                leveled_item_id: 4630
            })
        );
        assert!(!data.is_soul_crystal(9999));

        let timak = data.npc_soul_info(20583).unwrap();
        assert_eq!(timak.keys().copied().collect::<Vec<_>>(), [0, 1]);
        assert_eq!(
            timak[&0],
            LevelingSoulCrystalInfo {
                absorb_type: AbsorbCrystalType::LastHit,
                skill_needed: true,
                chance: 9
            }
        );
        // A later detail overwrites the levels it lists (`temp.put`); `Integer.decode` reads hex.
        let boss = data.npc_soul_info(25163).unwrap();
        assert_eq!(boss[&12].absorb_type, AbsorbCrystalType::FullParty);
        assert_eq!(boss[&13].absorb_type, AbsorbCrystalType::PartyOneRandom);
        assert_eq!(boss[&14].chance, 50);
        // A detail without maxLevel/levelList is skipped, and an NPC with nothing is dropped.
        assert!(data.npc_soul_info(1).is_none());
        // The default chance is 5.
        assert_eq!(data.npc_soul_info(2).unwrap()[&0].chance, 5);
        assert_eq!(data.npc_ids().collect::<Vec<_>>(), [2, 20583, 25163]);
    }

    #[test]
    fn the_chance_multiplier_is_applied_at_load_and_capped() {
        let data = LevelUpCrystalData::from_xml(XML, 1.5).unwrap();
        // (int) (9 * 1.5) = 13; (int) (60 * 1.5) = 90 -> 100 cap only above.
        assert_eq!(data.npc_soul_info(20583).unwrap()[&0].chance, 13);
        assert_eq!(data.npc_soul_info(25163).unwrap()[&12].chance, 90);
        let data = LevelUpCrystalData::from_xml(XML, 3.0).unwrap();
        assert_eq!(data.npc_soul_info(25163).unwrap()[&12].chance, 100);
    }

    #[test]
    fn the_shipped_file_loads() {
        let root = Path::new(r"C:\l2-rust\l2jmobius\dist\game");
        let path = root.join("data/LevelUpCrystalData.xml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let data = LevelUpCrystalData::from_xml(&text, 1.0).unwrap();
        assert_eq!(data.soul_crystal_count(), text.matches("leveledItemId=").count());
        assert_eq!(data.npc_count(), text.matches("npcId=").count());
        assert_eq!(data.soul_crystal(15541).map(|c| c.leveled_item_id), Some(15826));
        let timak = data.npc_soul_info(20583).unwrap();
        assert_eq!(timak[&1].chance, 9);
        assert!(timak[&1].skill_needed);
    }
}
