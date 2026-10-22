#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The post box (M6-MAIL-01): `RequestSendPost` and its checks, the fee, the attachments, reading,
//! deleting, collecting (with a payment request), rejecting, cancelling, the expiry task and the
//! notice at login. The expected values come from the Java code named in each test.
//! Java: network/clientpackets/{RequestSendPost,RequestReceivedPost,RequestPostAttachment,
//! RequestRejectPostAttachment,RequestCancelPostAttachment,RequestDeleteReceivedPost,
//! RequestDeleteSentPost}.java, managers/MailManager.java, network/holders/MailMessage.java,
//! taskmanagers/MessageDeletionTaskManager.java, entity/itemcontainer/Mail.java.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{PostAttachment, RequestSendPost};
use l2_protocol_game::server::{
    ExChangePostState, ExNoticePostArrived, ExReplyReceivedPost, ExShowReceivedPostList, InventoryUpdate,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

// ---- the datapack -------------------------------------------------------------------------------

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

const SWORD: i32 = 2;
const BOUND: i32 = 3;
const CUIRASS: i32 = 4;
const ORE: i32 = 10;
const ADENA: i32 = 57;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /></item>
<item id="3" type="EtcItem" name="Bound"><set name="is_tradable" val="false" /><set name="weight" val="1" /></item>
<item id="4" type="Armor" name="B Cuirass"><set name="crystal_type" val="B" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /></item>
<item id="10" type="EtcItem" name="Ore"><set name="etcitem_type" val="MATERIAL" /><set name="material" val="SILVER" /><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const A: ObjectId = ObjectId(0x6000_0001);
const B: ObjectId = ObjectId(0x6000_0002);
/// A character who is not online.
const C: ObjectId = ObjectId(0x6000_0003);
/// A GM who is not online.
const GM: ObjectId = ObjectId(0x6000_0004);
const RUST_IDS: i32 = 0x6000_0000;

fn item(object_id: i32, item_id: i32, count: i64, loc: &str) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: loc.into(),
        ..Default::default()
    }
}

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    item(object_id, item_id, count, "INVENTORY")
}

fn player(id: ObjectId, name: &str, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 50.0,
        max_hp: 100.0,
        cur_mp: 30.0,
        max_mp: 50.0,
        cur_cp: 0.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn alice_items() -> Vec<ItemData> {
    vec![
        inv(1001, ADENA, 100_000),
        inv(1002, SWORD, 1),
        inv(1003, BOUND, 1),
        inv(1004, ORE, 50),
    ]
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

/// A peace zone around the scene (the mail with attachments needs one).
fn zone_files(root: &std::path::Path, zone_type: &str) {
    std::fs::create_dir_all(root.join("data/zones")).unwrap();
    std::fs::create_dir_all(root.join("data/mapregion")).unwrap();
    std::fs::write(
        root.join("data/zones/test.xml"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\
             <zone id=\"1\" name=\"z1\" type=\"{zone_type}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
             <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone></list>\n",
            X - 500,
            Y - 500,
            X + 500,
            Y + 500
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("data/mapregion/talking_island_town.xml"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
         <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
         <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
         </region>\n</list>\n",
    )
    .unwrap();
}

impl Scene {
    /// Everybody stands in a peace zone.
    fn new() -> Self {
        Self::with_zone("PeaceZone")
    }

    fn with_zone(zone_type: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        zone_files(root.path(), zone_type);
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        // The flood protector gets its own test; here a player sends mail after mail.
        world.set_mail_settings(MailSettings {
            send_interval_ticks: 0,
            ..MailSettings::default()
        });
        world.load_mail(MailLoad {
            messages: vec![],
            characters: vec![
                MailCharacter {
                    id: C.0,
                    name: "Carol".into(),
                    access_level: 0,
                },
                MailCharacter {
                    id: GM.0,
                    name: "Gandalf".into(),
                    access_level: 100,
                },
            ],
        });
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        // The zones are entered a tick later.
        self.world.tick();
    }

    fn mail(&mut self, who: ObjectId, input: MailInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input: ClientInput::Mail(input),
        });
    }

    fn send(&mut self, who: ObjectId, receiver: &str, items: &[(i32, i64)], cod: Option<i64>) {
        self.mail(
            who,
            MailInput::Send(RequestSendPost {
                receiver: receiver.into(),
                is_cod: cod.is_some(),
                subject: "Subject".into(),
                text: "Text".into(),
                items: (!items.is_empty()).then(|| {
                    items
                        .iter()
                        .map(|(object_id, count)| PostAttachment {
                            object_id: *object_id,
                            count: *count,
                        })
                        .collect()
                }),
                req_adena: cod.unwrap_or(0),
            }),
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// Alice sends the mail the tests then work with, and returns its id.
    fn alice_sends(&mut self, items: &[(i32, i64)], cod: Option<i64>) -> i32 {
        self.send(A, "Bob", items, cod);
        let out = self.log.take();
        mail_writes(&out)
            .into_iter()
            .find_map(|w| match w {
                MailWrite::Insert(m) => Some(m.id),
                _ => None,
            })
            .expect("the mail was stored")
    }

    fn inventory_of(&self, who: ObjectId) -> Vec<(i32, i32, i64)> {
        let mut items: Vec<(i32, i32, i64)> = self
            .world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count))
            .collect();
        items.sort_unstable();
        items
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

fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn of<'a>(out: &'a [Out], to: ObjectId, name: &str) -> Vec<&'a [u8]> {
    packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == name)
        .collect()
}

fn messages(out: &[Out], to: ObjectId) -> Vec<SystemMessageId> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw).id)
        .collect()
}

fn mail_writes(out: &[Out]) -> Vec<MailWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Mail(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

/// One op as `(statement, owner, object id, item id, count, loc, loc data)`.
type Row = (&'static str, i32, i32, i32, i64, String, i32);

fn rows(out: &[Out]) -> Vec<Row> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemRows { ops, .. }) => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .map(|op| match op {
            ItemRowOp::Write { owner_id, item } => (
                "write",
                owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc,
                item.loc_data,
            ),
            ItemRowOp::Update { owner_id, item } => (
                "update",
                owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc,
                item.loc_data,
            ),
            ItemRowOp::Insert { owner_id, item } => (
                "insert",
                owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc,
                item.loc_data,
            ),
            ItemRowOp::Delete { object_id } => ("delete", 0, object_id.0, 0, 0, String::new(), 0),
        })
        .collect()
}

fn attachments(s: &Scene, msg_id: i32) -> Vec<(i32, i64)> {
    s.world
        .mail_message(msg_id)
        .unwrap()
        .attachments
        .iter()
        .map(|i| (i.item_id, i.count))
        .collect()
}

// ---- sending ------------------------------------------------------------------------------------

/// `RequestSendPost.removeItems`: 100 adena and 1000 per item, a whole item moves into the mail and
/// a stack is split; the rows go out before the message; the receiver is told.
#[test]
fn a_mail_with_attachments_costs_the_fee_and_moves_the_items() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![inv(2001, ADENA, 10)]));
    s.log.take();

    let before = unix_ms();
    s.send(A, "Bob", &[(1002, 1), (1004, 20)], Some(5_000));
    let after = unix_ms();
    let out = s.log.take();

    let writes = mail_writes(&out);
    let [MailWrite::Insert(mail)] = writes.as_slice() else {
        panic!("one insert, got {writes:?}");
    };
    assert_eq!((mail.sender_id, mail.receiver_id), (A.0, B.0));
    assert_eq!(
        (mail.subject.as_str(), mail.content.as_str()),
        ("Subject", "Text")
    );
    assert_eq!(mail.req_adena, 5_000);
    assert!(mail.has_attachments && mail.unread && !mail.returned);
    assert_eq!((mail.deleted_by_sender, mail.deleted_by_receiver), (false, false));
    assert!(mail.id >= RUST_IDS);
    // A payment request lives 12 hours.
    assert!(mail.expiration >= before + 12 * 3_600_000 && mail.expiration <= after + 12 * 3_600_000);

    // The sword moved whole, the ore was split: the mail holds a new stack of 20 (a new id).
    let rows = rows(&out);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(rows[0], ("write", A.0, 1002, SWORD, 1, "MAIL".into(), mail.id));
    assert_eq!(rows[1], ("write", A.0, 1004, ORE, 30, "INVENTORY".into(), 0));
    assert_eq!(
        (rows[2].0, rows[2].1, rows[2].3, rows[2].4),
        ("insert", A.0, ORE, 20)
    );
    assert_eq!((rows[2].5.as_str(), rows[2].6), ("MAIL", mail.id));
    // The rows are written before the message (Java updates the items in `Mail.addItem`).
    let first_rows = out
        .iter()
        .position(|o| matches!(o, Out::Persist(PersistCommand::ItemRows { .. })))
        .unwrap();
    let insert = out
        .iter()
        .position(|o| matches!(o, Out::Persist(PersistCommand::Mail(_))))
        .unwrap();
    assert!(first_rows < insert);

    // Adena: 100 + 2 * 1000.
    assert_eq!(s.world.adena_of(A), 100_000 - 2_100);
    assert_eq!(
        s.inventory_of(A),
        vec![(1001, ADENA, 97_900), (1003, BOUND, 1), (1004, ORE, 30)]
    );
    let mut held = attachments(&s, mail.id);
    held.sort_unstable();
    assert_eq!(held, vec![(SWORD, 1), (ORE, 20)]);

    // The sender got the notice and the message; the receiver the envelope.
    assert_eq!(
        messages(&out, A).last(),
        Some(&SystemMessageId::MailSuccessfullySent)
    );
    assert_eq!(of(&out, A, "EX_NOTICE_POST_SENT").len(), 1);
    let notice: ExNoticePostArrived = decode(of(&out, B, "EX_NOTICE_POST_ARRIVED")[0]);
    assert!(notice.show_animation);
    assert_eq!(
        (s.world.mail_inbox_size(B.0), s.world.mail_outbox_size(A.0)),
        (1, 1)
    );
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[test]
fn a_plain_mail_costs_100_adena_and_lives_15_days() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    let before = unix_ms();
    s.send(A, "carol", &[], None);
    let out = s.log.take();
    let writes = mail_writes(&out);
    let [MailWrite::Insert(mail)] = writes.as_slice() else {
        panic!("one insert");
    };
    assert_eq!(mail.receiver_id, C.0, "the name is matched ignoring the case");
    assert!(!mail.has_attachments);
    assert!(mail.expiration >= before + 360 * 3_600_000);
    assert_eq!(s.world.adena_of(A), 99_900);
    assert!(rows(&out).is_empty());
}

#[test]
fn attachments_need_a_peace_zone_but_plain_mail_does_not() {
    let mut s = Scene::with_zone("MotherTree");
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.send(A, "Carol", &[(1002, 1)], None);
    let out = s.log.take();
    assert_eq!(
        messages(&out, A),
        [SystemMessageId::YouCannotForwardInANonPeaceZoneLocation]
    );
    assert!(mail_writes(&out).is_empty());
    assert_eq!(s.world.adena_of(A), 100_000);

    s.send(A, "Carol", &[], None);
    let out = s.log.take();
    assert_eq!(mail_writes(&out).len(), 1);
}

/// The checks of `runImpl`, in Java's order, each with its message and without any change.
#[test]
fn the_checks_of_request_send_post() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();

    let expect = |s: &mut Scene, receiver: &str, items: &[(i32, i64)], cod: Option<i64>, message| {
        s.send(A, receiver, items, cod);
        let out = s.log.take();
        assert_eq!(messages(&out, A), [message], "{receiver} {items:?} {cod:?}");
        assert!(mail_writes(&out).is_empty());
        assert!(rows(&out).is_empty());
        assert_eq!(s.world.adena_of(A), 100_000, "no fee");
    };

    // A payment request needs an amount and an item.
    expect(
        &mut s,
        "Bob",
        &[(1002, 1)],
        Some(0),
        SystemMessageId::WhenNotEnteringTheAmountForThePaymentRequestYouCannotSendAnyMail,
    );
    expect(
        &mut s,
        "Bob",
        &[],
        Some(100),
        SystemMessageId::ItSAPaymentRequestTransactionPleaseAttachTheItem,
    );
    expect(
        &mut s,
        "Nobody",
        &[],
        None,
        SystemMessageId::WhenTheRecipientDoesnTExistOrTheCharacterHasBeenDeletedSendingMailIsNotPossible,
    );
    expect(
        &mut s,
        "alice",
        &[],
        None,
        SystemMessageId::YouCannotSendAMailToYourself,
    );
    // Items that cannot be sent: not tradeable, not owned.
    expect(
        &mut s,
        "Bob",
        &[(1003, 1)],
        None,
        SystemMessageId::TheItemThatYouReTryingToSendCannotBeForwardedBecauseItIsnTProper,
    );
    expect(
        &mut s,
        "Bob",
        &[(9999, 1)],
        None,
        SystemMessageId::TheItemThatYouReTryingToSendCannotBeForwardedBecauseItIsnTProper,
    );
    // More than the stack holds.
    expect(
        &mut s,
        "Bob",
        &[(1004, 51)],
        None,
        SystemMessageId::TheItemThatYouReTryingToSendCannotBeForwardedBecauseItIsnTProper,
    );

    // A recipient name above 16 characters, a title above 128, a text above 512.
    s.send(A, "ABCDEFGHIJKLMNOPQ", &[], None);
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::TheAllowedLengthForRecipientExceeded]
    );
    s.mail(
        A,
        MailInput::Send(RequestSendPost {
            receiver: "Bob".into(),
            is_cod: false,
            subject: "s".repeat(129),
            text: String::new(),
            items: None,
            req_adena: 0,
        }),
    );
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::TheAllowedLengthForATitleExceeded]
    );
    s.mail(
        A,
        MailInput::Send(RequestSendPost {
            receiver: "Bob".into(),
            is_cod: false,
            subject: String::new(),
            text: "t".repeat(513),
            items: None,
            req_adena: 0,
        }),
    );
    // NOTE(java-quirk): there is no message for a long text, Java reuses the title one.
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::TheAllowedLengthForATitleExceeded]
    );

    // More than 8 attachments.
    let nine: Vec<(i32, i64)> = (0..9).map(|_| (1004, 1)).collect();
    s.send(A, "Bob", &nine, None);
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::ItemSelectionIsPossibleUpTo8]
    );
}

#[test]
fn the_fee_needs_the_adena_that_is_not_attached() {
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Alice",
        vec![inv(1001, ADENA, 1_500), inv(1002, SWORD, 1)],
    ));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    // 1500 adena: the fee of one item is 1100, but 1000 of the adena go into the mail.
    s.send(A, "Bob", &[(1001, 1_000)], None);
    let out = s.log.take();
    assert_eq!(
        messages(&out, A),
        [SystemMessageId::YouCannotForwardBecauseYouDonTHaveEnoughAdena]
    );
    assert!(mail_writes(&out).is_empty());
    assert_eq!(s.world.adena_of(A), 1_500);
    // 500 attached leaves 1000, which is enough for the 1100? No: 1500 - 500 = 1000 < 1100.
    s.send(A, "Bob", &[(1001, 500)], None);
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::YouCannotForwardBecauseYouDonTHaveEnoughAdena]
    );
    // 400 attached leaves 1100: exactly the fee.
    s.send(A, "Bob", &[(1001, 400)], None);
    let out = s.log.take();
    assert_eq!(mail_writes(&out).len(), 1);
    assert_eq!(s.world.adena_of(A), 0);
}

#[test]
fn nobody_writes_to_the_gm_staff_but_a_gm() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.send(A, "Gandalf", &[], None);
    let out = s.log.take();
    let raw = of(&out, A, "SYSTEM_MESSAGE");
    let message: SystemMessage = decode(raw[0]);
    assert_eq!(
        message.id,
        SystemMessageId::YourMessageToC1DidNotReachItsRecipientYouCannotSendMailToTheGmStaff
    );
    assert_eq!(message.params, vec![SystemMessageParam::Text("Gandalf".into())]);
    assert!(mail_writes(&out).is_empty());

    // A GM may.
    let mut gm = player(B, "Bob", vec![inv(2001, ADENA, 1_000)]);
    gm.access_level = 100;
    s.enter(gm);
    s.log.take();
    s.send(B, "Gandalf", &[], None);
    assert_eq!(mail_writes(&s.log.take()).len(), 1);
}

#[test]
fn the_boxes_hold_240_mails() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    let now = unix_ms();
    // Bob's inbox is full (from somebody else), Alice's outbox is full (to somebody else).
    let from_c = |i: i32| MailMessage::new_player_mail(i, C.0, B.0, false, "s", "t", 0, now);
    let to_c = |i: i32| MailMessage::new_player_mail(i, A.0, C.0, false, "s", "t", 0, now);
    s.world.load_mail(MailLoad {
        messages: (1..=240).map(from_c).collect(),
        ..MailLoad::default()
    });
    s.log.take();
    s.send(A, "Bob", &[], None);
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::TheMailLimit240HasBeenExceededAndThisCannotBeForwarded]
    );
    s.world.load_mail(MailLoad {
        messages: (1001..=1240).map(to_c).collect(),
        ..MailLoad::default()
    });
    s.send(A, "Carol", &[], None);
    assert_eq!(
        messages(&s.log.take(), A),
        [SystemMessageId::TheMailLimit240HasBeenExceededAndThisCannotBeForwarded]
    );
}

/// `FloodProtectorSendMailInterval` (100 ticks of 100 ms): the second mail within 10 seconds is
/// refused, a GM is not limited.
#[test]
fn the_second_mail_within_the_interval_is_refused() {
    let mut s = Scene::new();
    s.world.set_mail_settings(MailSettings::default());
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.send(A, "Carol", &[], None);
    assert_eq!(mail_writes(&s.log.take()).len(), 1);
    s.send(A, "Carol", &[], None);
    let out = s.log.take();
    assert_eq!(
        messages(&out, A),
        [SystemMessageId::ThePreviousMailWasForwardedLessThan1MinuteAgoAndThisCannotBeForwarded]
    );
    assert!(mail_writes(&out).is_empty());
    assert_eq!(s.world.adena_of(A), 99_900, "only the first fee");
    s.advance(10_100);
    s.log.take();
    s.send(A, "Carol", &[], None);
    assert_eq!(mail_writes(&s.log.take()).len(), 1);
}

#[test]
fn mail_can_be_turned_off_and_attachments_too() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.world.set_mail_settings(MailSettings {
        allow_mail: false,
        ..MailSettings::default()
    });
    s.send(A, "Carol", &[], None);
    assert!(s.log.take().is_empty());

    s.world.set_mail_settings(MailSettings {
        allow_attachments: false,
        ..MailSettings::default()
    });
    // The items, the payment request and the amount are dropped: a plain mail.
    s.send(A, "Carol", &[(1002, 1)], Some(500));
    let out = s.log.take();
    let writes = mail_writes(&out);
    let [MailWrite::Insert(mail)] = writes.as_slice() else {
        panic!("one insert");
    };
    assert!(!mail.has_attachments);
    assert_eq!(mail.req_adena, 0);
    assert_eq!(s.world.adena_of(A), 99_900);
}

// ---- reading and the lists ----------------------------------------------------------------------

#[test]
fn the_receiver_reads_the_mail_once_and_it_is_marked() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], None);

    s.mail(B, MailInput::ReceivedPostList);
    let out = s.log.take();
    let list: ExShowReceivedPostList = decode(of(&out, B, "EX_SHOW_RECEIVED_POST_LIST")[0]);
    assert_eq!(list.mails.len(), 1);
    let entry = &list.mails[0];
    assert_eq!(
        (entry.msg_id, entry.sender_name.as_str(), entry.subject.as_str()),
        (id, "Alice", "Subject")
    );
    assert!(entry.unread && entry.has_attachments && !entry.locked && !entry.returned);

    s.mail(B, MailInput::ReceivedPost { msg_id: id });
    let out = s.log.take();
    let reply: ExReplyReceivedPost = decode(of(&out, B, "EX_REPLY_RECEIVED_POST")[0]);
    assert_eq!((reply.msg_id, reply.content.as_str()), (id, "Text"));
    assert_eq!(reply.items.len(), 1);
    assert_eq!(reply.items[0].item_id, SWORD);
    assert!(reply.has_attachments);
    let state: ExChangePostState = decode(of(&out, B, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!(
        (state.received_board, state.msg_ids, state.change_id),
        (true, vec![id], 1)
    );
    assert_eq!(mail_writes(&out), [MailWrite::MarkRead(id)]);
    assert!(!s.world.mail_has_unread_post(B));

    // Again: the packets again, the row not again.
    s.mail(B, MailInput::ReceivedPost { msg_id: id });
    let out = s.log.take();
    assert_eq!(of(&out, B, "EX_REPLY_RECEIVED_POST").len(), 1);
    assert!(mail_writes(&out).is_empty());
}

#[test]
fn only_the_receiver_opens_a_received_mail() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[], None);
    // Alice is the sender, not the receiver: an illegal action, nothing is sent.
    s.mail(A, MailInput::ReceivedPost { msg_id: id });
    let out = s.log.take();
    assert!(of(&out, A, "EX_REPLY_RECEIVED_POST").is_empty());
    assert!(mail_writes(&out).is_empty());
    assert!(s.world.mail_has_unread_post(B));
}

#[test]
fn the_sender_reads_the_sent_mail() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.send(A, "Carol", &[(1002, 1)], None);
    let out = s.log.take();
    let id = mail_writes(&out)
        .iter()
        .find_map(|w| match w {
            MailWrite::Insert(m) => Some(m.id),
            _ => None,
        })
        .unwrap();
    s.mail(A, MailInput::SentPostList);
    let out = s.log.take();
    assert_eq!(of(&out, A, "EX_SHOW_SENT_POST_LIST").len(), 1);
    s.mail(A, MailInput::SentPost { msg_id: id });
    let out = s.log.take();
    let reply: l2_protocol_game::server::ExReplySentPost = decode(of(&out, A, "EX_REPLY_SENT_POST")[0]);
    assert_eq!((reply.receiver_name.as_str(), reply.items.len()), ("Carol", 1));
    // Reading a sent mail changes nothing in the table.
    assert!(mail_writes(&out).is_empty());
}

#[test]
fn the_items_that_can_be_attached() {
    let mut s = Scene::new();
    let mut worn = item(1010, CUIRASS, 1, "PAPERDOLL");
    worn.loc_data = 10;
    let mut items = alice_items();
    items.push(worn);
    s.enter(player(A, "Alice", items));
    s.log.take();
    s.mail(A, MailInput::PostItemList);
    let out = s.log.take();
    let list: l2_protocol_game::server::ExReplyPostItemList =
        decode(of(&out, A, "EX_REPLY_POST_ITEM_LIST")[0]);
    let mut shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    shown.sort_unstable();
    // `getAvailableItems(true, false, false)`: adena yes, the bound item and the worn cuirass no.
    assert_eq!(shown, [1001, 1002, 1004]);
}

// ---- deleting -----------------------------------------------------------------------------------

#[test]
fn a_mail_is_gone_when_both_sides_deleted_it() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[], None);

    s.mail(
        B,
        MailInput::DeleteReceivedPost {
            msg_ids: Some(vec![id]),
        },
    );
    let out = s.log.take();
    assert_eq!(mail_writes(&out), [MailWrite::MarkDeletedByReceiver(id)]);
    let state: ExChangePostState = decode(of(&out, B, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!(
        (state.received_board, state.msg_ids, state.change_id),
        (true, vec![id], 0)
    );
    assert_eq!(s.world.mail_inbox_size(B.0), 0);
    assert!(s.world.mail_message(id).is_some());

    s.mail(
        A,
        MailInput::DeleteSentPost {
            msg_ids: Some(vec![id]),
        },
    );
    let out = s.log.take();
    assert_eq!(mail_writes(&out), [MailWrite::Delete(id)]);
    let state: ExChangePostState = decode(of(&out, A, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!((state.received_board, state.change_id), (false, 0));
    assert!(s.world.mail_message(id).is_none());
}

/// `RequestDeleteReceivedPost`: a mail with attachments ends the request without an answer.
#[test]
fn a_mail_with_attachments_cannot_be_deleted() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let plain = s.alice_sends(&[], None);
    let with_items = s.alice_sends(&[(1002, 1)], None);
    s.mail(
        B,
        MailInput::DeleteReceivedPost {
            msg_ids: Some(vec![plain, with_items]),
        },
    );
    let out = s.log.take();
    // The first was deleted before Java reached the second, which ends everything.
    assert_eq!(mail_writes(&out), [MailWrite::MarkDeletedByReceiver(plain)]);
    assert!(of(&out, B, "EX_CHANGE_POST_STATE").is_empty());
}

#[test]
fn deleting_needs_the_peace_zone() {
    let mut s = Scene::with_zone("MotherTree");
    s.enter(player(A, "Alice", alice_items()));
    s.log.take();
    s.send(A, "Carol", &[], None);
    let id = mail_writes(&s.log.take())
        .iter()
        .find_map(|w| match w {
            MailWrite::Insert(m) => Some(m.id),
            _ => None,
        })
        .unwrap();
    s.mail(
        A,
        MailInput::DeleteSentPost {
            msg_ids: Some(vec![id]),
        },
    );
    let out = s.log.take();
    assert_eq!(
        messages(&out, A),
        [SystemMessageId::YouCannotReceiveOrSendMailWithAttachedItemsInNonPeaceZoneRegions]
    );
    assert!(mail_writes(&out).is_empty());
}

// ---- collecting ---------------------------------------------------------------------------------

/// `RequestPostAttachment`: the receiver pays, gets the items, the online sender is paid.
#[test]
fn collecting_a_payment_request_pays_the_online_sender() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![inv(2001, ADENA, 7_000)]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1), (1004, 20)], Some(5_000));
    let alice_adena = s.world.adena_of(A);

    s.mail(B, MailInput::PostAttachment { msg_id: id });
    let out = s.log.take();

    assert_eq!(s.world.adena_of(B), 2_000);
    assert_eq!(s.world.adena_of(A), alice_adena + 5_000);
    let bob: Vec<(i32, i64)> = s
        .inventory_of(B)
        .into_iter()
        .map(|(_, item, count)| (item, count))
        .collect();
    assert!(bob.contains(&(SWORD, 1)) && bob.contains(&(ORE, 20)), "{bob:?}");

    // The sender's message names the receiver and the amount.
    let raw = of(&out, A, "SYSTEM_MESSAGE");
    let paid: SystemMessage = decode(raw.last().unwrap());
    assert_eq!(
        paid.id,
        SystemMessageId::S2HasMadeAPaymentOfS1AdenaPerYourPaymentRequestMail
    );
    assert_eq!(
        paid.params,
        vec![
            SystemMessageParam::LongNumber(5_000),
            SystemMessageParam::Text("Bob".into())
        ]
    );
    // The receiver: one "acquired" message per item, the state, the success message, the list.
    let acquired: Vec<SystemMessage> = of(&out, B, "SYSTEM_MESSAGE")
        .iter()
        .map(|r| decode(r))
        .filter(|m: &SystemMessage| m.id == SystemMessageId::YouHaveAcquiredS2S1)
        .collect();
    assert_eq!(acquired.len(), 2);
    assert_eq!(
        acquired[0].params,
        vec![
            SystemMessageParam::ItemName(SWORD),
            SystemMessageParam::LongNumber(1)
        ]
    );
    let state: ExChangePostState = decode(of(&out, B, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!((state.received_board, state.change_id), (true, 1));
    assert!(messages(&out, B).contains(&SystemMessageId::MailSuccessfullyReceived));
    assert_eq!(of(&out, B, "EX_SHOW_RECEIVED_POST_LIST").len(), 1);

    // The table: the attachments flag is cleared; the item rows moved to Bob's inventory.
    assert!(mail_writes(&out).contains(&MailWrite::RemoveAttachments(id)));
    let rows = rows(&out);
    assert!(
        rows.iter()
            .any(|r| r.0 == "update" && r.1 == B.0 && r.2 == 1002 && r.5 == "INVENTORY"),
        "{rows:?}"
    );
    let mail = s.world.mail_message(id).unwrap();
    assert!(!mail.has_attachments && mail.attachments.is_empty());

    // A second try finds nothing.
    s.mail(B, MailInput::PostAttachment { msg_id: id });
    assert!(mail_writes(&s.log.take()).is_empty());
}

#[test]
fn collecting_pays_an_offline_sender_with_a_new_adena_row() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![inv(2001, ADENA, 7_000)]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], Some(3_000));
    // Alice goes away.
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(A.0).unwrap()),
    });
    s.log.take();

    s.mail(B, MailInput::PostAttachment { msg_id: id });
    let out = s.log.take();
    assert_eq!(s.world.adena_of(B), 4_000);
    // NOTE(java-quirk): a new stack in the sender's inventory.
    let inserts: Vec<Row> = rows(&out).into_iter().filter(|r| r.0 == "insert").collect();
    assert_eq!(inserts.len(), 1, "{inserts:?}");
    assert_eq!((inserts[0].1, inserts[0].3, inserts[0].4), (A.0, ADENA, 3_000));
    assert_eq!(inserts[0].5, "INVENTORY");
}

#[test]
fn collecting_without_a_payment_tells_the_online_sender() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], None);
    s.mail(B, MailInput::PostAttachment { msg_id: id });
    let out = s.log.take();
    let told: SystemMessage = decode(of(&out, A, "SYSTEM_MESSAGE").last().unwrap());
    assert_eq!(told.id, SystemMessageId::S1AcquiredTheAttachedItemToYourMail);
    assert_eq!(told.params, vec![SystemMessageParam::Text("Bob".into())]);
}

#[test]
fn collecting_fails_without_the_adena_and_changes_nothing() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![inv(2001, ADENA, 100)]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], Some(5_000));
    s.mail(B, MailInput::PostAttachment { msg_id: id });
    let out = s.log.take();
    assert!(messages(&out, B).contains(&SystemMessageId::YouCannotReceiveBecauseYouDonTHaveEnoughAdena));
    assert!(mail_writes(&out).is_empty());
    assert!(rows(&out).is_empty());
    assert_eq!(s.world.adena_of(B), 100);
    assert!(s.world.mail_message(id).unwrap().has_attachments);
}

#[test]
fn collecting_needs_the_receiver() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], None);
    // The sender is not the receiver: nothing happens.
    s.mail(A, MailInput::PostAttachment { msg_id: id });
    let out = s.log.take();
    assert!(mail_writes(&out).is_empty());
    assert!(s.world.mail_message(id).unwrap().has_attachments);
}

#[test]
fn collecting_needs_a_peace_zone() {
    let mut s = Scene::with_zone("MotherTree");
    s.enter(player(B, "Bob", vec![]));
    s.world.load_mail(MailLoad {
        messages: vec![stored_mail(
            500,
            C,
            B,
            unix_ms() + 3_600_000,
            &[in_mail(7001, SWORD, 1, 500)],
        )],
        characters: vec![],
    });
    s.log.take();
    s.mail(B, MailInput::PostAttachment { msg_id: 500 });
    assert_eq!(
        messages(&s.log.take(), B),
        [SystemMessageId::YouCannotReceiveInANonPeaceZoneLocation]
    );
    // Opening a mail with attachments and rejecting it are refused there too.
    s.mail(B, MailInput::ReceivedPost { msg_id: 500 });
    assert_eq!(
        messages(&s.log.take(), B),
        [SystemMessageId::YouCannotReceiveOrSendMailWithAttachedItemsInNonPeaceZoneRegions]
    );
    s.mail(B, MailInput::RejectPostAttachment { msg_id: 500 });
    assert_eq!(
        messages(&s.log.take(), B),
        [SystemMessageId::YouCannotReceiveOrSendMailWithAttachedItemsInNonPeaceZoneRegions]
    );
    assert!(s.world.mail_message(500).unwrap().has_attachments);
}

// ---- rejecting and cancelling --------------------------------------------------------------------

#[test]
fn rejecting_returns_the_items_in_a_new_mail_to_the_sender() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], None);

    s.mail(B, MailInput::RejectPostAttachment { msg_id: id });
    let out = s.log.take();

    let writes = mail_writes(&out);
    assert_eq!(writes.len(), 2, "{writes:?}");
    // The old mail loses its attachments first, then the new one is inserted.
    assert_eq!(writes[0], MailWrite::RemoveAttachments(id));
    let MailWrite::Insert(returned) = &writes[1] else {
        panic!("the returned mail");
    };
    assert_eq!((returned.sender_id, returned.receiver_id), (A.0, A.0));
    assert!(returned.returned && returned.has_attachments && returned.unread);
    assert!(returned.deleted_by_sender && !returned.deleted_by_receiver);
    // NOTE(java-quirk): `MessageSenderType.NONE`.
    assert_eq!(returned.send_by_system, 2);
    assert_eq!(returned.req_adena, 0);
    assert_eq!(returned.attachments.len(), 1);
    // The item rows: the item now carries the new message id.
    let rows = rows(&out);
    assert_eq!(
        rows,
        vec![("update", A.0, 1002, SWORD, 1, "MAIL".into(), returned.id)]
    );
    assert!(!s.world.mail_message(id).unwrap().has_attachments);

    // Bob: the success message and the state; Alice: the envelope and who returned it.
    assert!(messages(&out, B).contains(&SystemMessageId::MailSuccessfullyReturned));
    let state: ExChangePostState = decode(of(&out, B, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!((state.received_board, state.change_id), (true, 2));
    assert_eq!(of(&out, A, "EX_NOTICE_POST_ARRIVED").len(), 1);
    let told: SystemMessage = decode(of(&out, A, "SYSTEM_MESSAGE").last().unwrap());
    assert_eq!(told.id, SystemMessageId::S1ReturnedTheMail);
    assert_eq!(told.params, vec![SystemMessageParam::Text("Bob".into())]);

    // Alice collects them from the returned mail ("****" as the sender).
    s.mail(A, MailInput::ReceivedPostList);
    let out = s.log.take();
    let list: ExShowReceivedPostList = decode(of(&out, A, "EX_SHOW_RECEIVED_POST_LIST")[0]);
    assert_eq!(list.mails.len(), 1);
    assert_eq!(
        (list.mails[0].sender_name.as_str(), list.mails[0].returned),
        ("****", true)
    );
    s.mail(A, MailInput::PostAttachment { msg_id: returned.id });
    let out = s.log.take();
    assert!(messages(&out, A).contains(&SystemMessageId::MailSuccessfullyReceived));
    assert!(s.inventory_of(A).iter().any(|i| i.1 == SWORD));
    // A returned mail cannot be returned again.
}

#[test]
fn the_sender_cancels_a_mail_nobody_collected() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1), (1004, 20)], None);

    s.mail(A, MailInput::CancelPostAttachment { msg_id: id });
    let out = s.log.take();

    // The items are back (the ore joined its stack), the mail is gone.
    let alice = s.inventory_of(A);
    assert!(alice.contains(&(1002, SWORD, 1)), "{alice:?}");
    assert!(alice.iter().any(|i| i.1 == ORE && i.2 == 50), "{alice:?}");
    assert!(s.world.mail_message(id).is_none());
    let writes = mail_writes(&out);
    assert!(writes.contains(&MailWrite::RemoveAttachments(id)));
    assert_eq!(writes.last(), Some(&MailWrite::Delete(id)));
    assert!(messages(&out, A).contains(&SystemMessageId::MailSuccessfullyCancelled));
    let state: ExChangePostState = decode(of(&out, A, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!((state.received_board, state.change_id), (false, 0));
    // The receiver is told and its list is updated.
    let told: SystemMessage = decode(of(&out, B, "SYSTEM_MESSAGE").last().unwrap());
    assert_eq!(told.id, SystemMessageId::S1CanceledTheSentMail);
    let state: ExChangePostState = decode(of(&out, B, "EX_CHANGE_POST_STATE")[0]);
    assert_eq!((state.received_board, state.change_id), (true, 0));
    assert_eq!(s.world.mail_inbox_size(B.0), 0);
}

#[test]
fn a_collected_mail_cannot_be_cancelled() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    let id = s.alice_sends(&[(1002, 1)], None);
    s.mail(B, MailInput::PostAttachment { msg_id: id });
    s.log.take();
    s.mail(A, MailInput::CancelPostAttachment { msg_id: id });
    let out = s.log.take();
    assert_eq!(
        messages(&out, A),
        [SystemMessageId::YouCannotCancelSentMailSinceTheRecipientReceivedIt]
    );
    assert!(s.world.mail_message(id).is_some());
}

// ---- the expiry ---------------------------------------------------------------------------------

fn stored_mail(
    id: i32,
    sender: ObjectId,
    receiver: ObjectId,
    expiration: i64,
    items: &[ItemData],
) -> MailMessage {
    let mut mail = MailMessage::new_player_mail(id, sender.0, receiver.0, false, "s", "t", 0, 0);
    mail.expiration = expiration;
    mail.has_attachments = !items.is_empty();
    mail.attachments = items.to_vec();
    mail
}

fn in_mail(object_id: i32, item_id: i32, count: i64, message: i32) -> ItemData {
    let mut item = item(object_id, item_id, count, "MAIL");
    item.loc_data = message;
    item
}

/// `MessageDeletionTaskManager`: every 10 seconds; an online sender gets the items in the private
/// warehouse and a message, the receiver a message, the row goes.
#[test]
fn an_expired_mail_returns_the_items_to_an_online_sender() {
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.world.load_mail(MailLoad {
        messages: vec![
            stored_mail(500, A, B, 1, &[in_mail(7001, SWORD, 1, 500)]),
            stored_mail(501, A, B, unix_ms() + 3_600_000, &[]),
        ],
        characters: vec![],
    });
    s.log.take();

    // The task runs 10 s after the world started its post box (two ticks went by already).
    s.advance(9_700);
    let early = s.log.take();
    assert!(
        mail_writes(&early).is_empty() && rows(&early).is_empty(),
        "nothing before the 10 s are up: {early:?}"
    );
    s.advance(500);
    let out = s.log.take();

    let warehouse = &s.world.player(A).unwrap().warehouse.private;
    assert_eq!(warehouse.len(), 1);
    assert_eq!(
        (warehouse[0].object_id.0, warehouse[0].loc.as_str()),
        (7001, "WAREHOUSE")
    );
    assert!(messages(&out, A).contains(&SystemMessageId::TheMailWasReturnedDueToTheExceededWaitingTime));
    assert!(messages(&out, B).contains(&SystemMessageId::TheMailWasReturnedDueToTheExceededWaitingTime));
    assert!(s.world.mail_message(500).is_none());
    assert!(s.world.mail_message(501).is_some());
    let writes = mail_writes(&out);
    assert_eq!(writes.last(), Some(&MailWrite::Delete(500)));
    assert!(writes.contains(&MailWrite::RemoveAttachments(500)));
    let rows = rows(&out);
    assert_eq!(rows[0], ("update", A.0, 7001, SWORD, 1, "WAREHOUSE".into(), 0));
}

#[test]
fn an_expired_mail_of_an_offline_sender_goes_to_the_warehouse_location() {
    let mut s = Scene::new();
    s.enter(player(B, "Bob", vec![]));
    s.world.load_mail(MailLoad {
        messages: vec![stored_mail(500, C, B, 1, &[in_mail(7001, SWORD, 1, 500)])],
        characters: vec![],
    });
    s.log.take();
    s.advance(10_100);
    let out = s.log.take();
    let rows = rows(&out);
    // `returnToWh(null)`: `setItemLocation(WAREHOUSE)`, written by `deleteMe`.
    assert_eq!(rows, vec![("update", C.0, 7001, SWORD, 1, "WAREHOUSE".into(), 0)]);
    assert!(messages(&out, B).contains(&SystemMessageId::TheMailWasReturnedDueToTheExceededWaitingTime));
    assert_eq!(mail_writes(&out).last(), Some(&MailWrite::Delete(500)));
}

#[test]
fn an_expired_mail_without_attachments_is_just_deleted() {
    let mut s = Scene::new();
    s.enter(player(B, "Bob", vec![]));
    s.world.load_mail(MailLoad {
        messages: vec![stored_mail(500, C, B, 1, &[])],
        characters: vec![],
    });
    s.log.take();
    s.advance(10_100);
    let out = s.log.take();
    assert_eq!(mail_writes(&out), [MailWrite::Delete(500)]);
    assert!(messages(&out, B).is_empty());
}

// ---- login --------------------------------------------------------------------------------------

#[test]
fn unread_mail_is_announced_at_login_without_the_animation() {
    let mut s = Scene::new();
    s.world.load_mail(MailLoad {
        messages: vec![stored_mail(500, C, B, unix_ms() + 3_600_000, &[])],
        characters: vec![],
    });
    s.enter(player(A, "Alice", vec![]));
    let out = s.log.take();
    assert!(of(&out, A, "EX_NOTICE_POST_ARRIVED").is_empty());
    s.enter(player(B, "Bob", vec![]));
    let out = s.log.take();
    let notices = of(&out, B, "EX_NOTICE_POST_ARRIVED");
    assert_eq!(notices.len(), 1);
    let notice: ExNoticePostArrived = decode(notices[0]);
    assert!(!notice.show_animation);
}

#[test]
fn the_lists_follow_the_order_of_a_java_concurrent_hash_map() {
    // Loaded in the order of the expiration; listed in the map's order. Small ids sit in the
    // bucket of their value, so they come out sorted.
    let mut s = Scene::new();
    s.enter(player(B, "Bob", vec![]));
    let now = unix_ms() + 3_600_000;
    s.world.load_mail(MailLoad {
        messages: vec![
            stored_mail(9, C, B, now, &[]),
            stored_mail(3, C, B, now + 1, &[]),
            stored_mail(5, C, B, now + 2, &[]),
        ],
        characters: vec![],
    });
    s.log.take();
    s.mail(B, MailInput::ReceivedPostList);
    let out = s.log.take();
    let list: ExShowReceivedPostList = decode(of(&out, B, "EX_SHOW_RECEIVED_POST_LIST")[0]);
    assert_eq!(list.mails.iter().map(|m| m.msg_id).collect::<Vec<_>>(), [3, 5, 9]);
}

#[test]
fn a_dropped_inventory_update_is_not_needed_to_check_the_inventory_packet() {
    // The attachments leave the inventory through an `InventoryUpdate`: removed for a whole
    // item, modified for a split stack.
    let mut s = Scene::new();
    s.enter(player(A, "Alice", alice_items()));
    s.enter(player(B, "Bob", vec![]));
    s.log.take();
    s.send(A, "Bob", &[(1002, 1), (1004, 20)], None);
    s.advance(500);
    let out = s.log.take();
    let updates: Vec<(i16, i32, i64)> = of(&out, A, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.change, u.item.object_id, u.item.count))
        .collect();
    assert!(
        updates.contains(&(InventoryUpdate::REMOVED, 1002, 1)),
        "{updates:?}"
    );
    assert!(
        updates.contains(&(InventoryUpdate::MODIFIED, 1004, 30)),
        "{updates:?}"
    );
}
