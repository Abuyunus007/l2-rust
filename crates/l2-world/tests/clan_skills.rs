#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Clan skills and the academy (M6-CLAN-03): the skills the members get, the reputation switch,
//! the clan trainer's lists, `RequestAcquireSkillInfo` / `RequestAcquireSkill` for the clan and
//! squad skills, the `clan_skills` rows and the academy graduation.
//! Java: entity/clan/Clan.java (addNewSkill, addSkillEffects, removeSkillEffects, skillsStatus,
//! isLearnableSubSkill, isLearnableSubPledgeSkill), data/xml/SkillTreeData.java,
//! entity/actor/instance/VillageMaster.java (showPledgeSkillList),
//! network/clientpackets/{RequestAcquireSkill,RequestAcquireSkillInfo}.java,
//! entity/actor/Player.java (setPlayerClass: the academy block).
//!
//! The order of the trainer's lists is the `ConcurrentHashMap` order of `skillId * 1021 + level`
//! (bucket `(h ^ h >>> 16) & 15` for the few lines here, worked out by hand); the messages are the
//! ones Java sends; the numbers are the ones of `pledgeSkillTree.xml` / `subPledgeSkillTree.xml`.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
    SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::pledge::{
    ExSubPledgeSkillAdd, PledgeShowMemberListDelete, PledgeSkillList, PledgeSkillListAdd,
};
use l2_protocol_game::server::{
    AcquireSkillInfo, AcquireSkillList, AcquireSkillRequirement, AcquireSkillType, NpcHtmlMessage, SkillList,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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

#[derive(Clone, Default)]
struct Outs(Arc<Mutex<Vec<PersistCommand>>>);

impl Outs {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Recorder(Outs);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Class 0 at level 1.
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

/// The clan skills 370 to 372 and 590, the squad skills 611 and 612, and a plain passive 500.
const SKILLS: &str = r#"<list>
<skill id="370" levels="2" name="Clan Body"><operateType>P</operateType></skill>
<skill id="371" levels="1" name="Clan Spirituality"><operateType>P</operateType></skill>
<skill id="372" levels="1" name="Clan Soul"><operateType>P</operateType></skill>
<skill id="590" levels="1" name="Residence Body"><operateType>P</operateType></skill>
<skill id="611" levels="2" name="Fire Squad"><operateType>P</operateType></skill>
<skill id="612" levels="1" name="Water Squad"><operateType>P</operateType></skill>
<skill id="500" levels="1" name="Plain"><operateType>P</operateType></skill>
</list>"#;

/// The lines of `pledgeSkillTree.xml` and `subPledgeSkillTree.xml` these tests need, with the
/// real numbers (Clan Body 1 and 2, Clan Spirituality 1, Fire Squad 1 and 2, Water Squad 1); Clan
/// Soul is the Baron skill of the real tree with the same shape.
const TREES: &str = r#"<list>
<skillTree type="pledgeSkillTree">
  <skill skillName="Clan Body" skillId="370" skillLevel="1" getLevel="5" levelUpSp="1500">
    <socialClass>APPRENTICE</socialClass><item id="9816" count="10" /></skill>
  <skill skillName="Clan Body" skillId="370" skillLevel="2" getLevel="7" levelUpSp="6900">
    <socialClass>APPRENTICE</socialClass><item id="9816" count="10" /></skill>
  <skill skillName="Clan Spirituality" skillId="371" skillLevel="1" getLevel="6" levelUpSp="2100">
    <socialClass>ELDER</socialClass><item id="9817" count="10" /></skill>
  <skill skillName="Clan Soul" skillId="372" skillLevel="1" getLevel="8" levelUpSp="12000">
    <socialClass>BARON</socialClass><item id="9818" count="10" /></skill>
  <skill skillName="Residence Body" skillId="590" skillLevel="1" getLevel="4" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>1</residenceId></skill>
</skillTree>
<skillTree type="subPledgeSkillTree">
  <skill skillName="Fire Squad" skillId="611" skillLevel="1" getLevel="8" levelUpSp="12000">
    <socialClass>KNIGHT</socialClass><item id="9910" count="8" /></skill>
  <skill skillName="Fire Squad" skillId="611" skillLevel="2" getLevel="9" levelUpSp="17400">
    <socialClass>KNIGHT</socialClass><item id="9910" count="11" /></skill>
  <skill skillName="Water Squad" skillId="612" skillLevel="1" getLevel="7" levelUpSp="10400">
    <socialClass>KNIGHT</socialClass><item id="9910" count="4" /></skill>
</skillTree>
</list>"#;

const ITEMS: &str = r#"<list>
<item id="8181" type="EtcItem" name="Academy Circlet"><set name="weight" val="1" /></item>
<item id="9816" type="EtcItem" name="Life Stone"><set name="is_stackable" val="true" /></item>
<item id="9818" type="EtcItem" name="Soul Stone"><set name="is_stackable" val="true" /></item>
<item id="9910" type="EtcItem" name="Squad Stone"><set name="is_stackable" val="true" /></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic {
            display_id: item_id,
            type2: if item_id == 57 { 4 } else { 5 },
            body_part: 0,
            quest_item: false,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        true
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        matches!(item_id, 57 | 9816 | 9817 | 9818 | 9910)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
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

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn messages_to(out: &Sent, to: i32) -> Vec<SystemMessage> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
}

fn ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).into_iter().map(|m| m.id).collect()
}

fn has_message(out: &Sent, to: i32, id: SystemMessageId, params: &[SystemMessageParam]) -> bool {
    messages_to(out, to)
        .iter()
        .any(|m| m.id == id && m.params == params)
}

fn clan_writes(outs: &[PersistCommand]) -> Vec<ClanWrite> {
    outs.iter()
        .filter_map(|o| match o {
            PersistCommand::Clan(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const P5: i32 = 0x6000_0005;
const CLAN: i32 = 500;
/// The village master of the tests.
const VM: i32 = 30026;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30026" type="VillageMasterFighter" level="70" name="Bitz" usingServerSideName="true">
    <status attackable="false" />
  </npc>
</list>"#;

/// The village master's page with the link of the clan skills (`9000-01.htm` of `ClanMaster`).
const VM_PAGE: &str = "<html><body>\
    <a action=\"bypass -h npc_%objectId%_learn_clan_skills\">skills</a>\
    </body></html>";

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    skills: Arc<SkillTemplates>,
    vm: Option<ObjectId>,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with(ClanSettings::default())
    }

    fn with(settings: ClanSettings) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write(&format!("data/html/villagemaster/{VM}.htm"), VM_PAGE);
        write(
            "data/html/villagemaster/NotClanLeader.htm",
            "<html><body>not qualified</body></html>",
        );
        write(
            "data/html/villagemaster/NoMoreSkills.htm",
            "<html><body>learned all</body></html>",
        );
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(outs.clone())));
        let stats = stat_data();
        let skills = Arc::clone(&stats.skills);
        world.set_static_data(Box::new(Data(stats)));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_clan_settings(settings);
        Self {
            world,
            log: Log::default(),
            outs,
            skills,
            vm: None,
            _root: root,
        }
    }

    fn load(&mut self, records: Vec<ClanRecord>) {
        self.world.load_clans(records, Vec::new());
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn clan_input(&mut self, id: i32, input: ClanInput) {
        self.input(id, ClientInput::Clan(input));
    }

    /// The player talks to the village master (spawned on first use).
    fn talk(&mut self, id: i32) -> ObjectId {
        let npc = match self.vm {
            Some(npc) => npc,
            None => {
                let npc = self.world.spawn_npc_at(VM, X + 50, Y, Z, 0).unwrap();
                self.vm = Some(npc);
                npc
            }
        };
        for _ in 0..2 {
            self.input(
                id,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
        }
        npc
    }

    /// `npc_<vm>_<command>` after talking to the village master.
    fn vm_command(&mut self, id: i32, command: &str) {
        let npc = self.talk(id);
        self.log.take();
        self.input(
            id,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        );
    }

    /// `RequestAcquireSkill` of the player at the village master.
    fn acquire(&mut self, id: i32, skill_id: i32, level: i32, skill_type: AcquireSkillType, sub_type: i32) {
        self.talk(id);
        self.log.take();
        self.outs.take();
        self.input(
            id,
            ClientInput::AcquireSkill {
                id: skill_id,
                level,
                skill_type,
                sub_type,
            },
        );
    }

    fn acquire_info(&mut self, id: i32, skill_id: i32, level: i32, skill_type: AcquireSkillType) {
        self.talk(id);
        self.log.take();
        self.input(
            id,
            ClientInput::AcquireSkillInfo {
                id: skill_id,
                level,
                skill_type,
            },
        );
    }

    fn take(&mut self) -> Sent {
        self.log.take()
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms / 50 {
            self.world.tick();
        }
    }

    fn clan(&self) -> &Clan {
        self.world.clan(CLAN).unwrap()
    }

    /// The skills a player has (id, level), sorted.
    fn skills_of(&self, id: i32) -> Vec<(i32, i32)> {
        let mut skills = self.world.player(ObjectId(id)).unwrap().core.skills.clone();
        skills.sort_unstable();
        skills
    }

    fn is_disabled(&mut self, id: i32, skill_id: i32, level: i32) -> bool {
        let skills = Arc::clone(&self.skills);
        self.world
            .is_skill_disabled(ObjectId(id), skills.get(skill_id, level).unwrap())
    }
}

fn pdata(id: i32, level: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x6000_0000),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X + (id & 0xff),
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

/// A character of clan `clan_id` as the loader gives it.
fn member_data(id: i32, clan_id: i32, power_grade: i32, pledge_type: i32) -> PlayerData {
    let mut data = pdata(id, 40);
    data.profile.clan = Some(PlayerClanState {
        clan_id,
        power_grade,
        pledge_type,
        ..PlayerClanState::default()
    });
    data
}

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".to_owned(),
        ..Default::default()
    }
}

fn member(id: i32, power_grade: i32, pledge_type: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id,
        name: format!("P{}", id - 0x6000_0000),
        level: 40,
        class_id: 0,
        title: String::new(),
        power_grade,
        pledge_type,
        apprentice: 0,
        sponsor: 0,
        female: false,
        race: 0,
    }
}

/// Clan 500 "Lions" of `level` and `reputation`, led by P1: P2 a member, P3 in the royal guard
/// 100, P4 in the academy.
fn lions(level: i32, reputation: i32) -> ClanRecord {
    ClanRecord {
        clan_id: CLAN,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level,
            reputation_score: reputation,
            leader_id: P1,
            ..ClanDataRecord::default()
        }),
        members: vec![
            member(P1, 1, 0),
            member(P2, 5, 0),
            member(P3, 5, 100),
            member(P4, 9, -1),
        ],
        sub_pledges: vec![(100, "Royal".to_owned(), P3), (-1, "Cubs".to_owned(), 0)],
        ..ClanRecord::default()
    }
}

/// Lions with the three clan skills and a skill in the main squad and the royal guard.
fn lions_with_skills(level: i32, reputation: i32) -> ClanRecord {
    let mut record = lions(level, reputation);
    record.skills = vec![
        (370, 1, -2),
        (371, 1, -2),
        (372, 1, -2),
        (611, 1, 0),
        (612, 1, 100),
    ];
    record
}

/// The four members enter, the log is cleared.
fn enter_all(s: &mut Scene) {
    s.enter(member_data(P1, CLAN, 0, 0));
    s.enter(member_data(P2, CLAN, 0, 0));
    s.enter(member_data(P3, CLAN, 5, 100));
    let mut p4 = member_data(P4, CLAN, 9, -1);
    p4.profile.clan = p4.profile.clan.map(|c| PlayerClanState {
        lvl_joined_academy: 20,
        ..c
    });
    s.enter(p4);
    s.take();
}

fn level_one_ids(list: &AcquireSkillList) -> Vec<(i32, i32, i32, i32)> {
    list.skills
        .iter()
        .map(|e| (e.id, e.next_level, e.sp_cost, e.requirements))
        .collect()
}

// ---------------------------------------------------------------------------------- members

#[test]
fn members_get_the_skills_their_pledge_class_reaches() {
    let mut s = Scene::new();
    s.load(vec![lions_with_skills(8, 1000)]);
    enter_all(&mut s);
    // A clan of level 8: the leader is class 8 (Baron needs ordinal 6 <= 9), a main member 5
    // (6 >= Baron), the royal guard member 4, the academy member 1 (2 >= Apprentice only).
    // Clan Body is Apprentice (2), Clan Spirituality Elder (5), Clan Soul Baron (6).
    assert_eq!(s.skills_of(P1), [(370, 1), (371, 1), (372, 1), (611, 1)]);
    // Fire Squad (611, the main squad) needs a Knight (4): class 5 + 1 = 6 reaches it.
    assert_eq!(s.skills_of(P2), [(370, 1), (371, 1), (372, 1), (611, 1)]);
    // The royal guard (100) gets its squad's Water Squad without a class check; class 4 + 1 = 5
    // reaches Clan Body and Clan Spirituality but not Clan Soul.
    assert_eq!(s.skills_of(P3), [(370, 1), (371, 1), (612, 1)]);
    // The academy member: class 1 + 1 = 2 is Apprentice. Its squad (-1) has no skills.
    assert_eq!(s.skills_of(P4), [(370, 1)]);
    // None of them is a row of `character_skills`.
    for id in [P1, P2, P3, P4] {
        assert!(s.world.player(ObjectId(id)).unwrap().data.skills.is_empty());
    }
}

#[test]
fn the_skill_list_of_enter_world_shows_the_clan_skills() {
    let mut s = Scene::new();
    let mut record = lions(8, 1000);
    record.skills = vec![(370, 1, -2)];
    s.load(vec![record]);
    let mut data = member_data(P1, CLAN, 0, 0);
    data.skills = vec![SkillData {
        skill_id: 500,
        skill_level: 1,
    }];
    s.enter(data);
    s.take();
    // The second `SkillList`, 300 ms later, is built from what the player knows.
    s.run_ms(400);
    let out = s.take();
    let list = &decoded::<SkillList>(&out, P1, "SKILL_LIST")[0];
    let mut ids: Vec<(i32, bool)> = list.skills.iter().map(|e| (e.id, e.disabled)).collect();
    ids.sort_unstable();
    assert_eq!(ids, [(370, false), (500, false)]);
}

#[test]
fn a_negative_reputation_switches_the_clan_skills_off() {
    let mut s = Scene::new();
    let mut record = lions(8, -5);
    record.skills = vec![(370, 1, -2)];
    s.load(vec![record]);
    let mut data = member_data(P1, CLAN, 0, 0);
    data.skills = vec![SkillData {
        skill_id: 500,
        skill_level: 1,
    }];
    s.enter(data);
    s.take();
    // `addSkillEffects`: `if (_reputationScore < 0) skillsStatus(player, true)`.
    assert!(s.is_disabled(P1, 370, 1));
    assert!(!s.is_disabled(P1, 500, 1));
    s.run_ms(400);
    let out = s.take();
    let list = &decoded::<SkillList>(&out, P1, "SKILL_LIST")[0];
    let mut flags: Vec<(i32, bool)> = list.skills.iter().map(|e| (e.id, e.disabled)).collect();
    flags.sort_unstable();
    // `isDisabled = skill.isClanSkill() && clan.getReputationScore() < 0`.
    assert_eq!(flags, [(370, true), (500, false)]);
}

#[test]
fn crossing_zero_disables_and_enables_the_skills_of_the_online_members() {
    let mut s = Scene::new();
    s.load(vec![lions_with_skills(8, 100)]);
    enter_all(&mut s);
    assert!(!s.is_disabled(P2, 370, 1));

    s.world.take_reputation_score(CLAN, 200);
    let out = s.take();
    for id in [P1, P2, P3, P4] {
        assert!(
            ids_to(&out, id).contains(
                &SystemMessageId::SinceTheClanReputationScoreHasDroppedTo0OrLowerYourClanSkillSWillBeDeActivated
            ),
            "{id}"
        );
    }
    assert_eq!(s.clan().reputation_score, -100);
    assert!(s.is_disabled(P2, 370, 1));
    assert!(s.is_disabled(P2, 611, 1));
    // A royal guard member: its squad's skill goes too, and the main squad's does not reach it.
    assert!(s.is_disabled(P3, 612, 1));
    assert!(s.is_disabled(P4, 370, 1));

    // Staying below zero says nothing more.
    s.world.take_reputation_score(CLAN, 10);
    assert!(s.take().iter().all(|(_, b)| name_of(b) != "SYSTEM_MESSAGE"));

    s.world.add_reputation_score(CLAN, 300);
    let out = s.take();
    assert!(
        ids_to(&out, P2)
            .contains(&SystemMessageId::ClanSkillsWillNowBeActivatedSinceTheClanSReputationScoreIs0OrHigher)
    );
    assert_eq!(s.clan().reputation_score, 190);
    assert!(!s.is_disabled(P2, 370, 1));
    assert!(!s.is_disabled(P3, 612, 1));
}

#[test]
fn leaving_takes_the_clan_skills_and_joining_gives_them() {
    let mut s = Scene::new();
    s.load(vec![lions_with_skills(8, 1000)]);
    enter_all(&mut s);
    assert_eq!(s.skills_of(P2), [(370, 1), (371, 1), (372, 1), (611, 1)]);
    s.clan_input(P2, ClanInput::WithdrawalPledge);
    assert!(s.skills_of(P2).is_empty());
    assert_eq!(s.skills_of(P1), [(370, 1), (371, 1), (372, 1), (611, 1)]);

    // P5 joins the main squad: the new member is class 5 (a level 8 clan), reaching every skill.
    s.enter(pdata(P5, 40));
    s.take();
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P5,
            pledge_type: 0,
        },
    );
    s.clan_input(P5, ClanInput::AnswerJoinPledge { answer: 1 });
    assert_eq!(s.skills_of(P5), [(370, 1), (371, 1), (372, 1), (611, 1)]);
    // Joining also sends the clan's skills to the new member.
    let out = s.take();
    assert_eq!(
        decoded::<PledgeSkillList>(&out, P5, "PLEDGE_SKILL_LIST")[0]
            .skills
            .len(),
        3
    );
}

// ---------------------------------------------------------------------------------- the lists

#[test]
fn the_clan_trainer_lists_the_next_level_of_each_skill() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 1000)]);
    enter_all(&mut s);
    s.vm_command(P1, "learn_clan_skills");
    let out = s.take();
    let list = &decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST")[0];
    assert_eq!(list.skill_type, AcquireSkillType::Pledge);
    // Level 1 of every skill a level 8 clan reaches, without the castle skill, in the map order
    // of `skillId * 1021 + level` (Clan Soul 0, Clan Spirituality 13, Clan Body 14): the cost
    // is the reputation, the last number the ordinal of the social class.
    assert_eq!(
        level_one_ids(list),
        [(372, 1, 12000, 6), (371, 1, 2100, 5), (370, 1, 1500, 2)]
    );

    // With Clan Body 1 the next line is level 2 (map order 9).
    let mut s = Scene::new();
    let mut record = lions(8, 1000);
    record.skills = vec![(370, 1, -2)];
    s.load(vec![record]);
    enter_all(&mut s);
    s.vm_command(P1, "learn_clan_skills");
    let out = s.take();
    let list = &decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST")[0];
    assert_eq!(
        level_one_ids(list),
        [(372, 1, 12000, 6), (370, 2, 6900, 2), (371, 1, 2100, 5)]
    );
}

#[test]
fn the_clan_trainer_answers_a_clan_that_cannot_learn_more() {
    // Not the leader.
    let mut s = Scene::new();
    s.load(vec![lions(8, 1000)]);
    enter_all(&mut s);
    s.vm_command(P2, "learn_clan_skills");
    let out = s.take();
    let page = &decoded::<NpcHtmlMessage>(&out, P2, "NPC_HTML_MESSAGE")[0];
    assert!(page.html.contains("not qualified"));
    assert_eq!(page.npc_obj_id, 0);
    assert!(names_to(&out, P2).contains(&"ACTION_FAIL".to_owned()));

    // Below level 5 the message names 5, from there the next level.
    for (level, shown) in [(3, 5), (4, 5), (5, 6), (6, 7), (7, 8)] {
        let mut s = Scene::new();
        // Nothing to learn: every skill is at its last level or needs a higher clan level.
        let mut record = lions(level, 0);
        record.skills = vec![(370, 2, -2), (371, 1, -2), (372, 1, -2)];
        s.load(vec![record]);
        enter_all(&mut s);
        s.vm_command(P1, "learn_clan_skills");
        let out = s.take();
        assert!(
            has_message(
                &out,
                P1,
                SystemMessageId::YouDoNotHaveAnyFurtherSkillsToLearnComeBackWhenYouHaveReachedLevelS1,
                &[SystemMessageParam::IntNumber(shown)]
            ),
            "level {level}"
        );
        assert!(decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST").is_empty());
    }

    // From level 8 on: the page.
    let mut s = Scene::new();
    let mut record = lions(8, 0);
    record.skills = vec![(370, 2, -2), (371, 1, -2), (372, 1, -2)];
    s.load(vec![record]);
    enter_all(&mut s);
    s.vm_command(P1, "learn_clan_skills");
    let out = s.take();
    let page = &decoded::<NpcHtmlMessage>(&out, P1, "NPC_HTML_MESSAGE")[0];
    assert!(page.html.contains("learned all"));
}

#[test]
fn the_squad_list_follows_is_learnable_sub_skill() {
    let mut s = Scene::new();
    let mut record = lions(8, 1000);
    record.data.as_mut().unwrap().castle_id = 1;
    s.load(vec![record]);
    enter_all(&mut s);
    s.world.show_sub_unit_skill_list(ObjectId(P1));
    let out = s.take();
    let list = &decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST")[0];
    assert_eq!(list.skill_type, AcquireSkillType::Subpledge);
    // Clan level 8: Fire Squad 1 (level 8) and Water Squad 1 (level 7), map order 1 and 12. Fire
    // Squad 2 needs level 9. The requirement column is 0.
    assert_eq!(level_one_ids(list), [(611, 1, 12000, 0), (612, 1, 10400, 0)]);

    // A squad that has Fire Squad 1 can take level 2, and the academy never counts. Another squad
    // still takes level 1 (Java asks every squad).
    let mut s = Scene::new();
    let mut record = lions(9, 1000);
    record.skills = vec![(611, 1, 100)];
    s.load(vec![record]);
    enter_all(&mut s);
    s.world.show_sub_unit_skill_list(ObjectId(P1));
    let list = decoded::<AcquireSkillList>(&s.take(), P1, "ACQUIRE_SKILL_LIST").remove(0);
    let mut ids: Vec<(i32, i32)> = list.skills.iter().map(|e| (e.id, e.next_level)).collect();
    ids.sort_unstable();
    assert_eq!(ids, [(611, 1), (611, 2), (612, 1)]);
}

#[test]
fn an_empty_squad_list_says_there_is_nothing() {
    let mut s = Scene::new();
    // Level 3: no squad skill line is reachable.
    s.load(vec![lions(3, 0)]);
    enter_all(&mut s);
    s.world.show_sub_unit_skill_list(ObjectId(P1));
    let out = s.take();
    assert_eq!(ids_to(&out, P1), [SystemMessageId::ThereAreNoOtherSkillsToLearn]);
}

// ---------------------------------------------------------------------------------- the info

#[test]
fn the_skill_info_lists_the_life_crystal_for_the_leader() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 1000)]);
    enter_all(&mut s);
    s.acquire_info(P1, 370, 1, AcquireSkillType::Pledge);
    let out = s.take();
    let info = &decoded::<AcquireSkillInfo>(&out, P1, "ACQUIRE_SKILL_INFO")[0];
    assert_eq!(
        (info.skill_type, info.skill_id, info.level, info.sp_cost),
        (AcquireSkillType::Pledge, 370, 1, 1500)
    );
    assert_eq!(info.requirements, [AcquireSkillRequirement::item(9816, 10)]);

    // Not the leader: nothing.
    s.acquire_info(P2, 370, 1, AcquireSkillType::Pledge);
    assert!(decoded::<AcquireSkillInfo>(&s.take(), P2, "ACQUIRE_SKILL_INFO").is_empty());
    // A line that is not in the clan tree: nothing.
    s.acquire_info(P1, 370, 3, AcquireSkillType::Pledge);
    assert!(decoded::<AcquireSkillInfo>(&s.take(), P1, "ACQUIRE_SKILL_INFO").is_empty());

    // Without `LifeCrystalNeeded` a clan skill lists no items.
    let mut s = Scene::with(ClanSettings {
        life_crystal_needed: false,
        ..ClanSettings::default()
    });
    s.load(vec![lions(8, 1000)]);
    enter_all(&mut s);
    s.acquire_info(P1, 370, 1, AcquireSkillType::Pledge);
    let info = &decoded::<AcquireSkillInfo>(&s.take(), P1, "ACQUIRE_SKILL_INFO")[0];
    assert!(info.requirements.is_empty());
}

#[test]
fn the_squad_skill_info_needs_the_leader_with_the_fame_privilege() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 1000)]);
    enter_all(&mut s);
    s.acquire_info(P1, 611, 1, AcquireSkillType::Subpledge);
    let info = &decoded::<AcquireSkillInfo>(&s.take(), P1, "ACQUIRE_SKILL_INFO")[0];
    assert_eq!(
        (info.skill_type, info.sp_cost, info.requirements.clone()),
        (
            AcquireSkillType::Subpledge,
            12000,
            vec![AcquireSkillRequirement::item(9910, 8)]
        )
    );
    s.acquire_info(P2, 611, 1, AcquireSkillType::Subpledge);
    assert!(decoded::<AcquireSkillInfo>(&s.take(), P2, "ACQUIRE_SKILL_INFO").is_empty());
}

// ---------------------------------------------------------------------------------- learning

#[test]
fn the_leader_buys_a_clan_skill_with_reputation_and_the_life_crystal() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 9000)]);
    let mut leader = member_data(P1, CLAN, 0, 0);
    leader.items = vec![item(0x6000_1001, 9816, 25)];
    s.enter(leader);
    s.enter(member_data(P2, CLAN, 0, 0));
    s.take();

    s.acquire(P1, 370, 1, AcquireSkillType::Pledge, 0);
    let out = s.take();
    let sent = ids_to(&out, P1);
    // The item goes with its message, the reputation is deducted, every member learns the skill,
    // then `AcquireSkillDone` and the list again.
    assert_eq!(
        sent,
        [
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
            SystemMessageId::TheClanSkillS1HasBeenAdded
        ]
    );
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S2S1HasDisappeared,
        &[
            SystemMessageParam::ItemName(9816),
            SystemMessageParam::LongNumber(10)
        ]
    ));
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
        &[SystemMessageParam::IntNumber(1500)]
    ));
    assert!(has_message(
        &out,
        P2,
        SystemMessageId::TheClanSkillS1HasBeenAdded,
        &[SystemMessageParam::SkillName { id: 370, level: 1 }]
    ));
    for id in [P1, P2] {
        let add = &decoded::<PledgeSkillListAdd>(&out, id, "PLEDGE_SKILL_LIST_ADD")[0];
        assert_eq!((add.id, add.level), (370, 1));
        let all = &decoded::<PledgeSkillList>(&out, id, "PLEDGE_SKILL_LIST")[0];
        assert_eq!(all.skills.len(), 1);
    }
    let names = names_to(&out, P1);
    let at = |name: &str| names.iter().position(|n| n == name).unwrap();
    assert!(at("PLEDGE_SKILL_LIST") < at("ACQUIRE_SKILL_DONE"));
    assert!(at("ACQUIRE_SKILL_DONE") < at("ACQUIRE_SKILL_LIST"));
    // The next list: level 2 of Clan Body is there now.
    let list = &decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST")[0];
    assert_eq!(
        level_one_ids(list),
        [(372, 1, 12000, 6), (370, 2, 6900, 2), (371, 1, 2100, 5)]
    );

    // The state: 10 of 25 life stones gone, 1500 reputation, the skill in the clan and on the
    // members (a main member of class 5 reaches an Apprentice skill).
    assert_eq!(s.clan().reputation_score, 7500);
    assert_eq!(s.clan().all_skills(), [(370, 1)]);
    assert_eq!(s.skills_of(P1), [(370, 1)]);
    assert_eq!(s.skills_of(P2), [(370, 1)]);
    let stones = s
        .world
        .player(ObjectId(P1))
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == 9816)
        .unwrap()
        .count;
    assert_eq!(stones, 15);
    // The row: a new skill is an INSERT with the clan skill's `sub_pledge_id` -2.
    let writes = clan_writes(&s.outs.take());
    assert!(writes.contains(&ClanWrite::InsertSkill {
        clan_id: CLAN,
        skill_id: 370,
        skill_level: 1,
        skill_name: "Clan Body".to_owned(),
        sub_pledge_id: -2,
    }));

    // Level 2: the leader knows level 1 as a skill of the clan (the previous-level check) and the
    // row is updated.
    s.acquire(P1, 370, 2, AcquireSkillType::Pledge, 0);
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
        &[SystemMessageParam::IntNumber(6900)]
    ));
    assert_eq!(s.clan().reputation_score, 600);
    assert_eq!(s.clan().all_skills(), [(370, 2)]);
    assert_eq!(s.skills_of(P1), [(370, 2)]);
    assert_eq!(
        clan_writes(&s.outs.take())
            .into_iter()
            .filter(|w| matches!(w, ClanWrite::UpdateSkill { .. }))
            .collect::<Vec<_>>(),
        [ClanWrite::UpdateSkill {
            clan_id: CLAN,
            skill_id: 370,
            skill_level: 2
        }]
    );
    assert!(!s.is_disabled(P1, 370, 2));
}

#[test]
fn a_clan_skill_without_enough_reputation_or_items_is_refused() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 1499)]);
    let mut leader = member_data(P1, CLAN, 0, 0);
    leader.items = vec![item(0x6000_1001, 9816, 10)];
    s.enter(leader);
    s.enter(member_data(P2, CLAN, 0, 0));
    s.take();

    // 1499 < 1500: refused, the list is shown again, nothing is taken.
    s.acquire(P1, 370, 1, AcquireSkillType::Pledge, 0);
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::TheAttemptToAcquireTheSkillHasFailedBecauseOfAnInsufficientClanReputationScore]
    );
    assert_eq!(
        decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST").len(),
        1
    );
    assert_eq!(s.clan().reputation_score, 1499);
    assert!(clan_writes(&s.outs.take()).is_empty());

    // Enough reputation but no life stones: the message and the list, the reputation stays.
    let mut s = Scene::new();
    s.load(vec![lions(8, 5000)]);
    s.enter(member_data(P1, CLAN, 0, 0));
    s.take();
    s.acquire(P1, 370, 1, AcquireSkillType::Pledge, 0);
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill]
    );
    assert_eq!(
        decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST").len(),
        1
    );
    assert_eq!(s.clan().reputation_score, 5000);
    assert!(s.clan().all_skills().is_empty());

    // Not the leader: nothing at all.
    s.enter(member_data(P2, CLAN, 0, 0));
    s.take();
    s.acquire(P2, 370, 1, AcquireSkillType::Pledge, 0);
    assert!(s.take().is_empty());
}

#[test]
fn without_life_crystal_needed_only_the_reputation_is_paid() {
    let mut s = Scene::with(ClanSettings {
        life_crystal_needed: false,
        ..ClanSettings::default()
    });
    s.load(vec![lions(8, 5000)]);
    s.enter(member_data(P1, CLAN, 0, 0));
    s.take();
    s.acquire(P1, 370, 1, AcquireSkillType::Pledge, 0);
    let out = s.take();
    assert!(!ids_to(&out, P1).contains(&SystemMessageId::S2S1HasDisappeared));
    assert_eq!(s.clan().reputation_score, 3500);
    assert_eq!(s.clan().all_skills(), [(370, 1)]);
}

#[test]
fn a_squad_skill_goes_to_the_members_of_that_squad() {
    let mut s = Scene::new();
    let mut record = lions(9, 40_000);
    record.data.as_mut().unwrap().castle_id = 1;
    s.load(vec![record]);
    let mut leader = member_data(P1, CLAN, 0, 0);
    leader.items = vec![item(0x6000_1001, 9910, 30)];
    s.enter(leader);
    s.enter(member_data(P2, CLAN, 0, 0));
    s.enter(member_data(P3, CLAN, 5, 100));
    s.take();

    // Fire Squad 1 for the royal guard (100): 8 stones and 12000 reputation.
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, 100);
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S2S1HasDisappeared,
        &[
            SystemMessageParam::ItemName(9910),
            SystemMessageParam::LongNumber(8)
        ]
    ));
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
        &[SystemMessageParam::IntNumber(12000)]
    ));
    // Only the members of the royal guard learn it, with `ExSubPledgeSkillAdd`.
    let add = &decoded::<ExSubPledgeSkillAdd>(&out, P3, "EX_SUB_PLEDGE_SKILL_ADD")[0];
    assert_eq!((add.sub_type, add.skill_id, add.skill_level), (100, 611, 1));
    assert!(has_message(
        &out,
        P3,
        SystemMessageId::TheClanSkillS1HasBeenAdded,
        &[SystemMessageParam::SkillName { id: 611, level: 1 }]
    ));
    assert!(decoded::<ExSubPledgeSkillAdd>(&out, P2, "EX_SUB_PLEDGE_SKILL_ADD").is_empty());
    assert_eq!(s.skills_of(P3), [(611, 1)]);
    assert!(s.skills_of(P2).is_empty());
    assert_eq!(s.clan().all_sub_skills(), [(100, 611, 1)]);
    assert_eq!(s.clan().reputation_score, 28_000);
    // `PledgeSkillList` to everybody, `AcquireSkillDone`, the squad list again.
    let all = &decoded::<PledgeSkillList>(&out, P2, "PLEDGE_SKILL_LIST")[0];
    assert_eq!(
        (all.skills.len(), all.sub_skills.len(), all.sub_skills[0].sub_type),
        (0, 1, 100)
    );
    assert!(names_to(&out, P1).contains(&"ACQUIRE_SKILL_DONE".to_owned()));
    let list = &decoded::<AcquireSkillList>(&out, P1, "ACQUIRE_SKILL_LIST")[0];
    assert_eq!(list.skill_type, AcquireSkillType::Subpledge);
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes
            .iter()
            .filter(|w| matches!(w, ClanWrite::InsertSkill { .. }))
            .collect::<Vec<_>>(),
        [&ClanWrite::InsertSkill {
            clan_id: CLAN,
            skill_id: 611,
            skill_level: 1,
            skill_name: "Fire Squad".to_owned(),
            sub_pledge_id: 100,
        }]
    );

    // The same level again: "already acquired"; the next level is taken.
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, 100);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::ThisSquadSkillHasAlreadyBeenAcquired]
    );
    s.acquire(P1, 611, 2, AcquireSkillType::Subpledge, 100);
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
        &[SystemMessageParam::IntNumber(17400)]
    ));
    assert_eq!(s.clan().all_sub_skills(), [(100, 611, 2)]);
    assert_eq!(s.skills_of(P3), [(611, 2)]);
    assert_eq!(s.clan().reputation_score, 10_600);
    // The row of a skill the squad had is an UPDATE.
    assert_eq!(
        clan_writes(&s.outs.take())
            .into_iter()
            .filter(|w| matches!(w, ClanWrite::UpdateSkill { .. }))
            .collect::<Vec<_>>(),
        [ClanWrite::UpdateSkill {
            clan_id: CLAN,
            skill_id: 611,
            skill_level: 2
        }]
    );
}

#[test]
fn a_squad_skill_needs_a_castle_a_real_squad_and_the_stones() {
    let mut s = Scene::new();
    // No castle or fortress: ignored.
    s.load(vec![lions(9, 20_000)]);
    let mut leader = member_data(P1, CLAN, 0, 0);
    leader.items = vec![item(0x6000_1001, 9910, 30)];
    s.enter(leader);
    s.take();
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, 100);
    assert!(s.take().is_empty());

    let mut s = Scene::new();
    let mut record = lions(9, 11_999);
    record.data.as_mut().unwrap().castle_id = 1;
    s.load(vec![record]);
    let mut leader = member_data(P1, CLAN, 0, 0);
    leader.items = vec![item(0x6000_1001, 9910, 3)];
    s.enter(leader);
    s.take();
    // The academy never takes a squad skill: "already acquired".
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, -1);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::ThisSquadSkillHasAlreadyBeenAcquired]
    );
    // A squad that does not exist: Java throws, nothing is sent.
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, 777);
    assert!(s.take().is_empty());
    // 11999 < 12000.
    s.acquire(P1, 611, 1, AcquireSkillType::Subpledge, 100);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheAttemptToAcquireTheSkillHasFailedBecauseOfAnInsufficientClanReputationScore]
    );
    // Enough reputation (a Water Squad costs 10400) but 3 stones are less than 4.
    s.acquire(P1, 612, 1, AcquireSkillType::Subpledge, 100);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill]
    );
    assert_eq!(s.clan().reputation_score, 11_999);
    assert!(s.clan().all_sub_skills().is_empty());
}

// ---------------------------------------------------------------------------------- the academy

#[test]
fn an_academy_member_graduates_with_the_second_class_transfer() {
    let mut s = Scene::new();
    s.load(vec![lions(8, 0)]);
    enter_all(&mut s);
    assert!(s.world.is_academy_member(ObjectId(P4)));

    // A class that is not of the 2nd transfer changes nothing (Duelist 88 is the 3rd).
    assert!(!s.world.academy_graduation(ObjectId(P4), 88));
    assert!(!s.world.academy_graduation(ObjectId(P4), 1));
    assert!(s.world.is_academy_member(ObjectId(P4)));
    // A member who is not in the academy does not graduate.
    assert!(!s.world.academy_graduation(ObjectId(P2), 2));
    assert!(s.take().is_empty());

    // Joined at level 20: 650 - (20 - 16) * 20 = 570 points.
    assert!(s.world.academy_graduation(ObjectId(P4), 2));
    let out = s.take();
    assert_eq!(s.clan().reputation_score, 570);
    for id in [P1, P4] {
        assert!(has_message(
            &out,
            id,
            SystemMessageId::ClanMemberS1HasBeenExpelled,
            &[SystemMessageParam::PlayerName("P4".to_owned())]
        ));
        assert_eq!(
            decoded::<PledgeShowMemberListDelete>(&out, id, "PLEDGE_SHOW_MEMBER_LIST_DELETE")[0].player_name,
            "P4"
        );
    }
    // The graduate left the clan without a penalty and is told.
    assert!(!s.clan().is_member(P4));
    assert!(!s.world.is_academy_member(ObjectId(P4)));
    assert_eq!(s.world.clan_id_of(ObjectId(P4)), 0);
    let state = s.world.player_clan_state(ObjectId(P4));
    assert_eq!(state.clan_join_expiry_time, 0);
    assert!(ids_to(&out, P4).contains(
        &SystemMessageId::CongratulationsYouWillNowGraduateFromTheClanAcademyAndLeaveYourCurrentClanAsAGraduateOfTheAcademyYouCanImmediatelyJoinAClanAsARegularMemberWithoutBeingSubjectToAnyPenalties
    ));
    assert!(names_to(&out, P4).contains(&"PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL".to_owned()));
    // The academy circlet.
    let circlets: Vec<i64> = s
        .world
        .player(ObjectId(P4))
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == 8181)
        .map(|i| i.count)
        .collect();
    assert_eq!(circlets, [1]);
    // A second try does nothing: he is gone from the academy.
    assert!(!s.world.academy_graduation(ObjectId(P4), 2));
}

#[test]
fn the_graduation_reward_follows_the_join_level_and_the_settings() {
    let mut s = Scene::with(ClanSettings {
        join_academy_min_rep_score: 100,
        join_academy_max_rep_score: 500,
        ..ClanSettings::default()
    });
    s.load(vec![lions(8, 0)]);
    s.enter(member_data(P1, CLAN, 0, 0));
    // Joined at level 39 or later: the minimum.
    let mut p4 = member_data(P4, CLAN, 9, -1);
    p4.profile.clan = p4.profile.clan.map(|c| PlayerClanState {
        lvl_joined_academy: 39,
        ..c
    });
    s.enter(p4);
    assert!(s.world.academy_graduation(ObjectId(P4), 127));
    assert_eq!(s.clan().reputation_score, 100);
}
