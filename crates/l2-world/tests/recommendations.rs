#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Recommendations and the daily reset (M5-FU-03): `RequestVoteNew`, `RecoGiveTask`,
//! `DailyResetManager.onReset` (`resetNevit`, `resetRecommends`).
//!
//! The expected numbers and messages are worked out by hand from the Java sources named in
//! `rewards/recommendations.rs`. Every clock is pinned: the daily reset is scheduled with
//! `start_daily_reset_task_at` and a fixed "now", never the wall clock.
//! Java: network/clientpackets/RequestVoteNew.java, entity/actor/tasks/player/RecoGiveTask.java,
//! managers/DailyResetManager.java

use std::sync::{Arc, Mutex};

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::enterworld::ExVoteSystemInfo;
use l2_protocol_game::server::{ExNevitAdventTimeChange, SystemMessage, SystemMessageParam, UserInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

// ------------------------------------------------------------------ harness

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

#[derive(Default)]
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

fn class_templates() -> l2_data::PlayerTemplates {
    let levels: String = (1..=85)
        .map(|l| format!("<level val=\"{l}\"><hp>{l}</hp><mp>{l}</mp><cp>{l}</cp></level>"))
        .collect();
    let xml = format!(
        r#"<list><classId>10</classId><staticData>
<baseINT>41</baseINT><baseSTR>22</baseSTR><baseCON>27</baseCON><baseMEN>39</baseMEN><baseDEX>21</baseDEX><baseWIT>20</baseWIT>
<collisionMale><radius>7.5</radius><height>22.8</height></collisionMale>
<collisionFemale><radius>6.5</radius><height>22.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()
}

/// One monster the player can select to get "an incorrect target".
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="10" name="Monster">
    <acquire exp="1000" sp="100" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ALICE: i32 = 0x2000_0001;
const BOB: i32 = 0x2000_0002;

/// A logout a long time in the future: no vitality recovered at the enter and no Nevit reset
/// (`lastAccess` is compared with the real clock there).
const FAR_FUTURE_MS: i64 = 4_000_000_000_000;

fn profile(recom_have: i32, recom_left: i32) -> PlayerProfile {
    PlayerProfile {
        recom_have,
        recom_left,
        recom_time_left_ms: 3_600_000,
        last_access: FAR_FUTURE_MS,
        ..PlayerProfile::default()
    }
}

fn player_data(id: i32, name: &str, profile: PlayerProfile) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.into(),
        title: String::new(),
        class_id: 10,
        base_class: 10,
        race: 0,
        sex: 1,
        face: 1,
        hair_style: 2,
        hair_color: 3,
        level: 45,
        exp: 1000,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 45.0,
        max_hp: 45.0,
        cur_mp: 45.0,
        max_mp: 45.0,
        cur_cp: 45.0,
        max_cp: 45.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile,
    }
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new(nevit_enabled: bool) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        world.set_settings(WorldSettings {
            nevit_enabled,
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(StatData {
            player_templates: Arc::new(class_templates()),
            ..StatData::default()
        })));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        world.set_npc_data(npcs, Arc::new(SpawnData::default()));
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

    /// Both players stand in the same place; what the enter sent is dropped.
    fn two_players(nevit_enabled: bool, alice: PlayerProfile, bob: PlayerProfile) -> Self {
        let mut s = Self::new(nevit_enabled);
        s.enter(player_data(ALICE, "Alice", alice));
        s.enter(player_data(BOB, "Bob", bob));
        s.log.take();
        s
    }

    fn input(&mut self, who: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who).unwrap()),
            input,
        });
    }

    fn vote(&mut self, who: i32, target_id: i32) {
        self.input(who, ClientInput::VoteNew { target_id });
    }

    fn target(&mut self, who: i32, target: i32) {
        self.world.set_target(ObjectId(who), Some(ObjectId(target)));
        assert_eq!(self.world.target_of(ObjectId(who)), Some(ObjectId(target)));
        self.log.take();
    }

    fn ticks(&mut self, n: u64) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn player(&self, id: i32) -> &Player {
        self.world.player(ObjectId(id)).unwrap()
    }

    fn packets(out: &[Out], to: i32) -> Vec<&[u8]> {
        out.iter()
            .filter_map(|o| match o {
                Out::Packet(id, bytes) if *id == to => Some(bytes.as_slice()),
                _ => None,
            })
            .collect()
    }

    fn persisted(out: &[Out]) -> Vec<&PersistCommand> {
        out.iter()
            .filter_map(|o| match o {
                Out::Persist(c) => Some(c),
                Out::Packet(..) => None,
            })
            .collect()
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

fn of<'a>(out: &'a [Out], to: i32, name: &str) -> Vec<&'a [u8]> {
    Scene::packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == name)
        .collect()
}

/// The system messages one client got (id and parameters).
fn messages(out: &[Out], to: i32) -> Vec<SystemMessage> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages(out, to).into_iter().map(|m| m.id).collect()
}

/// (recom_left, recom_have, bonus time, bonus value, status) of each `ExVoteSystemInfo`.
fn vote_infos(out: &[Out], to: i32) -> Vec<(i32, i32, i32, i32, i32)> {
    of(out, to, "EX_VOTE_SYSTEM_INFO")
        .iter()
        .map(|raw| {
            let p = decode::<ExVoteSystemInfo>(raw);
            (
                p.recom_left,
                p.recom_have,
                p.bonus_time,
                p.bonus_value,
                p.bonus_type,
            )
        })
        .collect()
}

/// (started, time) of each `ExNevitAdventTimeChange`.
fn time_changes(out: &[Out], to: i32) -> Vec<(bool, i32)> {
    of(out, to, "EX_NAVIT_ADVENT_TIME_CHANGE")
        .iter()
        .map(|raw| {
            let p = decode::<ExNevitAdventTimeChange>(raw);
            (p.started, p.time)
        })
        .collect()
}

fn user_infos(out: &[Out], to: i32) -> Vec<UserInfo> {
    of(out, to, "USER_INFO")
        .iter()
        .map(|raw| decode::<UserInfo>(raw))
        .collect()
}

// ------------------------------------------------------------------ RequestVoteNew

/// A vote: the target gets one more, the player one less; the messages carry the names and the
/// number left; the target's `UserInfo` is refreshed (the player's is not, a Java quirk);
/// `ExVoteSystemInfo` goes to both with Nevit on; both rows are written with
/// `storeRecommendationValues` (without `time_left`).
#[test]
fn a_vote_moves_one_recommendation() {
    let mut s = Scene::two_players(true, profile(0, 20), profile(7, 3));
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    let out = s.log.take();

    assert_eq!((s.player(ALICE).recom_have, s.player(ALICE).recom_left), (0, 19));
    assert_eq!((s.player(BOB).recom_have, s.player(BOB).recom_left), (8, 3));

    // YOU_HAVE_RECOMMENDED_C1_YOU_HAVE_S2_RECOMMENDATIONS_LEFT: $c1 Bob, $s2 19.
    let to_alice = messages(&out, ALICE);
    assert_eq!(to_alice.len(), 1, "{to_alice:?}");
    assert_eq!(
        to_alice[0].id,
        SystemMessageId::YouHaveRecommendedC1YouHaveS2RecommendationsLeft
    );
    assert_eq!(
        to_alice[0].params,
        vec![
            SystemMessageParam::PlayerName("Bob".into()),
            SystemMessageParam::IntNumber(19)
        ]
    );
    // YOU_HAVE_BEEN_RECOMMENDED_BY_C1: $c1 Alice.
    let to_bob = messages(&out, BOB);
    assert_eq!(to_bob.len(), 1, "{to_bob:?}");
    assert_eq!(to_bob[0].id, SystemMessageId::YouHaveBeenRecommendedByC1);
    assert_eq!(
        to_bob[0].params,
        vec![SystemMessageParam::PlayerName("Alice".into())]
    );

    // Bob's UserInfo shows 8; Alice gets none.
    let infos = user_infos(&out, BOB);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].recom_have, 8);
    assert!(user_infos(&out, ALICE).is_empty());

    // ExVoteSystemInfo: (left, have, hourglass 3600 s, bonus %, active). Level 45 is row 4;
    // 8 recommendations is column 0 (7 / 10): 8%. Alice has 0 received: no bonus.
    assert_eq!(vote_infos(&out, ALICE), vec![(19, 0, 3600, 0, 0)]);
    assert_eq!(vote_infos(&out, BOB), vec![(3, 8, 3600, 8, 0)]);

    // Java writes the player first, then the target.
    let persisted = Scene::persisted(&out);
    assert_eq!(
        persisted,
        vec![
            &PersistCommand::RecoValues {
                owner: ObjectId(ALICE),
                rec_have: 0,
                rec_left: 19
            },
            &PersistCommand::RecoValues {
                owner: ObjectId(BOB),
                rec_have: 8,
                rec_left: 3
            },
        ]
    );
    // The `REPLACE` left no `time_left`; the mirror of the row says so.
    assert_eq!(s.player(ALICE).data.profile.recom_time_left_ms, 0);
    assert_eq!(s.player(BOB).data.profile.recom_time_left_ms, 0);
    assert_eq!(s.player(BOB).data.profile.recom_have, 8);
}

/// The full store writes the live counters (`storeRecommendations`), with the time the hourglass
/// has left again: the `REPLACE` of a vote had dropped it.
#[test]
fn the_next_store_writes_the_counters_and_the_hourglass_time() {
    let mut s = Scene::two_players(true, profile(0, 20), profile(7, 3));
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    s.log.take();
    s.input(BOB, ClientInput::Logout);
    let out = s.log.take();
    let stored = Scene::persisted(&out)
        .into_iter()
        .find_map(|c| match c {
            PersistCommand::StorePlayer(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    let profile = &stored.profile;
    assert!(profile.reco_store);
    assert_eq!(
        (profile.recom_have, profile.recom_left, profile.recom_time_left_ms),
        (8, 3, 3_600_000)
    );
}

/// Without Nevit the recommendations are never loaded (`loadRecommendations` is only called by
/// the Nevit code), so nobody has any to give: a Java quirk the port keeps.
#[test]
fn without_nevit_the_recommendations_are_not_loaded() {
    let mut s = Scene::two_players(false, profile(0, 20), profile(0, 0));
    assert_eq!((s.player(ALICE).recom_have, s.player(ALICE).recom_left), (0, 0));
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::YouAreOutOfRecommendationsTryAgainLater]
    );
    assert!(vote_infos(&out, ALICE).is_empty());
}

/// Every refusal of `RequestVoteNew.runImpl`, in Java's order, and what each one changes (nothing).
#[test]
fn a_vote_is_refused_like_java_does() {
    let mut s = Scene::two_players(true, profile(0, 20), profile(255, 20));

    // No target: SELECT_TARGET.
    s.vote(ALICE, BOB);
    let out = s.log.take();
    assert_eq!(message_ids(&out, ALICE), vec![SystemMessageId::SelectTarget]);

    // A target that is no player: THAT_IS_AN_INCORRECT_TARGET.
    let npc = s.world.spawn_npc_at(9001, X, Y, Z, 0).unwrap();
    s.target(ALICE, npc.0);
    s.vote(ALICE, npc.0);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::ThatIsAnIncorrectTarget]
    );

    // The client's id is not the target: ignored without a word.
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB + 1);
    assert!(s.log.take().is_empty());

    // Yourself.
    s.target(ALICE, ALICE);
    s.vote(ALICE, ALICE);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::YouCannotRecommendYourself]
    );

    // Bob has 255: "can no longer receive".
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::YourSelectedTargetCanNoLongerReceiveARecommendation]
    );
    assert_eq!(s.player(BOB).recom_have, 255);
}

/// The check on the player's own number: no recommendations left.
#[test]
fn a_player_without_recommendations_left_cannot_vote() {
    let mut s = Scene::two_players(true, profile(0, 0), profile(0, 0));
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::YouAreOutOfRecommendationsTryAgainLater]
    );
    assert!(Scene::persisted(&out).is_empty());
    assert_eq!(s.player(BOB).recom_have, 0);
}

/// The last recommendation brings the player to 0, and 255 is the cap of the target.
#[test]
fn the_counters_stop_at_zero_and_255() {
    let mut s = Scene::two_players(true, profile(0, 1), profile(254, 0));
    s.target(ALICE, BOB);
    s.vote(ALICE, BOB);
    assert_eq!((s.player(ALICE).recom_left, s.player(BOB).recom_have), (0, 255));
    s.log.take();
    s.vote(ALICE, BOB);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::YouAreOutOfRecommendationsTryAgainLater]
    );
}

// ------------------------------------------------------------------ RecoGiveTask

/// 2 hours after the login 10 recommendations, then 1 an hour; each run tells the player, sends
/// `UserInfo` and writes the counters (`storeRecommendationValues`).
#[test]
fn the_recommendations_are_given_after_two_hours_and_then_every_hour() {
    let mut s = Scene::new(true);
    s.enter(player_data(ALICE, "Alice", profile(0, 5)));
    s.log.take();
    assert!(s.world.has_reco_give_task(ObjectId(ALICE)));
    let hour = 72_000; // 3_600_000 ms in 50 ms ticks

    s.ticks(2 * hour - 1);
    let out = s.log.take();
    assert!(
        !message_ids(&out, ALICE).contains(&SystemMessageId::YouObtainedS1RecommendationS),
        "too early"
    );
    s.ticks(1);
    let out = s.log.take();
    let obtained: Vec<_> = messages(&out, ALICE)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::YouObtainedS1RecommendationS)
        .collect();
    assert_eq!(obtained.len(), 1);
    assert_eq!(obtained[0].params, vec![SystemMessageParam::IntNumber(10)]);
    assert_eq!(s.player(ALICE).recom_left, 15);
    let infos = user_infos(&out, ALICE);
    assert_eq!(infos.last().map(|i| i.recom_left), Some(15));
    assert!(Scene::persisted(&out).contains(&&PersistCommand::RecoValues {
        owner: ObjectId(ALICE),
        rec_have: 0,
        rec_left: 15,
    }));

    // One hour later: 1.
    s.ticks(hour - 1);
    s.log.take();
    s.ticks(1);
    let out = s.log.take();
    let obtained: Vec<_> = messages(&out, ALICE)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::YouObtainedS1RecommendationS)
        .collect();
    assert_eq!(obtained[0].params, vec![SystemMessageParam::IntNumber(1)]);
    assert_eq!(s.player(ALICE).recom_left, 16);
}

/// The task starts only with Nevit on (`checkRecoBonusTask` is only called then), and a logout
/// stops it.
#[test]
fn the_give_task_needs_nevit_and_stops_at_the_logout() {
    let mut s = Scene::new(false);
    s.enter(player_data(ALICE, "Alice", profile(0, 5)));
    assert!(!s.world.has_reco_give_task(ObjectId(ALICE)));

    let mut s = Scene::new(true);
    s.enter(player_data(ALICE, "Alice", profile(0, 5)));
    assert!(s.world.has_reco_give_task(ObjectId(ALICE)));
    s.input(ALICE, ClientInput::Logout);
    assert!(!s.world.has_reco_give_task(ObjectId(ALICE)));
    s.log.take();
    s.ticks(150_000);
    assert!(s.log.take().is_empty());
}

// ------------------------------------------------------------------ the daily reset

/// 2026-10-19 00:00:00 UTC.
const MIDNIGHT: i64 = 1_792_368_000_000;
const HOUR: i64 = 3_600_000;

fn variable(s: &Scene, who: i32, name: &str) -> Option<String> {
    s.player(who).data.profile.variables.get(name).cloned()
}

/// `resetNevit`: the advent time of everybody online is 0 and they are told (paused: the client
/// gets `ExNevitAdventTimeChange(0, true)`, which it reads as "not started"); `resetRecommends`:
/// the database statements go out first, the players get 0 to give (a Java quirk) and then,
/// with Nevit on, the hourglass restarts from the row (one hour, 20 to give, 20 fewer received).
#[test]
fn the_daily_reset_with_nevit() {
    let mut alice = profile(45, 3);
    alice.variables.insert("hunting_time".into(), "500".into());
    let mut s = Scene::two_players(true, alice, profile(5, 0));
    s.world.daily_reset();
    let out = s.log.take();

    // resetNevit
    assert_eq!(variable(&s, ALICE, "hunting_time").as_deref(), Some("0"));
    assert_eq!(variable(&s, BOB, "hunting_time").as_deref(), Some("0"));
    assert_eq!(time_changes(&out, ALICE), vec![(false, 0)]);
    assert_eq!(time_changes(&out, BOB), vec![(false, 0)]);

    // resetRecommends: the first ExVoteSystemInfo is the one of the loop (0 to give, 25
    // received, the hourglass has 3600 s), the second the restarted hourglass (20 to give).
    // 25 received at level 45 is column 2 of row 4: 25%. Bob: 0 received, no bonus.
    assert_eq!(
        vote_infos(&out, ALICE),
        vec![(0, 25, 3600, 25, 0), (20, 25, 3600, 25, 0)]
    );
    assert_eq!(
        vote_infos(&out, BOB),
        vec![(0, 0, 3600, 0, 0), (20, 0, 3600, 0, 0)]
    );
    // broadcastUserInfo: the player's UserInfo shows the numbers of that moment.
    let infos = user_infos(&out, ALICE);
    assert_eq!((infos[0].recom_left, infos[0].recom_have), (0, 25));
    assert_eq!((s.player(ALICE).recom_left, s.player(ALICE).recom_have), (20, 25));
    assert_eq!((s.player(BOB).recom_left, s.player(BOB).recom_have), (20, 0));

    // The mirror of the rows, and the writes in Java's order: the `UPDATE`s, then the variables
    // of each player (`storeMe`).
    assert_eq!(s.player(ALICE).data.profile.recom_time_left_ms, 3_600_000);
    let persisted = Scene::persisted(&out);
    assert_eq!(persisted[0], &PersistCommand::ResetRecommends);
    assert_eq!(
        persisted[1],
        &PersistCommand::PlayerVariables {
            owner: ObjectId(ALICE),
            upserts: vec![("hunting_time".into(), "0".into())],
            removed: vec![],
        }
    );
    assert_eq!(persisted.len(), 3, "{persisted:?}");
}

/// Without Nevit nothing about the advent time happens, the players keep the 0 to give until they
/// log in again, and there is no hourglass to restart.
#[test]
fn the_daily_reset_without_nevit() {
    let mut s = Scene::two_players(false, profile(45, 3), profile(10, 9));
    s.world.daily_reset();
    let out = s.log.take();
    assert!(time_changes(&out, ALICE).is_empty());
    assert_eq!(variable(&s, ALICE, "hunting_time"), None);
    // One ExVoteSystemInfo from the loop even with Nevit off: nothing was loaded (0 to give,
    // 0 received), there is no hourglass ("maintain", 10).
    assert_eq!(vote_infos(&out, ALICE), vec![(0, 0, 0, 0, 10)]);
    assert_eq!((s.player(ALICE).recom_left, s.player(ALICE).recom_have), (0, 0));
    assert_eq!((s.player(BOB).recom_left, s.player(BOB).recom_have), (0, 0));
    // The database statements go out all the same.
    assert_eq!(Scene::persisted(&out), vec![&PersistCommand::ResetRecommends]);
}

/// The schedule: pinned to 06:00 UTC the first run is 30 minutes away (not before), and it
/// repeats every 24 hours.
#[test]
fn the_daily_reset_runs_at_half_past_six_and_every_24_hours() {
    let mut s = Scene::two_players(true, profile(45, 3), profile(0, 0));
    assert!(!s.world.has_daily_reset_task());
    s.world.start_daily_reset_task_at(MIDNIGHT + 6 * HOUR, |_| 0);
    assert!(s.world.has_daily_reset_task());
    let resets = |out: &[Out]| {
        Scene::persisted(out)
            .iter()
            .filter(|c| ***c == PersistCommand::ResetRecommends)
            .count()
    };

    // 30 minutes = 36000 ticks.
    s.ticks(35_999);
    assert_eq!(resets(&s.log.take()), 0);
    s.ticks(1);
    assert_eq!(resets(&s.log.take()), 1);

    // 24 hours later (1_728_000 ticks): once more, not before.
    s.ticks(1_727_999);
    assert_eq!(resets(&s.log.take()), 0);
    s.ticks(1);
    assert_eq!(resets(&s.log.take()), 1);
}

/// A reset in another zone is as far away as the zone's local 06:30 (pinned: UTC+2 at 03:00 UTC
/// means 05:00 local, 90 minutes to go).
#[test]
fn the_schedule_follows_the_local_zone() {
    let mut s = Scene::new(true);
    s.world
        .start_daily_reset_task_at(MIDNIGHT + 3 * HOUR, |_| 2 * HOUR);
    let resets = |out: &[Out]| {
        Scene::persisted(out)
            .iter()
            .filter(|c| ***c == PersistCommand::ResetRecommends)
            .count()
    };
    s.ticks(107_999); // 90 minutes = 108000 ticks
    assert_eq!(resets(&s.log.take()), 0);
    s.ticks(1);
    assert_eq!(resets(&s.log.take()), 1);
}
