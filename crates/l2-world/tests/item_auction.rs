#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Item auctions (L-05): an auction from its start to its end with two bidders, the outbid and
//! the returned bid, the winner's item, the rows and messages Java writes, and the side by side
//! switch (D-53): with `CycleDatabaseWrites` off the global rows and the end stay with the Java
//! game server and the rows are read again before they are shown (D-50). Expected values come
//! from `ItemAuction.java`, `ItemAuctionInstance.java`, `ItemAuctionLink.java`,
//! `RequestInfoItemAuction.java`, `RequestBidItemAuction.java` and `ExItemAuctionInfoPacket.java`.
//!
//! The world counts ticks while the auction times are wall-clock times: an auction is set up a
//! few seconds from now and the test ticks past its start and end.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ExItemAuctionInfo, SystemMessage, SystemMessageParam};
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

fn decoded<P: Decode>(out: &[Out], name: &str) -> Vec<P> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) if name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
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

fn writes(out: &[Out]) -> Vec<ItemAuctionWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemAuction(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn requests(out: &[Out]) -> Vec<(i32, ItemAuctionThen)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemAuctionRequest { instance_id, then }) => {
                Some((*instance_id, *then))
            }
            _ => None,
        })
        .collect()
}

fn item_rows(out: &[Out]) -> Vec<ItemRowOp> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemRows { ops, .. }) => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn now_ms() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
}

// ---- data --------------------------------------------------------------------------------------

const ADENA: i32 = 57;
/// A weapon-like item that does not stack.
const FALCHION: i32 = 9901;
/// A stackable item.
const LIFE_STONE: i32 = 9576;
const AUCTIONEER: i32 = 32320;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="9901" type="EtcItem" name="Improved Falchion"><set name="enchanted" val="15" /></item>
<item id="9576" type="EtcItem" name="Life Stone"><set name="is_stackable" val="true" /></item>
</list>"#;

const NPCS: &str = r#"<list>
  <npc id="32320" type="Folk" level="10" name="Auctioneer"/>
</list>"#;

const PAGE: &str = "<html><body>Auction %objectId% \
<a action=\"bypass -h npc_%objectId%_ItemAuction show\">Buy</a>\
<a action=\"bypass -h npc_%objectId%_ItemAuction cancel\">Cancel</a></body></html>";

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

const A: ObjectId = ObjectId(0x2000_0001);
const B: ObjectId = ObjectId(0x2000_0002);
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

fn player_data(object_id: ObjectId, name: &str, adena: i64, warehouse: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id,
        account: format!("acc{}", object_id.0),
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
        items: vec![item(object_id.0 + 0x1000_0000, ADENA, 0, adena)],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile {
            warehouse: WarehouseLoad {
                private: warehouse,
                ..WarehouseLoad::default()
            },
            ..PlayerProfile::default()
        },
    }
}

/// An `ItemAuctions.xml` instance: the item 9901 (+15, 100 adena to start, a minute long) and the
/// stackable life stone (a minute long, 100 adena).
fn instance_def() -> InstanceDef {
    InstanceDef {
        id: AUCTIONEER,
        generator: DateGenerator::new(Some(1), None, Some(12), Some(0)).unwrap(),
        items: vec![
            AuctionItemDef {
                auction_item_id: 1,
                auction_length: 1,
                auction_init_bid: 100,
                item_id: FALCHION,
                item_count: 1,
                augmentation_id: 0,
            },
            AuctionItemDef {
                auction_item_id: 2,
                auction_length: 1,
                auction_init_bid: 100,
                item_id: LIFE_STONE,
                item_count: 3,
                augmentation_id: 5555,
            },
        ],
    }
}

fn record(
    auction_id: i32,
    item: i32,
    start: i64,
    end: i64,
    state: i8,
    bids: &[(i32, i64)],
) -> ItemAuctionRecord {
    ItemAuctionRecord {
        auction_id,
        instance_id: AUCTIONEER,
        auction_item_id: item,
        starting_time: start,
        ending_time: end,
        state_id: state,
        bids: bids.to_vec(),
    }
}

fn settings(cycle: bool) -> ItemAuctionSettings {
    ItemAuctionSettings {
        enabled: true,
        expired_after_days: 14,
        time_extends_on_bid_ms: 0,
        cycle_database_writes: cycle,
    }
}

struct Scene {
    world: World,
    outbox: Log,
    a: Log,
    b: Log,
    npc: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    /// Two characters (A with 1000 adena, B with 1000) next to the auctioneer; `load` is the
    /// auction state at the start, built from the time of the start.
    fn new(settings: ItemAuctionSettings, load: impl FnOnce(i64) -> Vec<ItemAuctionRecord>) -> Self {
        Self::with_warehouse(settings, load, Vec::new())
    }

    fn with_warehouse(
        settings: ItemAuctionSettings,
        load: impl FnOnce(i64) -> Vec<ItemAuctionRecord>,
        a_warehouse: Vec<ItemData>,
    ) -> Self {
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
        write("data/html/default/32320.htm", PAGE);

        let outbox = Log::default();
        let mut world = World::new(
            WorldRng::from_seed(20_261_027),
            Box::new(Recorder(outbox.clone())),
        );
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        let (a, b) = (Log::default(), Log::default());
        for (id, name, adena, log, session, warehouse) in [
            (A, "Alpha", 1000, &a, 1, a_warehouse),
            (B, "Bravo", 1000, &b, 2, Vec::new()),
        ] {
            world.handle(WorldCommand::EnterWorld {
                session: SessionId(session),
                player: Box::new(player_data(id, name, adena, warehouse)),
                sink: Box::new(Sink(log.clone())),
                ip: None,
            });
        }
        let npc = world.spawn_npc_at(AUCTIONEER, X + 20, Y, Z, 0).unwrap();
        world.set_item_auction_settings(settings);
        let auctions = load(now_ms());
        // `SELECT auctionId .. ORDER BY auctionId DESC LIMIT 0, 1` plus one.
        let next_auction_id = auctions.iter().map(|a| a.auction_id).max().map_or(1, |id| id + 1);
        world.load_item_auctions(ItemAuctionLoad {
            next_auction_id,
            instances: vec![instance_def()],
            auctions,
        });
        let mut scene = Self {
            world,
            outbox,
            a,
            b,
            npc,
            _root: root,
        };
        scene.tick(20);
        scene
    }

    fn tick(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.world.tick();
        }
    }

    /// Ticks past the transaction flood window, then drops what was logged.
    fn wait(&mut self) {
        self.tick(40);
    }

    fn clear(&self) {
        self.outbox.take();
        self.a.take();
        self.b.take();
    }

    /// An input of player A (session 1) or B (session 2); the log of that player and the outbox.
    fn input(&mut self, who: ObjectId, input: ClientInput) -> (Vec<Out>, Vec<Out>) {
        self.wait();
        self.clear();
        let session = if who == A { SessionId(1) } else { SessionId(2) };
        self.world.handle(WorldCommand::Input { session, input });
        self.tick(5);
        let player = if who == A { &self.a } else { &self.b };
        (player.take(), self.outbox.take())
    }

    fn bid(&mut self, who: ObjectId, bid: i64) -> (Vec<Out>, Vec<Out>) {
        self.input(
            who,
            ClientInput::ItemAuction(ItemAuctionInput::Bid {
                instance_id: AUCTIONEER,
                bid,
            }),
        )
    }

    /// Opens the auctioneer's html (the links are only valid after it was shown) and returns the
    /// outputs of the following bypass.
    fn bypass(&mut self, who: ObjectId, command: &str) -> (Vec<Out>, Vec<Out>) {
        let npc = self.npc.0;
        self.input(
            who,
            ClientInput::Action {
                object_id: npc,
                action_id: 0,
            },
        );
        // The first click targets, the second talks.
        let (shown, _) = self.input(
            who,
            ClientInput::Action {
                object_id: npc,
                action_id: 0,
            },
        );
        assert!(
            names(&shown).contains(&"NPC_HTML_MESSAGE".to_owned()),
            "{:?}",
            names(&shown)
        );
        self.input(
            who,
            ClientInput::Bypass {
                command: format!("npc_{npc}_{command}"),
            },
        )
    }

    fn adena(&self, who: ObjectId) -> i64 {
        self.world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == ADENA && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    fn warehouse(&self, who: ObjectId) -> Vec<ItemData> {
        self.world.player(who).unwrap().warehouse.private.clone()
    }
}

fn long(value: i64) -> SystemMessageParam {
    SystemMessageParam::LongNumber(value)
}

/// A started auction of the falchion, ending a minute from now, plus the next one tomorrow.
fn started_falchion(now: i64, bids: &[(i32, i64)]) -> Vec<ItemAuctionRecord> {
    vec![
        record(5, 1, now - 10_000, now + 60_000, 1, bids),
        record(6, 1, now + 86_400_000, now + 86_400_000 + 60_000, 0, &[]),
    ]
}

// ---- the whole life of an auction --------------------------------------------------------------

/// A created auction starts, two players bid (the outbid, the raise, the refusals), the loser takes
/// the bid back, the end is extended twice (5 and 3 minutes), the winner finds the item in the
/// warehouse and the canceled bid row goes. Every row is Java's, in Java's order.
#[test]
fn an_auction_from_start_to_end_with_two_bidders() {
    let mut s = Scene::new(settings(true), |now| {
        vec![record(5, 1, now + 8_000, now + 68_000, 0, &[])]
    });

    // The constructor: auction 5 starts within a minute so it is the current one, and the next
    // one is made (id 6, `storeMe`).
    let stored = writes(&s.outbox.take());
    assert_eq!(stored.len(), 1);
    let ItemAuctionWrite::Store {
        auction_id,
        instance_id,
        state_id,
        ..
    } = stored[0]
    else {
        panic!("{stored:?}");
    };
    assert_eq!((auction_id, instance_id, state_id), (6, AUCTIONEER, 0));

    // RequestInfoItemAuction: the created auction (no time left), its item and the next one.
    let (out, _) = s.input(
        A,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert!(info.refresh);
    assert_eq!(
        (info.instance_id, info.bid, info.time_remaining),
        (AUCTIONEER, 100, 0)
    );
    assert_eq!(info.item.count, 1);
    assert_eq!(info.item.enchant, 15);
    let next = info.next.as_ref().unwrap();
    assert_eq!(next.init_bid, 100);
    assert!(next.start_secs > 0);

    // Bids before the start are ignored, silently.
    let (out, rows) = s.bid(A, 150);
    assert!(out.is_empty() && writes(&rows).is_empty());
    assert_eq!(s.adena(A), 1000);

    // The start (`setAuctionState(CREATED, STARTED)` + `storeMe`).
    s.tick(60);
    let started = writes(&s.outbox.take());
    assert!(
        started.iter().any(|w| matches!(
            w,
            ItemAuctionWrite::Store {
                auction_id: 5,
                state_id: 1,
                ..
            }
        )),
        "{started:?}"
    );

    // A bids 150: the adena is paid, the bid row is written, the end is extended by 5 minutes.
    let (out, rows) = s.bid(A, 150);
    assert_eq!(s.adena(A), 850);
    assert_eq!(
        writes(&rows),
        [ItemAuctionWrite::Bid {
            auction_id: 5,
            player_obj_id: A.0,
            bid: 150
        }]
    );
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::S1AdenaDisappeared,
            SystemMessageId::YouHaveSubmittedABidForTheAuctionOfS1,
            SystemMessageId::BidderExistsTheAuctionTimeHasBeenExtendedBy5Minutes,
        ]
    );
    assert_eq!(messages(&out)[1].params, [long(150)]);

    // Below the minimum, then below the highest bid: refused, nothing paid.
    let (out, rows) = s.bid(B, 50);
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid]
    );
    assert!(writes(&rows).is_empty());
    let (out, _) = s.bid(B, 120);
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YourBidMustBeHigherThanTheCurrentHighestBid]
    );
    assert_eq!(s.adena(B), 1000);

    // B bids 200: A is outbid; the second bidder extends by 3 minutes.
    let (out_b, rows) = s.bid(B, 200);
    let out_a = s.a.take();
    assert_eq!(s.adena(B), 800);
    assert_eq!(message_ids(&out_a)[0], SystemMessageId::YouHaveBeenOutbid);
    assert_eq!(
        message_ids(&out_b),
        [
            SystemMessageId::S1AdenaDisappeared,
            SystemMessageId::YouHaveSubmittedABidForTheAuctionOfS1,
            SystemMessageId::BidderExistsAuctionTimeHasBeenExtendedBy3Minutes,
        ]
    );
    assert_eq!(
        writes(&rows),
        [ItemAuctionWrite::Bid {
            auction_id: 5,
            player_obj_id: B.0,
            bid: 200
        }]
    );

    // A raises to 300: only the 150 difference is paid, B is outbid.
    let (out, _) = s.bid(A, 300);
    let out_b = s.b.take();
    assert_eq!(s.adena(A), 700);
    assert_eq!(message_ids(&out_b)[0], SystemMessageId::YouHaveBeenOutbid);
    assert_eq!(
        messages(&out)[0].params,
        [long(150)],
        "the raise pays the difference"
    );

    // The info shows the highest bid and the time left (more than the original minute).
    let (out, _) = s.input(
        B,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert_eq!(info.bid, 300);
    assert!(info.time_remaining > 60);

    // B takes the 200 back through the auctioneer's `ItemAuction cancel`.
    let (out, rows) = s.bypass(B, "ItemAuction cancel");
    assert_eq!(s.adena(B), 1000);
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::YouHaveEarnedS1Adena,
            SystemMessageId::YouHaveCanceledYourBid
        ]
    );
    assert_eq!(
        writes(&rows),
        [ItemAuctionWrite::Bid {
            auction_id: 5,
            player_obj_id: B.0,
            bid: -1
        }]
    );
    // A is the highest bidder: the cancel only tells it the reserve is not met, nothing returned.
    let (out, rows) = s.bypass(A, "ItemAuction cancel");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::YouCurrentlyHaveTheHighestBidButTheReserveHasNotBeenMet]
    );
    assert!(writes(&rows).is_empty());
    assert_eq!(s.adena(A), 700);
    // Nothing left for B: "no offerings".
    let (out, _) = s.bypass(B, "ItemAuction cancel");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::ThereAreNoOfferingsIOwnOrIMadeABidFor]
    );

    // The end: the first run (original end) sees the extension and waits for the new end, the
    // second ends the auction. 8 minutes more than the minute the auction had.
    s.clear();
    s.tick(20 * 60 * 11);
    let out_a = s.a.take();
    let out_b = s.b.take();
    let rows = s.outbox.take();
    let done = writes(&rows);
    assert_eq!(done.len(), 2, "{done:?}");
    assert!(
        matches!(
            done[0],
            ItemAuctionWrite::Store {
                auction_id: 5,
                instance_id: AUCTIONEER,
                auction_item_id: 1,
                state_id: 2,
                ..
            }
        ),
        "{done:?}"
    );
    // The canceled bid of B is cleared once the winner has the item.
    assert_eq!(
        done[1],
        ItemAuctionWrite::DeleteBid {
            auction_id: 5,
            player_obj_id: B.0
        }
    );
    // Both bidders hear the auction ended; the winner also gets the item message.
    assert_eq!(
        messages(&out_b)
            .iter()
            .map(|m| (m.id, m.params.clone()))
            .collect::<Vec<_>>(),
        [(
            SystemMessageId::S1SAuctionHasEnded,
            vec![SystemMessageParam::IntNumber(5)]
        )]
    );
    assert_eq!(
        message_ids(&out_a),
        [
            SystemMessageId::S1SAuctionHasEnded,
            SystemMessageId::YouHaveBidTheHighestPriceAndHaveWonTheItemTheItemCanBeFoundInYourPersonalWarehouse
        ]
    );
    // The item is a new row of A's warehouse: not stackable, enchant level 15.
    let stored = s.warehouse(A);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        (stored[0].item_id, stored[0].count, stored[0].loc.as_str()),
        (FALCHION, 1, "WAREHOUSE")
    );
    assert_eq!(stored[0].enchant_level, 15);
    assert_eq!(
        item_rows(&rows),
        [ItemRowOp::Insert {
            owner_id: A.0,
            item: stored[0].clone()
        }]
    );
    // The loser got its adena back and nothing else.
    assert_eq!((s.adena(A), s.adena(B)), (700, 1000));
}

/// `AltItemAuctionTimeExtendsOnBid`: after the 5 and the 3 minute extensions a bid of another
/// bidder than the last one adds the configured time (phase A), a bid of the same bidder adds
/// nothing, and a bid of the other one in phase A waits for the state task (phase B).
#[test]
fn the_configured_extension_follows_the_two_fixed_ones() {
    let mut s = Scene::new(
        ItemAuctionSettings {
            time_extends_on_bid_ms: 90_000,
            ..settings(true)
        },
        |now| started_falchion(now, &[]),
    );
    let remaining = |s: &mut Scene| {
        let (out, _) = s.input(
            A,
            ClientInput::ItemAuction(ItemAuctionInput::Info {
                instance_id: AUCTIONEER,
            }),
        );
        decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0].time_remaining
    };
    let start = remaining(&mut s);
    s.bid(A, 150); // + 5 minutes
    s.bid(B, 200); // + 3 minutes
    let after_fixed = remaining(&mut s);
    assert!((after_fixed - start - 480).abs() <= 8, "{start} {after_fixed}");
    s.bid(A, 250); // another bidder than the last: + 90 seconds
    let after_a = remaining(&mut s);
    assert!((after_a - after_fixed - 90).abs() <= 8, "{after_fixed} {after_a}");
    s.bid(B, 300); // phase A waits for the state task to switch to phase B: nothing
    let after_b = remaining(&mut s);
    assert!((after_b - after_a).abs() <= 8, "{after_a} {after_b}");
}

/// The winner is offline: the item is a `WAREHOUSE` row of its character. A stackable item joins
/// the stack an online winner already has (`ItemContainer.addItem`), and the augmentation of
/// `<extra>` is on the item.
#[test]
fn the_winner_gets_the_item_offline_or_on_the_stack() {
    // An offline winner (character 777 bid 500 on the falchion).
    let mut s = Scene::new(settings(true), |now| started_falchion(now, &[(777, 500)]));
    s.clear();
    s.tick(20 * 61);
    let rows = s.outbox.take();
    let ops = item_rows(&rows);
    assert_eq!(ops.len(), 1);
    let ItemRowOp::Insert { owner_id, item } = &ops[0] else {
        panic!("{ops:?}");
    };
    assert_eq!(
        (*owner_id, item.item_id, item.loc.as_str(), item.loc_data),
        (777, FALCHION, "WAREHOUSE", 0)
    );
    // Auction 6 (tomorrow) is the next one; auction 5 is finished: the rows are only its state.
    assert!(writes(&rows).iter().any(|w| matches!(
        w,
        ItemAuctionWrite::Store {
            auction_id: 5,
            state_id: 2,
            ..
        }
    )));

    // An online winner with a life stone stack of 2 in the warehouse: three more join it.
    let stack = ItemData {
        object_id: ObjectId(0x3000_0099),
        item_id: LIFE_STONE,
        count: 2,
        loc: "WAREHOUSE".into(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    };
    let mut s = Scene::with_warehouse(
        settings(true),
        |now| {
            vec![
                record(5, 2, now - 10_000, now + 60_000, 1, &[(A.0, 500)]),
                record(6, 1, now + 86_400_000, now + 86_460_000, 0, &[]),
            ]
        },
        vec![stack],
    );
    s.clear();
    s.tick(20 * 61);
    let stored = s.warehouse(A);
    assert_eq!(stored.len(), 1);
    assert_eq!((stored[0].object_id, stored[0].count), (ObjectId(0x3000_0099), 5));
    assert_eq!(
        item_rows(&s.outbox.take()),
        [ItemRowOp::Update {
            owner_id: A.0,
            item: stored[0].clone()
        }]
    );
}

/// The augmentation of `<extra>` is on the winner's item and in the packet.
#[test]
fn the_extra_augmentation_is_on_the_item() {
    let mut s = Scene::new(settings(true), |now| {
        vec![
            record(5, 2, now - 10_000, now + 60_000, 1, &[(A.0, 500)]),
            record(6, 1, now + 86_400_000, now + 86_460_000, 0, &[]),
        ]
    });
    let (out, _) = s.input(
        A,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert_eq!(
        (info.item.item_id, info.item.count, info.item.augmentation),
        (LIFE_STONE, 3, 5555)
    );
    assert_eq!(info.bid, 500);
    assert!(info.time_remaining > 0 && info.time_remaining <= 60);
    s.clear();
    s.tick(20 * 61);
    let stored = s.warehouse(A);
    assert_eq!((stored[0].count, stored[0].augmentation), (3, Some(5555)));
}

// ---- refusals ----------------------------------------------------------------------------------

/// Not enough adena: `reduceAdena` and `reduceItemCount` and `registerBid` each tell the player (a
/// Java quirk), nothing is written. A bid above `MaxAdena` is dropped by the packet.
#[test]
fn a_bid_without_the_adena_is_refused_three_times() {
    let mut s = Scene::new(settings(true), |now| started_falchion(now, &[]));
    let (out, rows) = s.bid(A, 5_000);
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::YouDoNotHaveEnoughAdena,
            SystemMessageId::YouDoNotHaveEnoughAdenaForThisBid,
            SystemMessageId::YouDoNotHaveEnoughAdenaForThisBid,
        ]
    );
    assert!(writes(&rows).is_empty());
    assert_eq!(s.adena(A), 1000);
    // A negative bid and a bid above MaxAdena are dropped without a word.
    let (out, _) = s.bid(A, -1);
    assert!(out.is_empty());
    let (out, _) = s.bid(A, i64::MAX);
    assert!(out.is_empty());
    // An unknown instance too.
    let (out, rows) = s.input(
        A,
        ClientInput::ItemAuction(ItemAuctionInput::Bid {
            instance_id: 1,
            bid: 500,
        }),
    );
    assert!(out.is_empty() && rows.is_empty());
    // Bidding twice in a row is "too fast": the second is refused with Java's text.
    s.world.set_flood_settings(FloodSettings::java_defaults());
    s.tick(200);
    s.clear();
    for bid in [200, 210] {
        s.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::ItemAuction(ItemAuctionInput::Bid {
                instance_id: AUCTIONEER,
                bid,
            }),
        });
    }
    let out = s.a.take();
    assert!(
        messages(&out)
            .iter()
            .any(|m| m.params == [SystemMessageParam::Text("You are bidding too fast.".into())])
    );
    assert_eq!(s.adena(A), 800, "only the first of two quick bids is taken");
}

/// The auctioneer's `ItemAuction show` sends the info (not a refresh), the second one within 2
/// seconds is refused (`isItemAuctionPolling` after a `RequestInfoItemAuction`), and an instance
/// without a running auction answers "not an auction period" with the next date.
#[test]
fn the_show_bypass_and_the_periods() {
    let mut s = Scene::new(settings(true), |now| started_falchion(now, &[(A.0, 300)]));
    let (out, _) = s.bypass(B, "ItemAuction show");
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert!(!info.refresh);
    assert_eq!((info.bid, info.item.item_id), (300, FALCHION));
    assert!(info.next.is_some());

    // The window refreshes with `RequestInfoItemAuction`; a bypass right after is ignored.
    s.input(
        B,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    s.clear();
    let npc = s.npc.0;
    s.world.handle(WorldCommand::Input {
        session: SessionId(2),
        input: ClientInput::Bypass {
            command: format!("npc_{npc}_ItemAuction show"),
        },
    });
    assert!(names(&s.b.take()).iter().all(|n| n != "EX_ITEM_AUCTION_INFO"));

    // Only a created auction (tomorrow): no current auction.
    let mut s = Scene::new(settings(true), |now| {
        vec![record(5, 1, now + 86_400_000, now + 86_460_000, 0, &[])]
    });
    let (out, _) = s.bypass(A, "ItemAuction show");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::ItIsNotAnAuctionPeriod, SystemMessageId::S12]
    );
    let text = match &messages(&out)[1].params[..] {
        [SystemMessageParam::Text(t)] => t.clone(),
        other => panic!("{other:?}"),
    };
    assert!(text.starts_with("The next auction will begin on the "), "{text}");
    assert!(text.ends_with('.'));
    // RequestInfoItemAuction without a running auction does nothing.
    let (out, _) = s.input(
        A,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    assert!(out.is_empty());
}

/// A finished auction keeps its bids: a loser takes the bid back (the row is deleted), the winner
/// cannot.
#[test]
fn a_bid_of_a_finished_auction_can_be_taken_back_by_the_loser_only() {
    let mut s = Scene::new(settings(true), |now| {
        vec![
            record(
                4,
                1,
                now - 3_600_000,
                now - 3_000_000,
                2,
                &[(A.0, 300), (B.0, 200)],
            ),
            record(5, 1, now + 86_400_000, now + 86_460_000, 0, &[]),
        ]
    });
    let (out, _) = s.bypass(A, "ItemAuction cancel");
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::ThereAreNoOfferingsIOwnOrIMadeABidFor],
        "the winner of a finished auction cannot take its bid back"
    );
    let (out, rows) = s.bypass(B, "ItemAuction cancel");
    assert_eq!(s.adena(B), 1200);
    assert_eq!(message_ids(&out)[1], SystemMessageId::YouHaveCanceledYourBid);
    assert_eq!(
        writes(&rows),
        [ItemAuctionWrite::DeleteBid {
            auction_id: 4,
            player_obj_id: B.0
        }]
    );
}

// ---- the load ----------------------------------------------------------------------------------

/// `loadAuction`: invalid rows (start after end, an unknown auction item, a bad state, a finished
/// auction older than `AltItemAuctionExpiredAfter`) are dropped and, with the switch on, deleted.
#[test]
fn invalid_and_expired_rows_are_dropped() {
    let day = 86_400_000;
    let rows = |now: i64| {
        vec![
            record(10, 1, now + 100, now + 50, 0, &[]),
            record(11, 99, now + 100, now + 5_000, 0, &[]),
            record(12, 1, now + 100, now + 5_000, 7, &[]),
            record(13, 1, now - 15 * day, now - 15 * day + 60_000, 2, &[(A.0, 5)]),
            record(14, 1, now - 13 * day, now - 13 * day + 60_000, 2, &[]),
        ]
    };
    let outbox = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(Recorder(outbox.clone())));
    world.set_static_data(Box::new(Data(stat_data())));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_item_auction_settings(settings(true));
    world.load_item_auctions(ItemAuctionLoad {
        next_auction_id: 15,
        instances: vec![instance_def()],
        auctions: rows(now_ms()),
    });
    let all = outbox.take();
    let deleted: Vec<i32> = writes(&all)
        .iter()
        .filter_map(|w| match w {
            ItemAuctionWrite::DeleteAuction { auction_id } => Some(*auction_id),
            _ => None,
        })
        .collect();
    assert_eq!(deleted, [10, 11, 12, 13]);
    // Auction 14 is the only one left (a finished one, recent): it is the current one and a new
    // next auction (id 15) was made.
    assert!(
        writes(&all)
            .iter()
            .any(|w| matches!(w, ItemAuctionWrite::Store { auction_id: 15, .. }))
    );

    // With the switch off nothing of this is written.
    let outbox = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(Recorder(outbox.clone())));
    world.set_static_data(Box::new(Data(stat_data())));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_item_auction_settings(settings(false));
    world.load_item_auctions(ItemAuctionLoad {
        next_auction_id: 15,
        instances: vec![instance_def()],
        auctions: rows(now_ms()),
    });
    assert!(writes(&outbox.take()).is_empty());
}

// ---- side by side (D-53) -----------------------------------------------------------------------

/// With the switch off: the rows are read again before the info is shown, and what the Java game
/// server wrote (a bid of another player) is what the info shows. A failed read refuses the
/// request.
#[test]
fn off_the_info_is_shown_from_the_rows_read_again() {
    let mut s = Scene::new(settings(false), |now| started_falchion(now, &[(A.0, 150)]));
    // Nothing global was written at the start.
    assert!(writes(&s.outbox.take()).is_empty());

    let (out, rows) = s.input(
        B,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    assert!(out.is_empty(), "nothing is shown before the rows are back");
    let asked = requests(&rows);
    assert_eq!(asked.len(), 1);
    let (instance_id, then) = asked[0];
    assert_eq!(
        (instance_id, then),
        (AUCTIONEER, ItemAuctionThen::Info { player: B })
    );

    // A failed read: nothing happens.
    s.world.handle(WorldCommand::ItemAuctionReread {
        instance_id,
        result: Err(()),
        then,
    });
    assert!(s.b.take().is_empty());

    // The Java game server has another bid of 400 in the rows now.
    let now = now_ms();
    s.world.handle(WorldCommand::ItemAuctionReread {
        instance_id,
        result: Ok(vec![
            record(5, 1, now - 10_000, now + 60_000, 1, &[(A.0, 150), (999, 400)]),
            record(6, 1, now + 86_400_000, now + 86_460_000, 0, &[]),
        ]),
        then,
    });
    let out = s.b.take();
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert_eq!(info.bid, 400);
    assert!(info.refresh);
}

/// With the switch off a bid is checked against the rows read again, pays the adena and writes its
/// own bid row (a per-player row), and the Java game server's higher bid refuses it.
#[test]
fn off_a_bid_is_checked_against_the_rows_read_again_and_its_row_is_written() {
    let mut s = Scene::new(settings(false), |now| started_falchion(now, &[(A.0, 150)]));
    s.clear();
    let (out, rows) = s.bid(B, 200);
    assert!(out.is_empty());
    let asked = requests(&rows);
    let (instance_id, then) = asked[0];
    assert_eq!(then, ItemAuctionThen::Bid { player: B, bid: 200 });

    // The rows now have a 400 bid (Java's): 200 is too low.
    let now = now_ms();
    let java_rows = |bids: &[(i32, i64)]| {
        vec![
            record(5, 1, now - 10_000, now + 60_000, 1, bids),
            record(6, 1, now + 86_400_000, now + 86_460_000, 0, &[]),
        ]
    };
    s.world.handle(WorldCommand::ItemAuctionReread {
        instance_id,
        result: Ok(java_rows(&[(A.0, 150), (999, 400)])),
        then,
    });
    assert_eq!(
        message_ids(&s.b.take()),
        [SystemMessageId::YourBidMustBeHigherThanTheCurrentHighestBid]
    );
    assert_eq!(s.adena(B), 1000);

    // With the old rows the bid goes through, and the row is written although the switch is off.
    s.world.handle(WorldCommand::ItemAuctionReread {
        instance_id,
        result: Ok(java_rows(&[(A.0, 150)])),
        then,
    });
    assert_eq!(s.adena(B), 800);
    assert_eq!(
        writes(&s.outbox.take()),
        [ItemAuctionWrite::Bid {
            auction_id: 5,
            player_obj_id: B.0,
            bid: 200
        }]
    );
    assert_eq!(
        message_ids(&s.b.take())[..2],
        [
            SystemMessageId::S1AdenaDisappeared,
            SystemMessageId::YouHaveSubmittedABidForTheAuctionOfS1
        ]
    );
    // A, outbid, hears it, and the bidders hear that the end of the auction was extended (it
    // ends in a minute, inside the last 10).
    assert_eq!(
        message_ids(&s.a.take()),
        [
            SystemMessageId::YouHaveBeenOutbid,
            SystemMessageId::BidderExistsTheAuctionTimeHasBeenExtendedBy5Minutes
        ]
    );
}

/// With the switch off the end of an auction stays with the Java game server: this server writes no
/// state, hands out no item, clears no bid row; the bidders still hear that the auction ended. The
/// next auction is chosen again and a state the rows say is further on wins.
#[test]
fn off_the_end_is_left_to_the_java_game_server() {
    let mut s = Scene::new(settings(false), |now| {
        vec![
            record(5, 1, now - 10_000, now + 60_000, 1, &[(A.0, 150), (B.0, 100)]),
            record(6, 1, now + 80_000, now + 140_000, 0, &[]),
        ]
    });
    s.clear();
    s.tick(20 * 61);
    let out_a = s.a.take();
    let rows = s.outbox.take();
    assert!(writes(&rows).is_empty(), "{rows:?}");
    assert!(item_rows(&rows).is_empty());
    assert_eq!(message_ids(&out_a), [SystemMessageId::S1SAuctionHasEnded]);
    assert!(s.warehouse(A).is_empty());

    // Showing the finished auction asks for the rows; the Java game server has started the next
    // auction (id 6), which wins over this server's guess.
    let (_, rows) = s.input(
        A,
        ClientInput::ItemAuction(ItemAuctionInput::Info {
            instance_id: AUCTIONEER,
        }),
    );
    let (instance_id, then) = requests(&rows)[0];
    let now = now_ms();
    s.world.handle(WorldCommand::ItemAuctionReread {
        instance_id,
        result: Ok(vec![
            record(5, 1, now - 70_000, now - 10_000, 2, &[(A.0, 150), (B.0, 100)]),
            record(6, 1, now + 80_000, now + 140_000, 1, &[]),
        ]),
        then,
    });
    let out = s.a.take();
    let info = &decoded::<ExItemAuctionInfo>(&out, "EX_ITEM_AUCTION_INFO")[0];
    assert_eq!(info.bid, 100, "auction 6 is running now, with its starting bid");
    assert!(info.time_remaining > 0);
}

/// The settings off: the bypass says it is not an auction period, and nothing is loaded.
#[test]
fn disabled_auctions_load_nothing() {
    let mut s = Scene::new(
        ItemAuctionSettings {
            enabled: false,
            ..settings(true)
        },
        |now| started_falchion(now, &[]),
    );
    let (out, rows) = s.bypass(A, "ItemAuction show");
    assert_eq!(message_ids(&out), [SystemMessageId::ItIsNotAnAuctionPeriod]);
    assert!(rows.is_empty());
    let (out, _) = s.bid(A, 200);
    assert!(out.is_empty());
}
