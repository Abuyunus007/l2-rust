use super::*;

fn load_with(npcs: &str, options: &NpcLoadOptions) -> NpcTemplates {
    NpcTemplates::from_xml(&format!(r#"<?xml version="1.0"?><list>{npcs}</list>"#), options).unwrap()
}

fn load(npcs: &str) -> NpcTemplates {
    load_with(npcs, &NpcLoadOptions::default())
}

#[test]
fn an_empty_npc_gets_every_java_default() {
    let t = load(r#"<npc id="7"/>"#);
    let n = t.get(7).unwrap();
    assert_eq!((n.id, n.display_id, n.level), (7, 7, 85));
    assert_eq!(
        (n.npc_type.as_str(), n.name.as_str(), n.title.as_str()),
        ("Folk", "", "")
    );
    assert_eq!(
        (n.race, n.sex, n.ai_type),
        (NpcRace::None, NpcSex::Etc, AiType::Fighter)
    );
    assert!(n.attackable && n.targetable && n.talkable && n.show_name && n.random_animation);
    assert!(n.undying && n.random_walk && n.can_move && n.is_aggressive);
    assert!(!n.unique && !n.flying && !n.quest_monster && !n.fake_player && !n.is_chaos);
    assert_eq!(n.corpse_time, 7);
    assert_eq!((n.run_speed, n.walk_speed), (120.0, 50.0));
    assert_eq!((n.swim_run_speed, n.swim_walk_speed), (120.0, 50.0));
    assert_eq!((n.fly_run_speed, n.fly_walk_speed), (120.0, 50.0));
    assert_eq!((n.base_p_atk_spd, n.base_m_atk_spd), (300, 333));
    assert_eq!((n.base_crit_rate, n.base_mcrit_rate, n.base_breath), (4, 5, 100));
    assert_eq!((n.base_attack_range, n.base_attack_angle), (40, 120));
    assert_eq!(n.base_attack_type, WeaponType::Fist);
    assert_eq!((n.min_skill_chance, n.max_skill_chance), (7, 15));
    assert_eq!((n.collision_radius, n.collision_height), (0.0, 0.0));
    assert!(n.parameters.is_empty() && n.skills.is_empty() && n.clans.is_empty());
}

#[test]
fn identity_attributes_and_flags() {
    let t = load(
        r#"<npc id="0x10" displayId="5" level="200" type="Merchant" name="Bob" title="Quest Giver"
               usingServerSideName="true" usingServerSideTitle="TRUE"/>"#,
    );
    let n = t.get(16).unwrap();
    assert_eq!((n.display_id, n.name.as_str()), (5, "Bob"));
    // Java `getByte` wraps: 200 is -56.
    assert_eq!(n.level, -56);
    assert!(n.using_server_side_name && n.using_server_side_title);
    assert!(n.quest_monster);
    assert!(n.is_type("merchant"));
}

#[test]
fn type_decides_the_defaults_of_undying_and_random_walk() {
    let t = load(
        r#"<npc id="1" type="Monster"/><npc id="2" type="RaidBoss"/><npc id="3" type="GrandBoss"/>
           <npc id="4" type="Guard"/><npc id="5" type="monster"/>
           <npc id="6" type="Guard"><status randomWalk="true" undying="false"/></npc>"#,
    );
    assert!(!t.get(1).unwrap().undying && !t.get(2).unwrap().undying && !t.get(3).unwrap().undying);
    assert!(t.get(4).unwrap().undying && !t.get(4).unwrap().random_walk);
    // `equals` is case sensitive for these two defaults.
    assert!(t.get(5).unwrap().undying);
    assert!(t.get(6).unwrap().random_walk && !t.get(6).unwrap().undying);
}

#[test]
fn stats_are_converted_like_the_stat_set_getters() {
    let t = load(
        r#"<npc id="1"><stats str="40" int="21" dex="30" wit="20" con="43" men="22">
            <vitals hp="39.74519" hpRegen="2" mp="40.0" mpRegen="0.9" />
            <attack physical="8.47458" magical="5.78704" random="30" critical="1" accuracy="9" attackSpeed="253"
                    type="BLUNT" range="50" distance="80" width="90" />
            <defence physical="44.9" magical="29.59" evasion="-18" shield="383" shieldRate="20" />
            <attribute>
                <attack type="fire" value="330" /><attack type="HOLY" value="5" /><attack type="NOPE" value="9" />
                <defence fire="1" water="2" wind="3" earth="4" holy="5" dark="6" default="7" />
            </attribute>
            <speed><walk ground="19.5" swim="10" /><run ground="160" fly="3.5" /></speed>
            <hitTime>370</hitTime>
        </stats>
        <collision><radius normal="8.9" grown="9.5"/><height normal="25.5" grown="26"/></collision></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(
        (
            n.base_str, n.base_int, n.base_dex, n.base_wit, n.base_con, n.base_men
        ),
        (40, 21, 30, 20, 43, 22)
    );
    assert_eq!(
        (n.base_hp_max, n.base_hp_reg, n.base_mp_max, n.base_mp_reg),
        (39.74519, 2.0, 40.0, 0.9)
    );
    // Doubles read with getInt are truncated.
    assert_eq!(
        (n.base_p_atk, n.base_m_atk, n.base_p_def, n.base_m_def),
        (8, 5, 44, 29)
    );
    assert_eq!(
        (n.random_damage, n.base_crit_rate, n.base_p_atk_spd),
        (30, 1, 253)
    );
    assert_eq!(
        (n.base_attack_type, n.base_attack_range, n.base_attack_angle),
        (WeaponType::Blunt, 50, 90)
    );
    assert_eq!((n.base_shld_def, n.base_shld_rate), (383, 20));
    // Attribute attack types are upper-cased first; unknown ones are ignored.
    assert_eq!((n.base_fire, n.base_holy, n.base_dark), (330, 5, 0));
    assert_eq!(
        (n.base_fire_res, n.base_water_res, n.base_wind_res),
        (1.0, 2.0, 3.0)
    );
    assert_eq!(
        (
            n.base_earth_res,
            n.base_holy_res,
            n.base_dark_res,
            n.base_element_res
        ),
        (4.0, 5.0, 6.0, 7.0)
    );
    assert_eq!((n.walk_speed, n.run_speed), (19.5, 160.0));
    assert_eq!((n.swim_walk_speed, n.swim_run_speed), (10.0, 160.0));
    assert_eq!((n.fly_walk_speed, n.fly_run_speed), (19.5, 3.5));
    assert_eq!((n.collision_radius, n.collision_height), (8.9, 25.5));
    assert_eq!((n.collision_radius_int(), n.collision_height_int()), (8, 25));
    assert_eq!((n.collision_radius_grown, n.collision_height_grown), (9.5, 26.0));
}

#[test]
fn a_ground_speed_of_zero_becomes_point_one_and_means_the_npc_can_move() {
    let t = load(
        r#"<npc id="1"><stats><speed><walk ground="0"/><run ground="-3"/></speed></stats>
               <status canMove="false"/></npc>
           <npc id="2"><stats><speed><walk ground="1"/></speed></stats><status canMove="false"/></npc>
           <npc id="3"><status canMove="false"/></npc>"#,
    );
    let a = t.get(1).unwrap();
    assert_eq!((a.walk_speed, a.run_speed), (0.1, 0.1));
    assert!(a.can_move);
    assert!(!t.get(2).unwrap().can_move);
    assert!(!t.get(3).unwrap().can_move);
}

#[test]
fn equipment_acquire_shots_status_and_corpse_time() {
    let t = load(
        r#"<npc id="1">
            <equipment chest="1" rhand="67" lhand="633" weaponEnchant="4"/>
            <acquire exp="2690116" sp="69902.5" raidPoints="284"/>
            <shots soul="2" spirit="3" shotChance="40" spiritChance="50"/>
            <status unique="true" attackable="false" targetable="False" talkable="false" undying="TRUE"
                    showName="false" randomWalk="false" randomAnimation="false" flying="true" canMove="1"
                    noSleepMode="true" passableDoor="true" hasSummoner="true" canBeSown="true"/>
            <corpseTime>12</corpseTime>
        </npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(
        (n.chest_id, n.rhand_id, n.lhand_id, n.weapon_enchant),
        (1, 67, 633, 4)
    );
    assert_eq!((n.exp, n.sp, n.raid_points), (2_690_116.0, 69_902.5, 284.0));
    assert_eq!(
        (
            n.soul_shot,
            n.spirit_shot,
            n.soul_shot_chance,
            n.spirit_shot_chance
        ),
        (2, 3, 40, 50)
    );
    assert!(n.unique && n.undying && n.flying && n.no_sleep_mode && n.passable_door);
    assert!(n.has_summoner && n.can_be_sown);
    assert!(!n.attackable && !n.targetable && !n.talkable && !n.show_name && !n.random_animation);
    assert!(!n.random_walk);
    // `Boolean.valueOf("1")` is false.
    assert!(!n.can_move);
    assert_eq!(n.corpse_time, 12);
}

#[test]
fn enums_are_read_exactly_but_race_and_sex_text_is_upper_cased() {
    let t = load(
        r#"<npc id="1"><race>dark_elf</race><sex>Female</sex></npc>
           <npc id="2"><race>ELVEN</race></npc>
           <npc id="3"><race> HUMAN </race></npc>
           <npc id="4"><stats><attack type="sword"/></stats></npc>
           <npc id="5"><ai type="Mage"/></npc>
           <npc id="6"><ai type="MAGE"/></npc>
           <npc id="7"><stats><attack type="DUALFIST"/></stats></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!((n.race, n.sex), (NpcRace::DarkElf, NpcSex::Female));
    assert!(t.get(2).is_none(), "unknown race");
    assert!(t.get(3).is_none(), "the text is not trimmed");
    assert!(t.get(4).is_none(), "attack type is exact case");
    assert!(t.get(5).is_none(), "ai type is exact case");
    assert_eq!(t.get(6).unwrap().ai_type, AiType::Mage);
    assert_eq!(t.get(7).unwrap().base_attack_type, WeaponType::DualFist);
}

#[test]
fn ai_fields_and_the_monster_aggro_cap() {
    let t = load(
        r#"<npc id="1" type="Monster"><ai type="ARCHER" aggroRange="1000" clanHelpRange="300" isChaos="true"
                isAggressive="false"><skill minChance="1" maxChance="2" primaryId="4000" shortRangeId="1"
                shortRangeChance="3" longRangeId="2" longRangeChance="4"/></ai></npc>
           <npc id="2" type="MONSTER"><ai aggroRange="451"/></npc>
           <npc id="3" type="Monster"><ai aggroRange="450"/></npc>
           <npc id="4" type="Folk"><ai aggroRange="1000"/></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(
        (n.ai_type, n.aggro_range, n.clan_help_range),
        (AiType::Archer, 450, 300)
    );
    assert!(n.is_chaos && !n.is_aggressive);
    assert_eq!((n.min_skill_chance, n.max_skill_chance), (1, 2));
    assert_eq!(t.get(2).unwrap().aggro_range, 450);
    assert_eq!(t.get(3).unwrap().aggro_range, 450);
    assert_eq!(t.get(4).unwrap().aggro_range, 1000);
    let custom = NpcLoadOptions {
        max_aggro_range: 100,
        default_corpse_time: 9,
        ..NpcLoadOptions::default()
    };
    let t = load_with(
        r#"<npc id="1" type="Monster"><ai aggroRange="300"/></npc>"#,
        &custom,
    );
    assert_eq!(
        (t.get(1).unwrap().aggro_range, t.get(1).unwrap().corpse_time),
        (100, 9)
    );
}

#[test]
fn clan_lists_hand_out_ids_in_first_seen_order() {
    let t = load(
        r#"<npc id="1"><ai><clanList><clan>ALL</clan><clan>Dragon</clan><ignoreNpcId>12</ignoreNpcId>
               <ignoreNpcId>12</ignoreNpcId></clanList></ai></npc>
           <npc id="2"><ai><clanList><clan>Dragon</clan><clan>Orc</clan></clanList></ai></npc>
           <npc id="3"><ai><clanList><clan>Bad</clan><ignoreNpcId>x</ignoreNpcId></clanList></ai></npc>"#,
    );
    assert_eq!(t.generic_clan_id(), 0);
    assert_eq!(
        (t.clan_id("Dragon"), t.clan_id("Orc"), t.clan_id("Elf")),
        (1, 2, -1)
    );
    assert_eq!(t.get(1).unwrap().clans, BTreeSet::from([0, 1]));
    assert_eq!(t.get(1).unwrap().ignore_clan_npc_ids, BTreeSet::from([12]));
    assert_eq!(t.get(2).unwrap().clans, BTreeSet::from([1, 2]));
    // `Integer.parseInt` fails: the NPC is not loaded.
    assert!(t.get(3).is_none());
}

#[test]
fn parameters_keep_text_skills_and_minions() {
    let t = load(
        r#"<npc id="1"><parameters>
            <param name="IsGateKeeperForCoreTime" value="1"/>
            <param name="MoveArounding" value="0"/>
            <param name="Gone" value="x"/>
            <param name="Gone"/>
            <skill name="RangeHold_a" id="4197" level="2"/>
            <minions name="Privates"><npc id="25002" count="3" respawnTime="360" weightPoint="1"/>
                <npc id="25003" count="1" max="4" respawnTime="10"/></minions>
            <minions name="Empty"/>
        </parameters><parameters><param name="Second" value="2"/></parameters></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(n.param_str("IsGateKeeperForCoreTime"), Some("1"));
    assert_eq!(n.param_int("IsGateKeeperForCoreTime"), Some(1));
    assert_eq!(n.param_str("Second"), Some("2"));
    // A param without a value removes the earlier one (Java stores null).
    assert!(!n.parameters.contains_key("Gone"));
    assert_eq!(
        n.parameters.get("RangeHold_a"),
        Some(&NpcParam::Skill { id: 4197, level: 2 })
    );
    assert_eq!(
        n.minion_list("Privates"),
        &[
            MinionDef {
                id: 25002,
                count: 3,
                max: 0,
                respawn_time: 360,
                weight_point: 1
            },
            MinionDef {
                id: 25003,
                count: 1,
                max: 4,
                respawn_time: 10,
                weight_point: 0
            },
        ]
    );
    // Java does not store an empty minion list.
    assert!(!n.parameters.contains_key("Empty"));
    // `MoveArounding` = 0 switches random walk off even though the flag is on.
    assert!(n.random_walk && !n.is_random_walk_enabled());
    assert!(t.master_monster_ids().contains(&1));
}

#[test]
fn master_monsters_need_privates_and_no_summon_private_rate() {
    let minions = r#"<minions name="Privates"><npc id="2" count="1" respawnTime="1"/></minions>"#;
    let t = load(&format!(
        r#"<npc id="1"><parameters>{minions}</parameters></npc>
           <npc id="2"><parameters>{minions}<param name="SummonPrivateRate" value="50"/></parameters></npc>
           <npc id="3"><parameters><param name="Privates" value="x"/></parameters></npc>
           <npc id="4"/>"#
    ));
    assert_eq!(
        t.master_monster_ids().iter().copied().collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn skill_list_is_kept_raw_and_a_later_list_replaces_it() {
    let t = load(
        r#"<npc id="1"><skillList><skill id="4408" level="1"/><skill id="4410" level="11"/><skill id="4408" level="2"/></skillList></npc>
           <npc id="2"><skillList><skill id="1" level="1"/></skillList><skillList><skill id="2" level="3"/></skillList></npc>
           <npc id="3"><skillList><skill id="1"/></skillList></npc>"#,
    );
    let ids: Vec<(i32, i32)> = t.get(1).unwrap().skills.iter().map(|s| (s.id, s.level)).collect();
    assert_eq!(ids, vec![(4408, 1), (4410, 11), (4408, 2)]);
    assert_eq!(t.get(2).unwrap().skills, vec![NpcSkillRef { id: 2, level: 3 }]);
    assert!(t.get(3).is_none(), "a skill without a level fails the NPC");
}

#[test]
fn drop_lists_are_kept_raw_and_bad_entries_are_skipped() {
    let t = load(
        r#"<npc id="1"><dropLists>
            <drop>
                <item id="57" min="10" max="0x20" chance="75.5"/>
                <item id="58" min="1" max="1"/>
                <group chance="25"><item id="1" min="1" max="1" chance="60"/><item id="2" min="2" max="3" chance="40"/></group>
                <group><item id="9" min="1" max="1" chance="1"/></group>
            </drop>
            <spoil><item id="3" min="1" max="2" chance="10"/></spoil>
            <other><item id="4" min="1" max="1" chance="1"/></other>
        </dropLists></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(n.drops.len(), 1);
    assert_eq!(
        (
            n.drops[0].item_id,
            n.drops[0].min,
            n.drops[0].max,
            n.drops[0].chance
        ),
        (57, 10, 32, 75.5)
    );
    assert_eq!(n.drops[0].drop_type, DropType::Drop);
    assert_eq!(n.spoils.len(), 1);
    assert_eq!(n.spoils[0].drop_type, DropType::Spoil);
    assert_eq!(n.drop_groups.len(), 1);
    assert_eq!(n.drop_groups[0].chance, 25.0);
    assert_eq!(
        n.drop_groups[0]
            .items
            .iter()
            .map(|d| d.item_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn fake_players_are_skipped_unless_enabled() {
    let xml = r#"<npc id="1" name="A"/><npc id="2" name="B"><fakePlayer classId="107" hair="1"/></npc>"#;
    let t = load(xml);
    assert_eq!(t.iter().map(|n| n.id).collect::<Vec<_>>(), vec![1]);
    let on = NpcLoadOptions {
        fake_players_enabled: true,
        ..NpcLoadOptions::default()
    };
    let t = load_with(xml, &on);
    assert_eq!(t.len(), 2);
    assert!(t.get(2).unwrap().fake_player && !t.get(1).unwrap().fake_player);
}

#[test]
fn a_broken_npc_is_skipped_and_the_rest_loads() {
    let t = load(
        r#"<npc id="1"/>
           <npc id="2"><stats><speed><walk swim="3"/></speed></stats></npc>
           <npc id="3" level="x"/>
           <npc/>
           <npc id="5"><stats str="abc"/></npc>
           <npc id="6"><corpseTime>soon</corpseTime></npc>
           <npc id="7"><stats><attribute><attack value="3"/></attribute></stats></npc>
           <npc id="8"><acquire exp="1,5"/></npc>
           <npc id="9"><corpseTime>5</corpseTime></npc>"#,
    );
    assert_eq!(t.iter().map(|n| n.id).collect::<Vec<_>>(), vec![1, 9]);
}

#[test]
fn a_later_definition_of_an_id_replaces_the_earlier_one() {
    let t = load(r#"<npc id="1" name="First"><race>HUMAN</race></npc><npc id="1" name="Second"/>"#);
    assert_eq!(t.len(), 1);
    let n = t.get(1).unwrap();
    assert_eq!((n.name.as_str(), n.race), ("Second", NpcRace::None));
}

#[test]
fn lookups_and_filters() {
    let t = load(
        r#"<npc id="1" level="10" type="Monster" name="Wolf"/><npc id="2" level="10" type="Folk" name="Wanda"/>
           <npc id="3" level="20" type="Monster" name="wolf"/><npc id="4" level="20" type="Folk" name="Walter"/>"#,
    );
    assert_eq!(t.get_by_name("WOLF").unwrap().id, 1);
    assert!(t.get_by_name("cat").is_none());
    assert_eq!(
        t.all_of_level(&[20]).map(|n| n.id).collect::<Vec<_>>(),
        vec![3, 4]
    );
    assert_eq!(
        t.all_monsters_of_level(&[10, 20])
            .map(|n| n.id)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(
        t.all_npc_starting_with("Wa").map(|n| n.id).collect::<Vec<_>>(),
        vec![2, 4]
    );
    assert_eq!(t.all_of_class_type(&["monster"]).count(), 2);
}

#[test]
fn unknown_elements_comments_and_entities_are_tolerated() {
    let t = load(
        r#"<npc id="1" name="Dragon&apos;s &amp; Co"><!-- c --><exCrtEffect>true</exCrtEffect><sNpcPropHpRate>1</sNpcPropHpRate>
           <weird a="b"><deep/></weird><stats><hitTime>600</hitTime><vitals hp="5"/></stats></npc>"#,
    );
    let n = t.get(1).unwrap();
    assert_eq!(n.name, "Dragon's & Co");
    assert_eq!(n.base_hp_max, 5.0);
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn custom_npcs_load_after_the_main_folder_and_only_when_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let npcs = dir.path().join("data/stats/npcs");
    std::fs::create_dir_all(npcs.join("custom/sub")).unwrap();
    write(
        &npcs,
        "a.xml",
        r#"<list><npc id="1" name="main"/><npc id="2" name="main2"/></list>"#,
    );
    write(
        &npcs.join("custom"),
        "c.xml",
        r#"<list><npc id="2" name="custom"/><npc id="3" name="custom3"/></list>"#,
    );
    write(
        &npcs.join("custom/sub"),
        "d.xml",
        r#"<list><npc id="4" name="deep"/></list>"#,
    );
    write(&npcs, "broken.xml", "<list><npc id=");

    let off = NpcTemplates::load(dir.path(), &NpcLoadOptions::default());
    assert_eq!(off.iter().map(|n| n.id).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(off.get(2).unwrap().name, "main2");

    let on = NpcTemplates::load(
        dir.path(),
        &NpcLoadOptions {
            custom_npc_data: true,
            ..NpcLoadOptions::default()
        },
    );
    // Subfolders of custom are read too (recursive), and the custom definition of 2 wins.
    assert_eq!(on.iter().map(|n| n.id).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert_eq!(on.get(2).unwrap().name, "custom");
}

#[test]
fn missing_folder_loads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(NpcTemplates::load(dir.path(), &NpcLoadOptions::default()).is_empty());
}
