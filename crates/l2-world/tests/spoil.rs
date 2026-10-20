#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Spoil and sweep (M5-SPOIL-01), end to end on the real world: the Spoil effect and its
//! `Formulas.calcMagicSuccess` roll, the SPOIL list rolled when the spoiled monster dies, the
//! blue glow in `Die`, the longer corpse, the Sweeper skill with its `canSweep` condition and the
//! reward messages, and the reset on respawn.
//! Java: handlers/skill/effects/{Spoil,Sweeper}.java, mechanics/conditions/ConditionPlayerCanSweep.java,
//! entity/actor/Attackable.java (checkSpoilOwner, isOldCorpse, getSpoilLootItems, takeSweep,
//! doItemDrop, onSpawn), taskmanagers/DecayTaskManager.java, mechanics/stats/Formulas.java
//! (calcMagicSuccess).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{Die, InventoryUpdate, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::formulas::Rnd;
use l2_world::rewards::drop::{DropKiller, DropVictim, calculate_drops};
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

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

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
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
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.0.skills.iter().any(|s| s.id == skill_id && s.is_passive())
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
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

/// The system messages one client got, decoded.
fn system_messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == ServerOpcode::SystemMessage.name())
        .map(|(_, b)| decode::<SystemMessage>(b))
        .collect()
}

fn messages(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    system_messages(sent, to).into_iter().map(|m| m.id).collect()
}

// ---- data --------------------------------------------------------------------------------------

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1060" type="EtcItem" name="Potion"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1000" type="EtcItem" name="Dagger Bit"><set name="weight" val="1" /></item>
<item id="1001" type="EtcItem" name="Bit B"><set name="weight" val="1" /></item>
</list>"#;

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

/// Spoil as the datapack has it (254: no `activateRate`, so the magic resist roll decides), with
/// a longer cast range so nobody walks; 9254 always lands (`activateRate` 100); 9255 has a magic
/// level of 80. Sweeper is skill 42, written as in the datapack (the `canSweep` condition, the
/// `Sweeper` and `ConsumeBody` effects).
const SKILLS: &str = r##"<list>
<skill id="254" levels="1" name="Spoil"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-138</effectPoint><effectRange>900</effectRange>
<hitTime>500</hitTime><magicLevel>1</magicLevel><mpConsume>3</mpConsume><reuseDelay>0</reuseDelay>
<effects><effect name="Spoil" /></effects></skill>
<skill id="9254" levels="1" name="Sure Spoil"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-138</effectPoint><effectRange>900</effectRange>
<hitTime>500</hitTime><magicLevel>1</magicLevel><mpConsume>3</mpConsume><reuseDelay>0</reuseDelay>
<activateRate>100</activateRate>
<effects><effect name="Spoil" /></effects></skill>
<skill id="9255" levels="1" name="High Spoil"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-138</effectPoint><effectRange>900</effectRange>
<hitTime>500</hitTime><magicLevel>80</magicLevel><mpConsume>3</mpConsume><reuseDelay>0</reuseDelay>
<effects><effect name="Spoil" /></effects></skill>
<skill id="42" levels="1" name="Sweeper"><operateType>A1</operateType><targetType>CORPSE_MOB</targetType>
<castRange>600</castRange><effectRange>900</effectRange><hitTime>500</hitTime><isMagic>2</isMagic>
<magicLevel>10</magicLevel><mpConsume>3</mpConsume><reuseDelay>500</reuseDelay>
<conditions><player canSweep="true" /></conditions>
<effects><effect name="Sweeper" /><effect name="ConsumeBody" /></effects></skill>
</list>"##;

const SPOIL: i32 = 254;
const SURE_SPOIL: i32 = 9254;
const HIGH_SPOIL: i32 = 9255;
const SWEEPER: i32 = 42;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

fn npcs() -> String {
    format!(
        r#"<list>
  <npc id="9001" type="Monster" level="1" name="Gremlin">{STATS}
    <dropLists>
      <drop><item id="57" min="5" max="5" chance="100"/></drop>
      <spoil><item id="1060" min="3" max="3" chance="100"/><item id="1000" min="1" max="1" chance="100"/></spoil>
    </dropLists>
  </npc>
  <npc id="9002" type="Monster" level="80" name="Big Gremlin">{STATS}</npc>
  <npc id="9004" type="Monster" level="10" name="Mid Gremlin">{STATS}</npc>
  <npc id="9003" type="Monster" level="1" name="Poor Gremlin">{STATS}
    <dropLists><drop><item id="57" min="5" max="5" chance="100"/></drop></dropLists>
  </npc>
</list>"#
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;
const OTHER: i32 = 0x2000_0002;

fn player_data(id: i32, name: &str, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
    let skills = [SPOIL, SURE_SPOIL, HIGH_SPOIL, SWEEPER]
        .into_iter()
        .map(|skill_id| SkillData {
            skill_id,
            skill_level: 1,
        })
        .collect();
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
        skills,
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new(seed: u64) -> Self {
        Self::with_settings(seed, WorldSettings::default(), None)
    }

    /// `spawns` is the body of a spawn file.
    fn with_settings(seed: u64, settings: WorldSettings, spawns: Option<&str>) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(seed), Box::new(NoOutbox));
        world.set_settings(settings);
        // No MP regeneration, so the costs can be read off the MP exactly.
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                SKILLS,
                &l2_data::skill::EnchantSkillGroups::default(),
                &l2_data::SkillLoadOptions::default(),
            )),
        };
        world.set_static_data(Box::new(Data(stats)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let templates = Arc::new(NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap());
        let spawn_data = match spawns {
            Some(body) => {
                let dir = tempfile::tempdir().unwrap();
                let spawns_dir = dir.path().join("data/spawns");
                std::fs::create_dir_all(&spawns_dir).unwrap();
                std::fs::write(
                    spawns_dir.join("a.xml"),
                    format!(r#"<?xml version="1.0"?><list enabled="true">{body}</list>"#),
                )
                .unwrap();
                Arc::new(SpawnData::load(
                    dir.path(),
                    &templates,
                    &SpawnLoadOptions::default(),
                ))
            }
            None => Arc::new(SpawnData::default()),
        };
        world.set_npc_data(templates, spawn_data);
        if spawns.is_some() {
            let _ = world.spawn_all_npcs(false);
        }
        Self { world, log }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, pos, items)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.ticks(20);
        self.log.take();
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn advance_ms(&mut self, ms: u64) {
        self.ticks(u32::try_from(ms / 50).unwrap());
    }

    fn monster(&mut self, npc_id: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + 50, Y, Z, 0).unwrap()
    }

    fn apply(&mut self, caster: i32, target: ObjectId, skill: i32) {
        self.world
            .apply_effects(ObjectId(caster), target, SkillKey::new(skill, 1));
    }

    /// `RequestMagicSkillUse` with `target` selected.
    fn cast(&mut self, caster: i32, target: ObjectId, skill: i32) {
        self.world.set_target(ObjectId(caster), Some(target));
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(caster).unwrap()),
            input: ClientInput::MagicSkillUse {
                magic_id: skill,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        });
    }

    /// `killer` hits the monster for good.
    fn kill(&mut self, monster: ObjectId, killer: i32) {
        self.world
            .reduce_current_hp(monster, 1_000_000.0, Some(ObjectId(killer)));
    }

    fn inventory(&self, who: i32) -> Vec<(i32, i64)> {
        self.world
            .player(ObjectId(who))
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.item_id, i.count))
            .collect()
    }

    fn spoil_templates(&self) -> NpcTemplates {
        NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap()
    }
}

// ---- Spoil -------------------------------------------------------------------------------------

/// Java Spoil.onStart: the caster becomes the spoiler and hears "The spoil condition has been
/// activated"; the monster is told it was attacked (so it turns on the caster).
#[test]
fn spoil_makes_the_caster_the_spoiler() {
    let mut s = Scene::new(1);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    assert!(!s.world.is_spoiled(monster));

    s.apply(ME, monster, SURE_SPOIL);
    let sent = s.log.take();
    assert!(s.world.is_spoiled(monster));
    assert_eq!(s.world.spoiler_of(monster), Some(ObjectId(ME)));
    assert_eq!(
        messages(&sent, ME),
        [SystemMessageId::TheSpoilConditionHasBeenActivated]
    );
    // `notifyActionAttacked(effector)`: the monster hates the spoiler now.
    assert!(
        s.world
            .aggro_list(monster)
            .iter()
            .any(|a| a.attacker == ObjectId(ME)),
        "the monster has the spoiler in its aggro list"
    );
}

/// Java Spoil.onStart: an already spoiled monster keeps its spoiler.
#[test]
fn spoiling_a_spoiled_monster_changes_nothing() {
    let mut s = Scene::new(2);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    s.enter(OTHER, "Rival", (X, Y + 20, Z), vec![]);
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.log.take();

    s.apply(OTHER, monster, SURE_SPOIL);
    let sent = s.log.take();
    assert_eq!(messages(&sent, OTHER), [SystemMessageId::ItHasAlreadyBeenSpoiled]);
    assert_eq!(s.world.spoiler_of(monster), Some(ObjectId(ME)));
}

/// Java Spoil.onStart: only a living monster can be spoiled.
#[test]
fn only_a_living_monster_can_be_spoiled() {
    let mut s = Scene::new(3);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    s.enter(OTHER, "Rival", (X, Y + 20, Z), vec![]);

    // A player.
    s.world
        .apply_effects(ObjectId(ME), ObjectId(OTHER), SkillKey::new(SURE_SPOIL, 1));
    assert_eq!(messages(&s.log.take(), ME), [SystemMessageId::InvalidTarget]);

    // A dead monster.
    let monster = s.monster(9001);
    s.kill(monster, OTHER);
    s.log.take();
    s.apply(ME, monster, SURE_SPOIL);
    assert_eq!(messages(&s.log.take(), ME), [SystemMessageId::InvalidTarget]);
    assert!(!s.world.is_spoiled(monster));
}

/// Java `Formulas.calcMagicSuccess`: a monster 79 levels above the skill's magic level resists
/// (the rate is below zero), and the roll is one draw of the world's RNG.
#[test]
fn spoil_fails_against_a_much_higher_level_monster() {
    let mut s = Scene::new(4);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9002); // level 80, magic level of the skill is 1
    let mut reference = s.world.rng().clone();

    s.apply(ME, monster, SPOIL);
    assert!(!s.world.is_spoiled(monster));
    assert!(
        messages(&s.log.take(), ME).is_empty(),
        "a resisted Spoil says nothing"
    );
    // The roll drew exactly once (`Rnd.get(100)`).
    reference.get_int(100);
    assert_eq!(s.world.rng().next_i32(), reference.next_i32());
}

/// A level 10 monster against a skill of magic level 1: the rate is 100 - round(1.3^9) = 89, and
/// every attempt replays its draw from the seed.
#[test]
fn spoil_succeeds_at_the_rate_of_calc_magic_success() {
    let mut s = Scene::new(5);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let (mut landed, mut missed) = (0, 0);
    for attempt in 0..300 {
        let monster = s.monster(9004);
        // The roll is the first draw of the application: a copy of the generator predicts it.
        let expected = s.world.rng().clone().get_int(100) < 89;
        s.apply(ME, monster, SPOIL);
        let said = messages(&s.log.take(), ME);
        assert_eq!(
            s.world.is_spoiled(monster),
            expected,
            "attempt {attempt}: {said:?}"
        );
        assert_eq!(
            said,
            if expected {
                vec![SystemMessageId::TheSpoilConditionHasBeenActivated]
            } else {
                vec![]
            }
        );
        if expected {
            landed += 1;
        } else {
            missed += 1;
        }
    }
    assert!(landed > 230 && missed >= 10, "{landed} landed, {missed} missed");
}

/// Java `calcMagicSuccess` with `CalculateMagicSuccessBySkillMagicLevel`: on, the skill's magic
/// level is the reference (80 against an 80 level monster: lands), off, the attacker's level 1
/// is (a 79 level gap: never lands).
#[test]
fn the_magic_level_setting_picks_the_reference_level() {
    let attempts = |by_magic_level: bool| {
        let mut settings = WorldSettings::default();
        settings.combat.calculate_magic_success_by_skill_magic_level = by_magic_level;
        let mut s = Scene::with_settings(6, settings, None);
        s.enter(ME, "Thief", (X, Y, Z), vec![]);
        let mut landed = 0;
        for _ in 0..40 {
            let monster = s.monster(9002);
            s.apply(ME, monster, HIGH_SPOIL);
            landed += i32::from(s.world.is_spoiled(monster));
        }
        landed
    };
    // The level 80 monster is above `MinNPCLevelForMagicPenalty` and 79 levels over the player:
    // the penalty table's last entry (3.5) makes the rate 96.
    assert!(attempts(true) >= 35);
    assert_eq!(attempts(false), 0);
}

/// Java `calcMagicSuccess`: the level penalty of `SkillChancePenaltyForLvLDifferences` applies
/// from `MinNPCLevelForMagicPenalty`. With the penalty table made large the rate drops below
/// zero, so a monster that is level-matched to the skill still resists.
#[test]
fn the_npc_level_penalty_lowers_the_rate() {
    let mut settings = WorldSettings::default();
    settings.combat.npc_skill_chance_penalty = vec![1000.0];
    let mut s = Scene::with_settings(7, settings, None);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    for _ in 0..20 {
        let monster = s.monster(9002);
        s.apply(ME, monster, HIGH_SPOIL);
        assert!(!s.world.is_spoiled(monster));
    }
    // Below the penalty's level threshold it does not apply.
    let mut settings = WorldSettings::default();
    settings.combat.npc_skill_chance_penalty = vec![1000.0];
    settings.combat.min_npc_level_magic_penalty = 81;
    let mut s = Scene::with_settings(7, settings, None);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9002);
    // The skill's `activateRate` is not 100 here: 9255 rolls 100 - round(1) = 99.
    let mut landed = false;
    for _ in 0..5 {
        s.apply(ME, monster, HIGH_SPOIL);
        landed |= s.world.is_spoiled(monster);
    }
    assert!(landed);
}

// ---- the SPOIL list and the corpse -------------------------------------------------------------

/// Java `Attackable.doItemDrop`: a spoiled monster rolls its SPOIL list before the DROP list
/// (the draws follow Java's order), the corpse is sweepable (`Die`'s blue glow), and the monster
/// that was not spoiled has nothing to sweep.
#[test]
fn a_spoiled_monster_rolls_its_spoil_list_before_its_drops() {
    let mut s = Scene::new(8);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let spoiled = s.monster(9001);
    let plain = s.monster(9001);
    s.apply(ME, spoiled, SURE_SPOIL);
    s.log.take();

    // Replay the draws of the kill: the SPOIL list, then the DROP list.
    let templates = s.spoil_templates();
    let items = ItemTemplates::from_xml(ITEMS).unwrap();
    let victim = DropVictim {
        level: 1,
        raid: false,
        champion: false,
        seeded: false,
    };
    let killer = DropKiller::plain(1);
    let settings = DropSettings::default();
    let npc = templates.get(9001).unwrap();
    let mut reference = s.world.rng().clone();
    let spoil_roll = calculate_drops(
        &settings,
        &items,
        npc,
        l2_data::npc::DropType::Spoil,
        &victim,
        &killer,
        &mut reference,
    );
    assert_eq!(spoil_roll.len(), 2, "both spoil items are certain");
    let drop_roll = calculate_drops(
        &settings,
        &items,
        npc,
        l2_data::npc::DropType::Drop,
        &victim,
        &killer,
        &mut reference,
    );

    s.kill(spoiled, ME);
    assert!(s.world.is_sweep_active(spoiled));
    assert_eq!(
        s.world
            .spoil_loot_items(spoiled)
            .iter()
            .map(|h| (h.id, h.count))
            .collect::<Vec<_>>(),
        spoil_roll.iter().map(|h| (h.id, h.count)).collect::<Vec<_>>()
    );
    // The adena lies on the ground like the replayed DROP roll says.
    let ground: Vec<(i32, i64)> = s
        .world
        .ground_item_ids()
        .into_iter()
        .map(|id| {
            let g = s.world.ground_item(id).unwrap();
            (g.data.item_id, g.data.count)
        })
        .collect();
    assert_eq!(
        ground,
        drop_roll.iter().map(|h| (h.id, h.count)).collect::<Vec<_>>()
    );

    // `Die` carries the blue glow for the spoiled corpse only.
    assert!(s.world.die_packet(spoiled).sweepable);
    s.kill(plain, ME);
    assert!(!s.world.is_sweep_active(plain));
    assert!(!s.world.die_packet(plain).sweepable);
    let sent = s.log.take();
    let dies: Vec<Die> = sent
        .iter()
        .filter(|(to, b)| *to == ME && name_of(b) == ServerOpcode::Die.name())
        .map(|(_, b)| decode::<Die>(b))
        .collect();
    assert_eq!(
        dies.iter()
            .map(|d| (d.object_id, d.sweepable))
            .collect::<Vec<_>>(),
        [(spoiled.0, true), (plain.0, false)]
    );
}

/// A monster without a SPOIL list that is spoiled has nothing to sweep, and a monster killed by
/// nobody who is a player rolls nothing.
#[test]
fn a_spoiled_monster_without_a_spoil_list_has_nothing_to_sweep() {
    let mut s = Scene::new(9);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9003);
    s.apply(ME, monster, SURE_SPOIL);
    assert!(s.world.is_spoiled(monster));
    s.kill(monster, ME);
    assert!(!s.world.is_sweep_active(monster));
    assert!(!s.world.die_packet(monster).sweepable);
}

/// Java `DecayTaskManager.add`: a spoiled corpse stays `SpoiledCorpseExtendTime` seconds longer.
#[test]
fn a_spoiled_corpse_stays_longer() {
    let mut s = Scene::new(10);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let spoiled = s.monster(9001);
    let plain = s.monster(9003);
    s.apply(ME, spoiled, SURE_SPOIL);
    s.kill(spoiled, ME);
    s.kill(plain, ME);
    // `DefaultCorpseTime` 7 s for both; +10 s for the spoiled one. Both died on the same tick.
    assert_eq!(s.world.decay_remaining_ms(plain), 7_000);
    assert_eq!(s.world.decay_remaining_ms(spoiled), 17_000);

    // The extension is the setting.
    let mut settings = WorldSettings::default();
    settings.death.spoiled_corpse_extend_time = 25;
    let mut s = Scene::with_settings(10, settings, None);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let spoiled = s.monster(9001);
    s.apply(ME, spoiled, SURE_SPOIL);
    s.kill(spoiled, ME);
    assert_eq!(s.world.decay_remaining_ms(spoiled), 32_000);
}

// ---- Sweeper -----------------------------------------------------------------------------------

/// The whole sweep as a player lives it: Spoil, the kill, the Sweeper cast on the corpse. The
/// spoil items come into the inventory with "You have earned" (`ItemProcessType.SWEEP`), the
/// corpse is consumed, and a second sweep finds nothing.
#[test]
fn the_spoiler_sweeps_the_corpse() {
    let mut s = Scene::new(11);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    s.cast(ME, monster, SURE_SPOIL);
    s.advance_ms(1000);
    assert_eq!(s.world.spoiler_of(monster), Some(ObjectId(ME)));
    s.kill(monster, ME);
    s.log.take();

    s.cast(ME, monster, SWEEPER);
    s.advance_ms(1500);
    let sent = s.log.take();
    let mine = system_messages(&sent, ME);
    // 3 potions (S2 of S1: "earned 3 Potion(s)") and the single dagger bit (S1).
    let earned: Vec<(SystemMessageId, Vec<SystemMessageParam>)> = mine
        .iter()
        .filter(|m| {
            matches!(
                m.id,
                SystemMessageId::YouHaveEarnedS1 | SystemMessageId::YouHaveEarnedS2S1S
            )
        })
        .map(|m| (m.id, m.params.clone()))
        .collect();
    assert_eq!(
        earned,
        [
            (
                SystemMessageId::YouHaveEarnedS2S1S,
                vec![
                    SystemMessageParam::ItemName(1060),
                    SystemMessageParam::LongNumber(3)
                ]
            ),
            (
                SystemMessageId::YouHaveEarnedS1,
                vec![SystemMessageParam::ItemName(1000)]
            ),
        ],
        "{mine:?}"
    );
    assert!(
        !mine
            .iter()
            .any(|m| m.id == SystemMessageId::YouHaveObtainedS1
                || m.id == SystemMessageId::YouHaveObtainedS2S1),
        "sweeping never says 'obtained'"
    );
    assert_eq!(s.inventory(ME), [(1060, 3), (1000, 1)]);
    // An InventoryUpdate reached the client.
    assert!(
        sent.iter()
            .any(|(to, b)| *to == ME && name_of(b) == ServerOpcode::InventoryUpdate.name())
    );
    // Nothing is left to sweep, and `ConsumeBody` took the corpse away.
    assert!(!s.world.is_sweep_active(monster));
    assert!(s.world.npc(monster).is_none(), "the corpse is consumed");
}

/// Java `Sweeper.onStart` called without the condition (an effect applied by hand, as `applyEffects`
/// from a script or a second cast would do): the items go to the spoiler only once.
#[test]
fn the_sweep_items_can_be_taken_only_once() {
    let mut s = Scene::new(12);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    s.apply(ME, monster, SWEEPER);
    assert_eq!(s.inventory(ME), [(1060, 3), (1000, 1)]);
    s.apply(ME, monster, SWEEPER);
    assert_eq!(s.inventory(ME), [(1060, 3), (1000, 1)]);
}

/// Java `ConditionPlayerCanSweep` + `checkSpoilOwner`: somebody else cannot sweep and is told so;
/// the effect itself refuses too.
#[test]
fn only_the_spoiler_can_sweep() {
    let mut s = Scene::new(13);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    s.enter(OTHER, "Rival", (X, Y + 20, Z), vec![]);
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    s.log.take();

    s.cast(OTHER, monster, SWEEPER);
    s.advance_ms(1500);
    let sent = s.log.take();
    assert_eq!(
        messages(&sent, OTHER),
        [SystemMessageId::ThereAreNoPriorityRightsOnASweeper]
    );
    assert!(s.inventory(OTHER).is_empty());
    assert!(s.world.is_sweep_active(monster), "the loot stays for the spoiler");

    // The effect on its own: `checkSpoilOwner(player, false)` is silent.
    s.apply(OTHER, monster, SWEEPER);
    assert!(s.inventory(OTHER).is_empty());
    assert!(messages(&s.log.take(), OTHER).is_empty());
}

/// Java `ConditionPlayerCanSweep`: a corpse nobody spoiled says "Sweeper failed, target not
/// spoiled".
#[test]
fn a_corpse_that_was_not_spoiled_cannot_be_swept() {
    let mut s = Scene::new(14);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    s.kill(monster, ME);
    s.log.take();
    s.cast(ME, monster, SWEEPER);
    s.advance_ms(1500);
    let sent = s.log.take();
    assert_eq!(
        messages(&sent, ME),
        [SystemMessageId::SweeperFailedTargetNotSpoiled]
    );
    assert!(s.world.npc(monster).is_some(), "ConsumeBody did not run");
}

/// Java `ConditionPlayerCanSweep` + `isOldCorpse`: with less than
/// `CorpseConsumeSkillAllowedTimeBeforeDecay` left the corpse is too old.
#[test]
fn an_old_corpse_cannot_be_swept() {
    let mut s = Scene::new(15);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    // 17 s of corpse; wait until less than 2 s are left.
    s.advance_ms(15_100);
    assert!(s.world.decay_remaining_ms(monster) < 2_000);
    s.log.take();

    s.cast(ME, monster, SWEEPER);
    s.advance_ms(500);
    let sent = s.log.take();
    assert_eq!(
        messages(&sent, ME),
        [SystemMessageId::TheCorpseIsTooOldTheSkillCannotBeUsed]
    );
    assert!(s.inventory(ME).is_empty());
    assert!(s.world.is_sweep_active(monster));
}

/// The old corpse limit is the setting `CorpseConsumeSkillAllowedTimeBeforeDecay`.
#[test]
fn the_old_corpse_limit_is_the_setting() {
    let mut settings = WorldSettings::default();
    settings.death.corpse_consume_skill_allowed_time_before_decay = 20_000;
    let mut s = Scene::with_settings(16, settings, None);
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    s.log.take();
    // 17 s of corpse against a limit of 20 s: too old at once.
    s.cast(ME, monster, SWEEPER);
    s.advance_ms(500);
    assert_eq!(
        messages(&s.log.take(), ME),
        [SystemMessageId::TheCorpseIsTooOldTheSkillCannotBeUsed]
    );
}

/// Java `ConditionPlayerCanSweep` + `checkInventorySlotsAndWeight(.., true, true)`: no room for
/// the items says "inventory full" and then the skill message; nothing is taken.
#[test]
fn a_full_inventory_cannot_sweep() {
    // Two items in the bag, three slots: the potion and the bit need two more.
    let settings = WorldSettings {
        inventory_maximum_no_dwarf: 3,
        ..WorldSettings::default()
    };
    let mut s = Scene::with_settings(17, settings, None);
    s.enter(
        ME,
        "Thief",
        (X, Y, Z),
        vec![item(0x2000_0101, 1001, 1), item(0x2000_0102, 1001, 1)],
    );
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    s.log.take();

    s.cast(ME, monster, SWEEPER);
    s.advance_ms(500);
    let sent = s.log.take();
    assert_eq!(
        messages(&sent, ME),
        [
            SystemMessageId::YourInventoryIsFull,
            SystemMessageId::WeightAndVolumeLimitHaveBeenExceededThatSkillIsCurrentlyUnavailable
        ]
    );
    assert_eq!(s.inventory(ME).len(), 2);
    assert!(s.world.is_sweep_active(monster));
}

/// A stack the sweeper has needs no slot (`isStackable && count > 0`): with one free slot the
/// potion joins the stack and only the bit needs a slot.
#[test]
fn a_stack_in_the_bag_needs_no_slot() {
    let settings = WorldSettings {
        inventory_maximum_no_dwarf: 3,
        ..WorldSettings::default()
    };
    let mut s = Scene::with_settings(18, settings, None);
    s.enter(
        ME,
        "Thief",
        (X, Y, Z),
        vec![item(0x2000_0101, 1001, 1), item(0x2000_0102, 1060, 5)],
    );
    let monster = s.monster(9001);
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    s.log.take();

    s.cast(ME, monster, SWEEPER);
    s.advance_ms(1500);
    let sent = s.log.take();
    assert!(
        !messages(&sent, ME).contains(&SystemMessageId::YourInventoryIsFull),
        "{:?}",
        messages(&sent, ME)
    );
    assert_eq!(s.inventory(ME), [(1001, 1), (1060, 8), (1000, 1)]);
    let update = sent
        .iter()
        .filter(|(to, b)| *to == ME && name_of(b) == ServerOpcode::InventoryUpdate.name())
        .map(|(_, b)| decode::<InventoryUpdate>(b))
        .next_back()
        .unwrap();
    assert!(!update.updates.is_empty());
}

// ---- respawn -----------------------------------------------------------------------------------

/// Java `Attackable.onSpawn`: "Clear mob spoil" and `_sweepItems.set(null)`. The respawned
/// monster has its old object id and none of its old spoil.
#[test]
fn a_respawned_monster_is_not_spoiled() {
    let spawn = r#"<spawn><npc id="9001" x="-71338" y="258271" z="-3104" respawnDelay="5"/></spawn>"#;
    let mut s = Scene::with_settings(19, WorldSettings::default(), Some(spawn));
    s.enter(ME, "Thief", (X, Y, Z), vec![]);
    let monster = s
        .world
        .npc_ids()
        .into_iter()
        .find(|id| s.world.npc(*id).is_some_and(|n| n.npc_id == 9001))
        .unwrap();
    s.apply(ME, monster, SURE_SPOIL);
    s.kill(monster, ME);
    assert!(s.world.is_sweep_active(monster));

    // The corpse (17 s) decays and the spawn brings the same object back after 5 s.
    s.advance_ms(17_100);
    assert!(s.world.npc(monster).is_none(), "the corpse decayed");
    assert!(!s.world.is_spoiled(monster));
    assert!(!s.world.is_sweep_active(monster));
    s.advance_ms(6_000);
    let back = s.world.npc(monster).expect("the same object id came back");
    assert!(back.core.cur_hp > 0.0);
    assert!(!s.world.is_dead(monster));
    assert!(!s.world.is_spoiled(monster));
    assert!(!s.world.is_sweep_active(monster));
    assert_eq!(s.world.spoiler_of(monster), None);
}
