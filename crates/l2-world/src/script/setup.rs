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
            if kind.register_type() == RegisterType::Global {
                self.global.entry(*kind).or_default().push(script);
                continue;
            }
            for &id in ids {
                if kind.register_type() == RegisterType::Npc
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
