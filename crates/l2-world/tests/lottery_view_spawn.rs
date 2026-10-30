#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! S-HAND-03: the Lucky Lottery (`LotteryManager`, `Loto`), `NpcViewMod` and spawn protection.
//! Expected behaviour comes from the Java classes named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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

impl Persisted {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct CapturingOutbox(Persisted);

impl Outbox for CapturingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
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

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="10" type="Weapon" name="No-grade Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="price" val="100" /></item>
<item id="4442" type="EtcItem" name="Lottery Ticket"><set name="icon" val="icon.etc_lottery_card_i00" /></item>
<item id="112" type="EtcItem" name="Apprentice's Earring"><set name="icon" val="icon.accessary_apprentice_earring_i00" /></item>
</list>"#;

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="30990" type="Folk" level="10" name="Lottery Seller" usingServerSideName="true">
  <race>FAIRY</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
</npc>
<npc id="20001" type="Monster" level="10" name="Goblin" usingServerSideName="true">
  <race>FAIRY</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <dropLists>
    <drop>
      <group chance="70"><item id="57" min="30" max="42" chance="100" /></group>
      <item id="112" min="1" max="1" chance="12.5" />
    </drop>
  </dropLists>
</npc>
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
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x1000_0001);
/// 2026-10-27 (a Tuesday) 12:00:00 UTC.
const TUESDAY_NOON: i64 = 1_793_102_400_000;
const MINUTE: i64 = 60_000;

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn ticket(object_id: i32, draw: i32, numbers: [i32; 5]) -> ItemData {
    let (mut enchant, mut type2) = (0, 0);
    for n in numbers {
        if n < 17 {
            enchant += 1 << (n - 1);
        } else {
            type2 += 1 << (n - 17);
        }
    }
    ItemData {
        custom_type1: draw,
        enchant_level: enchant,
        custom_type2: type2,
        ..item(object_id, 4442, 1)
    }
}

fn player_data(id: ObjectId, items: Vec<ItemData>) -> PlayerData {
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
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    me: Log,
    persisted: Persisted,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(items: Vec<ItemData>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/html/default/30990-1.htm",
            "<html><body>first %objectId% race %race% price %ticket_price%</body></html>",
        );
        write(
            "data/html/default/30990-2.htm",
            "<html><body>instructions</body></html>",
        );
        write(
            "data/html/default/30990-3.htm",
            "<html><body>jackpot %adena% ends %enddate% 5:%prize5% 4:%prize4% 3:%prize3% 2:%prize2%</body></html>",
        );
        write(
            "data/html/default/30990-4.htm",
            "<html><body>%result%</body></html>",
        );
        write(
            "data/html/default/30990-5.htm",
            "<html><body>fore=\"L2UI.lottoNum01\" back=\"L2UI.lottoNum01a_check\" fore=\"L2UI.lottoNum12\" back=\"L2UI.lottoNum12a_check\" <a action=\"bypass -h npc_%objectId%_Loto 0\">Return</a></body></html>",
        );
        write(
            "data/html/default/30990-6.htm",
            "<html><body>bought</body></html>",
        );
        write(
            "data/html/mods/NpcView/Info.htm",
            "<html><body>%name% (%npcId%) %hpGauge% respawn:%respawn% type:%atktype% range:%atkrange% %menuButtons%</body></html>",
        );
        write(
            "data/html/mods/NpcView/DropList.htm",
            "<html><body>%name% %menuButtons% %pages% %items%</body></html>",
        );
        write(
            "data/html/CommunityBoard/Custom/navigation.html",
            "<nav>NAVIGATION</nav>",
        );
        write(
            "data/html/CommunityBoard/Custom/dropsearch/main.html",
            "<html>%navigation%<table>%searchResult%</table><table>%pages%</table></html>",
        );
        write(
            "data/spawns/test.xml",
            r#"<list enabled="true"><spawn><npc id="20001" x="-71000" y="258000" z="-3100" heading="0" respawnDelay="30" /></spawn></list>"#,
        );
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(CapturingOutbox(persisted.clone())),
        );
        let data = data();
        world.set_static_data(Box::new(data));
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(
            root.path(),
            &npcs,
            &l2_data::SpawnLoadOptions::default(),
        ));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_flood_settings(FloodSettings::java_defaults());
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            player: Box::new(player_data(ME, items)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            persisted,
            _root: root,
        };
        scene.advance(1000);
        scene.me.take();
        scene.persisted.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn npc(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        let npc = self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap();
        self.me.take();
        npc
    }

    fn bypass(&mut self, command: &str, npc: Option<ObjectId>) -> Option<bool> {
        let handler = BypassHandlers::default().get(command)?;
        Some(handler(&mut self.world, ME, command, npc))
    }

    fn count_of(&self, item_id: i32) -> i64 {
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

    fn use_item(&mut self, object_id: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        });
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

fn of<P: Decode>(out: &[Vec<u8>], opcode: ServerOpcode) -> Vec<P> {
    out.iter()
        .filter(|b| name_of(b) == opcode.name())
        .map(|b| decode::<P>(b))
        .collect()
}

fn messages(out: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(out, ServerOpcode::SystemMessage)
}

fn html(out: &[Vec<u8>]) -> Vec<NpcHtmlMessage> {
    of(out, ServerOpcode::NpcHtmlMessage)
}

fn running_lottery() -> LotterySettings {
    LotterySettings {
        cycle_database_writes: true,
        ..LotterySettings::default()
    }
}

/// The write of kind `pick` among the persisted commands.
fn lottery_writes(persisted: &[PersistCommand]) -> Vec<LotteryWrite> {
    persisted
        .iter()
        .filter_map(|c| match c {
            PersistCommand::Lottery(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------- lottery

/// `LotteryManager.startLottery` with no row: a new lottery #1 is announced, sells tickets until 10
/// minutes before the end and stores its row; `Loto` sells a ticket for the picked numbers.
#[test]
fn a_lottery_starts_and_sells_a_ticket() {
    let mut s = Scene::new(vec![item(0x100, 57, 10_000)]);
    s.world.set_lottery_settings(running_lottery());
    s.world.set_lottery_clock(Some(TUESDAY_NOON));
    s.world.load_lottery(None);
    assert!(s.world.lottery_is_started() && s.world.lottery_is_sellable());
    assert_eq!(s.world.lottery_id(), 1);
    let text: Vec<u8> = "Lottery tickets are now available for Lucky Lottery #1."
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let out = s.me.take();
    assert!(
        out.iter()
            .any(|b| name_of(b) == ServerOpcode::Say2.name() && b.windows(text.len()).any(|w| w == text)),
        "the announcement is a Say2 packet"
    );
    let writes = lottery_writes(&s.persisted.take());
    let LotteryWrite::Insert {
        idnr,
        enddate,
        prize,
        newprize,
    } = writes[0].clone()
    else {
        panic!("{writes:?}");
    };
    assert_eq!((idnr, prize, newprize), (1, 50_000, 50_000));
    // A Sunday 19:00 after the clock (the zone shifts it by at most a day).
    assert!(enddate > TUESDAY_NOON + 4 * 86_400_000 && enddate < TUESDAY_NOON + 6 * 86_400_000);

    let npc = s.npc(30990, 50);
    // Page 0 resets the picks and shows the first page.
    assert_eq!(s.bypass("Loto 0", Some(npc)), Some(false));
    let page = html(&s.me.take()).remove(0).html;
    assert!(page.contains("race 1 price 2000"), "{page}");
    // Pick five numbers; the buttons that are picked flip their images.
    for n in [1, 12, 3, 4, 5] {
        s.bypass(&format!("Loto {n}"), Some(npc));
    }
    let page = html(&s.me.take()).pop().unwrap().html;
    assert!(
        page.contains("fore=\"L2UI.lottoNum01a_check\" back=\"L2UI.lottoNum01\""),
        "{page}"
    );
    assert!(
        page.contains("22\">Your lucky numbers have been selected above."),
        "{page}"
    );
    // Buy.
    s.bypass("Loto 22", Some(npc));
    let out = s.me.take();
    assert!(
        messages(&out)
            .iter()
            .any(|m| m.id == SystemMessageId::YouHaveEarnedS1),
        "{:?}",
        messages(&out).iter().map(|m| m.id).collect::<Vec<_>>()
    );
    assert_eq!(html(&out).pop().unwrap().html, "<html><body>bought</body></html>");
    assert_eq!(s.count_of(57), 8000);
    assert_eq!(s.count_of(4442), 1);
    let bought = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == 4442)
        .unwrap()
        .clone();
    assert_eq!(
        (bought.custom_type1, bought.enchant_level, bought.custom_type2),
        (1, 1 + 4 + 8 + 16 + 2048, 0)
    );
    assert_eq!(s.world.lottery_prize(), 52_000);
    assert_eq!(
        lottery_writes(&s.persisted.take()),
        vec![LotteryWrite::UpdatePrice {
            idnr: 1,
            prize: 52_000
        }]
    );
}

/// Without `CycleDatabaseWrites` (D-53) this server sells no ticket.
#[test]
fn no_ticket_is_sold_while_the_java_server_owns_the_lottery() {
    let mut s = Scene::new(vec![item(0x100, 57, 10_000)]);
    s.world.set_lottery_settings(LotterySettings::default());
    s.world.set_lottery_clock(Some(TUESDAY_NOON));
    s.world.load_lottery(Some(LotteryRow {
        idnr: 7,
        prize: 90_000,
        newprize: 90_000,
        enddate: TUESDAY_NOON + 3 * 86_400_000,
        finished: false,
    }));
    assert!(!s.world.lottery_is_started());
    assert_eq!((s.world.lottery_id(), s.world.lottery_prize()), (7, 90_000));
    let npc = s.npc(30990, 50);
    s.bypass("Loto 1", Some(npc));
    assert_eq!(
        messages(&s.me.take()).iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![SystemMessageId::LotteryTicketsAreNotCurrentlyBeingSold]
    );
    assert!(s.persisted.take().is_empty());
}

/// `finishLottery`: the draw is announced with its winners, the row is finished, and the winner
/// of the first prize claims it at the NPC (`Loto` with the ticket's object id).
#[test]
fn a_lottery_draw_pays_the_winning_ticket() {
    let mut s = Scene::new(vec![item(0x100, 57, 0), ticket(0x200, 1, [1, 2, 3, 4, 5])]);
    s.world.set_lottery_settings(running_lottery());
    s.world.set_lottery_clock(Some(TUESDAY_NOON));
    // An unfinished lottery that ends within two minutes finishes at once.
    s.world.load_lottery(Some(LotteryRow {
        idnr: 1,
        prize: 100_000,
        newprize: 100_000,
        enddate: TUESDAY_NOON + MINUTE,
        finished: false,
    }));
    let requests: Vec<_> = s
        .persisted
        .take()
        .into_iter()
        .filter(|c| matches!(c, PersistCommand::LotteryRequest { .. }))
        .collect();
    assert_eq!(
        requests,
        vec![PersistCommand::LotteryRequest {
            then: LotteryThen::Draw { number: 1 }
        }]
    );
    // The ticket in the database is the drawn one (a player who holds it wins), another one is a
    // loser, a third has one number in common.
    let (enchant, type2) = s.world.lottery_pending_numbers().unwrap();
    let one_in_common = enchant & enchant.wrapping_neg();
    s.world.handle(WorldCommand::LotteryReread {
        result: Ok(LotteryReply::Tickets(vec![
            (enchant, type2),
            (0, 0),
            (one_in_common, 0),
        ])),
        then: LotteryThen::Draw { number: 1 },
    });
    let out = s.me.take();
    let drawn = messages(&out);
    assert_eq!(drawn.len(), 1);
    // One first prize winner: "The prize amount for the winner of lottery #1 is 100000 adena. We
    // have 1 first prize winners."
    assert_eq!(
        drawn[0].id,
        SystemMessageId::ThePrizeAmountForTheWinnerOfLotteryS1IsS2AdenaWeHaveS3FirstPrizeWinners
    );
    assert_eq!(
        drawn[0].params,
        vec![
            SystemMessageParam::IntNumber(1),
            SystemMessageParam::LongNumber(100_000),
            SystemMessageParam::LongNumber(1)
        ]
    );
    // prize4 = 1 ticket * 200; prize1 = ((100000 - 200) * 0.6) / 1 as float arithmetic.
    let writes = lottery_writes(&s.persisted.take());
    let [
        LotteryWrite::Finish {
            idnr,
            prize,
            newprize,
            number1,
            number2,
            prize1,
            prize2,
            prize3,
        },
    ] = writes.as_slice()
    else {
        panic!("{writes:?}");
    };
    assert_eq!((*idnr, *prize, *number1, *number2), (1, 100_000, enchant, type2));
    let expected_prize1 = (((100_000_i64 - 200) as f32 * 0.6_f32) / 1.0_f32) as i64;
    assert_eq!((*prize1, *prize2, *prize3), (expected_prize1, 0, 0));
    assert_eq!(*newprize, 100_000 - (expected_prize1 + 200));
    // The next lottery is #2 and starts a minute later.
    assert_eq!(s.world.lottery_id(), 2);
    assert!(!s.world.lottery_is_started());

    // Page 24 asks for the rows; with them the ticket is listed with its prize.
    let npc = s.npc(30990, 50);
    s.bypass("Loto 24", Some(npc));
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::LotteryRequest {
            then: LotteryThen::Page { player: ME, npc }
        }]
    );
    let row = LotteryResult {
        idnr: 1,
        number1: enchant,
        number2: type2,
        prize1: *prize1,
        prize2: 0,
        prize3: 0,
    };
    s.world.handle(WorldCommand::LotteryReread {
        result: Ok(LotteryReply::Results(vec![row])),
        then: LotteryThen::Page { player: ME, npc },
    });
    let page = html(&s.me.take()).pop().unwrap().html;
    // Our ticket holds 1 2 3 4 5; whether it won depends on the draw, but it is listed.
    assert!(
        page.contains(&format!(
            "npc_{}_Loto {}\">1 Event Number 1 2 3 4 5 ",
            npc.0, 0x200
        )),
        "{page}"
    );

    // A claim: give ourselves the drawn numbers' ticket and claim it.
    let numbers = {
        let mut n = Vec::new();
        for bit in 0..16 {
            if enchant & (1 << bit) != 0 {
                n.push(bit + 1);
            }
        }
        for bit in 0..4 {
            if type2 & (1 << bit) != 0 {
                n.push(bit + 17);
            }
        }
        n
    };
    assert_eq!(numbers.len(), 5);
    let winning = ticket(
        0x300,
        1,
        [numbers[0], numbers[1], numbers[2], numbers[3], numbers[4]],
    );
    s.world.player_mut(ME).unwrap().data.items.push(winning);
    s.me.take();
    s.bypass("Loto 768", Some(npc));
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::LotteryRequest {
            then: LotteryThen::Claim {
                player: ME,
                npc,
                object_id: 768
            }
        }]
    );
    s.world.handle(WorldCommand::LotteryReread {
        result: Ok(LotteryReply::Results(vec![row])),
        then: LotteryThen::Claim {
            player: ME,
            npc,
            object_id: 768,
        },
    });
    // The prize is in the inventory and the ticket is gone.
    assert_eq!(s.count_of(57), expected_prize1);
    assert!(
        !s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == ObjectId(768))
    );
    assert!(
        messages(&s.me.take())
            .iter()
            .any(|m| m.id == SystemMessageId::S1HasDisappeared)
    );
}

/// A lottery nobody wins rolls the jackpot over (`newprize`), and a read that failed counts no
/// ticket.
#[test]
fn a_draw_without_a_winner_keeps_the_jackpot() {
    let mut s = Scene::new(vec![]);
    s.world.set_lottery_settings(running_lottery());
    s.world.set_lottery_clock(Some(TUESDAY_NOON));
    s.world.load_lottery(Some(LotteryRow {
        idnr: 4,
        prize: 70_000,
        newprize: 70_000,
        enddate: TUESDAY_NOON,
        finished: false,
    }));
    s.persisted.take();
    s.world.handle(WorldCommand::LotteryReread {
        result: Err(()),
        then: LotteryThen::Draw { number: 4 },
    });
    let sms = messages(&s.me.take());
    assert_eq!(
        sms[0].id,
        SystemMessageId::ThePrizeAmountForLuckyLotteryS1IsS2AdenaThereWasNoFirstPrizeWinnerInThisDrawingThereforeTheJackpotWillBeAddedToTheNextDrawing
    );
    let writes = lottery_writes(&s.persisted.take());
    assert!(
        matches!(
            &writes[0],
            LotteryWrite::Finish {
                idnr: 4,
                prize: 70_000,
                newprize: 70_000,
                ..
            }
        ),
        "{writes:?}"
    );
    // A minute later the next lottery (#5) starts with the rolled over jackpot.
    s.world.set_lottery_clock(Some(TUESDAY_NOON + MINUTE));
    s.advance(61_000);
    assert_eq!((s.world.lottery_id(), s.world.lottery_prize()), (5, 70_000));
    assert!(s.world.lottery_is_started());
}

// ---------------------------------------------------------------------------------- NpcViewMod

/// `NpcViewMod view`, `dropList` and the menu buttons.
#[test]
fn the_npc_view_pages() {
    let mut s = Scene::new(vec![]);
    let mob = s.npc(20001, 80);
    assert_eq!(s.bypass(&format!("NpcViewMod view {}", mob.0), None), Some(true));
    let page = html(&s.me.take()).pop().unwrap().html;
    assert!(page.contains("Goblin (20001)"), "{page}");
    assert!(page.contains("respawn:None"), "{page}");
    assert!(page.contains("type:Sword"), "{page}");
    // 300 / 300 in the HP gauge, and the Drop button (the NPC has a drop list but no spoil).
    assert!(page.contains("<td width=112 align=right>300</td>"), "{page}");
    assert!(
        page.contains(&format!("bypass NpcViewMod dropList DROP {}", mob.0)),
        "{page}"
    );
    assert!(!page.contains("SPOIL"), "{page}");

    // The drop list: sorted by item id (57, then 112); the group chance multiplies the item's
    // (70% * 100%), the rates are 1.
    assert_eq!(
        s.bypass(&format!("NpcViewMod dropList DROP {}", mob.0), None),
        Some(true)
    );
    let page = html(&s.me.take()).pop().unwrap().html;
    assert!(page.contains("&#57;"), "{page}");
    assert!(page.contains("30 - 42"), "{page}");
    assert!(page.contains("70.00%"), "{page}");
    assert!(page.contains("&#112;"), "{page}");
    assert!(page.contains("12.50%"), "{page}");
    assert!(page.find("&#57;").unwrap() < page.find("&#112;").unwrap());

    // Bad input is refused the way Java does: no page, `false`.
    assert_eq!(s.bypass("NpcViewMod dropList NOPE 5", None), Some(false));
    assert_eq!(s.bypass("NpcViewMod view abc", None), Some(false));
    assert_eq!(s.bypass("NpcViewMod", None), Some(false));
    assert!(html(&s.me.take()).is_empty());
}

// ---------------------------------------------------------------------------------- spawn protection

/// `Player.setSpawnProtection`, `isSpawnProtected`, `onActionRequest`: the protection starts at the
/// login with `PlayerSpawnProtection` seconds, ends with the first action (with its message) or
/// with its time, and `FindPvP` sets it again.
#[test]
fn spawn_protection_ends_with_the_first_action_or_its_time() {
    let mut s = Scene::new(vec![item(0x400, 10, 1)]);
    // Not protected without the setting.
    assert!(!s.world.is_spawn_protected(ME));
    s.world.set_spawn_protection(ME, true);
    assert!(!s.world.is_spawn_protected(ME), "0 seconds protect nothing");

    s.world.set_pet_settings(PetSettings {
        player_spawn_protection: 5,
        ..PetSettings::default()
    });
    s.world.set_spawn_protection(ME, true);
    assert!(s.world.is_spawn_protected(ME));
    // Java's `Player.isInvul` does not include it (only `Summon.isInvul` does).
    assert!(!s.world.is_invul(ME));
    // Its time runs out.
    s.advance(5200);
    assert!(!s.world.is_spawn_protected(ME));

    // The first action ends it and says so.
    s.world.set_spawn_protection(ME, true);
    s.me.take();
    s.use_item(0x400);
    assert!(!s.world.is_spawn_protected(ME));
    let ids: Vec<_> = messages(&s.me.take()).iter().map(|m| m.id).collect();
    assert!(
        ids.contains(&SystemMessageId::YouAreNoLongerProtectedFromAggressiveMonsters),
        "{ids:?}"
    );
    // A second action says nothing more.
    s.use_item(0x400);
    let ids: Vec<_> = messages(&s.me.take()).iter().map(|m| m.id).collect();
    assert!(!ids.contains(&SystemMessageId::YouAreNoLongerProtectedFromAggressiveMonsters));

    // `setSpawnProtection(false)` clears it.
    s.world.set_spawn_protection(ME, true);
    s.world.set_spawn_protection(ME, false);
    assert!(!s.world.is_spawn_protected(ME));
}

// ---------------------------------------------------------------------------------- DropSearchBoard

/// The pages `DropSearchBoard` sends to a player: the content of the `ShowBoard` packets that have
/// one (`separateAndSend` sends the page in the first of three).
fn board_pages(out: &[Vec<u8>]) -> Vec<String> {
    of::<l2_protocol_game::server::ShowBoard>(out, ServerOpcode::ShowBoard)
        .into_iter()
        .filter_map(|b| b.content.strip_prefix("101\u{8}").map(str::to_owned))
        .collect()
}

impl Scene {
    /// Everything the command made the world send to the player.
    fn board_raw(&mut self, command: &str) -> Vec<Vec<u8>> {
        // The bypass flood protector lets one command through per interval.
        self.advance(1000);
        self.me.take();
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::Bypass {
                command: command.to_owned(),
            },
        });
        self.me.take()
    }

    fn board(&mut self, command: &str) -> Vec<String> {
        board_pages(&self.board_raw(command))
    }
}

/// `_bbs_search_item` lists the items with drops whose name has the text (adena never), the page
/// of an item lists its droppers sorted by level with the rates applied and the pages row, and
/// `_bbs_npc_trace` sends the mini map and, 500 ms later, the radar marker of the first spawn.
#[test]
fn the_drop_search_board_pages() {
    let mut s = Scene::new(vec![]);

    // The search: "earring" finds item 112; adena is blocked even when its name matches.
    let pages = s.board("_bbs_search_item earring");
    assert_eq!(pages.len(), 1, "{pages:?}");
    let page = &pages[0];
    assert!(page.contains("NAVIGATION"), "{page}");
    assert!(page.contains("background=\"icon.accessary_apprentice_earring_i00\""), "{page}");
    assert!(page.contains("bypass _bbs_search_drop 112 1 $order $level"), "{page}");
    assert!(page.contains("&#112;"), "{page}");
    // One row of two cells, then the padding rows up to seven lines.
    assert_eq!(page.matches("<tr><td height=36></td></tr>").count(), 6, "{page}");
    let pages = s.board("_bbs_search_item adena");
    assert!(pages[0].contains("<tr><td width=100 align=CENTER>No Match</td></tr>"), "{pages:?}");

    // The drop page: Goblin (level 10) drops it at 12.5% with a rate of 1.
    let pages = s.board("_bbs_search_drop 112 1 0 0");
    let page = &pages[0];
    assert!(page.contains("<td width=30>10</td>"), "{page}");
    assert!(page.contains("<a action=\"bypass _bbs_npc_trace 20001\">&@20001;</a>"), "{page}");
    assert!(page.contains("<td width=80 align=CENTER>1.0-1.0</td>"), "{page}");
    assert!(page.contains("<td width=50 align=CENTER>12.50%</td>"), "{page}");
    assert!(page.contains("<td width=50 align=CENTER>Drop</td>"), "{page}");
    assert!(
        page.contains("<tr><td><a action=\"bypass -h _bbs_search_drop 112 1 $order $level\">1</a></td></tr>"),
        "{page}"
    );
    // An item nobody drops, or a bad number: Java throws, nothing is sent.
    assert!(s.board("_bbs_search_drop 999 1 0 0").is_empty());
    assert!(s.board("_bbs_search_drop x 1 0 0").is_empty());

    // The trace: a monster without a spawn only gets a message; Goblin has one.
    let sent = s.board_raw("_bbs_npc_trace 30990");
    assert_eq!(messages(&sent).len(), 1, "\"Cannot find any spawn.\"");
    // The main page goes out in any case, with its placeholders (Java replaces only the navigation).
    assert_eq!(board_pages(&sent).len(), 1);
    let first = s.board_raw("_bbs_npc_trace 20001");
    assert!(of::<l2_protocol_game::server::ShowMiniMap>(&first, ServerOpcode::ShowMinimap)
        .iter()
        .any(|m| m.map_id == -1));
    assert!(of::<l2_protocol_game::server::RadarControl>(&first, ServerOpcode::RadarControl).is_empty());
    s.advance(600);
    let later = of::<l2_protocol_game::server::RadarControl>(&s.me.take(), ServerOpcode::RadarControl);
    let spots: Vec<_> = later.iter().map(|r| (r.show_radar, r.radar_type, r.x, r.y, r.z)).collect();
    assert_eq!(
        spots,
        [
            (2, 2, -71_000, 258_000, -3_100),
            (0, 1, -71_000, 258_000, -3_100),
            (0, 2, -71_000, 258_000, -3_100)
        ]
    );
}
