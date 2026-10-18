#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java NPC files (124 files in `data/stats/npcs`, 15 more in `custom`). Skipped
//! when the folder does not exist (CI).
//!
//! The folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! The expected numbers were counted from the XML with an independent script (every `<npc>`
//! element, values converted with Java's rules); Java's own count lines are
//! `NpcData: Loaded 10461 NPCs.` and, with `CustomNpcData`, `NpcData: Loaded 23 custom NPCs.`
//! (the two `fakePlayer` NPCs of the custom folder are skipped unless fake players are enabled).

use std::path::PathBuf;
use std::sync::OnceLock;

use l2_data::item::WeaponType;
use l2_data::npc::{AiType, DropType, NpcParam, NpcRace, NpcSex, NpcTemplate};
use l2_data::{NpcLoadOptions, NpcTemplates};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

/// The default load (no custom NPCs), shared by the tests: it takes seconds in a debug build.
fn main_npcs() -> Option<&'static NpcTemplates> {
    static NPCS: OnceLock<NpcTemplates> = OnceLock::new();
    let dir = game_dir()?;
    Some(NPCS.get_or_init(|| NpcTemplates::load(&dir, &NpcLoadOptions::default())))
}

/// The load with `CustomNpcData`, with fake players enabled or not (also shared).
fn custom_npcs(fake_players_enabled: bool) -> &'static NpcTemplates {
    static OFF: OnceLock<NpcTemplates> = OnceLock::new();
    static ON: OnceLock<NpcTemplates> = OnceLock::new();
    let cell = if fake_players_enabled { &ON } else { &OFF };
    cell.get_or_init(|| {
        let options = NpcLoadOptions {
            custom_npc_data: true,
            fake_players_enabled,
            ..NpcLoadOptions::default()
        };
        NpcTemplates::load(&game_dir().unwrap(), &options)
    })
}

#[test]
fn loads_every_npc_with_javas_counts() {
    let Some(npcs) = main_npcs() else { return };
    assert_eq!(npcs.len(), 10461);
    assert!(npcs.get(0).is_none());
    assert!(npcs.get(30006).is_some());
    // Custom NPCs are not loaded without CustomNpcData.
    assert!(npcs.get(50009).is_none());

    let with_custom = custom_npcs(false);
    assert_eq!(with_custom.len(), 10461 + 23);
    assert!(with_custom.get(80000).is_none(), "fake players are skipped");
    assert_eq!(custom_npcs(true).len(), 10461 + 25);
}

#[test]
fn aggregates_match_the_xml() {
    let Some(npcs) = main_npcs() else { return };
    let count = |f: &dyn Fn(&NpcTemplate) -> bool| npcs.iter().filter(|n| f(n)).count();
    let sum = |f: &dyn Fn(&NpcTemplate) -> i64| npcs.iter().map(f).sum::<i64>();

    // By type (names are Java instance classes).
    assert_eq!(count(&|n| n.npc_type == "Monster"), 3797);
    assert_eq!(count(&|n| n.npc_type == "Folk"), 2737);
    assert_eq!(count(&|n| n.npc_type == "Defender"), 633);
    assert_eq!(count(&|n| n.npc_type == "Pet"), 851);
    assert_eq!(count(&|n| n.npc_type == "RaidBoss"), 344);
    assert_eq!(count(&|n| n.npc_type == "GrandBoss"), 26);
    assert_eq!(count(&|n| n.npc_type == "Merchant"), 213);
    assert_eq!(count(&|n| n.npc_type == "Teleporter"), 103);
    assert_eq!(count(&|n| n.npc_type == "Guard"), 182);
    assert_eq!(count(&|n| n.npc_type == "L2Monster"), 1);

    // By race.
    assert_eq!(count(&|n| n.race == NpcRace::Human), 1188);
    assert_eq!(count(&|n| n.race == NpcRace::Elf), 220);
    assert_eq!(count(&|n| n.race == NpcRace::DarkElf), 197);
    assert_eq!(count(&|n| n.race == NpcRace::Construct), 1260);
    assert_eq!(count(&|n| n.race == NpcRace::CastleGuard), 597);
    assert_eq!(count(&|n| n.race == NpcRace::Undead), 939);
    assert_eq!(count(&|n| n.race == NpcRace::None), 10461 - 10183);

    // Flags.
    assert_eq!(count(&|n| !n.attackable), 3380);
    assert_eq!(count(&|n| !n.talkable), 54);
    assert_eq!(count(&|n| !n.targetable), 77);
    assert_eq!(count(&|n| !n.can_move), 1369);

    // Sums.
    assert_eq!(sum(&|n| i64::from(n.level)), 687_006);
    assert_eq!(sum(&|n| i64::from(n.base_p_atk)), 12_203_714);
    assert_eq!(sum(&|n| i64::from(n.base_p_def)), 10_405_770);
    // Hp is a Java float: values above 2^24 lose their low digits.
    assert_eq!(sum(&|n| n.base_hp_max as i64), 677_803_681);
    // Explicit corpse times plus the default (7) for the 10230 NPCs without one.
    assert_eq!(sum(&|n| i64::from(n.corpse_time)), 4_236_021 + 10_230 * 7);
    // Monster aggro ranges are capped to 450.
    assert_eq!(sum(&|n| i64::from(n.aggro_range)), 4_434_976);

    // Raw parts.
    assert_eq!(sum(&|n| n.skills.len() as i64), 113_672);
    assert_eq!(sum(&|n| n.clans.len() as i64), 4455);
    assert_eq!(sum(&|n| n.ignore_clan_npc_ids.len() as i64), 581);
    assert_eq!(sum(&|n| n.drops.len() as i64), 5887);
    assert_eq!(sum(&|n| n.spoils.len() as i64), 7335);
    assert_eq!(sum(&|n| n.drop_groups.len() as i64), 13_485);
    assert_eq!(
        sum(&|n| n.drop_groups.iter().map(|g| g.items.len() as i64).sum()),
        50_628
    );
    let params = |f: fn(&NpcParam) -> i64| sum(&|n| n.parameters.values().map(f).sum());
    assert_eq!(params(|p| i64::from(matches!(p, NpcParam::Text(_)))), 39_941);
    assert_eq!(params(|p| i64::from(matches!(p, NpcParam::Skill { .. }))), 10_941);
    assert_eq!(params(|p| i64::from(matches!(p, NpcParam::Minions(_)))), 415);
    assert_eq!(
        params(|p| if let NpcParam::Minions(list) = p {
            list.len() as i64
        } else {
            0
        }),
        718
    );
    assert_eq!(npcs.master_monster_ids().len(), 395);
}

#[test]
fn a_monster_is_capped_a_gatekeeper_has_its_parameters() {
    let Some(npcs) = main_npcs() else { return };

    // 20001: a level 1 fairy monster (aggroRange 1000 in the XML).
    let monster = npcs.get(20001).unwrap();
    assert_eq!((monster.level, monster.npc_type.as_str()), (1, "Monster"));
    assert_eq!((monster.name.as_str(), monster.title.as_str()), ("", ""));
    assert_eq!((monster.race, monster.sex), (NpcRace::Fairy, NpcSex::Male));
    assert_eq!(monster.aggro_range, 450);
    assert_eq!((monster.clan_help_range, monster.is_aggressive), (300, false));
    assert!(!monster.undying && monster.can_be_sown && monster.attackable);
    assert_eq!((monster.exp, monster.sp), (29.0, 2.0));
    assert_eq!(monster.base_hp_max, 39.74519_f32);
    assert_eq!(
        (
            monster.base_p_atk,
            monster.base_m_atk,
            monster.base_p_def,
            monster.base_m_def
        ),
        (8, 5, 44, 29)
    );
    assert_eq!((monster.walk_speed, monster.run_speed), (20.0, 50.0));
    assert_eq!((monster.collision_radius, monster.collision_height), (10.0, 15.0));
    assert_eq!(monster.base_attack_type, WeaponType::Sword);
    assert_eq!(
        (
            monster.base_wind_res,
            monster.base_fire_res,
            monster.base_element_res
        ),
        (53.0, 20.0, 0.0)
    );
    assert_eq!(monster.corpse_time, 7);
    assert_eq!(monster.skills.len(), 9);
    assert_eq!(monster.param_int("MoveAroundSocial1"), Some(0));

    // 30006: Roxxy, a Teleporter ("Gatekeeper").
    let gatekeeper = npcs.get(30006).unwrap();
    assert_eq!(
        (gatekeeper.name.as_str(), gatekeeper.title.as_str()),
        ("Roxxy", "Gatekeeper")
    );
    assert_eq!(gatekeeper.npc_type, "Teleporter");
    assert_eq!(
        (gatekeeper.race, gatekeeper.sex),
        (NpcRace::Human, NpcSex::Female)
    );
    assert!(!gatekeeper.attackable && gatekeeper.talkable && gatekeeper.targetable && gatekeeper.undying);
    // Not a Monster, so the aggro range of 1000 is not capped.
    assert_eq!((gatekeeper.aggro_range, gatekeeper.is_aggressive), (1000, false));
    assert_eq!(gatekeeper.param_str("IsGateKeeperForCoreTime"), Some("1"));
    assert_eq!(gatekeeper.param_str("MoveAroundSocial1"), Some("90"));
    assert_eq!(
        (gatekeeper.collision_radius, gatekeeper.collision_height),
        (8.0, 23.0)
    );
    assert!(gatekeeper.is_random_walk_enabled());
    assert!(!gatekeeper.is_type("Monster") && gatekeeper.is_type("teleporter"));
}

#[test]
fn a_merchant_and_a_raid_boss() {
    let Some(npcs) = main_npcs() else { return };

    // 30002: Jackson, Armor Merchant.
    let merchant = npcs.get(30002).unwrap();
    assert_eq!(
        (merchant.name.as_str(), merchant.title.as_str()),
        ("Jackson", "Armor Merchant")
    );
    assert_eq!((merchant.level, merchant.npc_type.as_str()), (70, "Merchant"));
    assert_eq!(
        (merchant.base_str, merchant.base_int, merchant.base_dex),
        (40, 21, 30)
    );
    assert_eq!(
        (merchant.base_wit, merchant.base_con, merchant.base_men),
        (20, 43, 20)
    );
    assert_eq!(
        (merchant.base_hp_max, merchant.base_mp_max),
        (2444.4683_f32, 1345.8_f32)
    );
    assert_eq!(merchant.base_p_atk_spd, 253);
    assert_eq!((merchant.walk_speed, merchant.run_speed), (50.0, 120.0));
    assert!(!merchant.attackable);
    assert_eq!(merchant.param_str("MoveAroundSocial1"), Some("110"));
    assert_eq!(merchant.base_fire_res, 150.0);
    assert_eq!(merchant.base_element_res, 150.0);

    // 25001: Greyclaw Kutus, a raid boss with two minion kinds, a rhand and a lhand weapon.
    let boss = npcs.get(25001).unwrap();
    assert_eq!(
        (boss.name.as_str(), boss.title.as_str()),
        ("Greyclaw Kutus", "Raid Boss")
    );
    assert_eq!((boss.level, boss.npc_type.as_str()), (23, "RaidBoss"));
    assert_eq!(
        (boss.rhand_id, boss.lhand_id, boss.chest_id, boss.weapon_enchant),
        (67, 633, 0, 0)
    );
    assert_eq!(
        (boss.exp, boss.sp, boss.raid_points),
        (2_690_116.0, 69_902.0, 284.0)
    );
    assert!(
        !boss.undying,
        "undying=false in the XML (and RaidBoss defaults to false anyway)"
    );
    assert_eq!(boss.ai_type, AiType::Balanced);
    assert_eq!((boss.base_shld_def, boss.base_shld_rate), (383, 20));
    assert_eq!(boss.skills.len(), 13);
    assert_eq!(
        boss.parameters.get("RangeHold_a"),
        Some(&NpcParam::Skill { id: 4197, level: 2 })
    );
    assert_eq!(boss.param_str("RaidSpawnMusic"), Some("Rm03_A"));
    let minions = boss.minion_list("Privates");
    assert_eq!(minions.len(), 2);
    assert_eq!(
        (
            minions[0].id,
            minions[0].count,
            minions[0].respawn_time,
            minions[0].weight_point
        ),
        (25002, 3, 360, 1)
    );
    assert!(npcs.master_monster_ids().contains(&25001));
    assert_eq!(boss.drops.len(), 12);
    assert_eq!(boss.drops[0].item_id, 348);
    assert_eq!(boss.drops[0].drop_type, DropType::Drop);
    assert_eq!((boss.drops[0].min, boss.drops[0].max), (1, 1));
    assert!((boss.drops[0].chance - 27.2693751596832).abs() < 1e-12);
}

#[test]
fn more_spot_checks() {
    let Some(npcs) = main_npcs() else { return };

    // 100: Thomas D. Turkey, an immobile-ish folk with a huge defence.
    let turkey = npcs.get(100).unwrap();
    assert_eq!(
        (turkey.name.as_str(), turkey.race, turkey.level),
        ("Thomas D. Turkey", NpcRace::Animal, 80)
    );
    assert_eq!((turkey.base_p_def, turkey.base_m_def), (60_000, 200_000));
    assert_eq!((turkey.walk_speed, turkey.run_speed), (19.5, 160.0));
    assert_eq!((turkey.aggro_range, turkey.clan_help_range), (300, 1000));
    assert_eq!(turkey.base_attack_type, WeaponType::Blunt);

    // 101: an unnamed construct that cannot move; a ground speed of 1 is not 0.1.
    let construct = npcs.get(101).unwrap();
    assert_eq!(construct.name, "");
    assert!(!construct.can_move && !construct.attackable);
    assert_eq!(
        (construct.collision_radius, construct.collision_height),
        (0.1, 0.1)
    );

    // 29001: Queen Ant, a grand boss with minions.
    let ant = npcs.get(29001).unwrap();
    assert_eq!(
        (ant.npc_type.as_str(), ant.race, ant.sex),
        ("GrandBoss", NpcRace::Bug, NpcSex::Female)
    );
    assert!(!ant.undying);
    assert_eq!(ant.base_hp_max, 96_596.0);
    assert_eq!(ant.minion_list("Privates").len(), 2);
    assert_eq!(ant.base_fire_res, -83.0);

    // 30039: a Guard does not walk around by default.
    let guard = npcs.get(30039).unwrap();
    assert_eq!((guard.npc_type.as_str(), guard.level), ("Guard", 80));
    assert!(!guard.random_walk && !guard.is_random_walk_enabled());

    // 27003: "Quest Monster" in the title.
    assert!(npcs.get(27003).unwrap().quest_monster);
    assert!(!npcs.get(20001).unwrap().quest_monster);

    // Lookups.
    assert_eq!(npcs.get_by_name("roxxy").unwrap().id, 30006);
    assert!(npcs.all_of_class_type(&["Teleporter"]).any(|n| n.id == 30006));
}

#[test]
fn custom_npcs_have_display_ids_and_server_side_names() {
    if game_dir().is_none() {
        return;
    }
    let npcs = custom_npcs(false);

    // 50009: the custom global gatekeeper looks like gatekeeper 30006.
    let gatekeeper = npcs.get(50009).unwrap();
    assert_eq!((gatekeeper.display_id, gatekeeper.level), (30006, 80));
    assert_eq!(gatekeeper.name, "Fiorella");
    assert!(gatekeeper.using_server_side_name && gatekeeper.using_server_side_title);
    assert_eq!(gatekeeper.race, NpcRace::Elf);
    assert!(!gatekeeper.attackable);
    // The same NPC without CustomNpcData does not exist, and the main ones are unchanged.
    assert_eq!(npcs.get(30006).unwrap().display_id, 30006);

    // 900101: no type and no level in the XML.
    let manager = npcs.get(900101).unwrap();
    assert_eq!((manager.npc_type.as_str(), manager.level), ("Folk", 85));

    // 80000: a fake player, only loaded when enabled.
    assert!(npcs.get(80000).is_none());
    let fakes_on = custom_npcs(true);
    let evi = fakes_on.get(80000).unwrap();
    assert!(evi.fake_player && !evi.attackable && !evi.talkable && evi.undying);
    assert_eq!(
        (evi.name.as_str(), evi.level, evi.race, evi.sex),
        ("Evi", 78, NpcRace::DarkElf, NpcSex::Female)
    );
}
