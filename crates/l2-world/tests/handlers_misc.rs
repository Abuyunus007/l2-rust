#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The small handlers of S-HAND-01: the user commands `/time`, `/clanpenalty`, `/clanwarlist` (and
//! its two siblings) and `/siegestatus`, the NPC bypasses `player_help`, `tutorial_close` and
//! `questlist`, and the click on an artefact. Java:
//! dist/game/data/scripts/handlers/chat/commands/user/{Time,ClanPenalty,ClanWarsList,SiegeStatus}.java,
//! dist/game/data/scripts/handlers/bypass/npc/{PlayerHelp,TutorialClose,QuestList}.java and
//! dist/game/data/scripts/handlers/actions/click/ArtefactClick.java.

use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, PlayerClanState};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

type Sent = Vec<Vec<u8>>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data;

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
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
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn names(out: &Sent) -> Vec<String> {
    out.iter().map(|b| name_of(b)).collect()
}

fn messages(out: &Sent) -> Vec<SystemMessage> {
    out.iter()
        .filter(|b| name_of(b) == "SYSTEM_MESSAGE")
        .map(|b| decode::<SystemMessage>(b))
        .collect()
}

fn html(out: &Sent) -> Vec<NpcHtmlMessage> {
    out.iter()
        .filter(|b| name_of(b) == "NPC_HTML_MESSAGE")
        .map(|b| decode::<NpcHtmlMessage>(b))
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x1000_0001;
const LONER: i32 = 0x1000_0002;
/// The clans: mine, the one I declared war on that declared on me, one I declared on, one that
/// declared on me.
const MINE: i32 = 500;
const MUTUAL: i32 = 600;
const PREY: i32 = 700;
const HUNTER: i32 = 800;

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="9901" type="Artefact" level="1" name="Artifact" />
<npc id="9902" type="Adventurer" level="1" name="Guide" />
<npc id="9903" type="Folk" level="1" name="Villager" />
</list>"#;

fn clan_record(id: i32, name: &str, leader: i32, ally: Option<(i32, &str)>) -> ClanRecord {
    ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 3,
            leader_id: leader,
            ally_id: ally.map_or(0, |a| a.0),
            ally_name: ally.map(|a| a.1.to_owned()),
            // Only my clan has an invite penalty.
            char_penalty_expiry_time: if id == MINE { i64::MAX / 4 } else { 0 },
            ..ClanDataRecord::default()
        }),
        members: vec![ClanMemberRecord {
            object_id: leader,
            name: format!("Boss{id}"),
            level: 40,
            power_grade: 1,
            ..ClanMemberRecord::default()
        }],
        ..ClanRecord::default()
    }
}

fn pdata(id: i32, name: &str, clan_id: i32, join_expiry: i64) -> PlayerData {
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.to_owned(),
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
    };
    if clan_id != 0 || join_expiry != 0 {
        data.profile.clan = Some(PlayerClanState {
            clan_id,
            clan_join_expiry_time: join_expiry,
            ..PlayerClanState::default()
        });
    }
    data
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write("data/html/help/rules.htm", "<html><body>Rules</body></html>");
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(3), Box::new(NullOutbox));
        world.set_static_data(Box::new(Data));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        world.set_npc_data(npcs, Arc::new(SpawnData::default()));
        world.load_clans(
            vec![
                clan_record(MINE, "Mine", ME, None),
                // The leading clan of the alliance is the clan itself.
                clan_record(MUTUAL, "Mutual", 0x1000_0010, Some((MUTUAL, "Allies"))),
                clan_record(PREY, "Prey", 0x1000_0011, None),
                clan_record(HUNTER, "Hunter", 0x1000_0012, None),
            ],
            vec![
                (MINE.to_string(), MUTUAL.to_string()),
                (MUTUAL.to_string(), MINE.to_string()),
                (MINE.to_string(), PREY.to_string()),
                (HUNTER.to_string(), MINE.to_string()),
            ],
        );
        let mut scene = Self {
            world,
            log,
            _root: root,
        };
        // A join penalty far in the future for the second player.
        scene.enter(pdata(ME, "Me", MINE, 0));
        scene.enter(pdata(LONER, "Loner", 0, i64::MAX / 4));
        scene.log.take();
        scene
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(self.log.clone())),
            ip: None,
        });
    }

    fn user_command(&mut self, player: i32, id: i32) -> Sent {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::UserCommand { id },
        });
        self.log.take()
    }
}

// ---------------------------------------------------------------------------------- tests

/// `Time`: `THE_CURRENT_TIME_IS_S1_S2` (or the `_2` text at night) with the hour and the minutes
/// (two digits). `DisplayServerTime` adds the server's clock line.
#[test]
fn time_shows_the_game_time() {
    let mut s = Scene::new();
    let out = s.user_command(ME, 77);
    let first = messages(&out);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0].id,
        SystemMessageId::TheCurrentTimeIsS1S2 | SystemMessageId::TheCurrentTimeIsS1S22
    ));
    let [SystemMessageParam::Text(hour), SystemMessageParam::Text(minute)] = first[0].params.as_slice()
    else {
        panic!("{:?}", first[0].params);
    };
    assert!(hour.parse::<u32>().unwrap() < 24);
    assert_eq!(minute.len(), 2, "the minutes are padded: {minute}");

    s.world.set_user_command_info_settings(UserCommandInfoSettings {
        display_server_time: true,
    });
    let out = s.user_command(ME, 77);
    let both = messages(&out);
    assert_eq!(both.len(), 2);
    let SystemMessageParam::Text(text) = &both[1].params[0] else {
        panic!("{:?}", both[1].params);
    };
    assert!(text.starts_with("Server time is "), "{text}");
    assert!(text.ends_with('.'));
}

/// `ClanPenalty`: one row per penalty that is on (the player's join penalty, the clan's invite
/// penalty); with none, the "No penalty is imposed." row.
#[test]
fn clan_penalty_lists_the_penalties() {
    let mut s = Scene::new();
    let page = html(&s.user_command(ME, 100)).pop().unwrap().html;
    assert!(page.contains("<td width=170>Unable to invite a clan member.</td><td width=100 align=center>"));
    assert!(!page.contains("Unable to join a clan."));
    assert!(!page.contains("No penalty is imposed."));

    let page = html(&s.user_command(LONER, 100)).pop().unwrap().html;
    assert!(page.contains("<td width=170>Unable to join a clan.</td><td width=100 align=center>"));
    assert!(!page.contains("Unable to invite a clan member."));

    // A player with no penalty at all.
    s.enter(pdata(0x1000_0003, "Clean", 0, 0));
    s.log.take();
    let page = html(&s.user_command(0x1000_0003, 100)).pop().unwrap().html;
    assert!(page.contains("<td width=170>No penalty is imposed.</td><td width=100 align=center></td>"));
}

/// `ClanWarsList`: the three lists of the war sets, each with its header and the `=====` footer;
/// a player without a clan gets `NOT_JOINED_IN_ANY_CLAN`.
#[test]
fn the_clan_war_lists() {
    let mut s = Scene::new();
    let list = |out: &Sent| -> Vec<(SystemMessageId, Vec<String>)> {
        messages(out)
            .into_iter()
            .map(|m| {
                let texts = m
                    .params
                    .iter()
                    .map(|p| match p {
                        SystemMessageParam::Text(t) => t.clone(),
                        other => format!("{other:?}"),
                    })
                    .collect();
                (m.id, texts)
            })
            .collect()
    };

    // 88: the clans I declared war on that did not declare war on me: Prey.
    assert_eq!(
        list(&s.user_command(ME, 88)),
        vec![
            (SystemMessageId::ClansYouVeDeclaredWarOn, vec![]),
            (SystemMessageId::S1NoAllianceExists, vec!["Prey".to_owned()]),
            (SystemMessageId::Empty3, vec![]),
        ]
    );
    // 89: the clans that declared war on me and I did not: Hunter.
    assert_eq!(
        list(&s.user_command(ME, 89)),
        vec![
            (SystemMessageId::ClansThatHaveDeclaredWarOnYou, vec![]),
            (SystemMessageId::S1NoAllianceExists, vec!["Hunter".to_owned()]),
            (SystemMessageId::Empty3, vec![]),
        ]
    );
    // 90: both ways: Mutual, which is in an alliance.
    assert_eq!(
        list(&s.user_command(ME, 90)),
        vec![
            (SystemMessageId::WarList, vec![]),
            (
                SystemMessageId::S1S2Alliance,
                vec!["Mutual".to_owned(), "Allies".to_owned()]
            ),
            (SystemMessageId::Empty3, vec![]),
        ]
    );
    assert_eq!(
        list(&s.user_command(LONER, 88)),
        vec![(SystemMessageId::NotJoinedInAnyClan, vec![])]
    );
}

/// `SiegeStatus`: only a noble clan leader sees it, and only during a siege of its clan.
#[test]
fn siege_status_is_refused_without_a_siege() {
    let mut s = Scene::new();
    for player in [ME, LONER] {
        let out = s.user_command(player, 99);
        assert_eq!(
            messages(&out).iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![
                SystemMessageId::OnlyAClanLeaderThatIsANoblesseCanViewTheSiegeWarStatusWindowDuringASiegeWar
            ]
        );
    }
}

/// The user commands that are still not ported do nothing (`/mybirthday`, `/unstuck`).
#[test]
fn the_unported_user_commands_stay_quiet() {
    let mut s = Scene::new();
    assert!(s.user_command(ME, 126).is_empty());
}

/// `PlayerHelp`: `player_help <page>[#<itemId>]` shows `data/html/help/<page>` as a page of the item.
#[test]
fn player_help_shows_a_help_page() {
    let mut s = Scene::new();
    let handlers = BypassHandlers::default();
    let handler = handlers.get("player_help rules.htm").unwrap();
    assert!(handler(
        &mut s.world,
        ObjectId(ME),
        "player_help rules.htm#57",
        None
    ));
    let page = html(&s.log.take()).pop().unwrap();
    assert_eq!((page.npc_obj_id, page.item_id), (0, 57));
    assert!(page.html.contains("Rules"));

    // A missing page is Java's "My Text is missing" page; `..` and a short command show nothing.
    assert!(handler(
        &mut s.world,
        ObjectId(ME),
        "player_help nothing.htm",
        None
    ));
    let page = html(&s.log.take()).pop().unwrap();
    assert!(
        page.html
            .contains("My Text is missing:<br>data/html/help/nothing.htm")
    );
    assert!(!handler(&mut s.world, ObjectId(ME), "player_help ../x.htm", None));
    assert!(!handler(&mut s.world, ObjectId(ME), "player_help", None));
    assert!(s.log.take().is_empty());
    // A bad item id is an exception Java logs; the handler still reports success.
    assert!(handler(
        &mut s.world,
        ObjectId(ME),
        "player_help rules.htm#x",
        None
    ));
    assert!(s.log.take().is_empty());
}

/// `TutorialClose` sends `TutorialCloseHtml`; `QuestList` opens the quest window only for an
/// `Adventurer` NPC.
#[test]
fn tutorial_close_and_quest_list() {
    let mut s = Scene::new();
    let handlers = BypassHandlers::default();
    let close = handlers.get("tutorial_close").unwrap();
    close(&mut s.world, ObjectId(ME), "tutorial_close", None);
    assert_eq!(names(&s.log.take()), ["TUTORIAL_CLOSE_HTML"]);

    let guide = s.world.spawn_npc_at(9902, X + 20, Y, Z, 0).unwrap();
    let villager = s.world.spawn_npc_at(9903, X + 30, Y, Z, 0).unwrap();
    s.log.take();
    let quest_list = handlers.get("questlist").unwrap();
    assert!(!quest_list(&mut s.world, ObjectId(ME), "questlist", None));
    assert!(!quest_list(
        &mut s.world,
        ObjectId(ME),
        "questlist",
        Some(villager)
    ));
    assert!(s.log.take().is_empty());
    assert!(quest_list(&mut s.world, ObjectId(ME), "questlist", Some(guide)));
    assert_eq!(names(&s.log.take()), ["EX_SHOW_QUEST_INFO"]);
}

/// `ArtefactClick`: the first click targets, a second click close by does nothing more (no
/// dialog, unlike `NpcClick`).
#[test]
fn an_artefact_is_targeted_and_never_talks() {
    let mut s = Scene::new();
    let artefact = s.world.spawn_npc_at(9901, X + 20, Y, Z, 0).unwrap();
    s.log.take();
    for _ in 0..2 {
        s.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME).unwrap()),
            input: ClientInput::Action {
                object_id: artefact.0,
                action_id: 0,
            },
        });
    }
    assert_eq!(s.world.target_of(ObjectId(ME)), Some(artefact));
    let out = s.log.take();
    assert!(html(&out).is_empty(), "{:?}", names(&out));
    assert!(
        !names(&out).contains(&"MOVE_TO_PAWN".to_owned()),
        "{:?}",
        names(&out)
    );
}
