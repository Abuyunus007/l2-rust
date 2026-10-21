#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Clans (M6-CLAN-01): the clan table, members entering and leaving, the village master's clan
//! commands, invitations, withdrawal and expulsion, ranks and privileges, the leader change,
//! the clan level, sub pledges, the dissolution, the notice, the clan target handlers and the
//! clan conditions.
//! Java: entity/clan/{Clan,ClanMember}.java, data/sql/ClanTable.java,
//! entity/actor/instance/VillageMaster.java, network/clientpackets/{RequestJoinPledge,
//! RequestAnswerJoinPledge,RequestWithdrawalPledge,RequestOustPledgeMember,RequestPledge*}.java,
//! network/clientpackets/EnterWorld.java (the clan block), Player.restore / deleteMe,
//! dist/game/data/scripts/handlers/skill/targets/{Clan,PartyClan,CorpseClan}.java.
//!
//! The messages, packets and rows are Java's; the numbers (costs, limits, ranks, masks) come
//! from the Java sources and the default `Feature.ini` / `Player.ini`.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::pledge::{
    AskJoinPledge, JoinPledge, ManagePledgePower, PledgeInfo, PledgePowerGradeList, PledgeReceiveMemberInfo,
    PledgeReceivePowerInfo, PledgeReceiveSubPledgeCreated, PledgeShowInfoUpdate, PledgeShowMemberListAdd,
    PledgeShowMemberListAll, PledgeShowMemberListDelete, PledgeShowMemberListDeleteAll,
    PledgeShowMemberListUpdate, PledgeSkillList, PledgeStatusChanged,
};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam, UserInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanAccess, ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, ClanRow};
use l2_world::skills::TargetContext;
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
    </body></html>";

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
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
            "data/html/clanNotice.htm",
            "<html><body>%clan_name%: %notice_text%</body></html>",
        );
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

    fn leave(&mut self, id: i32) {
        self.world.handle(WorldCommand::Disconnected {
            session: SessionId(u64::try_from(id).unwrap()),
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

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms / 50 {
            self.world.tick();
        }
    }

    fn state(&self, id: i32) -> PlayerClanState {
        self.world.player_clan_state(ObjectId(id))
    }

    fn clan(&self) -> &Clan {
        self.world.clan(CLAN).unwrap()
    }

    /// The last stored snapshot of a player.
    fn stored(&mut self, id: i32) -> PlayerData {
        self.outs
            .take()
            .into_iter()
            .rev()
            .find_map(|o| match o {
                PersistCommand::StorePlayer(p) if p.object_id.0 == id => Some(*p),
                _ => None,
            })
            .unwrap()
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

// ---------------------------------------------------------------------------------- the table

#[test]
fn clans_load_like_clan_restore() {
    let mut s = Scene::new();
    let mut lions = lions(5, vec![member(P1, 1, 0), member(P2, 5, 0), member(P3, 9, -1)]);
    if let Some(data) = lions.data.as_mut() {
        data.reputation_score = 1234;
        // Expired penalties are dropped (the clan one a day after its end, `DaysBeforeJoinAClan`).
        data.char_penalty_expiry_time = now_ms() - 2 * DAY_MS;
        data.ally_penalty_expiry_time = 1;
        data.ally_penalty_type = 1;
        // The alliance's leading clan 999 does not exist: `allianceCheck` drops it.
        data.ally_id = 999;
        data.ally_name = Some("Old".to_owned());
    }
    // A rank outside 1 to 9 ends the restore of the privileges (Java's NullPointerException).
    lions.rank_privs = vec![(2, 0, 0x10), (-1, 0, 7), (12, 0, 5), (4, 0, 0x20)];
    lions.sub_pledges = vec![(100, "Royal".to_owned(), P2), (-1, "Cubs".to_owned(), 0)];
    lions.skills = vec![(370, 1, -2), (371, 1, 0), (372, 1, 100), (373, 1, 1001)];
    lions.notices = vec![("TRUE".to_owned(), "Hello".to_owned())];
    let tigers = ClanRecord {
        clan_id: 600,
        data: Some(ClanDataRecord {
            name: Some("Tigers".to_owned()),
            level: 2,
            char_penalty_expiry_time: now_ms() + DAY_MS,
            ..ClanDataRecord::default()
        }),
        ..ClanRecord::default()
    };
    s.world.load_clans(
        vec![lions, tigers],
        vec![
            ("500".to_owned(), "600".to_owned()),
            ("x".to_owned(), "600".to_owned()),
            ("600".to_owned(), "500".to_owned()),
        ],
    );

    let clan = s.clan();
    assert_eq!(
        (clan.name.as_str(), clan.level, clan.reputation_score),
        ("Lions", 5, 1234)
    );
    assert_eq!(clan.leader, Some(ObjectId(P1)));
    let members: Vec<i32> = clan.members().iter().map(|m| m.object_id.0).collect();
    assert_eq!(members, [P1, P2, P3]);
    assert_eq!(
        (
            clan.char_penalty_expiry_time,
            clan.ally_penalty_expiry_time,
            clan.ally_penalty_type
        ),
        (0, 0, 0)
    );
    assert_eq!((clan.ally_id, clan.ally_name.as_deref()), (0, None));
    assert_eq!((clan.rank_privs(2), clan.rank_privs(4)), (0x10, 0));
    assert!(!clan.has_rank(12));
    let subs: Vec<i32> = clan.all_sub_pledges().iter().map(|s| s.id).collect();
    assert_eq!(subs, [-1, 100]);
    assert_eq!(clan.all_skills(), [(370, 1)]);
    assert_eq!(clan.all_sub_skills(), [(0, 371, 1), (100, 372, 1)]);
    assert_eq!(
        (clan.notice_enabled, clan.notice.as_deref()),
        (true, Some("Hello"))
    );
    // The first row is a war; the bad second row ends the restore (`getInt` throws).
    assert_eq!(clan.at_war_with, [600]);
    assert!(clan.at_war_attackers.is_empty());
    let tigers = s.world.clan(600).unwrap();
    assert_eq!(tigers.at_war_attackers, [CLAN]);
    // A penalty still running stays.
    assert!(tigers.char_penalty_expiry_time > now_ms());
    // Side by side the start-up check leaves the rows to the Java game server (D-49).
    assert!(clan_writes(&s.outs.take()).is_empty());

    // The ranks come one second later: level 5 is first, level 2 has none.
    assert_eq!(s.clan().rank, 0);
    s.run_ms(1000);
    assert_eq!((s.clan().rank, s.world.clan(600).unwrap().rank), (1, 0));
}

#[test]
fn the_alliance_check_writes_with_the_maintenance_switch() {
    let mut s = Scene::with(ClanSettings {
        maintenance_database_writes: true,
        ..ClanSettings::default()
    });
    let mut record = lions(5, vec![member(P1, 1, 0)]);
    if let Some(data) = record.data.as_mut() {
        data.ally_id = 999;
        data.ally_name = Some("Old".to_owned());
        data.ally_crest_id = 77;
    }
    s.load(vec![record]);
    let writes = clan_writes(&s.outs.take());
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes[0],
        ClanWrite::AllyCrestOfClan {
            clan_id: CLAN,
            crest_id: 0
        }
    );
    let ClanWrite::Update(row) = &writes[1] else {
        panic!("{writes:?}");
    };
    assert_eq!(
        (row.clan_id, row.leader_id, row.ally_id, row.ally_name.as_deref()),
        (CLAN, P1, 0, None)
    );
    assert_eq!(s.clan().ally_crest_id, 0);
}

#[test]
fn clan_ranks_follow_get_clan_rank() {
    let mut s = Scene::new();
    let clan = |id: i32, level: i32, reputation: i32| ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(format!("C{id}")),
            level,
            reputation_score: reputation,
            ..ClanDataRecord::default()
        }),
        ..ClanRecord::default()
    };
    s.load(vec![
        clan(1, 5, 100),
        clan(2, 5, 200),
        clan(3, 6, 0),
        clan(4, 3, 50),
        clan(5, 2, 9999),
    ]);
    // Same level and less or equal reputation counts as above.
    let ranks: Vec<i32> = (1..=5).map(|id| s.world.clan_rank(id)).collect();
    assert_eq!(ranks, [3, 2, 1, 4, 0]);
}

// ---------------------------------------------------------------------------------- the world

#[test]
fn a_member_entering_sees_the_clan_window_and_the_others_are_told() {
    let mut s = Scene::new();
    let mut record = lions_of_three(5, 0x2);
    record.skills = vec![(370, 1, -2)];
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 0, 0));
    let out = s.take();

    // Player.restore: the leader has every privilege and grade 1; level 5 makes it class 4.
    let ui = decoded::<UserInfo>(&out, P1, "USER_INFO");
    let first = &ui[0];
    assert_eq!(first.clan_id, CLAN);
    assert_eq!(first.clan_privileges, 0x00FF_FFFF);
    assert_eq!(first.relation, 0x40);
    assert_eq!((first.pledge_class, first.pledge_type), (4, 0));
    assert_eq!(s.state(P1).power_grade, 1);

    // EnterWorld's clan block, in Java's order after the first UserInfo.
    let names = names_to(&out, P1);
    let at = |name: &str| names.iter().position(|n| n == name).unwrap();
    assert!(at("PLEDGE_SKILL_LIST") < at("PLEDGE_SHOW_MEMBER_LIST_ALL"));
    assert!(at("PLEDGE_SHOW_MEMBER_LIST_ALL") < at("PLEDGE_STATUS_CHANGED"));
    let skills = decoded::<PledgeSkillList>(&out, P1, "PLEDGE_SKILL_LIST");
    assert_eq!(skills[0].skills.len(), 1);
    assert_eq!(skills[0].skills[0].display_id, 370);
    let all = &decoded::<PledgeShowMemberListAll>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_ALL")[0];
    assert_eq!(
        (
            all.clan_id,
            all.clan_name.as_str(),
            all.leader_name.as_str(),
            all.level
        ),
        (CLAN, "Lions", "P1", 5)
    );
    let rows: Vec<(String, i32, i32, i32)> = all
        .members
        .iter()
        .map(|m| (m.name.clone(), m.sex, m.race, m.online_object_id))
        .collect();
    // The online member shows its sex and race, the offline ones the constant 1 and 1.
    assert_eq!(
        rows,
        [
            ("P1".to_owned(), 0, 0, P1),
            ("P2".to_owned(), 1, 1, 0),
            ("P3".to_owned(), 1, 1, 0)
        ]
    );
    let status = decoded::<PledgeStatusChanged>(&out, P1, "PLEDGE_STATUS_CHANGED");
    assert_eq!((status[0].leader_id, status[0].clan_id), (P1, CLAN));

    // A second member: P1 is told; P2 holds the rank's own privileges (grade 0 becomes 5).
    s.enter(member_data(P2, CLAN, 0, 0));
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::ClanMemberS1HasLoggedIntoGame,
        &[text("P2")]
    ));
    let update = &decoded::<PledgeShowMemberListUpdate>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_UPDATE")[0];
    assert_eq!(update.name, "P2");
    assert_eq!(update.online.map(|o| o.object_id), Some(P2));
    assert_eq!(s.state(P2).privileges, ClanPrivilegesRef::Rank(5));
    assert_eq!(s.world.clan_privileges_of(ObjectId(P2)), 0x2);
    let ui = decoded::<UserInfo>(&out, P2, "USER_INFO");
    assert_eq!(
        (ui[0].pledge_class, ui[0].clan_privileges, ui[0].relation),
        (2, 0x2, 0)
    );

    // P2 leaves: P1 sees it offline, the member keeps P2's values, the store writes the clan
    // columns with the mask of the moment.
    s.leave(P2);
    let out = s.take();
    let update = &decoded::<PledgeShowMemberListUpdate>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_UPDATE")[0];
    assert_eq!((update.name.as_str(), update.online), ("P2", None));
    let m = s.clan().member(ObjectId(P2)).unwrap();
    assert_eq!((m.power_grade, m.has_player), (5, false));
    let stored = s.stored(P2);
    let clan = stored.profile.clan.unwrap();
    assert_eq!((clan.clan_id, clan.power_grade), (CLAN, 5));
    assert_eq!(clan.privileges, ClanPrivilegesRef::Own(0x2));
}

#[test]
fn a_member_of_a_clan_that_is_gone_leaves_it_at_login() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    // Clan 777 is unknown; P3 says it is in clan 500 but is not a member.
    for (id, clan_id) in [(P2, 777), (P3, CLAN)] {
        let mut data = member_data(id, clan_id, 5, 0);
        data.title = "Old title".to_owned();
        data.profile.noble = true;
        s.enter(data);
    }
    for id in [P2, P3] {
        assert_eq!(s.world.clan_id_of(ObjectId(id)), 0);
        assert_eq!(s.world.player(ObjectId(id)).unwrap().data.title, "");
    }
    // NOTE(java-quirk): the noble of the unknown clan gets class 5, the one that is no longer a
    // member of a clan that exists keeps 0.
    assert_eq!(
        (
            s.world.pledge_class_of(ObjectId(P2)),
            s.world.pledge_class_of(ObjectId(P3))
        ),
        (5, 0)
    );
    s.leave(P2);
    let stored = s.stored(P2);
    let clan = stored.profile.clan.unwrap();
    assert_eq!(
        (clan.clan_id, clan.power_grade, clan.stored_privileges()),
        (0, 0, 0)
    );
    assert_eq!(stored.title, "");
}

#[test]
fn the_join_penalty_and_the_notice_show_at_login() {
    let mut s = Scene::new();
    let mut record = lions(5, vec![member(P1, 1, 0)]);
    record.notices = vec![("true".to_owned(), "Hi\nall <b>x</b>".to_owned())];
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    let mut penalised = pdata(P2, 40);
    penalised.profile.clan = Some(PlayerClanState {
        clan_join_expiry_time: now_ms() + DAY_MS,
        ..PlayerClanState::default()
    });
    s.enter(penalised);
    let out = s.take();
    let pages = decoded::<NpcHtmlMessage>(&out, P1, "NPC_HTML_MESSAGE");
    assert_eq!(pages[0].html, "<html><body>Lions: Hi<br>all x</body></html>");
    assert!(ids_to(&out, P2).contains(
        &SystemMessageId::YouHaveRecentlyBeenDismissedFromAClanYouAreNotAllowedToJoinAnotherClanFor24Hours
    ));
    assert!(decoded::<NpcHtmlMessage>(&out, P2, "NPC_HTML_MESSAGE").is_empty());
}

// ---------------------------------------------------------------------------------- creating

#[test]
fn a_clan_is_created_at_the_village_master() {
    let mut s = Scene::new();
    s.enter(pdata(P1, 10));
    s.take();
    s.vm_command(P1, "create_clan Lions");
    let out = s.take();
    let clan_id = s.world.clan_id_of(ObjectId(P1));
    assert!(clan_id >= RUST_FIRST_OBJECT_ID, "{clan_id}");
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes,
        [ClanWrite::Insert(ClanRow {
            clan_id,
            name: "Lions".to_owned(),
            level: 0,
            castle_id: 0,
            blood_alliance_count: 0,
            blood_oath_count: 0,
            ally_id: 0,
            ally_name: None,
            leader_id: P1,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            new_leader_id: 0,
        })]
    );
    // createClan's packets in Java's order.
    let names: Vec<String> = names_to(&out, P1)
        .into_iter()
        .filter(|n| n.starts_with("PLEDGE") || n == "USER_INFO" || n == "SYSTEM_MESSAGE")
        .collect();
    assert_eq!(
        names,
        [
            "PLEDGE_SHOW_INFO_UPDATE",
            "PLEDGE_SHOW_MEMBER_LIST_ALL",
            "USER_INFO",
            "USER_INFO",
            "PLEDGE_SHOW_MEMBER_LIST_UPDATE",
            "SYSTEM_MESSAGE"
        ]
    );
    assert_eq!(ids_to(&out, P1), [SystemMessageId::YourClanHasBeenCreated]);
    let info = &decoded::<PledgeShowInfoUpdate>(&out, P1, "PLEDGE_SHOW_INFO_UPDATE")[0];
    assert_eq!((info.clan_id, info.level), (clan_id, 0));
    assert_eq!(s.world.clan_privileges_of(ObjectId(P1)), 0x00FF_FFFF);
    assert!(s.world.is_clan_leader(ObjectId(P1)));
    // Level 0: everybody is class 1.
    assert_eq!(s.world.pledge_class_of(ObjectId(P1)), 1);
    // The clan columns go with the next store.
    s.leave(P1);
    let stored = s.stored(P1);
    let clan = stored.profile.clan.unwrap();
    assert_eq!((clan.clan_id, clan.stored_privileges()), (clan_id, 0x00FF_FFFF));
}

#[test]
fn clan_creation_checks_tell_why() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(pdata(P2, 9));
    let mut waiting = pdata(P3, 20);
    waiting.profile.clan = Some(PlayerClanState {
        clan_create_expiry_time: now_ms() + DAY_MS,
        ..PlayerClanState::default()
    });
    s.enter(waiting);
    s.enter(pdata(P4, 20));
    s.take();
    let cases: [(i32, &str, SystemMessageId); 9] = [
        (
            P2,
            "create_clan Tigers",
            SystemMessageId::YouDoNotMeetTheCriteriaInOrderToCreateAClan,
        ),
        (
            P1,
            "create_clan Tigers",
            SystemMessageId::YouHaveFailedToCreateAClan,
        ),
        (
            P3,
            "create_clan Tigers",
            SystemMessageId::YouMustWait10DaysBeforeCreatingANewClan,
        ),
        (P4, "create_clan", SystemMessageId::PleaseCreateYourClanName),
        (P4, "create_clan Big Cats", SystemMessageId::ClanNameIsInvalid),
        (P4, "create_clan Li_on", SystemMessageId::ClanNameIsInvalid),
        (P4, "create_clan L", SystemMessageId::ClanNameIsInvalid),
        (
            P4,
            "create_clan ABCDEFGHIJKLMNOPQ",
            SystemMessageId::ClanNameSLengthIsIncorrect,
        ),
        (P4, "create_clan lions", SystemMessageId::S1AlreadyExists),
    ];
    for (id, command, expected) in cases {
        s.vm_command(id, command);
        let out = s.take();
        assert_eq!(ids_to(&out, id), [expected], "{command}");
    }
    assert!(has_message(
        &{
            s.vm_command(P4, "create_clan LIONS");
            s.take()
        },
        P4,
        SystemMessageId::S1AlreadyExists,
        &[text("LIONS")]
    ));
    assert_eq!(s.world.clans().count(), 1);
}

#[test]
fn the_clan_name_template_is_checked_by_the_village_master() {
    let mut s = Scene::with(ClanSettings {
        clan_name_template: Arc::new(|name: &str| name.chars().all(|c| c.is_ascii_uppercase())),
        ..ClanSettings::default()
    });
    s.enter(pdata(P1, 10));
    s.vm_command(P1, "create_clan Lions");
    assert_eq!(ids_to(&s.take(), P1), [SystemMessageId::ClanNameIsInvalid]);
    s.vm_command(P1, "create_clan LIONS");
    assert_eq!(ids_to(&s.take(), P1), [SystemMessageId::YourClanHasBeenCreated]);
}

// ---------------------------------------------------------------------------------- joining

#[test]
fn an_invitation_answered_yes_joins_the_clan() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(pdata(P2, 20));
    s.take();
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P2,
            pledge_type: 0,
        },
    );
    let out = s.take();
    let ask = &decoded::<AskJoinPledge>(&out, P2, "ASK_JOIN_PLEDGE")[0];
    assert_eq!(
        (
            ask.requestor_object_id,
            ask.sub_pledge_name.as_deref(),
            ask.pledge_type,
            ask.pledge_name.as_str()
        ),
        (P1, None, 0, "Lions")
    );
    assert!(s.world.is_request_partner_busy(ObjectId(P1)) && s.world.is_request_partner_busy(ObjectId(P2)));

    s.clan_input(P2, ClanInput::AnswerJoinPledge { answer: 1 });
    let out = s.take();
    assert_eq!(decoded::<JoinPledge>(&out, P2, "JOIN_PLEDGE")[0].pledge_id, CLAN);
    assert!(ids_to(&out, P2).contains(&SystemMessageId::EnteredTheClan));
    for id in [P1, P2] {
        assert!(has_message(
            &out,
            id,
            SystemMessageId::S1HasJoinedTheClan,
            &[text("P2")]
        ));
        assert_eq!(
            decoded::<PledgeShowInfoUpdate>(&out, id, "PLEDGE_SHOW_INFO_UPDATE").len(),
            1
        );
    }
    let add = &decoded::<PledgeShowMemberListAdd>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_ADD")[0];
    assert_eq!(
        (add.name.as_str(), add.online_object_id, add.pledge_type),
        ("P2", P2, 0)
    );
    let all = &decoded::<PledgeShowMemberListAll>(&out, P2, "PLEDGE_SHOW_MEMBER_LIST_ALL")[0];
    assert_eq!(all.members.len(), 2);
    let state = s.state(P2);
    assert_eq!(
        (
            state.clan_id,
            state.power_grade,
            state.pledge_type,
            state.clan_join_expiry_time
        ),
        (CLAN, 5, 0, 0)
    );
    // `setClanPrivileges(clan.getRankPrivs(5))`: the rank's own object.
    assert_eq!(state.privileges, ClanPrivilegesRef::Rank(5));
    assert!(s.clan().is_member(P2));
    assert!(!s.world.is_request_partner_busy(ObjectId(P1)) && !s.world.is_request_partner_busy(ObjectId(P2)));
}

#[test]
fn an_academy_member_joins_at_grade_9() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(pdata(P2, 20));
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P2,
            pledge_type: -1,
        },
    );
    s.clan_input(P2, ClanInput::AnswerJoinPledge { answer: 1 });
    let state = s.state(P2);
    assert_eq!(
        (state.pledge_type, state.power_grade, state.lvl_joined_academy),
        (-1, 9, 20)
    );
    assert!(s.world.is_academy_member(ObjectId(P2)));
    // Level 5: an academy member is class 2 (level 5 does not look at the unit).
    assert_eq!(s.world.pledge_class_of(ObjectId(P2)), 2);
}

#[test]
fn an_invitation_answered_no_tells_both() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(pdata(P2, 20));
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P2,
            pledge_type: 0,
        },
    );
    s.take();
    s.clan_input(P2, ClanInput::AnswerJoinPledge { answer: 0 });
    let out = s.take();
    assert!(has_message(
        &out,
        P2,
        SystemMessageId::YouDidnTRespondToS1SInvitationJoiningHasBeenCancelled,
        &[text("P1")]
    ));
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1DidNotRespondInvitationToTheClanHasBeenCancelled,
        &[text("P2")]
    ));
    assert_eq!(s.world.clan_id_of(ObjectId(P2)), 0);
}

#[test]
fn a_request_waits_fifteen_seconds_and_blocks_others() {
    let mut s = Scene::new();
    let mut record = lions(5, vec![member(P1, 1, 0), member(P2, 5, 0)]);
    record.rank_privs = vec![(5, 0, ClanAccess::InviteMember.mask())];
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.enter(pdata(P3, 20));
    s.enter(pdata(P4, 20));
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P3,
            pledge_type: 0,
        },
    );
    s.take();
    // P1 waits for an answer; P3 is busy.
    s.clan_input(
        P1,
        ClanInput::JoinPledge {
            target: P4,
            pledge_type: 0,
        },
    );
    s.clan_input(
        P2,
        ClanInput::JoinPledge {
            target: P3,
            pledge_type: 0,
        },
    );
    let out = s.take();
    assert_eq!(ids_to(&out, P1), [SystemMessageId::WaitingForAnotherReply]);
    assert!(has_message(
        &out,
        P2,
        SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater,
        &[text("P3")]
    ));
    s.run_ms(14_950);
    assert!(s.world.is_request_partner_busy(ObjectId(P1)));
    s.run_ms(100);
    assert!(!s.world.is_request_partner_busy(ObjectId(P1)));
    assert!(!s.world.is_request_partner_busy(ObjectId(P3)));
    // A late answer finds no requestor.
    s.clan_input(P3, ClanInput::AnswerJoinPledge { answer: 1 });
    assert_eq!(s.world.clan_id_of(ObjectId(P3)), 0);
}

#[test]
fn the_join_conditions_follow_check_clan_join_condition() {
    let mut s = Scene::new();
    let mut members = vec![member(P1, 1, 0), member(P2, 5, 0)];
    // Level 0 holds 10 main members.
    for i in 0..8 {
        members.push(member(0x6000_0100 + i, 5, 0));
    }
    let mut record = lions(0, members);
    record.rank_privs = vec![(5, 0, 0)];
    let tigers = ClanRecord {
        clan_id: 600,
        data: Some(ClanDataRecord {
            name: Some("Tigers".to_owned()),
            level: 5,
            leader_id: P6,
            char_penalty_expiry_time: now_ms() + DAY_MS,
            ..ClanDataRecord::default()
        }),
        members: vec![member(P6, 1, 0)],
        ..ClanRecord::default()
    };
    s.load(vec![record, tigers]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.enter(pdata(P3, 50));
    let mut penalised = pdata(P4, 20);
    penalised.profile.clan = Some(PlayerClanState {
        clan_join_expiry_time: now_ms() + DAY_MS,
        ..PlayerClanState::default()
    });
    s.enter(penalised);
    s.enter(pdata(P5, 20));
    s.enter(member_data(P6, 600, 1, 0));
    s.take();
    let ask = |s: &mut Scene, from: i32, to: i32, pledge_type: i32| {
        s.clan_input(
            from,
            ClanInput::JoinPledge {
                target: to,
                pledge_type,
            },
        );
        s.take()
    };
    // P2 has no invite privilege.
    assert_eq!(
        ids_to(&ask(&mut s, P2, P5, 0), P2),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    assert_eq!(
        ids_to(&ask(&mut s, P1, P1, 0), P1),
        [SystemMessageId::YouCannotAskYourselfToApplyToAClan]
    );
    assert_eq!(
        ids_to(&ask(&mut s, P1, 0x6000_7777, 0), P1),
        [SystemMessageId::YouHaveInvitedTheWrongTarget]
    );
    assert!(has_message(
        &ask(&mut s, P1, P6, 0),
        P1,
        SystemMessageId::S1IsAlreadyAMemberOfAnotherClan,
        &[text("P6")]
    ));
    assert!(has_message(
        &ask(&mut s, P1, P4, 0),
        P1,
        SystemMessageId::C1CannotJoinTheClanBecauseOneDayHasNotYetPassedSinceTheyLeftAnotherClan,
        &[text("P4")]
    ));
    assert_eq!(
        ids_to(&ask(&mut s, P1, P3, -1), P1),
        [
            SystemMessageId::S1DoesNotMeetTheRequirementsToJoinAClanAcademy,
            SystemMessageId::ToJoinAClanAcademyCharactersMustBeLevel40OrBelowNotBelongAnotherClanAndNotYetCompletedTheir2ndClassTransfer
        ]
    );
    assert!(has_message(
        &ask(&mut s, P1, P5, 0),
        P1,
        SystemMessageId::S1IsFullAndCannotAcceptAdditionalClanMembersAtThisTime,
        &[text("Lions")]
    ));
    // Level 0 has no academy: its limit of 20 is not reached.
    assert!(decoded::<AskJoinPledge>(&ask(&mut s, P1, P5, -1), P5, "ASK_JOIN_PLEDGE").len() == 1);
    // A clan that dismissed somebody waits a day.
    s.enter(pdata(0x6000_0042, 20));
    assert_eq!(
        ids_to(&ask(&mut s, P6, 0x6000_0042, 0), P6),
        [SystemMessageId::AfterAClanMemberIsDismissedFromAClanTheClanMustWaitAtLeastADayBeforeAcceptingANewMember]
    );
}

// ---------------------------------------------------------------------------------- leaving

#[test]
fn a_member_withdraws_and_waits_a_day() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    s.outs.take();
    s.clan_input(P1, ClanInput::WithdrawalPledge);
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::AClanLeaderCannotWithdrawFromTheirOwnClan]
    );

    let before = now_ms();
    s.clan_input(P2, ClanInput::WithdrawalPledge);
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1HasWithdrawnFromTheClan,
        &[text("P2")]
    ));
    assert_eq!(
        decoded::<PledgeShowMemberListDelete>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_DELETE")[0].player_name,
        "P2"
    );
    let mine = ids_to(&out, P2);
    assert_eq!(
        mine,
        [
            SystemMessageId::YouHaveWithdrawnFromTheClan,
            SystemMessageId::AfterLeavingOrHavingBeenDismissedFromAClanYouMustWaitAtLeastADayBeforeJoiningAnotherClan
        ]
    );
    assert_eq!(
        decoded::<PledgeShowMemberListDeleteAll>(&out, P2, "PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL").len(),
        1
    );
    let state = s.state(P2);
    assert_eq!((state.clan_id, state.power_grade), (0, 0));
    assert!(
        state.clan_join_expiry_time >= before + DAY_MS && state.clan_join_expiry_time <= now_ms() + DAY_MS
    );
    assert!(!s.clan().is_member(P2));
    // `exMember.saveApprenticeAndSponsor(0, 0)`; the rest goes with P2's next store.
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::CharApprenticeSponsor {
            char_id: P2,
            apprentice: 0,
            sponsor: 0
        }]
    );
}

#[test]
fn an_officer_expels_members_online_and_offline() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    s.outs.take();
    s.clan_input(
        P2,
        ClanInput::OustPledgeMember {
            target: "P3".to_owned(),
        },
    );
    assert_eq!(
        ids_to(&s.take(), P2),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    s.clan_input(
        P1,
        ClanInput::OustPledgeMember {
            target: "p1".to_owned(),
        },
    );
    assert_eq!(ids_to(&s.take(), P1), [SystemMessageId::YouCannotDismissYourself]);

    // P3 is offline: its row is written.
    let before = now_ms();
    s.clan_input(
        P1,
        ClanInput::OustPledgeMember {
            target: "P3".to_owned(),
        },
    );
    let out = s.take();
    let writes = clan_writes(&s.outs.take());
    assert_eq!(writes.len(), 3, "{writes:?}");
    assert_eq!(
        writes[0],
        ClanWrite::CharApprenticeSponsor {
            char_id: P3,
            apprentice: 0,
            sponsor: 0
        }
    );
    let ClanWrite::RemoveMember {
        char_id,
        clan_join_expiry_time,
        clan_create_expiry_time,
    } = writes[1]
    else {
        panic!("{writes:?}");
    };
    assert_eq!((char_id, clan_create_expiry_time), (P3, 0));
    assert!(clan_join_expiry_time >= before + DAY_MS);
    let ClanWrite::Update(row) = &writes[2] else {
        panic!("{writes:?}");
    };
    assert!(row.char_penalty_expiry_time >= before + DAY_MS);
    for id in [P1, P2] {
        assert!(has_message(
            &out,
            id,
            SystemMessageId::ClanMemberS1HasBeenExpelled,
            &[text("P3")]
        ));
        assert_eq!(
            decoded::<PledgeShowMemberListDelete>(&out, id, "PLEDGE_SHOW_MEMBER_LIST_DELETE")[0].player_name,
            "P3"
        );
    }
    assert_eq!(
        ids_to(&out, P1)
            .into_iter()
            .filter(|id| *id != SystemMessageId::ClanMemberS1HasBeenExpelled)
            .collect::<Vec<_>>(),
        [
            SystemMessageId::YouHaveSucceededInExpellingTheClanMember,
            SystemMessageId::AfterAClanMemberIsDismissedFromAClanTheClanMustWaitAtLeastADayBeforeAcceptingANewMember
        ]
    );

    // P2 is online: no row now, the message, and the store carries the penalty.
    s.clan_input(
        P1,
        ClanInput::OustPledgeMember {
            target: "P2".to_owned(),
        },
    );
    let out = s.take();
    assert!(ids_to(&out, P2).contains(
        &SystemMessageId::YouHaveRecentlyBeenDismissedFromAClanYouAreNotAllowedToJoinAnotherClanFor24Hours
    ));
    assert!(s.state(P2).clan_join_expiry_time >= before + DAY_MS);
    assert!(
        !clan_writes(&s.outs.take())
            .iter()
            .any(|w| matches!(w, ClanWrite::RemoveMember { .. }))
    );
}

// ---------------------------------------------------------------------------------- ranks

#[test]
fn rank_privileges_reach_the_members_of_the_rank() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(5, 0x2)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    s.outs.take();
    s.clan_input(
        P1,
        ClanInput::PledgePower {
            rank: 5,
            action: 2,
            privs: 0x46,
        },
    );
    let out = s.take();
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::RankPrivs {
            clan_id: CLAN,
            rank: 5,
            privs: 0x46
        }]
    );
    assert_eq!(s.world.clan_privileges_of(ObjectId(P2)), 0x46);
    assert_eq!(
        decoded::<UserInfo>(&out, P2, "USER_INFO")[0].clan_privileges,
        0x46
    );
    // `broadcastClanStatus`: everybody's list again.
    for id in [P1, P2] {
        assert_eq!(
            decoded::<PledgeShowMemberListDeleteAll>(&out, id, "PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL").len(),
            1
        );
    }
    // The academy rank keeps the warehouse and the two doors only.
    s.clan_input(
        P1,
        ClanInput::PledgePower {
            rank: 9,
            action: 2,
            privs: -1,
        },
    );
    assert_eq!(s.clan().rank_privs(9), 8 | 2048 | 65536);
    // Not the leader: nothing.
    s.clan_input(
        P2,
        ClanInput::PledgePower {
            rank: 5,
            action: 2,
            privs: 0,
        },
    );
    assert_eq!(s.clan().rank_privs(5), 0x46);
    s.take();
    s.clan_input(
        P2,
        ClanInput::PledgePower {
            rank: 5,
            action: 1,
            privs: 0,
        },
    );
    s.clan_input(
        P2,
        ClanInput::PledgePower {
            rank: 5,
            action: 3,
            privs: 0,
        },
    );
    let out = s.take();
    // `ManagePledgePower` writes nothing for an action other than 1.
    assert_eq!(
        decoded::<ManagePledgePower>(&out, P2, "MANAGE_PLEDGE_POWER"),
        [ManagePledgePower { privileges: 0x46 }]
    );
    s.clan_input(P2, ClanInput::PledgePowerGradeList);
    let list = &decoded::<PledgePowerGradeList>(&s.take(), P2, "PLEDGE_POWER_GRADE_LIST")[0];
    let ranks: Vec<(i32, i32)> = list.privs.iter().map(|p| (p.rank, p.privileges)).collect();
    // NOTE(java-quirk): Java writes `getParty()` (0) where the privileges would be.
    assert_eq!(ranks, (1..10).map(|r| (r, 0)).collect::<Vec<_>>());
}

#[test]
fn power_grades_change_online_and_offline() {
    let mut s = Scene::new();
    let mut record = lions_of_three(5, 0x2);
    record.rank_privs.push((3, 0, 0x30));
    record.rank_privs.push((4, 0, 0x40));
    record.members.push(member(P4, 9, -1));
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.take();
    s.outs.take();
    s.clan_input(
        P1,
        ClanInput::PledgeSetMemberPowerGrade {
            member: "P2".to_owned(),
            power_grade: 3,
        },
    );
    let out = s.take();
    // "An online member keeps the privileges of the old rank until refreshed": a copy.
    assert_eq!(s.state(P2).privileges, ClanPrivilegesRef::Own(0x30));
    assert_eq!(s.state(P2).power_grade, 3);
    let info = &decoded::<PledgeReceivePowerInfo>(&out, P1, "PLEDGE_RECEIVE_POWER_INFO")[0];
    assert_eq!(
        (info.power_grade, info.name.as_str(), info.privileges),
        (3, "P2", 0x30)
    );
    assert!(clan_writes(&s.outs.take()).is_empty());

    s.clan_input(
        P1,
        ClanInput::PledgeSetMemberPowerGrade {
            member: "P3".to_owned(),
            power_grade: 4,
        },
    );
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::CharPowerGrade {
            char_id: P3,
            power_grade: 4
        }]
    );
    s.take();
    s.clan_input(
        P1,
        ClanInput::PledgeSetMemberPowerGrade {
            member: "P4".to_owned(),
            power_grade: 4,
        },
    );
    assert_eq!(
        texts_to(&s.take(), P1),
        ["You cannot change academy member grade"]
    );

    s.clan_input(
        P1,
        ClanInput::PledgeMemberInfo {
            player: "P3".to_owned(),
        },
    );
    let info = &decoded::<PledgeReceiveMemberInfo>(&s.take(), P1, "PLEDGE_RECEIVE_MEMBER_INFO")[0];
    assert_eq!(
        (
            info.pledge_type,
            info.name.as_str(),
            info.power_grade,
            info.pledge_name.as_str()
        ),
        (0, "P3", 4, "Lions")
    );
    s.clan_input(P1, ClanInput::PledgeInfo { clan_id: CLAN });
    let info = &decoded::<PledgeInfo>(&s.take(), P1, "PLEDGE_INFO")[0];
    assert_eq!(
        (info.clan_id, info.clan_name.as_str(), info.ally_name.as_str()),
        (CLAN, "Lions", "")
    );
}

#[test]
fn reorganising_swaps_the_units_of_two_members() {
    let mut s = Scene::new();
    let mut record = lions_of_three(6, 0);
    record.sub_pledges = vec![(100, "Royal".to_owned(), 0)];
    record.members[2].pledge_type = 100;
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.outs.take();
    s.clan_input(
        P1,
        ClanInput::PledgeReorganizeMember {
            is_member_selected: 1,
            member_name: "P2".to_owned(),
            new_pledge_type: 100,
            selected_member: "P3".to_owned(),
        },
    );
    assert_eq!(s.state(P2).pledge_type, 100);
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::CharSubPledge {
            char_id: P3,
            pledge_type: 0
        }]
    );
    assert_eq!(s.clan().member(ObjectId(P3)).unwrap().pledge_type, 0);
}

#[test]
fn sponsors_and_apprentices_are_linked_and_saved() {
    let mut s = Scene::new();
    let mut record = lions(5, vec![member(P1, 1, 0), member(P2, 9, -1), member(P3, 5, 0)]);
    record.sub_pledges = vec![(-1, "Cubs".to_owned(), 0)];
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 9, -1));
    s.take();
    s.outs.take();
    s.clan_input(
        P1,
        ClanInput::PledgeSetAcademyMaster {
            set: 1,
            curr_player_name: "P2".to_owned(),
            target_player_name: "P3".to_owned(),
        },
    );
    let out = s.take();
    assert_eq!(s.state(P2).sponsor, P3);
    assert_eq!(s.clan().member(ObjectId(P3)).unwrap().apprentice, P2);
    assert_eq!(
        clan_writes(&s.outs.take()),
        [
            ClanWrite::CharApprenticeSponsor {
                char_id: P2,
                apprentice: 0,
                sponsor: P3
            },
            ClanWrite::CharApprenticeSponsor {
                char_id: P3,
                apprentice: P2,
                sponsor: 0
            }
        ]
    );
    let sm = [text("P3"), text("P2")];
    // The sponsor is offline: the leader and the apprentice are told.
    for id in [P1, P2] {
        assert!(has_message(
            &out,
            id,
            SystemMessageId::S2HasBeenDesignatedAsTheApprenticeOfClanMemberS1,
            &sm
        ));
    }
    s.clan_input(
        P1,
        ClanInput::PledgeSetAcademyMaster {
            set: 1,
            curr_player_name: "P2".to_owned(),
            target_player_name: "P3".to_owned(),
        },
    );
    assert_eq!(texts_to(&s.take(), P1), ["Remove previous connections first."]);
    // The academy member's info names its sponsor.
    s.clan_input(
        P1,
        ClanInput::PledgeMemberInfo {
            player: "P2".to_owned(),
        },
    );
    let info = &decoded::<PledgeReceiveMemberInfo>(&s.take(), P1, "PLEDGE_RECEIVE_MEMBER_INFO")[0];
    assert_eq!(
        (
            info.pledge_name.as_str(),
            info.apprentice_or_sponsor_name.as_str()
        ),
        ("Cubs", "P3")
    );
}

// ---------------------------------------------------------------------------------- leader

#[test]
fn a_leader_change_gives_the_new_leaders_rank_every_privilege() {
    let mut s = Scene::with(ClanSettings {
        alt_clan_leader_instant_activation: true,
        ..ClanSettings::default()
    });
    s.load(vec![lions_of_three(5, 0x2)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.outs.take();
    s.vm_command(P1, "change_clan_leader P2");
    let out = s.take();
    assert_eq!(s.clan().leader, Some(ObjectId(P2)));
    assert!(s.world.is_clan_leader(ObjectId(P2)) && !s.world.is_clan_leader(ObjectId(P1)));
    // NOTE(java-quirk): `newLeader.getClanPrivileges().enableAll()` on the rank's own object: rank
    // 5 has every privilege now.
    assert_eq!(s.clan().rank_privs(5), 0x00FF_FFFF);
    assert_eq!(s.world.clan_privileges_of(ObjectId(P1)), 0);
    for id in [P1, P2] {
        assert!(has_message(
            &out,
            id,
            SystemMessageId::ClanLordPrivilegesHaveBeenTransferredToC1,
            &[text("P2")]
        ));
    }
    // Level 5: the new leader is class 4, the old one 2.
    assert_eq!(
        (
            s.world.pledge_class_of(ObjectId(P2)),
            s.world.pledge_class_of(ObjectId(P1))
        ),
        (4, 2)
    );
    let writes = clan_writes(&s.outs.take());
    let ClanWrite::Update(row) = &writes[0] else {
        panic!("{writes:?}");
    };
    assert_eq!(row.leader_id, P2);
}

#[test]
fn a_leader_change_waits_for_wednesday_without_instant_activation() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(5, 0x2)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    let mut academy = member_data(P3, CLAN, 9, -1);
    if let Some(clan) = academy.profile.clan.as_mut() {
        clan.lvl_joined_academy = 20;
    }
    s.enter(academy);
    s.outs.take();
    s.vm_command(P1, "change_clan_leader P2");
    let page = &decoded::<NpcHtmlMessage>(&s.take(), P1, "NPC_HTML_MESSAGE")[0];
    assert!(page.html.contains("9000-07-success"));
    assert_eq!(s.clan().new_leader_id, P2);
    assert!(matches!(&clan_writes(&s.outs.take())[..], [ClanWrite::Update(row)] if row.new_leader_id == P2));
    s.vm_command(P1, "change_clan_leader P2");
    assert!(
        decoded::<NpcHtmlMessage>(&s.take(), P1, "NPC_HTML_MESSAGE")[0]
            .html
            .contains("9000-07-in-progress")
    );
    // Without `ClanMaintenanceDatabaseWrites` the weekly job is the Java game server's.
    s.world.clan_leader_apply();
    assert_eq!(s.clan().leader, Some(ObjectId(P1)));
    s.vm_command(P1, "cancel_clan_leader_change");
    assert!(
        decoded::<NpcHtmlMessage>(&s.take(), P1, "NPC_HTML_MESSAGE")[0]
            .html
            .contains("9000-07-canceled")
    );
    assert_eq!(s.clan().new_leader_id, 0);
    s.vm_command(P1, "cancel_clan_leader_change");
    assert!(
        decoded::<NpcHtmlMessage>(&s.take(), P1, "NPC_HTML_MESSAGE")[0]
            .html
            .contains("You don't have clan leader delegation applications submitted yet!")
    );
    // The checks of the command.
    for (name, expected) in [
        ("Nobody", SystemMessageId::S1DoesNotExist),
        ("P4", SystemMessageId::S1DoesNotExist),
        (
            "P3",
            SystemMessageId::ThatPrivilegeCannotBeGrantedToAClanAcademyMember,
        ),
    ] {
        s.vm_command(P1, &format!("change_clan_leader {name}"));
        assert_eq!(ids_to(&s.take(), P1), [expected], "{name}");
    }
    s.vm_command(P2, "change_clan_leader P3");
    assert_eq!(
        ids_to(&s.take(), P2),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
}

#[test]
fn the_weekly_leader_change_runs_with_the_maintenance_switch() {
    let mut s = Scene::with(ClanSettings {
        maintenance_database_writes: true,
        ..ClanSettings::default()
    });
    let mut record = lions_of_three(5, 0x2);
    if let Some(data) = record.data.as_mut() {
        data.new_leader_id = P3;
    }
    s.load(vec![record]);
    s.outs.take();
    s.world.clan_leader_apply();
    assert_eq!(s.clan().leader, Some(ObjectId(P3)));
    assert_eq!(s.clan().new_leader_id, 0);
    // Both leaders are offline: their `clan_privs` rows.
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes[0],
        ClanWrite::CharClanPrivs {
            char_id: P1,
            privs: 0
        }
    );
    assert!(writes.contains(&ClanWrite::CharClanPrivs {
        char_id: P3,
        privs: 0x00FF_FFFF
    }));
}

// ---------------------------------------------------------------------------------- level

#[test]
fn level_one_costs_sp_and_adena() {
    let mut s = Scene::new();
    s.load(vec![lions(0, vec![member(P1, 1, 0)])]);
    let mut leader = member_data(P1, CLAN, 1, 0);
    leader.sp = 20_000;
    leader.items = vec![item(0x1000_0001, 57, 700_000)];
    s.enter(leader);
    s.outs.take();
    s.vm_command(P1, "increase_clan_level");
    let out = s.take();
    assert_eq!(s.clan().level, 1);
    assert_eq!(s.world.player(ObjectId(P1)).unwrap().data.sp, 0);
    assert_eq!(s.world.adena_of(ObjectId(P1)), 50_000);
    assert!(clan_writes(&s.outs.take()).contains(&ClanWrite::Level {
        clan_id: CLAN,
        level: 1
    }));
    let ids = ids_to(&out, P1);
    assert_eq!(
        ids,
        [
            SystemMessageId::S1AdenaDisappeared,
            SystemMessageId::YourSpHasDecreasedByS1,
            SystemMessageId::YourClanSLevelHasIncreased
        ]
    );
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::YourSpHasDecreasedByS1,
        &[SystemMessageParam::IntNumber(20_000)]
    ));
    assert_eq!(
        decoded::<PledgeShowInfoUpdate>(&out, P1, "PLEDGE_SHOW_INFO_UPDATE")[0].level,
        1
    );
    assert!(names_to(&out, P1).contains(&"MAGIC_SKILL_USE".to_owned()));
    // Level 2 needs 100,000 SP: not met.
    s.vm_command(P1, "increase_clan_level");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheConditionsNecessaryToIncreaseTheClanSLevelHaveNotBeenMet]
    );
}

#[test]
fn level_six_costs_reputation_and_needs_members() {
    let mut s = Scene::with(ClanSettings {
        clan_level_requirements: [3, 50, 80, 120, 140, 170],
        ..ClanSettings::default()
    });
    let mut record = lions_of_three(5, 0);
    if let Some(data) = record.data.as_mut() {
        data.reputation_score = 6000;
    }
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.outs.take();
    s.vm_command(P1, "increase_clan_level");
    let out = s.take();
    assert_eq!((s.clan().level, s.clan().reputation_score), (6, 1000));
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore,
        &[SystemMessageParam::IntNumber(5000)]
    ));
    assert!(
        ids_to(&out, P1)
            .contains(&SystemMessageId::NowThatYourClanLevelIsAboveLevel5ItCanAccumulateClanReputationPoints)
    );
    // NOTE(java-quirk): `changeLevel` does not compute the pledge classes again: the leader stays
    // class 4 (of level 5) until it logs in again.
    assert_eq!(s.world.pledge_class_of(ObjectId(P1)), 4);
    // The reputation is written with the next `updateClanInDB` (here: the shutdown save of a
    // changed clan).
    s.outs.take();
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| ()),
    });
    let writes = clan_writes(&s.outs.take());
    assert!(
        matches!(&writes[..], [ClanWrite::Update(row)] if row.reputation_score == 1000),
        "{writes:?}"
    );
}

#[test]
fn the_reputation_crossing_zero_is_announced() {
    let mut s = Scene::new();
    s.load(vec![lions(5, vec![member(P1, 1, 0)])]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.take();
    s.world.take_reputation_score(CLAN, 10);
    s.world.add_reputation_score(CLAN, 10);
    s.world.add_reputation_score(CLAN, 200_000_000);
    let out = s.take();
    assert_eq!(
        ids_to(&out, P1),
        [
            SystemMessageId::SinceTheClanReputationScoreHasDroppedTo0OrLowerYourClanSkillSWillBeDeActivated,
            SystemMessageId::ClanSkillsWillNowBeActivatedSinceTheClanSReputationScoreIs0OrHigher
        ]
    );
    let scores: Vec<i32> = decoded::<PledgeShowInfoUpdate>(&out, P1, "PLEDGE_SHOW_INFO_UPDATE")
        .iter()
        .map(|p| p.reputation_score)
        .collect();
    assert_eq!(scores, [-10, 0, 100_000_000]);
}

// ---------------------------------------------------------------------------------- sub pledges

#[test]
fn sub_pledges_are_created_renamed_and_led() {
    let mut s = Scene::new();
    let mut record = lions_of_three(5, 0);
    if let Some(data) = record.data.as_mut() {
        data.reputation_score = 20_000;
    }
    s.load(vec![record]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.outs.take();
    s.vm_command(P1, "create_royal Guards P2");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheConditionsNecessaryToCreateAMilitaryUnitHaveNotBeenMet]
    );
    s.vm_command(P1, "create_academy Cubs");
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::CongratulationsTheS1SClanAcademyHasBeenCreated,
        &[text("Lions")]
    ));
    let created = &decoded::<PledgeReceiveSubPledgeCreated>(&out, P2, "PLEDGE_RECEIVE_SUB_PLEDGE_CREATED")[0];
    assert_eq!(
        (created.id, created.name.as_str(), created.leader_name.as_str()),
        (-1, "Cubs", "")
    );
    assert_eq!(
        clan_writes(&s.outs.take()),
        [ClanWrite::InsertSubPledge {
            clan_id: CLAN,
            sub_pledge_id: -1,
            name: "Cubs".to_owned(),
            leader_id: 0
        }]
    );
    assert_eq!(s.clan().reputation_score, 20_000);
    s.vm_command(P1, "create_academy Cubs2");
    assert_eq!(
        texts_to(&s.take(), P1),
        ["You can't create any more sub-units of this type"]
    );

    s.world.change_clan_level(CLAN, 6);
    s.take();
    s.outs.take();
    s.vm_command(P1, "create_royal Guards P2");
    let out = s.take();
    assert!(has_message(
        &out,
        P1,
        SystemMessageId::TheRoyalGuardOfS1HaveBeenCreated,
        &[text("Lions")]
    ));
    assert_eq!(s.clan().reputation_score, 15_000);
    let writes = clan_writes(&s.outs.take());
    assert_eq!(
        writes[0],
        ClanWrite::InsertSubPledge {
            clan_id: CLAN,
            sub_pledge_id: 100,
            name: "Guards".to_owned(),
            leader_id: P2
        }
    );
    // Level 6: the captain of a royal guard is class 4.
    assert_eq!(s.world.pledge_class_of(ObjectId(P2)), 4);
    // The name is taken (any clan, any case); a captain cannot lead two units.
    s.vm_command(P1, "create_royal guards P3");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::AnotherMilitaryUnitIsAlreadyUsingThatNamePleaseEnterADifferentName]
    );
    s.vm_command(P1, "create_royal Second P2");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheCaptainOfTheRoyalGuardCannotBeAppointed]
    );
    s.vm_command(P1, "create_royal Second P3");
    assert!(s.clan().sub_pledge(200).is_some());
    s.vm_command(P1, "create_royal Third P3");
    s.take();
    s.vm_command(P1, "rename_pledge 100 Shields");
    assert_eq!(texts_to(&s.take(), P1), ["Pledge name changed."]);
    assert_eq!(s.clan().sub_pledge(100).unwrap().name, "Shields");
    s.vm_command(P1, "rename_pledge 1001 Swords");
    assert_eq!(texts_to(&s.take(), P1), ["Pledge don't exists."]);
    s.outs.take();
    // A new captain for the renamed unit: P2 leaves it first.
    if let Some(clan) = s.world.clans().find(|c| c.id == CLAN) {
        assert_eq!(clan.leader_sub_pledge(P2), 100);
    }
    s.vm_command(P1, "assign_subpl_leader Shields P2");
    assert_eq!(
        ids_to(&s.take(), P1),
        [SystemMessageId::TheCaptainOfTheRoyalGuardCannotBeAppointed]
    );
    s.vm_command(P1, "assign_subpl_leader Cubs P2");
    assert_eq!(ids_to(&s.take(), P1), [SystemMessageId::ClanNameIsInvalid]);
}

// ---------------------------------------------------------------------------------- dissolve

#[test]
fn a_dissolution_waits_and_leaves_the_rows_to_java_side_by_side() {
    let mut s = Scene::with(ClanSettings {
        alt_clan_dissolve_days: 0,
        ..ClanSettings::default()
    });
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.outs.take();
    s.vm_command(P2, "dissolve_clan");
    s.vm_command(P1, "dissolve_clan");
    let writes = clan_writes(&s.outs.take());
    assert!(
        matches!(&writes[..], [ClanWrite::Update(row)] if row.dissolving_expiry_time > 0),
        "{writes:?}"
    );
    s.vm_command(P1, "dissolve_clan");
    // NOTE(java-quirk): the time is `now + 0 days`, so it is not in the future any more and the
    // request can be repeated; the first timer decides.
    s.take();
    s.run_ms(299_000);
    assert!(s.world.clan(CLAN).is_some());
    s.take();
    s.run_ms(2_000);
    assert!(s.world.clan(CLAN).is_none());
    let out = s.take();
    assert!(ids_to(&out, P1).contains(&SystemMessageId::ClanHasDispersed));
    assert_eq!(
        decoded::<PledgeShowMemberListDeleteAll>(&out, P1, "PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL").len(),
        1
    );
    assert_eq!(s.world.clan_id_of(ObjectId(P1)), 0);
    // The leader waits 10 days for a new clan.
    assert!(s.state(P1).clan_create_expiry_time > now_ms() + 9 * DAY_MS);
    // D-49: no `DELETE`s and no member rows from this server.
    let writes = clan_writes(&s.outs.take());
    assert!(
        !writes
            .iter()
            .any(|w| matches!(w, ClanWrite::Delete { .. } | ClanWrite::RemoveMember { .. })),
        "{writes:?}"
    );
}

#[test]
fn a_dissolution_deletes_the_rows_with_the_maintenance_switch() {
    let mut s = Scene::with(ClanSettings {
        alt_clan_dissolve_days: 0,
        maintenance_database_writes: true,
        ..ClanSettings::default()
    });
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.vm_command(P1, "dissolve_clan");
    s.outs.take();
    s.run_ms(301_000);
    let writes = clan_writes(&s.outs.take());
    let removed: BTreeSet<i32> = writes
        .iter()
        .filter_map(|w| match w {
            ClanWrite::RemoveMember { char_id, .. } => Some(*char_id),
            _ => None,
        })
        .collect();
    // P1 is online: its row goes with its store.
    assert_eq!(removed, [P2, P3].into_iter().collect());
    assert_eq!(
        writes.last(),
        Some(&ClanWrite::Delete {
            clan_id: CLAN,
            castle_id: 0
        })
    );
}

#[test]
fn a_recovered_clan_is_not_destroyed() {
    let mut s = Scene::with(ClanSettings {
        alt_clan_dissolve_days: 0,
        ..ClanSettings::default()
    });
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.vm_command(P1, "dissolve_clan");
    s.vm_command(P1, "recover_clan");
    assert_eq!(s.clan().dissolving_expiry_time, 0);
    s.run_ms(301_000);
    assert!(s.world.clan(CLAN).is_some());
    // A clan at war or in an alliance cannot dissolve.
    s.world.clan(CLAN).unwrap();
    s.take();
}

// ---------------------------------------------------------------------------------- targets

fn skill(id: i32) -> l2_data::SkillTemplate {
    stat_data().skills.get(id, 1).unwrap().clone()
}

fn targets(s: &mut Scene, skill_id: i32, caster: i32, only_first: bool) -> Vec<i32> {
    s.world
        .skill_target_list(
            &skill(skill_id),
            ObjectId(caster),
            only_first,
            None,
            &TargetContext::default(),
        )
        .targets
        .iter()
        .map(|t| t.0)
        .collect()
}

#[test]
fn clan_targets_are_the_online_members_in_range() {
    let mut s = Scene::new();
    let mut record = lions_of_three(5, 0);
    record.members.push(member(P5, 5, 0));
    s.load(vec![record]);
    // P4 is not a member; P3 is far away; P5 is dead.
    for id in [P1, P2, P3, P5] {
        let mut data = member_data(id, CLAN, 5, 0);
        if id == P3 {
            data.x = X + 5000;
        }
        s.enter(data);
    }
    s.enter(pdata(P4, 40));
    s.world.set_dead(ObjectId(P5), true);

    // CLAN: the caster, then the members in map order.
    assert_eq!(targets(&mut s, 1, P1, false), [P1, P2]);
    assert_eq!(targets(&mut s, 1, P1, true), [P1]);
    assert_eq!(targets(&mut s, 1, P4, false), [P4]);
    // PARTY_CLAN: the caster and the clan members around (region order).
    assert_eq!(targets(&mut s, 2, P1, false), [P1, P2]);
    assert_eq!(targets(&mut s, 2, P4, false), [P4]);
    // CORPSE_CLAN: the dead members only, not the caster; nothing without a clan.
    assert_eq!(targets(&mut s, 3, P1, false), [P5]);
    assert!(targets(&mut s, 3, P4, false).is_empty());

    // checkPvpSkill: a debuff on a clan mate needs ctrl, the target and a damage skill.
    let debuff = skill(4);
    let ctx = TargetContext::default();
    assert!(!s.world.check_pvp_skill(ObjectId(P1), ObjectId(P2), &debuff, &ctx));
    // An area hit never takes a clan mate.
    assert!(
        !s.world
            .check_for_area_offensive_skills(ObjectId(P1), ObjectId(P2), &debuff, false, &ctx)
    );
}

#[test]
fn clan_conditions_read_the_clan() {
    let mut s = Scene::new();
    s.load(vec![lions_of_three(5, 0)]);
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.enter(pdata(P3, 40));
    let key = |id| SkillKey { id, level: 1 };
    let check = |s: &mut Scene, player: i32, skill: i32| {
        s.world
            .check_skill_condition(ObjectId(player), ObjectId(player), key(skill))
    };
    // isClanLeader
    assert!(check(&mut s, P1, 100));
    assert!(!check(&mut s, P2, 100));
    assert!(!check(&mut s, P3, 100));
    // pledgeClass 3: the leader always, a level 5 member (class 2) not, no clan not.
    assert!(check(&mut s, P1, 101));
    assert!(!check(&mut s, P2, 101));
    assert!(!check(&mut s, P3, 101));
    // castle 0: the clan's castle is 0 (`hasCastle`); without a clan `0` holds too.
    assert!(check(&mut s, P1, 102));
    assert!(check(&mut s, P3, 102));
}

// ---------------------------------------------------------------------------------- PvP

fn tigers_at_war(p_tiger: i32) -> ClanRecord {
    ClanRecord {
        clan_id: 600,
        data: Some(ClanDataRecord {
            name: Some("Tigers".to_owned()),
            level: 5,
            leader_id: p_tiger,
            ..ClanDataRecord::default()
        }),
        members: vec![member(p_tiger, 1, 0)],
        ..ClanRecord::default()
    }
}

#[test]
fn relations_show_the_clan_the_leader_and_the_wars() {
    use l2_protocol_game::server::visibility::relation;
    let mut s = Scene::new();
    s.world.load_clans(
        vec![lions_of_three(5, 0), tigers_at_war(P4)],
        vec![("600".to_owned(), "500".to_owned())],
    );
    s.enter(member_data(P1, CLAN, 1, 0));
    s.enter(member_data(P2, CLAN, 5, 0));
    s.enter(member_data(P4, 600, 1, 0));
    s.enter(pdata(P5, 40));
    let (p1, p2, p4, p5) = (ObjectId(P1), ObjectId(P2), ObjectId(P4), ObjectId(P5));
    // `getRelation(target)`: the subject's clan bits, and the viewer's war on the subject's clan.
    assert_eq!(
        s.world.relation(p1, p2),
        relation::CLAN_MEMBER | relation::CLAN_MATE | relation::LEADER
    );
    assert_eq!(s.world.relation(p2, p5), relation::CLAN_MEMBER);
    // The Tigers declared war on the Lions (one way): a Tiger sees the Lions at war.
    assert_eq!(
        s.world.relation(p2, p4),
        relation::CLAN_MEMBER | relation::ONE_SIDED_WAR
    );
    assert_eq!(s.world.relation(p4, p2), relation::CLAN_MEMBER | relation::LEADER);
    assert_eq!(s.world.relation(p5, p1), 0);
    // A one-sided war makes nobody auto attackable; a clan mate never is.
    assert!(!s.world.is_auto_attackable(p2, p4));
    assert!(!s.world.is_auto_attackable(p2, p1));
}

#[test]
fn a_war_both_ways_makes_members_auto_attackable_and_mutual() {
    use l2_protocol_game::server::visibility::relation;
    let mut s = Scene::new();
    s.world.load_clans(
        vec![lions_of_three(5, 0), tigers_at_war(P4)],
        vec![
            ("600".to_owned(), "500".to_owned()),
            ("500".to_owned(), "600".to_owned()),
        ],
    );
    s.enter(member_data(P2, CLAN, 5, 0));
    s.enter(member_data(P4, 600, 1, 0));
    let (p2, p4) = (ObjectId(P2), ObjectId(P4));
    assert_eq!(
        s.world.relation(p2, p4),
        relation::CLAN_MEMBER | relation::ONE_SIDED_WAR | relation::MUTUAL_WAR
    );
    assert!(s.world.is_auto_attackable(p2, p4));
    assert!(s.world.is_auto_attackable(p4, p2));
    // `getWantsPeace()` of either ends it.
    let mut peaceful = member_data(P3, CLAN, 5, 0);
    if let Some(clan) = peaceful.profile.clan.as_mut() {
        clan.wants_peace = 1;
    }
    s.enter(peaceful);
    assert!(!s.world.is_auto_attackable(ObjectId(P3), p4));
}
