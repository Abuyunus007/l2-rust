//! The base of the Territory War quests ("For the Sake of the Territory ...", "Protect the ...",
//! "Kill the ...") and the script "TerritoryWarSuperClass" (quest id -1) the war talks to.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/TerritoryWarSuperClass.java
//
// Java's `class TerritoryWarSuperClass extends Quest` with public array fields becomes a
// [`TerritoryWarData`] static per quest plus the free functions here that take it;
// [`territory_war_quest!`] writes the `impl Script` of a quest (the same shape as the sagas'
// `saga_quest!`). The quest lists of `TerritoryWarSuperClassLoader` are in `loader`.
//
// The instance with `questId < 0` is the [`TerritoryWarSuperClass`] script. Its constructor
// (the next war date, `addSkillSeeId(36590)`) is `World::load_territories` and
// `World::territory_on_skill_see` (M7-TW-01); the war calls `setOnEnterWorld(value)` on it as the
// event `setOnEnterWorld 1` / `setOnEnterWorld 0` (`World::tw_update_player_state_flags`). Java's
// `setOnEnterWorld(true)` registers a login listener and `false` removes it; here the script
// always listens to the login and answers only while the switch in its state is on.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::warn;

use super::territory_war_super_class_loader as loader;

/// `Q00147_PathtoBecominganEliteMercenary` / `Q00148_PathtoBecominganExaltedMercenary` /
/// `Q00176_StepsForHonor` (Java reads `X.class.getSimpleName()`).
const Q00147: &str = "Q00147_PathtoBecominganEliteMercenary";
const Q00148: &str = "Q00148_PathtoBecominganExaltedMercenary";
const Q00176: &str = "Q00176_StepsForHonor";

/// What a Territory War quest sets in its constructor (Java's public fields).
pub struct TerritoryWarData {
    /// The Java class name, which is the quest name.
    pub name: &'static str,
    /// `super(questId)`.
    pub quest_id: i32,
    /// `CATAPULT_ID` ("For the Sake").
    pub catapult_id: i32,
    /// `TERRITORY_ID` ("For the Sake"): 81..89.
    pub territory_id: i32,
    /// `LEADER_IDS` ("For the Sake").
    pub leader_ids: &'static [i32],
    /// `GUARD_IDS` ("For the Sake").
    pub guard_ids: &'static [i32],
    /// `npcString`.
    pub npc_string: &'static [NpcStringId],
    /// `NPC_IDS` ("Protect the").
    pub npc_ids: &'static [i32],
    /// `getTerritoryIdForThisNPCId(npcId)` ("Protect the"); the base returns 0.
    pub territory_of_npc: fn(i32) -> i32,
    /// `CLASS_IDS` ("Kill the").
    pub class_ids: &'static [i32],
    /// `RANDOM_MIN` ("Kill the").
    pub random_min: i32,
    /// `RANDOM_MAX` ("Kill the").
    pub random_max: i32,
}

/// `getTerritoryIdForThisNPCId` of the base class.
fn no_territory(_npc_id: i32) -> i32 {
    0 // TODO: Implement this. (Java)
}

impl TerritoryWarData {
    /// A quest with nothing set: the fields not named in a quest keep this (Java's defaults).
    pub const NONE: TerritoryWarData = TerritoryWarData {
        name: "",
        quest_id: 0,
        catapult_id: 0,
        territory_id: 0,
        leader_ids: &[],
        guard_ids: &[],
        npc_string: &[],
        npc_ids: &[],
        territory_of_npc: no_territory,
        class_ids: &[],
        random_min: 0,
        random_max: 0,
    };
}

/// The state of the script "TerritoryWarSuperClass": whether its login listener is on.
#[derive(Default)]
struct EnterWorldListener {
    on: bool,
}

/// Java `TerritoryWarSuperClass` constructed with `questId < 0`, the instance the war manager
/// finds by the name `TerritoryWarSuperClass` (`TerritoryWarManager.qn`).
pub struct TerritoryWarSuperClass;

impl Script for TerritoryWarSuperClass {
    fn name(&self) -> &'static str {
        "TerritoryWarSuperClass"
    }

    fn path(&self) -> &'static str {
        "quests/TerritoryWarScripts"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_enter_world();
    }

    /// `setOnEnterWorld(value)`, called by the war manager.
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "setOnEnterWorld 1" => set_on_enter_world(ctx, true),
            "setOnEnterWorld 0" => set_on_enter_world(ctx, false),
            _ => {}
        }
        None
    }

    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if ctx.state::<EnterWorldListener>().on {
            on_enter_world(ctx, player);
        }
    }
}

/// Java `newQuestState(player)` or the player's state of a "For the Sake" quest, then the war
/// state: started, cond 1, not saved.
fn start_territory_quest(ctx: &mut ScriptCtx<'_>, name: &str, player: ObjectId) {
    let Some(qs) = ctx.quest_state_of(name, player, true) else {
        return;
    };
    qs.set_state_with(ctx, STARTED, false);
    qs.set_cond(ctx, 1, false);
}

/// Java `player.addNotifyQuestOfDeath(qs)` of the "Kill the" quest of the player's class (a
/// player of level 61 or more), or the warning when there is none.
fn register_player_on_death(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if ctx.player_level(player) < 61 {
        return;
    }
    let class_id = ctx.player_class_id(player);
    match loader::kill_the(class_id) {
        Some(killthe) => {
            if let Some(qs) = ctx.quest_state_of(killthe.name, player, true) {
                qs.add_notify_of_death(ctx, player);
            }
        }
        None => {
            warn!("TerritoryWar: Missing Kill the quest for player {player:?} whose class id: {class_id}")
        }
    }
}

/// Java `TerritoryWarSuperClass.onEnterWorld(player)`.
fn on_enter_world(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let territory_id = ctx.world().territory_registered_id(player);
    if territory_id > 0 {
        // register Territory Quest
        // NOTE(java-quirk): a territory without a quest is a NullPointerException in Java.
        let Some(territory_quest) = loader::for_the_sake(territory_id) else {
            return;
        };
        start_territory_quest(ctx, territory_quest.name, player);
        // register player on Death
        register_player_on_death(ctx, player);
    }
}

/// Java `TerritoryWarSuperClass.setOnEnterWorld(value)`: the listener and the quests of every
/// player on a side.
fn set_on_enter_world(ctx: &mut ScriptCtx<'_>, value: bool) {
    ctx.state::<EnterWorldListener>().on = value;
    let mut players: Vec<ObjectId> = ctx.world().players().map(|p| p.core.object_id).collect();
    players.sort_unstable();
    for player in players {
        let side = ctx.world().siege_side_of(player);
        if side > 0 {
            let Some(territory_quest) = loader::for_the_sake(side) else {
                continue;
            };
            if value {
                start_territory_quest(ctx, territory_quest.name, player);
                // register player on Death
                register_player_on_death(ctx, player);
            } else {
                if let Some(qs) = ctx.quest_state_of(territory_quest.name, player, true) {
                    qs.exit_quest(ctx, false, false);
                }
                for protect in loader::PROTECT_THE {
                    if let Some(qs) = ctx.quest_state_of(protect.name, player, false) {
                        qs.exit_quest(ctx, false, false);
                    }
                }
                // unregister player on Death
                // NOTE(java-quirk): the map is keyed by class id but asked for the class index
                // (0 to 3), which is the id of a few classes of the warrior quest (2 and 3).
                let class_index = ctx
                    .world()
                    .player(player)
                    .map_or(0, |p| p.data.profile.class_index);
                if let Some(killthe) = loader::kill_the(class_index)
                    && let Some(qs) = ctx.quest_state_of(killthe.name, player, false)
                {
                    qs.remove_notify_of_death(ctx, player);
                }
            }
        }
    }
}

/// `setup` of a Territory War quest: Java `registerKillIds()` ("For the Sake") and
/// `addAttackId(NPC_IDS)` ("Protect the").
pub fn register_npcs(data: &TerritoryWarData, s: &mut ScriptSetup) {
    if data.catapult_id != 0 {
        s.add_kill_id(&[data.catapult_id]);
    }
    s.add_kill_id(data.leader_ids);
    s.add_kill_id(data.guard_ids);
    s.add_attack_id(data.npc_ids);
}

/// Java `Calendar.getInstance().get(Calendar.DAY_OF_YEAR)` for a local time in milliseconds.
fn day_of_year(local_ms: i64) -> i32 {
    // Howard Hinnant's civil-from-days, then the days since 1 January of that year.
    let days = local_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let cumulative = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let day_of_month = doy - (153 * mp + 2) / 5 + 1;
    let mut day = cumulative[(month - 1) as usize] + day_of_month;
    if leap && month > 2 {
        day += 1;
    }
    day as i32
}

/// `Calendar.getInstance().get(Calendar.DAY_OF_YEAR)` now.
fn today(ctx: &ScriptCtx<'_>) -> i32 {
    let now = ctx.now_ms();
    day_of_year(now + l2_util::time::local_offset_millis(now))
}

/// Java `handleKillTheQuest(player)`.
fn handle_kill_the_quest(data: &TerritoryWarData, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let Some(qs) = ctx.get_quest_state(player, true) else {
        return;
    };
    let mut kill = 1;
    let max;
    if !qs.is_completed(ctx) {
        if !qs.is_started(ctx) {
            qs.set_state(ctx, STARTED);
            qs.set_cond(ctx, 1, false);
            qs.set(ctx, "kill", "1");
            max = ctx.get_random_range(data.random_min, data.random_max);
            qs.set(ctx, "max", &max.to_string());
        } else {
            kill = qs.get_int(ctx, "kill") + 1;
            max = qs.get_int(ctx, "max");
        }

        if kill >= max {
            ctx.world_mut().territory_give_quest_point(player);
            ctx.add_exp_and_sp(player, 534_000, 51_000);
            let day = today(ctx);
            qs.set(ctx, "doneDate", &day.to_string());
            qs.exit_quest(ctx, true, false);
            if let Some(done) = data.npc_string.get(1) {
                ctx.show_on_screen_npc_string(player, *done, 2, 10_000, &[]);
            }
        } else {
            qs.set(ctx, "kill", &kill.to_string());
            if let Some(progress) = data.npc_string.first() {
                ctx.show_on_screen_npc_string(
                    player,
                    *progress,
                    2,
                    10_000,
                    &[&max.to_string(), &kill.to_string()],
                );
            }
        }
    } else if qs.get_int(ctx, "doneDate") != today(ctx) {
        restart_kill_the_quest(data, ctx, player, qs);
    } else if ctx.is_gm(player) {
        // just for test
        let name = ctx.script_name();
        ctx.world()
            .send_message(player, &format!("Cleaning {name} Territory War quest by force!"));
        restart_kill_the_quest(data, ctx, player, qs);
    }
}

/// The body both "completed" branches of `handleKillTheQuest` share (Java repeats it).
fn restart_kill_the_quest(data: &TerritoryWarData, ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs) {
    qs.set_state(ctx, STARTED);
    qs.set_cond(ctx, 1, false);
    qs.set(ctx, "kill", "1");
    let max = ctx.get_random_range(data.random_min, data.random_max);
    qs.set(ctx, "max", &max.to_string());
    if let Some(progress) = data.npc_string.first() {
        ctx.show_on_screen_npc_string(player, *progress, 2, 10_000, &[&max.to_string(), "1"]);
    }
}

/// Java `onAttack(npc, player, damage, isSummon)` ("Protect the ...").
pub fn on_attack(data: &TerritoryWarData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    if ctx.npc_current_hp(npc) == f64::from(ctx.npc_max_hp(npc)) && data.npc_ids.contains(&npc.id) {
        let territory_id = (data.territory_of_npc)(npc.id);
        if (81..=89).contains(&territory_id) {
            let mut players: Vec<ObjectId> = ctx.world().players().map(|p| p.core.object_id).collect();
            players.sort_unstable();
            for pl in players {
                if ctx.world().siege_side_of(pl) == territory_id {
                    let Some(qs) = ctx.get_quest_state(pl, true) else {
                        continue;
                    };
                    if !qs.is_started(ctx) {
                        qs.set_state_with(ctx, STARTED, false);
                        qs.set_cond(ctx, 1, false);
                    }
                }
            }
        }
    }
}

/// Java `onDeath(killer, victim, qs)` ("Kill the ..."): `qs.player()` is the victim.
pub fn on_death(
    data: &TerritoryWarData,
    ctx: &mut ScriptCtx<'_>,
    killer: ObjectId,
    victim: ObjectId,
    qs: Qs,
) {
    if killer == victim || ctx.world().player(victim).is_none() || ctx.player_level(victim) < 61 {
        return;
    }

    let Some(acting_player) = ctx.world().acting_player(killer) else {
        return;
    };
    let victim_side = ctx.world().siege_side_of(qs.player());
    if let Some(party) = ctx.world().party_of(acting_player) {
        for pl in ctx.world().party_members(party) {
            let side = ctx.world().siege_side_of(pl);
            if side == victim_side || side == 0 || !ctx.check_if_in_range(2000, killer, pl, false) {
                continue;
            }

            if pl == acting_player {
                handle_steps_for_honor(ctx, acting_player);
                handle_become_mercenary_quest(ctx, acting_player, false);
            }

            handle_kill_the_quest(data, ctx, pl);
        }
    } else if ctx.world().siege_side_of(acting_player) != victim_side
        && ctx.world().siege_side_of(acting_player) > 0
    {
        handle_kill_the_quest(data, ctx, acting_player);
        handle_steps_for_honor(ctx, acting_player);
        handle_become_mercenary_quest(ctx, acting_player, false);
    }

    ctx.world_mut()
        .territory_give_point(acting_player, victim_side, 1);
}

/// Java `onKill(npc, killer, isSummon)` ("For the Sake of the Territory ...").
pub fn on_kill(data: &TerritoryWarData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId) {
    if npc.id == data.catapult_id {
        ctx.world_mut()
            .territory_catapult_destroyed(data.territory_id - 80);
        ctx.world_mut().territory_give_point(killer, data.territory_id, 4);
        if let Some(destroyed) = data.npc_string.first() {
            ctx.world_mut()
                .territory_announce_screen_message(*destroyed, 2, 10_000, 135_000, 13_500);
        }
        handle_become_mercenary_quest(ctx, killer, true);
    } else if data.leader_ids.contains(&npc.id) {
        ctx.world_mut().territory_give_point(killer, data.territory_id, 3);
    }

    let side = ctx.world().siege_side_of(killer);
    if side != data.territory_id && ctx.world().territory(side - 80).is_some() {
        ctx.world_mut().territory_add_quest_done(side - 80, 0);
    }
}

/// Java `handleBecomeMercenaryQuest(player, catapult)`.
fn handle_become_mercenary_quest(ctx: &mut ScriptCtx<'_>, player: ObjectId, catapult: bool) {
    let mut enemy_count = 10;
    let mut catapult_count = 1;
    let mut qs = ctx.quest_state_of(Q00147, player, false);
    if let Some(elite) = qs
        && elite.is_completed(ctx)
    {
        qs = ctx.quest_state_of(Q00148, player, false);
        enemy_count = 30;
        catapult_count = 2;
    }

    if let Some(qs) = qs
        && qs.is_started(ctx)
    {
        let cond = qs.get_cond(ctx);
        if catapult {
            if cond == 1 || cond == 2 {
                let count = qs.get_int(ctx, "catapult") + 1;
                qs.set(ctx, "catapult", &count.to_string());
                if count >= catapult_count {
                    qs.set_cond(ctx, if cond == 1 { 3 } else { 4 }, false);
                }
            }
        } else if cond == 1 || cond == 3 {
            let kills = qs.get_int(ctx, "kills") + 1;
            qs.set(ctx, "kills", &kills.to_string());
            if kills >= enemy_count {
                qs.set_cond(ctx, if cond == 1 { 2 } else { 4 }, false);
            }
        }
    }
}

/// Java `handleStepsForHonor(player)`.
fn handle_steps_for_honor(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let Some(q176) = ctx.quest_state_of(Q00176, player, false) else {
        return;
    };
    if !q176.is_started(ctx) {
        return;
    }
    let cond = q176.get_cond(ctx);
    if cond == 1 || cond == 3 || cond == 5 || cond == 7 {
        let kills = q176.get_int(ctx, "kills") + 1;
        q176.set_int(ctx, "kills", kills);
        if cond == 1 && kills >= 9 {
            q176.set_cond(ctx, 2, false);
            q176.set(ctx, "kills", "0");
        } else if cond == 3 && kills >= 18 {
            q176.set_cond(ctx, 4, false);
            q176.set(ctx, "kills", "0");
        } else if cond == 5 && kills >= 27 {
            q176.set_cond(ctx, 6, false);
            q176.set(ctx, "kills", "0");
        } else if cond == 7 && kills >= 36 {
            q176.set_cond(ctx, 8, false);
            q176.unset(ctx, "kills");
        }
    }
}

/// Writes the `impl Script` of a Territory War quest: `territory_war_quest!(Q00717..., DATA);`
/// (the type and its [`TerritoryWarData`] static). The hooks are the ones the base class
/// overrides; the NPCs a quest listens to come from its data (`register_npcs`).
macro_rules! territory_war_quest {
    ($ty:ident, $data:expr) => {
        impl l2_world::Script for $ty {
            fn name(&self) -> &'static str {
                $data.name
            }

            fn path(&self) -> &'static str {
                "quests/TerritoryWarScripts"
            }

            fn id(&self) -> i32 {
                $data.quest_id
            }

            fn setup(&self, s: &mut l2_world::ScriptSetup) {
                $crate::quests::territory_war::territory_war_super_class::register_npcs(&$data, s);
            }

            fn on_attack(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                _attacker: l2_util::ObjectId,
                _damage: i32,
                _is_summon: bool,
                _skill: Option<l2_world::SkillKey>,
            ) {
                $crate::quests::territory_war::territory_war_super_class::on_attack(&$data, ctx, npc);
            }

            fn on_kill(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                killer: l2_util::ObjectId,
                _is_summon: bool,
            ) {
                $crate::quests::territory_war::territory_war_super_class::on_kill(&$data, ctx, npc, killer);
            }

            fn on_death(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                killer: l2_util::ObjectId,
                victim: l2_util::ObjectId,
                qs: l2_world::Qs,
            ) {
                $crate::quests::territory_war::territory_war_super_class::on_death(
                    &$data, ctx, killer, victim, qs,
                );
            }
        }
    };
}

pub(crate) use territory_war_quest;

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const SAKE: &str = "Q00717_ForTheSakeOfTheTerritoryGludio";
    const DION: &str = "Q00718_ForTheSakeOfTheTerritoryDion";
    const PROTECT: &str = "Q00729_ProtectTheTerritoryCatapult";
    const HEALERS: &str = "Q00737_DenyBlessings";
    const SUPER: &str = "TerritoryWarSuperClass";
    const ELITE: &str = "Q00147_PathtoBecominganEliteMercenary";
    const HONOR: &str = "Q00176_StepsForHonor";

    // The catapult and a leader of Gludio's quest.
    const CATAPULT: i32 = 36499;
    const LEADER: i32 = 36508;

    const NPCS: &str = r#"
        <npc id="36499" type="Monster" level="80" name="Gludio Catapult"/>
        <npc id="36508" type="Monster" level="80" name="Gludio Military Leader"/>"#;

    /// Stand-in of the quests of other folders the Territory War quests count for.
    struct StandIn(&'static str, i32);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/StandIn"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(ELITE, 147));
        registry.add(StandIn(HONOR, 176));
        crate::quests::territory_war::register(&mut registry);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        // Player 1 (a Phoenix Knight) is on Gludio's side, player 2 (a Bishop) on Dion's.
        t.enter_as(1, 80, 0, 90, vec![]);
        t.enter_as(2, 80, 0, 16, vec![]);
        t.world.set_siege_side(Oid(1), 81);
        t.world.set_siege_side(Oid(2), 82);
        t
    }

    /// The war manager's `setOnEnterWorld(value)`.
    fn set_on_enter_world(t: &mut ScriptTest, value: bool) {
        let event = if value {
            "setOnEnterWorld 1"
        } else {
            "setOnEnterWorld 0"
        };
        assert!(t.world.run_as_script(SUPER, |ctx| {
            TerritoryWarSuperClass.on_event(ctx, event, None, None);
        }));
    }

    fn started(t: &ScriptTest, player: i32, quest: &str) -> bool {
        t.world
            .player_quests(Oid(player))
            .and_then(|q| q.get(quest))
            .is_some_and(|s| s.is_started())
    }

    fn cond(t: &ScriptTest, player: i32, quest: &str) -> i32 {
        t.world
            .player_quests(Oid(player))
            .and_then(|q| q.get(quest))
            .map_or(-1, |s| s.cond())
    }

    fn var(t: &ScriptTest, player: i32, quest: &str, name: &str) -> Option<String> {
        t.world
            .player_quests(Oid(player))
            .and_then(|q| q.get(quest))
            .and_then(|s| s.get(name).map(str::to_owned))
    }

    /// Java `newQuestState` + `startQuest` of the quest `name`.
    fn start(t: &mut ScriptTest, name: &str, player: i32) {
        t.world.run_as_script(name, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.start_quest(ctx);
        });
    }

    #[test]
    fn the_war_starts_and_ends_the_quests_of_the_sides() {
        let mut t = setup();
        // The listener is off before the war: a login does nothing.
        t.world
            .run_as_script(SUPER, |ctx| TerritoryWarSuperClass.on_enter_world(ctx, Oid(1)));
        assert!(!started(&t, 1, SAKE));

        // `setOnEnterWorld(true)`: the "For the Sake" quest of the side runs from cond 1.
        set_on_enter_world(&mut t, true);
        assert!(started(&t, 1, SAKE));
        assert_eq!(cond(&t, 1, SAKE), 1);
        assert!(started(&t, 2, DION));
        assert!(!started(&t, 1, DION));

        // `setOnEnterWorld(false)`: the quest and a "Protect the" quest end.
        start(&mut t, PROTECT, 1);
        assert!(started(&t, 1, PROTECT));
        set_on_enter_world(&mut t, false);
        assert!(!started(&t, 1, SAKE) && !started(&t, 1, PROTECT));
        assert!(!started(&t, 2, DION));
    }

    #[test]
    fn a_kill_of_an_enemy_class_counts_for_the_killer_s_quest_and_points() {
        let mut t = setup();
        set_on_enter_world(&mut t, true);
        // Player 2 (a Bishop) dies by player 1: the "Deny Blessings" quest of player 1 starts.
        t.world.do_die(Oid(2), Some(Oid(1)));
        assert!(started(&t, 1, HEALERS));
        assert_eq!(var(&t, 1, HEALERS, "kill").as_deref(), Some("1"));
        let max: i32 = var(&t, 1, HEALERS, "max").unwrap().parse().unwrap();
        assert!((3..=8).contains(&max));
        // `giveTWPoint(killer, victimSide, 1)`: a player kill for player 1.
        assert_eq!(t.world.territory_points(Oid(1)).unwrap()[1], 1);
        assert_eq!(t.world.territory_points(Oid(1)).unwrap()[2], 0);
    }

    #[test]
    fn the_last_kill_gives_the_quest_point_and_the_badges_of_the_war_end() {
        let mut t = setup();
        set_on_enter_world(&mut t, true);
        // The kill count is drawn at the first kill; kill until the quest is done.
        let mut kills = 0;
        while t.world.territory_points(Oid(1)).is_none_or(|p| p[2] == 0) {
            kills += 1;
            assert!(kills <= 8, "the quest asks 3 to 8 kills");
            t.world.run_as_script(HEALERS, |ctx| {
                let qs = ctx.get_quest_state(Oid(2), true).unwrap();
                on_death(
                    &super::super::q00737_deny_blessings::DATA,
                    ctx,
                    Oid(1),
                    Oid(2),
                    qs,
                );
            });
        }
        assert!(kills >= 3);
        // `giveTWQuestPoint` and `exitQuest(true)` (the quest is not started any more). The
        // exp is times the quest rates, which the test world does not set.
        let points = t.world.territory_points(Oid(1)).unwrap();
        assert_eq!([points[1], points[2]], [kills, 1]);
        assert!(!started(&t, 1, HEALERS));
        // The war end: ten minutes online, then the badges (1 online + 7 for the quest).
        for _ in 0..10 {
            t.world.territory_give_point(Oid(1), 1000, 6);
        }
        assert_eq!(t.world.territory_calc_reward(Oid(1)), [81, 8]);
    }

    #[test]
    fn a_party_shares_the_kill_and_the_steps_for_honor_count_for_the_killer() {
        let mut t = setup();
        t.enter_as(3, 80, 0, 90, vec![]);
        t.world.set_siege_side(Oid(3), 81);
        t.party(1, 3);
        start(&mut t, HONOR, 1);
        set_on_enter_world(&mut t, true);
        t.world.do_die(Oid(2), Some(Oid(1)));
        // Both members of the party get the quest, only the killer the steps for honor.
        assert_eq!(var(&t, 1, HEALERS, "kill").as_deref(), Some("1"));
        assert_eq!(var(&t, 3, HEALERS, "kill").as_deref(), Some("1"));
        assert_eq!(var(&t, 1, HONOR, "kills").as_deref(), Some("1"));
        assert!(var(&t, 3, HONOR, "kills").is_none());
        // `giveTWPoint` with a party: a kill for each member of the other side within 2000.
        assert_eq!(t.world.territory_points(Oid(1)).unwrap()[1], 1);
        assert_eq!(t.world.territory_points(Oid(3)).unwrap()[1], 1);
    }

    #[test]
    fn a_catapult_and_a_leader_give_points_and_count_for_the_mercenary_quest() {
        let mut t = setup();
        t.world.set_siege_side(Oid(1), 82);
        start(&mut t, ELITE, 1);
        let catapult = t.spawn(CATAPULT, 20);
        t.kill(1, catapult);
        // 4 for a catapult of the territory 81, and the mercenary quest's catapult count.
        assert_eq!(t.world.territory_points(Oid(1)).unwrap()[4], 1);
        assert_eq!(var(&t, 1, ELITE, "catapult").as_deref(), Some("1"));
        assert_eq!(cond(&t, 1, ELITE), 3);
        let leader = t.spawn(LEADER, 20);
        t.kill(1, leader);
        assert_eq!(t.world.territory_points(Oid(1)).unwrap()[3], 1);
    }

    #[test]
    fn a_hit_on_a_protected_npc_starts_the_protect_quest_of_its_territory() {
        let mut t = setup();
        let catapult = t.spawn(CATAPULT, 20);
        t.world.run_as_script(PROTECT, |ctx| {
            let npc = ctx.script_npc(catapult).unwrap();
            on_attack(
                &super::super::q00729_protect_the_territory_catapult::DATA,
                ctx,
                npc,
            );
        });
        // Player 1 is on side 81 (36499 - 36418), player 2 on 82.
        assert!(started(&t, 1, PROTECT));
        assert!(!started(&t, 2, PROTECT));
    }

    #[test]
    fn the_day_of_the_year_is_java_s() {
        let day = |y: i64, m: i64, d: i64| {
            let y = if m <= 2 { y - 1 } else { y };
            let era = y.div_euclid(400);
            let yoe = y - era * 400;
            let mp = (m + 9) % 12;
            let doy = (153 * mp + 2) / 5 + d - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            day_of_year((era * 146_097 + doe - 719_468) * 86_400_000 + 3_600_000)
        };
        assert_eq!(day(2024, 1, 1), 1);
        assert_eq!(day(2024, 3, 1), 61);
        assert_eq!(day(2023, 3, 1), 60);
        assert_eq!(day(2024, 12, 31), 366);
        assert_eq!(day(2023, 12, 31), 365);
    }
}
