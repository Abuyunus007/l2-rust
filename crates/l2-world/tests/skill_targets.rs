#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Skill target handlers (M5-TARGET-01): `World::skill_target_list` and the 34 handlers of
//! `dist/game/data/scripts/handlers/skill/targets`, with creatures around the caster.
//!
//! The scenes use small synthetic templates, so they run in CI. Geometry: the caster stands at
//! `(X, Y, Z)` facing east (heading 0). Monsters have a collision radius of 10, players 9.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use l2_data::skill::{EnchantSkillGroups, TargetType};
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplate, SkillTemplates, SpawnData, Zones,
};
use l2_protocol_game::SystemMessageId;
use l2_world::movement::heading_from_points;
use l2_world::skills::{
    HANDLED_TARGET_TYPES, NOT_HANDLED_TEXT, TargetContext, TargetMessage, TargetResult,
    is_target_type_handled,
};
use l2_world::*;

// ------------------------------------------------------------------ harness

struct Sink;

impl PacketSink for Sink {
    fn send(&self, _bytes: Vec<u8>) {}
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

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

/// Class 0 at level 1; collision radius 9 (male).
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

fn npc_xml(id: i32, ty: &str, ai: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{ty}" level="1" name="N{id}">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    {ai}
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

/// 9001 monster, 9002 folk (not attackable), 9003 chest, 9004 artefact, 9005 monster of clan
/// Orc, 9006 another Orc, 9007 a Dragon, 9008 a monster of clan ALL (the generic clan).
fn npcs() -> String {
    let orc = "<ai><clanList><clan>Orc</clan></clanList></ai>";
    let dragon = "<ai><clanList><clan>Dragon</clan></clanList></ai>";
    let all = "<ai><clanList><clan>ALL</clan></clanList></ai>";
    format!(
        "<?xml version=\"1.0\"?><list>{}{}{}{}{}{}{}{}</list>",
        npc_xml(9001, "Monster", ""),
        npc_xml(9002, "Folk", ""),
        npc_xml(9003, "Chest", ""),
        npc_xml(9004, "Artefact", ""),
        npc_xml(9005, "Monster", orc),
        npc_xml(9006, "Monster", orc),
        npc_xml(9007, "Monster", dragon),
        npc_xml(9008, "Monster", all),
    )
}

fn synthetic_stats() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::default()),
    }
}

fn player_data(id: i32, heading: i32, karma: i32, (x, y, z): (i32, i32, i32)) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
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
        karma,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y,
        z,
        heading,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);
const P3: ObjectId = ObjectId(0x2000_0003);
const P4: ObjectId = ObjectId(0x2000_0004);
const SEED: u64 = 20_261_020;

struct Scene {
    world: World,
}

impl Scene {
    fn new() -> Self {
        let mut world = World::new(WorldRng::from_seed(SEED), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        Self { world }
    }

    /// A scene whose zones come from `zone_xml` (cuboids around the caster).
    fn with_zones(tag: &str, zone_xml: &str) -> Self {
        let mut scene = Self::new();
        let root = TempRoot::new(tag, zone_xml);
        let zones = Arc::new(Zones::load(root.path()));
        let regions = Arc::new(MapRegions::load(root.path()));
        scene.world.set_zone_data(zones, regions);
        scene
    }

    fn enter(&mut self, id: ObjectId, heading: i32, karma: i32, pos: (i32, i32, i32)) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id.0).unwrap()),
            player: Box::new(player_data(id.0, heading, karma, pos)),
            sink: Box::new(Sink),
            ip: None,
        });
    }

    /// The caster, facing east.
    fn me(&mut self) {
        self.enter(ME, 0, 0, (X, Y, Z));
    }

    fn player(&mut self, id: ObjectId, dx: i32, dy: i32) {
        self.enter(id, 0, 0, (X + dx, Y + dy, Z));
    }

    fn npc(&mut self, npc_id: i32, dx: i32, dy: i32) -> ObjectId {
        self.npc_z(npc_id, dx, dy, 0)
    }

    fn npc_z(&mut self, npc_id: i32, dx: i32, dy: i32, dz: i32) -> ObjectId {
        self.world
            .spawn_npc_at(npc_id, X + dx, Y + dy, Z + dz, 0)
            .unwrap()
    }

    fn dead_npc(&mut self, npc_id: i32, dx: i32, dy: i32) -> ObjectId {
        let id = self.npc(npc_id, dx, dy);
        self.world.set_dead(id, true);
        id
    }

    fn cast(
        &mut self,
        skill: &SkillTemplate,
        caster: ObjectId,
        only_first: bool,
        target: Option<ObjectId>,
    ) -> TargetResult {
        self.world
            .skill_target_list(skill, caster, only_first, target, &TargetContext::default())
    }
}

/// A datapack root in the temp folder with `data/zones/test.xml`; removed on drop.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str, zone_xml: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-skilltgt-{tag}-{}", std::process::id()));
        let zones = dir.join("data").join("zones");
        std::fs::create_dir_all(&zones).unwrap();
        std::fs::write(
            zones.join("test.xml"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n{zone_xml}\n</list>\n"
            ),
        )
        .unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cuboid(id: i32, ty: &str) -> String {
    format!(
        "<zone id=\"{id}\" name=\"z{id}\" type=\"{ty}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
        X - 2_000,
        Y - 2_000,
        X + 2_000,
        Y + 2_000
    )
}

/// One skill with the given target type and extra tags (`<castRange>600</castRange>` ...) and
/// effects.
fn skill(target_type: &str, tags: &str, effects: &str) -> SkillTemplate {
    let xml = format!(
        "<list><skill id=\"1\" levels=\"1\" name=\"T\"><operateType>A1</operateType>\
         <targetType>{target_type}</targetType>{tags}<effects>{effects}</effects></skill></list>"
    );
    let table = SkillTemplates::from_xml(&xml, &EnchantSkillGroups::default(), &SkillLoadOptions::default());
    table.get(1, 1).unwrap().clone()
}

const NEGATIVE: &str = "<effectPoint>-100</effectPoint>";
const POSITIVE: &str = "<effectPoint>100</effectPoint>";

fn set(ids: &[ObjectId]) -> BTreeSet<i32> {
    ids.iter().map(|i| i.0).collect()
}

fn set_of(result: &TargetResult) -> BTreeSet<i32> {
    set(&result.targets)
}

fn incorrect() -> Vec<TargetMessage> {
    vec![TargetMessage::System(SystemMessageId::ThatIsAnIncorrectTarget)]
}

// ------------------------------------------------------------------ the table

#[test]
fn thirty_four_target_types_have_a_handler() {
    assert_eq!(HANDLED_TARGET_TYPES.len(), 34);
    let unique: BTreeSet<&str> = HANDLED_TARGET_TYPES.iter().map(|t| t.name()).collect();
    assert_eq!(unique.len(), 34);
    for t in [
        TargetType::AreaUndead,
        TargetType::Corpse,
        TargetType::None,
        TargetType::Undead,
    ] {
        assert!(!is_target_type_handled(t), "{t:?}");
    }
}

#[test]
fn types_without_a_handler_send_the_not_handled_text() {
    let mut s = Scene::new();
    s.me();
    let mob = s.npc(9001, 100, 0);
    for name in ["AREA_UNDEAD", "CORPSE", "NONE", "UNDEAD"] {
        let sk = skill(name, "", "");
        let r = s.cast(&sk, ME, false, Some(mob));
        assert!(r.targets.is_empty(), "{name}");
        assert_eq!(r.messages, vec![TargetMessage::Text(NOT_HANDLED_TEXT)], "{name}");
    }
}

// ------------------------------------------------------------------ single targets

#[test]
fn self_and_one() {
    let mut s = Scene::new();
    s.me();
    let mob = s.npc(9001, 100, 0);
    let dead = s.dead_npc(9001, 120, 0);

    let sk = skill("SELF", NEGATIVE, "");
    assert_eq!(s.cast(&sk, ME, false, Some(mob)).targets, vec![ME]);
    assert_eq!(s.cast(&sk, ME, false, None).targets, vec![ME]);

    let one = skill("ONE", NEGATIVE, "");
    let r = s.cast(&one, ME, false, Some(mob));
    assert_eq!((r.targets, r.messages), (vec![mob], vec![]));
    // No target, a dead target, or yourself with a negative skill: incorrect.
    for target in [None, Some(dead), Some(ME)] {
        let r = s.cast(&one, ME, false, target);
        assert!(r.targets.is_empty(), "{target:?}");
        assert_eq!(r.messages, incorrect(), "{target:?}");
    }
    // Yourself with a positive skill is fine.
    let buff = skill("ONE", POSITIVE, "");
    assert_eq!(s.cast(&buff, ME, false, Some(ME)).targets, vec![ME]);
    // SELF is not a negative target type even with a negative effect point.
    let self_negative = skill("SELF", NEGATIVE, "");
    let one_self = skill("ONE", "<effectPoint>-1</effectPoint>", "");
    assert!(s.cast(&one_self, ME, false, Some(ME)).targets.is_empty());
    assert_eq!(s.cast(&self_negative, ME, false, Some(ME)).targets, vec![ME]);
}

#[test]
fn current_target_is_used_when_none_is_given() {
    let mut s = Scene::new();
    s.me();
    let mob = s.npc(9001, 100, 0);
    let one = skill("ONE", NEGATIVE, "");
    let ctx = TargetContext::default();

    let r = s.world.skill_target_list_current(&one, ME, false, &ctx);
    assert_eq!(r.messages, incorrect(), "nothing selected");

    s.world.set_target(ME, Some(mob));
    let r = s.world.skill_target_list_current(&one, ME, false, &ctx);
    assert_eq!(r.targets, vec![mob]);
    // Java `getFirstOfTargetList`.
    assert_eq!(
        s.world.skill_first_of_target_list(&one, ME, &ctx).first(),
        Some(mob)
    );
}

#[test]
fn corpse_mob_needs_a_dead_attackable() {
    let mut s = Scene::new();
    s.me();
    let alive = s.npc(9001, 100, 0);
    let dead = s.dead_npc(9001, 120, 0);
    let folk = s.dead_npc(9002, 140, 0);
    let sk = skill("CORPSE_MOB", "", "");

    assert_eq!(s.cast(&sk, ME, false, Some(dead)).targets, vec![dead]);
    for target in [None, Some(alive), Some(folk), Some(ME)] {
        let r = s.cast(&sk, ME, false, target);
        assert!(r.targets.is_empty(), "{target:?}");
        assert_eq!(r.messages, incorrect(), "{target:?}");
    }
}

#[test]
fn pc_body_needs_a_dead_player() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    s.player(P3, 150, 0);
    let mob = s.dead_npc(9001, 120, 0);
    s.world.set_dead(P2, true);
    let sk = skill(
        "PC_BODY",
        POSITIVE,
        "<effect name=\"Resurrection\"><power>70</power></effect>",
    );

    assert_eq!(s.cast(&sk, ME, false, Some(P2)).targets, vec![P2]);
    for target in [None, Some(P3), Some(mob)] {
        let r = s.cast(&sk, ME, false, target);
        assert!(r.targets.is_empty(), "{target:?}");
        assert_eq!(r.messages, incorrect(), "{target:?}");
    }
}

#[test]
fn pc_body_resurrection_ignores_a_peace_zone() {
    // The refusal in a siege zone needs `ZoneId::Siege`, which castle zones set (M6 sieges); a
    // peace zone does not stop a resurrection.
    let mut s = Scene::with_zones("pcbody", &cuboid(1, "PeaceZone"));
    s.me();
    s.player(P2, 100, 0);
    s.world.set_dead(P2, true);
    assert!(s.world.is_inside_zone(P2, ZoneId::Peace));
    let sk = skill(
        "PC_BODY",
        POSITIVE,
        "<effect name=\"Resurrection\"><power>70</power></effect>",
    );
    assert_eq!(s.cast(&sk, ME, false, Some(P2)).targets, vec![P2]);
}

#[test]
fn unlockable_holy_and_flag_pole() {
    let mut s = Scene::new();
    s.me();
    let chest = s.npc(9003, 100, 0);
    let mob = s.npc(9001, 120, 0);
    let artefact = s.npc(9004, 140, 0);

    let unlock = skill("UNLOCKABLE", "", "");
    assert_eq!(s.cast(&unlock, ME, false, Some(chest)).targets, vec![chest]);
    for target in [None, Some(mob)] {
        let r = s.cast(&unlock, ME, false, target);
        assert!(r.targets.is_empty());
        assert_eq!(
            r.messages,
            vec![TargetMessage::System(SystemMessageId::InvalidTarget)]
        );
    }

    let holy = skill("HOLY", "", "");
    assert_eq!(s.cast(&holy, ME, false, Some(artefact)).targets, vec![artefact]);
    for target in [None, Some(mob)] {
        let r = s.cast(&holy, ME, false, target);
        assert_eq!((r.targets, r.messages), (vec![], vec![]));
    }

    let flag = skill("FLAGPOLE", "", "");
    assert_eq!(s.cast(&flag, ME, false, Some(mob)).targets, vec![mob]);
    // An NPC caster gets nothing.
    assert!(s.cast(&flag, mob, false, Some(artefact)).targets.is_empty());
}

// ------------------------------------------------------------------ area

fn area_scene() -> (Scene, ObjectId, Vec<ObjectId>) {
    let mut s = Scene::new();
    s.me();
    // The target, 300 east of the caster.
    let t = s.npc(9001, 300, 0);
    let near = s.npc(9001, 400, 0);
    // 215 from the target: inside affect range 200 + the two radii of 10.
    let edge_in = s.npc(9001, 300, 215);
    // 225 from the target: outside.
    let edge_out = s.npc(9001, 300, -225);
    // 100 away on the plane but 300 above: the 3D distance is out of range.
    let high = s.npc_z(9001, 400, 0, 300);
    let dead = s.dead_npc(9001, 350, 0);
    let folk = s.npc(9002, 320, 0);
    (s, t, vec![near, edge_in, edge_out, high, dead, folk])
}

#[test]
fn area_hits_mobs_in_range_of_the_target() {
    let (mut s, t, around) = area_scene();
    let (near, edge_in, _edge_out, _high, _dead, _folk) =
        (around[0], around[1], around[2], around[3], around[4], around[5]);
    let sk = skill(
        "AREA",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );

    let r = s.cast(&sk, ME, false, Some(t));
    assert_eq!(r.targets[0], t, "the target comes first");
    assert_eq!(
        set_of(&r),
        set(&[t, near, edge_in]),
        "range + collision radii, 3D, no corpses, no folk"
    );
    assert!(r.messages.is_empty());

    // onlyFirst stops at the target.
    assert_eq!(s.cast(&sk, ME, true, Some(t)).targets, vec![t]);
}

#[test]
fn area_rejects_bad_targets_with_a_message() {
    let (mut s, t, around) = area_scene();
    let sk = skill(
        "AREA",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );
    let dead = around[4];
    let folk = around[5];
    assert!(
        !s.cast(&sk, ME, false, Some(t)).targets.is_empty(),
        "the target itself is fine"
    );
    for target in [None, Some(ME), Some(dead), Some(folk)] {
        let r = s.cast(&sk, ME, false, target);
        assert!(r.targets.is_empty(), "{target:?}");
        assert_eq!(r.messages, incorrect(), "{target:?}");
    }
}

#[test]
fn area_with_a_negative_cast_range_is_centred_on_the_caster() {
    let mut s = Scene::new();
    s.me();
    let a = s.npc(9001, 150, 0);
    let b = s.npc(9001, -150, 0);
    let far = s.npc(9001, 300, 0);
    let sk = skill(
        "AREA",
        &format!("<castRange>-1</castRange><affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );
    // No target needed, the caster itself is not part of the list.
    let r = s.cast(&sk, ME, false, None);
    // The first test of Java's handler runs even for a negative cast range: a null target fails.
    assert!(r.targets.is_empty());
    assert_eq!(r.messages, incorrect());
    let r = s.cast(&sk, ME, false, Some(far));
    assert_eq!(set_of(&r), set(&[a, b]), "the target {far:?} itself is not added");
}

#[test]
fn area_affect_limit_counts_the_origin_and_draws_from_the_rng() {
    let mut s = Scene::new();
    s.me();
    let t = s.npc(9001, 300, 0);
    for i in 0..5 {
        s.npc(9001, 300 + 10 * (i + 1), 0);
    }
    // limit 2: the origin and one more.
    let sk = skill(
        "AREA",
        &format!(
            "<castRange>600</castRange><affectRange>200</affectRange><affectLimit>2-0</affectLimit>{NEGATIVE}"
        ),
        "",
    );
    let r = s.cast(&sk, ME, false, Some(t));
    assert_eq!(r.targets.len(), 2);
    assert_eq!(r.targets[0], t);

    // limit 1-3: 1 + Rnd.get(3), the draw replays from the seed.
    let sk = skill(
        "AREA",
        &format!(
            "<castRange>600</castRange><affectRange>200</affectRange><affectLimit>1-3</affectLimit>{NEGATIVE}"
        ),
        "",
    );
    let expected = usize::try_from(1 + WorldRng::from_seed(7).below(3)).unwrap();
    let r = s.world.skill_target_list_with(
        &mut WorldRng::from_seed(7),
        &sk,
        ME,
        false,
        Some(t),
        &TargetContext::default(),
    );
    assert_eq!(r.targets.len(), expected);
}

#[test]
fn area_vs_players_follows_check_pvp_skill() {
    let mut s = Scene::new();
    s.me();
    let t = s.npc(9001, 300, 0);
    s.enter(P2, 0, 0, (X + 350, Y, Z)); // peaceful
    s.enter(P3, 0, 5, (X + 360, Y, Z)); // karma
    let negative = skill(
        "AREA",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );
    let r = s.cast(&negative, ME, false, Some(t));
    assert_eq!(
        set_of(&r),
        set(&[t, P3]),
        "a peaceful player is not hit by a negative skill"
    );

    let positive = skill(
        "AREA",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{POSITIVE}"),
        "",
    );
    let r = s.cast(&positive, ME, false, Some(t));
    assert_eq!(set_of(&r), set(&[t, P2, P3]));
}

#[test]
fn check_pvp_skill_rules() {
    let mut s = Scene::new();
    s.me();
    s.enter(P2, 0, 0, (X + 100, Y, Z)); // peaceful
    s.enter(P3, 0, 5, (X + 110, Y, Z)); // karma
    let mob = s.npc(9001, 120, 0);
    let w = &s.world;
    let ctx = TargetContext::default();

    let negative = skill("ONE", NEGATIVE, "");
    assert!(
        w.check_pvp_skill(ME, mob, &negative, &ctx),
        "not a playable: true"
    );
    assert!(!w.check_pvp_skill(ME, ME, &negative, &ctx));
    assert!(!w.check_pvp_skill(ME, P2, &negative, &ctx), "peaceful");
    assert!(w.check_pvp_skill(ME, P3, &negative, &ctx), "karma player");
    let positive = skill("ONE", POSITIVE, "");
    assert!(w.check_pvp_skill(ME, P2, &positive, &ctx));

    // Force attack on a peaceful player: ctrl, the selected target, a damage effect and a
    // positive effect range.
    let hit = skill(
        "ONE",
        &format!("{NEGATIVE}<effectRange>400</effectRange>"),
        "<effect name=\"PhysicalDamage\"><power>10</power></effect>",
    );
    let ctrl = TargetContext {
        ctrl_pressed: true,
        ground: None,
    };
    assert!(!w.check_pvp_skill(ME, P2, &hit, &ctrl), "not selected");
    s.world.set_target(ME, Some(P2));
    assert!(s.world.check_pvp_skill(ME, P2, &hit, &ctrl));
    assert!(!s.world.check_pvp_skill(ME, P2, &hit, &ctx), "without ctrl");
    let no_range = skill(
        "ONE",
        NEGATIVE,
        "<effect name=\"PhysicalDamage\"><power>10</power></effect>",
    );
    assert!(
        !s.world.check_pvp_skill(ME, P2, &no_range, &ctrl),
        "effectRange <= 0"
    );
    // A PvP-only skill needs a flagged target outside the PvP zone.
    let pvp_only = skill("ONE", &format!("{NEGATIVE}<isPvPOnly>true</isPvPOnly>"), "");
    assert!(s.world.check_pvp_skill(ME, P3, &pvp_only, &ctx));
}

#[test]
fn peace_and_arena_zones_decide_area_hits_between_players() {
    // Everyone in a peace zone: a negative skill hits nobody, even with karma.
    let mut s = Scene::with_zones("peace", &cuboid(1, "PeaceZone"));
    s.me();
    s.enter(P3, 0, 5, (X + 100, Y, Z));
    assert!(s.world.is_inside_zone(P3, ZoneId::Peace));
    let aura = skill("AURA", &format!("<affectRange>300</affectRange>{NEGATIVE}"), "");
    assert!(s.cast(&aura, ME, false, None).targets.is_empty());
    // A positive skill ignores the peace zone.
    let buff = skill("AURA", &format!("<affectRange>300</affectRange>{POSITIVE}"), "");
    assert_eq!(s.cast(&buff, ME, false, None).targets, vec![P3]);

    // An arena: the source is in a PvP zone, so unflagged players are fair game.
    let mut s = Scene::with_zones("arena", &cuboid(2, "ArenaZone"));
    s.me();
    s.player(P2, 100, 0);
    assert!(s.world.is_inside_zone(ME, ZoneId::Pvp));
    assert_eq!(s.cast(&aura, ME, false, None).targets, vec![P2]);
}

// ------------------------------------------------------------------ aura

#[test]
fn aura_hits_what_is_in_range_of_the_caster() {
    let mut s = Scene::new();
    s.me();
    let a = s.npc(9001, 100, 0);
    let b = s.npc(9001, -150, 0);
    let _far = s.npc(9001, 250, 0);
    let _dead = s.dead_npc(9001, 50, 0);
    let _folk = s.npc(9002, 60, 0);
    let sk = skill("AURA", &format!("<affectRange>200</affectRange>{NEGATIVE}"), "");

    let r = s.cast(&sk, ME, false, None);
    assert_eq!(set_of(&r), set(&[a, b]));
    assert_eq!(s.cast(&sk, ME, true, None).targets.len(), 1);

    // A playable's positive aura leaves monsters alone but reaches players.
    s.player(P2, 80, 0);
    let buff = skill("AURA", &format!("<affectRange>200</affectRange>{POSITIVE}"), "");
    assert_eq!(s.cast(&buff, ME, false, None).targets, vec![P2]);
}

#[test]
fn a_mob_caster_does_not_hit_other_mobs() {
    let mut s = Scene::new();
    s.me();
    let caster = s.npc(9001, 0, 100);
    let _other_mob = s.npc(9001, 50, 100);
    let sk = skill("AURA", &format!("<affectRange>300</affectRange>{NEGATIVE}"), "");
    // `checkForAreaOffensiveSkills`: mob against mob is refused; the player is hit.
    assert_eq!(s.cast(&sk, caster, false, None).targets, vec![ME]);
}

#[test]
fn aura_corpse_mob_takes_dead_attackables_only() {
    let mut s = Scene::new();
    s.me();
    let d1 = s.dead_npc(9001, 100, 0);
    let d2 = s.dead_npc(9001, -100, 0);
    let _alive = s.npc(9001, 120, 0);
    let _far = s.dead_npc(9001, 600, 0);
    let _dead_folk = s.dead_npc(9002, 80, 0);
    let sk = skill("AURA_CORPSE_MOB", "<affectRange>200</affectRange>", "");
    assert_eq!(set_of(&s.cast(&sk, ME, false, None)), set(&[d1, d2]));
    assert_eq!(s.cast(&sk, ME, true, None).targets.len(), 1);
    let limited = skill(
        "AURA_CORPSE_MOB",
        "<affectRange>200</affectRange><affectLimit>1-0</affectLimit>",
        "",
    );
    assert_eq!(s.cast(&limited, ME, false, None).targets.len(), 1);
}

#[test]
fn area_corpse_mob_starts_at_the_dead_target() {
    let mut s = Scene::new();
    s.me();
    let corpse = s.dead_npc(9001, 300, 0);
    let alive_near = s.npc(9001, 350, 0);
    let _alive_far = s.npc(9001, 700, 0);
    let _other_corpse = s.dead_npc(9001, 320, 0);
    let sk = skill(
        "AREA_CORPSE_MOB",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );

    // NOTE(java-quirk): other corpses fail `checkForAreaOffensiveSkills` (dead), living mobs pass.
    let r = s.cast(&sk, ME, false, Some(corpse));
    assert_eq!(set_of(&r), set(&[corpse, alive_near]));
    assert_eq!(s.cast(&sk, ME, true, Some(corpse)).targets, vec![corpse]);
    for target in [None, Some(alive_near), Some(ME)] {
        let r = s.cast(&sk, ME, false, target);
        assert!(r.targets.is_empty());
        assert_eq!(r.messages, incorrect());
    }
}

// ------------------------------------------------------------------ front and behind

#[test]
fn front_and_behind_aura_use_the_casters_heading() {
    let mut s = Scene::new();
    s.me(); // facing east
    let front = s.npc(9001, 100, 0);
    let back = s.npc(9001, -100, 0);
    let side = s.npc(9001, 0, 100);
    let _front_far = s.npc(9001, 400, 0);

    let f = skill(
        "FRONT_AURA",
        &format!("<affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );
    let b = skill(
        "BEHIND_AURA",
        &format!("<affectRange>200</affectRange>{NEGATIVE}"),
        "",
    );
    assert_eq!(s.cast(&f, ME, false, None).targets, vec![front]);
    assert_eq!(s.cast(&b, ME, false, None).targets, vec![back]);
    let _ = side;
    assert_eq!(s.cast(&f, ME, true, None).targets, vec![front]);
}

#[test]
fn front_area_turns_the_caster_towards_the_origin_before_it_looks() {
    let mut s = Scene::new();
    s.me(); // facing east, but the target is to the north
    let t = s.npc(9001, 0, 300);
    let north = s.npc(9001, 0, 200);
    let east = s.npc(9001, 100, 0);
    let sk = skill(
        "FRONT_AREA",
        &format!("<castRange>600</castRange><affectRange>500</affectRange>{NEGATIVE}"),
        "",
    );

    let r = s.cast(&sk, ME, false, Some(t));
    assert_eq!(r.caster_heading, Some(heading_from_points(X, Y, X, Y + 300)));
    // NOTE(java-quirk): the new heading is used at once, so the mob to the north is "in front"
    // and the one to the east (the old facing) is on the side.
    assert_eq!(set_of(&r), set(&[t, north]));
    assert!(!r.targets.contains(&east));
    // onlyFirst returns before the heading is set.
    let r = s.cast(&sk, ME, true, Some(t));
    assert_eq!((r.targets, r.caster_heading), (vec![t], None));
}

#[test]
fn behind_area_picks_mobs_behind_the_caster() {
    let mut s = Scene::new();
    s.me();
    let t = s.npc(9001, 0, 300);
    let behind = s.npc(9001, -100, 0);
    let front = s.npc(9001, 100, 0);
    let sk = skill(
        "BEHIND_AREA",
        &format!("<castRange>600</castRange><affectRange>500</affectRange>{NEGATIVE}"),
        "",
    );
    let r = s.cast(&sk, ME, false, Some(t));
    assert_eq!(r.caster_heading, None, "only FRONT_AREA turns the caster");
    assert_eq!(set_of(&r), set(&[t, behind]));
    assert!(!r.targets.contains(&front));
}

// ------------------------------------------------------------------ friendly

#[test]
fn area_friendly_hits_friendly_players_only() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    s.player(P3, 150, 0);
    s.enter(P4, 0, 5, (X + 160, Y, Z)); // karma: not friendly
    let _mob = s.npc(9001, 120, 0);
    let _folk = s.npc(9002, 130, 0);
    let sk = skill(
        "AREA_FRIENDLY",
        &format!("<castRange>600</castRange><affectRange>200</affectRange>{POSITIVE}"),
        "",
    );

    let r = s.cast(&sk, ME, false, Some(P2));
    assert_eq!(r.targets[0], P2);
    assert_eq!(set_of(&r), set(&[P2, P3]));
    assert_eq!(s.cast(&sk, ME, true, Some(P2)).targets, vec![P2]);
    // Yourself is a fine target.
    assert!(s.cast(&sk, ME, false, Some(ME)).targets.contains(&ME));
    // Mobs, NPCs, flagged players and nothing at all are not.
    for target in [None, Some(P4)] {
        let r = s.cast(&sk, ME, false, target);
        assert!(r.targets.is_empty(), "{target:?}");
        assert_eq!(r.messages, incorrect(), "{target:?}");
    }
}

#[test]
fn aura_friendly_has_no_range_test() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    // NOTE(java-quirk): the affect range is not used, everything in the surrounding regions counts.
    s.player(P3, 1_000, 0);
    s.enter(P4, 0, 5, (X + 100, Y + 20, Z));
    let monster = s.npc(9001, 50, 0);
    let folk = s.npc(9002, 60, 0);
    let dead_folk = s.dead_npc(9002, 70, 0);
    let sk = skill(
        "AURA_FRIENDLY",
        &format!("<affectRange>10</affectRange>{POSITIVE}"),
        "",
    );

    let r = s.cast(&sk, ME, false, None);
    assert_eq!(set_of(&r), set(&[P2, P3, folk]));
    assert!(!r.targets.contains(&monster) && !r.targets.contains(&dead_folk));
    // An NPC caster has no player: nothing.
    assert!(s.cast(&sk, monster, false, None).targets.is_empty());
}

// ------------------------------------------------------------------ ground

#[test]
fn ground_uses_the_skill_world_position() {
    let mut s = Scene::new();
    s.me();
    let a = s.npc(9001, 400, 0);
    let b = s.npc(9001, 400, 100);
    let _out = s.npc(9001, 400, 200);
    let sk = skill(
        "GROUND",
        &format!("<castRange>900</castRange><affectRange>150</affectRange>{NEGATIVE}"),
        "",
    );
    let ctx = TargetContext {
        ctrl_pressed: false,
        ground: Some((X + 400, Y, Z)),
    };
    let r = s.world.skill_target_list(&sk, ME, false, None, &ctx);
    assert_eq!(set_of(&r), set(&[a, b]));

    // NOTE(java-quirk): the affect limit never limits.
    let limited = skill(
        "GROUND",
        &format!("<affectRange>150</affectRange><affectLimit>1-0</affectLimit>{NEGATIVE}"),
        "",
    );
    assert_eq!(
        s.world
            .skill_target_list(&limited, ME, false, None, &ctx)
            .targets
            .len(),
        2
    );

    // No ground point: Java throws on the first candidate.
    let r = s.cast(&sk, ME, false, None);
    assert!(r.targets.is_empty());
    assert_eq!(r.messages, vec![TargetMessage::Text(NOT_HANDLED_TEXT)]);

    // Nobody there: a summon-NPC skill is cast on the caster.
    let empty = TargetContext {
        ctrl_pressed: false,
        ground: Some((X - 5_000, Y, Z)),
    };
    let summon = skill(
        "GROUND",
        &format!("<affectRange>100</affectRange>{POSITIVE}"),
        "<effect name=\"SummonNpc\"><npcId>9001</npcId></effect>",
    );
    assert_eq!(
        s.world
            .skill_target_list(&summon, ME, false, None, &empty)
            .targets,
        vec![ME]
    );
    assert!(
        s.world
            .skill_target_list(&sk, ME, false, None, &empty)
            .targets
            .is_empty()
    );
}

// ------------------------------------------------------------------ clan

#[test]
fn clan_of_a_player_without_a_clan_is_the_player() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    for name in ["CLAN", "COMMAND_CHANNEL", "PARTY", "PARTY_CLAN"] {
        let sk = skill(name, &format!("<affectRange>500</affectRange>{POSITIVE}"), "");
        assert_eq!(s.cast(&sk, ME, false, Some(P2)).targets, vec![ME], "{name}");
        assert_eq!(s.cast(&sk, ME, true, Some(P2)).targets, vec![ME], "{name}");
    }
    // CORPSE_CLAN adds only dead clan members.
    let corpse = skill("CORPSE_CLAN", "<affectRange>500</affectRange>", "");
    assert!(s.cast(&corpse, ME, false, None).targets.is_empty());
    // The rest of the party family has nobody without a party.
    for name in ["PARTY_NOTME", "CLAN_MEMBER"] {
        let sk = skill(name, "<affectRange>500</affectRange>", "");
        let r = s.cast(&sk, ME, false, Some(P2));
        assert_eq!((r.targets, r.messages), (vec![], vec![]), "{name}");
    }
}

#[test]
fn clan_of_an_npc_is_its_clan_mates_in_cast_range() {
    let mut s = Scene::new();
    s.me();
    let caster = s.npc(9005, 0, 100);
    let orc1 = s.npc(9006, 50, 100);
    let orc2 = s.npc(9006, 80, 100);
    let _dragon = s.npc(9007, 60, 100);
    let _far_orc = s.npc(9006, 900, 100);
    let _plain = s.npc(9001, 70, 100);

    let sk = skill("CLAN", "<castRange>500</castRange>", "");
    let r = s.cast(&sk, caster, false, None);
    assert_eq!(r.targets[0], caster);
    assert_eq!(set_of(&r), set(&[caster, orc1, orc2]));

    // `maxTargets > 0 && size >= maxTargets` stops it, drawing the limit for each mate.
    let limited = skill(
        "CLAN",
        "<castRange>500</castRange><affectLimit>2-0</affectLimit>",
        "",
    );
    assert_eq!(s.cast(&limited, caster, false, None).targets.len(), 2);

    // Without clans the NPC is alone; the generic clan makes everyone a mate.
    let loner = s.npc(9001, 0, -100);
    assert_eq!(s.cast(&sk, loner, false, None).targets, vec![loner]);
    let generic = s.npc(9008, 0, 200);
    assert!(s.cast(&sk, generic, false, None).targets.contains(&generic));
}

#[test]
fn clan_member_of_an_npc_is_the_first_mate_or_itself() {
    let mut s = Scene::new();
    s.me();
    let caster = s.npc(9005, 0, 100);
    let sk = skill("CLAN_MEMBER", "<castRange>500</castRange>", "");
    assert_eq!(
        s.cast(&sk, caster, false, None).targets,
        vec![caster],
        "nobody around"
    );
    let mate = s.npc(9006, 50, 100);
    assert_eq!(s.cast(&sk, caster, false, None).targets, vec![mate]);
    let loner = s.npc(9001, 0, -100);
    assert_eq!(s.cast(&sk, loner, false, None).targets, vec![loner]);
    // A player caster gets nothing.
    assert!(s.cast(&sk, ME, false, None).targets.is_empty());
}

#[test]
fn corpse_clan_of_an_npc_checks_the_limit_without_the_zero_test() {
    let mut s = Scene::new();
    s.me();
    let caster = s.npc(9005, 0, 100);
    let orc = s.npc(9006, 50, 100);
    let sk = skill(
        "CORPSE_CLAN",
        "<castRange>500</castRange><affectLimit>5-0</affectLimit>",
        "",
    );
    assert_eq!(set_of(&s.cast(&sk, caster, false, None)), set(&[caster, orc]));
    // NOTE(java-quirk): a limit of 0 means "none", not "unlimited", for this handler.
    let zero = skill("CORPSE_CLAN", "<castRange>500</castRange>", "");
    assert_eq!(s.cast(&zero, caster, false, None).targets, vec![caster]);
}

// ------------------------------------------------------------------ party family

#[test]
fn party_member_and_party_other() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    let mob = s.npc(9001, 120, 0);
    let member = skill("PARTY_MEMBER", POSITIVE, "");

    assert_eq!(s.cast(&member, ME, false, Some(ME)).targets, vec![ME]);
    let r = s.cast(&member, ME, false, None);
    assert_eq!((r.targets, r.messages), (vec![], incorrect()));
    // Another player, with no party in common.
    let r = s.cast(&member, ME, false, Some(P2));
    assert!(r.targets.is_empty());
    assert_eq!(r.messages, vec![TargetMessage::UnsuitableTerms]);
    // A mob: no message.
    let r = s.cast(&member, ME, false, Some(mob));
    assert_eq!((r.targets, r.messages), (vec![], vec![]));
    // A dead player is also refused with the message.
    s.world.set_dead(P2, true);
    assert_eq!(
        s.cast(&member, ME, false, Some(P2)).messages,
        vec![TargetMessage::UnsuitableTerms]
    );

    let other = skill("PARTY_OTHER", POSITIVE, "");
    for target in [None, Some(ME), Some(P2)] {
        let r = s.cast(&other, ME, false, target);
        assert_eq!((r.targets, r.messages), (vec![], incorrect()), "{target:?}");
    }
}

#[test]
fn target_party_without_a_party_is_the_target() {
    let mut s = Scene::new();
    s.me();
    s.player(P2, 100, 0);
    let mob = s.npc(9001, 120, 0);
    let sk = skill("TARGET_PARTY", "<affectRange>500</affectRange>", "");

    // Java reads the selected object, not the argument.
    s.world.set_target(ME, Some(P2));
    assert_eq!(s.cast(&sk, ME, false, Some(P2)).targets, vec![P2]);
    for target in [None, Some(ME)] {
        let r = s.cast(&sk, ME, false, target);
        assert_eq!((r.targets, r.messages), (vec![], incorrect()), "{target:?}");
    }
    s.world.set_target(ME, Some(mob));
    // NOTE(java-quirk): `getTarget().asPlayer()` is null for a mob: NullPointerException.
    let r = s.cast(&sk, ME, false, Some(P2));
    assert!(r.targets.is_empty());
    assert_eq!(r.messages, vec![TargetMessage::Text(NOT_HANDLED_TEXT)]);
    s.world.set_dead(P2, true);
    assert_eq!(s.cast(&sk, ME, false, Some(P2)).messages, incorrect());
}

#[test]
fn summon_targets_find_nothing_without_summons() {
    let mut s = Scene::new();
    s.me();
    let mob = s.npc(9001, 100, 0);
    for name in [
        "AREA_SUMMON",
        "ENEMY_SUMMON",
        "OWNER_PET",
        "PET",
        "SERVITOR",
        "SUMMON",
    ] {
        let sk = skill(name, "<affectRange>500</affectRange>", "");
        let r = s.cast(&sk, ME, false, Some(mob));
        assert_eq!((r.targets, r.messages), (vec![], vec![]), "{name}");
    }
}

#[test]
fn every_handled_type_runs_without_the_not_handled_text() {
    let mut s = Scene::new();
    s.me();
    let mob = s.npc(9001, 100, 0);
    for t in HANDLED_TARGET_TYPES {
        let sk = skill(
            t.name(),
            "<castRange>500</castRange><affectRange>300</affectRange>",
            "",
        );
        for target in [None, Some(mob), Some(ME)] {
            let r = s.cast(&sk, ME, false, target);
            assert!(
                !r.messages.contains(&TargetMessage::Text(NOT_HANDLED_TEXT))
                    || t == TargetType::TargetParty
                    || t == TargetType::Ground,
                "{t:?} {target:?}"
            );
        }
    }
}
