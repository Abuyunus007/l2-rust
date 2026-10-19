//! World-level tests of the stat engine: the formula functions, item functions at enter world,
//! enchant bonuses, armor sets, hennas, conditions and the NPC calculators.
//!
//! Every expected number is hand-checked from the Java code named next to it; the Java vectors
//! (M4-JV-01, `tests/stats_vectors.rs`) check the real datapack.

use std::sync::Arc;

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, Stat};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::{ObjectId, WorldRng};

use super::calculators::{Func, FuncOwner};
use super::{BaseStat, BaseStatBonus, StatData};
use crate::command::{LeaveReason, SessionId, WorldCommand};
use crate::data::{AccessLevelStatic, ItemStatic, StaticData};
use crate::outbox::{LsEvent, Outbox, PacketSink, PersistCommand};
use crate::player::{ItemData, PlayerData, PlayerProfile};
use crate::world::World;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

struct NoSink;

impl PacketSink for NoSink {
    fn send(&self, _bytes: Vec<u8>) {}
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Class 0: base P. Atk. 4, M. Atk. 6, P. Def. slots 31/18/12/7/8/3/1 (sum 80), M. Def. slots
/// 9/8/5/(5)/13 (sum 40), 100 HP / 50 MP / 20 CP at level 1.
const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const ITEMS: &str = r#"<list>
<item id="1" type="Weapon" name="S Bow"><set name="crystal_type" val="S" /><set name="weapon_type" val="BOW" /><set name="bodypart" val="lrhand" />
<stats><stat type="pAtk">100</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">293</stat></stats></item>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="3" type="Armor" name="D Breastplate"><set name="crystal_type" val="D" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" />
<stats><stat type="pDef">40</stat><stat type="maxMp">10</stat></stats></item>
<item id="4" type="Armor" name="D Gaiters"><set name="crystal_type" val="D" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="legs" />
<stats><stat type="pDef">25</stat></stats></item>
<item id="5" type="Armor" name="Shield"><set name="crystal_type" val="D" /><set name="bodypart" val="lhand" />
<stats><stat type="sDef">50</stat></stats></item>
<item id="6" type="Armor" name="Ring"><set name="bodypart" val="rfinger;lfinger" />
<stats><stat type="mDef">10</stat></stats></item>
<item id="7" type="Armor" name="Robe"><set name="armor_type" val="MAGIC" /><set name="bodypart" val="fullarmor" />
<stats><stat type="pDef">30</stat></stats></item>
<item id="8" type="Armor" name="Light top"><set name="armor_type" val="LIGHT" /><set name="bodypart" val="chest" />
<stats><stat type="pDef">20</stat></stats></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(
            ArmorSets::from_xml(r#"<list><set id="1"><chest id="3" /><legs id="4" /><str val="2" /><con val="-1" /></set></list>"#)
                .unwrap(),
        ),
        enchant_hp_bonus: Arc::new(
            EnchantHpBonus::from_xml(r#"<list><enchantHP grade="D"><bonus>3</bonus><bonus>9</bonus><bonus>26</bonus></enchantHP></list>"#)
                .unwrap(),
        ),
        hennas: Arc::new(
            Hennas::from_xml(
                r#"<list>
<henna dyeId="1" dyeName="a" dyeItemId="4445" dyeLevel="1"><stats str="3" con="-1" dex="0" int="0" men="0" wit="0" /><wear count="1" fee="1" /><cancel count="1" fee="1" /><classId>0</classId></henna>
<henna dyeId="2" dyeName="b" dyeItemId="4446" dyeLevel="1"><stats str="4" con="0" dex="0" int="0" men="0" wit="0" /><wear count="1" fee="1" /><cancel count="1" fee="1" /><classId>0</classId></henna>
<henna dyeId="3" dyeName="c" dyeItemId="4447" dyeLevel="1"><stats str="-1" con="0" dex="0" int="0" men="0" wit="0" /><wear count="1" fee="1" /><cancel count="1" fee="1" /><classId>0</classId></henna>
</list>"#,
            )
            .unwrap(),
        ),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
    }
}

fn item(object_id: i32, item_id: i32, slot: usize, enchant: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        enchant_level: enchant,
        loc: "PAPERDOLL".into(),
        loc_data: i32::try_from(slot).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }
}

fn player(items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ObjectId(1),
        account: "acc".into(),
        name: "P".into(),
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
        x: 1000,
        y: 1000,
        z: 0,
        heading: 0,
        cur_hp: 5000.0,
        max_hp: 5000.0,
        cur_mp: 30.0,
        max_mp: 5000.0,
        cur_cp: -3.0,
        max_cp: 5000.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn world_with(data: StatData, p: PlayerData) -> World {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    world.set_static_data(Box::new(Data(data)));
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(p),
        sink: Box::new(NoSink),
        ip: None,
    });
    world
}

const ME: ObjectId = ObjectId(1);

#[test]
fn naked_player_follows_the_formulas() {
    let world = world_with(stat_data(), player(vec![]));
    let s = world.stats(ME).unwrap();
    // No bonus table: every bonus is 1. levelMod = (1 + 89) / 100 = 0.9.
    assert_eq!(s.level_mod(), 0.9);
    assert_eq!((s.str(), s.dex(), s.con()), (40, 30, 43));
    // FuncPAtkMod 4 * 1 * 0.9 = 3.6; FuncMAtkMod 6 * 1 * 0.81 = 4.86.
    assert_eq!(s.p_atk(None), 4.0 * 0.9);
    assert_eq!(s.m_atk(None, None), 6.0 * (1.0 * 1.0) * (0.9 * 0.9));
    // FuncPDefMod: nothing worn, 80 * 0.9; FuncMDefMod 40 * 1 * 0.9.
    assert_eq!(s.p_def(None), 80.0 * 0.9);
    assert_eq!(s.m_def(None, None), 40.0 * 0.9);
    // FuncAtkAccuracy / FuncAtkEvasion: sqrt(30) * 6 + 1 = 33.86 -> 34.
    assert_eq!((s.accuracy(), s.evasion_rate(None)), (34, 34));
    // FuncAtkCritical 4 * 1 * 10 = 40, (int) (40 + .5).
    assert_eq!(s.critical_hit(None, None), 40);
    // FuncMAtkCritical: a player without a weapon keeps 1; (int) 1 * 10.
    assert_eq!(s.m_critical_hit(None, None), 10);
    assert_eq!((s.p_atk_spd(), s.m_atk_spd()), (300.0, 333));
    // Max HP/MP/CP of level 1, and the stored values clamped to them (setCurrentCp: max(0, cp)).
    assert_eq!((s.max_hp(), s.max_mp(), s.max_cp()), (100, 50, 20));
    let core = &world.player(ME).unwrap().core;
    assert_eq!((core.cur_hp, core.cur_mp, core.cur_cp), (100.0, 30.0, 0.0));
    assert_eq!((core.max_hp, core.max_mp, core.max_cp), (100.0, 50.0, 20.0));
    // getMaxLoad: floor(1 * 69000 * 1).
    assert_eq!(s.max_load(), 69_000);
    // Unarmed: the fists (item 246) are not in this item table, so there is no active weapon
    // and the range is the template's (Java default 40).
    assert_eq!(s.physical_attack_range(), 40);
    assert_eq!(
        (s.run_speed(), s.walk_speed(), s.move_speed()),
        (120.0, 80.0, 120.0)
    );
    assert_eq!(s.collision_radius(), 9.0);
}

#[test]
fn equipped_items_add_their_functions_with_enchant_bonuses() {
    // S bow +5 (two-handed, bow): enchant 3 + overenchant 2 -> 10 * 3 + 20 * 2 = 70 P. Atk.,
    // M. Atk. S: 4 * 3 + 8 * 2 = 28.
    let world = world_with(stat_data(), player(vec![item(10, 1, paperdoll::RHAND, 5)]));
    let s = world.stats(ME).unwrap();
    assert_eq!(s.p_atk(None), (100.0 + 70.0) * 0.9);
    assert_eq!(s.m_atk(None, None), (40.0 + 28.0) * (0.9 * 0.9));
    assert_eq!(s.p_atk_spd(), 293.0);
    assert_eq!(s.physical_attack_range(), 0, "the weapon has no pAtkRange");

    // C sword +4 (one-handed): 3 * 3 + 6 * 1 = 15 for P. Atk. and M. Atk.
    let world = world_with(stat_data(), player(vec![item(10, 2, paperdoll::RHAND, 4)]));
    let s = world.stats(ME).unwrap();
    assert_eq!(s.p_atk(None), (50.0 + 15.0) * 0.9);
    assert_eq!(s.m_atk(None, None), (40.0 + 15.0) * (0.9 * 0.9));
    // FuncMAtkCritical with a weapon: 1 * WIT bonus 1 * 10 = 10, (int) * 10 = 100.
    assert_eq!(s.m_critical_hit(None, None), 100);
}

#[test]
fn armor_replaces_the_slot_defence_and_gets_enchant_and_hp_bonuses() {
    let items = vec![
        item(10, 3, paperdoll::CHEST, 2),
        item(11, 5, paperdoll::LHAND, 5),
        item(12, 6, paperdoll::RFINGER, 1),
    ];
    let world = world_with(stat_data(), player(items));
    let s = world.stats(ME).unwrap();
    // Chest: 80 - 31 + 40 + enchant 2 = 91, * 0.9.
    assert_eq!(s.p_def(None), (80.0 - 31.0 + 40.0 + 2.0) * 0.9);
    // Ring: 40 - 5 (rfinger) + 10 + enchant 1 = 46, * 0.9.
    assert_eq!(s.m_def(None, None), (40.0 - 5.0 + 10.0 + 1.0) * 0.9);
    // NOTE(java-quirk): the shield's enchant function adds nothing to sDef.
    assert_eq!(s.shld_def(), 50);
    // FuncEnchantHp of the D chest at +2: the second bonus, 9. The shield is D too: +5 -> the
    // last entry, 26. Max MP + 10 from the chest.
    assert_eq!(s.max_hp(), 100 + 9 + 26);
    assert_eq!(s.max_mp(), 60);
    // The functions are owned by the items, in Java's order (order 0 before the formula).
    let funcs = world
        .player(ME)
        .unwrap()
        .core
        .calculators
        .funcs(Stat::PowerDefence);
    assert_eq!(funcs.len(), 3);
    assert_eq!(funcs[0].owner, FuncOwner::Item(ObjectId(10)));
    assert_eq!(funcs[1].owner, FuncOwner::Item(ObjectId(10)));
    assert_eq!(funcs[2].owner, FuncOwner::Std);
    let max_hp = world.player(ME).unwrap().core.calculators.funcs(Stat::MaxHp);
    assert_eq!(max_hp.iter().map(|f| f.order).collect::<Vec<_>>(), [1, 40, 40]);
}

#[test]
fn full_armor_frees_the_legs_slot_too() {
    let world = world_with(stat_data(), player(vec![item(10, 7, paperdoll::CHEST, 0)]));
    // 80 - chest 31 - legs 18 + 30.
    assert_eq!(
        world.stats(ME).unwrap().p_def(None),
        (80.0 - 31.0 - 18.0 + 30.0) * 0.9
    );
}

#[test]
fn unequip_removes_the_items_functions() {
    let mut world = world_with(stat_data(), player(vec![item(10, 2, paperdoll::RHAND, 0)]));
    assert_eq!(world.stats(ME).unwrap().p_atk(None), 50.0 * 0.9);
    assert_eq!(
        world.on_unequip(ME, ObjectId(10)),
        [
            Stat::PowerAttack,
            Stat::PowerAttack,
            Stat::MagicAttack,
            Stat::MagicAttack,
            Stat::PowerAttackSpeed
        ]
    );
    assert_eq!(world.stats(ME).unwrap().p_atk(None), 4.0 * 0.9);
    assert!(world.on_unequip(ME, ObjectId(10)).is_empty());
    let sword = item(10, 2, paperdoll::RHAND, 0);
    assert_eq!(world.on_equip(ME, &sword).len(), 5);
    assert_eq!(world.stats(ME).unwrap().p_atk(None), 50.0 * 0.9);
}

#[test]
fn a_complete_armor_set_adds_its_base_stats() {
    let full = vec![item(10, 3, paperdoll::CHEST, 0), item(11, 4, paperdoll::LEGS, 0)];
    let world = world_with(stat_data(), player(full));
    let s = world.stats(ME).unwrap();
    assert_eq!((s.str(), s.con()), (42, 42));
    let half = vec![item(10, 3, paperdoll::CHEST, 0)];
    let world = world_with(stat_data(), player(half));
    assert_eq!(world.stats(ME).unwrap().str(), 40);
}

#[test]
fn hennas_are_capped_at_five_per_stat_like_java() {
    let mut p = player(vec![]);
    p.profile.hennas = [1, 2, 3];
    let world = world_with(stat_data(), p);
    let s = world.stats(ME).unwrap();
    // recalcHennaStats: 3, then 3 + 4 > 5 -> 5, then 5 - 1 = 4 (not above 5, so it counts).
    assert_eq!(s.henna_stats().unwrap().str, 4);
    assert_eq!((s.str(), s.con()), (44, 42));
}

#[test]
fn base_stat_bonuses_come_from_the_table() {
    let mut bonus = BaseStatBonus::loaded();
    assert!(bonus.set(BaseStat::Str, 40, 1.2));
    assert!(bonus.set(BaseStat::Con, 43, 1.5));
    let data = StatData {
        base_stat_bonus: Arc::new(bonus),
        ..stat_data()
    };
    let world = world_with(data, player(vec![]));
    let s = world.stats(ME).unwrap();
    assert_eq!(s.p_atk(None), 4.0 * 1.2 * 0.9);
    assert_eq!(s.max_hp(), 150);
    assert_eq!(s.max_load(), 103_500);
    // DEX 30 is not in the table: bonus 0, so the attack speed falls to 0 and calcStat makes 1.
    assert_eq!(s.p_atk_spd(), 1.0);
}

#[test]
fn template_functions_test_their_conditions() {
    use l2_data::stats::node::parse_xml;
    use l2_data::stats::{ConditionParser, func::parse_func_element};

    let no_tables = |_: &str| None;
    let parser = ConditionParser::new("t.xml", &no_tables);
    let funcs = |xml: &str| parse_func_element(&parser, &parse_xml(xml).unwrap(), None).unwrap();
    let mut world = world_with(stat_data(), player(vec![item(10, 8, paperdoll::CHEST, 0)]));
    let owner = FuncOwner::Skill { id: 142, level: 1 };
    let add = |world: &mut World, xml: &str| {
        let f: Vec<Func> = funcs(xml).iter().map(|t| Func::from_template(t, owner)).collect();
        world.player_mut(ME).unwrap().core.calculators.add_all(f);
    };
    // Light chest without legs is not "LIGHT" (ConditionUsingItemType: chest and legs must
    // match unless it is a full armor), but it is "not MAGIC".
    add(
        &mut world,
        r#"<add stat="rEvas" val="3"><using kind="LIGHT"/></add>"#,
    );
    add(
        &mut world,
        r#"<add stat="accCombat" val="5"><not><using kind="MAGIC"/></not></add>"#,
    );
    add(
        &mut world,
        r#"<mul stat="pAtkSpd" val="2"><player level="2"/></mul>"#,
    );
    add(&mut world, r#"<div stat="mAtkSpd" val="0"/>"#);
    let s = world.stats(ME).unwrap();
    assert_eq!(s.evasion_rate(None), 34);
    assert_eq!(s.accuracy(), 39);
    assert_eq!(s.p_atk_spd(), 300.0);
    // FuncDiv by 0 is an infinity in Java too, capped by MaxMAtkSpeed.
    assert_eq!(s.m_atk_spd(), 1999);
    assert_eq!(
        world
            .player_mut(ME)
            .unwrap()
            .core
            .calculators
            .remove_owner(owner)
            .len(),
        4
    );
}

#[test]
fn npcs_share_the_standard_calculators() {
    let world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    let a = world.npc_calculators();
    let b = world.npc_calculators();
    assert!(a.is_shared() && b.is_shared());
    assert_eq!(a, b);
    assert_eq!(a.set().len(), 13);
}
