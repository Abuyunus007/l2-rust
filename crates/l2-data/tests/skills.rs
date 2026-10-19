#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Skill templates (M4-DATA-11).
//!
//! The first half builds skills from inline XML and checks `DocumentSkill`'s rules one by one:
//! tables, `#` lookups per level and per enchant route, inline level values, the effect list
//! fallbacks of enchant levels (with Java's copy-paste quirks) and the `getFirstChild()` quirk of
//! `parseTemplate`. The second half loads the real datapack (skipped when the folder does not
//! exist, e.g. in CI); its numbers come from an independent script over the XML (sum of `levels`
//! plus the sizes of the enchant groups, the maximum plain level per id).
//! The folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::skill::{
    EffectScope, EnchantSkillGroups, ParamValue, SetValue, SkillLoadOptions, SkillOperateType, SkillTemplate,
    SkillTemplates, TargetType, TraitType,
};
use l2_data::stats::enums::AbnormalType;
use l2_data::stats::{Condition, FuncKind, Stat};

fn groups(pairs: &[(i32, usize)]) -> EnchantSkillGroups {
    EnchantSkillGroups::new(pairs.iter().copied())
}

fn load_with(skills: &str, groups: &EnchantSkillGroups) -> SkillTemplates {
    SkillTemplates::from_xml(
        &format!(r#"<?xml version="1.0" encoding="UTF-8"?><list>{skills}</list>"#),
        groups,
        &SkillLoadOptions::default(),
    )
}

fn load(skills: &str) -> SkillTemplates {
    load_with(skills, &groups(&[]))
}

fn int(skill: &SkillTemplate, key: &str) -> Option<i64> {
    match skill.set.get(key) {
        Some(SetValue::Long(v)) => Some(*v),
        Some(SetValue::Int(v)) => Some(i64::from(*v)),
        _ => None,
    }
}

// ---- plain levels ----

#[test]
fn tables_values_and_inline_level_tables_per_level() {
    let t = load(
        r##"<skill id="10" levels="3" name="Test Skill">
            <!-- comments are dropped -->
            <table name="#mp">10 20 30</table>
            <table name="#hit">1000 900 800</table>
            <icon>icon.skill0010</icon>
            <operateType>A1</operateType>
            <targetType>ONE</targetType>
            <mpConsume>#mp</mpConsume>
            <hitTime>#hit</hitTime>
            <castRange>600</castRange>
            <reuseDelay levelValues="5000 6000 7000"/>
            <power levelValues="1.5 2.5 3.5"/>
            <isMagic>1</isMagic>
            <set name="coolTime" val="#hit"/>
            <set name="effectPoint">-50</set>
            <set name="trait">SHOCK</set>
            <set name="hpConsume"></set>
            <set val="5"/>
            <nextActionAttack>true</nextActionAttack>
            <abnormalType>STUN</abnormalType>
            <afterEffectId>1234</afterEffectId>
        </skill>"##,
    );
    assert_eq!(t.len(), 3);
    assert_eq!((t.max_level(10), t.is_enchantable(10)), (3, false));
    assert!(t.get(10, 0).is_none() && t.get(10, 4).is_none());
    for (level, mp, hit, reuse, power) in [
        (1, 10, 1000, 5000, 1.5),
        (2, 20, 900, 6000, 2.5),
        (3, 30, 800, 7000, 3.5),
    ] {
        let s = t.get(10, level).unwrap();
        assert_eq!((s.id, s.level, s.name.as_str()), (10, level, "Test Skill"));
        assert_eq!((s.mp_consume, s.hit_time, s.cool_time), (mp, hit, hit));
        assert_eq!((s.reuse_delay, s.power), (reuse, power));
        assert_eq!(s.pvp_power, power);
        assert_eq!((s.cast_range, s.effect_range, s.effect_point), (600, -1, -50));
        assert_eq!(s.operate_type, SkillOperateType::A1);
        assert!(s.is_magic() && !s.is_passive() && s.is_active());
        assert_eq!(s.target_type, TargetType::One);
        assert_eq!(s.trait_type, TraitType::Shock);
        assert_eq!(s.abnormal_type, AbnormalType::Stun);
        assert!(s.next_action_is_attack);
        assert_eq!(s.icon, "icon.skill0010");
        // An empty `<set>` and a `<set>` without a name write nothing; unread keys stay in the set.
        assert_eq!(s.hp_consume, 0);
        assert_eq!(int(s, "afterEffectId"), None, "kept as text, not a number");
        assert_eq!(s.set.get("afterEffectId"), Some(&SetValue::Str("1234".into())));
        assert_eq!(s.set.len(), 1);
    }
}

#[test]
fn table_errors_and_short_tables_give_empty_values() {
    // A table with fewer entries than levels: Java logs "Wrong level count" and stores "".
    let t = load(
        r##"<skill id="11" levels="3" name="Short">
            <table name="#mp">10 20</table>
            <operateType>A1</operateType>
            <mpConsume>#mp</mpConsume>
            <castRange>#missing</castRange>
        </skill>"##,
    );
    assert_eq!(
        t.len(),
        0,
        "the empty value fails the integer read, which ends the file"
    );

    // Stored as "" for a text value that is never read as a number.
    let t = load(
        r##"<skill id="12" levels="2" name="Short">
            <table name="#names">x</table>
            <operateType>A1</operateType>
            <attribute>#names</attribute>
        </skill>"##,
    );
    assert_eq!(t.get(12, 1).unwrap().attribute, "x");
    assert_eq!(t.get(12, 2).unwrap().attribute, "");
}

#[test]
fn defaults_of_the_skill_constructor() {
    let t = load(r#"<skill id="13" levels="1" name="Bare"><operateType>P</operateType></skill>"#);
    let s = t.get(13, 1).unwrap();
    assert_eq!((s.display_id, s.display_level), (13, 1));
    assert_eq!(
        (s.cast_range, s.effect_range, s.activate_rate, s.element),
        (-1, -1, -1, -1)
    );
    assert_eq!((s.min_chance, s.max_chance), (10, 90));
    assert!(s.stay_on_subclass_change && s.can_be_dispeled && !s.is_debuff);
    assert_eq!(s.target_type, TargetType::SelfTarget);
    assert!(s.is_passive() && s.effects.is_empty() && s.pre_conditions.is_empty());
    assert_eq!(s.icon, "icon.skill0000");
    assert_eq!(
        (s.channeling_tick_interval, s.channeling_tick_initial_delay),
        (2000, 2000)
    );
}

#[test]
fn a_skill_without_operate_type_ends_the_file() {
    // The constructor throws, `currentSkills.get(i)` then fails: the skill and everything after
    // it in the file are lost, the skills before it stay.
    let t = load(
        r#"<skill id="1" levels="2" name="Good"><operateType>P</operateType></skill>
           <skill id="2" levels="1" name="NoType"/>
           <skill id="3" levels="1" name="Never"><operateType>P</operateType></skill>"#,
    );
    assert_eq!((t.len(), t.loaded_count()), (2, 2));
    assert!(t.get(1, 2).is_some() && t.get(2, 1).is_none() && t.get(3, 1).is_none());
}

#[test]
fn other_parse_errors_also_end_the_file() {
    for broken in [
        // Unknown stat in a function.
        r#"<skill id="2" levels="1" name="B"><operateType>P</operateType><effects><effect name="Buff"><add stat="nope" val="1"/></effect></effects></skill>"#,
        // Nested effect.
        r#"<skill id="2" levels="1" name="B"><operateType>P</operateType><effects><effect name="Buff"><effect name="X"/></effect></effects></skill>"#,
        // A table without text.
        r##"<skill id="2" levels="1" name="B"><table name="#t"/><operateType>P</operateType></skill>"##,
        // A table name without the hash.
        r#"<skill id="2" levels="1" name="B"><table name="t">1</table><operateType>P</operateType></skill>"#,
        // Missing attribute.
        r#"<skill id="2" name="B"><operateType>P</operateType></skill>"#,
        // Effect without a name.
        r#"<skill id="2" levels="1" name="B"><operateType>P</operateType><effects><effect/></effects></skill>"#,
        // Bad affectLimit.
        r#"<skill id="2" levels="1" name="B"><operateType>P</operateType><affectLimit>5</affectLimit></skill>"#,
    ] {
        let t = load(&format!(
            r#"<skill id="1" levels="1" name="A"><operateType>P</operateType></skill>{broken}
               <skill id="3" levels="1" name="C"><operateType>P</operateType></skill>"#
        ));
        assert_eq!(t.len(), 1, "{broken}");
        assert!(t.get(1, 1).is_some(), "{broken}");
    }
}

#[test]
fn later_definitions_replace_earlier_ones_but_both_count() {
    let t = load(
        r#"<skill id="1" levels="1" name="First"><operateType>P</operateType></skill>
           <skill id="1" levels="1" name="Second"><operateType>A1</operateType></skill>"#,
    );
    assert_eq!((t.len(), t.loaded_count()), (1, 2));
    assert_eq!(t.get(1, 1).unwrap().name, "Second");
}

#[test]
fn get_skill_falls_back_to_the_max_level() {
    let t = load(r#"<skill id="1" levels="3" name="A"><operateType>P</operateType></skill>"#);
    assert_eq!(t.get_skill(1, 2).unwrap().level, 2);
    assert_eq!(t.get_skill(1, 9).unwrap().level, 3);
    assert!(t.get_skill(1, 0).is_none() && t.get_skill(7, 1).is_none());
    assert_eq!(SkillTemplates::hash_code(1, 3), 1024);
}

// ---- enchant routes ----

#[test]
fn enchant_levels_take_the_last_level_then_their_own_overrides() {
    let t = load_with(
        r##"<skill id="20" levels="2" name="Ench" enchantGroup1="1">
            <table name="#power">10 20</table>
            <table name="#ench1Power">100 110 120</table>
            <table name="#ench1Mp">5 6 7</table>
            <operateType>A1</operateType>
            <power>#power</power>
            <mpConsume>8</mpConsume>
            <hitTime>#power</hitTime>
            <castRange levelValues="400 500"/>
            <enchant1 name="power">#ench1Power</enchant1>
            <enchant1 name="mpConsume">#ench1Mp</enchant1>
            <enchant2 name="power">999</enchant2>
        </skill>"##,
        &groups(&[(1, 3)]),
    );
    assert_eq!(t.len(), 5);
    // `getMaxLevel` ignores enchant levels, `isEnchantable` is set by them.
    assert_eq!((t.max_level(20), t.is_enchantable(20)), (2, true));
    assert_eq!(
        (t.get(20, 1).unwrap().power, t.get(20, 2).unwrap().power),
        (10.0, 20.0)
    );
    assert_eq!(
        (t.get(20, 1).unwrap().cast_range, t.get(20, 2).unwrap().cast_range),
        (400, 500)
    );
    assert_eq!(t.get(20, 2).unwrap().mp_consume, 8);
    for (level, power, mp) in [(101, 100.0, 5), (102, 110.0, 6), (103, 120.0, 7)] {
        let s = t.get(20, level).unwrap();
        assert_eq!((s.power, s.mp_consume), (power, mp), "level {level}");
        // `#power` of an enchant level reads the last plain level (entry 2 of 2).
        assert_eq!(s.hit_time, 20, "level {level}");
        // NOTE(java-quirk): an element with `levelValues` but no `subLevel1Values` goes through
        // parseElementValue in the enchant loop, which finds no text and sets nothing.
        assert_eq!(s.cast_range, -1, "level {level}");
        assert_eq!(s.name, "Ench");
    }
    assert!(t.get(20, 104).is_none() && t.get(20, 201).is_none());
}

#[test]
fn sub_level_values_and_route_numbers() {
    let t = load_with(
        r##"<skill id="21" levels="1" name="Two Routes" enchantGroup1="1" enchantGroup2="1">
            <operateType>A1</operateType>
            <castRange levelValues="400" subLevel1Values="700 710" subLevel2Values="800 810"/>
            <enchant2 name="mpConsume">#t</enchant2>
            <table name="#t">3 4</table>
        </skill>"##,
        &groups(&[(1, 2)]),
    );
    assert_eq!(t.len(), 5);
    assert_eq!(t.get(21, 1).unwrap().cast_range, 400);
    assert_eq!(
        [101, 102, 201, 202].map(|l| t.get(21, l).unwrap().cast_range),
        [700, 710, 800, 810]
    );
    assert_eq!([201, 202].map(|l| t.get(21, l).unwrap().mp_consume), [3, 4]);
    assert_eq!([101, 102].map(|l| t.get(21, l).unwrap().mp_consume), [0, 0]);
}

#[test]
fn a_missing_enchant_group_gives_no_enchant_levels() {
    let t = load_with(
        r#"<skill id="22" levels="1" name="X" enchantGroup1="9"><operateType>P</operateType></skill>"#,
        &groups(&[(1, 2)]),
    );
    assert_eq!(t.len(), 1);
    assert!(!t.is_enchantable(22));
}

// ---- effects and conditions ----

fn effect_names(skill: &SkillTemplate, scope: EffectScope) -> Vec<&str> {
    skill.effects.get(scope).iter().map(|e| e.name.as_str()).collect()
}

fn pre_condition_levels(skill: &SkillTemplate) -> Vec<i32> {
    skill
        .pre_conditions
        .iter()
        .filter_map(|c| match &c.condition {
            Condition::PlayerLevel { level } => Some(*level),
            _ => None,
        })
        .collect()
}

#[test]
fn enchant_levels_use_their_own_lists_and_fall_back_per_kind() {
    let t = load_with(
        r##"<skill id="30" levels="2" name="Eff" enchantGroup1="1" enchantGroup2="1">
            <table name="#pAtk">2 3</table>
            <table name="#ench">7 8</table>
            <operateType>P</operateType>
            <effects>
                <effect name="Buff"><add stat="pAtk">#pAtk</add></effect>
            </effects>
            <startEffects><effect name="Start"/></startEffects>
            <selfEffects><effect name="Self"/></selfEffects>
            <enchant1effects>
                <effect name="EnchBuff"><add stat="pAtk">#ench</add></effect>
            </enchant1effects>
            <conditions msgId="113" addName="1"><player level="40"/></conditions>
            <enchant1conditions><player level="76"/></enchant1conditions>
        </skill>"##,
        &groups(&[(1, 2)]),
    );
    let pat = |skill: &SkillTemplate, scope| -> Vec<f64> {
        skill
            .effects
            .get(scope)
            .iter()
            .flat_map(|e| e.funcs.iter().map(|f| f.value))
            .collect()
    };

    // Plain levels: the passive list (not GENERAL), the `#pAtk` entry of the level.
    let l1 = t.get(30, 1).unwrap();
    assert_eq!(effect_names(l1, EffectScope::Passive), ["Buff"]);
    assert!(l1.effects.get(EffectScope::General).is_empty());
    assert_eq!(pat(l1, EffectScope::Passive), [2.0]);
    assert_eq!(pat(t.get(30, 2).unwrap(), EffectScope::Passive), [3.0]);
    assert_eq!(effect_names(l1, EffectScope::Start), ["Start"]);
    let cond = &l1.pre_conditions;
    assert_eq!(cond.len(), 1);
    assert_eq!(cond[0].condition, Condition::PlayerLevel { level: 40 });
    assert_eq!(
        (cond[0].message.message_id, cond[0].message.add_name),
        (113, true)
    );

    // Route 1 defines effects and conditions itself: those replace the plain ones; the kinds it
    // does not define (start, self) come from the plain list; its tables use the enchant index.
    for (level, ench) in [(101, 7.0), (102, 8.0)] {
        let s = t.get(30, level).unwrap();
        assert_eq!(
            effect_names(s, EffectScope::Passive),
            ["EnchBuff"],
            "level {level}"
        );
        assert_eq!(pat(s, EffectScope::Passive), [ench]);
        assert_eq!(effect_names(s, EffectScope::Start), ["Start"]);
        assert_eq!(effect_names(s, EffectScope::SelfScope), ["Self"]);
        assert_eq!(pre_condition_levels(s), [76]);
    }

    // Route 2 defines nothing: everything comes from the plain list, read at the LAST plain level
    // (`#pAtk` entry 2), except `startEffects`.
    // NOTE(java-quirk): the route 2 fallback loop has no `startEffects` branch.
    for level in [201, 202] {
        let s = t.get(30, level).unwrap();
        assert_eq!(effect_names(s, EffectScope::Passive), ["Buff"]);
        assert_eq!(pat(s, EffectScope::Passive), [3.0]);
        assert!(s.effects.get(EffectScope::Start).is_empty(), "level {level}");
        assert_eq!(effect_names(s, EffectScope::SelfScope), ["Self"]);
        assert_eq!(pre_condition_levels(s), [40]);
    }
}

#[test]
fn route_seven_guards_start_effects_with_the_channeling_flag() {
    // NOTE(java-quirk): the route 7 fallback takes `startEffects` when `!foundChannelingEffects`.
    let xml = |extra: &str| {
        format!(
            r#"<skill id="31" levels="1" name="R7" enchantGroup7="1">
                <operateType>A2</operateType>
                <startEffects><effect name="Start"/></startEffects>
                {extra}
            </skill>"#
        )
    };
    // Without a route 7 channeling list the start list is copied ...
    let t = load_with(&xml(""), &groups(&[(1, 1)]));
    assert_eq!(
        effect_names(t.get(31, 701).unwrap(), EffectScope::Start),
        ["Start"]
    );
    // ... with one, it is not, although the route has no start list of its own.
    let t = load_with(
        &xml(r#"<enchant7channelingEffects><effect name="Chan"/></enchant7channelingEffects>"#),
        &groups(&[(1, 1)]),
    );
    let s = t.get(31, 701).unwrap();
    assert!(s.effects.get(EffectScope::Start).is_empty());
    assert_eq!(effect_names(s, EffectScope::Channeling), ["Chan"]);
    // An active skill's plain list is GENERAL.
    let t = load(
        r#"<skill id="32" levels="1" name="A"><operateType>A2</operateType><effects><effect name="E"/></effects></skill>"#,
    );
    assert_eq!(effect_names(t.get(32, 1).unwrap(), EffectScope::General), ["E"]);
}

#[test]
fn conditions_inside_an_effects_list_only_count_without_leading_whitespace() {
    // NOTE(java-quirk): `parseTemplate` tests `node.getFirstChild()` for `<conditions>`; in an
    // indented file that is a whitespace text node, so the condition is neither the attach
    // condition nor parsed by the loop (it has no case). Written back to back it is the attach
    // condition of every effect and function of the list.
    let t = load(
        r#"<skill id="40" levels="1" name="Indented">
            <operateType>P</operateType>
            <effects>
                <conditions><player level="40"/></conditions>
                <effect name="Buff">
                    <conditions><player level="50"/></conditions>
                    <add stat="pAtk" val="1"/>
                </effect>
            </effects>
        </skill>
        <skill id="41" levels="1" name="Compact">
            <operateType>P</operateType>
            <effects><conditions msg="no"><player level="40"/></conditions><effect name="Buff"><conditions><player level="50"/></conditions><add stat="pAtk" val="1"/><mul stat="pDef" val="2"/></effect></effects>
        </skill>"#,
    );
    let indented = &t.get(40, 1).unwrap().effects.get(EffectScope::Passive)[0];
    assert_eq!(indented.attach_cond, None);
    assert_eq!(indented.funcs.len(), 1);
    assert_eq!(indented.funcs[0].attach_cond, None);

    let compact = &t.get(41, 1).unwrap().effects.get(EffectScope::Passive)[0];
    assert_eq!(
        compact.attach_cond.as_deref(),
        Some(&Condition::PlayerLevel { level: 40 })
    );
    // Inside the effect the first child is the `<conditions>` element itself too.
    assert_eq!(compact.funcs.len(), 2);
    for func in &compact.funcs {
        assert_eq!(
            func.attach_cond.as_deref(),
            Some(&Condition::PlayerLevel { level: 50 })
        );
    }
}

#[test]
fn effect_templates_hold_attributes_params_funcs_and_conditions() {
    let t = load(
        r##"<skill id="50" levels="2" name="Fx">
            <table name="#power">11 22</table>
            <table name="#val">1.5 2.5</table>
            <operateType>A2</operateType>
            <effects>
                <effect name="Heal" ticks="5" abnormalTime="#power">
                    <using kind="BOW"/>
                    <power>#power</power>
                    <param chance="10" mode="#power"/>
                    <rate><value level="1">30</value><value level="2">60</value></rate>
                    <mul stat="pAtk" val="#val"/>
                    <add stat="rEvas"><value>#val</value><using kind="LIGHT"/></add>
                    <enchant><pDef>0</pDef><mDef>1</mDef></enchant>
                </effect>
            </effects>
        </skill>"##,
    );
    for (level, power, val, rate) in [(1, "11", 1.5, "30"), (2, "22", 2.5, "60")] {
        let e = &t.get(50, level).unwrap().effects.get(EffectScope::General)[0];
        assert_eq!(
            (e.name.as_str(), e.ticks, e.scope),
            ("Heal", 5, EffectScope::General)
        );
        assert_eq!(e.attrs.get("abnormalTime").map(String::as_str), Some(power));
        assert_eq!(e.attrs.get("name").map(String::as_str), Some("Heal"));
        assert_eq!(e.param("power"), Some(power));
        assert_eq!(e.param("chance"), Some("10"));
        assert_eq!(e.param("mode"), Some(power));
        assert_eq!(e.param("rate"), Some(rate));
        // NOTE(java-quirk): a leaf condition element under <effect> is also a parameter.
        assert_eq!(e.param("using"), Some(""));
        assert!(!e.params.contains_key("mul") && !e.params.contains_key("enchant"));

        assert!(matches!(
            e.apply_cond.as_deref(),
            Some(Condition::UsingItemType { .. })
        ));
        let funcs: Vec<_> = e.funcs.iter().map(|f| (f.kind, f.stat, f.value)).collect();
        assert_eq!(
            funcs,
            [
                (FuncKind::Mul, Stat::from_xml("pAtk").unwrap(), val),
                (FuncKind::Add, Stat::from_xml("rEvas").unwrap(), val),
                (FuncKind::Enchant, Stat::from_xml("pDef").unwrap(), 0.0),
                (FuncKind::Enchant, Stat::from_xml("mDef").unwrap(), 1.0),
            ]
        );
        assert!(matches!(
            e.funcs[1].apply_cond.as_deref(),
            Some(Condition::UsingItemType { .. })
        ));
    }
}

#[test]
fn restoration_random_items_are_a_parameter() {
    let t = load(
        r#"<skill id="60" levels="2" name="Box">
            <operateType>A1</operateType>
            <effects>
                <effect name="RestorationRandom">
                    <items>
                        <value level="1"><item chance="40"><item id="57" count="10"/></item></value>
                        <value level="2"><item><item id="5" minEnchant="1"/></item><item chance="5"><item id="6"/></item></value>
                    </items>
                </effect>
            </effects>
        </skill>"#,
    );
    let items = |level| match t.get(60, level).unwrap().effects.get(EffectScope::General)[0]
        .params
        .get("items")
    {
        Some(ParamValue::Items(groups)) => groups.clone(),
        other => panic!("{other:?}"),
    };
    let level1 = items(1);
    assert_eq!(
        (level1.len(), level1[0].chance, level1[0].items[0].id),
        (1, 40.0, 57)
    );
    assert_eq!(level1[0].items[0].count, 10);
    let level2 = items(2);
    assert_eq!(level2.len(), 2);
    assert_eq!((level2[0].chance, level2[1].chance), (100.0, 5.0));
    assert_eq!(
        (level2[0].items[0].min_enchant, level2[0].items[0].max_enchant),
        (1, 0)
    );
}

#[test]
fn skill_conditions_come_per_level_in_file_order() {
    let t = load(
        r##"<skill id="70" levels="2" name="Cond">
            <table name="#lvl">10 20</table>
            <operateType>A1</operateType>
            <conditions msg="Not now"><player level="#lvl"/></conditions>
            <conditions><using kind="BOW"/></conditions>
            <conditions><game nothing="1"/></conditions>
        </skill>"##,
    );
    for (level, required) in [(1, 10), (2, 20)] {
        let s = t.get(70, level).unwrap();
        assert_eq!(s.pre_conditions.len(), 2, "the unrecognised block is dropped");
        assert_eq!(
            s.pre_conditions[0].condition,
            Condition::PlayerLevel { level: required }
        );
        assert_eq!(s.pre_conditions[0].message.message.as_deref(), Some("Not now"));
    }
}

// ---- files and options ----

#[test]
fn files_load_in_path_order_and_custom_only_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join("data/stats/skills");
    std::fs::create_dir_all(skills.join("custom")).unwrap();
    let doc = |body: &str| format!(r#"<?xml version="1.0" encoding="UTF-8"?><list>{body}</list>"#);
    let skill = |id: i32, name: &str| {
        format!(r#"<skill id="{id}" levels="2" name="{name}"><operateType>P</operateType></skill>"#)
    };
    std::fs::write(
        skills.join("a.xml"),
        doc(&format!("{}{}", skill(1, "A1"), skill(2, "A2"))),
    )
    .unwrap();
    std::fs::write(skills.join("b.xml"), doc(&skill(1, "B1"))).unwrap();
    std::fs::write(skills.join("notes.txt"), "ignored").unwrap();
    std::fs::write(skills.join("custom/c.xml"), doc(&skill(3, "C3"))).unwrap();
    std::fs::write(
        dir.path().join("data/EnchantSkillGroups.xml"),
        r#"<list><group id="1"><enchant level="1"/></group></list>"#,
    )
    .unwrap();

    let plain = SkillTemplates::load(dir.path(), &SkillLoadOptions::default());
    assert_eq!((plain.len(), plain.loaded_count()), (4, 6));
    assert_eq!(plain.get(1, 1).unwrap().name, "B1", "the later path wins");
    assert_eq!(plain.get(2, 2).unwrap().name, "A2");
    assert!(plain.get(3, 1).is_none());
    let order: Vec<_> = plain.iter().map(|s| (s.id, s.level)).collect();
    assert_eq!(order, [(1, 1), (1, 2), (2, 1), (2, 2)]);

    let custom = SkillTemplates::load(
        dir.path(),
        &SkillLoadOptions {
            custom_skills_load: true,
            ..SkillLoadOptions::default()
        },
    );
    assert_eq!(custom.get(3, 2).unwrap().name, "C3");
    assert_eq!(custom.len(), 6);

    // The same table every time, whatever the thread scheduling does.
    let again = SkillTemplates::load(dir.path(), &SkillLoadOptions::default());
    assert!(plain.iter().zip(again.iter()).all(|(x, y)| x == y));
}

// ---- the real datapack ----

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

fn real() -> Option<SkillTemplates> {
    Some(SkillTemplates::load(&game_dir()?, &SkillLoadOptions::default()))
}

fn funcs_of(skill: &SkillTemplate) -> Vec<(FuncKind, Stat, f64)> {
    skill
        .effects
        .get(EffectScope::Passive)
        .iter()
        .flat_map(|e| e.funcs.iter())
        .map(|f| (f.kind, f.stat, f.value))
        .collect()
}

fn stat(name: &str) -> Stat {
    Stat::from_xml(name).unwrap()
}

#[test]
fn real_datapack_counts() {
    let Some(skills) = real() else { return };
    // 8,136 skills with 30,227 plain levels, plus the enchant levels of groups 1, 2, 5, 6
    // (30, 30, 15, 15 details) as counted from the XML.
    assert_eq!(skills.len(), 65_957);
    assert_eq!(skills.loaded_count(), 65_957);
    let ids = skills
        .iter()
        .map(|s| s.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 8_136);
    assert_eq!(ids.iter().filter(|&&id| skills.is_enchantable(id)).count(), 526);

    // (id, highest plain level) counted from the `levels` attributes.
    for (id, max) in [
        (1, 37),
        (3, 9),
        (10, 8),
        (30, 37),
        (100, 15),
        (141, 3),
        (142, 5),
        (1040, 3),
        (1177, 5),
        (1204, 2),
        (1245, 14),
        (4408, 20),
        (5104, 3),
        (9999, 0),
    ] {
        assert_eq!(skills.max_level(id), max, "skill {id}");
    }
    assert!(skills.is_enchantable(1204) && !skills.is_enchantable(141));
    assert!(skills.get(1, 0).is_none() && skills.get(141, 4).is_none());
}

#[test]
fn real_weapon_mastery_and_armor_mastery() {
    let Some(skills) = real() else { return };
    for (level, add, magic_level) in [(1, 2.0, 5), (2, 3.0, 10), (3, 4.0, 15)] {
        let s = skills.get(141, level).unwrap();
        assert_eq!(
            (s.name.as_str(), s.operate_type),
            ("Weapon Mastery", SkillOperateType::P)
        );
        assert_eq!(s.magic_level, magic_level);
        assert_eq!(
            funcs_of(s),
            [
                (FuncKind::Mul, stat("pAtk"), 1.085),
                (FuncKind::Add, stat("pAtk"), add)
            ],
            "level {level}"
        );
        assert_eq!(s.effects.get(EffectScope::Passive)[0].name, "Buff");
    }

    // 142 Armor Mastery level 4: `add rEvas 3` only while wearing light armor.
    let s = skills.get(142, 4).unwrap();
    let funcs = funcs_of(s);
    assert_eq!(funcs.len(), 3);
    assert_eq!(funcs[0], (FuncKind::Add, stat("pDef"), 13.0));
    assert_eq!(funcs[1], (FuncKind::Mul, stat("regMp"), 1.1));
    let evas = &s.effects.get(EffectScope::Passive)[0].funcs[2];
    assert_eq!((evas.kind, evas.value), (FuncKind::Add, 3.0));
    assert_eq!(evas.stat, Stat::from_xml("rEvas").unwrap());
    assert!(matches!(evas.apply_cond.as_deref(), Some(Condition::UsingItemType { mask }) if *mask != 0));
    // Levels 1..3 add 0 evasion.
    assert_eq!(
        skills.get(142, 1).unwrap().effects.get(EffectScope::Passive)[0].funcs[2].value,
        0.0
    );
}

#[test]
fn real_hp_increase_table() {
    let Some(skills) = real() else { return };
    let s = skills.get(4408, 9).unwrap();
    let funcs = funcs_of(s);
    assert_eq!(
        funcs,
        [
            (FuncKind::Mul, stat("maxHp"), 0.5),
            (FuncKind::Mul, stat("regHp"), 0.5),
        ]
    );
    assert_eq!(s.magic_level, 1);
    assert_eq!(s.icon, "icon.skill4311");
}

#[test]
fn real_stun_attack_levels_and_enchant_route() {
    let Some(skills) = real() else { return };
    // 100 Stun Attack: tables #mpConsume, #power, #magicLevel, #effectPoints; enchantGroup1="2" (30 levels).
    let s = skills.get(100, 1).unwrap();
    assert_eq!(s.name, "Stun Attack");
    assert_eq!(
        (s.mp_consume, s.cast_range, s.hit_time, s.cool_time),
        (19, 40, 1080, 720)
    );
    assert_eq!((s.effect_range, s.reuse_delay, s.activate_rate), (400, 3000, 50));
    assert_eq!((s.power, s.magic_level, s.effect_point), (36.0, 18, -96));
    assert_eq!(s.target_type, TargetType::One);
    assert_eq!(s.abnormal_type, AbnormalType::Stun);
    assert_eq!((s.abnormal_level, s.abnormal_time), (1, 9));
    assert!(s.is_debuff && s.overhit && s.next_action_is_attack && !s.is_magic());
    let s15 = skills.get(100, 15).unwrap();
    assert_eq!((s15.mp_consume, s15.power, s15.magic_level), (33, 131.0, 36));

    // Enchant route 1: `#ench1Power` / `#enchMagicLvl` by enchant index, the rest from level 15.
    let e1 = skills.get(100, 101).unwrap();
    let e30 = skills.get(100, 130).unwrap();
    assert_eq!((e1.power, e1.magic_level), (794.0, 76));
    assert_eq!((e30.power, e30.magic_level), (928.0, 85));
    assert_eq!((e1.mp_consume, e1.cast_range, e1.hit_time), (33, 40, 1080));
    assert!(skills.get(100, 131).is_none() && skills.get(100, 201).is_none());
    assert_eq!(skills.max_level(100), 15);
    // The skill's `<conditions msgId="113" addName="1">` reaches the plain and the enchant levels.
    for s in [s, e1] {
        assert_eq!(s.pre_conditions.len(), 1);
        assert_eq!(
            (
                s.pre_conditions[0].message.message_id,
                s.pre_conditions[0].message.add_name
            ),
            (113, true)
        );
    }
    assert!(!s.effects.get(EffectScope::General).is_empty());
}

#[test]
fn real_every_skill_has_consistent_numbers() {
    let Some(skills) = real() else { return };
    for s in skills.iter() {
        assert_eq!(s.reuse_hash_code, s.id * 1021 + s.level);
        assert!(skills.get(s.id, s.level).is_some_and(|got| got == s));
        // Passive skills keep their effects in the PASSIVE list only.
        if s.is_passive() {
            assert!(s.effects.get(EffectScope::General).is_empty(), "{s}");
        }
    }
}
