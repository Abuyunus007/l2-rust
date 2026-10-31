#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Stat functions, conditions and skills on item templates (M4-DATA-10).
//!
//! The first half builds items from inline XML and checks Java's rules one by one. The second
//! half loads the real datapack (skipped when the folder does not exist, e.g. in CI); its numbers
//! were counted from the XML with an independent script (every `<stat>`, `<conditions>` and
//! `<skill>` element, plus the enchant functions Java adds per weapon/armor with a `<stats>` block).
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::item::{ChanceSkill, Element, ElementalValue, ItemTemplates, SkillRef};
use l2_data::stats::enums::CategoryType;
use l2_data::stats::{Condition, ConditionMessage, FuncKind, FuncTemplate, Stat};
use l2_data::{ItemTemplate, npc::NpcRace};

fn load(items: &str) -> ItemTemplates {
    ItemTemplates::from_xml(&format!(r#"<?xml version="1.0"?><list>{items}</list>"#)).unwrap()
}

/// `(stat, kind, order, value)` of every function, in order.
fn summary(item: &ItemTemplate) -> Vec<(Stat, FuncKind, i32, f64)> {
    item.funcs
        .iter()
        .map(|f| (f.stat, f.kind, f.order, f.value))
        .collect()
}

#[test]
fn stats_switch_picks_set_add_or_mul_with_order_zero() {
    let t = load(
        r#"<item id="1" type="EtcItem" name="x"><stats>
            <stat type="pAtk">1</stat><stat type="pAtkSpd">2</stat><stat type="pAtkRange">3</stat>
            <stat type="critRate">4</stat><stat type="mAtk">5</stat><stat type="sDef">6</stat><stat type="rShld">7</stat>
            <stat type="accCombat">8</stat><stat type="rEvas">9</stat><stat type="randomDamage">10</stat>
            <stat type="pDef">11</stat><stat type="mDef">12</stat><stat type="maxMp">13</stat>
            <stat type="magicSuccRes">14</stat>
        </stats></item>"#,
    );
    let got = summary(t.get(1).unwrap());
    let expected = [
        (Stat::PowerAttack, FuncKind::Set, 1.0),
        (Stat::PowerAttackSpeed, FuncKind::Set, 2.0),
        (Stat::PowerAttackRange, FuncKind::Set, 3.0),
        (Stat::CriticalRate, FuncKind::Set, 4.0),
        (Stat::MagicAttack, FuncKind::Set, 5.0),
        (Stat::ShieldDefence, FuncKind::Set, 6.0),
        (Stat::ShieldRate, FuncKind::Set, 7.0),
        (Stat::AccuracyCombat, FuncKind::Add, 8.0),
        (Stat::EvasionRate, FuncKind::Add, 9.0),
        (Stat::RandomDamage, FuncKind::Add, 10.0),
        (Stat::PowerDefence, FuncKind::Add, 11.0),
        (Stat::MagicDefence, FuncKind::Add, 12.0),
        (Stat::MaxMp, FuncKind::Add, 13.0),
        (Stat::MagicSuccessRes, FuncKind::Mul, 14.0),
    ];
    assert_eq!(got.len(), expected.len());
    for ((stat, kind, order, value), (e_stat, e_kind, e_value)) in got.into_iter().zip(expected) {
        // NOTE: Java passes order 0x00, not the kind's default (30 for add, 20 for mul).
        assert_eq!((stat, kind, order, value), (e_stat, e_kind, 0, e_value));
    }
    // An etc item gets no enchant functions.
    assert_eq!(t.get(1).unwrap().funcs.len(), 14);
}

#[test]
fn unhandled_stats_are_skipped_but_unknown_ones_fail_the_item() {
    let t = load(
        r#"<item id="1" type="EtcItem" name="skipped"><stats><stat type="maxHp">5</stat><stat type="pDef">1</stat></stats></item>
        <item id="2" type="EtcItem" name="unknown"><stats><stat type="nope">5</stat></stats></item>
        <item id="3" type="EtcItem" name="case"><stats><stat type="pdef">5</stat></stats></item>
        <item id="4" type="EtcItem" name="no type"><stats><stat>5</stat></stats></item>
        <item id="5" type="EtcItem" name="bad number"><stats><stat type="pDef">five</stat></stats></item>
        <item id="6" type="EtcItem" name="other element"><stats><nope type="pDef">1</nope><stat type="pDef"> 2 </stat></stats></item>"#,
    );
    // maxHp is a real stat that items do not handle: warning, skipped.
    assert_eq!(
        summary(t.get(1).unwrap()),
        vec![(Stat::PowerDefence, FuncKind::Add, 0, 1.0)]
    );
    for id in [2, 3, 4, 5] {
        assert!(t.get(id).is_none(), "item {id} should have failed");
    }
    assert_eq!(
        summary(t.get(6).unwrap()),
        vec![(Stat::PowerDefence, FuncKind::Add, 0, 2.0)]
    );
}

#[test]
fn enchant_functions_by_item_kind() {
    let t = load(
        r#"<item id="1" type="Weapon" name="w"><stats><stat type="pAtk">8</stat></stats></item>
        <item id="2" type="Armor" name="shield"><set name="bodypart" val="lhand"/><stats><stat type="sDef">8</stat></stats></item>
        <item id="3" type="Armor" name="ring"><set name="bodypart" val="rfinger;lfinger"/><stats><stat type="mDef">8</stat></stats></item>
        <item id="4" type="Armor" name="plate"><set name="armor_type" val="HEAVY"/><set name="bodypart" val="chest"/><stats><stat type="pDef">8</stat></stats></item>
        <item id="5" type="Armor" name="heavy left hand"><set name="armor_type" val="HEAVY"/><set name="bodypart" val="lhand"/><stats><stat type="pDef">8</stat></stats></item>
        <item id="6" type="Weapon" name="no stats block"/>
        <item id="7" type="Armor" name="empty stats block"><set name="bodypart" val="chest"/><stats/></item>"#,
    );
    let enchants = |id: i32| -> Vec<Stat> {
        t.get(id)
            .unwrap()
            .funcs
            .iter()
            .filter(|f| f.kind == FuncKind::Enchant)
            .map(|f| f.stat)
            .collect()
    };
    assert_eq!(enchants(1), vec![Stat::PowerAttack, Stat::MagicAttack]);
    assert_eq!(enchants(2), vec![Stat::ShieldDefence]);
    assert_eq!(enchants(3), vec![Stat::MagicDefence]);
    assert_eq!(enchants(4), vec![Stat::PowerDefence]);
    // A non-shield armor worn in the left hand is a plain armor.
    assert_eq!(enchants(5), vec![Stat::PowerDefence]);
    // The enchant functions come after the `<stat>` ones, with order 0 and value 0.
    let w = t.get(1).unwrap();
    assert_eq!(
        summary(w),
        vec![
            (Stat::PowerAttack, FuncKind::Set, 0, 8.0),
            (Stat::PowerAttack, FuncKind::Enchant, 0, 0.0),
            (Stat::MagicAttack, FuncKind::Enchant, 0, 0.0),
        ]
    );
    // No <stats> block: no functions at all, not even enchant ones. An empty block still enables them.
    assert!(t.get(6).unwrap().funcs.is_empty());
    assert_eq!(enchants(7), vec![Stat::PowerDefence]);
    assert_eq!(t.get(7).unwrap().funcs.len(), 1);
}

#[test]
fn a_second_stats_block_repeats_the_enchant_functions() {
    // NOTE(java-quirk): `hasFunction(FuncEnchant.class)` is always false in Java.
    let t = load(
        r#"<item id="1" type="Weapon" name="w"><stats><stat type="pAtk">8</stat></stats><stats><stat type="mAtk">5</stat></stats></item>"#,
    );
    let kinds: Vec<_> = t.get(1).unwrap().funcs.iter().map(|f| (f.stat, f.kind)).collect();
    assert_eq!(
        kinds,
        vec![
            (Stat::PowerAttack, FuncKind::Set),
            (Stat::PowerAttack, FuncKind::Enchant),
            (Stat::MagicAttack, FuncKind::Enchant),
            (Stat::MagicAttack, FuncKind::Set),
            (Stat::PowerAttack, FuncKind::Enchant),
            (Stat::MagicAttack, FuncKind::Enchant),
        ]
    );
}

#[test]
fn elemental_stats_set_the_base_element() {
    let t = load(
        r#"<item id="1" type="Armor" name="a"><set name="bodypart" val="chest"/><stats>
            <stat type="fireRes">10.9</stat><stat type="holyPower">250</stat><stat type="fireRes">30</stat><stat type="windRes">-4</stat>
        </stats></item>"#,
    );
    let item = t.get(1).unwrap();
    // `(int) value`, and a repeated element updates its entry instead of adding one.
    assert_eq!(
        item.elementals,
        vec![
            ElementalValue {
                element: Element::Fire,
                value: 30
            },
            ElementalValue {
                element: Element::Holy,
                value: 250
            },
            ElementalValue {
                element: Element::Wind,
                value: -4
            },
        ]
    );
    // All four are functions, though (+ the enchant function).
    assert_eq!(item.funcs.len(), 5);
    assert_eq!(Element::Dark as u8, 5);
}

#[test]
fn conditions_attach_in_order_with_their_messages() {
    let t = load(
        r#"<item id="1" type="EtcItem" name="x">
            <conditions msgId="113" addName="1"><player flyMounted="false"/></conditions>
            <conditions msg="Not for you"><player level="40"/></conditions>
            <conditions><foo/></conditions>
            <conditions msgId="1518"><and><player sex="1"/><not><target playable="true"/></not></and></conditions>
        </item>"#,
    );
    let conditions = &t.get(1).unwrap().conditions;
    // The block without a recognised condition is dropped (Java stores null and skips it).
    assert_eq!(conditions.len(), 3);
    assert!(t.get(1).unwrap().is_condition_attached());
    assert_eq!(
        conditions[0].condition,
        Condition::PlayerFlyMounted { value: false }
    );
    assert_eq!(
        conditions[0].message,
        ConditionMessage {
            message: None,
            message_id: 113,
            add_name: true
        }
    );
    assert_eq!(conditions[1].condition, Condition::PlayerLevel { level: 40 });
    assert_eq!(conditions[1].message.message.as_deref(), Some("Not for you"));
    assert_eq!(
        conditions[2].condition,
        Condition::And(vec![
            Condition::PlayerSex { sex: 1 },
            Condition::Not(Box::new(Condition::TargetPlayable)),
        ])
    );
}

#[test]
fn a_bad_condition_fails_the_item() {
    let t = load(
        r##"<item id="1" type="EtcItem" name="bad level"><conditions><player level="x"/></conditions></item>
        <item id="2" type="EtcItem" name="table ref"><conditions><player level="#lvl"/></conditions></item>
        <item id="3" type="EtcItem" name="bad race"><conditions><player races="ELFF"/></conditions></item>
        <item id="4" type="EtcItem" name="bad msgId"><conditions msgId="x"><player level="1"/></conditions></item>
        <item id="5" type="EtcItem" name="ok"><conditions><player level="1"/></conditions></item>"##,
    );
    let ids: Vec<i32> = t.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![5]);
}

#[test]
fn conditions_before_or_after_stats_both_work_and_keep_the_funcs_order() {
    let t = load(
        r#"<item id="1" type="Armor" name="before"><set name="bodypart" val="chest"/>
            <conditions><player level="5"/></conditions><stats><stat type="pDef">3</stat></stats></item>
        <item id="2" type="Armor" name="after"><set name="bodypart" val="chest"/>
            <stats><stat type="pDef">3</stat></stats><conditions><player level="5"/></conditions></item>"#,
    );
    for id in [1, 2] {
        let item = t.get(id).unwrap();
        assert_eq!(item.conditions.len(), 1);
        assert_eq!(item.funcs.len(), 2);
    }
}

#[test]
fn skills_by_type_and_item_class() {
    let t = load(
        r#"<item id="1" type="Weapon" name="w"><skills>
            <skill id="10" level="1"/><skill id="11" level="2" type="NORMAL"/>
            <skill id="20" level="3" type="ON_ENCHANT_4"/>
            <skill id="30" level="4" type="ON_UNEQUIP"/>
            <skill id="40" level="5" type="ON_CRITICAL_SKILL" type_chance="15"/>
            <skill id="50" level="6" type="ON_MAGIC_SKILL"/>
        </skills></item>
        <item id="2" type="Armor" name="a"><set name="bodypart" val="chest"/><skills>
            <skill id="10" level="1"/><skill id="20" level="3" type="ON_ENCHANT_4"/><skill id="40" level="5" type="ON_CRITICAL_SKILL"/>
        </skills></item>
        <item id="3" type="EtcItem" name="e"><skills><skill id="10" level="1"/><skill id="20" level="3" type="ON_ENCHANT_4"/></skills></item>
        <item id="4" type="EtcItem" name="none"/>"#,
    );
    let w = &t.get(1).unwrap().skills;
    assert_eq!(
        w.item_skills,
        Some(vec![SkillRef { id: 10, level: 1 }, SkillRef { id: 11, level: 2 }])
    );
    assert_eq!(w.enchant4_skill, Some(SkillRef { id: 20, level: 3 }));
    assert_eq!(w.unequip_skill, Some(SkillRef { id: 30, level: 4 }));
    assert_eq!(
        w.on_crit,
        Some(ChanceSkill {
            skill: SkillRef { id: 40, level: 5 },
            chance: 15
        })
    );
    assert_eq!(
        w.on_magic,
        Some(ChanceSkill {
            skill: SkillRef { id: 50, level: 6 },
            chance: 100
        })
    );
    assert!(t.get(1).unwrap().has_skills());

    let a = &t.get(2).unwrap().skills;
    assert_eq!(a.enchant4_skill, Some(SkillRef { id: 20, level: 3 }));
    assert_eq!(a.on_crit, None);

    let e = &t.get(3).unwrap().skills;
    assert_eq!(e.item_skills, Some(vec![SkillRef { id: 10, level: 1 }]));
    assert_eq!(e.enchant4_skill, None);

    assert!(!t.get(4).unwrap().has_skills());
    assert_eq!(t.get(4).unwrap().skills, Default::default());
}

#[test]
fn a_set_item_skill_and_skill_elements_share_one_value() {
    // `<skills>` appends to whatever `item_skill` already holds, like Java's StatSet.
    let t = load(
        r#"<item id="1" type="EtcItem" name="x"><set name="item_skill" val="5-1"/><skills><skill id="6" level="2"/></skills></item>"#,
    );
    assert_eq!(
        t.get(1).unwrap().skills.item_skills,
        Some(vec![SkillRef { id: 5, level: 1 }, SkillRef { id: 6, level: 2 }])
    );
}

#[test]
fn skills_do_not_create_the_item_but_sets_after_stats_still_fail() {
    let t = load(
        r#"<item id="1" type="EtcItem" name="ok"><skills><skill id="6" level="2"/></skills><set name="weight" val="5"/></item>
        <item id="2" type="EtcItem" name="bad"><stats/><set name="weight" val="5"/></item>
        <item id="3" type="EtcItem" name="bad skill"><skills><skill id="x" level="2"/></skills></item>
        <item id="4" type="EtcItem" name="no level"><skills><skill id="6"/></skills></item>"#,
    );
    assert_eq!(t.get(1).unwrap().weight, 5);
    assert!(t.get(2).is_none());
    assert!(t.get(3).is_none());
    assert!(t.get(4).is_none());
}

// ---- the real datapack ----

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

fn real_items() -> Option<ItemTemplates> {
    let dir = game_dir()?;
    let items = ItemTemplates::load(&dir, false);
    // The stats/skills/conditions parsing must not lose a single item.
    assert_eq!(items.len(), 19199);
    Some(items)
}

fn leaves<'a>(c: &'a Condition, out: &mut Vec<&'a Condition>) {
    match c {
        Condition::And(all) | Condition::Or(all) => all.iter().for_each(|c| leaves(c, out)),
        Condition::Not(inner) => leaves(inner, out),
        leaf => out.push(leaf),
    }
}

#[test]
fn real_funcs_match_the_xml_counts() {
    let Some(items) = real_items() else { return };
    let all: Vec<&FuncTemplate> = items.iter().flat_map(|i| i.funcs.iter()).collect();
    assert_eq!(items.iter().filter(|i| !i.funcs.is_empty()).count(), 7207);
    // 30936 <stat> elements + 2 enchant functions for each of 3892 weapons and 1 for each of
    // 2658 armor pieces with a <stats> block.
    assert_eq!(all.len(), 30936 + 2 * 3892 + 2658);
    assert!(
        all.iter()
            .all(|f| f.order == 0 && f.attach_cond.is_none() && f.apply_cond.is_none())
    );

    let count = |stat: Stat, kind: FuncKind| all.iter().filter(|f| f.stat == stat && f.kind == kind).count();
    let expected = [
        (Stat::AccuracyCombat, FuncKind::Add, 2740),
        (Stat::CriticalRate, FuncKind::Set, 4035),
        (Stat::DarkRes, FuncKind::Add, 23),
        (Stat::EarthRes, FuncKind::Add, 14),
        (Stat::FireRes, FuncKind::Add, 16),
        (Stat::HolyPower, FuncKind::Add, 14),
        (Stat::HolyRes, FuncKind::Add, 14),
        (Stat::MagicAttack, FuncKind::Set, 3999),
        (Stat::MagicDefence, FuncKind::Add, 550),
        (Stat::MagicSuccessRes, FuncKind::Mul, 2),
        (Stat::MaxMp, FuncKind::Add, 525),
        (Stat::PowerAttack, FuncKind::Set, 3999),
        (Stat::PowerAttackRange, FuncKind::Set, 3857),
        (Stat::PowerAttackSpeed, FuncKind::Set, 4047),
        (Stat::PowerDefence, FuncKind::Add, 2448),
        (Stat::EvasionRate, FuncKind::Add, 215),
        (Stat::ShieldRate, FuncKind::Set, 191),
        (Stat::RandomDamage, FuncKind::Add, 4029),
        (Stat::ShieldDefence, FuncKind::Set, 186),
        (Stat::WaterRes, FuncKind::Add, 16),
        (Stat::WindRes, FuncKind::Add, 16),
    ];
    let mut listed = 0;
    for (stat, kind, n) in expected {
        assert_eq!(count(stat, kind), n, "{stat:?} {kind:?}");
        listed += n;
    }
    assert_eq!(listed, 30936, "every <stat> is accounted for");

    // The automatic enchant functions: 3892 weapons, 169 shields, 442 jewels, 2047 other armor.
    assert_eq!(count(Stat::PowerAttack, FuncKind::Enchant), 3892);
    assert_eq!(count(Stat::MagicAttack, FuncKind::Enchant), 3892);
    assert_eq!(count(Stat::ShieldDefence, FuncKind::Enchant), 169);
    assert_eq!(count(Stat::MagicDefence, FuncKind::Enchant), 442);
    assert_eq!(count(Stat::PowerDefence, FuncKind::Enchant), 2047);
    assert_eq!(
        all.iter().filter(|f| f.kind == FuncKind::Enchant).count(),
        2 * 3892 + 2658
    );
}

#[test]
fn real_elementals_match_the_xml() {
    let Some(items) = real_items() else { return };
    assert_eq!(items.iter().filter(|i| !i.elementals.is_empty()).count(), 83);
    assert_eq!(items.iter().map(|i| i.elementals.len()).sum::<usize>(), 113);
    // Item 6611 (hero weapon): holy power 250.
    assert_eq!(
        items.get(6611).unwrap().elementals,
        vec![ElementalValue {
            element: Element::Holy,
            value: 250
        }]
    );
}

#[test]
fn real_short_sword() {
    let Some(items) = real_items() else { return };
    let sword = items.get(1).unwrap();
    assert_eq!(
        summary(sword),
        vec![
            (Stat::PowerAttack, FuncKind::Set, 0, 8.0),
            (Stat::MagicAttack, FuncKind::Set, 0, 6.0),
            (Stat::CriticalRate, FuncKind::Set, 0, 8.0),
            (Stat::PowerAttackSpeed, FuncKind::Set, 0, 379.0),
            (Stat::RandomDamage, FuncKind::Add, 0, 10.0),
            (Stat::PowerAttackRange, FuncKind::Set, 0, 40.0),
            (Stat::PowerAttack, FuncKind::Enchant, 0, 0.0),
            (Stat::MagicAttack, FuncKind::Enchant, 0, 0.0),
        ]
    );
    assert!(sword.conditions.is_empty() && !sword.has_skills());
}

#[test]
fn real_shield_accessory_and_armor() {
    let Some(items) = real_items() else { return };
    // Round Shield: conditions come before <stats> in the XML.
    let shield = items.get(102).unwrap();
    assert_eq!(
        summary(shield),
        vec![
            (Stat::EvasionRate, FuncKind::Add, 0, -8.0),
            (Stat::ShieldRate, FuncKind::Set, 0, 20.0),
            (Stat::ShieldDefence, FuncKind::Set, 0, 79.0),
            (Stat::ShieldDefence, FuncKind::Enchant, 0, 0.0),
        ]
    );
    assert_eq!(shield.conditions.len(), 1);
    assert_eq!(
        shield.conditions[0].condition,
        Condition::PlayerRace {
            races: [
                NpcRace::Human,
                NpcRace::Elf,
                NpcRace::DarkElf,
                NpcRace::Orc,
                NpcRace::Dwarf
            ]
            .into_iter()
            .collect()
        }
    );
    assert_eq!(shield.conditions[0].message.message_id, 1518);
    assert!(!shield.conditions[0].message.add_name);

    // Apprentice's Earring (jewel): the enchant function is on mDef.
    assert_eq!(
        summary(items.get(112).unwrap()),
        vec![
            (Stat::MagicDefence, FuncKind::Add, 0, 11.0),
            (Stat::MagicDefence, FuncKind::Enchant, 0, 0.0),
        ]
    );
    // Plate Helmet.
    assert_eq!(
        summary(items.get(2412).unwrap()),
        vec![
            (Stat::PowerDefence, FuncKind::Add, 0, 44.0),
            (Stat::PowerDefence, FuncKind::Enchant, 0, 0.0),
        ]
    );
    // Circlet of Innadril: no <stats>, so no functions; an `and` condition and an item skill.
    let circlet = items.get(6834).unwrap();
    assert!(circlet.funcs.is_empty());
    assert_eq!(
        circlet.conditions[0].condition,
        Condition::And(vec![
            Condition::PlayerHasCastle { castle: 6 },
            Condition::PlayerPledgeClass { pledge_class: 2 },
        ])
    );
    assert_eq!(
        circlet.skills.item_skills,
        Some(vec![SkillRef { id: 3633, level: 1 }])
    );
}

#[test]
fn real_conditions_match_the_xml() {
    let Some(items) = real_items() else { return };
    let blocks: Vec<_> = items.iter().flat_map(|i| i.conditions.iter()).collect();
    assert_eq!(blocks.len(), 2799);
    assert_eq!(items.iter().filter(|i| i.is_condition_attached()).count(), 2791);

    let with_id = |id: i32| blocks.iter().filter(|b| b.message.message_id == id).count();
    assert_eq!(with_id(1518), 2074);
    assert_eq!(with_id(113), 535);
    assert_eq!(with_id(600), 106);
    assert_eq!(with_id(1902), 38);
    assert_eq!(with_id(2453), 25);
    assert_eq!(with_id(1685), 16);
    assert_eq!(with_id(0), 5);
    assert_eq!(blocks.iter().filter(|b| b.message.add_name).count(), 535);
    assert!(blocks.iter().all(|b| b.message.message.is_none()));

    // Item 726 has two separate blocks.
    assert_eq!(items.get(726).unwrap().conditions.len(), 2);

    let mut flat = Vec::new();
    for block in &blocks {
        leaves(&block.condition, &mut flat);
    }
    let n = |f: &dyn Fn(&Condition) -> bool| flat.iter().filter(|c| f(c)).count();
    assert_eq!(n(&|c| matches!(c, Condition::PlayerRace { .. })), 1892);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerFlyMounted { .. })), 383);
    assert_eq!(n(&|c| matches!(c, Condition::CategoryType { .. })), 261);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerLevel { .. })), 126);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerPledgeClass { .. })), 71);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerInstanceId { .. })), 39);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerLevelRange { .. })), 38);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerSex { .. })), 33);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerIsHero { .. })), 29);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerCloakStatus { .. })), 25);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerHasCastle { .. })), 25);
    assert_eq!(n(&|c| matches!(c, Condition::SiegeZone { own: true, .. })), 22);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerPkCount { .. })), 16);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerSubclass { .. })), 12);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerHasClanHall { .. })), 8);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerInsideZoneId { .. })), 7);
    assert_eq!(
        n(&|c| matches!(
            c,
            Condition::PlayerState {
                check: l2_data::stats::PlayerState::Chaotic,
                ..
            }
        )),
        6
    );
    assert_eq!(n(&|c| matches!(c, Condition::PlayerVehicleMounted { .. })), 2);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerIsPvpFlagged { .. })), 2);
    assert_eq!(n(&|c| matches!(c, Condition::TargetLevelRange { .. })), 1);
    assert_eq!(n(&|c| matches!(c, Condition::PlayerHasFort { .. })), 1);
    // Nothing else exists in the shipped item files.
    assert_eq!(
        flat.len(),
        1892 + 383 + 261 + 126 + 71 + 39 + 38 + 33 + 29 + 25 + 25 + 22 + 16 + 12 + 8 + 7 + 6 + 2 + 2 + 1 + 1
    );

    // A wolf item uses a category type.
    assert!(flat.iter().any(|c| matches!(
        c,
        Condition::CategoryType { category_types } if category_types.contains(&CategoryType::AllWolfGroup)
    )));
}

#[test]
fn real_skills_match_the_xml() {
    let Some(items) = real_items() else { return };
    assert_eq!(items.iter().filter(|i| i.has_skills()).count(), 5732);
    let entries: usize = items
        .iter()
        .map(|i| i.skills.item_skills.as_ref().map_or(0, Vec::len))
        .sum();
    assert_eq!(entries, 8774);

    let by = |f: &dyn Fn(&ItemTemplate) -> bool| items.iter().filter(|i| f(i)).count();
    assert_eq!(by(&|i| i.skills.unequip_skill.is_some() && i.is_weapon()), 35);
    assert_eq!(by(&|i| i.skills.unequip_skill.is_some() && i.is_armor()), 296);
    assert_eq!(by(&|i| i.skills.unequip_skill.is_some() && i.is_etc_item()), 1);
    assert_eq!(by(&|i| i.skills.enchant4_skill.is_some() && i.is_weapon()), 480);
    assert_eq!(by(&|i| i.skills.enchant4_skill.is_some() && i.is_armor()), 29);
    assert_eq!(by(&|i| i.skills.enchant4_skill.is_some() && i.is_etc_item()), 0);
    assert_eq!(by(&|i| i.skills.on_crit.is_some()), 170);
    assert_eq!(by(&|i| i.skills.on_magic.is_some()), 128);

    // Cursed Maingauche: bleed on critical, 50 percent.
    assert_eq!(
        items.get(1660).unwrap().skills.on_crit,
        Some(ChanceSkill {
            skill: SkillRef { id: 3005, level: 1 },
            chance: 50
        })
    );
    // Heavy Doom Hammer - Magic Regeneration: 30 percent on magic.
    assert_eq!(
        items.get(4864).unwrap().skills.on_magic,
        Some(ChanceSkill {
            skill: SkillRef { id: 1044, level: 1 },
            chance: 30
        })
    );
    // An agathion bracelet: three item skills and the same one as unequip skill.
    let bracelet = items.get(9605).unwrap();
    assert_eq!(
        bracelet.skills.item_skills,
        Some(vec![
            SkillRef { id: 5414, level: 1 },
            SkillRef { id: 3269, level: 1 },
            SkillRef { id: 3267, level: 1 },
        ])
    );
    assert_eq!(
        bracelet.skills.unequip_skill,
        Some(SkillRef { id: 3267, level: 1 })
    );
}
