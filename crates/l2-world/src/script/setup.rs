//! Registration: the listeners a script asks for in its constructor, and the tables the world
//! dispatches events through.
// Java: mechanics/script/Quest.java (addStartNpc, addTalkId, addKillId, ..., registerQuestItems,
//       setOnEnterWorld, addCondStart, registerListener)
// Java: mechanics/events/{EventType,ListenerRegisterType,ListenersContainer,EventDispatcher}.java
//
// Java keeps the listeners on the NPC template, item template, zone or global container, each a
// `PriorityBlockingQueue` ordered by priority. Every listener a `Quest` registers has priority
// 0, and a priority queue keeps equal elements in insertion order, so Java's dispatch order is
// the registration order. Here the tables map `(event, id)` to the scripts in registration
// order; scripts register in the order of [`super::ScriptRegistry`].

use std::collections::HashMap;

use l2_util::ObjectId;

/// The index of a registered script in the world's script table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScriptId(pub(crate) u32);

/// The events scripts can listen to (Java `EventType`, the part `Quest` registers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// `ON_NPC_QUEST_START` (`addStartNpc`).
    NpcQuestStart,
    /// `ON_NPC_TALK` (`addTalkId`).
    NpcTalk,
    /// `ON_NPC_FIRST_TALK` (`addFirstTalkId`).
    NpcFirstTalk,
    /// `ON_ATTACKABLE_KILL` (`addKillId`).
    AttackableKill,
    /// `ON_ATTACKABLE_ATTACK` (`addAttackId`).
    AttackableAttack,
    /// `ON_NPC_SPAWN` (`addSpawnId`).
    NpcSpawn,
    /// `ON_NPC_TELEPORT` (`addTeleportId`).
    NpcTeleport,
    /// `ON_NPC_SKILL_SEE` (`addSkillSeeId`).
    NpcSkillSee,
    /// `ON_NPC_SKILL_FINISHED` (`addSpellFinishedId`).
    NpcSkillFinished,
    /// `ON_ATTACKABLE_FACTION_CALL` (`addFactionCallId`).
    AttackableFactionCall,
    /// `ON_ATTACKABLE_AGGRO_RANGE_ENTER` (`addAggroRangeEnterId`).
    AttackableAggroRangeEnter,
    /// `ON_CREATURE_SEE` (`addCreatureSeeId`).
    CreatureSee,
    /// `ON_NPC_EVENT_RECEIVED` (`addEventReceivedId`).
    NpcEventReceived,
    /// `ON_NPC_MOVE_FINISHED` (`addMoveFinishedId`).
    NpcMoveFinished,
    /// `ON_NPC_MOVE_NODE_ARRIVED` (`addNodeArrivedId`).
    NpcMoveNodeArrived,
    /// `ON_NPC_MOVE_ROUTE_FINISHED` (`addRouteFinishedId`).
    NpcMoveRouteFinished,
    /// `ON_NPC_HATE` (`addNpcHateId`).
    NpcHate,
    /// `ON_NPC_HATE` through `addCanSeeMeId`.
    ///
    /// NOTE(java-quirk): Java's `addCanSeeMeId` registers an `ON_NPC_HATE` listener that answers
    /// with `!onCanSeeMe(npc, player)`, not an `ON_NPC_CAN_BE_SEEN` one. It shares the
    /// `NpcHate` table (and its order); this kind only marks which hook answers.
    NpcHateCanSeeMe,
    /// `ON_ITEM_TALK` (`addItemTalkId`), by item id.
    ItemTalk,
    /// `ON_ITEM_BYPASS_EVENT` (`addItemBypassEventId`), by item id.
    ItemBypassEvent,
    /// `ON_CREATURE_ZONE_ENTER` (`addEnterZoneId`), by zone id.
    CreatureZoneEnter,
    /// `ON_CREATURE_ZONE_EXIT` (`addExitZoneId`), by zone id.
    CreatureZoneExit,
    /// `ON_PLAYER_LOGIN` (`setOnEnterWorld(true)`), global.
    PlayerLogin,
    /// `ON_PLAYER_BYPASS` of `ListenerRegisterType.GLOBAL_PLAYERS` (S-API-03), global.
    PlayerBypass,
    /// `ON_PLAYER_PRESS_TUTORIAL_MARK` of `GLOBAL_PLAYERS` (S-API-03), global.
    PlayerPressTutorialMark,
    /// `ON_PLAYER_LOGOUT` of `GLOBAL_PLAYERS` (S-API-03), global.
    PlayerLogout,
    /// `ON_DAY_NIGHT_CHANGE` (S-API-03), global.
    DayNightChange,
    /// `ON_CREATURE_DEATH` of `ListenerRegisterType.NPC` (`@Id`, S-API-03), by NPC id.
    CreatureDeath,
    /// `ON_CREATURE_ATTACK` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureAttack,
    /// `ON_CREATURE_ATTACK_AVOID` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureAttackAvoid,
    /// `ON_CREATURE_ATTACKED` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureAttacked,
    /// `ON_CREATURE_DAMAGE_RECEIVED` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureDamageReceived,
    /// `ON_CREATURE_DAMAGE_DEALT` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureDamageDealt,
    /// `ON_CREATURE_KILLED` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureKilled,
    /// `ON_CREATURE_SKILL_USE` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CreatureSkillUse,
    /// `ON_PLAYER_SIT` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerSit,
    /// `ON_PLAYER_LEVEL_CHANGED` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerLevelChanged,
    /// `ON_PLAYER_ITEM_ADD` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerItemAdd,
    /// `ON_PLAYER_ITEM_PICKUP` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerItemPickup,
    /// `ON_PLAYER_PROFESSION_CHANGE` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerProfessionChange,
    /// `ON_PLAYER_SUMMON_TALK` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayerSummonTalk,
    /// `ON_PLAYABLE_EXP_CHANGED` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    PlayableExpChanged,
    /// `ON_NPC_MANOR_BYPASS` (`add_npc_manor_bypass_id`, S-API-04), by NPC id.
    NpcManorBypass,
    /// `ON_TRAP_ACTION` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    TrapAction,
    /// `ON_DAILY_RESET` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    DailyReset,
    /// `ON_CLAN_WAR_FINISH` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    ClanWarFinish,
    /// `ON_CLAN_WAR_START` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    ClanWarStart,
    /// `ON_CASTLE_SIEGE_FINISH` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CastleSiegeFinish,
    /// `ON_CASTLE_SIEGE_OWNER_CHANGE` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CastleSiegeOwnerChange,
    /// `ON_CASTLE_SIEGE_START` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    CastleSiegeStart,
    /// `ON_FORT_SIEGE_FINISH` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    FortSiegeFinish,
    /// `ON_FORT_SIEGE_START` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    FortSiegeStart,
    /// `ON_OLYMPIAD_MATCH_RESULT` (S-API-04). Global, or on one object with `ScriptCtx::add_listener`.
    OlympiadMatchResult,
    /// `ON_PLAYER_SUMMON_SPAWN` (`addSummonSpawnId`, S-API-05), by the summon's NPC id.
    PlayerSummonSpawn,
}

impl EventKind {
    /// Java `ListenerRegisterType` of the event: what its ids are.
    pub fn register_type(self) -> RegisterType {
        match self {
            Self::ItemTalk | Self::ItemBypassEvent => RegisterType::Item,
            Self::CreatureZoneEnter | Self::CreatureZoneExit => RegisterType::Zone,
            Self::PlayerLogin
            | Self::PlayerBypass
            | Self::PlayerPressTutorialMark
            | Self::PlayerLogout
            | Self::DayNightChange => RegisterType::Global,
            Self::NpcManorBypass => RegisterType::Npc,
            Self::CreatureAttack
            | Self::CreatureAttackAvoid
            | Self::CreatureAttacked
            | Self::CreatureDamageReceived
            | Self::CreatureDamageDealt
            | Self::CreatureKilled
            | Self::CreatureSkillUse
            | Self::PlayerSit
            | Self::PlayerLevelChanged
            | Self::PlayerItemAdd
            | Self::PlayerItemPickup
            | Self::PlayerProfessionChange
            | Self::PlayerSummonTalk
            | Self::PlayableExpChanged
            | Self::TrapAction
            | Self::DailyReset
            | Self::ClanWarFinish
            | Self::ClanWarStart
            | Self::CastleSiegeFinish
            | Self::CastleSiegeOwnerChange
            | Self::CastleSiegeStart
            | Self::FortSiegeFinish
            | Self::FortSiegeStart
            | Self::OlympiadMatchResult => RegisterType::Global,
            _ => RegisterType::Npc,
        }
    }

    /// Java `EventType` of the listener the kind registers, with its position in the enum
    /// (its line in `EventType.java`, which keeps the order): `//quest_info` prints it.
    pub(crate) fn java_event_type(self) -> (u32, &'static str) {
        match self {
            Self::AttackableAggroRangeEnter => (121, "ON_ATTACKABLE_AGGRO_RANGE_ENTER"),
            Self::AttackableAttack => (122, "ON_ATTACKABLE_ATTACK"),
            Self::AttackableFactionCall => (123, "ON_ATTACKABLE_FACTION_CALL"),
            Self::AttackableKill => (124, "ON_ATTACKABLE_KILL"),
            Self::CreatureDeath => (141, "ON_CREATURE_DEATH"),
            Self::CreatureSee => (143, "ON_CREATURE_SEE"),
            Self::CreatureZoneEnter => (146, "ON_CREATURE_ZONE_ENTER"),
            Self::CreatureZoneExit => (147, "ON_CREATURE_ZONE_EXIT"),
            Self::ItemBypassEvent => (154, "ON_ITEM_BYPASS_EVENT"),
            Self::ItemTalk => (156, "ON_ITEM_TALK"),
            Self::NpcEventReceived => (160, "ON_NPC_EVENT_RECEIVED"),
            Self::NpcFirstTalk => (161, "ON_NPC_FIRST_TALK"),
            Self::NpcHate | Self::NpcHateCanSeeMe => (162, "ON_NPC_HATE"),
            Self::NpcMoveFinished => (163, "ON_NPC_MOVE_FINISHED"),
            Self::NpcMoveNodeArrived => (164, "ON_NPC_MOVE_NODE_ARRIVED"),
            Self::NpcMoveRouteFinished => (165, "ON_NPC_MOVE_ROUTE_FINISHED"),
            Self::NpcQuestStart => (166, "ON_NPC_QUEST_START"),
            Self::NpcSkillFinished => (167, "ON_NPC_SKILL_FINISHED"),
            Self::NpcSkillSee => (168, "ON_NPC_SKILL_SEE"),
            Self::NpcSpawn => (169, "ON_NPC_SPAWN"),
            Self::NpcTalk => (170, "ON_NPC_TALK"),
            Self::NpcTeleport => (171, "ON_NPC_TELEPORT"),
            Self::PlayerBypass => (182, "ON_PLAYER_BYPASS"),
            Self::PlayerLogin => (218, "ON_PLAYER_LOGIN"),
            Self::PlayerLogout => (219, "ON_PLAYER_LOGOUT"),
            Self::PlayerPressTutorialMark => (221, "ON_PLAYER_PRESS_TUTORIAL_MARK"),
            Self::DayNightChange => (240, "ON_DAY_NIGHT_CHANGE"),
            Self::CreatureAttack => (136, "ON_CREATURE_ATTACK"),
            Self::CreatureAttackAvoid => (137, "ON_CREATURE_ATTACK_AVOID"),
            Self::CreatureAttacked => (138, "ON_CREATURE_ATTACKED"),
            Self::CreatureDamageReceived => (139, "ON_CREATURE_DAMAGE_RECEIVED"),
            Self::CreatureDamageDealt => (140, "ON_CREATURE_DAMAGE_DEALT"),
            Self::CreatureKilled => (142, "ON_CREATURE_KILLED"),
            Self::CreatureSkillUse => (144, "ON_CREATURE_SKILL_USE"),
            Self::PlayerSit => (229, "ON_PLAYER_SIT"),
            Self::PlayerLevelChanged => (217, "ON_PLAYER_LEVEL_CHANGED"),
            Self::PlayerItemAdd => (207, "ON_PLAYER_ITEM_ADD"),
            Self::PlayerItemPickup => (210, "ON_PLAYER_ITEM_PICKUP"),
            Self::PlayerProfessionChange => (223, "ON_PLAYER_PROFESSION_CHANGE"),
            Self::PlayerSummonTalk => (232, "ON_PLAYER_SUMMON_TALK"),
            Self::PlayableExpChanged => (178, "ON_PLAYABLE_EXP_CHANGED"),
            Self::NpcManorBypass => (172, "ON_NPC_MANOR_BYPASS"),
            Self::TrapAction => (236, "ON_TRAP_ACTION"),
            Self::DailyReset => (241, "ON_DAILY_RESET"),
            Self::ClanWarFinish => (132, "ON_CLAN_WAR_FINISH"),
            Self::ClanWarStart => (133, "ON_CLAN_WAR_START"),
            Self::CastleSiegeFinish => (127, "ON_CASTLE_SIEGE_FINISH"),
            Self::CastleSiegeOwnerChange => (128, "ON_CASTLE_SIEGE_OWNER_CHANGE"),
            Self::CastleSiegeStart => (129, "ON_CASTLE_SIEGE_START"),
            Self::FortSiegeFinish => (150, "ON_FORT_SIEGE_FINISH"),
            Self::FortSiegeStart => (151, "ON_FORT_SIEGE_START"),
            Self::OlympiadMatchResult => (175, "ON_OLYMPIAD_MATCH_RESULT"),
            Self::PlayerSummonSpawn => (231, "ON_PLAYER_SUMMON_SPAWN"),
        }
    }

    /// The Java method that registers it, for the "non existing NPC" log line.
    fn java_method(self) -> &'static str {
        match self {
            Self::NpcQuestStart => "addStartNpc",
            Self::NpcTalk => "addTalkId",
            Self::NpcFirstTalk => "addFirstTalkId",
            Self::AttackableKill => "addKillId",
            Self::AttackableAttack => "addAttackId",
            Self::NpcSpawn => "addSpawnId",
            Self::NpcSkillSee => "addSkillSeeId",
            Self::NpcSkillFinished => "addSpellFinishedId",
            Self::AttackableFactionCall => "addFactionCallId",
            Self::AttackableAggroRangeEnter => "addAggroRangeEnterId",
            Self::CreatureSee => "addCreatureSeeId",
<<<<<<< HEAD
            Self::TrapAction => "addTrapActionId",
=======
            Self::PlayerSummonSpawn => "addSummonSpawnId",
>>>>>>> main
            _ => "",
        }
    }
}

/// Java `ListenerRegisterType`, the part scripts use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterType {
    Npc,
    Item,
    Zone,
    Global,
}

/// A start condition of `addCondStart(predicate, html)`: the predicate gets the world and the
/// player.
pub type StartCondition = fn(&crate::World, ObjectId) -> bool;

/// What a script's [`super::Script::setup`] fills in: Java's `add*Id` calls.
#[derive(Debug, Default)]
pub struct ScriptSetup {
    pub(crate) listeners: Vec<(EventKind, Vec<i32>)>,
    pub(crate) quest_items: Vec<i32>,
    pub(crate) start_conditions: Vec<(StartCondition, String)>,
}

impl ScriptSetup {
    fn add(&mut self, kind: EventKind, ids: &[i32]) {
        self.listeners.push((kind, ids.to_vec()));
    }

    /// Java `addStartNpc(npcIds)`: the NPCs that offer the quest (the quest list).
    pub fn add_start_npc(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcQuestStart, npc_ids);
    }

    /// Java `addTalkId(npcIds)`: the NPCs whose quest list leads to `on_talk`.
    pub fn add_talk_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcTalk, npc_ids);
    }

    /// Java `addFirstTalkId(npcIds)`.
    pub fn add_first_talk_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcFirstTalk, npc_ids);
    }

    /// Java `addKillId(npcIds)`.
    pub fn add_kill_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::AttackableKill, npc_ids);
    }

    /// Java `addAttackId(npcIds)`.
    pub fn add_attack_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::AttackableAttack, npc_ids);
    }

    /// Java `addSpawnId(npcIds)`.
    pub fn add_spawn_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcSpawn, npc_ids);
    }

    /// Java `addSummonSpawnId(npcIds)`: `on_summon_spawn` when a summon (servitor or pet) of one
    /// of the NPC ids spawns (S-API-05).
    pub fn add_summon_spawn_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::PlayerSummonSpawn, npc_ids);
    }

    /// Java `addTeleportId(npcIds)`.
    pub fn add_teleport_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcTeleport, npc_ids);
    }

    /// Java `addSkillSeeId(npcIds)`.
    pub fn add_skill_see_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcSkillSee, npc_ids);
    }

    /// Java `addSpellFinishedId(npcIds)`.
    pub fn add_spell_finished_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcSkillFinished, npc_ids);
    }

    /// Java `addFactionCallId(npcIds)`.
    pub fn add_faction_call_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::AttackableFactionCall, npc_ids);
    }

    /// Java `addAggroRangeEnterId(npcIds)`.
    pub fn add_aggro_range_enter_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::AttackableAggroRangeEnter, npc_ids);
    }

    /// Java `addCreatureSeeId(npcIds)`.
    pub fn add_creature_see_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::CreatureSee, npc_ids);
    }

    /// Java `addEventReceivedId(npcIds)`.
    pub fn add_event_received_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcEventReceived, npc_ids);
    }

    /// Java `addMoveFinishedId(npcIds)`.
    pub fn add_move_finished_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcMoveFinished, npc_ids);
    }

    /// Java `addNodeArrivedId(npcIds)`.
    pub fn add_node_arrived_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcMoveNodeArrived, npc_ids);
    }

    /// Java `addRouteFinishedId(npcIds)`.
    pub fn add_route_finished_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcMoveRouteFinished, npc_ids);
    }

    /// Java `addNpcHateId(npcIds)`.
    pub fn add_npc_hate_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcHate, npc_ids);
    }

    /// Java `addCanSeeMeId(npcIds)`: an `ON_NPC_HATE` listener answering with `on_can_see_me`.
    pub fn add_can_see_me_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcHateCanSeeMe, npc_ids);
    }

    /// Java `addItemTalkId(itemIds)`.
    pub fn add_item_talk_id(&mut self, item_ids: &[i32]) {
        self.add(EventKind::ItemTalk, item_ids);
    }

    /// Java `addItemBypassEventId(itemIds)`.
    pub fn add_item_bypass_event_id(&mut self, item_ids: &[i32]) {
        self.add(EventKind::ItemBypassEvent, item_ids);
    }

    /// Java `addEnterZoneId(zoneIds)`.
    pub fn add_enter_zone_id(&mut self, zone_ids: &[i32]) {
        self.add(EventKind::CreatureZoneEnter, zone_ids);
    }

    /// Java `addExitZoneId(zoneIds)`.
    pub fn add_exit_zone_id(&mut self, zone_ids: &[i32]) {
        self.add(EventKind::CreatureZoneExit, zone_ids);
    }

    /// Java `setOnEnterWorld(true)`: `on_enter_world` for every player who logs in.
    pub fn set_on_enter_world(&mut self) {
        self.add(EventKind::PlayerLogin, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_BYPASS)` + `@RegisterType(GLOBAL_PLAYERS)`: `on_player_bypass`
    /// for the bypass of every player.
    pub fn set_on_player_bypass(&mut self) {
        self.add(EventKind::PlayerBypass, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_PRESS_TUTORIAL_MARK)` + `GLOBAL_PLAYERS`.
    pub fn set_on_player_press_tutorial_mark(&mut self) {
        self.add(EventKind::PlayerPressTutorialMark, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_LOGOUT)` + `GLOBAL_PLAYERS`.
    pub fn set_on_player_logout(&mut self) {
        self.add(EventKind::PlayerLogout, &[]);
    }

    /// `@RegisterEvent(ON_DAY_NIGHT_CHANGE)` (a global listener).
    pub fn set_on_day_night_change(&mut self) {
        self.add(EventKind::DayNightChange, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_DEATH)` + `@RegisterType(NPC)` + `@Id(..)`.
    pub fn add_creature_death_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::CreatureDeath, npc_ids);
    }

    /// `@RegisterEvent(ON_CREATURE_ATTACK)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_attack(&mut self) {
        self.add(EventKind::CreatureAttack, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_ATTACKED)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_attacked(&mut self) {
        self.add(EventKind::CreatureAttacked, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_ATTACK_AVOID)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_attack_avoid(&mut self) {
        self.add(EventKind::CreatureAttackAvoid, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_SKILL_USE)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_skill_use(&mut self) {
        self.add(EventKind::CreatureSkillUse, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_DAMAGE_DEALT)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_damage_dealt(&mut self) {
        self.add(EventKind::CreatureDamageDealt, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_DAMAGE_RECEIVED)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_damage_received(&mut self) {
        self.add(EventKind::CreatureDamageReceived, &[]);
    }

    /// `@RegisterEvent(ON_CREATURE_KILLED)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_creature_killed(&mut self) {
        self.add(EventKind::CreatureKilled, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_SIT)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_sit(&mut self) {
        self.add(EventKind::PlayerSit, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_LEVEL_CHANGED)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_level_changed(&mut self) {
        self.add(EventKind::PlayerLevelChanged, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_ITEM_ADD)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_item_add(&mut self) {
        self.add(EventKind::PlayerItemAdd, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_ITEM_PICKUP)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_item_pickup(&mut self) {
        self.add(EventKind::PlayerItemPickup, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_PROFESSION_CHANGE)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_profession_change(&mut self) {
        self.add(EventKind::PlayerProfessionChange, &[]);
    }

    /// `@RegisterEvent(ON_PLAYER_SUMMON_TALK)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_player_summon_talk(&mut self) {
        self.add(EventKind::PlayerSummonTalk, &[]);
    }

    /// `@RegisterEvent(ON_PLAYABLE_EXP_CHANGED)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_playable_exp_changed(&mut self) {
        self.add(EventKind::PlayableExpChanged, &[]);
    }

    /// `@RegisterEvent(ON_TRAP_ACTION)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_trap_action(&mut self) {
        self.add(EventKind::TrapAction, &[]);
    }

    /// `@RegisterEvent(ON_DAILY_RESET)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_daily_reset(&mut self) {
        self.add(EventKind::DailyReset, &[]);
    }

    /// `@RegisterEvent(ON_CLAN_WAR_START)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_clan_war_start(&mut self) {
        self.add(EventKind::ClanWarStart, &[]);
    }

    /// `@RegisterEvent(ON_CLAN_WAR_FINISH)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_clan_war_finish(&mut self) {
        self.add(EventKind::ClanWarFinish, &[]);
    }

    /// `@RegisterEvent(ON_CASTLE_SIEGE_START)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_castle_siege_start(&mut self) {
        self.add(EventKind::CastleSiegeStart, &[]);
    }

    /// `@RegisterEvent(ON_CASTLE_SIEGE_FINISH)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_castle_siege_finish(&mut self) {
        self.add(EventKind::CastleSiegeFinish, &[]);
    }

    /// `@RegisterEvent(ON_CASTLE_SIEGE_OWNER_CHANGE)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_castle_siege_owner_change(&mut self) {
        self.add(EventKind::CastleSiegeOwnerChange, &[]);
    }

    /// `@RegisterEvent(ON_FORT_SIEGE_START)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_fort_siege_start(&mut self) {
        self.add(EventKind::FortSiegeStart, &[]);
    }

    /// `@RegisterEvent(ON_FORT_SIEGE_FINISH)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_fort_siege_finish(&mut self) {
        self.add(EventKind::FortSiegeFinish, &[]);
    }

    /// `@RegisterEvent(ON_OLYMPIAD_MATCH_RESULT)` (a global listener, S-API-04): the script hears the event for
    /// every object. The hook is the typed one of the event (see `Script::on_listener_event`).
    pub fn set_on_olympiad_match_result(&mut self) {
        self.add(EventKind::OlympiadMatchResult, &[]);
    }

    /// Java `addTrapActionId(npcIds)` (S-HAND-04): the script hears `ON_TRAP_ACTION` of the traps
    /// with these NPC ids (`on_trap_action`).
    pub fn add_trap_action_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::TrapAction, npc_ids);
    }

    /// `@RegisterEvent(ON_NPC_MANOR_BYPASS)` + `@RegisterType(NPC)` + `@Id(..)` (S-API-04).
    pub fn add_npc_manor_bypass_id(&mut self, npc_ids: &[i32]) {
        self.add(EventKind::NpcManorBypass, npc_ids);
    }

    /// Java `registerQuestItems(items)`: the items `exitQuest` takes away. A second call
    /// replaces the first, like Java's assignment.
    pub fn register_quest_items(&mut self, item_ids: &[i32]) {
        self.quest_items = item_ids.to_vec();
    }

    /// Java `addCondStart(predicate, html)`: a player who fails `condition` and has no state of
    /// the quest gets `html` instead of `on_talk`.
    pub fn add_cond_start(&mut self, condition: StartCondition, html: &str) {
        self.start_conditions.push((condition, html.to_owned()));
    }
}

/// The listener tables: who hears an event, in Java's order.
#[derive(Debug, Default)]
pub(crate) struct Listeners {
    by_id: HashMap<(EventKind, i32), Vec<ScriptId>>,
    global: HashMap<EventKind, Vec<ScriptId>>,
    /// The `NpcHate` listeners registered by `addCanSeeMeId`.
    can_see_me: std::collections::HashSet<(ScriptId, i32)>,
    /// Java `_registeredIds`, per script.
    registered: HashMap<ScriptId, Vec<(EventKind, i32)>>,
    /// The zone ids with an `ON_CREATURE_ZONE_ENTER` or `EXIT` listener, sorted (the NPC zone
    /// check reads it on every NPC move, S-API-05).
    zone_ids: Vec<i32>,
}

impl Listeners {
    /// Java `registerListener` for one script. NPC ids without a template are logged and
    /// skipped (`template.addListener` is only called for a known template); `npc_known` is
    /// `None` when no templates are loaded (tests without NPC data), and then every id counts.
    pub(crate) fn register(
        &mut self,
        script: ScriptId,
        script_name: &str,
        setup: &ScriptSetup,
        npc_known: Option<&dyn Fn(i32) -> bool>,
    ) {
        for (kind, ids) in &setup.listeners {
            if kind.register_type() == RegisterType::Global && ids.is_empty() {
                self.global.entry(*kind).or_default().push(script);
                continue;
            }
            for &id in ids {
                // `addTrapActionId(ids)`: the global kind `TrapAction` registered by NPC id.
                if (kind.register_type() == RegisterType::Npc || *kind == EventKind::TrapAction)
                    && let Some(known) = npc_known
                    && !known(id)
                {
                    let method = kind.java_method();
                    if !method.is_empty() {
                        tracing::error!("{script_name}: Found {method} for non existing NPC: {id}!");
                    }
                    continue;
                }
                let key_kind = if *kind == EventKind::NpcHateCanSeeMe {
                    self.can_see_me.insert((script, id));
                    EventKind::NpcHate
                } else {
                    *kind
                };
                if matches!(
                    key_kind,
                    EventKind::CreatureZoneEnter | EventKind::CreatureZoneExit
                ) && let Err(at) = self.zone_ids.binary_search(&id)
                {
                    self.zone_ids.insert(at, id);
                }
                let entry = self.by_id.entry((key_kind, id)).or_default();
                // Java's `PriorityBlockingQueue` holds the same listener object once per call;
                // a script that registers an id twice gets two listeners there, and here.
                entry.push(script);
                self.registered.entry(script).or_default().push((*kind, id));
            }
        }
    }

    /// The scripts listening to `kind` for `id`, in dispatch order.
    pub(crate) fn of(&self, kind: EventKind, id: i32) -> &[ScriptId] {
        self.by_id.get(&(kind, id)).map_or(&[], Vec::as_slice)
    }

    /// The scripts listening to a global event, in dispatch order.
    pub(crate) fn global(&self, kind: EventKind) -> &[ScriptId] {
        self.global.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// Whether the `NpcHate` listener of `script` for `npc_id` came from `addCanSeeMeId`.
    pub(crate) fn is_can_see_me(&self, script: ScriptId, npc_id: i32) -> bool {
        self.can_see_me.contains(&(script, npc_id))
    }

    /// Java `hasListener(type)` for an id.
    pub(crate) fn has(&self, kind: EventKind, id: i32) -> bool {
        !self.of(kind, id).is_empty()
    }

    /// The zone ids some script listens to (`ON_CREATURE_ZONE_ENTER` / `EXIT`), sorted.
    pub(crate) fn listened_zone_ids(&self) -> &[i32] {
        &self.zone_ids
    }

    /// Every kind the script registered (the ids and the global ones), without repeats.
    pub(crate) fn registered_kinds(&self, script: ScriptId) -> Vec<EventKind> {
        let mut kinds: Vec<EventKind> = Vec::new();
        let mut add = |kind: EventKind| {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        };
        if let Some(ids) = self.registered.get(&script) {
            for (kind, _) in ids {
                add(*kind);
            }
        }
        for (kind, scripts) in &self.global {
            if scripts.contains(&script) {
                add(*kind);
            }
        }
        kinds
    }

    /// Every `(kind, id)` the script registered for ids.
    pub(crate) fn registered_all(&self, script: ScriptId) -> Vec<(EventKind, i32)> {
        self.registered.get(&script).cloned().unwrap_or_default()
    }

    /// Java `getRegisteredIds(type)` of one script.
    pub(crate) fn registered_ids(&self, script: ScriptId, kind: EventKind) -> Vec<i32> {
        self.registered
            .get(&script)
            .map(|ids| {
                ids.iter()
                    .filter(|(k, _)| *k == kind)
                    .map(|(_, id)| *id)
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listeners_keep_registration_order_and_skip_unknown_npcs() {
        let mut a = ScriptSetup::default();
        a.add_talk_id(&[1, 2]);
        a.add_kill_id(&[99]);
        a.set_on_enter_world();
        let mut b = ScriptSetup::default();
        b.add_talk_id(&[2]);
        let mut listeners = Listeners::default();
        let known = |id: i32| id != 99;
        listeners.register(ScriptId(0), "A", &a, Some(&known));
        listeners.register(ScriptId(1), "B", &b, Some(&known));
        assert_eq!(listeners.of(EventKind::NpcTalk, 2), [ScriptId(0), ScriptId(1)]);
        assert_eq!(listeners.of(EventKind::NpcTalk, 1), [ScriptId(0)]);
        assert!(!listeners.has(EventKind::AttackableKill, 99));
        assert_eq!(listeners.global(EventKind::PlayerLogin), [ScriptId(0)]);
        assert_eq!(listeners.registered_ids(ScriptId(0), EventKind::NpcTalk), [1, 2]);
    }

    #[test]
    fn item_and_zone_ids_are_not_checked_against_npcs() {
        let mut a = ScriptSetup::default();
        a.add_item_talk_id(&[57]);
        a.add_enter_zone_id(&[12]);
        let mut listeners = Listeners::default();
        let known = |_: i32| false;
        listeners.register(ScriptId(0), "A", &a, Some(&known));
        assert!(listeners.has(EventKind::ItemTalk, 57));
        assert!(listeners.has(EventKind::CreatureZoneEnter, 12));
    }
}
