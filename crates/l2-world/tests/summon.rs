#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Servitors (M6-SUMMON-01): the `Summon` effect and what the owner and the others see
//! (`PetInfo`, `PetStatusUpdate`, `SummonInfo`, `RelationChanged`), the 5 s life task (time,
//! the item it eats, `SetSummonRemainTime`), unsummoning (action 52, `Unsummon`, the owner leaving)
//! and the rows Java writes for it (`character_summons`, `character_summon_skills_save`), the
//! summon actions of `RequestActionUse`, `ServitorShare`, the exp penalty, death and decay, the
//! summon target types. Expected values follow the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PetSkillData, PlayerTemplates, SkillTemplates, SkillTrees, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    PetDelete, PetInfo, PetStatusUpdate, SetSummonRemainTime, SummonInfo, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::effects::SkillKey;
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

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    /// The summon table writes since the last call.
    fn summon_writes(&self) -> Vec<SummonWrite> {
        let mut commands = self.0.lock().unwrap();
        let writes = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Summon(w) => Some(w.clone()),
                _ => None,
            })
            .collect();
        commands.clear();
        writes
    }
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

/// 1111 is the shape of the datapack's Summon Kat the Cat (20 s of life, an ore every 10 s);
/// 1557 the datapack's Servitor Share; the 9xxx skills are made for one rule each.
const SKILLS: &str = r##"<list>
<skill id="1111" levels="1" name="Summon Kat the Cat">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<hitTime>500</hitTime>
	<isMagic>1</isMagic>
	<conditions><player canSummon="true" /></conditions>
	<effects>
		<effect name="Summon">
			<npcId>14111</npcId>
			<consumeItemId>3031</consumeItemId>
			<consumeItemCount>2</consumeItemCount>
			<consumeItemInterval>10</consumeItemInterval>
			<lifeTime>20</lifeTime>
			<expMultiplier>0.5</expMultiplier>
		</effect>
	</effects>
</skill>
<skill id="1557" levels="1" name="Servitor Share">
	<operateType>A2</operateType>
	<targetType>SELF</targetType>
	<abnormalLevel>1</abnormalLevel>
	<abnormalTime>1200</abnormalTime>
	<abnormalType>ABILITY_CHANGE</abnormalType>
	<isMagic>1</isMagic>
	<effects>
		<effect name="ServitorShare">
			<pAtk>1.5</pAtk>
			<maxHp>1.1</maxHp>
		</effect>
	</effects>
</skill>
<skill id="9101" levels="1" name="Banish Servitor"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-100</effectPoint><isMagic>1</isMagic>
<effects><effect name="Unsummon"><param chance="100"/></effect></effects></skill>
<skill id="9102" levels="1" name="Summon Buff"><operateType>A2</operateType><targetType>SERVITOR</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel><isMagic>1</isMagic>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9103" levels="1" name="Owner Heal"><operateType>A1</operateType><targetType>OWNER_PET</targetType>
<effectPoint>100</effectPoint><isMagic>1</isMagic>
<effects><effect name="Heal"><param power="10"/></effect></effects></skill>
<skill id="4025" levels="2" name="Master Recharge"><operateType>A1</operateType><targetType>OWNER_PET</targetType>
<castRange>600</castRange><effectPoint>100</effectPoint><hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>5</mpConsume>
<effects><effect name="ManaHeal"><param power="20"/></effect></effects></skill>
<skill id="5110" levels="10" name="Cannon Fodder"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-100</effectPoint><isMagic>1</isMagic></skill>
</list>"##;

/// The spirit ore Kat eats.
const SPIRIT_ORE: i32 = 3031;

/// 14111 the datapack's Kat the Cat (stats rounded), 9001 a monster.
const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="14111" level="22" type="Pet" name="Kat the Cat">
    <parameters><skill name="HealMagic" id="4025" level="2" /></parameters>
    <race>ELEMENTAL</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="387" hpRegen="3.5" mp="218" mpRegen="1.5" />
      <attack physical="51" magical="35" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="92" magical="67" />
      <speed><walk ground="80" /><run ground="120" /></speed>
    </stats>
    <shots soul="1" spirit="1" />
    <collision><radius normal="9"/><height normal="16"/></collision>
  </npc>
<npc id="9001" type="Monster" level="1" name="Gremlin">
    <race>FAIRY</race>
    <acquire exp="1000" sp="100" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        (item_id == SPIRIT_ORE).then(|| ItemStatic::unknown(item_id))
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == SPIRIT_ORE
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

/// Levels 1..=30 of `experience.xml` (enough for the level 22 summon and the monster kill).
const EXP: [i64; 30] = [
    0, 68, 364, 1169, 2799, 5689, 10521, 18149, 29775, 46645, 70581, 103707, 148453, 207883, 284889, 382585,
    504265, 653421, 833889, 1050163, 1306997, 1609583, 1963577, 2374935, 2849807, 3394753, 4016663, 4722783,
    5520693, 6418300,
];

fn experience() -> Arc<ExperienceTable> {
    let rows: String = EXP
        .iter()
        .enumerate()
        .map(|(i, exp)| format!(r#"<experience level="{}" tolevel="{exp}" />"#, i + 1))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="29" maxPetLevel="29">{rows}</table>"#
        ))
        .unwrap(),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);
const SUMMON_KAT: i32 = 1111;
const SERVITOR_SHARE: i32 = 1557;
const BANISH: i32 = 9101;
const SUMMON_BUFF: i32 = 9102;
const OWNER_HEAL: i32 = 9103;

fn ore(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x2100_0001),
        item_id: SPIRIT_ORE,
        count,
        loc: "INVENTORY".into(),
        ..Default::default()
    }
}

fn player_data(id: ObjectId, x: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0),
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
        items,
        skills: [SUMMON_KAT, SERVITOR_SHARE]
            .into_iter()
            .map(|skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    me: Log,
    p2: Log,
    persisted: Persisted,
}

impl Scene {
    fn new(items: Vec<ItemData>) -> Self {
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::default()),
            Arc::new(KarmaLoss::default()),
            Arc::new(SkillTrees::default()),
        );
        let skills = stat_data().skills;
        world.set_pet_skill_data(Arc::new(
            PetSkillData::from_xml(
                r#"<list><skill npcId="14111" skillId="5110" skillLevel="0" /></list>"#,
                &|id, level| skills.get(id, level).is_some(),
            )
            .unwrap(),
        ));
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, X, items)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(P2, X + 300, vec![])),
            sink: Box::new(Sink(p2.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            p2,
            persisted,
        };
        scene.advance(1000);
        scene.me.take();
        scene.p2.take();
        scene.persisted.summon_writes();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// The `Summon` effect of 1111, as `Skill.applyEffects` runs it.
    fn summon(&mut self) -> ObjectId {
        self.world.apply_effects(ME, ME, SkillKey::new(SUMMON_KAT, 1));
        self.world.summon_of(ME).expect("a servitor")
    }

    fn action(&mut self, action_id: i32, ctrl: bool) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::ActionUse {
                action_id,
                ctrl_pressed: ctrl,
                shift_pressed: false,
            },
        });
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names(packets: &[Vec<u8>]) -> Vec<String> {
    packets.iter().map(|p| name_of(p)).collect()
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

fn ore_count(world: &World) -> i64 {
    world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == SPIRIT_ORE)
        .map(|i| i.count)
        .sum()
}

// ---- summoning ---------------------------------------------------------------------------------

/// `Summon.onStart` -> `spawnMe` -> `Summon.sendInfo` (the owner's `PetInfo` with the summon
/// animation, everybody else's `SummonInfo`), then `Summon.onSpawn`: `updateAndBroadcastStatus(0)`
/// (`PetInfo` and `PetStatusUpdate`), the owner's `RelationChanged`, and `setRunning`'s
/// `ChangeMoveType`. The servitor is level 22 with the exp of its level, full HP and MP, the
/// template's shots per hit, the owner's name as title and 20 s of life.
#[test]
fn summoning_shows_the_servitor_to_owner_and_viewers() {
    let mut s = Scene::new(vec![ore(10)]);
    let summon = s.summon();
    assert!(s.world.is_servitor(summon));
    assert_eq!(s.world.summon_owner(summon), Some(ME));
    assert_eq!(s.world.object(summon).unwrap().kind, ObjectKind::Summon);

    let mine = s.me.take();
    let infos: Vec<PetInfo> = of(&mine, "PET_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(infos.len() >= 2, "{:?}", names(&mine));
    // `sendInfo` with the summon animation, then `updateAndBroadcastStatus(0)` (still animated:
    // `setShowSummonAnimation(false)` comes after).
    assert_eq!(infos[0].spawn_state, 2);
    let info = &infos[0];
    assert_eq!(info.summon_type, 1);
    assert_eq!(info.object_id, summon.0);
    assert_eq!(info.display_id, 14111);
    assert_eq!(info.level, 22);
    assert_eq!(info.title, format!("P{}", ME.0));
    // A servitor without a server side name shows the client's (`""`).
    assert_eq!(info.name, "");
    assert_eq!((info.cur_fed, info.max_fed), (20_000, 20_000));
    assert_eq!(info.cur_hp, info.max_hp);
    assert_eq!(info.exp, EXP[21], "getExpForLevel(22 % 30)");
    assert_eq!(info.exp_for_this_level, EXP[21]);
    assert_eq!(info.exp_for_next_level, EXP[22]);
    assert_eq!((info.soul_shots_per_hit, info.spirit_shots_per_hit), (1, 1));
    assert!(info.has_owner);
    let status: PetStatusUpdate = decode(of(&mine, "PET_STATUS_UPDATE")[0]);
    assert_eq!((status.summon_type, status.level), (1, 22));
    assert!(!of(&mine, "RELATION_CHANGED").is_empty());

    let theirs = s.p2.take();
    let seen: Vec<SummonInfo> = of(&theirs, "NPC_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(!seen.is_empty(), "{:?}", names(&theirs));
    assert_eq!(seen[0].object_id, summon.0);
    assert_eq!(seen[0].spawn_state, 2);
    assert_eq!(seen[0].name, "Kat the Cat");
    assert_eq!(seen[0].title, format!("P{}", ME.0));
    // `Summon.isAutoAttackable(player)`: the owner's (not attackable).
    assert!(!seen[0].attackable);

    // Only one summon at a time: the effect does nothing for a player who has one.
    s.world.apply_effects(ME, ME, SkillKey::new(SUMMON_KAT, 1));
    assert_eq!(s.world.summon_of(ME), Some(summon));
}

/// `Servitor.run` every 5 s: the life time goes down, every 10 s (`consumeItemInterval`) two ores
/// are eaten with "A summoned monster uses $s1.", and the owner sees `SetSummonRemainTime`.
/// When the ore is gone the servitor leaves ("Since you do not have enough items ...").
#[test]
fn the_life_task_eats_ore_and_counts_down() {
    let mut s = Scene::new(vec![ore(2)]);
    let summon = s.summon();
    s.me.take();
    // The first run comes at once (Java's delay 0): 5 s gone.
    s.advance(100);
    let first = s.me.take();
    let remain: SetSummonRemainTime = decode(of(&first, "SET_SUMMON_REMAIN_TIME")[0]);
    assert_eq!((remain.max_time, remain.remaining_time), (20_000, 15_000));
    // The second run: 10 s of the interval are used, two ores go.
    s.advance(5000);
    let second = s.me.take();
    assert!(message_ids(&second).contains(&SystemMessageId::ASummonedMonsterUsesS1));
    assert_eq!(ore_count(&s.world), 0);
    // The fourth run: no ore left.
    s.advance(10_000);
    let last = s.me.take();
    assert!(
        message_ids(&last).contains(
            &SystemMessageId::SinceYouDoNotHaveEnoughItemsToMaintainTheServitorSStayTheServitorHasDisappeared
        ),
        "{:?}",
        message_ids(&last)
    );
    let delete: PetDelete = decode(of(&last, "PET_DELETE")[0]);
    assert_eq!((delete.pet_type, delete.pet_object_id), (1, summon.0));
    assert_eq!(s.world.summon_of(ME), None);
    assert!(s.world.object(summon).is_none());
}

/// A servitor whose time is up: "Your servitor passed away." and it leaves; `saveSummon` writes
/// nothing for it (no time left), `storeEffect` replaces its saved buffs, and the row of
/// `character_summons` goes (`!_restoreSummon` -> `removeServitor`).
#[test]
fn the_servitor_passes_away_when_its_time_is_up() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.me.take();
    s.persisted.summon_writes();
    s.advance(21_000);
    let packets = s.me.take();
    assert!(message_ids(&packets).contains(&SystemMessageId::YourServitorPassedAway));
    assert!(s.world.object(summon).is_none());
    let writes = s.persisted.summon_writes();
    assert_eq!(
        writes,
        vec![
            SummonWrite::ServitorEffects {
                owner: ME,
                class_index: 0,
                summon_skill_id: SUMMON_KAT,
                effects: vec![],
            },
            SummonWrite::RemoveServitor { owner: ME },
        ]
    );
}

/// Action 52 `Unsummon Servitor`: `PetDelete` to the owner, the row is written (`saveSummon`) and
/// removed again (`removeServitor`, no restore), the buffs are saved for the next summon of the
/// same skill. Without a servitor: "You do not have a servitor."
#[test]
fn action_52_unsummons_and_writes_javas_rows() {
    let mut s = Scene::new(vec![ore(100)]);
    s.action(52, false);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YouDoNotHaveAServitor]
    );

    let summon = s.summon();
    // A buff on the servitor is kept for its next summon.
    s.world.apply_effects(ME, summon, SkillKey::new(SUMMON_BUFF, 1));
    s.advance(100);
    s.me.take();
    s.persisted.summon_writes();
    s.action(52, false);
    let packets = s.me.take();
    assert!(!of(&packets, "PET_DELETE").is_empty(), "{:?}", names(&packets));
    assert_eq!(s.world.summon_of(ME), None);
    let writes = s.persisted.summon_writes();
    assert_eq!(writes.len(), 3, "{writes:?}");
    assert!(matches!(
        writes[0],
        SummonWrite::SaveServitor {
            owner: ME,
            summon_skill_id: SUMMON_KAT,
            ..
        }
    ));
    match &writes[1] {
        SummonWrite::ServitorEffects { effects, .. } => {
            assert_eq!(effects.len(), 1);
            assert_eq!((effects[0].skill_id, effects[0].skill_level), (SUMMON_BUFF, 1));
            assert!(effects[0].remaining_time <= 60 && effects[0].remaining_time > 50);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(writes[2], SummonWrite::RemoveServitor { owner: ME });

    // The next Kat comes back with the buff (`restoreEffects` from the memory table).
    let again = s.summon();
    assert!(
        s.world
            .effect_icons(again)
            .iter()
            .any(|i| i.skill_id == SUMMON_BUFF)
    );
}

/// `Player.deleteMe`: the servitor is stored for a restore (`setRestoreSummon(true)`): the
/// `character_summons` row stays.
#[test]
fn the_owner_leaving_stores_the_servitor_for_a_restore() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.persisted.summon_writes();
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Logout,
    });
    assert!(s.world.object(summon).is_none());
    let writes = s.persisted.summon_writes();
    assert!(matches!(writes[0], SummonWrite::SaveServitor { owner: ME, time, .. } if time == 20_000));
    assert!(
        !writes.contains(&SummonWrite::RemoveServitor { owner: ME }),
        "{writes:?}"
    );
    // P2 saw it go.
    assert!(!of(&s.p2.take(), "DELETE_OBJECT").is_empty());
}

// ---- orders ------------------------------------------------------------------------------------

/// The servitor follows its owner after the summon; action 21 switches the movement mode (idle),
/// 23 stops it, 21 again follows.
#[test]
fn movement_mode_and_stop_actions_drive_the_summon_ai() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    assert_eq!(s.world.intention_of(summon), Intention::Follow);
    assert_eq!(s.world.follow_target_of(summon), Some(ME));
    s.action(21, false);
    assert_eq!(s.world.intention_of(summon), Intention::Active);
    assert!(!s.world.follow_status(summon));
    s.action(21, false);
    assert_eq!(s.world.intention_of(summon), Intention::Follow);
    // Action 15 is the pet's: a servitor answers it too (`checkPet && isPet() || isServitor()`).
    s.action(15, false);
    assert_eq!(s.world.intention_of(summon), Intention::Active);
}

/// Action 22: the servitor attacks the owner's target (`canAttack`, `doSummonAttack`); without a
/// target nothing happens.
#[test]
fn action_22_sends_the_servitor_at_the_owners_target() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.action(22, false);
    assert_eq!(s.world.intention_of(summon), Intention::Follow);
    let mob = s.world.spawn_npc_at(9001, X + 150, Y, Z, 0).unwrap();
    s.world.set_target(ME, Some(mob));
    s.action(22, false);
    assert_eq!(s.world.intention_of(summon), Intention::Attack);
    assert_eq!(s.world.attack_target_of(summon), Some(mob));
}

/// The skill actions: 45 (`HealMagic` of the template's parameters, on the owner) and 1039 (skill
/// 5110, the level from `PetSkillData`: level 0 = `22 / 10` for a level 22 summon).
/// `SummonAI.thinkCast` stops following (`setFollowStatus(false)`, `setIntentionIdle()`) before
/// `doCast`, and `notifyActionFinishCasting` follows the owner again.
#[test]
fn skill_actions_use_the_template_and_pet_skill_data() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    assert_eq!(s.world.pet_skill_available_level(summon, 5110), 2);
    assert_eq!(s.world.pet_skill_available_level(summon, 9999), 0);
    s.world.set_current_mp(ME, 10.0, false);
    s.action(45, false);
    assert!(s.world.is_casting_now(summon));
    assert_eq!(s.world.cast_skill_of(summon), Some(SkillKey::new(4025, 2)));
    assert_eq!(s.world.intention_of(summon), Intention::Active);
    s.advance(2000);
    assert!(!s.world.is_casting_now(summon));
    assert_eq!(
        s.world.player(ME).unwrap().core.cur_mp,
        30.0,
        "Master Recharge: 20 MP"
    );
    assert_eq!(s.world.intention_of(summon), Intention::Follow);
}

// ---- effects -----------------------------------------------------------------------------------

/// `Servitor.getPAtk(target)`: plus the owner's P. Atk. times `1.5 - 1`; `getMaxHp()` plus the
/// owner's times `0.1`, cast to int. `onExit` takes it away again.
#[test]
fn servitor_share_adds_part_of_the_owners_stats() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    let p_atk = s.world.stats(summon).unwrap().p_atk(None);
    let max_hp = s.world.stats(summon).unwrap().max_hp();
    let owner_p_atk = s.world.stats(ME).unwrap().p_atk(None);
    let owner_max_hp = s.world.stats(ME).unwrap().max_hp();
    s.world.apply_effects(ME, ME, SkillKey::new(SERVITOR_SHARE, 1));
    let shared = s.world.stats(summon).unwrap();
    assert_eq!(shared.p_atk(None), p_atk + owner_p_atk * 0.5);
    assert_eq!(
        shared.max_hp(),
        (f64::from(max_hp) + f64::from(owner_max_hp) * (1.1 - 1.0)) as i32
    );
    assert_eq!(s.world.servitor_share_bonus(ME, l2_data::Stat::PowerAttack), 1.5);
}

/// `Unsummon` on a servitor: it leaves and its owner hears "Your servitor has vanished!".
#[test]
fn the_unsummon_effect_sends_the_servitor_away() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.me.take();
    s.world.apply_effects(P2, summon, SkillKey::new(BANISH, 1));
    let packets = s.me.take();
    assert!(
        message_ids(&packets).contains(&SystemMessageId::YourServitorHasVanishedYouLlNeedToSummonANewOne)
    );
    assert_eq!(s.world.summon_of(ME), None);
}

/// The target types: `SERVITOR` is the caster's servitor, `OWNER_PET` the summon's owner.
#[test]
fn summon_target_types_find_owner_and_servitor() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    let skills = stat_data().skills;
    let ctx = l2_world::skills::targets::TargetContext::default();
    let servitor = skills.get(SUMMON_BUFF, 1).unwrap();
    assert_eq!(
        s.world.skill_target_list(servitor, ME, false, None, &ctx).targets,
        vec![summon]
    );
    let owner_pet = skills.get(OWNER_HEAL, 1).unwrap();
    assert_eq!(
        s.world
            .skill_target_list(owner_pet, summon, false, None, &ctx)
            .targets,
        vec![ME]
    );
    // A player has no owner.
    assert!(
        s.world
            .skill_target_list(owner_pet, ME, false, None, &ctx)
            .targets
            .is_empty()
    );
}

// ---- death and rewards -------------------------------------------------------------------------

/// `Summon.doDie`: the corpse waits for its decay (`DecayTaskManager`), `Servitor.doDie` removes
/// the row; `onDecay` -> `deleteMe`: `PetDelete` to the owner.
#[test]
fn a_dead_servitor_decays_with_pet_delete() {
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.persisted.summon_writes();
    s.me.take();
    s.world.reduce_current_hp(summon, 10_000.0, Some(P2));
    assert!(s.world.is_dead(summon));
    assert!(s.world.has_decay_task(summon));
    let writes = s.persisted.summon_writes();
    assert!(
        writes.contains(&SummonWrite::RemoveServitor { owner: ME }),
        "{writes:?}"
    );
    let died = s.me.take();
    assert!(message_ids(&died).contains(&SystemMessageId::C1HasReceivedS3DamageFromC2));
    // The template's corpse time (the NPC default, 7 s).
    s.advance(8000);
    assert!(!of(&s.me.take(), "PET_DELETE").is_empty());
    assert!(s.world.object(summon).is_none());
    assert_eq!(s.world.summon_of(ME), None);
}

/// `Attackable.calculateRewards`: the summon's damage counts for its owner, and the owner's exp
/// is multiplied by the servitor's `expMultiplier` (0.5).
#[test]
fn the_servitor_damage_counts_for_the_owner_with_the_exp_penalty() {
    let mut with = Scene::new(vec![ore(100)]);
    let summon = with.summon();
    let mob = with.world.spawn_npc_at(9001, X + 100, Y, Z, 0).unwrap();
    with.world.reduce_current_hp(mob, 10_000.0, Some(summon));
    let exp_with = with.world.player(ME).unwrap().data.exp;

    let mut without = Scene::new(vec![]);
    let mob = without.world.spawn_npc_at(9001, X + 100, Y, Z, 0).unwrap();
    without.world.reduce_current_hp(mob, 10_000.0, Some(ME));
    let exp_without = without.world.player(ME).unwrap().data.exp;
    assert!(exp_without > 0);
    assert_eq!(exp_with, (exp_without as f64 * 0.5).round() as i64);
}

// ---- the party ---------------------------------------------------------------------------------

/// `Party.addPartyMember`: the new member gets the summon block of `PartySmallWindowAll` and an
/// `ExPartyPetWindowAdd` of every member's summon; `Summon.updateAndBroadcastStatus` keeps the
/// other members' pet window up to date, and the summon's buffs reach them as `PartySpelled`
/// (type 2, servitor). Leaving the party sends `ExPartyPetWindowDelete`.
#[test]
fn party_members_see_the_servitor_in_their_pet_window() {
    use l2_protocol_game::server::{
        ExPartyPetWindowAdd, ExPartyPetWindowDelete, PartySmallWindowAll, PartySpelled,
    };
    let mut s = Scene::new(vec![ore(100)]);
    let summon = s.summon();
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::JoinParty {
            name: format!("P{}", P2.0),
            distribution_type_id: 0,
        },
    });
    s.world.handle(WorldCommand::Input {
        session: SessionId(2),
        input: ClientInput::AnswerJoinParty { response: 1 },
    });
    let theirs = s.p2.take();
    let all: PartySmallWindowAll = decode(of(&theirs, "PARTY_SMALL_WINDOW_ALL")[0]);
    let block = all.members[0].summon.as_ref().expect("the summon block");
    assert_eq!(
        (block.object_id, block.display_id, block.summon_type, block.level),
        (summon.0, 14111, 1, 22)
    );
    let add: ExPartyPetWindowAdd = decode(of(&theirs, "EX_PARTY_PET_WINDOW_ADD")[0]);
    assert_eq!(add.window.owner_object_id, ME.0);
    assert_eq!(add.window.summon.object_id, summon.0);

    s.world.apply_effects(ME, summon, SkillKey::new(SUMMON_BUFF, 1));
    s.advance(500);
    let theirs = s.p2.take();
    assert!(
        !of(&theirs, "EX_PARTY_PET_WINDOW_UPDATE").is_empty(),
        "{:?}",
        names(&theirs)
    );
    let spelled: Vec<PartySpelled> = of(&theirs, "PARTY_SPELLED")
        .into_iter()
        .map(|p| decode(p))
        .collect();
    let of_summon = spelled
        .iter()
        .find(|p| p.creature_id == summon.0)
        .expect("the summon's icons");
    assert_eq!(of_summon.creature_type, PartySpelled::SERVITOR);
    assert_eq!(of_summon.effects.len(), 1);
    // The owner gets them too.
    let mine = s.me.take();
    assert!(
        of(&mine, "PARTY_SPELLED")
            .into_iter()
            .map(|p| decode::<PartySpelled>(p))
            .any(|p| p.creature_id == summon.0)
    );

    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::WithdrawParty,
    });
    let theirs = s.p2.take();
    let gone: Vec<ExPartyPetWindowDelete> = of(&theirs, "EX_PARTY_PET_WINDOW_DELETE")
        .into_iter()
        .map(|p| decode(p))
        .collect();
    assert_eq!(
        gone.first().map(|d| d.summon_object_id),
        Some(summon.0),
        "{:?}",
        names(&theirs)
    );
}
