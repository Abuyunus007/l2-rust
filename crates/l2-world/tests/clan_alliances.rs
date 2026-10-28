#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Crests, alliances and clan wars (M6-CLAN-02): the crest table and the crest uploads, the
//! alliance (create, invite, leave, expel, dissolve, info), the clan war (declare, answer, stop,
//! surrender, list), the war reputation on a kill and the `ClanMaster` / `AllianceMaster` pages.
//! Java: data/sql/CrestTable.java, entity/clan/Clan.java (the alliance and crest parts),
//! data/sql/ClanTable.java (storeClanWars, deleteClanWars), network/clientpackets/{RequestSet*Crest,
//! RequestJoinAlly,RequestAnswerJoinAlly,AllyLeave,AllyDismiss,RequestDismissAlly,RequestAllyInfo,
//! Request*PledgeWar,RequestPledgeWarList}.java, entity/actor/Player.java (doDie),
//! dist/game/data/scripts/{handlers/bypass/npc/ScriptLink,village_master/*}.java.
//!
//! The messages, packets and rows are Java's; the numbers (limits, days, privileges) come from the
//! Java sources and the default `Player.ini`.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::pledge::{
    AllianceInfo, AllyCrest, AskJoinAlly, ExPledgeEmblem, PledgeCrest, PledgeReceiveWarList,
    PledgeShowMemberListAll,
};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, CrestRecord, CrestType, CrestWrite, WarWrite};
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

/// Clan skills 370 to 373 (displayed as themselves), two skills with clan conditions, and the
/// target handler skills 1 to 3.
const SKILLS: &str = r#"<list>
<skill id="370" levels="1" name="Clan Vitality"><operateType>P</operateType></skill>
<skill id="371" levels="1" name="Squad Skill"><operateType>P</operateType></skill>
<skill id="372" levels="1" name="Royal Skill"><operateType>P</operateType></skill>
<skill id="373" levels="1" name="Knight Skill"><operateType>P</operateType></skill>
<skill id="100" levels="1" name="Leader Only"><operateType>A1</operateType><targetType>SELF</targetType>
  <conditions msgId="113" addName="1"><player isClanLeader="true" /></conditions></skill>
<skill id="101" levels="1" name="Pledge Class 3"><operateType>A1</operateType><targetType>SELF</targetType>
  <conditions msgId="113" addName="1"><player pledgeClass="3" /></conditions></skill>
<skill id="102" levels="1" name="Castle 0"><operateType>A1</operateType><targetType>SELF</targetType>
  <conditions msgId="113" addName="1"><player castle="0" /></conditions></skill>
<skill id="1" levels="1" name="Clan Buff"><operateType>A1</operateType><targetType>CLAN</targetType>
  <affectRange>1000</affectRange><effectPoint>100</effectPoint></skill>
<skill id="2" levels="1" name="Party Clan Buff"><operateType>A1</operateType><targetType>PARTY_CLAN</targetType>
  <affectRange>1000</affectRange><effectPoint>100</effectPoint></skill>
<skill id="3" levels="1" name="Clan Rez"><operateType>A1</operateType><targetType>CORPSE_CLAN</targetType>
  <affectRange>1000</affectRange><effectPoint>100</effectPoint></skill>
<skill id="4" levels="1" name="Debuff"><operateType>A1</operateType><targetType>ONE</targetType>
  <isDebuff>true</isDebuff><effectPoint>-100</effectPoint></skill>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
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
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        matches!(item_id, 57 | 9910 | 9911)
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

/// The `sendMessage` texts sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    messages_to(out, to)
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn has_message(out: &Sent, to: i32, id: SystemMessageId, params: &[SystemMessageParam]) -> bool {
    messages_to(out, to)
        .iter()
        .any(|m| m.id == id && m.params == params)
}

fn text(s: &str) -> SystemMessageParam {
    SystemMessageParam::Text(s.to_owned())
}

fn clan_writes(outs: &[PersistCommand]) -> Vec<ClanWrite> {
    outs.iter()
        .filter_map(|o| match o {
            PersistCommand::Clan(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn now_ms() -> i64 {
    l2_util::time::now_millis()
}

const DAY_MS: i64 = 86_400_000;
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const P5: i32 = 0x6000_0005;
const P6: i32 = 0x6000_0006;
const CLAN: i32 = 500;
/// The village master of the tests.
const VM: i32 = 30026;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30026" type="VillageMasterFighter" level="70" name="Bitz" usingServerSideName="true">
    <status attackable="false" />
  </npc>
</list>"#;

/// The village master's page: every clan command as the `ClanMaster` script pages list them.
const VM_PAGE: &str = "<html><body>\
    <a action=\"bypass -h npc_%objectId%_create_clan $name\">create</a>\
    <a action=\"bypass -h npc_%objectId%_create_academy $name\">academy</a>\
    <a action=\"bypass -h npc_%objectId%_create_royal $name $leader\">royal</a>\
    <a action=\"bypass -h npc_%objectId%_create_knight $name $leader\">knight</a>\
    <a action=\"bypass -h npc_%objectId%_rename_pledge $type $name\">rename</a>\
    <a action=\"bypass -h npc_%objectId%_assign_subpl_leader $name $leader\">assign</a>\
    <a action=\"bypass -h npc_%objectId%_dissolve_clan\">dissolve</a>\
    <a action=\"bypass -h npc_%objectId%_recover_clan\">recover</a>\
    <a action=\"bypass -h npc_%objectId%_increase_clan_level\">level</a>\
    <a action=\"bypass -h npc_%objectId%_change_clan_leader $name\">leader</a>\
    <a action=\"bypass -h npc_%objectId%_cancel_clan_leader_change\">cancel</a>\
    <a action=\"bypass -h npc_%objectId%_create_ally $name\">ally</a>\
    <a action=\"bypass -h npc_%objectId%_dissolve_ally\">dissolve ally</a>\
    <a action=\"bypass -h Script ClanMaster\">Clan.</a>\
    <a action=\"bypass -h Script AllianceMaster\">Alliance.</a>\
    <a action=\"bypass Script\">Quest.</a>\
    </body></html>";

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    vm: Option<ObjectId>,
    /// The `crests` ids the database has (M7-FU-03): what the persistence task answers to a
    /// `CrestIdsRequest`.
    db_crest_ids: Vec<i32>,
    /// A test that answers the requests itself sets this.
    hold_crest_answers: bool,
    answered_tickets: Vec<u32>,
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
            "data/html/clanNotice.htm",
            "<html><body>%clan_name%: %notice_text%</body></html>",
        );
        for (script, page, html) in SCRIPT_PAGES {
            write(&format!("data/scripts/village_master/{script}/{page}"), html);
        }
        for page in ["9000-07-success", "9000-07-in-progress", "9000-07-canceled"] {
            write(
                &format!("data/scripts/village_master/ClanMaster/{page}.htm"),
                &format!("<html><body>{page}</body></html>"),
            );
        }
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(outs.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
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
            vm: None,
            db_crest_ids: Vec::new(),
            hold_crest_answers: false,
            answered_tickets: Vec::new(),
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
        if !self.hold_crest_answers {
            self.answer_crest_requests();
        }
    }

    /// The tickets of the `CrestIdsRequest`s in the log that were not answered yet.
    fn crest_requests(&self) -> Vec<u32> {
        let outs = self.outs.0.lock().unwrap();
        outs.iter()
            .filter_map(|o| match o {
                PersistCommand::CrestIdsRequest { ticket } if !self.answered_tickets.contains(ticket) => {
                    Some(*ticket)
                }
                _ => None,
            })
            .collect()
    }

    /// What the persistence task does for a `CrestIdsRequest`: it reads the ids of the table.
    fn answer_crest_requests(&mut self) {
        for ticket in self.crest_requests() {
            self.answered_tickets.push(ticket);
            let used = self.db_crest_ids.clone();
            self.world.handle(WorldCommand::CrestIdsLoaded {
                ticket,
                result: Ok(used),
            });
        }
    }

    /// The player talks to the village master (spawned on first use): its page is cached.
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

    fn take(&mut self) -> Sent {
        self.log.take()
    }

    fn clan(&self) -> &Clan {
        self.world.clan(CLAN).unwrap()
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

/// A character of clan `clan_id` as the loader gives it (`characters.clanid`, ...).
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

/// Clan 500 "Lions" of `level`, led by P1, with these members.
fn lions(level: i32, members: Vec<ClanMemberRecord>) -> ClanRecord {
    ClanRecord {
        clan_id: CLAN,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level,
            leader_id: P1,
            ..ClanDataRecord::default()
        }),
        members,
        ..ClanRecord::default()
    }
}

/// Lions with P1 (leader), P2 and P3, the given rank 5 privileges.
fn lions_of_three(level: i32, rank5: i32) -> ClanRecord {
    let mut record = lions(level, vec![member(P1, 1, 0), member(P2, 5, 0), member(P3, 5, 0)]);
    record.rank_privs = vec![(5, 0, rank5)];
    record
}

// ---------------------------------------------------------------------------------- fixtures

const TIGERS: i32 = 600;
const BEARS: i32 = 700;
const P7: i32 = 0x6000_0007;

/// A clan of `level` led by `leader`, with the members `(id, power grade)`.
fn clan_record(id: i32, name: &str, level: i32, leader: i32, members: &[(i32, i32)]) -> ClanRecord {
    ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: members.iter().map(|(m, grade)| member(*m, *grade, 0)).collect(),
        ..ClanRecord::default()
    }
}

/// Lions (500; P1 leads, P2 and P3) in the alliance "Pride" of its own, Tigers (600; P4 leads,
/// P5) and Bears (700; P6), all level 5.
fn three_clans() -> Vec<ClanRecord> {
    let mut lions = clan_record(CLAN, "Lions", 5, P1, &[(P1, 1), (P2, 5), (P3, 5)]);
    if let Some(d) = lions.data.as_mut() {
        d.ally_id = CLAN;
        d.ally_name = Some("Pride".to_owned());
    }
    vec![
        lions,
        clan_record(TIGERS, "Tigers", 5, P4, &[(P4, 1), (P5, 5)]),
        clan_record(BEARS, "Bears", 5, P6, &[(P6, 1)]),
    ]
}

fn data_of(records: &mut [ClanRecord], clan_id: i32) -> &mut ClanDataRecord {
    records
        .iter_mut()
        .find(|r| r.clan_id == clan_id)
        .and_then(|r| r.data.as_mut())
        .unwrap()
}

/// The three clans loaded, their members and a clanless P7 in the world. `tweak` changes the
/// records first.
fn clan_scene(settings: ClanSettings, tweak: impl FnOnce(&mut Vec<ClanRecord>)) -> Scene {
    let mut s = Scene::with(settings);
    let mut records = three_clans();
    tweak(&mut records);
    s.load(records);
    for (id, clan, grade) in [
        (P1, CLAN, 1),
        (P2, CLAN, 5),
        (P3, CLAN, 5),
        (P4, TIGERS, 1),
        (P5, TIGERS, 5),
        (P6, BEARS, 1),
    ] {
        s.enter(member_data(id, clan, grade, 0));
    }
    s.enter(pdata(P7, 40));
    s.take();
    s.outs.take();
    s
}

fn war_settings() -> ClanSettings {
    ClanSettings {
        alt_clan_members_for_war: 2,
        ..ClanSettings::default()
    }
}

/// The ids of the `SystemMessage`s sent to `to`, read from the bytes: a message whose id declares
/// a parameter that Java never added (`..._REFUSED_2`) does not decode.
fn raw_message_ids(out: &Sent, to: i32) -> Vec<i32> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SYSTEM_MESSAGE")
        .map(|(_, b)| i32::from_le_bytes([b[1], b[2], b[3], b[4]]))
        .collect()
}

fn messages_with_params(out: &Sent, to: i32) -> Vec<(SystemMessageId, Vec<SystemMessageParam>)> {
    messages_to(out, to)
        .into_iter()
        .map(|m| (m.id, m.params))
        .collect()
}

// ---------------------------------------------------------------------------------- crests

fn crest_record(crest_id: i32, data: &[u8], crest_type: i32) -> CrestRecord {
    CrestRecord {
        crest_id,
        data: data.to_vec(),
        crest_type,
    }
}

#[test]
fn crests_load_like_crest_table_and_clans_lose_the_ones_that_are_gone() {
    let records = || {
        let mut lions = lions(5, vec![member(P1, 1, 0)]);
        if let Some(d) = lions.data.as_mut() {
            d.crest_id = 5;
            d.crest_large_id = 6;
            d.ally_crest_id = 9;
        }
        vec![lions]
    };
    let crests = vec![
        crest_record(1, &[1], 1),
        crest_record(5, &[5], 1),
        // The newest crest is never deleted, used or not: its id is not given out again.
        crest_record(8, &[8], 3),
    ];

    // With the switch: the unused crest 1 is deleted, the clan columns of the missing crests are
    // cleared.
    let mut s = Scene::with(ClanSettings {
        maintenance_database_writes: true,
        ..ClanSettings::default()
    });
    s.load(records());
    s.outs.take();
    s.world.load_crests(crests.clone());
    assert!(s.world.crest(1).is_none());
    assert_eq!(s.world.crest(5).map(|c| c.data.clone()), Some(vec![5]));
    assert_eq!(s.world.crest(8).map(|c| c.kind), Some(CrestType::Ally));
    assert_eq!(
        (s.world.crest_table().len(), s.world.crest_table().next_id()),
        (2, 9)
    );
    assert_eq!(
        (s.clan().crest_id, s.clan().crest_large_id, s.clan().ally_crest_id),
        (5, 0, 0)
    );
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Delete { crest_id: 1 }),
            ClanWrite::Crest(CrestWrite::ClanLargeCrest {
                clan_id: CLAN,
                crest_id: 0
            }),
            ClanWrite::Crest(CrestWrite::AllyCrestOfClan {
                clan_id: CLAN,
                crest_id: 0
            }),
        ]
    );

    // Without it (D-49) the same state, and the rows are left to the Java game server.
    let mut s = Scene::new();
    s.load(records());
    s.world.load_crests(crests);
    assert!(s.world.crest(1).is_none());
    assert_eq!((s.clan().crest_large_id, s.clan().ally_crest_id), (0, 0));
    assert!(clan_writes(&s.outs.take()).is_empty());
}

#[test]
fn a_crest_of_an_unknown_type_is_not_cached() {
    let mut s = Scene::new();
    let mut lions = lions(5, vec![member(P1, 1, 0)]);
    if let Some(d) = lions.data.as_mut() {
        d.crest_large_id = 2;
    }
    s.load(vec![lions]);
    s.world
        .load_crests(vec![crest_record(4, &[4], 9), crest_record(2, &[2], 2)]);
    // 4 is the newest, so it stays in the table's numbering, but Java logs the type and drops it.
    assert!(s.world.crest(4).is_none());
    assert_eq!(s.world.crest(2).map(|c| c.kind), Some(CrestType::PledgeLarge));
    assert_eq!(s.world.crest_table().next_id(), 5);
}

#[test]
fn crest_requests_answer_with_the_cached_image() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.world.load_crests(vec![crest_record(3, &[1, 2, 3], 1)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.clan_input(P1, ClanInput::PledgeCrest { crest_id: 3 });
    s.clan_input(P1, ClanInput::PledgeCrest { crest_id: 99 });
    s.clan_input(P1, ClanInput::ExPledgeCrestLarge { crest_id: 3 });
    s.clan_input(P1, ClanInput::AllyCrest { crest_id: 3 });
    let out = s.take();
    // Java does not look at the type; an unknown id is an empty image.
    let pledge = decoded::<PledgeCrest>(&out, P1, "PLEDGE_CREST");
    assert_eq!(
        pledge,
        [
            PledgeCrest {
                crest_id: 3,
                data: vec![1, 2, 3]
            },
            PledgeCrest {
                crest_id: 99,
                data: vec![]
            }
        ]
    );
    assert_eq!(
        decoded::<ExPledgeEmblem>(&out, P1, "EX_PLEDGE_EMBLEM"),
        [ExPledgeEmblem {
            crest_id: 3,
            data: vec![1, 2, 3]
        }]
    );
    assert_eq!(
        decoded::<AllyCrest>(&out, P1, "ALLIANCE_CREST"),
        [AllyCrest {
            crest_id: 3,
            data: vec![1, 2, 3]
        }]
    );
}

#[test]
fn a_clan_crest_is_registered_replaced_and_deleted() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    let upload = |s: &mut Scene, who: i32, data: &[u8]| {
        s.clan_input(
            who,
            ClanInput::SetPledgeCrest {
                length: i32::try_from(data.len()).unwrap(),
                data: Some(data.to_vec()),
            },
        );
    };

    // Rank 5 has no privilege to change the crest.
    upload(&mut s, P2, &[1, 2, 3, 4]);
    let out = s.take();
    assert_eq!(ids_to(&out, P2), [SystemMessageId::YouAreNotAuthorizedToDoThat]);
    assert!(clan_writes(&s.outs.take()).is_empty());

    upload(&mut s, P1, &[1, 2, 3, 4]);
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::TheCrestWasSuccessfullyRegistered]
    );
    assert_eq!(s.clan().crest_id, 1);
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Insert {
                crest_id: 1,
                data: vec![1, 2, 3, 4],
                crest_type: 1
            }),
            ClanWrite::Crest(CrestWrite::ClanCrest {
                clan_id: CLAN,
                crest_id: 1
            }),
        ]
    );
    // `member.broadcastUserInfo()` for every online member.
    for id in [P1, P2] {
        assert!(names_to(&out, id).contains(&"USER_INFO".to_owned()), "{id}");
    }
    assert_eq!(s.world.clan_crest_id_of(ObjectId(P2)), 1);

    // A new crest replaces it: the old one goes (it is not the newest id).
    upload(&mut s, P1, &[9, 9]);
    assert_eq!(s.clan().crest_id, 2);
    assert!(s.world.crest(1).is_none() && s.world.crest(2).is_some());
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Insert {
                crest_id: 2,
                data: vec![9, 9],
                crest_type: 1
            }),
            ClanWrite::Crest(CrestWrite::Delete { crest_id: 1 }),
            ClanWrite::Crest(CrestWrite::ClanCrest {
                clan_id: CLAN,
                crest_id: 2
            }),
        ]
    );

    // Length 0 deletes it; the newest crest's row stays so its id is not reused.
    s.take();
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 0,
            data: Some(vec![]),
        },
    );
    let out = s.take();
    assert_eq!(ids_to(&out, P1), [SystemMessageId::TheClanSCrestHasBeenDeleted]);
    assert_eq!(s.clan().crest_id, 0);
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::Crest(CrestWrite::ClanCrest {
            clan_id: CLAN,
            crest_id: 0
        })]
    );
    // Nothing to delete: no message.
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 0,
            data: Some(vec![]),
        },
    );
    assert!(ids_to(&s.take(), P1).is_empty());
}

#[test]
fn a_crest_upload_is_checked_like_request_set_pledge_crest() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(2, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    // Above 256 bytes the packet carries no data; below 0 is the size message as well.
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 300,
            data: None,
        },
    );
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: -1,
            data: None,
        },
    );
    // Level 2.
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 1,
            data: Some(vec![1]),
        },
    );
    assert_eq!(
        ids_to(&s.take(), P1),
        [
            SystemMessageId::TheSizeOfTheImageFileIsInappropriatePleaseAdjustTo16x12Pixels,
            SystemMessageId::TheSizeOfTheUploadedCrestOrInsigniaDoesNotMeetTheStandardRequirements,
            SystemMessageId::AClanCrestCanOnlyBeRegisteredWhenTheClanSSkillLevelIs3OrAbove,
        ]
    );
    assert!(s.world.crest_table().is_empty());
    assert!(clan_writes(&s.outs.take()).is_empty());

    // A clan whose dissolution was asked for cannot change its crest.
    let mut dissolving = lions_of_three(3, 0);
    if let Some(d) = dissolving.data.as_mut() {
        d.dissolving_expiry_time = now_ms() + 3 * DAY_MS;
    }
    let mut s = Scene::new();
    s.load(vec![dissolving]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 1,
            data: Some(vec![1]),
        },
    );
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionYouCannotRegisterOrDeleteAClanCrest]
    );
}

/// M7-FU-03: a new crest takes the lowest free id, not `max + 1`.
#[test]
fn a_new_crest_takes_the_lowest_free_id() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    // The other server wrote 1, 2 and 4; 3 is free.
    s.db_crest_ids = vec![4, 1, 2];
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 2,
            data: Some(vec![8, 8]),
        },
    );
    assert_eq!(s.clan().crest_id, 3);
    assert!(
        clan_writes(&s.outs.take()).contains(&ClanWrite::Crest(CrestWrite::Insert {
            crest_id: 3,
            data: vec![8, 8],
            crest_type: 1
        }))
    );
    // `_nextId` is still above the highest id the world gave out.
    assert_eq!(s.world.crest_table().next_id(), 4);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheCrestWasSuccessfullyRegistered]
    );
}

/// M7-FU-03: without a gap the id is `max + 1`.
#[test]
fn a_new_crest_without_a_gap_takes_max_plus_one() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.db_crest_ids = vec![1, 2, 3];
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 1,
            data: Some(vec![1]),
        },
    );
    assert_eq!(s.clan().crest_id, 4);
}

/// M7-FU-03: two uploads asked before the first answer (the answer of the second does not list
/// the first insert) still get two ids.
#[test]
fn two_crests_asked_together_do_not_collide() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.hold_crest_answers = true;
    for data in [vec![1], vec![2]] {
        s.clan_input(
            P1,
            ClanInput::SetPledgeCrest {
                length: 1,
                data: Some(data),
            },
        );
    }
    let tickets = s.crest_requests();
    assert_eq!(tickets.len(), 2);
    for ticket in tickets {
        s.world.handle(WorldCommand::CrestIdsLoaded {
            ticket,
            result: Ok(Vec::new()),
        });
    }
    // 1, then 2 (the first crest is replaced by the second and deleted).
    assert_eq!(s.clan().crest_id, 2);
    let inserts: Vec<i32> = clan_writes(&s.outs.take())
        .into_iter()
        .filter_map(|w| match w {
            ClanWrite::Crest(CrestWrite::Insert { crest_id, .. }) => Some(crest_id),
            _ => None,
        })
        .collect();
    assert_eq!(inserts, [1, 2]);
}

/// M7-FU-03: an insert that fails is retried once under a fresh read of the ids.
#[test]
fn a_failed_crest_insert_is_retried_once_with_a_fresh_read() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 1,
            data: Some(vec![5]),
        },
    );
    assert_eq!(s.clan().crest_id, 1);
    s.outs.take();
    // The other server took id 1 meanwhile: the insert failed.
    s.hold_crest_answers = true;
    s.world.handle(WorldCommand::CrestInsertFailed { crest_id: 1 });
    let tickets = s.crest_requests();
    assert_eq!(tickets.len(), 1);
    s.world.handle(WorldCommand::CrestIdsLoaded {
        ticket: tickets[0],
        result: Ok(vec![1]),
    });
    assert_eq!(s.clan().crest_id, 2);
    assert!(s.world.crest(1).is_none());
    assert_eq!(s.world.crest(2).map(|c| c.data.clone()), Some(vec![5]));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Insert {
                crest_id: 2,
                data: vec![5],
                crest_type: 1
            }),
            ClanWrite::Crest(CrestWrite::ClanCrest {
                clan_id: CLAN,
                crest_id: 2
            }),
        ]
    );
    // A second failure of the new id is not retried again.
    s.world.handle(WorldCommand::CrestInsertFailed { crest_id: 2 });
    assert!(s.crest_requests().is_empty());
}

/// M7-FU-03: a failed read creates nothing.
#[test]
fn a_failed_crest_id_read_creates_no_crest() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.hold_crest_answers = true;
    s.clan_input(
        P1,
        ClanInput::SetPledgeCrest {
            length: 1,
            data: Some(vec![5]),
        },
    );
    let tickets = s.crest_requests();
    s.world.handle(WorldCommand::CrestIdsLoaded {
        ticket: tickets[0],
        result: Err(()),
    });
    assert_eq!(s.clan().crest_id, 0);
    assert!(s.world.crest_table().is_empty());
    assert!(ids_to(&s.take(), P1).is_empty());
}

#[test]
fn a_large_crest_follows_request_ex_set_pledge_crest_large() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(3, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    // Above the limit the packet has no data, and Java stops without a word.
    s.clan_input(
        P1,
        ClanInput::ExSetPledgeCrestLarge {
            length: 3000,
            data: None,
        },
    );
    assert!(s.take().is_empty());

    s.clan_input(
        P2,
        ClanInput::ExSetPledgeCrestLarge {
            length: 2,
            data: Some(vec![7, 7]),
        },
    );
    assert_eq!(
        ids_to(&s.take(), P2),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );

    s.clan_input(
        P1,
        ClanInput::ExSetPledgeCrestLarge {
            length: 2,
            data: Some(vec![7, 7]),
        },
    );
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::TheClanCrestWasSuccessfullyRegisteredRememberOnlyAClanThatOwnsAClanHallOrCastleCanDisplayACrest]
    );
    assert_eq!(s.clan().crest_large_id, 1);
    assert_eq!(s.world.crest(1).map(|c| c.kind), Some(CrestType::PledgeLarge));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Insert {
                crest_id: 1,
                data: vec![7, 7],
                crest_type: 2
            }),
            ClanWrite::Crest(CrestWrite::ClanLargeCrest {
                clan_id: CLAN,
                crest_id: 1
            }),
        ]
    );
    // Only a clan with a castle or a clan hall shows it.
    assert_eq!(s.world.clan_crest_large_id_of(ObjectId(P1)), 0);

    s.clan_input(
        P1,
        ClanInput::ExSetPledgeCrestLarge {
            length: 0,
            data: Some(vec![]),
        },
    );
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheClanSCrestHasBeenDeleted]
    );
    assert_eq!(s.clan().crest_large_id, 0);
}

#[test]
fn an_alliance_crest_goes_to_every_clan_of_the_alliance() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        data_of(records, TIGERS).ally_id = CLAN;
        data_of(records, TIGERS).ally_name = Some("Pride".to_owned());
    });
    let upload = |s: &mut Scene, who: i32, data: &[u8]| {
        s.clan_input(
            who,
            ClanInput::SetAllyCrest {
                length: i32::try_from(data.len()).unwrap(),
                data: Some(data.to_vec()),
            },
        );
    };
    // The leader of a clan that is only a member, a member of the leading clan and a clan
    // outside the alliance are not alliance leaders.
    upload(&mut s, P4, &[1]);
    upload(&mut s, P2, &[1]);
    upload(&mut s, P6, &[1]);
    let out = s.take();
    for who in [P4, P2, P6] {
        assert_eq!(
            ids_to(&out, who),
            [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders],
            "{who}"
        );
    }
    s.clan_input(
        P1,
        ClanInput::SetAllyCrest {
            length: 193,
            data: None,
        },
    );
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::PleaseAdjustTheImageSizeTo8x12]
    );
    // `sendMessage("File transfer error.")`
    s.clan_input(
        P1,
        ClanInput::SetAllyCrest {
            length: -1,
            data: None,
        },
    );
    assert_eq!(texts_to(&s.take(), P1), ["File transfer error."]);

    upload(&mut s, P1, &[4, 5, 6]);
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::TheCrestWasSuccessfullyRegistered]
    );
    assert_eq!(s.world.crest(1).map(|c| c.kind), Some(CrestType::Ally));
    assert_eq!(s.world.clan(CLAN).map(|c| c.ally_crest_id), Some(1));
    assert_eq!(s.world.clan(TIGERS).map(|c| c.ally_crest_id), Some(1));
    assert_eq!(s.world.ally_crest_id_of(ObjectId(P5)), 1);
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::Crest(CrestWrite::Insert {
                crest_id: 1,
                data: vec![4, 5, 6],
                crest_type: 3
            }),
            ClanWrite::Crest(CrestWrite::AllyCrestOfAlly {
                ally_id: CLAN,
                crest_id: 1
            }),
        ]
    );
    // Both clans' online members are sent their `UserInfo`.
    for who in [P1, P2, P3, P4, P5] {
        assert!(names_to(&out, who).contains(&"USER_INFO".to_owned()), "{who}");
    }
    assert!(!names_to(&out, P6).contains(&"USER_INFO".to_owned()));

    // Length 0 takes it away from both; the newest crest's row stays.
    s.clan_input(
        P1,
        ClanInput::SetAllyCrest {
            length: 0,
            data: Some(vec![]),
        },
    );
    assert_eq!(s.world.clan(TIGERS).map(|c| c.ally_crest_id), Some(0));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::Crest(CrestWrite::AllyCrestOfAlly {
            ally_id: CLAN,
            crest_id: 0
        })]
    );
}

// ---------------------------------------------------------------------------------- alliances

#[test]
fn an_alliance_is_created_at_the_village_master() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        // Bears are level 4, Tigers dissolve, nobody else has an alliance.
        data_of(records, BEARS).level = 4;
        data_of(records, TIGERS).dissolving_expiry_time = now_ms() + 3 * DAY_MS;
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
    });
    let try_create = |s: &mut Scene, who: i32, name: &str| {
        s.vm_command(who, &format!("create_ally {name}"));
        ids_to(&s.take(), who)
    };
    assert_eq!(
        try_create(&mut s, P7, "Pride"),
        [SystemMessageId::OnlyClanLeadersMayCreateAlliances]
    );
    assert_eq!(
        try_create(&mut s, P2, "Pride"),
        [SystemMessageId::OnlyClanLeadersMayCreateAlliances]
    );
    assert_eq!(
        try_create(&mut s, P6, "Pride"),
        [SystemMessageId::ToCreateAnAllianceYourClanMustBeLevel5OrHigher]
    );
    assert_eq!(
        try_create(&mut s, P4, "Pride"),
        [SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionNoAllianceCanBeCreated]
    );
    assert_eq!(
        try_create(&mut s, P1, "Pr!de"),
        [SystemMessageId::IncorrectAllianceNamePleaseTryAgain]
    );
    assert_eq!(
        try_create(&mut s, P1, "P"),
        [SystemMessageId::IncorrectLengthForAnAllianceName]
    );
    assert_eq!(
        try_create(&mut s, P1, "ABCDEFGHIJKLMNOPQ"),
        [SystemMessageId::IncorrectLengthForAnAllianceName]
    );
    assert!(clan_writes(&s.outs.take()).is_empty());

    s.vm_command(P1, "create_ally Pride");
    let out = s.take();
    assert_eq!(
        (s.clan().ally_id, s.clan().ally_name.as_deref()),
        (CLAN, Some("Pride"))
    );
    assert_eq!(texts_to(&out, P1), ["Alliance Pride has been created."]);
    assert!(names_to(&out, P1).contains(&"USER_INFO".to_owned()));
    let writes = clan_writes(&s.outs.take());
    match writes.as_slice() {
        [ClanWrite::Update(row)] => {
            assert_eq!(
                (row.clan_id, row.ally_id, row.ally_name.as_deref()),
                (CLAN, CLAN, Some("Pride"))
            );
            assert_eq!((row.ally_penalty_expiry_time, row.ally_penalty_type), (0, 0));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(s.world.ally_id_of(ObjectId(P2)), CLAN);

    // A clan in an alliance cannot make a second one.
    assert_eq!(
        try_create(&mut s, P1, "other"),
        [SystemMessageId::YouAlreadyBelongToAnotherAlliance]
    );
}

#[test]
fn an_alliance_name_is_unique_and_a_dissolved_alliance_blocks_a_new_one() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_penalty_expiry_time = now_ms() + DAY_MS;
        tigers.ally_penalty_type = 4;
    });
    s.vm_command(P6, "create_ally PRIDE");
    assert_eq!(
        ids_to(&s.take(), P6),
        [SystemMessageId::ThatAllianceNameAlreadyExists]
    );
    s.vm_command(P4, "create_ally Fresh");
    assert_eq!(
        ids_to(&s.take(), P4),
        [SystemMessageId::YouCannotCreateANewAllianceWithin1DayOfDissolution]
    );
}

/// P1 (leader of the alliance "Pride") invites `target`; the messages P1 gets.
fn invite(s: &mut Scene, who: i32, target: i32) -> Vec<SystemMessageId> {
    s.take();
    s.clan_input(who, ClanInput::JoinAlly { id: target });
    ids_to(&s.take(), who)
}

#[test]
fn the_ally_join_conditions_follow_check_ally_join_condition() {
    let tweak_none = |_: &mut Vec<ClanRecord>| {};
    let mut s = clan_scene(ClanSettings::default(), tweak_none);
    assert_eq!(
        invite(&mut s, P1, 0x6000_0063),
        [SystemMessageId::YouHaveInvitedTheWrongTarget]
    );
    assert_eq!(
        invite(&mut s, P7, P4),
        [SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction]
    );
    // Not the leader of the alliance's leading clan.
    assert_eq!(
        invite(&mut s, P2, P4),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    assert_eq!(
        invite(&mut s, P4, P6),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    assert_eq!(
        invite(&mut s, P1, P1),
        [SystemMessageId::YouCannotAskYourselfToApplyToAClan]
    );
    assert_eq!(
        invite(&mut s, P1, P7),
        [SystemMessageId::TheTargetMustBeAClanMember]
    );
    s.take();
    s.clan_input(P1, ClanInput::JoinAlly { id: P5 });
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1IsNotAClanLeader,
        &[text("P5")]
    ));

    // Tigers belong to an alliance already.
    let mut s = clan_scene(ClanSettings::default(), |records| {
        // (An alliance whose leading clan is gone is dropped at the load.)
        let tigers = data_of(records, TIGERS);
        tigers.ally_id = BEARS;
        tigers.ally_name = Some("Other".to_owned());
    });
    s.take();
    s.clan_input(P1, ClanInput::JoinAlly { id: P4 });
    assert!(has_message(
        &s.take(),
        P1,
        SystemMessageId::S1ClanIsAlreadyAMemberOfS2Alliance,
        &[text("Tigers"), text("Other")]
    ));

    // Penalties: left an alliance, was expelled, or the inviter expelled a clan.
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_penalty_expiry_time = now_ms() + DAY_MS;
        tigers.ally_penalty_type = 1;
    });
    s.take();
    s.clan_input(P1, ClanInput::JoinAlly { id: P4 });
    // `getAllyName()` of a clan without an alliance is null: an empty string.
    assert!(has_message(
        &s.take(),
        P1,
        SystemMessageId::S1ClanCannotJoinTheAllianceBecauseOneDayHasNotYetPassedSinceTheyLeftAnotherAlliance,
        &[text("Tigers"), text("")]
    ));
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_penalty_expiry_time = now_ms() + DAY_MS;
        tigers.ally_penalty_type = 2;
    });
    assert_eq!(
        invite(&mut s, P1, P4),
        [SystemMessageId::AClanThatHasWithdrawnOrBeenExpelledCannotEnterIntoAnAllianceWithinOneDayOfWithdrawalOrExpulsion]
    );
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let lions = data_of(records, CLAN);
        lions.ally_penalty_expiry_time = now_ms() + DAY_MS;
        lions.ally_penalty_type = 3;
    });
    assert_eq!(
        invite(&mut s, P1, P4),
        [SystemMessageId::YouMayNotAcceptAnyClanWithinADayAfterExpellingAnotherClan]
    );
    // The same penalties after they ran out do not matter.
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let lions = data_of(records, CLAN);
        lions.ally_penalty_expiry_time = now_ms() - 1;
        lions.ally_penalty_type = 3;
    });
    assert!(invite(&mut s, P1, P4).is_empty());

    // At war with the invited clan.
    let mut s = clan_scene(ClanSettings::default(), tweak_none);
    s.world.store_clan_wars(CLAN, TIGERS);
    assert_eq!(
        invite(&mut s, P1, P4),
        [SystemMessageId::YouMayNotAllyWithAClanYouAreCurrentlyAtWarWithThatWouldBeDiabolicalAndTreacherous]
    );

    // `AltMaxNumOfClansInAlly`
    let mut s = clan_scene(
        ClanSettings {
            alt_max_num_of_clans_in_ally: 1,
            ..ClanSettings::default()
        },
        tweak_none,
    );
    assert_eq!(invite(&mut s, P1, P4), [SystemMessageId::YouHaveExceededTheLimit]);
}

#[test]
fn an_invitation_answered_yes_joins_the_alliance() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        data_of(records, CLAN).ally_crest_id = 3;
    });
    s.clan_input(P1, ClanInput::JoinAlly { id: P4 });
    let out = s.take();
    assert!(has_message(
        &out,
        P4,
        SystemMessageId::S1LeaderS2HasRequestedAnAlliance,
        &[text("Pride"), text("P1")]
    ));
    assert_eq!(
        decoded::<AskJoinAlly>(&out, P4, "ASK_JOIN_ALLIANCE"),
        [AskJoinAlly {
            requestor_object_id: P1,
            requestor_name: "Pride".to_owned()
        }]
    );
    assert!(s.world.is_request_partner_busy(ObjectId(P1)) && s.world.is_request_partner_busy(ObjectId(P4)));

    s.clan_input(P4, ClanInput::AnswerJoinAlly { response: 1 });
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::ThatPersonHasBeenSuccessfullyAddedToYourFriendList]
    );
    assert_eq!(ids_to(&out, P4), [SystemMessageId::YouHaveAcceptedTheAlliance]);
    let tigers = s.world.clan(TIGERS).unwrap();
    assert_eq!(
        (
            tigers.ally_id,
            tigers.ally_name.as_deref(),
            tigers.ally_crest_id,
            tigers.ally_penalty_expiry_time
        ),
        (CLAN, Some("Pride"), 3, 0)
    );
    assert_eq!(s.world.clan_allies(CLAN).len(), 2);
    // The new clan's members are sent their `UserInfo`.
    for who in [P4, P5] {
        assert!(names_to(&out, who).contains(&"USER_INFO".to_owned()), "{who}");
    }
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes[0],
        ClanWrite::Crest(CrestWrite::AllyCrestOfClan {
            clan_id: TIGERS,
            crest_id: 3
        })
    );
    match &writes[1] {
        ClanWrite::Update(row) => assert_eq!(
            (row.clan_id, row.ally_id, row.ally_name.as_deref()),
            (TIGERS, CLAN, Some("Pride"))
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(writes.len(), 2);
    assert!(!s.world.is_request_partner_busy(ObjectId(P1)) && !s.world.is_request_partner_busy(ObjectId(P4)));
}

#[test]
fn an_invitation_answered_no_tells_both_and_a_stale_one_is_checked_again() {
    let mut s = clan_scene(ClanSettings::default(), |_| {});
    s.clan_input(P1, ClanInput::JoinAlly { id: P4 });
    s.take();
    s.clan_input(P4, ClanInput::AnswerJoinAlly { response: 0 });
    let out = s.take();
    assert_eq!(
        ids_to(&out, P4),
        [SystemMessageId::NoResponseYourEntranceToTheAllianceHasBeenCancelled]
    );
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::NoResponseInvitationToJoinAnAllianceHasBeenCancelled]
    );
    assert_eq!(s.world.clan(TIGERS).map(|c| c.ally_id), Some(0));
    assert!(!s.world.is_request_partner_busy(ObjectId(P1)));

    // Nobody invited the answering player.
    s.clan_input(P4, ClanInput::AnswerJoinAlly { response: 1 });
    assert!(s.take().is_empty());
}

#[test]
fn a_clan_leaves_the_alliance_and_waits_a_day() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_id = CLAN;
        tigers.ally_name = Some("Pride".to_owned());
        tigers.ally_crest_id = 3;
    });
    let leave = |s: &mut Scene, who: i32| {
        s.take();
        s.clan_input(who, ClanInput::AllyLeave);
        s.take()
    };
    assert_eq!(
        ids_to(&leave(&mut s, P7), P7),
        [SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction]
    );
    assert_eq!(
        ids_to(&leave(&mut s, P5), P5),
        [SystemMessageId::OnlyTheClanLeaderMayApplyForWithdrawalFromTheAlliance]
    );
    assert_eq!(
        ids_to(&leave(&mut s, P6), P6),
        [SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans]
    );
    assert_eq!(
        ids_to(&leave(&mut s, P1), P1),
        [SystemMessageId::AllianceLeadersCannotWithdraw]
    );

    let before = now_ms();
    let out = leave(&mut s, P4);
    assert_eq!(
        ids_to(&out, P4),
        [SystemMessageId::YouHaveWithdrawnFromTheAlliance]
    );
    let tigers = s.world.clan(TIGERS).unwrap();
    assert_eq!(
        (tigers.ally_id, tigers.ally_name.clone(), tigers.ally_crest_id),
        (0, None, 0)
    );
    assert_eq!(tigers.ally_penalty_type, 1);
    assert!((before + DAY_MS..=now_ms() + DAY_MS).contains(&tigers.ally_penalty_expiry_time));
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes[0],
        ClanWrite::Crest(CrestWrite::AllyCrestOfClan {
            clan_id: TIGERS,
            crest_id: 0
        })
    );
    match &writes[1] {
        ClanWrite::Update(row) => {
            assert_eq!((row.clan_id, row.ally_id, row.ally_penalty_type), (TIGERS, 0, 1))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_alliance_leader_expels_a_clan() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_id = CLAN;
        tigers.ally_name = Some("Pride".to_owned());
    });
    let dismiss = |s: &mut Scene, who: i32, clan: &str| {
        s.take();
        s.clan_input(
            who,
            ClanInput::AllyDismiss {
                clan_name: clan.to_owned(),
            },
        );
        ids_to(&s.take(), who)
    };
    assert_eq!(
        dismiss(&mut s, P7, "Tigers"),
        [SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction]
    );
    assert_eq!(
        dismiss(&mut s, P6, "Tigers"),
        [SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans]
    );
    assert_eq!(
        dismiss(&mut s, P4, "Lions"),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    assert_eq!(
        dismiss(&mut s, P2, "Tigers"),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    assert_eq!(
        dismiss(&mut s, P1, "Nobody"),
        [SystemMessageId::ThatClanDoesNotExist]
    );
    assert_eq!(
        dismiss(&mut s, P1, "lions"),
        [SystemMessageId::AllianceLeadersCannotWithdraw]
    );
    assert_eq!(dismiss(&mut s, P1, "Bears"), [SystemMessageId::DifferentAlliance]);
    assert!(clan_writes(&s.outs.take()).is_empty());

    let before = now_ms();
    assert_eq!(
        dismiss(&mut s, P1, "TIGERS"),
        [SystemMessageId::YouHaveSucceededInExpellingTheClan]
    );
    let lions = s.world.clan(CLAN).unwrap();
    assert_eq!((lions.ally_penalty_type, lions.ally_id), (3, CLAN));
    assert!((before + DAY_MS..=now_ms() + DAY_MS).contains(&lions.ally_penalty_expiry_time));
    let tigers = s.world.clan(TIGERS).unwrap();
    assert_eq!(
        (tigers.ally_id, tigers.ally_name.clone(), tigers.ally_penalty_type),
        (0, None, 2)
    );
    let writes = clan_writes(&s.outs.take());
    let rows: Vec<(i32, i32, i32)> = writes
        .iter()
        .filter_map(|w| match w {
            ClanWrite::Update(row) => Some((row.clan_id, row.ally_id, row.ally_penalty_type)),
            _ => None,
        })
        .collect();
    // The inviter first (`leaderClan.updateClanInDB()`), then the expelled clan.
    assert_eq!(rows, [(CLAN, CLAN, 3), (TIGERS, 0, 2)]);
    assert!(writes.contains(&ClanWrite::Crest(CrestWrite::AllyCrestOfClan {
        clan_id: TIGERS,
        crest_id: 0
    })));
}

#[test]
fn dissolving_an_alliance_frees_the_clans_and_keeps_javas_stale_crest() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        for (clan, crest) in [(CLAN, 2), (TIGERS, 2)] {
            let d = data_of(records, clan);
            d.ally_crest_id = crest;
        }
        let tigers = data_of(records, TIGERS);
        tigers.ally_id = CLAN;
        tigers.ally_name = Some("Pride".to_owned());
    });
    s.world.load_crests(vec![
        crest_record(1, &[1], 3),
        crest_record(2, &[2], 3),
        crest_record(3, &[3], 1),
    ]);
    s.outs.take();
    // Only the alliance's leader.
    s.vm_command(P4, "dissolve_ally");
    assert_eq!(
        ids_to(&s.take(), P4),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    s.vm_command(P6, "dissolve_ally");
    assert_eq!(
        ids_to(&s.take(), P6),
        [SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans]
    );

    let before = now_ms();
    s.vm_command(P1, "dissolve_ally");
    let out = s.take();
    // Everybody online in the alliance is told.
    for who in [P1, P2, P3, P4, P5] {
        assert_eq!(
            ids_to(&out, who),
            [SystemMessageId::TheAllianceHasBeenDissolved],
            "{who}"
        );
    }
    assert!(ids_to(&out, P6).is_empty());
    let lions = s.world.clan(CLAN).unwrap();
    assert_eq!(
        (lions.ally_id, lions.ally_name.clone(), lions.ally_penalty_type),
        (0, None, 4)
    );
    assert!((before + DAY_MS..=now_ms() + DAY_MS).contains(&lions.ally_penalty_expiry_time));
    let tigers = s.world.clan(TIGERS).unwrap();
    assert_eq!(
        (
            tigers.ally_id,
            tigers.ally_name.clone(),
            tigers.ally_penalty_expiry_time
        ),
        (0, None, 0)
    );
    // NOTE(java-quirk): `changeAllyCrest(0, false)` runs after `setAllyId(0)`: its statement is
    // `WHERE ally_id = 0`, the clans keep the old crest id and the crest row of the alliance goes.
    assert_eq!((lions.ally_crest_id, tigers.ally_crest_id), (2, 2));
    assert!(s.world.crest(2).is_none());
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes
            .iter()
            .filter_map(|w| match w {
                ClanWrite::Update(row) => Some((row.clan_id, row.ally_id)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [(TIGERS, 0), (CLAN, 0)]
    );
    let crest_writes: Vec<&ClanWrite> = writes
        .iter()
        .filter(|w| matches!(w, ClanWrite::Crest(_)))
        .collect();
    assert_eq!(
        crest_writes,
        [
            &ClanWrite::Crest(CrestWrite::Delete { crest_id: 2 }),
            &ClanWrite::Crest(CrestWrite::AllyCrestOfAlly {
                ally_id: 0,
                crest_id: 0
            }),
        ]
    );
    // A second alliance within the day is refused.
    s.vm_command(P1, "create_ally Again");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::YouCannotCreateANewAllianceWithin1DayOfDissolution]
    );
}

#[test]
fn the_alliance_dialog_command_leaves_the_leader_check_to_request_dismiss_ally() {
    let mut s = clan_scene(ClanSettings::default(), |_| {});
    s.take();
    s.clan_input(P2, ClanInput::DismissAlly);
    assert_eq!(
        ids_to(&s.take(), P2),
        [SystemMessageId::ThisFeatureIsOnlyAvailableToAllianceLeaders]
    );
    s.clan_input(P1, ClanInput::DismissAlly);
    assert_eq!(s.world.clan(CLAN).map(|c| c.ally_id), Some(0));
}

#[test]
fn the_alliance_window_lists_the_clans_and_the_connections() {
    let mut s = clan_scene(ClanSettings::default(), |records| {
        let tigers = data_of(records, TIGERS);
        tigers.ally_id = CLAN;
        tigers.ally_name = Some("Pride".to_owned());
    });
    s.take();
    s.clan_input(P6, ClanInput::AllyInfo);
    assert_eq!(
        ids_to(&s.take(), P6),
        [SystemMessageId::YouAreNotCurrentlyAlliedWithAnyClans]
    );
    s.clan_input(P4, ClanInput::AllyInfo);
    let out = s.take();
    let info = &decoded::<AllianceInfo>(&out, P4, "ALLIANCE_INFO")[0];
    assert_eq!(
        (
            info.name.as_str(),
            info.total,
            info.online,
            info.leader_clan.as_str(),
            info.leader_player.as_str()
        ),
        ("Pride", 5, 5, "Lions", "P1")
    );
    let mut clans: Vec<(&str, i32, &str, i32, i32)> = info
        .allies
        .iter()
        .map(|a| {
            (
                a.clan_name.as_str(),
                a.level,
                a.leader_name.as_str(),
                a.total,
                a.online,
            )
        })
        .collect();
    clans.sort_unstable();
    assert_eq!(clans, [("Lions", 5, "P1", 3, 3), ("Tigers", 5, "P4", 2, 2)]);

    // The chat lines: the alliance, then one block per clan with the separators between them.
    let messages = messages_with_params(&out, P4);
    let ids: Vec<SystemMessageId> = messages.iter().map(|m| m.0).collect();
    assert_eq!(
        &ids[..5],
        [
            SystemMessageId::AllianceInformation,
            SystemMessageId::AllianceNameS1,
            SystemMessageId::AllianceLeaderS2OfS1,
            SystemMessageId::ConnectionS1TotalS2,
            SystemMessageId::AffiliatedClansTotalS1ClanS,
        ]
    );
    assert_eq!(messages[1].1, [text("Pride")]);
    assert_eq!(messages[2].1, [text("Lions"), text("P1")]);
    assert_eq!(
        messages[3].1,
        [SystemMessageParam::IntNumber(5), SystemMessageParam::IntNumber(5)]
    );
    assert_eq!(messages[4].1, [SystemMessageParam::IntNumber(2)]);
    let block = [
        SystemMessageId::ClanNameS1,
        SystemMessageId::ClanLeaderS1,
        SystemMessageId::ClanLevelS1,
        SystemMessageId::ConnectionS1TotalS2,
    ];
    let mut expected = vec![SystemMessageId::ClanInformation];
    expected.extend(block);
    expected.push(SystemMessageId::Empty4);
    expected.extend(block);
    expected.push(SystemMessageId::Empty5);
    assert_eq!(&ids[5..], expected);
}

// ---------------------------------------------------------------------------------- wars

#[test]
fn a_war_is_declared_and_both_clans_are_told() {
    use l2_protocol_game::server::visibility::relation;
    let mut s = clan_scene(war_settings(), |records| {
        // No alliance between the two.
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
    });
    s.clan_input(
        P1,
        ClanInput::StartPledgeWar {
            pledge_name: "tigers".to_owned(),
        },
    );
    let out = s.take();
    assert_eq!(
        s.world.clan(CLAN).map(|c| c.at_war_with.clone()),
        Some(vec![TIGERS])
    );
    assert_eq!(
        s.world.clan(TIGERS).map(|c| c.at_war_attackers.clone()),
        Some(vec![CLAN])
    );
    assert!(s.world.clan(TIGERS).is_some_and(|c| !c.is_at_war()));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::War(WarWrite::Store {
            clan1: CLAN,
            clan2: TIGERS
        })]
    );
    for who in [P1, P2, P3] {
        assert!(has_message(
            &out,
            who,
            SystemMessageId::AClanWarHasBeenDeclaredAgainstTheClanS1IfYouAreKilledDuringTheClanWarByMembersOfTheOpposingClanYouWillOnlyLoseAQuarterOfTheNormalExperienceFromDeath,
            &[text("Tigers")]
        ));
    }
    for who in [P4, P5] {
        assert!(has_message(
            &out,
            who,
            SystemMessageId::S1HasDeclaredAClanWar,
            &[text("Lions")]
        ));
    }
    // `broadcastClanStatus()` and `broadcastUserInfo()` of every online member of both.
    for who in [P1, P2, P3, P4, P5] {
        let names = names_to(&out, who);
        assert!(
            names.contains(&"PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL".to_owned()),
            "{who}"
        );
        assert!(names.contains(&"USER_INFO".to_owned()), "{who}");
        let all = decoded::<PledgeShowMemberListAll>(&out, who, "PLEDGE_SHOW_MEMBER_LIST_ALL");
        assert_eq!(all.len(), 1, "{who}");
    }
    assert!(!names_to(&out, P6).contains(&"USER_INFO".to_owned()));

    // The Lions see the Tigers at war; the Tigers do not see it back, and nobody is auto
    // attackable on one side.
    let (p2, p4) = (ObjectId(P2), ObjectId(P4));
    assert_ne!(s.world.relation(p4, p2) & relation::ONE_SIDED_WAR, 0);
    assert_eq!(s.world.relation(p2, p4) & relation::ONE_SIDED_WAR, 0);
    assert!(!s.world.is_auto_attackable(p2, p4));

    // The same war again: five days have to pass.
    s.take();
    s.clan_input(
        P1,
        ClanInput::StartPledgeWar {
            pledge_name: "Tigers".to_owned(),
        },
    );
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::YouHaveAlreadyBeenAtWarWithTheS1Clan5DaysMustPassBeforeYouCanDeclareWarAgain,
        &[text("Tigers")]
    ));
    assert!(names_to(&out, P1).contains(&"ACTION_FAIL".to_owned()));
    assert!(clan_writes(&s.outs.take()).is_empty());

    // The Tigers answer with a declaration of their own: a war both ways.
    s.clan_input(
        P4,
        ClanInput::StartPledgeWar {
            pledge_name: "Lions".to_owned(),
        },
    );
    s.take();
    assert_eq!(
        s.world.relation(p4, p2) & (relation::ONE_SIDED_WAR | relation::MUTUAL_WAR),
        relation::ONE_SIDED_WAR | relation::MUTUAL_WAR
    );
    assert!(s.world.is_auto_attackable(p2, p4));
}

#[test]
fn a_war_declaration_is_checked_like_request_start_pledge_war() {
    let refuse = |settings: ClanSettings,
                  tweak: &dyn Fn(&mut Vec<ClanRecord>),
                  who: i32,
                  name: &str|
     -> (Vec<SystemMessageId>, bool) {
        let mut s = clan_scene(settings, |r| tweak(r));
        s.clan_input(
            who,
            ClanInput::StartPledgeWar {
                pledge_name: name.to_owned(),
            },
        );
        let out = s.take();
        let failed = names_to(&out, who).contains(&"ACTION_FAIL".to_owned());
        assert!(clan_writes(&s.outs.take()).is_empty());
        (ids_to(&out, who), failed)
    };
    let no_ally = |records: &mut Vec<ClanRecord>| {
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
    };
    let too_small = SystemMessageId::AClanWarCanOnlyBeDeclaredIfTheClanIsLevel3OrAboveAndTheNumberOfClanMembersIsFifteenOrGreater;
    // Bears have one member: the defaults ask for 15 on both sides, the test setting for 2.
    assert_eq!(
        refuse(ClanSettings::default(), &no_ally, P1, "Tigers"),
        (vec![too_small], true)
    );
    assert_eq!(
        refuse(war_settings(), &no_ally, P6, "Lions"),
        (vec![too_small], true)
    );
    assert_eq!(
        refuse(war_settings(), &no_ally, P1, "Bears"),
        (vec![too_small], true)
    );
    // Level 2.
    assert_eq!(
        refuse(
            war_settings(),
            &|r| {
                no_ally(r);
                data_of(r, TIGERS).level = 2;
            },
            P1,
            "Tigers"
        ),
        (vec![too_small], true)
    );
    // Rank 5 has no war declaration privilege.
    assert_eq!(
        refuse(war_settings(), &no_ally, P2, "Tigers"),
        (vec![SystemMessageId::YouAreNotAuthorizedToDoThat], true)
    );
    assert_eq!(
        refuse(war_settings(), &no_ally, P1, "Nobody"),
        (
            vec![SystemMessageId::AClanWarCannotBeDeclaredAgainstAClanThatDoesNotExist],
            true
        )
    );
    // Allied clans.
    assert_eq!(
        refuse(
            war_settings(),
            &|r| {
                let tigers = data_of(r, TIGERS);
                tigers.ally_id = CLAN;
                tigers.ally_name = Some("Pride".to_owned());
            },
            P1,
            "Tigers"
        ),
        (
            vec![SystemMessageId::ADeclarationOfClanWarAgainstAnAlliedClanCanTBeMade],
            true
        )
    );
    // Without a clan nothing happens at all.
    assert_eq!(refuse(war_settings(), &no_ally, P7, "Tigers"), (vec![], false));
}

#[test]
fn a_war_is_stopped_by_a_member_with_the_privilege() {
    let mut s = clan_scene(war_settings(), |records| {
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
        // P2's rank may declare and stop wars (privilege bit 5).
        records[0].rank_privs = vec![(5, 0, 1 << 5)];
    });
    s.world.store_clan_wars(CLAN, TIGERS);
    s.take();
    s.outs.take();
    let stop = |s: &mut Scene, who: i32, name: &str| {
        s.take();
        s.clan_input(
            who,
            ClanInput::StopPledgeWar {
                pledge_name: name.to_owned(),
            },
        );
        s.take()
    };
    assert_eq!(texts_to(&stop(&mut s, P1, "Nobody"), P1), ["No such clan."]);
    assert_eq!(
        texts_to(&stop(&mut s, P1, "Bears"), P1),
        ["You aren't at war with this clan."]
    );
    // The Tigers never declared war on the Lions.
    assert_eq!(
        texts_to(&stop(&mut s, P4, "Lions"), P4),
        ["You aren't at war with this clan."]
    );
    // A member of the clan in a fight stops it.
    s.world.add_attack_stance_task(ObjectId(P3));
    let out = stop(&mut s, P1, "Tigers");
    assert_eq!(
        ids_to(&out, P1),
        [SystemMessageId::ACeaseFireDuringAClanWarCanNotBeCalledWhileMembersOfYourClanAreEngagedInBattle]
    );
    assert!(s.world.clan(CLAN).is_some_and(|c| c.is_at_war_with(TIGERS)));
    s.world.remove_attack_stance_task(ObjectId(P3));

    let out = stop(&mut s, P2, "Tigers");
    assert!(!ids_to(&out, P2).contains(&SystemMessageId::YouAreNotAuthorizedToDoThat));
    assert!(s.world.clan(CLAN).is_some_and(|c| !c.is_at_war()));
    assert!(
        s.world
            .clan(TIGERS)
            .is_some_and(|c| c.at_war_attackers.is_empty())
    );
    for who in [P1, P2, P3] {
        assert!(has_message(
            &out,
            who,
            SystemMessageId::TheWarAgainstS1ClanHasBeenStopped,
            &[text("Tigers")]
        ));
    }
    for who in [P4, P5] {
        assert!(has_message(
            &out,
            who,
            SystemMessageId::TheClanS1HasDecidedToStopTheWar,
            &[text("Lions")]
        ));
    }
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::War(WarWrite::Delete {
            clan1: CLAN,
            clan2: TIGERS
        })]
    );

    // Without the privilege (rank 5 of the Tigers) the stop is refused.
    s.world.store_clan_wars(TIGERS, CLAN);
    let out = stop(&mut s, P5, "Lions");
    assert_eq!(ids_to(&out, P5), [SystemMessageId::YouAreNotAuthorizedToDoThat]);
}

#[test]
fn a_clan_surrenders() {
    let mut s = clan_scene(war_settings(), |_| {});
    s.world.store_clan_wars(CLAN, TIGERS);
    s.take();
    s.outs.take();
    s.clan_input(
        P1,
        ClanInput::SurrenderPledgeWar {
            pledge_name: "Nobody".to_owned(),
        },
    );
    assert_eq!(texts_to(&s.take(), P1), ["No such clan."]);
    s.clan_input(
        P1,
        ClanInput::SurrenderPledgeWar {
            pledge_name: "Bears".to_owned(),
        },
    );
    assert_eq!(texts_to(&s.take(), P1), ["You aren't at war with this clan."]);
    // The name goes into the message as the player typed it.
    s.clan_input(
        P1,
        ClanInput::SurrenderPledgeWar {
            pledge_name: "TIGERS".to_owned(),
        },
    );
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::YouHaveSurrenderedToTheS1Clan,
        &[text("TIGERS")]
    ));
    assert!(s.world.clan(CLAN).is_some_and(|c| !c.is_at_war()));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::War(WarWrite::Delete {
            clan1: CLAN,
            clan2: TIGERS
        })]
    );
}

#[test]
fn the_war_list_shows_declared_wars_and_attacks() {
    let mut s = clan_scene(war_settings(), |_| {});
    s.world.store_clan_wars(CLAN, TIGERS);
    s.world.store_clan_wars(CLAN, BEARS);
    s.world.store_clan_wars(TIGERS, CLAN);
    s.take();
    s.clan_input(P1, ClanInput::PledgeWarList { tab: 0 });
    let out = s.take();
    let list = &decoded::<PledgeReceiveWarList>(&out, P1, "PLEDGE_RECEIVE_WAR_LIST")[0];
    let mut names = list.clan_names.clone();
    names.sort_unstable();
    assert_eq!(
        (list.tab, list.list_size, names),
        (0, 2, vec!["Bears".to_owned(), "Tigers".to_owned()])
    );
    s.clan_input(P1, ClanInput::PledgeWarList { tab: 1 });
    let out = s.take();
    let list = &decoded::<PledgeReceiveWarList>(&out, P1, "PLEDGE_RECEIVE_WAR_LIST")[0];
    assert_eq!(
        (list.tab, list.list_size, list.clan_names.clone()),
        (1, 1, vec!["Tigers".to_owned()])
    );
    // No clan, no answer.
    s.clan_input(P7, ClanInput::PledgeWarList { tab: 0 });
    assert!(s.take().is_empty());
}

#[test]
fn the_war_replies_use_the_active_requester_of_any_transaction() {
    let mut s = clan_scene(war_settings(), |records| {
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
    });
    // Nobody asked the player: nothing happens.
    s.clan_input(P4, ClanInput::ReplyStartPledgeWar { answer: 1 });
    assert!(s.take().is_empty());
    assert!(clan_writes(&s.outs.take()).is_empty());

    // NOTE(java-quirk): `RequestStartPledgeWar` declares the war at once and nobody is asked, so
    // the replies read whatever request the player has, here a trade request of P1.
    s.input(P1, ClientInput::TradeRequest { object_id: P4 });
    s.take();
    s.clan_input(P4, ClanInput::ReplyStartPledgeWar { answer: 0 });
    let out = s.take();
    // NOTE(java-quirk): Java adds no clan name to this message.
    assert_eq!(
        raw_message_ids(&out, P1),
        [SystemMessageId::TheS1ClanDidNotRespondWarProclamationHasBeenRefused2.id()]
    );
    assert!(s.world.clan(CLAN).is_some_and(|c| !c.is_at_war()));

    s.input(P1, ClientInput::TradeRequest { object_id: P4 });
    s.take();
    s.clan_input(P4, ClanInput::ReplyStartPledgeWar { answer: 1 });
    assert!(s.world.clan(CLAN).is_some_and(|c| c.is_at_war_with(TIGERS)));
    // The request was answered.
    s.clan_input(P4, ClanInput::ReplyStartPledgeWar { answer: 1 });
    s.take();
    s.outs.take();

    // Stop and surrender replies delete the war the same way.
    s.input(P1, ClientInput::TradeRequest { object_id: P4 });
    s.take();
    s.clan_input(P4, ClanInput::ReplyStopPledgeWar { answer: 0 });
    assert_eq!(
        raw_message_ids(&s.take(), P1),
        [SystemMessageId::RequestToEndWarHasBeenDenied.id()]
    );
    s.input(P1, ClientInput::TradeRequest { object_id: P4 });
    s.take();
    s.clan_input(P4, ClanInput::ReplyStopPledgeWar { answer: 1 });
    assert!(s.world.clan(CLAN).is_some_and(|c| !c.is_at_war()));
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::War(WarWrite::Delete {
            clan1: CLAN,
            clan2: TIGERS
        })]
    );
}

#[test]
fn a_kill_in_a_clan_war_moves_reputation() {
    let at_war = |lions_reputation: i32| {
        let mut s = clan_scene(war_settings(), |records| {
            let lions = data_of(records, CLAN);
            lions.ally_id = 0;
            lions.ally_name = None;
            lions.reputation_score = lions_reputation;
            data_of(records, TIGERS).reputation_score = 100;
        });
        s.world.store_clan_wars(CLAN, TIGERS);
        s.world.store_clan_wars(TIGERS, CLAN);
        s.take();
        s
    };
    let reputation = |s: &Scene| {
        (
            s.world.clan(CLAN).map(|c| c.reputation_score).unwrap(),
            s.world.clan(TIGERS).map(|c| c.reputation_score).unwrap(),
        )
    };

    // `ReputationScorePerKill` 1: the killer's clan gains, the victim's clan loses.
    let mut s = at_war(100);
    assert!(s.world.do_die(ObjectId(P2), Some(ObjectId(P4))));
    assert_eq!(reputation(&s), (99, 101));

    // A victim clan without reputation gives none, but the killer's clan (with some) still takes
    // its point, as in Java.
    let mut s = at_war(0);
    s.world.do_die(ObjectId(P2), Some(ObjectId(P4)));
    assert_eq!(reputation(&s), (-1, 100));

    // A war on one side only is a plain PK: nothing moves.
    let mut s = clan_scene(war_settings(), |records| {
        let lions = data_of(records, CLAN);
        lions.ally_id = 0;
        lions.ally_name = None;
        lions.reputation_score = 100;
        data_of(records, TIGERS).reputation_score = 100;
    });
    s.world.store_clan_wars(CLAN, TIGERS);
    s.world.do_die(ObjectId(P2), Some(ObjectId(P4)));
    assert_eq!(reputation(&s), (100, 100));

    // A member of the same clan is no enemy.
    let mut s = at_war(100);
    s.world.do_die(ObjectId(P3), Some(ObjectId(P2)));
    assert_eq!(reputation(&s), (100, 100));
}

// ---------------------------------------------------------------------------------- scripts

const SCRIPT_PAGES: [(&str, &str, &str); 8] = [
    (
        "ClanMaster",
        "9000-01.htm",
        "<html><body>CM <a action=\"bypass Script ClanMaster 9000-03.htm\">level</a>\
         <a action=\"bypass Script ClanMaster 9000-12a.htm\">academy</a>\
         <a action=\"bypass Script ClanMaster 9000-02.htm\">create</a>%objectId% %playername%</body></html>",
    ),
    (
        "ClanMaster",
        "9000-02.htm",
        "<html><body>PAGE 9000-02</body></html>",
    ),
    (
        "ClanMaster",
        "9000-03.htm",
        "<html><body>PAGE 9000-03</body></html>",
    ),
    (
        "ClanMaster",
        "9000-03-no.htm",
        "<html><body>PAGE 9000-03-no</body></html>",
    ),
    (
        "ClanMaster",
        "9000-07-no.htm",
        "<html><body>PAGE 9000-07-no</body></html>",
    ),
    (
        "ClanMaster",
        "9000-12a.htm",
        "<html><body>PAGE 9000-12a</body></html>",
    ),
    (
        "AllianceMaster",
        "9001-01.htm",
        "<html><body>AM <a action=\"bypass Script AllianceMaster 9001-02.htm\">create</a></body></html>",
    ),
    (
        "AllianceMaster",
        "9001-02.htm",
        "<html><body>PAGE 9001-02</body></html>",
    ),
];

fn html_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

// The pages of `ClanMaster` and `AllianceMaster` are scripts of `l2-scripts` (S-API-02); their
// tests are in `crates/l2-scripts/src/village_master/`. Without a script installed the world
// answers `bypass Script ...` with the "no quest" page.
#[test]
fn an_unknown_script_has_no_quest() {
    let mut s = clan_scene(ClanSettings::default(), |_| {});
    for command in ["Script ClanMaster", "Script AllianceMaster"] {
        s.talk(P1);
        s.take();
        s.input(
            P1,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
        let pages = html_to(&s.take(), P1);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("not on a quest that involves this NPC"));
    }

    // `bypass Script` alone is the quest list: no quests yet, the "no quest" page.
    s.talk(P1);
    s.take();
    s.input(
        P1,
        ClientInput::Bypass {
            command: "Script".to_owned(),
        },
    );
    let pages = html_to(&s.take(), P1);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("not on a quest that involves this NPC"));
}
