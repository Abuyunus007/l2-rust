#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Teleport bookmarks and the mail contact list (L-04). Expected values come from
//! `Player.teleportBookmarkAdd/Modify/Delete/Go/Condition`, `RequestSaveBookMarkSlot.java` and
//! friends, `ExGetBookMarkInfoPacket.java`, `ContactList.java`, `RequestExAddContactToContactList.java`
//! and `FriendListExtended.java`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::enterworld::TeleportBookmark;
use l2_protocol_game::server::{
    ExConfirmAddingContact, ExGetBookMarkInfoPacket, ExShowContactList, FriendListExtended,
    FriendListExtendedEntry, SystemMessage, SystemMessageParam, TeleportToLocation,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- harness -----------------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Out {
    Packet(Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(Out::Packet(bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
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
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
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

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn packets<'a>(out: &'a [Out], name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) if name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .collect()
}

fn decoded<P: Decode>(out: &[Out], name: &str) -> Vec<P> {
    packets(out, name)
        .into_iter()
        .map(|raw| {
            let skip = if raw[0] == 0xFE { 3 } else { 1 };
            P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
        })
        .collect()
}

fn messages(out: &[Out]) -> Vec<SystemMessage> {
    decoded::<SystemMessage>(out, "SYSTEM_MESSAGE")
}

fn message_ids(out: &[Out]) -> Vec<SystemMessageId> {
    messages(out).into_iter().map(|m| m.id).collect()
}

fn names(out: &[Out]) -> Vec<String> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) => Some(name_of(b)),
            Out::Persist(_) => None,
        })
        .collect()
}

fn bookmark_writes(out: &[Out]) -> Vec<BookmarkWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Bookmark(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn contact_writes(out: &[Out]) -> Vec<ContactWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Contact(w)) => Some(*w),
            _ => None,
        })
        .collect()
}

// ---- data --------------------------------------------------------------------------------------

fn template(class_id: i32) -> String {
    format!(
        r#"<list><classId>{class_id}</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#
    )
}

const ADENA: i32 = 57;
/// The scroll `teleportBookmarkGo` uses up.
const SCROLL: i32 = 13016;
/// The default `BookmarkConsumeItemId`, a stackable flag.
const FLAG: i32 = 20033;
/// A consumable that is not the flag: `INCORRECT_ITEM_COUNT_2` when missing.
const OTHER: i32 = 20034;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="13016" type="EtcItem" name="My Teleport Scroll"/>
<item id="20033" type="EtcItem" name="My Teleport Flag"><set name="is_stackable" val="true" /></item>
<item id="20034" type="EtcItem" name="Other"><set name="is_stackable" val="true" /></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&template(2), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::from_xml("<list></list>").unwrap()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &l2_data::skill::EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const ME: ObjectId = ObjectId(0x2000_0001);
const BUDDY: ObjectId = ObjectId(0x2000_0002);
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn item(object_id: i32, item_id: i32, slot: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        loc_data: slot,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn bookmark(id: i32, name: &str) -> BookmarkData {
    BookmarkData {
        id,
        x: 1000 * id,
        y: 2000 * id,
        z: -3000,
        icon: id,
        tag: format!("tag{id}"),
        name: name.to_owned(),
    }
}

fn player_data(id: ObjectId, name: &str, items: Vec<ItemData>, profile: PlayerProfile) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
        title: String::new(),
        class_id: 2,
        base_class: 2,
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
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile,
    }
}

/// A datapack root with a `ConditionZone` (`NoBookmark`) around the character and the one map
/// region the teleport code needs.
struct Root(PathBuf);

impl Root {
    fn new(tag: &str, no_bookmark_zone: bool) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-l04-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        let zone = if no_bookmark_zone {
            format!(
                "<zone id=\"7\" name=\"nb\" type=\"ConditionZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
                 <stat name=\"NoBookmark\" val=\"true\"/>\
                 <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>",
                X - 2_000,
                Y - 2_000,
                X + 2_000,
                Y + 2_000
            )
        } else {
            String::new()
        };
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone}</list>\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Options {
    tag: &'static str,
    slots: i32,
    consume: i32,
    items: Vec<ItemData>,
    bookmarks: Vec<BookmarkData>,
    contacts: Vec<String>,
    zone: bool,
    allow_mail: bool,
}

impl Options {
    fn new(tag: &'static str) -> Self {
        Self {
            tag,
            slots: 3,
            consume: FLAG,
            items: vec![
                item(0x3000_0001, ADENA, 0, 100_000),
                item(0x3000_0002, FLAG, 1, 5),
                item(0x3000_0003, SCROLL, 2, 1),
                item(0x3000_0004, SCROLL, 3, 1),
            ],
            bookmarks: Vec::new(),
            contacts: Vec::new(),
            zone: false,
            allow_mail: true,
        }
    }
}

struct Scene {
    world: World,
    log: Log,
    /// What the character received while it entered the world.
    entered: Vec<Out>,
    _root: Root,
}

impl Scene {
    fn new(options: Options) -> Self {
        let root = Root::new(options.tag, options.zone);
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_027), Box::new(Recorder(log.clone())));
        world.set_settings(WorldSettings {
            bookmark_consume_item_id: options.consume,
            ..WorldSettings::default()
        });
        world.set_mail_settings(MailSettings {
            allow_mail: options.allow_mail,
            ..MailSettings::default()
        });
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        // `CharInfoTable`: the names of the database. "Stranger" is in it and never online.
        world.load_char_names([(0x1000_0042, "Stranger".to_owned())]);
        let profile = PlayerProfile {
            bookmark_slot: options.slots,
            bookmarks: options.bookmarks,
            contacts: options.contacts,
            ..PlayerProfile::default()
        };
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, "Hero", options.items, profile)),
            sink: Box::new(Sink(log.clone())),
            ip: None,
        });
        world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Appearing,
        });
        for _ in 0..20 {
            world.tick();
        }
        let entered = log.take();
        Self {
            world,
            log,
            entered,
            _root: root,
        }
    }

    /// A second character, online, with the given name.
    fn enter_buddy(&mut self, name: &str) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(BUDDY, name, vec![], PlayerProfile::default())),
            sink: Box::new(Sink(self.log.clone())),
            ip: None,
        });
        for _ in 0..20 {
            self.world.tick();
        }
        self.log.take();
    }

    fn input(&mut self, input: ClientInput) -> Vec<Out> {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
        // The inventory update goes out 100 ms after the change.
        for _ in 0..5 {
            self.world.tick();
        }
        self.log.take()
    }

    fn bookmark(&mut self, input: BookmarkInput) -> Vec<Out> {
        self.input(ClientInput::Bookmark(input))
    }

    fn contacts(&mut self, input: ContactsInput) -> Vec<Out> {
        self.input(ClientInput::Contacts(input))
    }

    fn save(&mut self, name: &str) -> Vec<Out> {
        self.bookmark(BookmarkInput::Save {
            name: name.into(),
            icon: 4,
            tag: "mine".into(),
        })
    }

    fn count(&self, item_id: i32) -> i64 {
        self.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    fn bookmarks(&self) -> Vec<BookmarkData> {
        self.world.player(ME).unwrap().data.profile.bookmarks.clone()
    }

    fn contact_names(&self) -> Vec<String> {
        self.world.player(ME).unwrap().data.profile.contacts.clone()
    }
}

fn list_of(out: &[Out]) -> Vec<ExGetBookMarkInfoPacket> {
    decoded::<ExGetBookMarkInfoPacket>(out, "EX_GET_BOOKMARK_INFO")
}

// ---- bookmarks ---------------------------------------------------------------------------------

/// The list at enter world and `RequestBookMarkSlotInfo` are `ExGetBookMarkInfoPacket`: the slot
/// count and every bookmark with its location, name, icon and tag.
#[test]
fn the_list_is_sent_at_enter_world_and_on_request() {
    let mut options = Options::new("list");
    options.bookmarks = vec![bookmark(1, "Gludio"), bookmark(2, "Giran")];
    let mut s = Scene::new(options);
    let at_enter = list_of(&s.entered);
    assert_eq!(at_enter.len(), 1);
    assert_eq!(at_enter[0].slot, 3);
    let expected = |b: &BookmarkData| TeleportBookmark {
        id: b.id,
        x: b.x,
        y: b.y,
        z: b.z,
        name: b.name.clone(),
        icon: b.icon,
        tag: b.tag.clone(),
    };
    // In the order of the profile (the session loads them sorted by id).
    assert_eq!(
        at_enter[0].bookmarks,
        vec![expected(&bookmark(1, "Gludio")), expected(&bookmark(2, "Giran"))]
    );

    let out = s.bookmark(BookmarkInput::SlotInfo);
    assert_eq!(list_of(&out), at_enter);
    // RequestChangeBookMarkSlot reads nothing and does nothing.
    assert!(s.bookmark(BookmarkInput::ChangeSlot).is_empty());
}

/// Save, use, modify and delete: the whole life of a bookmark, with the rows Java writes.
#[test]
fn save_modify_teleport_and_delete_a_bookmark() {
    let mut s = Scene::new(Options::new("life"));
    assert_eq!(s.count(FLAG), 5);

    // Save: the flag is used up, the location is the character's, the row is written and the
    // list sent.
    let out = s.save("Home");
    assert_eq!(s.count(FLAG), 4);
    let saved = BookmarkData {
        id: 1,
        x: X,
        y: Y,
        z: Z,
        icon: 4,
        tag: "mine".into(),
        name: "Home".into(),
    };
    assert_eq!(s.bookmarks(), [saved.clone()]);
    assert_eq!(
        bookmark_writes(&out),
        [BookmarkWrite::Add {
            char_id: ME.0,
            bookmark: saved.clone()
        }]
    );
    let list = list_of(&out);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].bookmarks.len(), 1);
    assert_eq!(list[0].bookmarks[0].name, "Home");
    assert_eq!(list[0].slot, 3);

    // Modify: icon, tag and name change; one UPDATE; the list is sent.
    let out = s.bookmark(BookmarkInput::Modify {
        id: 1,
        name: "Castle".into(),
        icon: 9,
        tag: "x".into(),
    });
    assert_eq!(
        bookmark_writes(&out),
        [BookmarkWrite::Update {
            char_id: ME.0,
            id: 1,
            icon: 9,
            tag: "x".into(),
            name: "Castle".into()
        }]
    );
    let b = &s.bookmarks()[0];
    assert_eq!((b.icon, b.tag.as_str(), b.name.as_str()), (9, "x", "Castle"));
    assert_eq!((b.x, b.y, b.z), (X, Y, Z));
    assert_eq!(list_of(&out).len(), 1);

    // Modify of a bookmark that is not there: no row, but the list is sent.
    let out = s.bookmark(BookmarkInput::Modify {
        id: 7,
        name: "No".into(),
        icon: 1,
        tag: String::new(),
    });
    assert!(bookmark_writes(&out).is_empty());
    assert_eq!(list_of(&out).len(), 1);

    // Teleport: "S1 has disappeared" for the scroll, one scroll used up, TeleportToLocation to the
    // bookmark (Java adds 5 to Z), the list sent.
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(s.count(SCROLL), 1);
    assert_eq!(message_ids(&out), [SystemMessageId::S1HasDisappeared]);
    assert_eq!(messages(&out)[0].params, [SystemMessageParam::ItemName(SCROLL)]);
    let tele = decoded::<TeleportToLocation>(&out, "TELEPORT_TO_LOCATION");
    assert_eq!(tele.len(), 1);
    assert_eq!((tele[0].x, tele[0].y, tele[0].z), (X, Y, Z + 5));
    assert_eq!(list_of(&out).len(), 1);

    // Delete: gone, one DELETE, the list sent. Deleting it again sends nothing.
    let out = s.bookmark(BookmarkInput::Delete { id: 1 });
    assert!(s.bookmarks().is_empty());
    assert_eq!(
        bookmark_writes(&out),
        [BookmarkWrite::Delete { char_id: ME.0, id: 1 }]
    );
    assert_eq!(list_of(&out)[0].bookmarks.len(), 0);
    assert!(s.bookmark(BookmarkInput::Delete { id: 1 }).is_empty());
}

/// The first free number is taken, and with every slot used the next save is refused before the
/// flag is touched.
#[test]
fn saves_take_the_first_free_slot_and_stop_at_the_slot_count() {
    let mut s = Scene::new(Options::new("slots"));
    s.save("a");
    s.save("b");
    s.save("c");
    assert_eq!(s.bookmarks().iter().map(|b| b.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(s.count(FLAG), 2);

    let out = s.save("d");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YouHaveNoSpaceToSaveTheTeleportLocation]
    );
    assert_eq!(s.count(FLAG), 2);
    assert!(bookmark_writes(&out).is_empty());

    // Slot 2 is free again and is the next one.
    s.bookmark(BookmarkInput::Delete { id: 2 });
    let out = s.save("e");
    assert_eq!(s.bookmarks().iter().map(|b| b.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(s.bookmarks()[1].name, "e");
    assert!(matches!(
        bookmark_writes(&out)[0],
        BookmarkWrite::Add { ref bookmark, .. } if bookmark.id == 2
    ));
}

/// `BookmarkConsumeItemId`: the flag (20033) has its own message, any other item says "incorrect
/// item count", and a value of 0 or less asks for nothing.
#[test]
fn the_consumed_item_decides_whether_a_save_is_allowed() {
    let mut no_flag = Options::new("noflag");
    no_flag.items = vec![item(0x3000_0001, ADENA, 0, 100)];
    let mut s = Scene::new(no_flag);
    let out = s.save("x");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YouCannotBookmarkThisLocationBecauseYouDoNotHaveAMyTeleportFlag]
    );
    assert!(s.bookmarks().is_empty() && bookmark_writes(&out).is_empty());

    let mut other = Options::new("other");
    other.consume = OTHER;
    other.items = vec![item(0x3000_0001, ADENA, 0, 100)];
    let mut s = Scene::new(other);
    assert_eq!(message_ids(&s.save("x")), [SystemMessageId::IncorrectItemCount2]);

    let mut free = Options::new("free");
    free.consume = -1;
    free.items = vec![];
    let mut s = Scene::new(free);
    let out = s.save("x");
    assert_eq!(s.bookmarks().len(), 1);
    assert!(message_ids(&out).is_empty());
}

/// A teleport needs a scroll with enchant 0; without one nothing is used and nothing moves.
#[test]
fn a_teleport_needs_a_scroll() {
    let mut options = Options::new("noscroll");
    options.items = vec![item(0x3000_0001, ADENA, 0, 1)];
    options.bookmarks = vec![bookmark(1, "Home")];
    let mut s = Scene::new(options);
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YouCannotTeleportBecauseYouDoNotHaveATeleportItem]
    );
    assert!(packets(&out, "TELEPORT_TO_LOCATION").is_empty());
    assert!(list_of(&out).is_empty());

    // An enchanted scroll does not count (`getInventoryItemCount(13016, 0)`).
    let mut options = Options::new("enchanted");
    options.items = vec![ItemData {
        enchant_level: 3,
        ..item(0x3000_0003, SCROLL, 1, 1)
    }];
    options.bookmarks = vec![bookmark(1, "Home")];
    let mut s = Scene::new(options);
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YouCannotTeleportBecauseYouDoNotHaveATeleportItem]
    );
}

/// NOTE(java-quirk): the message comes before the bookmark is looked up. An id that is not a
/// bookmark shows "S1 has disappeared" and costs nothing.
#[test]
fn an_unknown_bookmark_shows_the_message_and_costs_nothing() {
    let mut options = Options::new("unknown");
    options.bookmarks = vec![bookmark(1, "Home")];
    let mut s = Scene::new(options);
    let out = s.bookmark(BookmarkInput::Teleport { id: 9 });
    assert_eq!(message_ids(&out), [SystemMessageId::S1HasDisappeared]);
    assert_eq!(s.count(SCROLL), 2);
    assert!(packets(&out, "TELEPORT_TO_LOCATION").is_empty());
    assert_eq!(list_of(&out).len(), 1);
}

/// `teleportBookmarkCondition`: the refusals, with the message of each and for each of the two
/// uses; nothing is used up when it refuses.
#[test]
fn the_conditions_refuse_with_javas_messages() {
    use SystemMessageId as M;
    let mut options = Options::new("conditions");
    options.bookmarks = vec![bookmark(1, "Home")];
    let mut s = Scene::new(options);

    // A dead character.
    s.world.set_dead(ME, true);
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(message_ids(&out), [M::YouCannotUseMyTeleportsWhileYouAreDead]);
    let out = s.save("x");
    assert_eq!(message_ids(&out), [M::YouCannotUseMyTeleportsWhileYouAreDead]);
    s.world.set_dead(ME, false);

    // A siege participant.
    s.world.set_in_siege(ME, true);
    let out = s.save("x");
    assert_eq!(
        message_ids(&out),
        [M::YouCannotUseMyTeleportsWhileParticipatingALargeScaleBattleSuchAsACastleSiegeFortressSiegeOrHideoutSiege]
    );
    s.world.set_in_siege(ME, false);
    s.world.set_siege_state(ME, 2);
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(
        message_ids(&out),
        [M::YouCannotUseMyTeleportsWhileParticipatingALargeScaleBattleSuchAsACastleSiegeFortressSiegeOrHideoutSiege]
    );
    s.world.set_siege_state(ME, 0);

    assert_eq!(s.count(SCROLL), 2);
    assert_eq!(s.count(FLAG), 5);
    assert_eq!(s.bookmarks().len(), 1);
    // Free again: it works.
    s.save("ok");
    assert_eq!(s.bookmarks().len(), 2);
}

/// A `ConditionZone` with `NoBookmark`: "in this area" to use one, "to reach this area" to save
/// one.
#[test]
fn a_no_bookmark_zone_refuses_both_uses() {
    use SystemMessageId as M;
    let mut options = Options::new("zone");
    options.zone = true;
    options.bookmarks = vec![bookmark(1, "Home")];
    let mut s = Scene::new(options);
    assert!(s.world.is_inside_zone(ME, ZoneId::NoBookmark));
    let out = s.bookmark(BookmarkInput::Teleport { id: 1 });
    assert_eq!(message_ids(&out), [M::YouCannotUseMyTeleportsInThisArea]);
    let out = s.save("x");
    assert_eq!(message_ids(&out), [M::YouCannotUseMyTeleportsToReachThisArea]);
    assert_eq!(s.count(SCROLL), 2);
    assert_eq!(s.count(FLAG), 5);
}

// ---- contacts ----------------------------------------------------------------------------------

/// The contact list at enter world and on request is `ExShowContactList`.
#[test]
fn the_contact_list_is_sent_at_enter_world_and_on_request() {
    let mut options = Options::new("contact-list");
    options.contacts = vec!["Alpha".into(), "Beta".into()];
    let mut s = Scene::new(options);
    let at_enter = decoded::<ExShowContactList>(&s.entered, "EX_RECEIVE_SHOW_POST_FRIEND");
    assert_eq!(at_enter[0].contacts, ["Alpha", "Beta"]);
    let out = s.contacts(ContactsInput::Show);
    assert_eq!(
        decoded::<ExShowContactList>(&out, "EX_RECEIVE_SHOW_POST_FRIEND")[0].contacts,
        ["Alpha", "Beta"]
    );
}

/// Add and delete: the name must be a known character, the row is written with its id, and the
/// client gets the success message and `ExConfirmAddingContact`.
#[test]
fn contacts_are_added_and_deleted_by_name() {
    let mut s = Scene::new(Options::new("contacts"));
    s.enter_buddy("Buddy");

    // An online character.
    let out = s.contacts(ContactsInput::Add { name: "Buddy".into() });
    assert_eq!(s.contact_names(), ["Buddy"]);
    assert_eq!(
        contact_writes(&out),
        [ContactWrite::Add {
            char_id: ME.0,
            contact_id: BUDDY.0
        }]
    );
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::S1WasSuccessfullyAddedToYourContactList]
    );
    assert_eq!(
        messages(&out)[0].params,
        [SystemMessageParam::Text("Buddy".into())]
    );
    let confirm = decoded::<ExConfirmAddingContact>(&out, "EX_CONFIRM_ADDING_POST_FRIEND");
    assert_eq!(
        confirm,
        [ExConfirmAddingContact {
            name: "Buddy".into(),
            added: true
        }]
    );

    // A character of the database who is not online, typed in another case: the name is kept as
    // typed.
    let out = s.contacts(ContactsInput::Add {
        name: "Stranger".into(),
    });
    assert_eq!(s.contact_names(), ["Buddy", "Stranger"]);
    assert_eq!(
        contact_writes(&out),
        [ContactWrite::Add {
            char_id: ME.0,
            contact_id: 0x1000_0042
        }]
    );

    // Delete.
    let out = s.contacts(ContactsInput::Delete { name: "Buddy".into() });
    assert_eq!(s.contact_names(), ["Stranger"]);
    assert_eq!(
        contact_writes(&out),
        [ContactWrite::Remove {
            char_id: ME.0,
            contact_id: BUDDY.0
        }]
    );
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::S1WasSuccessfullyDeletedFromYourContactList]
    );
    // Delete sends no ExConfirmAddingContact.
    assert!(packets(&out, "EX_CONFIRM_ADDING_POST_FRIEND").is_empty());
}

/// The refusals of `ContactList.add` and `remove`, in Java's order; the client still gets
/// `ExConfirmAddingContact` for an add, with `added` false.
#[test]
fn contact_refusals_follow_javas_order() {
    use SystemMessageId as M;
    let mut options = Options::new("refusals");
    options.contacts = vec!["Stranger".into()];
    let mut s = Scene::new(options);
    s.enter_buddy("Buddy");

    let refused = |s: &mut Scene, name: &str| {
        let out = s.contacts(ContactsInput::Add { name: name.into() });
        let confirm = decoded::<ExConfirmAddingContact>(&out, "EX_CONFIRM_ADDING_POST_FRIEND");
        assert_eq!(confirm.len(), 1);
        assert!(!confirm[0].added, "{name}");
        assert!(contact_writes(&out).is_empty());
        message_ids(&out)
    };
    assert_eq!(
        refused(&mut s, "Stranger"),
        [M::TheNameAlreadyExistsOnTheAddedList]
    );
    // Same name in another case: found by the `equalsIgnoreCase` loop after the id lookup.
    assert_eq!(
        refused(&mut s, "stranger"),
        [M::TheNameAlreadyExistsOnTheAddedList]
    );
    assert_eq!(refused(&mut s, "Hero"), [M::YouCannotAddYourOwnName]);
    assert_eq!(
        refused(&mut s, "Nobody"),
        [M::TheNameS1DoesnTExistPleaseTryAnotherName]
    );
    assert_eq!(s.contact_names(), ["Stranger"]);

    // Deleting a name that is not on the list.
    let out = s.contacts(ContactsInput::Delete { name: "Buddy".into() });
    assert_eq!(message_ids(&out), [M::TheNameIsNotCurrentlyRegistered]);
    assert!(contact_writes(&out).is_empty());
}

/// At most 100 contacts.
#[test]
fn the_list_holds_a_hundred_names() {
    let mut options = Options::new("hundred");
    options.contacts = (0..100).map(|i| format!("Name{i}")).collect();
    let mut s = Scene::new(options);
    s.enter_buddy("Buddy");
    let out = s.contacts(ContactsInput::Add { name: "Buddy".into() });
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::TheMaximumNumberOfNames100HasBeenReachedYouCannotRegisterAnyMore]
    );
    assert_eq!(s.contact_names().len(), 100);
}

/// With `AllowMail = False` all four requests are ignored, like Java.
#[test]
fn contact_requests_are_ignored_without_mail() {
    let mut options = Options::new("nomail");
    options.allow_mail = false;
    let mut s = Scene::new(options);
    s.enter_buddy("Buddy");
    assert!(s.contacts(ContactsInput::Show).is_empty());
    assert!(s.contacts(ContactsInput::Add { name: "Buddy".into() }).is_empty());
    assert!(
        s.contacts(ContactsInput::Delete { name: "Buddy".into() })
            .is_empty()
    );
    assert!(s.contacts(ContactsInput::FriendListExtended).is_empty());
    assert!(s.contact_names().is_empty());
}

// ---- the friend list of the mail window --------------------------------------------------------

/// `FriendListExtended`: a friend who is in the world is answered at once from the world; one who
/// is not is read from the database first (`FriendSummariesRequest`), and the packet is built when
/// the rows are in, in friend list order.
#[test]
fn the_extended_friend_list_asks_for_the_offline_friends() {
    let root = Root::new("friends", false);
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(Recorder(log.clone())));
    world.set_static_data(Box::new(Data(stat_data())));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_zone_data(
        Arc::new(Zones::load(&root.0)),
        Arc::new(MapRegions::load(&root.0)),
    );
    let profile = PlayerProfile {
        friends: vec![
            FriendEntry {
                object_id: 0x1000_0042,
                name: Some("Stranger".into()),
            },
            FriendEntry {
                object_id: BUDDY.0,
                name: Some("Buddy".into()),
            },
            FriendEntry {
                object_id: 0x1000_0043,
                name: Some("Gone".into()),
            },
        ],
        ..PlayerProfile::default()
    };
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(player_data(ME, "Hero", vec![], profile)),
        sink: Box::new(Sink(log.clone())),
        ip: None,
    });
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(2),
        player: Box::new(player_data(BUDDY, "Buddy", vec![], PlayerProfile::default())),
        sink: Box::new(Sink(log.clone())),
        ip: None,
    });
    for _ in 0..20 {
        world.tick();
    }
    log.take();

    world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Contacts(ContactsInput::FriendListExtended),
    });
    let out = log.take();
    // Nothing is sent yet; the rows of the two friends who are not in the world are asked for.
    assert!(packets(&out, "FRIEND_LIST").is_empty());
    let asked: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::FriendSummariesRequest { requester, friends }) => {
                Some((*requester, friends.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(asked, [(ME, vec![0x1000_0042, 0x1000_0043])]);

    // The database knows the first one only (the read of the second found no row).
    world.handle(WorldCommand::FriendSummariesLoaded {
        requester: ME,
        rows: vec![FriendSummary {
            object_id: 0x1000_0042,
            name: "Stranger".into(),
            online: false,
            class_id: 18,
            level: 61,
        }],
    });
    let out = log.take();
    let list = decoded::<FriendListExtended>(&out, "FRIEND_LIST");
    assert_eq!(
        list,
        [FriendListExtended {
            friends: vec![
                FriendListExtendedEntry {
                    object_id: 0x1000_0042,
                    name: "Stranger".into(),
                    online: false,
                    class_id: 18,
                    level: 61
                },
                // The friend in the world: class and level from the world.
                FriendListExtendedEntry {
                    object_id: BUDDY.0,
                    name: "Buddy".into(),
                    online: true,
                    class_id: 2,
                    level: 40
                },
            ]
        }]
    );
    assert!(!names(&out).is_empty());
}
