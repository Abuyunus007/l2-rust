#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The player's own actions of `RequestActionUse` (M6-ACTION-01): sit and stand, walk and run,
//! the social emotes, the couple actions with `AnswerCoupleAction`, and the `NextAction` of a sit
//! or a couple request made while walking. Expected values follow the Java code named in each
//! test (`RequestActionUse`, `AnswerCoupleAction`, `Player.sitDown`, `CreatureAI.notifyActionArrived`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates, StaticObjectTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ChairSit, ChangeMoveType, ChangeWaitType, ExAskCoupleAction, ExRotation, SocialAction, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Out(i32, Vec<u8>);

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
        self.1.0.lock().unwrap().push(Out(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
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

struct Data {
    stats: StatData,
}

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
    fn access_level_allows_transaction(&self, _level: i32) -> bool {
        true
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml("<list></list>").unwrap());
    let stats = StatData {
        items,
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
    Data { stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);

fn player(id: ObjectId, name: &str, x: i32, karma: i32) -> PlayerData {
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
        karma,
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
        items: vec![],
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
        let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
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

    /// Alice (1) and Bob (2) stand `gap` apart; what the enter sent is dropped.
    fn pair(gap: i32) -> Self {
        let mut s = Self::new();
        s.enter(player(A, "Alice", 1000, 0));
        s.enter(player(B, "Bob", 1000 + gap, 0));
        s.log.take();
        s
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn action(&mut self, who: ObjectId, action_id: i32) {
        self.input(
            who,
            ClientInput::ActionUse {
                action_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        );
    }

    fn answer(&mut self, who: ObjectId, action_id: i32, answer: i32, asker: ObjectId) {
        self.input(
            who,
            ClientInput::AnswerCoupleAction {
                action_id,
                answer,
                object_id: asker.0,
            },
        );
    }

    fn target(&mut self, who: ObjectId, target: ObjectId) {
        self.world.player_mut(who).unwrap().target = Some(target);
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
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

fn of<'a>(out: &'a [Out], to: ObjectId, name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter(|o| o.0 == to.0 && name_of(&o.1) == name)
        .map(|o| o.1.as_slice())
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

fn wait_types(out: &[Out], to: ObjectId) -> Vec<i32> {
    of(out, to, "CHANGE_WAIT_TYPE")
        .iter()
        .map(|raw| decode::<ChangeWaitType>(raw).move_type)
        .collect()
}

fn socials(out: &[Out], to: ObjectId) -> Vec<(i32, i32)> {
    of(out, to, "SOCIAL_ACTION")
        .iter()
        .map(|raw| {
            let p = decode::<SocialAction>(raw);
            (p.object_id, p.action_id)
        })
        .collect()
}

// ---- sit and stand ------------------------------------------------------------------------------

/// `Player.sitDown` / `standUp`: `ChangeWaitType` to the player and to the others, and no second
/// change while the 2.5 s animation runs (`_sittingInProgress`).
#[test]
fn action_0_sits_down_and_stands_up() {
    let mut s = Scene::pair(50);
    s.action(A, 0);
    let out = s.log.take();
    assert_eq!(wait_types(&out, A), [ChangeWaitType::WT_SITTING]);
    assert_eq!(wait_types(&out, B), [ChangeWaitType::WT_SITTING]);
    assert!(s.world.is_sitting(A));

    // The animation is not over: standing up is refused (and says nothing).
    s.action(A, 0);
    assert!(wait_types(&s.log.take(), A).is_empty());
    assert!(s.world.is_sitting(A));

    s.advance(2600);
    s.log.take();
    s.action(A, 0);
    let out = s.log.take();
    assert_eq!(wait_types(&out, A), [ChangeWaitType::WT_STANDING]);
    assert_eq!(wait_types(&out, B), [ChangeWaitType::WT_STANDING]);
    s.advance(2600);
    assert!(!s.world.is_sitting(A));
}

/// `RequestActionUse` case 0: a player who walks (`isMoving()`) sits when it arrives, with the
/// target it had when it asked (`NextAction(ARRIVED, MOVE_TO, ...)`).
#[test]
fn action_0_while_walking_sits_on_arrival() {
    let mut s = Scene::pair(500);
    s.input(
        A,
        ClientInput::MoveTo {
            target_x: 1200,
            target_y: 1000,
            target_z: 0,
            origin_x: 1000,
            origin_y: 1000,
            origin_z: 0,
            movement_mode: 1,
        },
    );
    assert!(s.world.is_moving(A));
    s.log.take();
    s.action(A, 0);
    assert!(!s.world.is_sitting(A), "it sits when it arrives");
    assert!(wait_types(&s.log.take(), A).is_empty());

    for _ in 0..200 {
        s.world.tick();
        if s.world.is_sitting(A) {
            break;
        }
    }
    assert!(s.world.is_sitting(A));
    assert!(!s.world.is_moving(A));
    let out = s.log.take();
    assert_eq!(wait_types(&out, A), [ChangeWaitType::WT_SITTING]);
    assert_eq!(wait_types(&out, B), [ChangeWaitType::WT_SITTING]);
    let sat = decode::<ChangeWaitType>(of(&out, A, "CHANGE_WAIT_TYPE")[0]);
    assert_eq!((sat.x, sat.y), (1200, 1000));
}

/// `useSit` on a throne (`StaticObject` type 1) closer than 150: `ChairSit` first to the player
/// (`sendPacket`), then to everybody including the player (`broadcastPacket`).
#[test]
fn action_0_on_a_throne_sends_chair_sit() {
    let mut s = Scene::pair(50);
    let templates = StaticObjectTemplates::from_xml(
        r#"<list><object id="19210010" name="throne" type="1" x="1100" y="1000" z="0"/>
        <object id="17250001" name="map" type="0" x="1000" y="1050" z="0"/></list>"#,
    )
    .unwrap();
    let ids = IdFactory::new(Vec::<i32>::new()).unwrap();
    s.world.spawn_static_objects(&templates, &ids).unwrap();
    let throne = s
        .world
        .static_objects()
        .find(|o| o.static_object_id == 19_210_010)
        .unwrap()
        .object_id;
    let sign = s
        .world
        .static_objects()
        .find(|o| o.static_object_id == 17_250_001)
        .unwrap()
        .object_id;
    s.log.take();

    // A map sign is no chair: a plain sit.
    s.target(A, sign);
    s.action(A, 0);
    let out = s.log.take();
    assert!(of(&out, A, "CHAIR_SIT").is_empty());
    assert_eq!(wait_types(&out, A), [ChangeWaitType::WT_SITTING]);
    s.advance(2600);
    s.action(A, 0);
    s.advance(2600);
    assert!(!s.world.is_sitting(A));
    s.log.take();

    s.target(A, throne);
    s.action(A, 0);
    let out = s.log.take();
    let chair: Vec<ChairSit> = of(&out, A, "CHAIR_SIT").iter().map(|r| decode(r)).collect();
    assert_eq!(chair.len(), 2, "NOTE(java-quirk): the player gets it twice");
    assert_eq!(chair[0].object_id, A.0);
    assert_eq!(chair[0].static_object_id, 19_210_010);
    assert_eq!(of(&out, B, "CHAIR_SIT").len(), 1);
    assert_eq!(wait_types(&out, A), [ChangeWaitType::WT_SITTING]);
    assert!(s.world.is_sitting(A));
}

// ---- walk and run -------------------------------------------------------------------------------

/// `Creature.setRunning` / `setWalking`: `ChangeMoveType` (to the player too) and the new
/// `UserInfo`, each time the mode changes.
#[test]
fn action_1_toggles_walk_and_run() {
    let mut s = Scene::pair(50);
    assert!(s.world.is_running(A));
    s.action(A, 1);
    let out = s.log.take();
    assert!(!s.world.is_running(A));
    let walk: Vec<ChangeMoveType> = of(&out, A, "CHANGE_MOVE_TYPE")
        .iter()
        .map(|r| decode(r))
        .collect();
    assert_eq!(
        walk,
        [ChangeMoveType {
            object_id: A.0,
            running: false
        }]
    );
    assert_eq!(of(&out, B, "CHANGE_MOVE_TYPE").len(), 1);
    assert_eq!(of(&out, A, "USER_INFO").len(), 1);

    s.action(A, 1);
    let out = s.log.take();
    assert!(s.world.is_running(A));
    let run: ChangeMoveType = decode(of(&out, B, "CHANGE_MOVE_TYPE")[0]);
    assert!(run.running);
}

// ---- socials ------------------------------------------------------------------------------------

/// The `tryBroadcastSocial` ids of the action list.
#[test]
fn the_social_actions_broadcast_their_social_id() {
    let cases = [
        (12, 2),
        (13, 3),
        (14, 4),
        (24, 6),
        (25, 5),
        (26, 7),
        (29, 8),
        (30, 9),
        (31, 10),
        (33, 11),
        (34, 12),
        (35, 13),
        (62, 14),
        (66, 15),
    ];
    let mut s = Scene::pair(50);
    for (action_id, social_id) in cases {
        s.action(A, action_id);
        let out = s.log.take();
        assert_eq!(socials(&out, A), [(A.0, social_id)], "action {action_id}");
        assert_eq!(socials(&out, B), [(A.0, social_id)], "action {action_id}");
    }
}

/// `Player.canMakeSocialAction`: no emote while sitting (the intention is REST), and none with a
/// trade request open (`getActiveRequester()`).
#[test]
fn no_social_while_sitting_or_busy() {
    let mut s = Scene::pair(50);
    s.action(A, 0);
    s.log.take();
    s.action(A, 12);
    assert!(socials(&s.log.take(), A).is_empty());
    s.advance(2600);
    s.action(A, 0);
    s.advance(2600);
    s.log.take();
    s.action(A, 12);
    assert_eq!(socials(&s.log.take(), A), [(A.0, 2)]);

    // Alice asked Bob to trade: Bob has her as requester, and Alice has Bob.
    s.input(A, ClientInput::TradeRequest { object_id: B.0 });
    s.log.take();
    s.action(B, 12);
    assert!(socials(&s.log.take(), B).is_empty());
}

// ---- couple actions -----------------------------------------------------------------------------

fn name_param(message: &SystemMessage) -> Option<&str> {
    match message.params.first() {
        Some(SystemMessageParam::PlayerName(name)) => Some(name),
        _ => None,
    }
}

/// `useCoupleSocial` to the end of `AnswerCoupleAction`: the request, the question, the approval.
#[test]
fn a_couple_action_is_asked_and_approved() {
    let mut s = Scene::pair(50);
    s.target(A, B);
    s.action(A, 71);
    let out = s.log.take();
    let mine = messages(&out, A);
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].id, SystemMessageId::YouHaveRequestedACoupleActionWithC1);
    assert_eq!(name_param(&mine[0]), Some("Bob"));
    // Bob is asked with the social id (71 - 55).
    let ask: ExAskCoupleAction = decode(of(&out, B, "EX_ASK_COUPLE_ACTION")[0]);
    assert_eq!((ask.object_id, ask.action_id), (A.0, 16));

    // A wrong action id or a stranger is ignored.
    s.answer(B, 17, 1, A);
    assert!(s.log.take().is_empty());

    s.answer(B, 16, 1, A);
    let out = s.log.take();
    // Java's `player` is the one who answers (Bob), `target` the one who asked (Alice): Bob turns
    // first, towards Alice (west, (int)(180 * 182.044444444) = 32767), then Alice towards Bob
    // (east, 0).
    let alice_turn: Vec<ExRotation> = of(&out, A, "EX_ROTATION").iter().map(|r| decode(r)).collect();
    let bob_turn: Vec<ExRotation> = of(&out, B, "EX_ROTATION").iter().map(|r| decode(r)).collect();
    // Each of them hears both rotations (they see each other) and both social actions.
    assert_eq!(
        alice_turn,
        [
            ExRotation {
                char_id: B.0,
                heading: 32_767
            },
            ExRotation {
                char_id: A.0,
                heading: 0
            },
        ]
    );
    assert_eq!(alice_turn, bob_turn);
    assert_eq!(socials(&out, A), [(B.0, 16), (A.0, 16)]);
    assert_eq!(socials(&out, B), [(B.0, 16), (A.0, 16)]);
    assert_eq!(s.world.player(A).unwrap().core.heading, 0);
    assert_eq!(s.world.player(B).unwrap().core.heading, 32_767);

    // The request is spent: a second approval does nothing.
    s.answer(B, 16, 1, A);
    assert!(s.log.take().is_empty());
}

/// `AnswerCoupleAction`: 0 denies, -1 says the answering player refuses couple actions; both
/// clear the request.
#[test]
fn a_couple_action_can_be_denied_or_refused() {
    let mut s = Scene::pair(50);
    s.target(A, B);
    s.action(A, 72);
    s.log.take();
    s.answer(B, 17, 0, A);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::TheCoupleActionWasDenied]);

    s.action(A, 73);
    let out = s.log.take();
    let ask: ExAskCoupleAction = decode(of(&out, B, "EX_ASK_COUPLE_ACTION")[0]);
    assert_eq!((ask.object_id, ask.action_id), (A.0, 18));
    s.answer(B, 18, -1, A);
    let out = s.log.take();
    let refused = messages(&out, A);
    assert_eq!(
        refused[0].id,
        SystemMessageId::C1IsSetToRefuseCoupleActionsAndCannotBeRequestedForACoupleAction
    );
    assert_eq!(name_param(&refused[0]), Some("Bob"));
}

/// The refusals of `useCoupleSocial`, in Java's order, with the name of whoever is the reason.
#[test]
fn a_couple_action_is_refused_like_java() {
    use SystemMessageId as M;
    // No player targeted.
    let mut s = Scene::pair(50);
    s.action(A, 71);
    assert_eq!(message_ids(&s.log.take(), A), [M::InvalidTarget]);

    // Too far (`distance > 125`) and too close (`distance < 15`).
    let mut far = Scene::pair(126);
    far.target(A, B);
    far.action(A, 71);
    assert_eq!(
        message_ids(&far.log.take(), A),
        [M::TheRequestCannotBeCompletedBecauseTheTargetDoesNotMeetLocationRequirements]
    );
    let mut edge = Scene::pair(125);
    edge.target(A, B);
    edge.action(A, 71);
    assert_eq!(
        message_ids(&edge.log.take(), A),
        [M::YouHaveRequestedACoupleActionWithC1]
    );
    let mut near = Scene::pair(14);
    near.target(A, B);
    near.action(A, 71);
    assert_eq!(
        message_ids(&near.log.take(), A),
        [M::TheRequestCannotBeCompletedBecauseTheTargetDoesNotMeetLocationRequirements]
    );

    // A chaotic player (`getKarma() > 0`) names itself; a chaotic partner names the partner.
    let mut chaotic = Scene::new();
    chaotic.enter(player(A, "Alice", 1000, 100));
    chaotic.enter(player(B, "Bob", 1050, 0));
    chaotic.log.take();
    chaotic.target(A, B);
    chaotic.action(A, 71);
    let out = chaotic.log.take();
    let sent = messages(&out, A);
    assert_eq!(
        sent[0].id,
        M::C1IsInAChaoticStateAndCannotBeRequestedForACoupleAction
    );
    assert_eq!(name_param(&sent[0]), Some("Alice"));

    let mut partner = Scene::new();
    partner.enter(player(A, "Alice", 1000, 0));
    partner.enter(player(B, "Bob", 1050, 100));
    partner.log.take();
    partner.target(A, B);
    partner.action(A, 71);
    let out = partner.log.take();
    let sent = messages(&out, A);
    assert_eq!(
        sent[0].id,
        M::C1IsInAChaoticStateAndCannotBeRequestedForACoupleAction
    );
    assert_eq!(name_param(&sent[0]), Some("Bob"));
    assert!(of(&out, B, "EX_ASK_COUPLE_ACTION").is_empty());

    // The partner has a couple action of its own going (`getMultiSociaAction() > 0`, set on the
    // player who asked): Bob asked Alice, so Carol asking Bob gets the refusal.
    let mut busy = Scene::pair(50);
    busy.enter(player(ObjectId(3), "Carol", 1020, 0));
    busy.log.take();
    busy.target(B, A);
    busy.action(B, 71);
    busy.log.take();
    busy.target(ObjectId(3), B);
    busy.action(ObjectId(3), 71);
    let out = busy.log.take();
    let sent = messages(&out, ObjectId(3));
    assert_eq!(
        sent[0].id,
        M::C1IsAlreadyParticipatingInACoupleActionAndCannotBeRequestedForAnotherCoupleAction
    );
    assert_eq!(name_param(&sent[0]), Some("Bob"));

    // A sitting player (REST intention) can still ask: only the intention decides when the
    // question is delayed (see the next test).
}

/// `AnswerCoupleAction`: too far or too close when the answer comes keeps the request
/// (`return` before `setMultiSocialAction(0, 0)`) and tells both players.
#[test]
fn an_approval_from_the_wrong_distance_is_refused_for_both() {
    let mut s = Scene::pair(100);
    s.target(A, B);
    s.action(A, 71);
    s.log.take();
    // Bob walks away beyond 125.
    s.world.player_mut(B).unwrap().core.x = 1200;
    s.answer(B, 16, 1, A);
    let out = s.log.take();
    let id = SystemMessageId::TheRequestCannotBeCompletedBecauseTheTargetDoesNotMeetLocationRequirements;
    assert_eq!(message_ids(&out, A), [id]);
    assert_eq!(message_ids(&out, B), [id]);
    assert!(of(&out, A, "EX_ROTATION").is_empty());

    // NOTE(java-quirk): the request was not cleared, so Bob can answer again once he is back.
    s.world.player_mut(B).unwrap().core.x = 1100;
    s.answer(B, 16, 1, A);
    assert_eq!(socials(&s.log.take(), A), [(B.0, 16), (A.0, 16)]);
}

/// `useCoupleSocial` with a walking player: the question waits for `ARRIVED`
/// (`NextAction(ARRIVED, MOVE_TO, ...)`), the request message does not.
#[test]
fn a_couple_question_waits_for_a_walking_player_to_arrive() {
    let mut s = Scene::pair(50);
    s.target(A, B);
    s.input(
        A,
        ClientInput::MoveTo {
            target_x: 1000,
            target_y: 1100,
            target_z: 0,
            origin_x: 1000,
            origin_y: 1000,
            origin_z: 0,
            movement_mode: 1,
        },
    );
    s.log.take();
    s.action(A, 71);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveRequestedACoupleActionWithC1]
    );
    assert!(of(&out, B, "EX_ASK_COUPLE_ACTION").is_empty());

    // 100 units at run speed 120 take under a second; the partner is asked on arrival. Alice
    // ends 100 away from Bob: inside the 125 of the question (the distance is not checked again).
    s.advance(1500);
    assert!(!s.world.is_moving(A));
    let out = s.log.take();
    let ask: ExAskCoupleAction = decode(of(&out, B, "EX_ASK_COUPLE_ACTION")[0]);
    assert_eq!((ask.object_id, ask.action_id), (A.0, 16));

    // The action ran once: arriving again does not ask again.
    s.input(
        A,
        ClientInput::MoveTo {
            target_x: 1000,
            target_y: 1000,
            target_z: 0,
            origin_x: 1000,
            origin_y: 1100,
            origin_z: 0,
            movement_mode: 1,
        },
    );
    s.advance(1500);
    assert!(of(&s.log.take(), B, "EX_ASK_COUPLE_ACTION").is_empty());
}
