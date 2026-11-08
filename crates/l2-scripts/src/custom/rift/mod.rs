//! Rift invasions (L2Rust's own feature, D-64; settings `[RiftInvasion]` of `L2R.ini`, zones
//! `data/RiftInvasion.xml`).
//!
//! At random times a rift opens in one of the zones: it is announced, waves of invaders come out
//! of it, then the rift boss. Who fought (damage to the rift monsters), healed the fighters or
//! stood by gets rift shards (`RiftShardItemId`), which the Rissbeschwörer in Giran
//! (`RiftMerchantNpcId`) trades for rewards (multisell `RIFT_MULTISELL`).
//!
//! The course is [`machine::Machine`], ticked once a second by a repeating quest timer (the
//! world's timer wheel); the rewards are [`reward`]. Everything a rift spawns is a script spawn
//! (never saved): after a restart nothing of a running rift is left in the world.

mod machine;
mod reward;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::admin::JavaException;
use l2_world::{
    DamageInfo, NpcRewardOverride, RiftInvasionSettings, Script, ScriptCtx, ScriptNpc, ScriptSetup, World,
};
use tracing::{debug, info, warn};

use machine::{Action, Machine, Outcome, Phase, Timing};
use reward::{Rules, Stage};

const NAME: &str = "RiftInvasion";
/// The repeating timer that drives the rift.
const TICK: &str = "RIFT_TICK";
const TICK_MS: i64 = 1_000;
/// The visual marker of an open rift (template 50031, shown as the Vortex).
pub const MARKER_NPC: i32 = 50_031;
/// The rewards of the Rissbeschwörer (`data/multisell/90000001.xml`).
pub const RIFT_MULTISELL: i32 = 90_000_001;
/// What standing in the radius is worth each second: far below a fight, so presence alone
/// stays under `RiftMinContributionPercent` (anti-AFK), but it makes a character a participant.
const PRESENCE_POINTS: f64 = 1.0;
/// The ring around the rift its monsters stand in.
const RING: (i32, i32) = (150, 450);
/// How long after the rift's end a forgotten monster despawns by itself.
const DESPAWN_MARGIN_MS: i64 = 60_000;

/// The week (its start, local milliseconds) the shards below count in.
const VAR_WEEK: &str = "RiftWeek";
/// The shards a character got this week (the weekly cap).
const VAR_WEEK_SHARDS: &str = "RiftWeekShards";
/// The shards waiting for room in the inventory.
const VAR_PENDING: &str = "RiftPending";
/// A global variable per character (`RiftOffline_<objectId>`): shards earned while offline.
const GLOBAL_OFFLINE: &str = "RiftOffline_";

/// One wave, or the boss, of the running rift.
#[derive(Debug, Default)]
struct StageRun {
    stage: Stage,
    alive: BTreeSet<ObjectId>,
}

#[derive(Default)]
struct RiftState {
    machine: Machine,
    /// The zone of the announced or open rift (index into the zones).
    zone: Option<usize>,
    /// Where the rift opened.
    center: (i32, i32, i32),
    marker: Option<ObjectId>,
    /// The waves that came out, then the boss.
    stages: Vec<StageRun>,
    /// Every rift monster alive, with its stage.
    mobs: BTreeMap<ObjectId, usize>,
    /// Everybody who was in the radius or fought during this rift.
    participants: BTreeSet<ObjectId>,
    /// The Rissbeschwörer and where (template, location) it was spawned.
    merchant: Option<(ObjectId, i32, (i32, i32, i32))>,
    /// The last reason the rifts could not run (logged once each time it changes).
    problem: Option<String>,
}

/// The rift invasions.
pub struct RiftInvasion;

impl Script for RiftInvasion {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "custom/RiftInvasion"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_enter_world();
        s.set_on_creature_damage_received();
    }

    /// The Rissbeschwörer's id comes from `L2R.ini` (a changed id needs a restart).
    fn setup_with(&self, s: &mut ScriptSetup, world: &World) {
        self.setup(s);
        let merchant = world.l2r_settings().rift.merchant_npc_id;
        if merchant > 0 {
            s.add_start_npc(&[merchant]);
            s.add_first_talk_id(&[merchant]);
            s.add_talk_id(&[merchant]);
        }
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        for word in [
            "admin_rift_start",
            "admin_rift_stop",
            "admin_rift_status",
            "admin_rift_cap_reset",
        ] {
            ctx.world_mut().register_admin_handler(word, admin_rift);
        }
        ctx.start_quest_timer(TICK, TICK_MS, None, None, true);
    }

    fn is_event(&self) -> bool {
        true
    }

    /// `//event_start RiftInvasion`: the same as `//rift_start`.
    fn event_start(&self, ctx: &mut ScriptCtx<'_>, event_maker: Option<ObjectId>) -> bool {
        start(ctx, None, event_maker)
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        stop(ctx)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == TICK {
            tick(ctx);
            return None;
        }
        let player = player?;
        match event {
            "shop" => {
                ctx.world_mut()
                    .open_multisell(RIFT_MULTISELL, player, npc.map(|n| n.object_id), false);
                None
            }
            "status" => Some(status_page(ctx)),
            "collect" => {
                let text = match deliver_pending(ctx, player) {
                    Delivery::Nothing => "No rift shards are waiting for you.".to_owned(),
                    Delivery::Given(n) => format!("Here are your {n} rift shards."),
                    Delivery::NoRoom(n) => {
                        format!("Your inventory is still full: {n} rift shards keep waiting for you.")
                    }
                };
                Some(page(&text))
            }
            _ => Some(main_page(ctx, player)),
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(main_page(ctx, player))
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(main_page(ctx, player))
    }

    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        // Shards earned while offline, then what waits for room.
        let key = format!("{GLOBAL_OFFLINE}{}", player.0);
        let offline = ctx.global_variables().get_long(&key, 0);
        if offline > 0 {
            ctx.remove_global_variable(&key);
            grant(ctx, player, offline);
        } else if ctx.player_variable_int(player, VAR_PENDING, 0) > 0 {
            deliver_pending(ctx, player);
        }
    }

    fn on_creature_damage_received(&self, ctx: &mut ScriptCtx<'_>, info: &DamageInfo) {
        let Some(&stage) = ctx.state::<RiftState>().mobs.get(&info.target) else {
            return;
        };
        let Some(player) = info.attacker.and_then(|a| player_of(ctx.world(), a)) else {
            return;
        };
        let state = ctx.state::<RiftState>();
        state.participants.insert(player);
        if let Some(run) = state.stages.get_mut(stage) {
            *run.stage.contribution.entry(player).or_default() += info.damage.max(0.0);
        }
    }
}

/// The player behind a creature: itself, the owner of its summon or of its companion.
fn player_of(world: &World, creature: ObjectId) -> Option<ObjectId> {
    world
        .acting_player(creature)
        .or_else(|| world.companion_owner(creature))
}

fn settings(ctx: &ScriptCtx<'_>) -> RiftInvasionSettings {
    ctx.world().l2r_settings().rift.clone()
}

fn timing(s: &RiftInvasionSettings) -> Timing {
    Timing {
        lead_ms: s.announce_lead_ms,
        reminder: s.announce_reminder,
        wave_count: s.wave_count.max(1),
        wave_interval_ms: s.wave_interval_ms,
        duration_ms: s.duration_ms,
    }
}

/// Why no rift can run now, `None` when they can.
fn problem(ctx: &ScriptCtx<'_>, s: &RiftInvasionSettings) -> Option<String> {
    if !s.enabled {
        return Some("RiftEnabled = false".to_owned());
    }
    if let Some(missing) = s.missing_setting() {
        return Some(missing.to_owned());
    }
    if ctx.world().rift_invasion_zones().is_empty() {
        return Some("data/RiftInvasion.xml has no zone".to_owned());
    }
    None
}

/// Once a second: the settings (switched off: everything goes), the Rissbeschwörer, the plan,
/// and the course of the open rift.
fn tick(ctx: &mut ScriptCtx<'_>) {
    let s = settings(ctx);
    let problem = problem(ctx, &s);
    if ctx.state::<RiftState>().problem != problem {
        match &problem {
            Some(reason) if s.enabled => warn!("RiftInvasion: rift invasions are off: {reason}."),
            Some(reason) => info!("RiftInvasion: rift invasions are off ({reason})."),
            None => info!("RiftInvasion: rift invasions are on."),
        }
        ctx.state::<RiftState>().problem.clone_from(&problem);
    }
    if problem.is_some() {
        if ctx.state::<RiftState>().machine != Machine::default() {
            let actions = ctx.state::<RiftState>().machine.stop();
            perform(ctx, &s, actions);
        }
        remove_merchant(ctx);
        return;
    }
    keep_merchant(ctx, &s);
    if ctx.state::<RiftState>().machine.phase() == Phase::Idle
        && ctx.state::<RiftState>().machine.until_open().is_none()
    {
        let next = random_interval(ctx, &s);
        ctx.state::<RiftState>().machine.plan(next);
        debug!("RiftInvasion: the next rift opens in {} s.", next / 1000);
    }
    if ctx.state::<RiftState>().machine.is_running() {
        account(ctx, &s);
    }
    let (wave_cleared, boss_dead) = {
        let state = ctx.state::<RiftState>();
        let last = state.stages.last();
        let wave_cleared = last.is_some_and(|r| !r.stage.boss && r.stage.cleared);
        let boss_dead = last.is_some_and(|r| r.stage.boss && r.stage.cleared);
        (wave_cleared, boss_dead)
    };
    let actions = ctx
        .state::<RiftState>()
        .machine
        .tick(TICK_MS, &timing(&s), wave_cleared, boss_dead);
    perform(ctx, &s, actions);
}

fn random_interval(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) -> i64 {
    let min = i32::try_from(s.interval_min_ms).unwrap_or(i32::MAX);
    let max = i32::try_from(s.interval_max_ms).unwrap_or(i32::MAX).max(min);
    i64::from(ctx.get_random_range(min, max))
}

/// The dead monsters leave their stage; presence and heals of this second are counted.
fn account(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) {
    // The monsters that died or went.
    let mobs: Vec<(ObjectId, usize)> = ctx
        .state::<RiftState>()
        .mobs
        .iter()
        .map(|(m, i)| (*m, *i))
        .collect();
    for (mob, stage) in mobs {
        let gone = ctx.world().npc(mob).is_none_or(|n| n.decayed) || ctx.world().is_dead(mob);
        if gone {
            forget_mob(ctx, mob);
            let state = ctx.state::<RiftState>();
            if let Some(run) = state.stages.get_mut(stage) {
                run.alive.remove(&mob);
            }
        }
    }
    let present = players_in_radius(ctx, s);
    let heals = ctx.world_mut().take_heal_ledger();
    let world_view: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, f64)> = heals
        .into_iter()
        .map(|(healer, healed, hp)| {
            (
                healer,
                player_of(ctx.world(), healer),
                player_of(ctx.world(), healed),
                hp,
            )
        })
        .collect();
    let state = ctx.state::<RiftState>();
    state.participants.extend(present.iter().copied());
    for (index, run) in state.stages.iter_mut().enumerate() {
        if run.stage.cleared {
            continue;
        }
        for player in &present {
            *run.stage.contribution.entry(*player).or_default() += PRESENCE_POINTS;
        }
        // Healing a fighter is a contribution of the healer (both in the radius).
        for (_, healer, healed, hp) in &world_view {
            if let (Some(healer), Some(healed)) = (healer, healed)
                && present.contains(healer)
                && present.contains(healed)
            {
                *run.stage.contribution.entry(*healer).or_default() += hp;
            }
        }
        if run.alive.is_empty() {
            run.stage.cleared = true;
            let what = if run.stage.boss {
                "the boss".to_owned()
            } else {
                format!("wave {}", index + 1)
            };
            debug!(
                "RiftInvasion: {what} beaten; contributions: {:?}",
                run.stage
                    .contribution
                    .iter()
                    .map(|(p, c)| (p.0, c.round() as i64))
                    .collect::<Vec<_>>()
            );
        }
    }
}

/// The living players within `RiftParticipationRadius` of the open rift.
fn players_in_radius(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) -> BTreeSet<ObjectId> {
    let (cx, cy, cz) = ctx.state::<RiftState>().center;
    let r = f64::from(s.participation_radius);
    ctx.online_players()
        .into_iter()
        .filter(|p| !ctx.world().is_dead(*p))
        .filter(|p| {
            ctx.world().position_of(*p).is_some_and(|(x, y, z)| {
                let (dx, dy, dz) = (f64::from(x - cx), f64::from(y - cy), f64::from(z - cz));
                (dx * dx + dy * dy + dz * dz).sqrt() <= r
            })
        })
        .collect()
}

fn perform(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings, actions: Vec<Action>) {
    for action in actions {
        match action {
            Action::Announce => announce(ctx, s),
            Action::Remind => {
                let zone = zone_name(ctx);
                ctx.broadcast_to_all_online_players(&format!(
                    "Rift Invasion: the rift in {zone} opens in 1 minute!"
                ));
            }
            Action::Open => open(ctx, s),
            Action::Wave(n) => spawn_wave(ctx, s, n),
            Action::Boss => spawn_boss(ctx, s),
            Action::Finish(outcome) => finish(ctx, s, outcome),
        }
    }
}

/// Picks the zone (unless a GM picked one) and tells everybody.
fn announce(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) {
    let zones = ctx.world().rift_invasion_zones().len();
    if ctx.state::<RiftState>().zone.is_none_or(|z| z >= zones) {
        let pick = usize::try_from(ctx.get_random(i32::try_from(zones).unwrap_or(1))).unwrap_or(0);
        ctx.state::<RiftState>().zone = Some(pick);
    }
    let zone = zone_name(ctx);
    let minutes = ctx
        .state::<RiftState>()
        .machine
        .until_open()
        .map_or(0, |ms| (ms + 59_999) / 60_000);
    if minutes > 0 && s.announce_lead_ms > 0 {
        let plural = if minutes == 1 { "" } else { "s" };
        ctx.broadcast_to_all_online_players(&format!(
            "Rift Invasion: a rift is about to open in {zone} in {minutes} minute{plural}!"
        ));
    }
}

fn zone_name(ctx: &mut ScriptCtx<'_>) -> String {
    let zone = ctx.state::<RiftState>().zone;
    zone.and_then(|z| ctx.world().rift_invasion_zones().get(z))
        .map_or_else(|| "an unknown land".to_owned(), |z| z.name.clone())
}

/// The rift opens at one of the zone's points: its marker appears; the heals start to count.
fn open(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) {
    let Some(zone) = ctx.state::<RiftState>().zone else {
        return;
    };
    let Some(points) = ctx
        .world()
        .rift_invasion_zones()
        .get(zone)
        .map(|z| z.spawns.clone())
    else {
        return;
    };
    let pick = usize::try_from(ctx.get_random(i32::try_from(points.len()).unwrap_or(1))).unwrap_or(0);
    let (x, y, z) = points.get(pick).copied().unwrap_or_default();
    let z = ctx.geo_height(x, y, z);
    {
        let state = ctx.state::<RiftState>();
        state.center = (x, y, z);
        state.stages.clear();
        state.mobs.clear();
        state.participants.clear();
    }
    let lifetime = s.duration_ms + DESPAWN_MARGIN_MS;
    match ctx.add_spawn(MARKER_NPC, x, y, z, 0, false, lifetime, 0) {
        Some(marker) => ctx.state::<RiftState>().marker = Some(marker.object_id),
        None => warn!("RiftInvasion: the rift marker (NPC {MARKER_NPC}) cannot be spawned."),
    }
    ctx.world_mut().set_heal_ledger(true);
    let name = zone_name(ctx);
    info!("RiftInvasion: a rift opened in {name} at {x}, {y}, {z}.");
    ctx.broadcast_to_all_online_players(&format!(
        "Rift Invasion: a rift has opened in {name}! Invaders pour out of it."
    ));
}

/// What the players in the radius add to a wave: the factor on count and HP in percent.
fn scale_percent(s: &RiftInvasionSettings, players: usize) -> i32 {
    if !s.scale_with_players {
        return 100;
    }
    let extra = i32::try_from(players.saturating_sub(1)).unwrap_or(i32::MAX);
    100 + extra
        .saturating_mul(s.scale_per_player_percent)
        .min(s.scale_max_percent)
}

/// The level of wave `n` of `waves`: from the zone's lowest level up to its highest.
fn wave_level(min: i32, max: i32, n: usize, waves: usize) -> i32 {
    if waves <= 1 {
        return min;
    }
    let step = i32::try_from(n.saturating_sub(1)).unwrap_or(0);
    let span = i32::try_from(waves - 1).unwrap_or(1);
    min + (max - min) * step / span
}

fn spawn_wave(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings, n: usize) {
    let Some(zone) = ctx
        .state::<RiftState>()
        .zone
        .and_then(|z| ctx.world().rift_invasion_zones().get(z).cloned())
    else {
        return;
    };
    let Some(wave) = zone.waves.get(n - 1).or(zone.waves.last()).cloned() else {
        return;
    };
    let players = players_in_radius(ctx, s).len();
    let percent = scale_percent(s, players);
    let count = (i64::from(wave.count) * i64::from(percent) + 99) / 100;
    let level = wave_level(zone.min_level, zone.max_level, n, s.wave_count) + s.mob_level_offset;
    let hp = percent as f32 / 100.0;
    let index = ctx.state::<RiftState>().stages.len();
    ctx.state::<RiftState>().stages.push(StageRun::default());
    for _ in 0..count {
        let pick = ctx.get_random(i32::try_from(wave.mobs.len()).unwrap_or(1));
        let mob = wave.mobs[usize::try_from(pick).unwrap_or(0)];
        spawn_mob(ctx, s, mob, index, level.clamp(1, 99), hp);
    }
    let spawned = ctx.state::<RiftState>().stages[index].alive.len();
    info!(
        "RiftInvasion: wave {n}/{} in {}: {spawned} monsters of level {level} ({players} players near, {percent}%).",
        s.wave_count, zone.name
    );
    let text = format!(
        "Rift Invasion: wave {n} of {} comes out of the rift!",
        s.wave_count
    );
    screen_message(ctx, s, &text);
}

fn spawn_boss(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) {
    let Some(zone) = ctx
        .state::<RiftState>()
        .zone
        .and_then(|z| ctx.world().rift_invasion_zones().get(z).cloned())
    else {
        return;
    };
    let players = players_in_radius(ctx, s).len();
    let hp = scale_percent(s, players) as f32 / 100.0 * zone.boss_hp_multiplier as f32;
    let index = ctx.state::<RiftState>().stages.len();
    ctx.state::<RiftState>().stages.push(StageRun {
        stage: Stage {
            boss: true,
            ..Stage::default()
        },
        alive: BTreeSet::new(),
    });
    let level = (zone.max_level + s.mob_level_offset).clamp(1, 99);
    let boss = spawn_mob(ctx, s, zone.boss_id, index, level, hp);
    let name = boss
        .and_then(|b| {
            ctx.world()
                .stats(b)
                .and_then(|v| v.npc_template())
                .map(|t| t.name.clone())
        })
        .unwrap_or_else(|| "The rift boss".to_owned());
    info!(
        "RiftInvasion: the boss {name} (level {level}) came out in {}.",
        zone.name
    );
    ctx.broadcast_to_all_online_players(&format!(
        "Rift Invasion: {name} has come out of the rift in {}!",
        zone.name
    ));
    screen_message(ctx, s, &format!("Rift Invasion: {name} has appeared!"));
}

/// One rift monster: around the rift, at the ground, with its level, HP and rewards.
fn spawn_mob(
    ctx: &mut ScriptCtx<'_>,
    s: &RiftInvasionSettings,
    npc_id: i32,
    stage: usize,
    level: i32,
    hp: f32,
) -> Option<ObjectId> {
    let (cx, cy, cz) = ctx.state::<RiftState>().center;
    let angle = f64::from(ctx.get_random(360)).to_radians();
    let distance = f64::from(ctx.get_random_range(RING.0, RING.1));
    let x = cx + (distance * angle.cos()) as i32;
    let y = cy + (distance * angle.sin()) as i32;
    let z = ctx.geo_height(x, y, cz);
    let lifetime = ctx.state::<RiftState>().machine.time_left(&timing(s)) + DESPAWN_MARGIN_MS;
    let heading = ctx.get_random(65_536);
    let Some(npc) = ctx.add_spawn(npc_id, x, y, z, heading, false, lifetime, 0) else {
        warn!("RiftInvasion: NPC {npc_id} cannot be spawned.");
        return None;
    };
    let id = npc.object_id;
    ctx.world_mut().scale_monster(id, level, hp);
    ctx.world_mut().set_npc_reward_override(
        id,
        Some(NpcRewardOverride {
            exp_rate: s.mobs_exp_rate,
            sp_rate: s.mobs_sp_rate,
            drops: s.mobs_normal_drops,
        }),
    );
    let state = ctx.state::<RiftState>();
    state.mobs.insert(id, stage);
    if let Some(run) = state.stages.get_mut(stage) {
        run.alive.insert(id);
    }
    Some(id)
}

/// A message in the middle of the screen of everybody near the rift.
fn screen_message(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings, text: &str) {
    for player in players_in_radius(ctx, s) {
        ctx.show_on_screen_msg(player, text, 5_000);
    }
}

fn forget_mob(ctx: &mut ScriptCtx<'_>, mob: ObjectId) {
    ctx.state::<RiftState>().mobs.remove(&mob);
    ctx.world_mut().set_npc_reward_override(mob, None);
}

/// The rift is over: the rewards, the announcement, and nothing of it left in the world.
fn finish(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings, outcome: Outcome) {
    let zone = zone_name(ctx);
    let opened = ctx.state::<RiftState>().marker.is_some() || !ctx.state::<RiftState>().stages.is_empty();
    // Nothing of the rift stays.
    let mobs: Vec<ObjectId> = ctx.state::<RiftState>().mobs.keys().copied().collect();
    for mob in mobs {
        forget_mob(ctx, mob);
        if let Some(npc) = ctx.script_npc(mob) {
            ctx.delete_npc(npc);
        }
    }
    if let Some(marker) = ctx.state::<RiftState>().marker.take()
        && let Some(npc) = ctx.script_npc(marker)
    {
        ctx.delete_npc(npc);
    }
    ctx.world_mut().set_heal_ledger(false);

    let state = std::mem::take(ctx.state::<RiftState>());
    // The course, the merchant and the logged problem go on.
    {
        let next = ctx.state::<RiftState>();
        next.merchant = state.merchant;
        next.problem = state.problem.clone();
    }
    if !opened {
        info!("RiftInvasion: the rift announced in {zone} was called off.");
        ctx.broadcast_to_all_online_players(&format!("Rift Invasion: the rift in {zone} will not open."));
        return;
    }
    let stages: Vec<Stage> = state.stages.into_iter().map(|r| r.stage).collect();
    let rules = Rules {
        per_wave: s.shards_per_wave,
        boss: s.shards_boss,
        top_bonus: s.top_contributor_bonus,
        min_percent: s.min_contribution_percent,
    };
    let completed = outcome == Outcome::Completed;
    let awards = reward::awards(&stages, &rules, completed);
    let mut given = 0;
    let online: BTreeSet<ObjectId> = ctx.online_players().into_iter().collect();
    for (player, shards) in &awards {
        if online.contains(player) {
            given += grant(ctx, *player, *shards);
        } else {
            // Given (and counted against the weekly cap) at the next login.
            let key = format!("{GLOBAL_OFFLINE}{}", player.0);
            let before = ctx.global_variables().get_long(&key, 0);
            ctx.set_global_variable(&key, &(before + shards).to_string());
            given += shards;
        }
    }
    let beaten = stages.iter().filter(|s| s.cleared && !s.boss).count();
    info!(
        "RiftInvasion: the rift in {zone} ended {outcome:?}: {beaten}/{} waves beaten, {} participants, {} rewarded, {given} shards given.",
        s.wave_count,
        state.participants.len(),
        awards.len()
    );
    let text = match outcome {
        Outcome::Completed => format!(
            "Rift Invasion: the rift in {zone} is closed, its boss is slain! {} defenders were rewarded.",
            awards.len()
        ),
        Outcome::Failed => format!("Rift Invasion: the rift in {zone} collapsed, the invaders got away."),
        Outcome::Stopped => format!("Rift Invasion: the rift in {zone} was closed."),
    };
    ctx.broadcast_to_all_online_players(&text);
}

/// Gives a player `amount` shards as far as the weekly cap allows; what does not fit into the
/// inventory waits. Returns what was given (or waits).
fn grant(ctx: &mut ScriptCtx<'_>, player: ObjectId, amount: i64) -> i64 {
    let s = settings(ctx);
    let now = ctx.now_ms();
    let local = now + l2_util::time::local_offset_millis(now);
    let week = reward::week_start(local, s.weekly_reset_day, s.weekly_reset_hour);
    let stored_week = ctx
        .player_variable(player, VAR_WEEK)
        .and_then(|v| v.trim().parse::<i64>().ok());
    let already = if stored_week == Some(week) {
        ctx.player_variable(player, VAR_WEEK_SHARDS)
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(0)
    } else {
        0
    };
    let allowed = reward::capped(amount, already, s.weekly_cap);
    ctx.set_player_variable(player, VAR_WEEK, &week.to_string());
    ctx.set_player_variable(player, VAR_WEEK_SHARDS, &(already + allowed).to_string());
    if allowed < amount {
        ctx.send_message(
            player,
            &format!(
                "Rift Invasion: you reached the weekly limit of {} rift shards ({} not given).",
                s.weekly_cap,
                amount - allowed
            ),
        );
    }
    if allowed > 0 {
        let pending = ctx.player_variable_int(player, VAR_PENDING, 0);
        let total = i64::from(pending) + allowed;
        ctx.set_player_variable(player, VAR_PENDING, &total.to_string());
        deliver_pending(ctx, player);
    }
    allowed
}

enum Delivery {
    Nothing,
    Given(i64),
    NoRoom(i64),
}

/// Puts the waiting shards into the inventory when they fit; otherwise they keep waiting (never
/// on the ground).
fn deliver_pending(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Delivery {
    let pending = i64::from(ctx.player_variable_int(player, VAR_PENDING, 0));
    if pending <= 0 {
        return Delivery::Nothing;
    }
    let item = settings(ctx).shard_item_id;
    if item <= 0 {
        return Delivery::NoRoom(pending);
    }
    let fits = ctx.world().validate_capacity_by_item_id(player, item)
        && ctx.world().validate_weight_by_item_id(player, item, pending);
    if !fits {
        ctx.send_system_message(player, SystemMessageId::YourInventoryIsFull);
        ctx.send_message(
            player,
            &format!(
                "Rift Invasion: {pending} rift shards wait for you. Make room and talk to the Rissbeschwoerer in Giran, or log in again."
            ),
        );
        return Delivery::NoRoom(pending);
    }
    ctx.remove_player_variable(player, VAR_PENDING);
    ctx.give_items(player, item, pending);
    Delivery::Given(pending)
}

/// The Rissbeschwörer: the shop, the next rift, the waiting shards.
fn main_page(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> String {
    let pending = ctx.player_variable_int(player, VAR_PENDING, 0);
    let mut html = String::from(
        "<html><body><center><font color=\"LEVEL\">Rissbeschwoerer</font></center><br>\
         Rifts tear open all over the land. Close them, and bring me the shards they leave behind.<br><br>",
    );
    html.push_str("<a action=\"bypass -h Script RiftInvasion shop\">Trade rift shards</a><br>");
    html.push_str("<a action=\"bypass -h Script RiftInvasion status\">Next rift / status</a><br>");
    html.push_str(&format!(
        "<a action=\"bypass -h Script RiftInvasion collect\">Collect waiting shards ({pending})</a><br>"
    ));
    html.push_str("</body></html>");
    html
}

fn page(text: &str) -> String {
    format!(
        "<html><body><center><font color=\"LEVEL\">Rissbeschwoerer</font></center><br>{text}<br><br>\
         <a action=\"bypass -h Script RiftInvasion main\">Back</a></body></html>"
    )
}

fn status_page(ctx: &mut ScriptCtx<'_>) -> String {
    page(&status_text(ctx).replace('\n', "<br>"))
}

/// Where the rift stands, for the merchant's page and `//rift_status`.
fn status_text(ctx: &mut ScriptCtx<'_>) -> String {
    let s = settings(ctx);
    if let Some(reason) = problem(ctx, &s) {
        return format!("Rift invasions are off ({reason}).");
    }
    let zone = zone_name(ctx);
    let machine = ctx.state::<RiftState>().machine.clone();
    match machine.phase() {
        Phase::Idle => match machine.until_open() {
            Some(ms) => format!(
                "No rift is open. The next one opens in about {} minutes.",
                (ms + 59_999) / 60_000
            ),
            None => "No rift is open.".to_owned(),
        },
        Phase::Announced => format!(
            "A rift opens in {zone} in {} seconds.",
            machine.until_open().unwrap_or(0) / 1000
        ),
        Phase::Open | Phase::Boss => {
            let what = if machine.phase() == Phase::Boss {
                "the boss is out".to_owned()
            } else {
                format!("wave {} of {}", machine.wave(), s.wave_count)
            };
            let near = players_in_radius(ctx, &s).len();
            let state = ctx.state::<RiftState>();
            let alive = state.mobs.len();
            let participants = state.participants.len();
            format!(
                "A rift is open in {zone}: {what}, {alive} invaders alive.\nParticipants: {participants} ({near} near the rift).\nTime left: {} seconds.",
                machine.time_left(&timing(&s)) / 1000
            )
        }
    }
}

/// Starts a rift now, in `zone` (a name or its number) or a random one.
fn start(ctx: &mut ScriptCtx<'_>, zone: Option<&str>, gm: Option<ObjectId>) -> bool {
    let tell = |ctx: &mut ScriptCtx<'_>, text: &str| {
        if let Some(gm) = gm {
            ctx.send_message(gm, text);
        }
    };
    let s = settings(ctx);
    if let Some(reason) = problem(ctx, &s) {
        tell(ctx, &format!("Rift invasions are off ({reason})."));
        return false;
    }
    if ctx.state::<RiftState>().machine.is_running() {
        tell(ctx, "A rift is already open (//rift_stop ends it).");
        return false;
    }
    if let Some(wanted) = zone.map(str::trim).filter(|z| !z.is_empty()) {
        let zones = ctx.world().rift_invasion_zones();
        let by_number = wanted
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|n| *n < zones.len());
        let lower = wanted.to_lowercase();
        let found = by_number.or_else(|| zones.iter().position(|z| z.name.to_lowercase().contains(&lower)));
        let Some(found) = found else {
            let names: Vec<String> = zones
                .iter()
                .enumerate()
                .map(|(i, z)| format!("{} {}", i + 1, z.name))
                .collect();
            tell(
                ctx,
                &format!("No rift zone '{wanted}'. Zones: {}.", names.join(", ")),
            );
            return false;
        };
        ctx.state::<RiftState>().zone = Some(found);
    } else if ctx.state::<RiftState>().machine.phase() == Phase::Idle {
        ctx.state::<RiftState>().zone = None;
    }
    let actions = ctx.state::<RiftState>().machine.start_now();
    perform(ctx, &s, actions);
    let zone = zone_name(ctx);
    tell(ctx, &format!("A rift opened in {zone}."));
    true
}

fn stop(ctx: &mut ScriptCtx<'_>) -> bool {
    if ctx.state::<RiftState>().machine.phase() == Phase::Idle {
        return false;
    }
    let s = settings(ctx);
    let actions = ctx.state::<RiftState>().machine.stop();
    perform(ctx, &s, actions);
    true
}

/// `//rift_start [zone]`, `//rift_stop`, `//rift_status`, `//rift_cap_reset [player]`.
fn admin_rift(world: &mut World, gm: ObjectId, command: &str) -> Result<(), JavaException> {
    let (word, rest) = command.split_once(' ').unwrap_or((command, ""));
    let word = word.to_owned();
    let rest = rest.trim().to_owned();
    world.run_as_script(NAME, |ctx| match word.as_str() {
        "admin_rift_start" => {
            start(ctx, Some(&rest), Some(gm));
        }
        "admin_rift_stop" => {
            if !stop(ctx) {
                ctx.send_message(gm, "No rift is announced or open.");
            }
        }
        "admin_rift_status" => {
            for line in status_text(ctx).lines() {
                ctx.send_message(gm, line);
            }
        }
        "admin_rift_cap_reset" => cap_reset(ctx, gm, &rest),
        _ => {}
    });
    Ok(())
}

/// Resets the weekly shard count of a player (by name; the GM's target or the GM without one).
fn cap_reset(ctx: &mut ScriptCtx<'_>, gm: ObjectId, name: &str) {
    let player = if name.is_empty() {
        ctx.world()
            .target_of(gm)
            .filter(|t| ctx.world().player(*t).is_some())
            .unwrap_or(gm)
    } else {
        let Some(found) = ctx
            .online_players()
            .into_iter()
            .find(|p| ctx.player_name(*p).eq_ignore_ascii_case(name))
        else {
            ctx.send_message(gm, &format!("{name} is not online."));
            return;
        };
        found
    };
    ctx.remove_player_variable(player, VAR_WEEK);
    ctx.remove_player_variable(player, VAR_WEEK_SHARDS);
    let who = ctx.player_name(player);
    info!("RiftInvasion: the weekly rift shard count of {who} was reset.");
    ctx.send_message(gm, &format!("The weekly rift shard count of {who} is reset."));
}

/// The Rissbeschwörer stands where `L2R.ini` says (moved when that changes).
fn keep_merchant(ctx: &mut ScriptCtx<'_>, s: &RiftInvasionSettings) {
    let wanted = (s.merchant_npc_id, s.merchant_loc);
    if let Some((id, npc_id, loc)) = ctx.state::<RiftState>().merchant {
        // `ObjectId(0)`: it could not be spawned; tried again when the settings change.
        if (npc_id, loc) == wanted && (id == ObjectId(0) || ctx.world().npc(id).is_some_and(|n| !n.decayed)) {
            return;
        }
        remove_merchant(ctx);
    }
    let (x, y, z) = s.merchant_loc;
    let z = ctx.geo_height(x, y, z);
    match ctx.add_spawn(s.merchant_npc_id, x, y, z, 0, false, 0, 0) {
        Some(npc) => {
            ctx.state::<RiftState>().merchant = Some((npc.object_id, wanted.0, wanted.1));
        }
        None => {
            warn!(
                "RiftInvasion: the Rissbeschwoerer (NPC {}) cannot be spawned.",
                s.merchant_npc_id
            );
            ctx.state::<RiftState>().merchant = Some((ObjectId(0), wanted.0, wanted.1));
        }
    }
}

fn remove_merchant(ctx: &mut ScriptCtx<'_>) {
    if let Some((id, _, _)) = ctx.state::<RiftState>().merchant.take()
        && let Some(npc) = ctx.script_npc(id)
    {
        ctx.delete_npc(npc);
    }
}
