#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The generated list, set and map fields (M9-L03) read from the `.ini` files of the repo's own
//! `dist/` snapshot. Skipped when that folder does not exist.

use std::path::PathBuf;

use l2_config::game::{
    BossAnnouncementsConfig, ChampionMonstersConfig, CommunityBoardConfig, NpcConfig, RandomSpawnsConfig,
    RebirthConfig, TransmogConfig,
};
use l2_config::{IniFile, ItemHolder};

fn load(file: &str) -> Option<IniFile> {
    let dir = std::env::var_os("L2JMOBIUS_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist")));
    let path = dir.join("game/config").join(file);
    if !path.is_file() {
        eprintln!("skipped: {} does not exist", path.display());
        return None;
    }
    Some(IniFile::load(path).unwrap())
}

/// `Set<Integer>`: `CommunityAvailableBuffs` and the multi-line `MobsSpawnNotRandom`.
#[test]
fn int_sets() {
    if let Some(ini) = load(CommunityBoardConfig::FILE) {
        let c = CommunityBoardConfig::from_ini(&ini);
        assert_eq!(c.community_available_buffs.len(), 20);
        assert!(c.community_available_buffs.contains(&1077) && c.community_available_buffs.contains(&1268));
    }
    if let Some(ini) = load(RandomSpawnsConfig::FILE) {
        let r = RandomSpawnsConfig::from_ini(&ini);
        // The value continues over many lines (`\` at the end of a line).
        assert!(r.mobs_list_not_random.len() > 100, "{}", r.mobs_list_not_random.len());
        assert!(r.mobs_list_not_random.contains(&18812) && r.mobs_list_not_random.contains(&22136));
    }
    // Empty values.
    for file in [BossAnnouncementsConfig::FILE, TransmogConfig::FILE] {
        if let Some(ini) = load(file) {
            assert!(
                BossAnnouncementsConfig::from_ini(&ini)
                    .raidbosses_excluded_from_spawn_announcements
                    .is_empty()
            );
            assert!(TransmogConfig::from_ini(&ini).transmog_banned_item_ids.is_empty());
        }
    }
}

/// `List<Integer>` and `Map<Integer, Integer>` of `NPC.ini`.
#[test]
fn int_list_and_int_map() {
    let Some(ini) = load(NpcConfig::FILE) else {
        return;
    };
    let npc = NpcConfig::from_ini(&ini);
    assert_eq!(npc.list_pet_rent_npc, [30827]);
    assert_eq!(npc.minions_respawn_time.len(), 23);
    assert_eq!(npc.minions_respawn_time.get(&22450), Some(&30));
    assert_eq!(npc.minions_respawn_time.get(&25545), Some(&0));
}

/// `List<ItemHolder>`.
#[test]
fn item_holders() {
    let Some(ini) = load(ChampionMonstersConfig::FILE) else {
        return;
    };
    assert_eq!(
        ChampionMonstersConfig::from_ini(&ini).champion_reward_items,
        [ItemHolder { id: 6393, count: 1 }]
    );
}

/// `Map<String, Location>`: the teleport list (a multi-line value).
#[test]
fn locations() {
    let Some(ini) = load(CommunityBoardConfig::FILE) else {
        return;
    };
    let teleports = CommunityBoardConfig::from_ini(&ini).community_available_teleports;
    assert_eq!(teleports.len(), 17);
    let giran = teleports["Giran"];
    assert_eq!((giran.x, giran.y, giran.z, giran.heading), (82698, 148638, -3473, 0));
    assert_eq!(teleports["Kamael"].z, 368);
}

/// `List<String>` split at `;`.
#[test]
fn string_lists() {
    let Some(ini) = load(RebirthConfig::FILE) else {
        return;
    };
    let r = RebirthConfig::from_ini(&ini);
    assert_eq!(r.rebirth_mage_skills.len(), 6);
    assert_eq!(r.rebirth_mage_skills[0], "1059,1,6");
    assert_eq!(r.rebirth_fighter_skills.len(), 6);
    assert!(r.rebirth_skill_delete_whitelist.is_empty());
}

/// Every key missing: the Java defaults.
#[test]
fn defaults() {
    let empty = IniFile::empty("none.ini");
    assert_eq!(
        ChampionMonstersConfig::from_ini(&empty).champion_reward_items,
        [ItemHolder { id: 4356, count: 10 }]
    );
    assert_eq!(
        RandomSpawnsConfig::from_ini(&empty).mobs_list_not_random.into_iter().collect::<Vec<_>>(),
        [18812, 18813, 18814, 22138]
    );
    assert_eq!(NpcConfig::from_ini(&empty).list_pet_rent_npc, [30827]);
    assert!(NpcConfig::from_ini(&empty).minions_respawn_time.is_empty());
}
