#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The wedding system (M8-MISC-01): `.engage`, the answer, the priest NPC, `.divorce`, `.gotolove`
//! and the login of a couple. The expected messages are read off the Java sources
//! (`voiced/Wedding.java`, `Wedding.java` of the event script, `Player.engageAnswer`,
//! `CoupleManager.java`, `EnterWorld.java`), not off the code.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::Encode;
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ChatType, ConfirmDlg, CreatureSay, MagicSkillUse, SayBody, SenderField, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::wedding::{Couple, WeddingSettings, WeddingWrite};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone)]
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

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level>
<level val="20"><hp>300</hp><mp>150</mp><cp>60</cp></level></lvlUpgainData></list>"#;

const ADENA: i32 = 57;
const FORMAL_WEAR: i32 = 6408;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="6408" type="Armor" name="Formal Wear"><set name="armor_type" val="NONE" /><set name="bodypart" val="chest" /><set name="weight" val="1" /></item>
</list>"#;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
const CAROL: i32 = 0x1000_0003;
const PRICE: i64 = 1000;

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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn settings() -> WeddingSettings {
    WeddingSettings {
        allow_wedding: true,
        wedding_price: i32::try_from(PRICE).unwrap(),
        teleport_price: 100,
        teleport_duration: 2,
        ..WeddingSettings::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

/// A player: `sex` 1 is female, `friends` the object ids on its friend list, `wear` the formal wear.
struct Who {
    id: i32,
    name: &'static str,
    sex: i32,
    friends: Vec<(i32, &'static str)>,
    adena: i64,
    wear: bool,
    pos: (i32, i32, i32),
}

impl Who {
    fn new(id: i32, name: &'static str, sex: i32) -> Self {
        Self {
            id,
            name,
            sex,
            friends: Vec::new(),
            adena: 10_000,
            wear: true,
            pos: (X, Y, Z),
        }
    }
}

impl Scene {
    fn new() -> Self {
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
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(log.clone())));
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::default()),
        };
        world.set_static_data(Box::new(Data(stats)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &format!(
                    r#"<list><npc id="50007" type="Folk" level="70" name="Andromeda">{STATS}</npc></list>"#
                ),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_wedding_settings(settings());
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, who: Who) {
        let mut items = vec![ItemData {
            object_id: ObjectId(who.id + 0x100),
            item_id: ADENA,
            count: who.adena,
            loc: "INVENTORY".into(),
            ..ItemData::default()
        }];
        if who.wear {
            items.push(ItemData {
                object_id: ObjectId(who.id + 0x200),
                item_id: FORMAL_WEAR,
                count: 1,
                loc: "PAPERDOLL".into(),
                loc_data: 6,
                ..ItemData::default()
            });
        }
        let mut data = PlayerData {
            object_id: ObjectId(who.id),
            account: format!("acc{}", who.id),
            name: who.name.into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: who.sex,
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
            x: who.pos.0,
            y: who.pos.1,
            z: who.pos.2,
            heading: 0,
            cur_hp: 300.0,
            max_hp: 300.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 0.0,
            max_cp: 50.0,
            online_time: 10,
            items,
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        };
        data.profile.friends = who
            .friends
            .iter()
            .map(|(id, name)| FriendEntry {
                object_id: *id,
                name: Some((*name).to_owned()),
            })
            .collect();
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(who.id as u64),
            player: Box::new(data),
            sink: Box::new(Sink(who.id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input,
        });
    }

    fn say(&mut self, id: i32, text: &str) {
        self.input(
            id,
            ClientInput::Say {
                chat_type: 0,
                text: text.into(),
                target: None,
            },
        );
    }

    fn target(&mut self, id: i32, target: i32) {
        self.world.set_target(ObjectId(id), Some(ObjectId(target)));
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn take(&self) -> Vec<Out> {
        self.log.take()
    }

    /// Engages Alice (asker) and Bob (answers yes). Both are friends of each other.
    fn engaged_couple() -> Self {
        let mut s = Self::new();
        s.enter(Who {
            friends: vec![(BOB, "Bob")],
            ..Who::new(ALICE, "Alice", 1)
        });
        s.enter(Who {
            friends: vec![(ALICE, "Alice")],
            ..Who::new(BOB, "Bob", 0)
        });
        s.target(ALICE, BOB);
        s.say(ALICE, ".engage");
        s.input(
            BOB,
            ClientInput::DlgAnswer {
                message_id: SystemMessageId::S13.id(),
                answer: 1,
                requester_id: ALICE,
            },
        );
        s.take();
        s
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// The packets of one kind one client got, in order.
fn of<'a>(out: &'a [Out], to: i32, name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, b) if *id == to && name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .collect()
}

/// The plain text lines (`sendMessage`) one client got.
fn texts(out: &[Out], to: i32) -> Vec<String> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .filter_map(|raw| {
            let m: SystemMessage = decode(raw);
            match m.params.first() {
                Some(l2_protocol_game::server::SystemMessageParam::Text(t))
                    if m.id == SystemMessageId::S12 =>
                {
                    Some(t.clone())
                }
                _ => None,
            }
        })
        .collect()
}

fn persisted(out: &[Out]) -> Vec<&WeddingWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Wedding(w)) => Some(w),
            _ => None,
        })
        .collect()
}

fn state(s: &Scene, id: i32) -> wedding::PlayerWedding {
    s.world.player_wedding(ObjectId(id)).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------------- engage

/// `Wedding.engage`: the refusals in Java's order, each with its message.
#[test]
fn engage_refusals_use_javas_messages() {
    let mut s = Scene::new();
    s.enter(Who {
        friends: vec![(BOB, "Bob")],
        ..Who::new(ALICE, "Alice", 1)
    });
    s.enter(Who::new(BOB, "Bob", 0));
    s.enter(Who::new(CAROL, "Carol", 1));
    s.take();

    s.say(ALICE, ".engage");
    assert_eq!(texts(&s.take(), ALICE), ["You have no one targeted."]);

    s.target(ALICE, ALICE);
    s.say(ALICE, ".engage");
    assert_eq!(
        texts(&s.take(), ALICE),
        ["Is there something wrong with you, are you trying to go out with youself?"]
    );

    // The same sex.
    s.target(ALICE, CAROL);
    s.say(ALICE, ".engage");
    assert_eq!(
        texts(&s.take(), ALICE),
        ["Gay marriage is not allowed on this server!"]
    );

    // Bob does not have Alice on his friend list (Alice has Bob).
    s.target(ALICE, BOB);
    s.say(ALICE, ".engage");
    let out = s.take();
    assert_eq!(
        texts(&out, ALICE),
        [
            "The player you want to ask is not on your friends list, you must first be on each others friends list before you choose to engage."
        ]
    );
    assert!(of(&out, BOB, "CONFIRM_DLG").is_empty());
    assert!(persisted(&out).is_empty());
}

/// A command the wedding handler takes is not said; text that is no command still is.
#[test]
fn a_voiced_command_is_not_said_aloud() {
    let mut s = Scene::new();
    s.enter(Who::new(ALICE, "Alice", 1));
    s.enter(Who::new(BOB, "Bob", 0));
    s.take();
    s.say(ALICE, ".engage");
    let out = s.take();
    assert!(of(&out, BOB, "SAY2").is_empty());
    s.say(ALICE, ".unknown thing");
    let out = s.take();
    let expected = CreatureSay {
        sender_object_id: ALICE,
        chat_type: ChatType::General,
        sender: SenderField::Name("Alice".into()),
        message_id: -1,
        body: SayBody::Text(".unknown thing".into()),
    };
    assert_eq!(of(&out, BOB, "SAY2"), [expected.to_bytes().as_slice()]);
}

/// Asking works for a friend, the target gets Java's question with the 15 second limit, and
/// "yes" makes the couple: a `mods_wedding` row, the ids on both, the asker told.
#[test]
fn a_friend_is_asked_and_the_yes_makes_a_couple() {
    let mut s = Scene::new();
    s.enter(Who {
        friends: vec![(BOB, "Bob")],
        ..Who::new(ALICE, "Alice", 1)
    });
    s.enter(Who {
        friends: vec![(ALICE, "Alice")],
        ..Who::new(BOB, "Bob", 0)
    });
    s.take();
    s.target(ALICE, BOB);
    s.say(ALICE, ".engage");
    let out = s.take();
    let ask: ConfirmDlg = decode(of(&out, BOB, "CONFIRM_DLG")[0]);
    assert_eq!(ask.message.id, SystemMessageId::S13);
    assert_eq!(
        ask.message.params,
        [l2_protocol_game::server::SystemMessageParam::Text(
            "Alice is asking to engage you. Do you want to start a new relationship?".into()
        )]
    );
    assert_eq!(ask.time, 15_000);
    // A second asker finds the request pending.
    assert!(state(&s, BOB).engage_request);

    s.input(
        BOB,
        ClientInput::DlgAnswer {
            message_id: SystemMessageId::S13.id(),
            answer: 1,
            requester_id: ALICE,
        },
    );
    let out = s.take();
    assert_eq!(texts(&out, ALICE), ["Request to Engage has been >ACCEPTED<"]);
    let writes = persisted(&out);
    assert_eq!(writes.len(), 1);
    let WeddingWrite::Insert(couple) = writes[0] else {
        panic!("{writes:?}");
    };
    assert_eq!((couple.player1_id, couple.player2_id), (ALICE, BOB));
    assert!(!couple.married);
    assert_eq!(couple.affiance_date, couple.wedding_date);
    assert_eq!(s.world.couples().len(), 1);
    assert_eq!(s.world.couple(couple.id), Some(couple));
    for (me, other) in [(ALICE, BOB), (BOB, ALICE)] {
        let st = state(&s, me);
        assert_eq!(
            (st.partner_id, st.couple_id, st.married),
            (other, couple.id, false)
        );
        assert!(!st.engage_request);
    }
    // The couple id is an object id of the Rust range (D-39).
    assert!(couple.id >= RUST_FIRST_OBJECT_ID);

    // Engaged already: the next `.engage` says so (and the big head follows with the setting).
    s.target(ALICE, BOB);
    s.say(ALICE, ".engage");
    assert_eq!(texts(&s.take(), ALICE), ["You are already engaged."]);
}

/// "No" tells the asker, makes no couple.
#[test]
fn a_no_is_reported_and_makes_no_couple() {
    let mut s = Scene::new();
    s.enter(Who {
        friends: vec![(BOB, "Bob")],
        ..Who::new(ALICE, "Alice", 1)
    });
    s.enter(Who {
        friends: vec![(ALICE, "Alice")],
        ..Who::new(BOB, "Bob", 0)
    });
    s.target(ALICE, BOB);
    s.say(ALICE, ".engage");
    s.take();
    s.input(
        BOB,
        ClientInput::DlgAnswer {
            message_id: SystemMessageId::S13.id(),
            answer: 0,
            requester_id: ALICE,
        },
    );
    let out = s.take();
    assert_eq!(texts(&out, ALICE), ["Request to Engage has been >DENIED<!"]);
    assert!(persisted(&out).is_empty());
    assert!(s.world.couples().is_empty());
    assert_eq!(state(&s, BOB).partner_id, 0);
}

// ---------------------------------------------------------------------------------- marriage

// The priest (`Wedding`, NPC 50007) is a script of `l2-scripts` (S-API-02); the tests of the
// ceremony are in `crates/l2-scripts/src/custom/events/wedding.rs`.

// ---------------------------------------------------------------------------------- divorce

/// A broken engagement costs nothing; a divorce costs a share of the adena that goes to the
/// partner. Both delete the row and clear the ids.
#[test]
fn divorce_pays_the_partner_and_deletes_the_row() {
    // Engaged only.
    let mut s = Scene::engaged_couple();
    let id = s.world.couples()[0].id;
    s.say(ALICE, ".divorce");
    let out = s.take();
    assert_eq!(texts(&out, ALICE), ["You have broken up as a couple."]);
    assert_eq!(
        texts(&out, BOB),
        ["Your fiance has decided to break the engagement with you."]
    );
    assert_eq!(persisted(&out), [&WeddingWrite::Divorce { id }]);
    assert!(s.world.couples().is_empty());
    assert_eq!(state(&s, ALICE).partner_id, 0);
    assert_eq!(state(&s, BOB).partner_id, 0);
    assert_eq!(s.world.adena_of(ObjectId(ALICE)), 10_000);

    // Married: 20% of the adena of the one who divorces.
    let mut s = Scene::new();
    s.world.load_couples(vec![Couple {
        id: 0x6000_0010,
        player1_id: ALICE,
        player2_id: BOB,
        married: true,
        affiance_date: 1,
        wedding_date: 2,
    }]);
    s.enter(Who::new(ALICE, "Alice", 1));
    s.enter(Who::new(BOB, "Bob", 0));
    s.take();
    assert!(state(&s, ALICE).married && state(&s, BOB).married);
    s.say(BOB, ".divorce");
    let out = s.take();
    assert_eq!(texts(&out, BOB), ["You are now divorced."]);
    assert_eq!(texts(&out, ALICE), ["Your spouse has decided to divorce you."]);
    assert_eq!(s.world.adena_of(ObjectId(BOB)), 8_000);
    assert_eq!(s.world.adena_of(ObjectId(ALICE)), 12_000);
    assert_eq!(persisted(&out), [&WeddingWrite::Divorce { id: 0x6000_0010 }]);
    assert!(!state(&s, BOB).married && !state(&s, ALICE).married);
}

// ---------------------------------------------------------------------------------- login

/// `EnterWorld`: the player finds its couple, the partner hears it logged in. Nothing happens
/// with `AllowWedding` off.
#[test]
fn a_couple_finds_each_other_at_login() {
    let couple = Couple {
        id: 0x6000_0011,
        player1_id: ALICE,
        player2_id: BOB,
        married: true,
        affiance_date: 1,
        wedding_date: 2,
    };
    let mut s = Scene::new();
    s.world.load_couples(vec![couple.clone()]);
    s.enter(Who::new(ALICE, "Alice", 1));
    s.take();
    s.enter(Who::new(BOB, "Bob", 0));
    let out = s.take();
    assert_eq!(texts(&out, ALICE), ["Your partner has logged in."]);
    let st = state(&s, BOB);
    assert_eq!(
        (st.partner_id, st.couple_id, st.married),
        (ALICE, couple.id, true)
    );

    let mut off = Scene::new();
    off.world.set_wedding_settings(WeddingSettings::default());
    off.world.load_couples(vec![couple]);
    off.enter(Who::new(ALICE, "Alice", 1));
    assert_eq!(state(&off, ALICE).partner_id, 0);
    // The voiced commands do not exist either: the text is said.
    off.enter(Who::new(BOB, "Bob", 0));
    off.take();
    off.say(ALICE, ".divorce");
    assert_eq!(of(&off.take(), BOB, "SAY2").len(), 1);
}

// ---------------------------------------------------------------------------------- gotolove

/// `.gotolove`: the fee, the message with the time, then after the wait the player stands at the
/// partner. An unmarried player is refused; an abort of the cast (a hit, a move) cancels it.
#[test]
fn gotolove_teleports_after_the_wait() {
    let mut s = Scene::new();
    s.world.load_couples(vec![Couple {
        id: 0x6000_0012,
        player1_id: ALICE,
        player2_id: BOB,
        married: true,
        affiance_date: 1,
        wedding_date: 2,
    }]);
    s.enter(Who::new(ALICE, "Alice", 1));
    s.enter(Who {
        pos: (X + 1500, Y, Z),
        ..Who::new(BOB, "Bob", 0)
    });
    s.enter(Who::new(CAROL, "Carol", 1));
    s.take();

    s.say(CAROL, ".gotolove");
    assert_eq!(texts(&s.take(), CAROL), ["You're not married."]);

    s.say(ALICE, ".gotolove");
    let out = s.take();
    assert_eq!(
        texts(&out, ALICE),
        ["You will be teleported to your partner in 2 seconds."]
    );
    assert_eq!(s.world.adena_of(ObjectId(ALICE)), 10_000 - 100);
    // The Scroll of Escape animation and the gauge.
    let animation: MagicSkillUse = decode(of(&out, ALICE, "MAGIC_SKILL_USE")[0]);
    assert_eq!((animation.skill_id, animation.hit_time), (1050, 2000));
    assert_eq!(of(&out, ALICE, "SETUP_GAUGE").len(), 1);
    assert!(s.world.is_casting_now(ObjectId(ALICE)));
    assert_eq!(s.world.position_of(ObjectId(ALICE)), Some((X, Y, Z)));

    s.ticks(30);
    assert_eq!(
        s.world.position_of(ObjectId(ALICE)),
        Some((X, Y, Z)),
        "the wait is not over"
    );
    s.ticks(15);
    let (x, y, _) = s.world.position_of(ObjectId(ALICE)).unwrap();
    assert!((x - (X + 1500)).abs() <= 1 && (y - Y).abs() <= 1, "({x}, {y})");
    assert!(!s.world.is_casting_now(ObjectId(ALICE)));
}

/// An aborted cast cancels the `EscapeFinalizer` (Java's `abortCast` cancels `_skillCast`).
#[test]
fn an_aborted_gotolove_stays_put() {
    let mut s = Scene::new();
    s.world.load_couples(vec![Couple {
        id: 0x6000_0012,
        player1_id: ALICE,
        player2_id: BOB,
        married: true,
        affiance_date: 1,
        wedding_date: 2,
    }]);
    s.enter(Who::new(ALICE, "Alice", 1));
    s.enter(Who {
        pos: (X + 1500, Y, Z),
        ..Who::new(BOB, "Bob", 0)
    });
    s.say(ALICE, ".gotolove");
    s.ticks(5);
    s.world.abort_cast(ObjectId(ALICE));
    s.ticks(60);
    assert_eq!(s.world.position_of(ObjectId(ALICE)), Some((X, Y, Z)));
}

/// `VoiceCommand` (`voice .gotolove` on an NPC page) returns what `Wedding.onCommand` returns:
/// `false` for an unmarried player and for a word nobody handles, `true` when the partner is
/// reached (S-HAND-03).
#[test]
fn a_voice_bypass_returns_the_wedding_result() {
    let mut s = Scene::new();
    s.world.load_couples(vec![Couple {
        id: 0x6000_0012,
        player1_id: ALICE,
        player2_id: BOB,
        married: true,
        affiance_date: 1,
        wedding_date: 2,
    }]);
    s.enter(Who::new(ALICE, "Alice", 1));
    s.enter(Who {
        pos: (X + 1500, Y, Z),
        ..Who::new(BOB, "Bob", 0)
    });
    s.enter(Who::new(CAROL, "Carol", 1));
    s.take();
    let handler = BypassHandlers::default().get("voice .gotolove").unwrap();
    assert!(!handler(&mut s.world, ObjectId(CAROL), "voice .gotolove", None));
    assert_eq!(texts(&s.take(), CAROL), ["You're not married."]);
    assert!(handler(&mut s.world, ObjectId(ALICE), "voice .gotolove", None));
    assert!(s.world.is_casting_now(ObjectId(ALICE)));
    assert!(!handler(&mut s.world, ObjectId(ALICE), "voice .nothing", None));
}
