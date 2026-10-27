#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The community board (S-HAND-01): `RequestShowBoard`, `RequestBBSwrite` and the pages of the
//! handlers of `dist/game/data/scripts/handlers/bypass/communityboard/`. The expected values come
//! from the Java code named in each test; the pages are small files in a temporary datapack root
//! that print the placeholders the Java code replaces.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ShowBoard, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, PlayerClanState};
use l2_world::residence::{CastleRecord, ClanHallRecord, ResidenceLoad};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<(i32, Vec<u8>)>>>);

impl Log {
    fn take(&self) -> Vec<(i32, Vec<u8>)> {
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
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Recorder(Persisted);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

fn class_xml() -> String {
    let mut rows = String::new();
    for l in 1..=40 {
        rows.push_str(&format!(
            "<level val=\"{l}\"><hp>{l}</hp><mp>{l}</mp><cp>{l}</cp></level>"
        ));
    }
    format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

struct Data(StatData);

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
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// The leader of the Lions, a member of the Lions, a player without a clan, the leader of the Wolves.
const LEADER: i32 = 0x6000_0001;
const MEMBER: i32 = 0x6000_0002;
const LONER: i32 = 0x6000_0003;
const WOLF: i32 = 0x6000_0004;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const TINY: i32 = 700;

const PAGES: &[(&str, &str)] = &[
    (
        "home.html",
        "<html>HOME fav=%fav_count% region=%region_count% clans=%clan_count%</html>",
    ),
    ("homepage.html", "<html>HOMEPAGE</html>"),
    ("mail.html", "<html>MAIL</html>"),
    ("memo.html", "<html>MEMO</html>"),
    ("friends_list.html", "<html>FRIENDS</html>"),
    ("friends_block_list.html", "<html>IGNORED</html>"),
    ("favorite.html", "<html>FAV [%fav_list%]</html>"),
    (
        "favorite_list.html",
        "(%fav_id%|%fav_title%|%fav_bypass%|%fav_add_date%)",
    ),
    ("region.html", "<html>REGIONS %region_list%</html>"),
    (
        "region_list.html",
        "<r>%region_id%|%region_name%|%region_owning_clan%|%region_owning_clan_alliance%|%region_tax_rate%</r>",
    ),
    (
        "region_show.html",
        "<html>%regionName%|%tax%|%lord%|%clanName%|%allyName%|%siegeDate%|%hallsList%</html>",
    ),
];

fn clan_record(id: i32, name: &str, level: i32, leader: i32, members: &[i32]) -> ClanRecord {
    ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: members
            .iter()
            .map(|&object_id| ClanMemberRecord {
                object_id,
                name: format!("P{}", object_id - 0x6000_0000),
                level: 40,
                power_grade: i32::from(object_id != leader) + 1,
                ..ClanMemberRecord::default()
            })
            .collect(),
        notices: if id == LIONS {
            vec![("TRUE".to_owned(), "Hello<br>clan".to_owned())]
        } else {
            Vec::new()
        },
        ..ClanRecord::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: tempfile::TempDir,
}

fn pdata(id: i32, clan_id: i32, karma: i32) -> PlayerData {
    let mut data = PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma,
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
    if clan_id != 0 {
        data.profile.clan = Some(PlayerClanState {
            clan_id,
            ..PlayerClanState::default()
        });
    }
    data
}

impl Scene {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for (page, text) in PAGES {
            let file = root.path().join("data/html/CommunityBoard").join(page);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(persisted.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.load_clans(
            vec![
                clan_record(LIONS, "Lions", 5, LEADER, &[LEADER, MEMBER]),
                clan_record(WOLVES, "Wolves", 3, WOLF, &[WOLF]),
                // Level 1: no clan community.
                clan_record(TINY, "Tiny", 1, 0x6000_0010, &[0x6000_0010]),
            ],
            Vec::new(),
        );
        // Castle 1 (Gludio, the Wolves', 7 percent tax, a siege date) and two halls in Gludio.
        world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                owner_id: WOLVES,
                tax_percent: 7,
                // 2026-10-27 13:45:09 UTC.
                siege_date: 1_793_108_709_000,
                ..CastleRecord::default()
            }],
            clan_halls: vec![
                ClanHallRecord {
                    id: 21,
                    name: "Hall 21".to_owned(),
                    owner_id: WOLVES,
                    location: "Gludio".to_owned(),
                    ..ClanHallRecord::default()
                },
                ClanHallRecord {
                    id: 22,
                    name: "Hall 22".to_owned(),
                    owner_id: 0,
                    location: "Gludio".to_owned(),
                    ..ClanHallRecord::default()
                },
                ClanHallRecord {
                    id: 40,
                    name: "Hall 40".to_owned(),
                    owner_id: 0,
                    location: "Dion".to_owned(),
                    ..ClanHallRecord::default()
                },
            ],
            ..ResidenceLoad::default()
        });
        Self {
            world,
            log,
            persisted,
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
        self.log.take();
        self.persisted.take();
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn bypass(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    fn write(&mut self, player: i32, url: &str, args: [&str; 5]) {
        self.input(
            player,
            ClientInput::BbsWrite {
                url: url.to_owned(),
                args: args.map(str::to_owned),
            },
        );
    }

    /// What the world sent to `player` since the last call.
    fn out(&self) -> Vec<(i32, Vec<u8>)> {
        self.log.take()
    }

    /// The answer of the persistence task to a favorites read.
    fn answer(&mut self, player: i32, kind: FavoriteRead, result: Result<FavoriteAnswer, ()>) {
        self.world.handle(WorldCommand::BbsFavoritesLoaded {
            player: ObjectId(player),
            kind,
            result,
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

/// The `ShowBoard` packets sent to a player: the content (`<id>\u{8}<page>`).
fn board(out: &[(i32, Vec<u8>)], to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SHOW_BOARD")
        .map(|(_, b)| decode::<ShowBoard>(b).content)
        .collect()
}

/// The page of a board answer: the "101" packet's html (the pages here are one packet long).
fn page(out: &[(i32, Vec<u8>)], to: i32) -> String {
    let packets = board(out, to);
    assert_eq!(packets.len(), 3, "a short page is three packets: {packets:?}");
    assert!(packets[1].starts_with("102\u{8}") && packets[2].starts_with("103\u{8}"));
    packets[0].strip_prefix("101\u{8}").unwrap().to_owned()
}

fn texts_to(out: &[(i32, Vec<u8>)], to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SYSTEM_MESSAGE")
        .map(|(_, b)| decode::<SystemMessage>(b))
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn system_ids_to(out: &[(i32, Vec<u8>)], to: i32) -> Vec<SystemMessageId> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SYSTEM_MESSAGE")
        .map(|(_, b)| decode::<SystemMessage>(b).id)
        .filter(|id| *id != SystemMessageId::S12)
        .collect()
}

// ---------------------------------------------------------------------------------- tests

/// `RequestShowBoard` runs `BBSDefault` (`_bbshome`): the home page waits for the favorite count
/// (`HomeBoard.getFavoriteCount`), then replaces `%fav_count%`, `%region_count%` (always 0) and
/// `%clan_count%` (`ClanTable.getClanCount`).
#[test]
fn show_board_opens_the_home_page_with_the_counts() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    s.input(LONER, ClientInput::ShowBoard);
    assert!(
        s.out().is_empty(),
        "nothing goes out before the database answered"
    );
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::BbsFavoriteRequest {
            player: ObjectId(LONER),
            kind: FavoriteRead::Count,
        }]
    );
    s.answer(LONER, FavoriteRead::Count, Ok(FavoriteAnswer::Count(4)));
    let out = s.out();
    assert_eq!(page(&out, LONER), "<html>HOME fav=4 region=0 clans=3</html>");
}

/// A failed count read still shows the page with 0 favorites (the Java `catch` returns the 0).
#[test]
fn a_failed_count_read_shows_zero_favorites() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    s.bypass(LONER, "_bbshome");
    s.persisted.take();
    s.answer(LONER, FavoriteRead::Count, Err(()));
    assert_eq!(page(&s.out(), LONER), "<html>HOME fav=0 region=0 clans=3</html>");
}

/// `EnableCommunityBoard = False`: `THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE` for both packets.
#[test]
fn a_disabled_board_says_it_is_offline() {
    let mut s = Scene::new();
    s.world.set_community_board_settings(CommunityBoardSettings {
        enabled: false,
        ..CommunityBoardSettings::default()
    });
    s.enter(pdata(LONER, 0, 0));
    s.input(LONER, ClientInput::ShowBoard);
    s.bypass(LONER, "_bbsclan");
    s.write(LONER, "Notice", ["", "", "", "", ""]);
    let out = s.out();
    assert_eq!(
        system_ids_to(&out, LONER),
        vec![SystemMessageId::TheCommunityServerIsCurrentlyOffline; 3]
    );
    assert!(board(&out, LONER).is_empty());
    assert!(s.persisted.take().is_empty());
}

/// `HomeBoard.KARMA_CHECK` and `COMMUNITYBOARD_PEACE_ONLY`.
#[test]
fn karma_and_peace_checks_of_the_home_board() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 100));
    s.bypass(LONER, "_bbshome");
    let out = s.out();
    assert_eq!(
        texts_to(&out, LONER),
        vec!["Players with Karma cannot use the Community Board."]
    );
    assert!(s.persisted.take().is_empty());

    s.world.set_community_board_settings(CommunityBoardSettings {
        karma_disabled: false,
        peace_only: true,
        ..CommunityBoardSettings::default()
    });
    s.bypass(LONER, "_bbshome");
    // No zones are loaded, so nobody stands in a peace zone.
    assert_eq!(
        texts_to(&s.out(), LONER),
        vec!["Community Board cannot be used out of peace zone."]
    );
}

/// The boards that only show a page: homepage, mail, memo, friends and the ignore list.
#[test]
fn the_page_boards_show_their_file() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    for (command, expected) in [
        ("_bbslink", "<html>HOMEPAGE</html>"),
        ("_maillist", "<html>MAIL</html>"),
        ("_bbsmemo", "<html>MEMO</html>"),
        ("_bbstopics", "<html>MEMO</html>"),
        ("_friendlist", "<html>FRIENDS</html>"),
        ("_friendblocklist", "<html>IGNORED</html>"),
    ] {
        s.bypass(LONER, command);
        assert_eq!(page(&s.out(), LONER), expected, "{command}");
    }
    assert!(s.persisted.take().is_empty(), "no database for these pages");
}

/// A page of a missing file is `null` in Java: nothing is sent.
#[test]
fn a_missing_page_sends_nothing() {
    let mut s = Scene::new();
    std::fs::remove_file(s._root.path().join("data/html/CommunityBoard/mail.html")).unwrap();
    s.world
        .set_html_cache(Arc::new(HtmCache::load(s._root.path(), false)));
    s.enter(pdata(LONER, 0, 0));
    s.bypass(LONER, "_maillist");
    assert!(board(&s.out(), LONER).is_empty());
}

/// `FavoriteBoard`: the list, "add to favorites" (the last page's title and bypass), and delete.
#[test]
fn favorites_list_add_and_delete() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));

    s.bypass(LONER, "_bbsgetfav");
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::BbsFavoriteRequest {
            player: ObjectId(LONER),
            kind: FavoriteRead::List,
        }]
    );
    // 2026-10-27 13:45:09 UTC; the offset of this machine's zone is applied (D-48).
    let added_ms = 1_793_108_709_000;
    s.answer(
        LONER,
        FavoriteRead::List,
        Ok(FavoriteAnswer::List(vec![
            FavoriteEntry {
                fav_id: 12,
                title: "Home".to_owned(),
                bypass: "_bbshome".to_owned(),
                added_ms,
            },
            FavoriteEntry {
                fav_id: 9,
                title: "Clan".to_owned(),
                bypass: "_bbsclan".to_owned(),
                added_ms,
            },
        ])),
    );
    let html = page(&s.out(), LONER);
    // The date is "yyyy-MM-dd HH:mm:ss" in the zone of this machine: the 26th to the 28th.
    assert!(
        html.starts_with("<html>FAV [(12|Home|_bbshome|2026-10-2"),
        "{html}"
    );
    assert!(html.contains(")(9|Clan|_bbsclan|2026-10-2"), "{html}");
    assert!(html.ends_with(")]</html>"), "{html}");
    assert_eq!(html.matches('|').count(), 6);

    // "Add to favorites" with no page shown before does nothing.
    s.bypass(LONER, "bbs_add_fav");
    assert!(s.persisted.take().is_empty());
    // After the clan page the bypass is stored as "Clan&_bbsclan".
    s.bypass(LONER, "_bbsclan");
    s.out();
    s.persisted.take();
    s.bypass(LONER, "bbs_add_fav");
    assert_eq!(
        s.persisted.take(),
        vec![
            PersistCommand::BbsFavorite(FavoriteWrite::Add {
                player_id: LONER,
                title: "Clan".to_owned(),
                bypass: "_bbsclan".to_owned(),
            }),
            PersistCommand::BbsFavoriteRequest {
                player: ObjectId(LONER),
                kind: FavoriteRead::List,
            },
        ]
    );
    // The bypass was taken: a second click does nothing.
    s.bypass(LONER, "bbs_add_fav");
    assert!(s.persisted.take().is_empty());

    s.bypass(LONER, "_bbsdelfav_12");
    assert_eq!(
        s.persisted.take(),
        vec![
            PersistCommand::BbsFavorite(FavoriteWrite::Delete {
                player_id: LONER,
                fav_id: 12,
            }),
            PersistCommand::BbsFavoriteRequest {
                player: ObjectId(LONER),
                kind: FavoriteRead::List,
            },
        ]
    );
    // A bad id is refused (`StringUtil.isNumeric`).
    s.bypass(LONER, "_bbsdelfav_1x");
    s.bypass(LONER, "_bbsdelfav_");
    assert!(s.persisted.take().is_empty());
}

/// `RegionBoard`: the list of castles and the page of one castle with its clan halls.
#[test]
fn region_pages() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    s.bypass(LONER, "_bbsloc");
    // One castle is loaded; `REGIONS[0]` is 1049. Tax: `0.07 * 100` is a Java `double`.
    assert_eq!(
        page(&s.out(), LONER),
        "<html>REGIONS <r>0|1049|<a action=\"bypass _bbsclan_clanhome;600\">Wolves</a>|None|7.000000000000001%</r></html>"
    );

    s.bypass(LONER, "_bbsloc;0");
    let html = page(&s.out(), LONER);
    let offset = l2_util::time::local_offset_millis(1_793_108_709_000);
    let local = 1_793_108_709_000 + offset;
    let (hour, minute) = ((local / 3_600_000) % 24, (local / 60_000) % 60);
    let day = 27 + local / 86_400_000 - 1_793_108_709_000 / 86_400_000;
    let siege = format!("{day:02}-10-2026 {hour:02}:{minute:02}");
    let hall_head = "<br><br><table width=610 bgcolor=A7A19A><tr><td width=5></td><td width=200>Clan Hall Name</td><td width=200>Owning Clan</td><td width=200>Clan Leader Name</td><td width=5></td></tr></table><br1>";
    let row = |name: &str, owner: &str, leader: &str| {
        format!(
            "<table><tr><td width=5></td><td width=200>{name}</td><td width=200>{owner}</td><td width=200>{leader}</td><td width=5></td></tr></table><br1><img src=\"L2UI.Squaregray\" width=605 height=1><br1>"
        )
    };
    // The halls of Gludio, by id: Hall 21 (the Wolves') and Hall 22 (free). Hall 40 is in Dion.
    let halls = format!(
        "{hall_head}{}{}",
        row(
            "Hall 21",
            "<a action=\"bypass _bbsclan_clanhome;600\">Wolves</a>",
            "P4"
        ),
        row("Hall 22", "None", "None"),
    );
    assert_eq!(
        html,
        format!(
            "<html>Gludio|7.000000000000001%|P4|<a action=\"bypass _bbsclan_clanhome;600\">Wolves</a>|None|{siege}|{halls}</html>"
        )
    );

    // A region the server does not have, and bad words: nothing (a warning in the log).
    for command in ["_bbsloc;5", "_bbsloc;x", "_bbsloc;"] {
        s.bypass(LONER, command);
        assert!(board(&s.out(), LONER).is_empty(), "{command}");
    }
}

/// `ClanBoard`: without a clan (or below level 2) `_bbsclan` shows the clan list; the list is
/// ordered by clan id and shows `Clan.getLeaderName()`.
#[test]
fn clan_list_and_home() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    s.bypass(LONER, "_bbsclan");
    let html = page(&s.out(), LONER);
    assert!(html.contains("bypass _bbsclan_clanhome;0\">[GO TO MY CLAN]"));
    let lions = html.find(">Lions</a>").unwrap();
    let wolves = html.find(">Wolves</a>").unwrap();
    let tiny = html.find(">Tiny</a>").unwrap();
    assert!(lions < wolves && wolves < tiny);
    assert!(html.contains("<td FIXWIDTH=200 align=center>P1</td><td FIXWIDTH=100 align=center>5</td><td FIXWIDTH=100 align=center>2</td>"));
    // Three clans: one page of the pager (`3 / 8` rounded up).
    assert!(html.contains("<td> 1 </td>"));

    // `_bbsclan_clanlist;<page>`: a bad page shows the first one (and a warning).
    s.bypass(LONER, "_bbsclan_clanlist;2");
    let second = page(&s.out(), LONER);
    // Three clans: the window of page 2 starts at row 8 (`(index - 1) * 7`), so it is empty; the
    // pager has a previous button (without `bypass `, a Java quirk) and a next button.
    assert!(!second.contains(">Lions</a>"));
    assert!(second.contains("<td><button action=\"_bbsclan_clanlist;1\" back=\"l2ui_ch3.prev1_down\""));
    assert!(
        second.contains("<td><button action=\"bypass _bbsclan_clanlist;3\" back=\"l2ui_ch3.next1_down\"")
    );
    s.bypass(LONER, "_bbsclan_clanlist;x");
    assert!(page(&s.out(), LONER).contains("<td> 1 </td>"));

    // The home page of a clan of level 2 or more.
    s.bypass(LONER, "_bbsclan_clanhome;600");
    let home = page(&s.out(), LONER);
    assert!(home.contains("CLAN NAME</td><td fixWIDTH=195 align=left>Wolves</td>"));
    assert!(home.contains("CLAN LEVEL</td><td fixWIDTH=195 align=left height=16>3</td>"));
    assert!(home.contains("CLAN LEADER</td><td fixWIDTH=195 align=left height=16>P4</td>"));
    // A clan of level 1: the message and the clan list. An unknown clan: nothing.
    s.bypass(LONER, "_bbsclan_clanhome;700");
    let out = s.out();
    assert_eq!(
        system_ids_to(&out, LONER),
        vec![SystemMessageId::ThereAreNoCommunitiesInMyClanClanCommunitiesAreAllowedForClansWithSkillLevelsOf2AndHigher]
    );
    assert!(page(&out, LONER).contains("CLAN COMMUNITY"));
    s.bypass(LONER, "_bbsclan_clanhome;999");
    assert!(board(&s.out(), LONER).is_empty());

    // A member of a clan of level 2 or more goes to the home page of its own clan.
    s.enter(pdata(MEMBER, LIONS, 0));
    s.bypass(MEMBER, "_bbsclan");
    assert!(page(&s.out(), MEMBER).contains("CLAN NAME</td><td fixWIDTH=195 align=left>Lions</td>"));

    // Other words of the board.
    s.bypass(MEMBER, "_bbsclan_unknown");
    assert_eq!(
        page(&s.out(), MEMBER),
        "<html><body><br><br><center>Command _bbsclan_unknown need development.</center><br><br></body></html>"
    );
}

/// The clan notice: the leader's editor fills the multiedit; a member sees the notice when it is on;
/// `Write Notice` of the leader saves it.
#[test]
fn clan_notice_editor_view_and_write() {
    let mut s = Scene::new();
    s.enter(pdata(LEADER, LIONS, 0));
    s.enter(pdata(MEMBER, LIONS, 0));

    s.bypass(LEADER, "_bbsclan_clannotice_edit;500;cnotice");
    let out = s.out();
    let packets = board(&out, LEADER);
    assert_eq!(packets.len(), 2);
    assert!(packets[0].starts_with("1001\u{8}<html><body><br><br><table border=0 width=610>"));
    assert!(packets[0].contains(
        "on&nbsp;&nbsp;&nbsp;/&nbsp;&nbsp;&nbsp;<a action=\"bypass _bbsclan_clannotice_disable\">off</a>"
    ));
    // The "1002" packet: the 17 arguments, the player's name, object id and account, and the notice
    // text. `getNotice()` already took the tags out, so the `<br>` to line separator replace of
    // `fillMultiEditContent` finds none (a Java quirk).
    let expected = format!(
        "1002\u{8}0\u{8}0\u{8}0\u{8}0\u{8}0\u{8}0\u{8}P1\u{8}{LEADER}\u{8}acc{LEADER}\u{8}9\u{8} \u{8} \u{8}Helloclan\u{8}0\u{8}0\u{8}0\u{8}0\u{8}"
    );
    assert_eq!(packets[1], expected);

    // The member sees the view page with the notice (on).
    s.bypass(MEMBER, "_bbsclan_clannotice_edit;500;cnotice");
    let view = page(&s.out(), MEMBER);
    assert!(view.contains("You are not your clan's leader, and therefore cannot change the clan notice"));
    assert!(view.contains("The current clan notice:"));
    assert!(view.contains("align=left>Helloclan</td>"));

    // The leader switches it off: `Clan.setNoticeEnabled(false)` stores the notice and the page
    // shows the "on" link.
    s.bypass(LEADER, "_bbsclan_clannotice_disable");
    let out = s.out();
    assert!(board(&out, LEADER)[0].contains("<a action=\"bypass _bbsclan_clannotice_enable\">on</a>"));
    assert!(!s.world.clan(LIONS).unwrap().notice_enabled);
    assert!(s.persisted.take().iter().any(|p| matches!(
        p,
        PersistCommand::Clan(ClanWrite::Notice {
            clan_id: LIONS,
            enabled: false,
            ..
        })
    )));
    s.bypass(MEMBER, "_bbsclan_clannotice_edit;500;cnotice");
    assert!(!page(&s.out(), MEMBER).contains("The current clan notice:"));

    // `RequestBBSwrite("Notice", "Set", "_", text, ...)`: only the leader's text is saved.
    s.write(MEMBER, "Notice", ["Set", "_", "Hacked", "Content", "Content"]);
    assert!(s.persisted.take().is_empty());
    assert!(system_ids_to(&s.out(), MEMBER).is_empty());
    s.write(LEADER, "Notice", ["Set", "_", "New notice", "Content", "Content"]);
    assert_eq!(
        system_ids_to(&s.out(), LEADER),
        vec![SystemMessageId::YourClanNoticeHasBeenSaved]
    );
    assert_eq!(s.world.clan(LIONS).unwrap().notice.as_deref(), Some("New notice"));
    assert!(s.persisted.take().iter().any(|p| matches!(
        p,
        PersistCommand::Clan(ClanWrite::Notice { clan_id: LIONS, notice, .. }) if notice == "New notice"
    )));
}

/// `CommunityBoardHandler.handleWriteCommand`: the URL picks the board; an unknown URL gets the
/// "not implemented" page; `Topic` is the home board, which has no write.
#[test]
fn write_urls() {
    let mut s = Scene::new();
    s.enter(pdata(LONER, 0, 0));
    s.write(LONER, "Mystery", ["", "", "", "", ""]);
    assert_eq!(
        page(&s.out(), LONER),
        "<html><body><br><br><center>The command: Mystery is not implemented yet.</center><br><br></body></html>"
    );
    for url in ["Topic", "Post", "Region"] {
        s.write(LONER, url, ["", "", "", "", ""]);
        assert!(s.out().is_empty(), "{url}");
    }
    // `Notice` from a player without a clan: nothing.
    s.write(LONER, "Notice", ["Set", "_", "x", "", ""]);
    assert!(s.out().is_empty());
}

/// `_bbs` words that no board knows are not board commands: they fall through to the "not handled"
/// warning of `RequestBypassToServer`, and `admin_` and `npc_` words are not taken by the board.
#[test]
fn unknown_words_are_not_board_commands() {
    let s = Scene::new();
    for command in ["_bbsunknown", "admin_x", "npc_1_Chat", "_bbs", "come_here"] {
        assert!(!s.world.is_community_board_command(command), "{command}");
    }
    assert!(s.world.is_community_board_command("_bbshome"));
}
