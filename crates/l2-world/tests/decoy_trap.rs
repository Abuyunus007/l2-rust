#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! S-HAND-04: decoys (`Decoy`, the `SummonNpc` "Decoy" branch), skill traps (`Trap`, `SummonTrap`,
//! `TrapDetect`, `TrapRemove`, `Quest.addTrap`, `ON_TRAP_ACTION`) and the drop search board.
//! Expected values follow the Java classes named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, HtmCache, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{CharInfo, SystemMessage, TrapInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::effects::SkillKey;
use l2_world::script::TrapAction;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Vec<u8>>>>);

impl Log {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(bytes);
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct Quiet;

impl Outbox for Quiet {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

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

const TRAP_NPC: i32 = 13271;
const GOBLIN: i32 = 9001;
const SUMMON_DECOY: i32 = 9201;
const SUMMON_TRAP: i32 = 9211;
const DETECT: i32 = 9212;
const REMOVE: i32 = 9213;
const TRAP_SKILL: i32 = 9301;

/// 5272 is the shape of the datapack's Decoy Provocation; the others are made for one rule each.
const SKILLS: &str = r##"<list>
<skill id="5272" levels="1" name="Decoy Provocation"><operateType>A1</operateType><targetType>AURA</targetType>
<affectRange>200</affectRange><effectPoint>-4080</effectPoint><hitTime>1200</hitTime><isMagic>1</isMagic>
<effects><effect name="TargetMeProbability"><chance>80</chance></effect></effects></skill>
<skill id="9201" levels="1" name="Decoy"><operateType>A1</operateType><targetType>SELF</targetType><isMagic>1</isMagic>
<effects><effect name="SummonNpc"><npcId>13071</npcId><despawnDelay>20000</despawnDelay></effect></effects></skill>
<skill id="9211" levels="1" name="Flame Trap"><operateType>A1</operateType><targetType>SELF</targetType><isMagic>1</isMagic>
<effects><effect name="SummonTrap"><npcId>13271</npcId><despawnTime>30000</despawnTime></effect></effects></skill>
<skill id="9212" levels="1" name="Detect Trap"><operateType>A1</operateType><targetType>ONE</targetType><isMagic>1</isMagic>
<effects><effect name="TrapDetect"><power>10</power></effect></effects></skill>
<skill id="9213" levels="1" name="Remove Trap"><operateType>A1</operateType><targetType>ONE</targetType><isMagic>1</isMagic>
<effects><effect name="TrapRemove"><power>10</power></effect></effects></skill>
<skill id="9301" levels="1" name="Trap Explosion"><operateType>A1</operateType><targetType>AURA</targetType>
<affectRange>200</affectRange><hitTime>400</hitTime><isMagic>1</isMagic></skill>
</list>"##;

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="13071" type="Decoy" level="40" name="Decoy" usingServerSideName="true">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="500" hpRegen="2" mp="100.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="8"/><height normal="23"/></collision>
  </npc>
<npc id="13271" type="Trap" level="10" name="Flame Trap" usingServerSideName="true">
    <parameters><skill name="trap_skill" id="9301" level="1" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="500" hpRegen="2" mp="100.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="8"/><height normal="14"/></collision>
  </npc>
<npc id="9001" type="Monster" level="1" name="Gremlin">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    }
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic::unknown(item_id))
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

/// What the probe script heard: `"<action> <trap npc id> <trigger>"`.
#[derive(Clone, Default)]
struct Heard(Arc<Mutex<Vec<String>>>);

impl Heard {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

/// `addTrapActionId(TRAP_NPC)`.
struct Probe(Heard);

impl Script for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }
    fn path(&self) -> &'static str {
        "ai/others/Probe"
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.add_trap_action_id(&[TRAP_NPC]);
    }
    fn on_trap_action(&self, _ctx: &mut ScriptCtx<'_>, trap: ScriptNpc, trigger: ObjectId, action: TrapAction) {
        self.0
            .0
            .lock()
            .unwrap()
            .push(format!("{action:?} {} {}", trap.id, trigger.0));
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);

fn player_data(id: ObjectId, x: i32, karma: i32) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0 & 0xff),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 20,
        exp: 0,
        sp: 0,
        karma,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y: Y,
        z: Z,
        heading: 0,
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

struct Scene {
    world: World,
    me: Log,
    p2: Log,
    heard: Heard,
}

impl Scene {
    /// `karma` is the owner's: an owner without karma or PvP flag cannot have its trap detected
    /// (`Trap.setDetected`).
    fn new(karma: i32) -> Self {
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Quiet));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let root = std::env::temp_dir();
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(&root.join("l2-no-spawns"), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(&root.join("l2-no-html"), false)));
        let heard = Heard::default();
        let mut registry = ScriptRegistry::new();
        registry.add(Probe(heard.clone()));
        world.install_scripts(registry);
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, X, karma)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(P2, X + 300, 0)),
            sink: Box::new(Sink(p2.clone())),
            ip: None,
        });
        let mut scene = Self { world, me, p2, heard };
        scene.advance(1000);
        scene.me.take();
        scene.p2.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn of<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

// ---- decoys ------------------------------------------------------------------------------------

/// `SummonNpc` "Decoy" -> `new Decoy` -> `spawnMe` -> `Decoy.onSpawn` (`CharInfo` to the owner),
/// `sendInfo` (`CharInfo` of the owner's look at the decoy's object id), `player.setDecoy`.
#[test]
fn a_decoy_is_summoned_and_looks_like_its_owner() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_DECOY, 1));
    let decoy = s.world.decoy_of(ME).expect("a decoy");
    assert!(s.world.is_decoy(decoy));
    assert_eq!(s.world.decoy_owner(decoy), Some(ME));
    // Java `Decoy` is a `Creature`, not an `Npc`; `setSummoner(player)`.
    assert!(!s.world.is_npc(decoy));
    assert_eq!(s.world.summoner_of(decoy), Some(ME));
    assert_eq!(s.world.position_of(decoy), s.world.position_of(ME));

    let mine = s.me.take();
    let infos: Vec<CharInfo> = of(&mine, "CHAR_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(
        infos.iter().any(|i| i.object_id == decoy.0 && i.name == "P1"),
        "the owner gets the decoy's CharInfo with its own name"
    );
    let theirs = s.p2.take();
    let seen: Vec<CharInfo> = of(&theirs, "CHAR_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(seen.iter().any(|i| i.object_id == decoy.0 && i.name == "P1"));
}

/// `Decoy.isAutoAttackable` is the owner's: a monster attacks it, a stranger does not auto
/// attack it (`Player.isAutoAttackable`, no flag or karma). `DecoyClick` only targets.
#[test]
fn a_decoy_is_attacked_like_its_owner_and_dies_into_a_corpse() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_DECOY, 1));
    let decoy = s.world.decoy_of(ME).unwrap();
    let goblin = s.world.spawn_npc_at(GOBLIN, X + 60, Y, Z, 0).unwrap();
    assert!(s.world.is_auto_attackable_by(decoy, goblin), "monsters attack players");
    assert!(!s.world.is_auto_attackable_by(decoy, P2), "an unflagged player is not attacked");

    // DecoyClick: target only.
    s.world.handle(WorldCommand::Input {
        session: SessionId(2),
        input: ClientInput::Action {
            object_id: decoy.0,
            action_id: 0,
        },
    });
    assert_eq!(s.world.target_of(P2), Some(decoy));

    // The goblin hits it.
    s.world.set_intention_attack(goblin, decoy);
    let before = s.world.npc(decoy).unwrap().core.cur_hp;
    s.world.reduce_current_hp(decoy, 10.0, Some(goblin));
    assert!(s.world.npc(decoy).unwrap().core.cur_hp < before);

    // Its death: `unSummon` does not remove a dead decoy, `DecayTaskManager.add` does later.
    assert!(s.world.do_die(decoy, Some(goblin)));
    assert!(s.world.is_dead(decoy));
    assert!(s.world.has_decay_task(decoy));
    assert_eq!(s.world.decoy_of(ME), Some(decoy), "the owner still has it as a corpse");
    s.world.delete_npc(goblin);
    s.advance(10_000);
    s.advance(10_000);
    s.advance(10_000);
    assert_eq!(s.world.decoy_of(ME), None);
    assert!(s.world.npc(decoy).is_none());
}

/// The life task (`totalLifeTime`) unsummons a living decoy: it leaves the world and the owner
/// forgets it. The hate spam casts "Decoy Provocation" (level `displayId - 13070`) at itself.
#[test]
fn a_decoy_spams_hate_and_leaves_after_its_life_time() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_DECOY, 1));
    let decoy = s.world.decoy_of(ME).unwrap();
    s.me.take();
    s.advance(2500);
    let packets = s.me.take();
    let casts = of(&packets, "MAGIC_SKILL_USE");
    assert!(!casts.is_empty(), "the decoy cast its provocation after 2 s");
    s.advance(20_000);
    assert_eq!(s.world.decoy_of(ME), None);
    assert!(s.world.npc(decoy).is_none());
}

// ---- traps -------------------------------------------------------------------------------------

/// `SummonTrap` -> `new Trap(template, player, despawnTime)`: only the owner sees it (`TrapInfo`
/// with the owner's name as title); a second trap unsummons the first.
#[test]
fn a_trap_is_set_and_only_its_owner_sees_it() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let trap = s.world.trap_of(ME).expect("a trap");
    assert!(s.world.is_trap(trap));
    assert_eq!(s.world.trap_owner(trap), Some(ME));
    assert!(s.world.is_npc(trap));
    assert!(s.world.is_invul(trap));
    assert_eq!(s.world.trap_skill(trap), Some(SkillKey::new(TRAP_SKILL, 1)));

    let mine = s.me.take();
    let info: TrapInfo = decode(of(&mine, "NPC_INFO")[0]);
    assert_eq!(info.object_id, trap.0);
    assert_eq!(info.display_id, 13271);
    assert_eq!(info.title, "P1");
    assert!(!info.attackable, "the owner sees it as no target");
    assert!(of(&s.p2.take(), "NPC_INFO").is_empty(), "a stranger sees nothing");
    assert!(s.world.trap_can_be_seen(trap, Some(ME)));
    assert!(!s.world.trap_can_be_seen(trap, Some(P2)));
    assert!(s.world.is_auto_attackable_by(trap, P2));

    // A second trap: the first is unsummoned.
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let second = s.world.trap_of(ME).unwrap();
    assert_ne!(second, trap);
    assert!(s.world.npc(trap).is_none());
}

/// `TrapDetect` (power 10 >= trap level 10) with an owner who has karma: the detector sees the
/// trap and the script hears `TRAP_DETECTED`. `TrapRemove` then disarms it: `TRAP_DISARMED`, the
/// trap is deleted, the player is told the device has been stopped.
#[test]
fn a_trap_is_detected_and_removed() {
    let mut s = Scene::new(5);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let trap = s.world.trap_of(ME).unwrap();
    s.p2.take();

    // Not yet detected: the remove effect is refused with "invalid target".
    s.world.apply_effects(P2, trap, SkillKey::new(REMOVE, 1));
    assert_eq!(message_ids(&s.p2.take()), [SystemMessageId::InvalidTarget]);

    s.world.apply_effects(P2, trap, SkillKey::new(DETECT, 1));
    let seen = s.p2.take();
    let info: TrapInfo = decode(of(&seen, "NPC_INFO")[0]);
    assert_eq!(info.object_id, trap.0);
    assert!(!info.attackable, "who sees it does not attack it");
    assert!(s.world.trap_can_be_seen(trap, Some(P2)));
    s.advance(50);
    assert_eq!(s.heard.take(), [format!("TrapDetected {TRAP_NPC} {}", P2.0)]);

    s.world.apply_effects(P2, trap, SkillKey::new(REMOVE, 1));
    assert_eq!(
        message_ids(&s.p2.take()),
        [SystemMessageId::TheTrapDeviceHasBeenStopped]
    );
    s.advance(50);
    assert_eq!(s.heard.take(), [format!("TrapDisarmed {TRAP_NPC} {}", P2.0)]);
    assert!(s.world.npc(trap).is_none());
    assert_eq!(s.world.trap_of(ME), None);
}

/// A detector whose trap owner has no karma and no PvP flag does not detect (`setDetected`).
#[test]
fn an_unflagged_owner_hides_the_trap_from_detection() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let trap = s.world.trap_of(ME).unwrap();
    s.world.apply_effects(P2, trap, SkillKey::new(DETECT, 1));
    assert!(!s.world.trap_can_be_seen(trap, Some(P2)));
    s.advance(50);
    assert!(s.heard.take().is_empty());
}

/// `TrapTask` finds the first creature within 150 (a monster counts for a player's trap), the trap
/// goes off (`TRAP_TRIGGERED`, `TrapInfo` to everybody), casts its skill 500 ms later and is
/// unsummoned after the hit time and 300 ms.
#[test]
fn a_trap_goes_off_when_a_monster_comes_close() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let trap = s.world.trap_of(ME).unwrap();
    // The owner stands on it (no karma, no flag): not a target.
    s.advance(1100);
    assert!(!s.world.trap_is_triggered(trap));
    let goblin = s.world.spawn_npc_at(GOBLIN, X + 100, Y, Z, 0).unwrap();
    s.p2.take();
    s.advance(1100);
    assert!(s.world.trap_is_triggered(trap));
    let packets = s.p2.take();
    assert!(!of(&packets, "NPC_INFO").is_empty(), "everybody sees a trap that went off");
    assert_eq!(s.heard.take(), [format!("TrapTriggered {TRAP_NPC} {}", goblin.0)]);
    s.advance(600);
    assert!(!of(&s.p2.take(), "MAGIC_SKILL_USE").is_empty(), "the trap cast its skill");
    s.world.delete_npc(goblin);
    s.advance(1500);
    assert!(s.world.npc(trap).is_none(), "unsummoned after the cast");
    assert_eq!(s.world.trap_of(ME), None);
}

/// `Trap` with a life time: `TrapTask` counts it down and unsummons a trap whose skill is no aura
/// when it runs out (30 s).
#[test]
fn a_trap_runs_out() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_TRAP, 1));
    let trap = s.world.trap_of(ME).unwrap();
    s.advance(20_000);
    assert!(s.world.npc(trap).is_some());
    // The skill is an aura skill: running out triggers it on the trap itself.
    s.advance(11_000);
    assert!(s.world.trap_is_triggered(trap) || s.world.npc(trap).is_none());
    s.advance(3_000);
    assert!(s.world.npc(trap).is_none());
}

/// `Quest.addTrap`: no owner, no life time, invulnerable; nobody sees it until it is detected;
/// a script that did `addTrapActionId` hears the actions.
#[test]
fn a_script_trap_has_no_owner_and_no_life_time() {
    let mut s = Scene::new(0);
    let trap = s
        .world
        .spawn_trap(TRAP_NPC, None, -1, (X + 1000, Y, Z), 0, 0)
        .expect("a trap");
    assert_eq!(s.world.trap_owner(trap), None);
    assert!(s.world.is_invul(trap));
    assert!(!s.world.trap_can_be_seen(trap, Some(ME)));
    s.advance(40_000);
    assert!(s.world.npc(trap).is_some(), "no life time: it stays");
    // Detected by anybody (no owner to be unflagged).
    s.world.trap_set_detected(trap, ME);
    s.advance(50);
    assert!(s.world.trap_can_be_seen(trap, Some(ME)));
    assert_eq!(s.heard.take(), [format!("TrapDetected {TRAP_NPC} {}", ME.0)]);
    s.world.trap_unsummon(trap);
    assert!(s.world.npc(trap).is_none());
}

/// A monster that is told to attack the decoy hits it with its normal attack.
#[test]
fn a_monster_fights_the_decoy() {
    let mut s = Scene::new(0);
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_DECOY, 1));
    let decoy = s.world.decoy_of(ME).unwrap();
    let max = s.world.npc(decoy).unwrap().core.max_hp;
    let goblin = s.world.spawn_npc_at(GOBLIN, X + 30, Y, Z, 0).unwrap();
    s.world.set_intention_attack(goblin, decoy);
    s.advance(5_000);
    let hp = s.world.npc(decoy).map_or(0.0, |n| n.core.cur_hp);
    assert!(hp < max, "the goblin hurt the decoy: {hp} of {max}");
}
