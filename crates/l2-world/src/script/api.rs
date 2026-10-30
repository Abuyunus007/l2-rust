//! The [`Script`] trait: what a ported Java `Quest` / `Script` implements.
// Java: mechanics/script/Quest.java (the `on*` methods), mechanics/script/Script.java
//
// A Java script is a subclass of `Quest` that overrides some `on*` methods and registers its
// listeners in the constructor. In Rust it is a type that implements [`Script`]:
// - [`Script::setup`] is the constructor: it registers the listeners (`addTalkId`, ...).
// - Every hook has an empty default; a script overrides the ones its Java class overrides.
// - Hooks take `&self`: a script holds only constants. Anything a Java script keeps in fields
//   lives in the world ([`super::ScriptCtx::state`]), so the world owns all state (D-10).
// - Objects are ids (D-11): an NPC is a [`ScriptNpc`] (object id + template id), a player an
//   `ObjectId`.
// - A hook that returns `Option<String>` returns what Java's returns: `None` for `null`, else a
//   file name, an html text or a chat message (`Quest.showResult`).

use l2_util::ObjectId;

use super::ctx::ScriptCtx;
use super::events::{DamageInfo, TerminateReturn, TrapAction};
use super::qs::Qs;
use super::setup::ScriptSetup;
use super::timer_executor::TimerParams;
use crate::effects::SkillKey;

/// An NPC as a script sees it: Java `npc.getObjectId()` and `npc.getId()` (the template id).
/// Copied into events, so a delayed event (the kill notification) still names the NPC after it
/// left the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptNpc {
    pub object_id: ObjectId,
    /// Java `npc.getId()`: the template id.
    pub id: i32,
}

/// An item as a script sees it: Java `item.getObjectId()` and `item.getId()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptItem {
    pub object_id: ObjectId,
    /// Java `item.getId()`: the template id.
    pub id: i32,
}

/// A ported Java script. See the module documentation and `docs/design/scripts.md`.
///
/// Implementors are registered once at startup (D-16: no reload) through
/// [`super::ScriptRegistry::add`]. They must be `Send + Sync` because the world moves to its own
/// thread; they hold no state.
#[allow(unused_variables)]
pub trait Script: Send + Sync + 'static {
    /// Java `getName()` (`getClass().getSimpleName()`): the name in `character_quests.name`, in
    /// `bypass Script <name>` and in `ScriptManager.getScript(name)`. Case sensitive.
    fn name(&self) -> &'static str;

    /// Java `getPath()`: the folder of the script below `data/scripts`, where its html pages
    /// are (`quests/Q00001_LettersOfLove`, `ai/others/Asamah`).
    fn path(&self) -> &'static str;

    /// Java `super(questId)`: the quest id; -1 for a plain `Script` (Java `Script()`). Ids from
    /// 1 to 19999 are quests (journal, quest list, chooser).
    fn id(&self) -> i32 {
        -1
    }

    /// Java `isCustomQuest()`: a custom quest shows its path instead of a client string.
    fn is_custom(&self) -> bool {
        false
    }

    /// Whether Java creates this script at all. Some scripts' `main` (or constructor) tests a
    /// config key first (`if (OlympiadConfig.OLYMPIAD_ENABLED)`, `ENABLE_BLOCK_CHECKER_EVENT`);
    /// when this is `false` at [`World::install_scripts`](crate::World::install_scripts) the
    /// script is not installed: no listeners, no name, no `on_load` (S-API-04). The world's
    /// settings must be in before the scripts are installed.
    fn enabled(&self, world: &crate::World) -> bool {
        true
    }

    /// The Java constructor: registers the listeners and the quest items.
    fn setup(&self, setup: &mut ScriptSetup);

    /// The Java constructor of a script that registers from loaded data or settings
    /// (`for (int npcId : LevelUpCrystalData.getInstance().getNpcsSoulInfo().keySet())
    /// addKillId(npcId)`): called by [`World::install_scripts`](crate::World::install_scripts)
    /// instead of [`Script::setup`], with the world's data and settings already in. The default
    /// calls `setup`; an override calls `self.setup(setup)` first and then adds the data-driven
    /// ids (S-API-05).
    fn setup_with(&self, setup: &mut ScriptSetup, world: &crate::World) {
        self.setup(setup);
    }

    /// Java `onLoad()`: called once after every script was registered and the world data is in.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {}

    /// Java `onSave()`: called at shutdown.
    fn on_save(&self, ctx: &mut ScriptCtx<'_>) {}

    /// Java `onEvent(event, npc, player)`: a `bypass Script <name> <event>` link or a quest
    /// timer. `npc` and `player` may be absent (a timer started without them).
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        None
    }

    /// Java `onTalk(npc, talker)`: the player chose this quest at a talk NPC (`addTalkId`).
    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        None
    }

    /// Java `onFirstTalk(npc, player)`: the first click on an NPC of `addFirstTalkId`; the
    /// result replaces the NPC's own page.
    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        None
    }

    /// Java `onItemTalk(item, player)` (`addItemTalkId`).
    fn on_item_talk(&self, ctx: &mut ScriptCtx<'_>, item: ScriptItem, player: ObjectId) -> Option<String> {
        None
    }

    /// Java `onItemEvent(item, player, event)` (`addItemBypassEventId`).
    fn on_item_event(&self, ctx: &mut ScriptCtx<'_>, item: ScriptItem, player: ObjectId, event: &str) {}

    /// Java `onKill(npc, killer, isSummon)` (`addKillId`): delivered `onKillDelay` (2.5 s)
    /// after the death, like Java's `notifyEventAsyncDelayed`.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {}

    /// Java `onAttack(npc, attacker, damage, isSummon, skill)` (`addAttackId`). `skill` is
    /// `None` for a plain hit.
    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        is_summon: bool,
        skill: Option<SkillKey>,
    ) {
    }

    /// Java `onSpawn(npc)` (`addSpawnId`).
    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {}

    /// Java `onTeleport(npc)` (`addTeleportId`).
    fn on_teleport(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {}

    /// Java `onSkillSee(npc, caster, skill, targets, isSummon)` (`addSkillSeeId`).
    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        is_summon: bool,
    ) {
    }

    /// Java `onSpellFinished(npc, player, skill)` (`addSpellFinishedId`).
    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
    }

    /// Java `onFactionCall(npc, caller, attacker, isSummon)` (`addFactionCallId`).
    fn on_faction_call(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caller: ScriptNpc,
        attacker: ObjectId,
        is_summon: bool,
    ) {
    }

    /// Java `onAggroRangeEnter(npc, player, isSummon)` (`addAggroRangeEnterId`).
    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        is_summon: bool,
    ) {
    }

    /// Java `onCreatureSee(npc, creature)` (`addCreatureSeeId`).
    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {}

    /// Java `onTimerEvent(event, params, npc, player)`: a timer of [`ScriptCtx::add_timer`]
    /// (`getTimers()`) fired. The default logs the unhandled event like Java.
    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        tracing::warn!(
            "[{}]: Timer event arrived at non overriden onTimerEvent method event: {event} npc: {npc:?} player: {player:?}",
            ctx.script_name()
        );
    }

    /// Java `onTimerCancel(event, params, npc, player)`: a timer of `getTimers()` was cancelled.
    fn on_timer_cancel(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
    }

    /// `ON_PLAYER_BYPASS` (`set_on_player_bypass`): the player sent a bypass or a tutorial
    /// command (Java `OnPlayerBypass`, queued).
    fn on_player_bypass(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, command: &str) {}

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK` (`set_on_player_press_tutorial_mark`): the player clicked
    /// a tutorial question mark (Java `OnPlayerPressTutorialMark`, queued).
    fn on_player_press_tutorial_mark(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, mark_id: i32) {}

    /// `ON_PLAYER_LOGOUT` (`set_on_player_logout`): the player is leaving the world (Java
    /// `OnPlayerLogout`, queued: it runs after the player left, so only ids can be used).
    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {}

    /// `ON_DAY_NIGHT_CHANGE` (`set_on_day_night_change`): the game clock crossed dawn or dusk
    /// (Java `OnDayNightChange`, queued).
    fn on_day_night_change(&self, ctx: &mut ScriptCtx<'_>, is_night: bool) {}

    /// `ON_CREATURE_DEATH` (`add_creature_death_id`): a registered NPC died (Java
    /// `OnCreatureDeath`, synchronous: the creature is dead, the rest of the death did not
    /// happen yet). `killer` is `None` for a death without a killer.
    fn on_creature_death(&self, ctx: &mut ScriptCtx<'_>, killer: Option<ObjectId>, victim: ObjectId) {}

    /// Java `onEnterWorld(player)` (`setOnEnterWorld(true)`).
    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {}

    /// Java `onEnterZone(creature, zone)` (`addEnterZoneId`).
    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {}

    /// Java `onExitZone(creature, zone)` (`addExitZoneId`).
    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {}

    /// Java `onEventReceived(eventName, sender, receiver, reference)` (`addEventReceivedId`).
    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        sender: ScriptNpc,
        receiver: ScriptNpc,
        reference: Option<ObjectId>,
    ) -> Option<String> {
        None
    }

    /// Java `onMoveFinished(npc)` (`addMoveFinishedId`).
    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {}

    /// Java `onNodeArrived(npc)` (`addNodeArrivedId`): a walker reached a node of its route.
    fn on_node_arrived(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {}

    /// Java `onRouteFinished(npc)` (`addRouteFinishedId`): a walker reached its last node.
    fn on_route_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {}

    /// Java `onNpcHate(mob, player, isSummon)` (`addNpcHateId`): `false` keeps the monster
    /// from hating the player.
    fn on_npc_hate(
        &self,
        ctx: &mut ScriptCtx<'_>,
        mob: ScriptNpc,
        player: ObjectId,
        is_summon: bool,
    ) -> bool {
        true
    }

    /// Java `onCanSeeMe(npc, player)` (`addCanSeeMeId`).
    fn on_can_see_me(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> bool {
        false
    }

    /// Java `onDeath(killer, victim, qs)`: the player of `qs` died after
    /// [`Qs::add_notify_of_death`]. Java's default calls `onEvent("", npc killer, player)`.
    fn on_death(&self, ctx: &mut ScriptCtx<'_>, killer: ObjectId, victim: ObjectId, qs: Qs) {
        let npc = ctx.script_npc(killer);
        self.on_event(ctx, "", npc, Some(qs.player()));
    }

    /// Java `checkPartyMember(player, npc)`, the filter of
    /// [`ScriptCtx::random_party_member_for_npc`].
    fn check_party_member(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: Option<ScriptNpc>) -> bool {
        true
    }

    /// Java `checkPartyMember(qs, npc)`, the filter of
    /// [`ScriptCtx::random_party_member_state`].
    fn check_party_member_state(&self, ctx: &mut ScriptCtx<'_>, qs: Qs, npc: Option<ScriptNpc>) -> bool {
        true
    }

    /// Java `actionForEachPlayer(player, npc, isSummon)`, called by
    /// [`ScriptCtx::execute_for_each_player`].
    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        is_summon: bool,
    ) {
    }

    // ------------------------------------------------------------------ S-API-04 events

    /// A [`ScriptCtx::reread_global_variables`] could not read the rows (D-50): the action that
    /// waited for them is refused. The default only logs it.
    fn on_global_variables_reread_failed(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        tracing::warn!(
            "[{}]: the global variables could not be read for '{event}'; the action is refused.",
            ctx.script_name()
        );
    }

    /// The entry of every event of [`super::ScriptEvent`] for a listener of this script:
    /// `tag` is the owner given to [`ScriptCtx::add_listener`] (`None` for a global or NPC
    /// template listener). The default calls the typed hook below. The answer matters for the
    /// events whose Java call asks for a `TerminateReturn`.
    fn on_listener_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        tag: Option<i64>,
        event: &super::ScriptEvent,
    ) -> Option<TerminateReturn> {
        event.call_hook(self, ctx)
    }

    /// `ON_CREATURE_DEATH` with the answer `Playable.doDie` reads (`terminate` keeps the playable
    /// alive; `Creature.doDie` ignores the answer). The default calls
    /// [`Script::on_creature_death`], so a script that does not need the answer overrides that.
    fn on_creature_death_terminate(
        &self,
        ctx: &mut ScriptCtx<'_>,
        killer: Option<ObjectId>,
        victim: ObjectId,
    ) -> Option<TerminateReturn> {
        self.on_creature_death(ctx, killer, victim);
        None
    }

    /// `ON_CREATURE_ATTACK` (synchronous): `attacker` starts to attack. `terminate` cancels it.
    fn on_creature_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        attacker: ObjectId,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        None
    }

    /// `ON_CREATURE_ATTACKED` (synchronous, container: the target). `terminate` cancels the attack.
    fn on_creature_attacked(
        &self,
        ctx: &mut ScriptCtx<'_>,
        attacker: ObjectId,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        None
    }

    /// `ON_CREATURE_ATTACK_AVOID` (synchronous, container: the target that avoided).
    fn on_creature_attack_avoid(
        &self,
        ctx: &mut ScriptCtx<'_>,
        attacker: ObjectId,
        target: ObjectId,
        damage_over_time: bool,
    ) {
    }

    /// `ON_CREATURE_SKILL_USE` (synchronous, container: the caster). `terminate` cancels the cast.
    fn on_creature_skill_use(
        &self,
        ctx: &mut ScriptCtx<'_>,
        caster: ObjectId,
        skill: SkillKey,
        simultaneously: bool,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        None
    }

    /// `ON_CREATURE_DAMAGE_DEALT` (synchronous, container: the attacker).
    fn on_creature_damage_dealt(&self, ctx: &mut ScriptCtx<'_>, info: &DamageInfo) {}

    /// `ON_CREATURE_DAMAGE_RECEIVED` (queued, container: the target).
    fn on_creature_damage_received(&self, ctx: &mut ScriptCtx<'_>, info: &DamageInfo) {}

    /// `ON_CREATURE_KILLED` (synchronous, container: the victim), fired by `Creature.doDie` right
    /// after `ON_CREATURE_DEATH`. `terminate` keeps the creature from finishing its death (it
    /// stays dead at 0 hp: Java returns before the rewards).
    fn on_creature_killed(
        &self,
        ctx: &mut ScriptCtx<'_>,
        attacker: Option<ObjectId>,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        None
    }

    /// `ON_PLAYER_SIT` (queued, container: the player).
    fn on_player_sit(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {}

    /// `ON_PLAYER_LEVEL_CHANGED` (queued, container: the player).
    fn on_player_level_changed(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        old_level: i32,
        new_level: i32,
    ) {
    }

    /// `ON_PLAYER_ITEM_ADD` (queued, container: the player).
    fn on_player_item_add(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, item: ScriptItem) {}

    /// `ON_PLAYER_ITEM_PICKUP` (queued, container: the player).
    fn on_player_item_pickup(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, item: ScriptItem) {}

    /// `ON_PLAYER_PROFESSION_CHANGE` (queued, container: the player).
    fn on_player_profession_change(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        class_id: i32,
        is_sub_class: bool,
    ) {
    }

    /// `ON_PLAYER_SUMMON_TALK` (queued, container: the summon).
    fn on_player_summon_talk(&self, ctx: &mut ScriptCtx<'_>, summon: ObjectId) {}

    /// `ON_PLAYABLE_EXP_CHANGED` (synchronous, container: the playable). `terminate` refuses
    /// the experience.
    fn on_playable_exp_changed(
        &self,
        ctx: &mut ScriptCtx<'_>,
        playable: ObjectId,
        old_exp: i64,
        new_exp: i64,
    ) -> Option<TerminateReturn> {
        None
    }

    /// `ON_NPC_MANOR_BYPASS` (`add_npc_manor_bypass_id`, queued): the player pressed a
    /// `manor_menu_select` button at the NPC.
    fn on_npc_manor_bypass(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        request: i32,
        manor_id: i32,
        next_period: bool,
    ) {
    }

    /// `ON_TRAP_ACTION` (queued, container: the trap).
    fn on_trap_action(
        &self,
        ctx: &mut ScriptCtx<'_>,
        trap: ScriptNpc,
        trigger: ObjectId,
        action: TrapAction,
    ) {
    }

    /// `ON_DAILY_RESET` (synchronous).
    fn on_daily_reset(&self, ctx: &mut ScriptCtx<'_>) {}

    /// `ON_CLAN_WAR_START` (queued).
    fn on_clan_war_start(&self, ctx: &mut ScriptCtx<'_>, clan1: i32, clan2: i32) {}

    /// `ON_CLAN_WAR_FINISH` (queued).
    fn on_clan_war_finish(&self, ctx: &mut ScriptCtx<'_>, clan1: i32, clan2: i32) {}

    /// `ON_CASTLE_SIEGE_START` (queued).
    fn on_castle_siege_start(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {}

    /// `ON_CASTLE_SIEGE_FINISH` (queued).
    fn on_castle_siege_finish(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {}

    /// `ON_CASTLE_SIEGE_OWNER_CHANGE` (queued).
    fn on_castle_siege_owner_change(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {}

    /// `ON_FORT_SIEGE_START` (queued).
    fn on_fort_siege_start(&self, ctx: &mut ScriptCtx<'_>, fort_id: i32) {}

    /// `ON_FORT_SIEGE_FINISH` (queued).
    fn on_fort_siege_finish(&self, ctx: &mut ScriptCtx<'_>, fort_id: i32) {}

    /// `ON_OLYMPIAD_MATCH_RESULT` (queued): the fighters are character ids (`Participant`
    /// object ids; `winner` is `None` where Java passes `null`); `competition_type` is the
    /// `CompetitionType` ordinal (0 classed, 1 non-classed, 2 teams).
    fn on_olympiad_match_result(
        &self,
        ctx: &mut ScriptCtx<'_>,
        winner: Option<i32>,
        loser: i32,
        competition_type: i32,
    ) {
    }
}
