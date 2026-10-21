#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Private trade (M6-TRADE-01): `TradeRequest`, `AnswerTradeRequest`, `AddTradeItem`, `TradeDone`
//! and the exchange of `TradeList`, with the messages, packets and item rows of Java's
//! `TradeList` / `Player`. The expected values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    InventoryUpdate, SendTradeRequest, StatusUpdate, SystemMessage, SystemMessageParam, TradeDone,
    TradeOtherAdd, TradeOwnAdd, TradeStart, TradeUpdate,
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
const ADENA: i32 = 57;
const PILE: i32 = 23;
const ANVIL: i32 = 20;
const QUEST: i32 = 21;
const CUIRASS: i32 = 4;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="3" type="EtcItem" name="Bound"><set name="is_tradable" val="false" /><set name="weight" val="1" /></item>
<item id="4" type="Armor" name="Cuirass"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="20" type="EtcItem" name="Anvil"><set name="weight" val="1000" /><set name="is_stackable" val="true" /></item>
<item id="21" type="EtcItem" name="Quest thing"><set name="is_questitem" val="true" /><set name="is_stackable" val="true" /></item>
<item id="23" type="EtcItem" name="Pile"><set name="is_stackable" val="true" /><set name="weight" val="2" /></item>
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
    fn access_level_allows_transaction(&self, level: i32) -> bool {
        // `AccessLevels.xml`: levels 20 to 50 are the GMs that cannot trade.
        !(20..=50).contains(&level)
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
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);
const C: ObjectId = ObjectId(3);

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

fn player(id: ObjectId, name: &str, x: i32, items: Vec<ItemData>) -> PlayerData {
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
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y: 1000,
        z: 0,
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

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new() -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        Self { world, log }
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

    /// Alice (1) and Bob (2) stand 50 apart; what the enter sent is dropped.
    fn pair(alice: Vec<ItemData>, bob: Vec<ItemData>) -> Self {
        let mut s = Self::new();
        s.enter(player(A, "Alice", 1000, alice));
        s.enter(player(B, "Bob", 1050, bob));
        s.log.take();
        s
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn request(&mut self, from: ObjectId, to: ObjectId) {
        self.input(from, ClientInput::TradeRequest { object_id: to.0 });
    }

    fn answer(&mut self, who: ObjectId, response: i32) {
        self.input(who, ClientInput::AnswerTradeRequest { response });
    }

    fn add(&mut self, who: ObjectId, object_id: i32, count: i64) {
        self.input(
            who,
            ClientInput::AddTradeItem {
                trade_id: 0,
                object_id,
                count,
            },
        );
    }

    fn done(&mut self, who: ObjectId, response: i32) {
        self.input(who, ClientInput::TradeDone { response });
    }

    /// Alice asks, Bob accepts; the log is empty afterwards.
    fn begin_trade(&mut self) {
        self.request(A, B);
        self.answer(B, 1);
        self.log.take();
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn items_of(&self, who: ObjectId) -> Vec<(i32, i32, i64)> {
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

/// The packets one client got, in order.
fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn names(out: &[Out], to: ObjectId) -> Vec<String> {
    packets(out, to).into_iter().map(name_of).collect()
}

fn of<'a>(out: &'a [Out], to: ObjectId, name: &str) -> Vec<&'a [u8]> {
    packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == name)
        .collect()
}

fn messages(out: &[Out], to: ObjectId) -> Vec<SystemMessage> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

fn message_ids(out: &[Out], to: ObjectId) -> Vec<SystemMessageId> {
    messages(out, to).into_iter().map(|m| m.id).collect()
}

/// `(owner, upserted (object id, loc), removed object ids)` of every `ItemChanges`.
type Rows = (i32, Vec<(i32, String)>, Vec<i32>);

fn item_changes(out: &[Out]) -> Vec<Rows> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => Some((
                owner.0,
                upserts.iter().map(|i| (i.object_id.0, i.loc.clone())).collect(),
                removed.iter().map(|o| o.0).collect(),
            )),
            _ => None,
        })
        .collect()
}

// ---- TradeRequest --------------------------------------------------------------------------------

#[test]
fn a_request_reaches_the_partner_and_tells_the_requester() {
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    let out = s.log.take();
    // `partner.sendPacket(new SendTradeRequest(player.getObjectId()))`
    let request: SendTradeRequest = decode(of(&out, B, "TRADE_REQUEST")[0]);
    assert_eq!(request.sender_id, 1);
    assert_eq!(names(&out, B), ["TRADE_REQUEST"]);
    // YOU_HAVE_REQUESTED_A_TRADE_WITH_C1 (118) with the name as a string.
    let m = messages(&out, A);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].id, SystemMessageId::YouHaveRequestedATradeWithC1);
    assert_eq!(m[0].params, [SystemMessageParam::Text("Bob".into())]);
}

#[test]
fn a_request_to_oneself_or_a_missing_object_is_refused_like_java() {
    let mut s = Scene::pair(vec![], vec![]);
    // THAT_IS_AN_INCORRECT_TARGET (144).
    s.request(A, A);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::ThatIsAnIncorrectTarget]);
    // No object: ignored without a message.
    s.input(A, ClientInput::TradeRequest { object_id: 987_654 });
    assert!(s.log.take().is_empty());
}

#[test]
fn a_request_out_of_range_says_the_target_is_too_far() {
    let mut s = Scene::pair(vec![], vec![]);
    // 151 away in a straight line: `calculateDistance3D(partner) > 150`.
    s.world.set_position(B, 1151, 1000, 0);
    s.log.take();
    s.request(A, B);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::YourTargetIsOutOfRange]);
    assert!(packets(&out, B).is_empty());
    // 150 is still fine.
    s.world.set_position(B, 1150, 1000, 0);
    s.log.take();
    s.request(A, B);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouHaveRequestedATradeWithC1]
    );
}

#[test]
fn a_second_request_while_one_is_open_is_refused() {
    let mut s = Scene::pair(vec![], vec![]);
    let mut c = player(C, "Cleo", 1020, vec![]);
    c.name = "Cleo".into();
    s.enter(c);
    s.request(A, B);
    s.log.take();
    // `player.isProcessingTransaction()`: A has a request open.
    s.request(A, C);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouAreAlreadyTradingWithSomeone]
    );
    // B has a requester, so C hears that B is busy, with B's name.
    s.request(C, B);
    let out = s.log.take();
    let m = messages(&out, C);
    assert_eq!(
        m[0].id,
        SystemMessageId::C1IsAlreadyTradingWithAnotherPersonPleaseTryAgainLater
    );
    assert_eq!(m[0].params, [SystemMessageParam::Text("Bob".into())]);
}

#[test]
fn the_request_flags_karma_refusal_and_access_level() {
    let mut s = Scene::pair(vec![], vec![]);
    // `That person is in trade refusal mode.`
    s.world.player_mut(B).unwrap().trade.refusal = true;
    s.request(A, B);
    let out = s.log.take();
    let text = of(&out, A, "SYSTEM_MESSAGE");
    assert_eq!(text.len(), 1);
    assert_eq!(
        decode::<SystemMessage>(text[0]).params,
        [SystemMessageParam::Text(
            "That person is in trade refusal mode.".into()
        )]
    );
    assert!(packets(&out, B).is_empty());
    s.world.player_mut(B).unwrap().trade.refusal = false;

    // AltKarmaPlayerCanTrade = False: a chaotic requester or partner.
    s.world.set_settings(WorldSettings {
        inventory: InventorySettings {
            alt_game_karma_player_can_trade: false,
            ..InventorySettings::default()
        },
        ..WorldSettings::default()
    });
    s.world.player_mut(A).unwrap().data.karma = 5;
    s.request(A, B);
    let m = messages(&s.log.take(), A);
    assert_eq!(
        m[0].params,
        [SystemMessageParam::Text(
            "You cannot trade while you are in a chaotic state.".into()
        )]
    );
    s.world.player_mut(A).unwrap().data.karma = 0;
    s.world.player_mut(B).unwrap().data.karma = 5;
    s.request(A, B);
    let m = messages(&s.log.take(), A);
    assert_eq!(
        m[0].params,
        [SystemMessageParam::Text(
            "You cannot request a trade while your target is in a chaotic state.".into()
        )]
    );

    // A level that may not trade (`allowTransaction="false"`): message and ActionFailed.
    let mut s = Scene::pair(vec![], vec![]);
    s.world.player_mut(A).unwrap().data.access_level = 30;
    s.request(A, B);
    let out = s.log.take();
    assert_eq!(names(&out, A), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert!(packets(&out, B).is_empty());
}

// ---- AnswerTradeRequest -------------------------------------------------------------------------

#[test]
fn accepting_opens_the_window_for_both_with_the_tradeable_items() {
    let sword = inv(100, SWORD, 1);
    let mut worn = inv(101, CUIRASS, 1);
    worn.loc = "PAPERDOLL".into();
    worn.loc_data = 10;
    let items = vec![
        sword.clone(),
        worn,
        inv(102, BOUND, 1),
        inv(103, QUEST, 3),
        inv(104, ADENA, 500),
        inv(105, PILE, 7),
    ];
    let mut s = Scene::pair(items, vec![inv(200, PILE, 1)]);
    s.request(A, B);
    s.log.take();
    s.answer(B, 1);
    let out = s.log.take();

    // Player.startTrade: the asked player (B) first, then the requester.
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "TRADE_START"]);
    assert_eq!(names(&out, A), ["SYSTEM_MESSAGE", "TRADE_START"]);
    let m = messages(&out, B);
    assert_eq!(m[0].id, SystemMessageId::YouBeginTradingWithC1);
    assert_eq!(m[0].params, [SystemMessageParam::PlayerName("Alice".into())]);
    let m = messages(&out, A);
    assert_eq!(m[0].params, [SystemMessageParam::PlayerName("Bob".into())]);

    // `getAvailableItems(true, ...)`: adena and the stackable pile and the sword; not the worn
    // cuirass, the untradeable item or the quest item.
    let start: TradeStart = decode(of(&out, A, "TRADE_START")[0]);
    assert_eq!(start.partner_object_id, 2);
    let listed: Vec<(i32, i32)> = start.items.iter().map(|i| (i.object_id, i.item_id)).collect();
    assert_eq!(listed, [(100, SWORD), (104, ADENA), (105, PILE)]);
    let start: TradeStart = decode(of(&out, B, "TRADE_START")[0]);
    assert_eq!(start.partner_object_id, 1);
    assert_eq!(start.items.len(), 1);

    // Both have a list, with each other as the partner.
    assert_eq!(
        s.world.player(A).unwrap().trade.active.as_ref().unwrap().partner,
        Some(B)
    );
    assert_eq!(
        s.world.player(B).unwrap().trade.active.as_ref().unwrap().partner,
        Some(A)
    );
    // The request is answered: nobody is the requester any more.
    assert_eq!(s.world.player(B).unwrap().trade.active_requester, None);
    assert_eq!(s.world.player(A).unwrap().trade.request_expire_time, 0);
}

#[test]
fn declining_tells_the_requester() {
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    s.log.take();
    s.answer(B, 0);
    let out = s.log.take();
    // C1_HAS_DENIED_YOUR_REQUEST_TO_TRADE (119) with the asked player's name.
    let m = messages(&out, A);
    assert_eq!(m[0].id, SystemMessageId::C1HasDeniedYourRequestToTrade);
    assert_eq!(m[0].params, [SystemMessageParam::Text("Bob".into())]);
    assert!(packets(&out, B).is_empty());
    assert!(s.world.player(A).unwrap().trade.active.is_none());
    assert_eq!(s.world.player(B).unwrap().trade.active_requester, None);
    // The requester can ask again at once (`onTransactionResponse`).
    s.request(A, B);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouHaveRequestedATradeWithC1]
    );
}

#[test]
fn an_answer_without_a_request_is_refused() {
    let mut s = Scene::pair(vec![], vec![]);
    s.answer(B, 1);
    let out = s.log.take();
    // TradeDone(0) then THAT_PLAYER_IS_NOT_ONLINE (145).
    assert_eq!(names(&out, B), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(decode::<TradeDone>(of(&out, B, "TRADE_DONE")[0]).num, 0);
    assert_eq!(message_ids(&out, B), [SystemMessageId::ThatPlayerIsNotOnline]);
}

#[test]
fn a_request_runs_out_after_fifteen_seconds() {
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    s.log.take();
    // `REQUEST_TIMEOUT` is 15 s of game ticks.
    s.advance(14_900);
    assert!(s.world.player_mut(B).is_some());
    s.advance(200);
    s.answer(B, 1);
    let out = s.log.take();
    // `getActiveRequester()` forgot the expired request: the answer finds nobody.
    assert_eq!(message_ids(&out, B), [SystemMessageId::ThatPlayerIsNotOnline]);
    assert!(s.world.player(A).unwrap().trade.active.is_none());
    assert!(s.world.player(B).unwrap().trade.active.is_none());
}

// ---- AddTradeItem -------------------------------------------------------------------------------

#[test]
fn adding_an_item_shows_it_in_both_windows() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1), inv(105, PILE, 10)], vec![]);
    s.begin_trade();

    // A whole item: TradeOwnAdd, TradeUpdate (2: nothing of a stack left), partner TradeOtherAdd.
    s.add(A, 100, 1);
    let out = s.log.take();
    assert_eq!(names(&out, A), ["TRADE_OWN_ADD", "TRADE_UPDATE"]);
    assert_eq!(names(&out, B), ["TRADE_OTHER_ADD"]);
    let own: TradeOwnAdd = decode(of(&out, A, "TRADE_OWN_ADD")[0]);
    let other: TradeOtherAdd = decode(of(&out, B, "TRADE_OTHER_ADD")[0]);
    assert_eq!(own.item, other.item);
    // `writeTradeItem`: type1 0 (weapon), the id, the offered count, type2 0, body part rhand.
    assert_eq!(own.item.type1, 0);
    assert_eq!(own.item.object_id, 100);
    assert_eq!(own.item.item_id, SWORD);
    assert_eq!(own.item.count, 1);
    assert_eq!(own.item.type2, 0);
    assert_eq!(own.item.body_part, 0x80);
    let update: TradeUpdate = decode(of(&out, A, "TRADE_UPDATE")[0]);
    assert_eq!(update.change, 2);

    // Part of a stack: change 3 (`(newCount > 0) && stackable`), the offered count is shown.
    s.add(A, 105, 4);
    let out = s.log.take();
    let update: TradeUpdate = decode(of(&out, A, "TRADE_UPDATE")[0]);
    assert_eq!(update.change, 3);
    assert_eq!(update.item.count, 4);
    let list = s.world.player(A).unwrap().trade.active.as_ref().unwrap();
    assert_eq!(list.item_count(), 2);
    // The inventory is not touched until the exchange.
    assert_eq!(s.items_of(A), [(100, SWORD, 1), (105, PILE, 10)]);
}

#[test]
fn items_java_refuses_are_not_added() {
    let mut s = Scene::pair(
        vec![
            inv(100, SWORD, 1),
            inv(102, BOUND, 1),
            inv(103, QUEST, 3),
            inv(105, PILE, 10),
        ],
        vec![inv(200, SWORD, 1)],
    );
    s.begin_trade();
    // `addItem` returns null: nothing is sent.
    for (object_id, count) in [(102, 1), (103, 1), (105, 11), (100, 2), (105, 0)] {
        s.add(A, object_id, count);
        assert!(s.log.take().is_empty(), "item {object_id} x{count}");
    }
    // The same object twice: the second is ignored.
    s.add(A, 100, 1);
    s.log.take();
    s.add(A, 100, 1);
    assert!(s.log.take().is_empty());
    // Somebody else's item: `validateItemManipulation` -> NOTHING_HAPPENED (61).
    s.add(A, 200, 1);
    assert_eq!(message_ids(&s.log.take(), A), [SystemMessageId::NothingHappened]);
}

#[test]
fn nothing_can_be_added_once_a_side_confirmed() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1), inv(105, PILE, 10)], vec![]);
    s.begin_trade();
    s.add(A, 100, 1);
    s.log.take();
    s.done(B, 1);
    s.log.take();
    // YOU_MAY_NO_LONGER_ADJUST_ITEMS_IN_THE_TRADE_BECAUSE_THE_TRADE_HAS_BEEN_CONFIRMED (122) for
    // the partner's confirmation too.
    s.add(A, 105, 1);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouMayNoLongerAdjustItemsInTheTradeBecauseTheTradeHasBeenConfirmed]
    );
}

#[test]
fn adding_without_a_trade_does_nothing() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.add(A, 100, 1);
    assert!(s.log.take().is_empty());
}

// ---- confirm and cancel -------------------------------------------------------------------------

#[test]
fn the_first_confirmation_tells_the_partner() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.begin_trade();
    s.done(A, 1);
    let out = s.log.take();
    // C1_HAS_CONFIRMED_THE_TRADE (121) and TradeOtherDone for the partner only.
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "TRADE_PRESS_OTHER_OK"]);
    let m = messages(&out, B);
    assert_eq!(m[0].id, SystemMessageId::C1HasConfirmedTheTrade);
    assert_eq!(m[0].params, [SystemMessageParam::PlayerName("Alice".into())]);
    assert!(packets(&out, A).is_empty());
    // A second press of the same player changes nothing.
    s.done(A, 1);
    assert!(s.log.take().is_empty());
}

#[test]
fn adding_an_item_withdraws_the_own_confirmation() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1), inv(105, PILE, 10)], vec![]);
    s.begin_trade();
    s.done(A, 1);
    // Java: `invalidateConfirmation()` when the list changes... but the add itself is refused
    // while a side is confirmed, so the flag is still set.
    s.add(A, 100, 1);
    assert!(
        s.world
            .player(A)
            .unwrap()
            .trade
            .active
            .as_ref()
            .unwrap()
            .confirmed
    );
}

#[test]
fn cancelling_closes_both_windows_with_javas_messages() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.begin_trade();
    s.add(A, 100, 1);
    s.log.take();
    s.done(A, 0);
    let out = s.log.take();
    // `partner.onTradeCancel(this)`, then `onTradeCancel(this)`: TradeDone(0) and
    // C1_HAS_CANCELLED_THE_TRADE (124) each.
    assert_eq!(names(&out, B), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(names(&out, A), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(decode::<TradeDone>(of(&out, B, "TRADE_DONE")[0]).num, 0);
    let m = messages(&out, B);
    assert_eq!(m[0].id, SystemMessageId::C1HasCancelledTheTrade);
    assert_eq!(m[0].params, [SystemMessageParam::PlayerName("Alice".into())]);
    // NOTE(java-quirk): the canceller is told its own name.
    let m = messages(&out, A);
    assert_eq!(m[0].params, [SystemMessageParam::PlayerName("Alice".into())]);
    assert!(s.world.player(A).unwrap().trade.active.is_none());
    assert!(s.world.player(B).unwrap().trade.active.is_none());
    // The items never moved and nothing was written.
    assert_eq!(s.items_of(A), [(100, SWORD, 1)]);
    assert!(item_changes(&out).is_empty());
}

#[test]
fn walking_away_cancels_the_trade_when_the_player_confirms() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.begin_trade();
    s.add(A, 100, 1);
    // Java does nothing when a player walks off; `TradeDone` checks the 150 range.
    s.world.set_position(B, 1151, 1000, 0);
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();
    assert_eq!(names(&out, A), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(names(&out, B), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(s.items_of(A), [(100, SWORD, 1)]);
}

// ---- the exchange -------------------------------------------------------------------------------

/// `(object id, change, count)` of the entries of every `InventoryUpdate` one client got, sorted.
fn inventory_updates(out: &[Out], to: ObjectId) -> Vec<(i32, i16, i64)> {
    let mut entries: Vec<(i32, i16, i64)> = of(out, to, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.item.object_id, u.change, u.item.count))
        .collect();
    entries.sort_unstable();
    entries
}

fn cur_load(raw: &[u8]) -> i32 {
    let update: StatusUpdate = decode(raw);
    assert_eq!(update.attributes.len(), 1);
    update.attributes[0].1
}

#[test]
fn both_confirming_swaps_the_items_and_writes_the_rows() {
    let mut s = Scene::pair(
        vec![inv(100, SWORD, 1), inv(105, PILE, 10), inv(104, ADENA, 1000)],
        vec![inv(200, PILE, 3), inv(201, CUIRASS, 1)],
    );
    s.begin_trade();
    // A gives the sword and 4 of its 10 piles; B gives the cuirass and its 3 piles.
    s.add(A, 100, 1);
    s.add(A, 105, 4);
    s.add(B, 201, 1);
    s.add(B, 200, 3);
    s.done(A, 1);
    s.log.take();
    s.done(B, 1);
    let out = s.log.take();

    // `doExchange` runs on the second confirmer's list (B): the first confirmer's (A's) items
    // move first. The pile joins B's stack (3 + 4 = 7); then 3 of B's 7 piles go to A's stack
    // (6 + 3 = 9), so the stacks swap back and forth like Java's.
    assert_eq!(
        s.items_of(A),
        [(104, ADENA, 1000), (105, PILE, 9), (201, CUIRASS, 1)]
    );
    assert_eq!(s.items_of(B), [(100, SWORD, 1), (200, PILE, 4)]);

    // The rows, in the order of `updateDatabase`: a whole item leaves its owner (DELETE) and the
    // new owner gets the row (INSERT with the new owner); a stack that changes is an UPDATE.
    let rows = item_changes(&out);
    let inventory =
        |ids: &[i32]| -> Vec<(i32, String)> { ids.iter().map(|i| (*i, "INVENTORY".to_owned())).collect() };
    assert_eq!(
        rows,
        [
            (1, vec![], vec![100]),
            (2, inventory(&[100]), vec![]),
            (1, inventory(&[105]), vec![]),
            (2, inventory(&[200]), vec![]),
            (2, vec![], vec![201]),
            (1, inventory(&[201]), vec![]),
            (2, inventory(&[200]), vec![]),
            (1, inventory(&[105]), vec![]),
        ]
    );

    // Immediate packets: the load of the second confirmer, the load of the first, then
    // `onTradeFinish` for the first confirmer and the second: TradeDone(1) and the success message.
    for who in [A, B] {
        assert_eq!(
            names(&out, who),
            ["STATUS_UPDATE", "TRADE_DONE", "SYSTEM_MESSAGE"],
            "{who:?}"
        );
        assert_eq!(decode::<TradeDone>(of(&out, who, "TRADE_DONE")[0]).num, 1);
        assert_eq!(message_ids(&out, who), [SystemMessageId::YourTradeWasSuccessful]);
    }
    // A: 9 piles (2 each) and the cuirass (10); B: 4 piles and the sword (50).
    assert_eq!(cur_load(of(&out, A, "STATUS_UPDATE")[0]), 28);
    assert_eq!(cur_load(of(&out, B, "STATUS_UPDATE")[0]), 58);
    assert!(s.world.player(A).unwrap().trade.active.is_none());
    assert!(s.world.player(B).unwrap().trade.active.is_none());

    // `sendInventoryUpdate` waits 100 ms, `sendItemList(false)` 250 ms.
    s.advance(100);
    let later = s.log.take();
    const ADDED: i16 = InventoryUpdate::ADDED;
    const MODIFIED: i16 = InventoryUpdate::MODIFIED;
    const REMOVED: i16 = InventoryUpdate::REMOVED;
    assert_eq!(
        inventory_updates(&later, A),
        [(100, REMOVED, 1), (105, MODIFIED, 9), (201, ADDED, 1)]
    );
    assert_eq!(
        inventory_updates(&later, B),
        [(100, ADDED, 1), (200, MODIFIED, 4), (201, REMOVED, 1)]
    );
    s.advance(150);
    let later = s.log.take();
    for who in [A, B] {
        assert_eq!(names(&later, who), ["ITEM_LIST", "EX_QUEST_ITEM_LIST"], "{who:?}");
    }
}

#[test]
fn a_part_of_a_stack_becomes_a_new_item_with_an_id_from_the_rust_range() {
    let mut s = Scene::pair(vec![inv(105, PILE, 10)], vec![]);
    s.begin_trade();
    s.add(A, 105, 4);
    s.done(B, 1);
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();

    let a = s.items_of(A);
    assert_eq!(a, [(105, PILE, 6)]);
    let b = s.items_of(B);
    assert_eq!(b.len(), 1);
    let (new_id, item_id, count) = b[0];
    // D-39: the Java GS never allocates here.
    assert!(new_id >= RUST_FIRST_OBJECT_ID, "{new_id:#x}");
    assert_eq!((item_id, count), (PILE, 4));

    // `updateDatabase(true)` of the source (an UPDATE), then `updateDatabase()` of the new item
    // (an INSERT: the persistence task has never seen the row).
    let rows = item_changes(&out);
    assert_eq!(
        rows,
        [
            (1, vec![(105, "INVENTORY".to_owned())], vec![]),
            (2, vec![(new_id, "INVENTORY".to_owned())], vec![]),
        ]
    );

    // NOTE(java-quirk): `PlayerInventory.addItem(itemId, count, actor)` tells the actor (the
    // giver), not the receiver, about the new item: a StatusUpdate now and an InventoryUpdate
    // entry for the new object (ADDED) 100 ms later, next to A's own entries.
    let to_a = names(&out, A);
    assert_eq!(
        to_a,
        ["STATUS_UPDATE", "STATUS_UPDATE", "TRADE_DONE", "SYSTEM_MESSAGE"]
    );
    s.advance(100);
    let later = s.log.take();
    assert_eq!(
        inventory_updates(&later, A),
        [
            (105, InventoryUpdate::MODIFIED, 6),
            (new_id, InventoryUpdate::ADDED, 4)
        ]
    );
    assert_eq!(
        inventory_updates(&later, B),
        [(new_id, InventoryUpdate::ADDED, 4)]
    );
}

#[test]
fn giving_a_whole_stack_to_somebody_without_one_destroys_it_and_creates_a_new_item() {
    let mut s = Scene::pair(vec![inv(105, PILE, 10)], vec![]);
    s.begin_trade();
    s.add(A, 105, 10);
    s.done(B, 1);
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();
    assert!(s.items_of(A).is_empty());
    let b = s.items_of(B);
    assert_eq!((b[0].1, b[0].2), (PILE, 10));
    // `ItemManager.destroyItem` removes the old row at once; the new item is inserted.
    let rows = item_changes(&out);
    assert_eq!(
        rows,
        [
            (1, vec![], vec![105]),
            (2, vec![(b[0].0, "INVENTORY".to_owned())], vec![]),
        ]
    );
}

#[test]
fn too_much_weight_stops_the_exchange_and_closes_the_windows() {
    // 1000 anvils of weight 1000 are far over what B can carry.
    let mut s = Scene::pair(vec![inv(100, ANVIL, 1000)], vec![]);
    s.begin_trade();
    s.add(A, 100, 1000);
    s.done(B, 1);
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();
    // YOU_HAVE_EXCEEDED_THE_WEIGHT_LIMIT (422) to the first confirmer, then the second; then
    // `onTradeFinish(false)`: TradeDone(1) without the success message.
    for who in [A, B] {
        assert_eq!(
            message_ids(&out, who),
            [SystemMessageId::YouHaveExceededTheWeightLimit],
            "{who:?}"
        );
        assert_eq!(names(&out, who), ["SYSTEM_MESSAGE", "TRADE_DONE"]);
        assert_eq!(decode::<TradeDone>(of(&out, who, "TRADE_DONE")[0]).num, 1);
    }
    assert_eq!(s.items_of(A), [(100, ANVIL, 1000)]);
    assert!(s.items_of(B).is_empty());
    assert!(item_changes(&out).is_empty());
    assert!(s.world.player(A).unwrap().trade.active.is_none());
}

#[test]
fn a_full_inventory_stops_the_exchange() {
    // Bob has 80 swords, the inventory limit of a non-dwarf.
    let swords: Vec<ItemData> = (0..80).map(|i| inv(300 + i, SWORD, 1)).collect();
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], swords);
    s.begin_trade();
    s.add(A, 100, 1);
    s.done(B, 1);
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();
    for who in [A, B] {
        assert_eq!(
            message_ids(&out, who),
            [SystemMessageId::YourInventoryIsFull],
            "{who:?}"
        );
        assert_eq!(names(&out, who), ["SYSTEM_MESSAGE", "TRADE_DONE"]);
    }
    assert_eq!(s.items_of(A), [(100, SWORD, 1)]);
    assert_eq!(s.items_of(B).len(), 80);
}

#[test]
fn an_item_that_vanished_leaves_the_trade_locked_and_silent() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.begin_trade();
    s.add(A, 100, 1);
    s.done(B, 1);
    // The sword is gone before the second confirmation (`validate()` -> `checkItemManipulation`).
    s.world.player_mut(A).unwrap().data.items.clear();
    s.log.take();
    s.done(A, 1);
    let out = s.log.take();
    // NOTE(java-quirk): both lists stay confirmed and locked and nobody is told.
    assert!(out.is_empty(), "{out:?}");
    for who in [A, B] {
        let list = s.world.player(who).unwrap().trade.active.as_ref().unwrap();
        assert!(list.locked && list.confirmed);
    }
    // A locked list ignores `TradeDone` (`trade.isLocked()`), the cancel of the client included:
    // Java has the same dead end.
    s.done(A, 0);
    assert!(s.log.take().is_empty());
}

// ---- leaving, and the other packets that look at a trade -----------------------------------------

#[test]
fn a_player_who_leaves_in_the_middle_of_a_trade_does_not_cancel_it() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1)], vec![]);
    s.begin_trade();
    // NOTE(java-quirk): `deleteMe` cancels only `if (getActiveRequester() != null)`, and a trade
    // that has begun has no requester.
    s.input(B, ClientInput::Logout);
    let out = s.log.take();
    assert!(of(&out, A, "TRADE_DONE").is_empty());
    assert!(s.world.player(A).unwrap().trade.active.is_some());

    // The first time the partner touches the trade, it learns that the other is gone.
    s.add(A, 100, 1);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [
            SystemMessageId::ThatPlayerIsNotOnline,
            SystemMessageId::C1HasCancelledTheTrade
        ]
    );
    assert_eq!(names(&out, A), ["SYSTEM_MESSAGE", "TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert!(s.world.player(A).unwrap().trade.active.is_none());
}

#[test]
fn a_request_to_a_player_who_left_finds_nobody() {
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    s.input(A, ClientInput::Logout);
    s.log.take();
    s.answer(B, 1);
    let out = s.log.take();
    assert_eq!(names(&out, B), ["TRADE_DONE", "SYSTEM_MESSAGE"]);
    assert_eq!(message_ids(&out, B), [SystemMessageId::ThatPlayerIsNotOnline]);
    assert!(s.world.player(B).unwrap().trade.active.is_none());
}

#[test]
fn leaving_with_an_open_request_sends_nothing_to_the_requester() {
    // The player who was asked leaves while the request is open: `getActiveRequester() != null`,
    // so `setActiveRequester(null)` and `cancelActiveTrade()` (no trade, nothing is sent).
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    s.log.take();
    s.input(B, ClientInput::Logout);
    let out = s.log.take();
    assert!(of(&out, A, "TRADE_DONE").is_empty());
}

#[test]
fn items_cannot_be_used_destroyed_or_dropped_in_a_transaction() {
    let mut s = Scene::pair(vec![inv(100, SWORD, 1), inv(105, PILE, 10)], vec![]);
    // An open request already counts (`isProcessingTransaction`).
    s.request(A, B);
    s.log.take();
    s.input(
        A,
        ClientInput::DestroyItem {
            object_id: 105,
            count: 1,
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem]
    );
    s.input(
        A,
        ClientInput::DropItem {
            object_id: 105,
            count: 1,
            x: 1000,
            y: 1000,
            z: 0,
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem]
    );
    assert_eq!(s.items_of(A).len(), 2);

    // In the window an item cannot be used (`getActiveTradeList() != null`).
    s.answer(B, 1);
    s.log.take();
    s.input(
        A,
        ClientInput::UseItem {
            object_id: 100,
            ctrl_pressed: false,
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouCannotPickUpOrUseItemsWhileTrading]
    );
    assert!(
        s.world
            .player(A)
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.loc == "INVENTORY")
    );
}

#[test]
fn a_level_that_may_not_trade_cannot_answer() {
    let mut s = Scene::pair(vec![], vec![inv(200, SWORD, 1)]);
    s.request(A, B);
    s.log.take();
    s.world.player_mut(B).unwrap().data.access_level = 40;
    s.answer(B, 1);
    let out = s.log.take();
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(
        message_ids(&out, B),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
}

// ---- TradeList itself ----------------------------------------------------------------------------

#[test]
fn a_trade_list_applies_javas_add_item_rules() {
    let templates = ItemTemplates::from_xml(ITEMS).unwrap();
    let sword = templates.get(SWORD).unwrap();
    let pile = templates.get(PILE).unwrap();
    let quest = templates.get(QUEST).unwrap();
    let rules = AddItemRules {
        tradeable: true,
        max_adena: 99_900_000_000,
    };
    let mut list = TradeList::new(A);
    list.confirmed = true;
    let stack = inv(105, PILE, 10);

    // More than the stack holds, zero, a quest item and a non-stackable item with count 2.
    assert!(list.add_item(&stack, pile, 11, 0, rules).is_none());
    assert!(list.add_item(&stack, pile, 0, 0, rules).is_none());
    assert!(list.add_item(&inv(103, QUEST, 3), quest, 1, 0, rules).is_none());
    assert!(list.add_item(&inv(100, SWORD, 1), sword, 2, 0, rules).is_none());
    // `MAX_ADENA / count < price`.
    assert!(list.add_item(&stack, pile, 10, 9_990_000_001, rules).is_none());
    assert!(list.confirmed, "a refused item keeps the confirmation");
    // Not tradeable (`isTradeable()` false and no GM exception).
    let refuse = AddItemRules {
        tradeable: false,
        ..rules
    };
    assert!(list.add_item(&stack, pile, 1, 0, refuse).is_none());

    // A valid item withdraws the confirmation of the list's owner.
    let entry = list.add_item(&stack, pile, 4, 0, rules).unwrap();
    assert_eq!((entry.object_id.0, entry.count, entry.price), (105, 4, 0));
    assert!(!list.confirmed);
    // The same object again, and anything once the list is locked.
    assert!(list.add_item(&stack, pile, 1, 0, rules).is_none());
    list.lock();
    assert!(list.add_item(&inv(100, SWORD, 1), sword, 1, 0, rules).is_none());
    list.clear();
    assert!(!list.locked && list.item_count() == 0);
}

#[test]
fn a_player_who_was_asked_cannot_click_until_the_request_is_over() {
    let mut s = Scene::pair(vec![], vec![]);
    s.request(A, B);
    s.log.take();
    // `Action.runImpl`: `player.getActiveRequester() != null` -> ActionFailed.
    s.input(
        B,
        ClientInput::Action {
            object_id: A.0,
            action_id: 0,
        },
    );
    let out = s.log.take();
    assert_eq!(names(&out, B), ["ACTION_FAIL"]);
    assert_eq!(s.world.target_of(B), None);
    // The requester itself may click: only the asked player is blocked.
    s.input(
        A,
        ClientInput::Action {
            object_id: B.0,
            action_id: 0,
        },
    );
    assert_eq!(s.world.target_of(A), Some(B));
    // After the answer B can click again.
    s.answer(B, 0);
    s.log.take();
    s.input(
        B,
        ClientInput::Action {
            object_id: A.0,
            action_id: 0,
        },
    );
    assert_eq!(s.world.target_of(B), Some(A));
}
