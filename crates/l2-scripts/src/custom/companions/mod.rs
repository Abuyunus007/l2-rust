//! Hired companions: virtual party members that fight beside a player.
//!
//! Own feature (not from the Java datapack). A player talks to a gatekeeper and picks "Hire
//! companions" (`Script Companions main`, linked from the pages of the town gatekeepers). A
//! companion is an NPC of the class `Companion` (`type="Companion"`, `ai/guards.rs`): the monsters
//! fight it and it fights the monsters, and nobody else. It is made for its owner's level
//! ([`scale`]) and plays one of six roles ([`role`]), decided by the rules in [`brain`].
//!
//! # Ownership
//! The script owns the companions of every player in one [`State`], on the world loop: no
//! locks, no shared pointers, and the NPCs are named by [`ObjectId`] only. The brain of an owner
//! is one repeating timer of the world's timer wheel ([`TICK_MS`]), not a thread. A tick takes the
//! owner's [`Squad`] out of the state, works on it, and puts it back, so nothing borrows the world
//! and the state at once.
//!
//! # Leaving
//! A companion goes when its owner logs out or dies, when the owner is no longer in the party
//! that he hired it in, when it dies itself, or when the owner sends it away. A logout is heard at
//! once (`on_player_logout`); the rest is looked at each tick, so it takes a second at most. (The
//! engine tells scripts about the death of NPCs only, not of players, so a hook for the owner's
//! death would never be called.)
//!
//! # Buffing
//! A companion buffs itself with what `.buffme` gives: the group of its role in
//! [`class_buffer`](super::class_buffer), through the same function the command uses, at once
//! and again whenever the first buff of the group has run out. The hire fee pays for it.

mod brain;
mod gear_data;
mod heal;
mod look;
mod pages;
mod role;
mod scale;
mod spell;

use std::collections::BTreeMap;

use l2_util::ObjectId;
use l2_world::{CompanionSetup, Intention, PartyId, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use self::brain::{Action, Enemy, Memory, Pos, Unit, View, think};
use self::heal::heal_for;
use self::pages::{MainPage, main_page};
use self::role::Role;
use self::scale::scale_for;
use self::spell::spell_for;
use super::class_buffer::{buff_target, grouped};
use super::class_buffer_data::group_by_key;

/// What a companion costs for each level of its owner, in Adena.
pub(crate) const FEE_PER_LEVEL: i64 = 5_000;
/// The most companions a player may have at once.
const MAX_COMPANIONS: usize = 3;
/// The level a player needs to hire.
const MIN_OWNER_LEVEL: i32 = 20;
/// How often a brain thinks.
const TICK_MS: i64 = 1_000;
/// The name of the brain's timer.
const TIMER: &str = "brain";
/// How far around the owner the brain looks for monsters.
const SCAN_RANGE: i32 = 1_500;
/// The hate a taunt puts on a monster, more than any damage a player does.
const TAUNT_HATE: i64 = 50_000;
/// The hate a companion starts a fight with.
const ATTACK_DESIRE: i64 = 999;
/// Where the hired ones appear around their owner: the ring is filled one by one.
const SLOTS: [(i32, i32); 3] = [(80, 60), (-80, 60), (0, -90)];

/// The price of one companion for a player of `level`.
fn hire_fee(level: i32) -> i64 {
    i64::from(level.max(0)) * FEE_PER_LEVEL
}

/// A hired companion.
struct Member {
    npc: ScriptNpc,
    role: Role,
    /// The level it was made for, to see when its owner has grown.
    level: i32,
    memory: Memory,
}

/// The companions of one player.
struct Squad {
    /// The party the owner was in when he hired the first one. The squad ends when he is not in it.
    party: Option<PartyId>,
    members: Vec<Member>,
    /// What the owner's party window shows now: his party and who of the squad is in it. When
    /// that is not what the squad is, the window is sent again.
    window: Option<(Option<PartyId>, Vec<ObjectId>)>,
}

/// Every squad, by owner (ordered, so ticks do not depend on hashing, D-13).
#[derive(Default)]
struct State {
    squads: BTreeMap<ObjectId, Squad>,
}

/// The share of hit points of a creature, 0 to 1.
fn ratio(current: f64, max: f64) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        (current / max).clamp(0.0, 1.0) as f32
    }
}

fn pos_of(ctx: &ScriptCtx<'_>, id: ObjectId) -> Option<Pos> {
    ctx.world().position_of(id).map(|(x, y, _)| Pos { x, y })
}

fn player_hp(ctx: &ScriptCtx<'_>, player: ObjectId) -> f32 {
    let max = ctx.world().stats(player).map_or(0.0, |s| f64::from(s.max_hp()));
    ratio(ctx.world().player(player).map_or(0.0, |p| p.core.cur_hp), max)
}

/// The maximum of the stat engine, with the buffs: `core.max_hp` of an NPC is the one it spawned
/// with, and a companion is buffed right after.
fn npc_hp(ctx: &ScriptCtx<'_>, id: ObjectId) -> f32 {
    let max = ctx.world().stats(id).map_or(0.0, |s| f64::from(s.max_hp()));
    ratio(ctx.world().npc(id).map_or(0.0, |n| n.core.cur_hp), max)
}

/// See [`npc_hp`].
fn npc_mp(ctx: &ScriptCtx<'_>, id: ObjectId) -> f32 {
    let max = ctx.world().stats(id).map_or(0.0, |s| f64::from(s.max_mp()));
    ratio(ctx.world().npc(id).map_or(0.0, |n| n.core.cur_mp), max)
}

/// The monsters around `owner` that are alive.
fn enemies_near(ctx: &ScriptCtx<'_>, owner: ObjectId) -> Vec<Enemy> {
    let world = ctx.world();
    world
        .visible_objects_in_range(owner, SCAN_RANGE)
        .into_iter()
        .filter(|&id| world.is_monster_npc(id) && !world.is_dead(id))
        .filter_map(|id| {
            Some(Enemy {
                id,
                pos: pos_of(ctx, id)?,
                hp: npc_hp(ctx, id),
                target: world.attack_target_of(id),
            })
        })
        .collect()
}

/// Gives a companion what `.buffme` gives its group, as the command does.
///
/// Buffs raise the maximum HP and MP by up to three quarters and not the current values, so the
/// shares are kept: a healthy companion stays healthy, and its healer has nothing to do.
fn buff_itself(ctx: &mut ScriptCtx<'_>, member: &Member) {
    let id = member.npc.object_id;
    let Some(group) = group_by_key(member.role.buff_group_key()) else {
        return;
    };
    let (hp, mp) = (f64::from(npc_hp(ctx, id)), f64::from(npc_mp(ctx, id)));
    buff_target(ctx, id, id, group);
    if let Some((max_hp, max_mp)) = ctx
        .world()
        .stats(id)
        .map(|s| (f64::from(s.max_hp()), f64::from(s.max_mp())))
    {
        ctx.world_mut().set_current_hp_mp(id, max_hp * hp, max_mp * mp);
    }
}

/// Does the companion still carry the first buff of its group?
fn is_buffed(ctx: &ScriptCtx<'_>, member: &Member) -> bool {
    group_by_key(member.role.buff_group_key())
        .and_then(|group| group.buffs.first())
        .is_none_or(|buff| {
            ctx.world()
                .script_buff_skill_level(member.npc.object_id, buff.id)
                .is_some()
        })
}

/// Sends a squad away: the NPCs leave and the brain stops.
fn dismiss(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: Squad) {
    if squad.window.is_some() {
        ctx.world().clear_companion_window(owner);
    }
    for member in squad.members {
        ctx.delete_npc(member.npc);
    }
    ctx.cancel_quest_timer(TIMER, None, Some(owner));
}

/// Sends all the companions of `owner` away. `true` when he had any.
fn dismiss_all(ctx: &mut ScriptCtx<'_>, owner: ObjectId) -> bool {
    match ctx.state::<State>().squads.remove(&owner) {
        Some(squad) => {
            dismiss(ctx, owner, squad);
            true
        }
        None => false,
    }
}

/// Hires a companion of `role` for `owner`: the checks, the fee, the NPC at its level, its buffs
/// and the brain. `Err` is what the player is told.
fn hire(ctx: &mut ScriptCtx<'_>, owner: ObjectId, role: Role) -> Result<(), String> {
    let level = ctx.player_level(owner);
    if ctx.is_dead(owner) {
        return Err("You cannot hire a companion while you are dead.".to_owned());
    }
    if level < MIN_OWNER_LEVEL {
        return Err(format!("You need level {MIN_OWNER_LEVEL} to hire a companion."));
    }
    if ctx.world().is_in_olympiad_mode(owner) {
        return Err("You cannot hire a companion during the Olympiad.".to_owned());
    }
    let count = ctx
        .state::<State>()
        .squads
        .get(&owner)
        .map_or(0, |s| s.members.len());
    if count >= MAX_COMPANIONS {
        return Err(format!("You can have {MAX_COMPANIONS} companions at most."));
    }
    let fee = hire_fee(level);
    if ctx.adena_of(owner) < fee {
        return Err(format!("A companion costs {} Adena.", grouped(fee)));
    }
    let (x, y, z) = ctx
        .world()
        .position_of(owner)
        .ok_or_else(|| "You are not in the world.".to_owned())?;
    let (dx, dy) = SLOTS[count % SLOTS.len()];
    let (heading, instance) = (ctx.heading_of(owner), ctx.world().instance_id_of(owner));
    // The figure, the strength and the owner are born with the NPC: nobody sees it as anything
    // else first, and its kills pay the owner from its first blow.
    let setup = CompanionSetup {
        owner,
        scale: scale_for(level),
        look: look::roll(role, level, &mut |bound| ctx.get_random(bound)),
    };
    let object_id = ctx
        .world_mut()
        .spawn_companion(role.npc_id(), (x + dx, y + dy, z), heading, instance, setup)
        .ok_or_else(|| "The companion cannot come to this place.".to_owned())?;
    let npc = ScriptNpc {
        object_id,
        id: role.npc_id(),
    };
    // The fee is checked above; if the purchase still fails the NPC goes again.
    if !ctx.world_mut().reduce_adena(owner, fee, true) {
        ctx.delete_npc(npc);
        return Err(format!("A companion costs {} Adena.", grouped(fee)));
    }
    let member = Member {
        npc,
        role,
        level,
        memory: Memory::default(),
    };
    buff_itself(ctx, &member);
    let party = ctx.world().party_of(owner);
    let state = ctx.state::<State>();
    let mut squad = state.squads.remove(&owner).unwrap_or(Squad {
        party,
        members: Vec::new(),
        window: None,
    });
    squad.members.push(member);
    // He sees the new companion in his party window at once.
    show_in_party_window(ctx, owner, &mut squad);
    ctx.state::<State>().squads.insert(owner, squad);
    ctx.start_quest_timer(TIMER, TICK_MS, None, Some(owner), true);
    Ok(())
}

/// One second of an owner's squad.
fn tick(ctx: &mut ScriptCtx<'_>, owner: ObjectId) {
    let Some(mut squad) = ctx.state::<State>().squads.remove(&owner) else {
        ctx.cancel_quest_timer(TIMER, None, Some(owner));
        return;
    };
    let owner_here = ctx.world().player(owner).is_some() && !ctx.is_dead(owner);
    let party_kept = squad
        .party
        .is_none_or(|party| ctx.world().party_of(owner) == Some(party));
    if !owner_here || !party_kept {
        if owner_here {
            ctx.send_message(
                owner,
                "Your companions leave: you are not in the party you hired them in.",
            );
        }
        dismiss(ctx, owner, squad);
        return;
    }
    // The fallen and the vanished are not replaced; their corpses decay on their own.
    squad.members.retain(|member| {
        let id = member.npc.object_id;
        ctx.world().npc(id).is_some_and(|npc| !npc.decayed) && !ctx.is_dead(id)
    });
    if squad.members.is_empty() {
        if squad.window.is_some() {
            ctx.world().clear_companion_window(owner);
        }
        ctx.cancel_quest_timer(TIMER, None, Some(owner));
        return;
    }
    run(ctx, owner, &mut squad);
    show_in_party_window(ctx, owner, &mut squad);
    ctx.state::<State>().squads.insert(owner, squad);
}

/// The companions in the party window of their owner: the window is sent whole when the squad or
/// his party is not what it shows, and every second the HP and MP of each companion.
fn show_in_party_window(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: &mut Squad) {
    let ids: Vec<ObjectId> = squad.members.iter().map(|m| m.npc.object_id).collect();
    let shown = (ctx.world().party_of(owner), ids);
    if squad.window.as_ref() == Some(&shown) {
        for id in &shown.1 {
            ctx.world().update_companion_window(owner, *id);
        }
    } else {
        ctx.world().send_companion_window(owner, &shown.1);
        squad.window = Some(shown);
    }
}

/// Keeps the members of a squad fit and next to their owner, then lets each decide and act.
fn run(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: &mut Squad) {
    let Some(leader_pos) = pos_of(ctx, owner) else {
        return;
    };
    let level = ctx.player_level(owner);
    let leader = Unit {
        id: owner,
        pos: leader_pos,
        hp: player_hp(ctx, owner),
    };
    let now_ms = u64::try_from(ctx.world().world_ms()).unwrap_or(0);
    let enemies = enemies_near(ctx, owner);
    let leader_target = ctx
        .target_of(owner)
        .filter(|t| enemies.iter().any(|e| e.id == *t));

    // First the upkeep, and the picture of everybody who can be hurt.
    let mut allies = vec![leader];
    for member in &mut squad.members {
        let id = member.npc.object_id;
        if member.level != level {
            ctx.world_mut().set_npc_scale(id, scale_for(level));
            member.level = level;
        }
        catch_up(ctx, owner, member);
        if !is_buffed(ctx, member) {
            buff_itself(ctx, member);
        }
        if let Some(pos) = pos_of(ctx, id) {
            allies.push(Unit {
                id,
                pos,
                hp: npc_hp(ctx, id),
            });
        }
    }

    // Then every member decides on the same picture.
    for member in &mut squad.members {
        let id = member.npc.object_id;
        let Some(me) = allies.iter().find(|a| a.id == id).copied() else {
            continue;
        };
        let view = View {
            now_ms,
            me,
            mp: npc_mp(ctx, id),
            leader,
            leader_target,
            allies: &allies,
            enemies: &enemies,
        };
        let action = think(member.role.behavior(), &view, &mut member.memory);
        act(ctx, owner, member, action);
    }
}

/// A companion that is far away or in another instance is put next to its owner.
fn catch_up(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &Member) {
    let id = member.npc.object_id;
    let (Some(here), Some(there)) = (pos_of(ctx, id), pos_of(ctx, owner)) else {
        return;
    };
    let same_instance = ctx.world().instance_id_of(id) == ctx.world().instance_id_of(owner);
    if same_instance && here.distance(there) <= brain::CATCH_UP_DISTANCE {
        return;
    }
    if let Some((x, y, z)) = ctx.world().position_of(owner) {
        let slot = SLOTS[usize::try_from(id.0).unwrap_or(0) % SLOTS.len()];
        let target = l2_world::TeleportTarget::at(x + slot.0, y + slot.1, z)
            .with_instance(ctx.world().instance_id_of(owner));
        ctx.world_mut().tele_to_location(id, target);
    }
}

/// Carries out what a companion decided.
fn act(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &Member, action: Action) {
    let npc = member.npc;
    match action {
        Action::Hold => {}
        Action::Follow => follow(ctx, owner, npc),
        Action::Attack(target) => attack(ctx, npc, target),
        Action::Taunt(target) => {
            // The monster now hates the companion more than anybody.
            ctx.world_mut()
                .add_damage_hate(target, npc.object_id, 0, TAUNT_HATE);
            attack(ctx, npc, target);
        }
        Action::Heal(ally) => {
            if let Some(step) = heal_for(member.level)
                && !ctx.world().is_casting_now(npc.object_id)
            {
                ctx.cast_skill_at(npc, Some(ally), SkillKey::new(step.skill_id, step.level));
            }
        }
        Action::Cast(target) => match spell_for(member.level) {
            // The intention, not a plain cast: the mage walks into range first.
            Some(step) if !ctx.world().is_casting_now(npc.object_id) => {
                let skill = SkillKey::new(step.skill_id, step.level);
                ctx.world_mut()
                    .set_intention_cast(npc.object_id, skill, Some(target));
            }
            Some(_) => {}
            None => attack(ctx, npc, target),
        },
    }
}

fn follow(ctx: &mut ScriptCtx<'_>, owner: ObjectId, npc: ScriptNpc) {
    // Following is a standing order of the engine: it is given once, not every second.
    if ctx.world().intention_of(npc.object_id) == Intention::Follow {
        return;
    }
    ctx.set_running(npc);
    ctx.world_mut().npc_set_intention_follow(npc.object_id, owner);
}

fn attack(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, target: ObjectId) {
    let world = ctx.world();
    if world.intention_of(npc.object_id) == Intention::Attack
        && world.attack_target_of(npc.object_id) == Some(target)
    {
        return;
    }
    ctx.add_attack_desire(npc, target, ATTACK_DESIRE);
}

/// The main page for `owner`, with `note` on top.
fn page(ctx: &mut ScriptCtx<'_>, owner: ObjectId, note: Option<&str>) -> String {
    let level = ctx.player_level(owner);
    let squad: Vec<Role> = ctx
        .state::<State>()
        .squads
        .get(&owner)
        .map(|s| s.members.iter().map(|m| m.role).collect())
        .unwrap_or_default();
    main_page(&MainPage {
        fee: hire_fee(level),
        squad: &squad,
        max: MAX_COMPANIONS,
        min_level: MIN_OWNER_LEVEL,
        note,
    })
}

/// The companions.
pub struct Companions;

impl Script for Companions {
    fn name(&self) -> &'static str {
        "Companions"
    }

    fn path(&self) -> &'static str {
        "custom/Companions"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_player_logout();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let owner = player?;
        if event == TIMER {
            tick(ctx, owner);
            return None;
        }
        let (command, key) = event.split_once(':').unwrap_or((event, ""));
        let note = match command {
            "hire" => match Role::from_key(key) {
                Some(role) => match hire(ctx, owner, role) {
                    Ok(()) => Some(format!("Your {} joins you.", role.title())),
                    Err(why) => Some(why),
                },
                None => None,
            },
            "dismiss" => {
                let index = key.parse::<usize>().ok();
                let squad = ctx.state::<State>().squads.get_mut(&owner);
                let removed = index
                    .zip(squad)
                    .and_then(|(i, s)| (i < s.members.len()).then(|| s.members.remove(i)));
                match removed {
                    Some(member) => {
                        ctx.delete_npc(member.npc);
                        if ctx
                            .state::<State>()
                            .squads
                            .get(&owner)
                            .is_some_and(|s| s.members.is_empty())
                        {
                            dismiss_all(ctx, owner);
                        }
                        Some(format!("The {} leaves.", member.role.title()))
                    }
                    None => None,
                }
            }
            "dismissall" => dismiss_all(ctx, owner).then(|| "Your companions leave.".to_owned()),
            _ => None,
        };
        Some(page(ctx, owner, note.as_deref()))
    }

    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        dismiss_all(ctx, player);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use l2_world::script::{player_class, race};
    use l2_world::{ClientInput, ScriptRegistry};

    use super::role::{NPC_ARCHER, NPC_DAGGER, NPC_HEALER, NPC_MAGE, NPC_TANK, NPC_WARRIOR};
    use super::*;
    use crate::custom::class_buffer_data::GROUPS;
    use l2_protocol_game::server::{PartySmallWindowAll, PartySmallWindowUpdate};

    use crate::testing::{
        Pages, ScriptTest, Sent, X, Y, Z, datapack, decode, item, name_of, names_to, npc_xml, skill_xml,
    };

    const GATEKEEPER: i32 = 50_998;
    const MONSTER: i32 = 50_999;
    /// A monster that pays experience, SP and Adena, and that a quest script watches.
    const PRIZE: i32 = 50_996;
    /// What the quest script of the test gives whoever it thinks killed the prize.
    const KILL_CREDIT: i64 = 7_777;
    const OWNER: ObjectId = ObjectId(1);
    /// The hit points of the test monster (in `EXTRA_NPCS`).
    const MOB_HP: f64 = 60_000.0;
    const ADENA_ITEM: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const EXTRA_NPCS: &str = r#"
        <npc id="50996" level="40" type="Monster" name="Prize">
          <race>UNDEAD</race><sex>MALE</sex>
          <acquire exp="100000" sp="10000"/>
          <stats str="40" int="21" dex="30" wit="20" con="43" men="10">
            <vitals hp="60000" hpRegen="1" mp="800" mpRegen="1"/>
            <attack physical="120" magical="100" random="0" critical="4" accuracy="100" attackSpeed="300" type="FIST" range="40" distance="80" width="120"/>
            <defence physical="100" magical="100"/>
            <speed><walk ground="50"/><run ground="120"/></speed>
          </stats>
          <status attackable="true"/>
          <ai type="FIGHTER" aggroRange="0" isAggressive="false"/>
          <dropLists><drop><group chance="100"><item id="57" min="500" max="500" chance="100"/></group></drop></dropLists>
        </npc>
        <npc id="50998" level="40" type="Folk" name="Gatekeeper"/>
        <npc id="50997" level="30" type="Monster" name="Hunter">
          <race>UNDEAD</race><sex>MALE</sex>
          <stats str="40" int="21" dex="30" wit="20" con="43" men="10">
            <vitals hp="60000" hpRegen="1" mp="800" mpRegen="1"/>
            <attack physical="120" magical="100" random="0" critical="4" accuracy="100" attackSpeed="300" type="FIST" range="40" distance="80" width="120"/>
            <defence physical="100" magical="100"/>
            <speed><walk ground="50"/><run ground="120"/></speed>
          </stats>
          <status attackable="true"/>
          <ai type="FIGHTER" aggroRange="600" isAggressive="true"/>
        </npc>
        <npc id="50999" level="30" type="Monster" name="Mob">
          <race>UNDEAD</race><sex>MALE</sex>
          <stats str="40" int="21" dex="30" wit="20" con="43" men="10">
            <vitals hp="60000" hpRegen="1" mp="800" mpRegen="1"/>
            <attack physical="120" magical="100" random="0" critical="4" accuracy="100" attackSpeed="300" type="FIST" range="40" distance="80" width="120"/>
            <defence physical="100" magical="100"/>
            <speed><walk ground="50"/><run ground="120"/></speed>
          </stats>
          <status attackable="true"/>
          <ai type="FIGHTER" aggroRange="0" isAggressive="false"/>
        </npc>"#;

    /// A quest script that watches the prize.
    struct KillSpy;

    impl Script for KillSpy {
        fn name(&self) -> &'static str {
            "KillSpy"
        }

        fn path(&self) -> &'static str {
            "custom/KillSpy"
        }

        fn setup(&self, s: &mut ScriptSetup) {
            s.add_kill_id(&[PRIZE]);
        }

        fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
            ctx.give_items(killer, 57, KILL_CREDIT);
        }
    }

    /// The skills the scene needs: the buffs of every group and the heals.
    fn needed_skills() -> BTreeSet<i32> {
        let mut ids: BTreeSet<i32> = GROUPS.iter().flat_map(|g| g.all()).map(|b| b.id).collect();
        ids.extend([1011, 1217, 1230, 1274]);
        ids
    }

    /// What the gatekeepers' pages carry (`data/html/teleporter/<id>.htm`).
    const GATEKEEPER_PAGE: &str = r#"<html><body>Gatekeeper<br><a action="bypass -h Script Companions main">Hire companions.</a></body></html>"#;

    /// A level `level` Duelist with `adena`, who has talked to a gatekeeper and clicked "Hire
    /// companions": the server only lets through the links of the page the player was shown.
    fn scene(root: &Path, level: i32, adena: i64) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Companions);
        registry.add(KillSpy);
        let npcs = format!("{}{EXTRA_NPCS}", npc_xml(root, "stats/npcs/50000-50099.xml"));
        let pages =
            Pages::of(&Companions).file(&format!("data/html/default/{GATEKEEPER}.htm"), GATEKEEPER_PAGE);
        let mut t = ScriptTest::with_pages(registry, &npcs, ADENA_ITEM, pages);
        t.set_skills(ADENA_ITEM, &skill_xml(root, &needed_skills()));
        t.enter_as(
            1,
            level,
            race::HUMAN,
            player_class::DUELIST,
            vec![item(100, 57, adena)],
        );
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        t.talk(1, gatekeeper);
        let page = click(&mut t, "main");
        assert!(page.contains("Companions"), "{page}");
        t
    }

    /// An event given to the script straight, as a timer or a crafted link would: no page
    /// check on the way. What the script answers.
    fn direct(t: &mut ScriptTest, event: &str) -> String {
        let mut answer = None;
        t.world.run_as_script("Companions", |ctx| {
            answer = Companions.on_event(ctx, event, None, Some(OWNER));
        });
        answer.unwrap_or_default()
    }

    fn click(t: &mut ScriptTest, event: &str) -> String {
        t.event(1, "Companions", event)
    }

    fn only(t: &ScriptTest, npc_id: i32) -> ObjectId {
        let found = t.npcs_of(npc_id);
        assert_eq!(found.len(), 1, "npc {npc_id}: {found:?}");
        found[0]
    }

    fn all_companions(t: &ScriptTest) -> usize {
        [
            NPC_TANK,
            NPC_HEALER,
            NPC_WARRIOR,
            NPC_ARCHER,
            NPC_DAGGER,
            NPC_MAGE,
        ]
        .iter()
        .map(|id| t.npcs_of(*id).len())
        .sum()
    }

    fn seconds(t: &mut ScriptTest, n: u32) {
        t.ticks(n * 20);
    }

    fn distance(t: &ScriptTest, a: ObjectId, b: ObjectId) -> f64 {
        let (a, b) = (t.world.position_of(a).unwrap(), t.world.position_of(b).unwrap());
        f64::from(a.0 - b.0).hypot(f64::from(a.1 - b.1))
    }

    fn mob_hp(t: &ScriptTest, mob: ObjectId) -> f64 {
        t.world.npc(mob).map_or(0.0, |n| n.core.cur_hp)
    }

    /// A monster next to the owner that is fighting him.
    fn aggressive_mob(t: &mut ScriptTest, dx: i32) -> ObjectId {
        let mob = t.spawn(MONSTER, dx);
        t.world.add_damage_hate(mob, OWNER, 0, 100);
        t.world.set_intention_attack(mob, OWNER);
        mob
    }

    // ---- rewards ----

    /// The experience table, and the owner at the start of his level: without a table nobody
    /// gains experience.
    fn with_experience(root: &Path, t: &mut ScriptTest) {
        use std::sync::Arc;
        let table = Arc::new(l2_data::ExperienceTable::load(root));
        t.world
            .set_exp_data(Arc::clone(&table), Arc::default(), Arc::default(), Arc::default());
        let level = t.world.player(OWNER).unwrap().core.level;
        let start = table.exp_for_level(level).unwrap();
        t.world.add_exp(OWNER, start);
        assert_eq!(t.world.player(OWNER).unwrap().core.level, level);
    }

    /// What the owner has of experience, SP and Adena, and what lies on the ground for whom.
    fn reward_state(t: &ScriptTest) -> (i64, i64, i64, Vec<Option<ObjectId>>) {
        let player = t.world.player(OWNER).unwrap();
        let loot = t
            .world
            .ground_item_ids()
            .into_iter()
            .filter_map(|id| t.world.ground_item(id))
            .filter(|item| item.data.item_id == 57)
            .map(|item| item.owner)
            .collect();
        (player.data.exp, player.data.sp, t.count(1, 57), loot)
    }

    #[test]
    fn what_a_companion_kills_pays_experience_sp_drop_and_quest_credit_to_its_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        with_experience(root, &mut t);
        click(&mut t, "hire:warrior");
        let warrior = only(&t, NPC_WARRIOR);
        let (exp, sp, adena, loot) = reward_state(&t);
        assert!(loot.is_empty());

        let prize = t.spawn(PRIZE, 100);
        // The companion is the only one who fought it, and it lands the killing blow.
        t.world.add_damage_hate(prize, warrior, 5_000, 100);
        t.kill(warrior.0, prize);

        let (exp_after, sp_after, adena_after, loot) = reward_state(&t);
        assert!(exp_after > exp, "no experience: {exp} -> {exp_after}");
        assert!(sp_after > sp, "no SP: {sp} -> {sp_after}");
        assert_eq!(loot, vec![Some(OWNER)], "the drop is the owner's to pick up");
        assert_eq!(
            adena_after - adena,
            KILL_CREDIT,
            "the quest script saw the owner as the killer"
        );
    }

    #[test]
    fn a_kill_shared_by_the_owner_and_a_companion_pays_the_owner_once_for_both() {
        let Some(root) = datapack() else { return };
        let mut alone = scene(root, 40, 10_000_000);
        with_experience(root, &mut alone);
        click(&mut alone, "hire:warrior");
        let (exp, ..) = reward_state(&alone);
        let prize = alone.spawn(PRIZE, 100);
        alone.world.add_damage_hate(prize, OWNER, 10_000, 100);
        alone.kill(1, prize);
        let by_himself = reward_state(&alone).0 - exp;

        let mut together = scene(root, 40, 10_000_000);
        with_experience(root, &mut together);
        click(&mut together, "hire:warrior");
        let warrior = only(&together, NPC_WARRIOR);
        let (exp, ..) = reward_state(&together);
        let prize = together.spawn(PRIZE, 100);
        together.world.add_damage_hate(prize, OWNER, 5_000, 100);
        together.world.add_damage_hate(prize, warrior, 5_000, 100);
        together.kill(1, prize);
        let with_help = reward_state(&together).0 - exp;

        assert!(by_himself > 0);
        assert_eq!(
            by_himself, with_help,
            "the reward of the monster is the same, helped or not"
        );
    }

    #[test]
    fn a_monster_killed_by_nobody_of_his_pays_nothing() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        with_experience(root, &mut t);
        click(&mut t, "hire:warrior");
        let (exp, _, adena, _) = reward_state(&t);
        let prize = t.spawn(PRIZE, 100);
        t.kill(0, prize);
        let (exp_after, _, adena_after, loot) = reward_state(&t);
        assert_eq!((exp_after, adena_after), (exp, adena));
        assert!(loot.iter().all(Option::is_none) || loot.is_empty());
    }

    // ---- looks ----

    #[test]
    fn a_companion_is_told_to_the_clients_as_a_player_with_gear_of_its_level() {
        use l2_protocol_game::server::{CharInfo, charscreen::paperdoll};

        use super::look::grade_for;

        let Some(root) = datapack() else { return };
        for level in [25, 45, 55, 70, 78, 82, 85] {
            let mut t = scene(root, level, 10_000_000);
            for role in [Role::Tank, Role::Healer, Role::Warrior] {
                let _ = t.take();
                click(&mut t, &format!("hire:{}", role.key()));
                let id = only(&t, role.npc_id());
                // The world shows a new object to the players around it on its next tick.
                seconds(&mut t, 1);
                let sent = t.take();
                let names = names_to(&sent, 1);
                assert!(names.iter().any(|n| n == "CHAR_INFO"), "{role:?}: {names:?}");
                assert!(
                    !names.iter().any(|n| n == "NPC_INFO"),
                    "{role:?} was told as an NPC: {names:?}"
                );

                let info = t
                    .world
                    .fake_player_info(id)
                    .expect("a companion has a player's info");
                let sent_info = sent
                    .iter()
                    .filter(|(to, body)| *to == 1 && name_of(body) == "CHAR_INFO")
                    .map(|(_, body)| decode::<CharInfo>(body))
                    .find(|c| c.object_id == id.0)
                    .expect("the owner was shown the companion");
                assert_eq!(sent_info.name, info.name);
                assert_eq!(sent_info.paperdoll_item_ids, info.paperdoll_item_ids);
                assert!(t.world.npc_info(id, OWNER).is_none(), "no NPC model besides");

                assert!(!info.name.is_empty() && info.name != "Warrior", "{}", info.name);
                assert!((0..5).contains(&info.race), "race {}", info.race);
                assert_eq!(info.title, role.title());
                let worn = |slot: usize| info.paperdoll_item_ids[slot];
                assert!(worn(paperdoll::CHEST) != 0, "{role:?} {level}: no armor");
                assert!(worn(paperdoll::RHAND) != 0, "{role:?} {level}: no weapon");
                let grade = grade_for(level);
                assert!(
                    gear_data::WEAPONS.contains(&(role.key(), grade, worn(paperdoll::RHAND))),
                    "{role:?} {level}: weapon {} is not for grade {grade}",
                    worn(paperdoll::RHAND)
                );
                assert!(
                    gear_data::ARMOR
                        .iter()
                        .any(|(g, _, parts)| *g == grade && parts[1] == worn(paperdoll::CHEST)),
                    "{role:?} {level}: armor {} is not for grade {grade}",
                    worn(paperdoll::CHEST)
                );
                assert_eq!(
                    worn(paperdoll::LHAND) != 0,
                    role == Role::Tank,
                    "{role:?}: the shield"
                );
            }
        }
    }

    #[test]
    fn the_companions_differ_in_race_sex_and_name() {
        let Some(root) = datapack() else { return };
        let mut seen = BTreeSet::new();
        let mut sexes = BTreeSet::new();
        let mut t = scene(root, 60, 10_000_000);
        for _ in 0..16 {
            click(&mut t, "hire:warrior");
            let info = t.world.fake_player_info(only(&t, NPC_WARRIOR)).unwrap();
            seen.insert((info.race, info.name.clone()));
            sexes.insert(info.is_female);
            click(&mut t, "dismissall");
        }
        assert!(seen.len() >= 8, "{seen:?}");
        assert!(
            seen.iter().map(|(race, _)| race).collect::<BTreeSet<_>>().len() >= 3,
            "{seen:?}"
        );
        assert_eq!(sexes.len(), 2, "both sexes: {sexes:?}");
    }

    #[test]
    fn a_player_who_comes_into_view_later_is_told_the_same_figure() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 50, 10_000_000);
        click(&mut t, "hire:archer");
        let id = only(&t, NPC_ARCHER);
        // A player who comes into view later is told the same figure.
        let before = t.world.fake_player_info(id).unwrap();
        let _ = t.take();
        t.enter_as(2, 50, race::HUMAN, player_class::DUELIST, vec![]);
        let sent = t.take();
        let shown: Vec<_> = sent
            .iter()
            .filter(|(to, body)| *to == 2 && name_of(body) == "CHAR_INFO")
            .map(|(_, body)| decode::<l2_protocol_game::server::CharInfo>(body))
            .filter(|c| c.object_id == id.0)
            .collect();
        assert_eq!(shown.len(), 1, "{:?}", names_to(&sent, 2));
        assert_eq!(shown[0].name, before.name);
        assert_eq!(shown[0].paperdoll_item_ids, before.paperdoll_item_ids);
    }

    // ---- party window ----

    /// A link of the page, without the page being taken from the packets (`click` takes the log).
    fn press(t: &mut ScriptTest, event: &str) {
        t.bypass(1, &format!("Script Companions {event}"));
    }

    /// The party window packets the owner got, in order.
    fn window_packets(sent: &Sent) -> Vec<(String, Vec<u8>)> {
        sent.iter()
            .filter(|(to, body)| *to == 1 && name_of(body).starts_with("PARTY_SMALL_WINDOW"))
            .map(|(_, body)| (name_of(body), body.clone()))
            .collect()
    }

    /// The last whole window the owner was sent.
    fn last_window(sent: &Sent) -> Option<PartySmallWindowAll> {
        window_packets(sent)
            .into_iter()
            .rev()
            .find(|(name, _)| name == "PARTY_SMALL_WINDOW_ALL")
            .map(|(_, body)| decode::<PartySmallWindowAll>(&body))
    }

    #[test]
    fn a_hired_companion_shows_in_the_owners_party_window_with_hp_and_mp() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        let _ = t.take();
        press(&mut t, "hire:healer");
        let healer = only(&t, NPC_HEALER);
        let look = t.world.fake_player_info(healer).unwrap();

        // The packets of a command go out with the next tick.
        seconds(&mut t, 1);
        let window = last_window(&t.take()).expect("the window was sent when he was hired");
        assert_eq!(
            window.leader_object_id, OWNER.0,
            "alone, the owner leads the window"
        );
        assert_eq!(window.members.len(), 1);
        let member = &window.members[0];
        assert_eq!(member.vitals.object_id, healer.0);
        assert_eq!(member.vitals.name, look.name, "the name of its figure");
        assert_eq!(member.vitals.class_id, look.class_id);
        assert_eq!(member.race, look.race);
        assert_eq!(member.vitals.level, 40);
        let npc = t.world.npc(healer).unwrap();
        assert_eq!(member.vitals.hp, npc.core.cur_hp as i32);
        assert_eq!(member.vitals.max_hp, t.world.stats(healer).unwrap().max_hp());
        assert!(
            member.vitals.max_mp > 0 && member.vitals.mp > 0,
            "{:?}",
            member.vitals
        );
    }

    #[test]
    fn every_further_companion_lands_in_the_window_too() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        let mut hired = Vec::new();
        for (key, npc) in [("tank", NPC_TANK), ("healer", NPC_HEALER), ("mage", NPC_MAGE)] {
            let _ = t.take();
            press(&mut t, &format!("hire:{key}"));
            hired.push(only(&t, npc).0);
            seconds(&mut t, 1);
            let packets = window_packets(&t.take());
            let names: Vec<&str> = packets.iter().map(|(n, _)| n.as_str()).collect();
            // A client takes no new members from a second whole window: it is emptied first.
            let all = names.iter().position(|n| *n == "PARTY_SMALL_WINDOW_ALL");
            let all = all.unwrap_or_else(|| panic!("{key}: {names:?}"));
            assert!(all > 0, "{key}: {names:?}");
            assert_eq!(
                names[all - 1],
                "PARTY_SMALL_WINDOW_DELETE_ALL",
                "{key}: {names:?}"
            );
            let window = decode::<PartySmallWindowAll>(&packets[all].1);
            let ids: Vec<i32> = window.members.iter().map(|m| m.vitals.object_id).collect();
            assert_eq!(ids, hired, "{key}");
        }
    }

    #[test]
    fn the_party_window_follows_the_hp_and_mp_of_a_companion() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        seconds(&mut t, 1);
        let _ = t.take();

        let max_hp = t.world.stats(tank).unwrap().max_hp();
        t.world.set_current_hp_mp(tank, f64::from(max_hp) / 4.0, 10.0);
        seconds(&mut t, 1);
        let updates: Vec<_> = window_packets(&t.take())
            .into_iter()
            .filter(|(name, _)| name == "PARTY_SMALL_WINDOW_UPDATE")
            .map(|(_, body)| decode::<PartySmallWindowUpdate>(&body).member)
            .filter(|m| m.object_id == tank.0)
            .collect();
        let last = updates.last().expect("an update within the second");
        assert!(last.hp <= max_hp / 4 + 2 && last.hp > 0, "{last:?}");
        assert_eq!(last.max_hp, max_hp);
        assert!(last.mp <= 12, "{last:?}");
    }

    #[test]
    fn the_party_window_has_the_party_and_the_companions_together() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        t.enter_as(2, 40, race::HUMAN, player_class::DUELIST, vec![]);
        t.party(1, 2);
        let _ = t.take();
        press(&mut t, "hire:warrior");
        let warrior = only(&t, NPC_WARRIOR);
        seconds(&mut t, 1);
        let window = last_window(&t.take()).expect("the window was sent");
        let ids: Vec<i32> = window.members.iter().map(|m| m.vitals.object_id).collect();
        assert_eq!(ids, vec![2, warrior.0], "the friend first, then the companion");
        assert_eq!(window.leader_object_id, OWNER.0);
        // The friend sees his own party, not the companion.
        let friend = t.take();
        assert!(
            friend
                .iter()
                .filter(|(to, body)| *to == 2 && name_of(body).starts_with("PARTY_SMALL_WINDOW"))
                .count()
                == 0,
            "the others of the party get nothing"
        );
    }

    #[test]
    fn a_companion_that_goes_or_falls_leaves_the_window() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        let tank = only(&t, NPC_TANK);
        let _ = t.take();
        t.world.do_die(tank, None);
        seconds(&mut t, 2);
        let window = last_window(&t.take()).expect("the window is sent again without it");
        let ids: Vec<i32> = window.members.iter().map(|m| m.vitals.object_id).collect();
        assert_eq!(ids, vec![only(&t, NPC_HEALER).0]);

        // The last one dismissed: the window closes.
        let _ = t.take();
        press(&mut t, "dismiss:0");
        seconds(&mut t, 1);
        let names: Vec<String> = window_packets(&t.take()).into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names.last().map(String::as_str),
            Some("PARTY_SMALL_WINDOW_DELETE_ALL"),
            "{names:?}"
        );
    }

    #[test]
    fn the_window_is_closed_when_the_companions_leave_with_their_owner_dead_or_out_of_the_party() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        let _ = t.take();
        t.world.do_die(OWNER, None);
        seconds(&mut t, 2);
        let names: Vec<String> = window_packets(&t.take()).into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names.last().map(String::as_str),
            Some("PARTY_SMALL_WINDOW_DELETE_ALL"),
            "{names:?}"
        );
        assert_eq!(all_companions(&t), 0);
    }

    // ---- hiring ----

    #[test]
    fn hiring_charges_level_times_five_thousand_and_makes_a_companion_of_the_owners_level() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 500_000);
        let page = click(&mut t, "hire:tank");
        assert!(page.contains("Your Tank joins you."), "{page}");
        assert_eq!(t.count(1, 57), 300_000, "40 x 5,000");
        let tank = only(&t, NPC_TANK);
        // Its buffs (Bless the Body, Song of Vitality, ...) add to the HP; look at the plain value.
        t.world.stop_all_effects(tank);
        let stats = t.world.stats(tank).unwrap();
        assert_eq!(stats.level(), 40, "the level of the owner, not the template's 85");
        let expected_hp = 32_000.0 * (40.0_f64 / 85.0).powi(2);
        assert!(
            (f64::from(stats.max_hp()) - expected_hp).abs() < 3.0,
            "{} against {expected_hp}",
            stats.max_hp()
        );
        assert!(t.world.npc(tank).unwrap().core.cur_hp > 0.0, "it comes alive");
        assert!(page.contains("dismiss:0"), "it is on the list now");
    }

    #[test]
    fn a_companion_is_as_strong_as_its_owners_level() {
        let Some(root) = datapack() else { return };
        let mut strengths = Vec::new();
        for level in [20, 50, 85] {
            let mut t = scene(root, level, 10_000_000);
            click(&mut t, "hire:warrior");
            let id = only(&t, NPC_WARRIOR);
            t.world.stop_all_effects(id);
            let stats = t.world.stats(id).unwrap();
            assert_eq!(stats.level(), level);
            strengths.push((stats.max_hp(), stats.p_atk(None)));
        }
        assert!(
            strengths[0].0 < strengths[1].0 && strengths[1].0 < strengths[2].0,
            "{strengths:?}"
        );
        assert!(
            strengths[0].1 < strengths[1].1 && strengths[1].1 < strengths[2].1,
            "{strengths:?}"
        );
        assert_eq!(strengths[2].0, 24_000, "at level 85 it is the template");
    }

    #[test]
    fn the_hired_companion_buffs_itself_with_the_buffme_group_of_its_role() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 80, 5_000_000);
        for role in Role::ALL {
            click(&mut t, &format!("hire:{}", role.key()));
        }
        // Three at most: the fourth is refused, so check the three that came.
        assert_eq!(all_companions(&t), 3);
        for role in [Role::Tank, Role::Healer, Role::Warrior] {
            let id = only(&t, role.npc_id());
            let group = group_by_key(role.buff_group_key()).unwrap();
            for buff in group.all() {
                assert_eq!(
                    t.world.script_buff_skill_level(id, buff.id),
                    Some(buff.level),
                    "{}: {}",
                    role.key(),
                    buff.name
                );
            }
        }
    }

    #[test]
    fn a_companion_buffs_itself_again_when_the_buffs_have_run_out() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 1_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        let first = group_by_key("tank").unwrap().buffs[0].id;
        assert!(t.world.script_buff_skill_level(tank, first).is_some());
        t.world.stop_all_effects(tank);
        assert!(t.world.script_buff_skill_level(tank, first).is_none());
        seconds(&mut t, 2);
        assert!(
            t.world.script_buff_skill_level(tank, first).is_some(),
            "buffed again"
        );
    }

    #[test]
    fn hiring_is_refused_without_the_money_the_level_or_the_room_and_costs_nothing() {
        let Some(root) = datapack() else { return };
        // Too little Adena.
        let mut t = scene(root, 40, 199_999);
        assert!(direct(&mut t, "hire:tank").contains("costs 200,000 Adena"));
        assert_eq!((all_companions(&t), t.count(1, 57)), (0, 199_999));
        // Too low a level.
        let mut t = scene(root, 19, 10_000_000);
        assert!(direct(&mut t, "hire:tank").contains("You need level 20"));
        assert_eq!((all_companions(&t), t.count(1, 57)), (0, 10_000_000));
        // Three at most.
        let mut t = scene(root, 40, 10_000_000);
        for key in ["tank", "healer", "warrior"] {
            click(&mut t, &format!("hire:{key}"));
        }
        let before = t.count(1, 57);
        assert!(direct(&mut t, "hire:archer").contains("3 companions at most"));
        assert_eq!((all_companions(&t), t.count(1, 57)), (3, before));
        // Dead.
        let mut t = scene(root, 40, 10_000_000);
        t.world.do_die(OWNER, None);
        t.take();
        assert!(direct(&mut t, "hire:tank").contains("while you are dead"));
        assert_eq!(all_companions(&t), 0);
        // Nonsense.
        let mut t = scene(root, 40, 10_000_000);
        direct(&mut t, "hire:bard");
        direct(&mut t, "hire:");
        direct(&mut t, "teleport:home");
        assert_eq!((all_companions(&t), t.count(1, 57)), (0, 10_000_000));
    }

    #[test]
    fn the_player_can_dismiss_one_or_all() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        click(&mut t, "hire:warrior");
        let page = click(&mut t, "dismiss:1");
        assert!(page.contains("The Healer leaves."), "{page}");
        assert_eq!(t.npcs_of(NPC_HEALER).len(), 0);
        assert_eq!(all_companions(&t), 2);
        // An index that is not there does nothing.
        direct(&mut t, "dismiss:7");
        direct(&mut t, "dismiss:x");
        assert_eq!(all_companions(&t), 2);
        let page = click(&mut t, "dismissall");
        assert!(page.contains("Your companions leave."));
        assert_eq!(all_companions(&t), 0);
        // And they can be hired again.
        click(&mut t, "hire:tank");
        assert_eq!(all_companions(&t), 1);
    }

    // ---- leaving ----

    #[test]
    fn the_companions_go_when_the_owner_logs_out() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        assert_eq!(all_companions(&t), 2);
        t.input(1, ClientInput::Logout);
        seconds(&mut t, 3);
        assert_eq!(all_companions(&t), 0);
    }

    #[test]
    fn the_companions_go_when_the_owner_dies() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:warrior");
        t.world.do_die(OWNER, None);
        seconds(&mut t, 2);
        assert_eq!(all_companions(&t), 0);
    }

    #[test]
    fn the_companions_go_when_the_owner_leaves_the_party_he_hired_them_in() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        t.enter(2, 40, vec![]);
        t.party(2, 1);
        click(&mut t, "hire:tank");
        assert_eq!(all_companions(&t), 1);
        seconds(&mut t, 2);
        assert_eq!(all_companions(&t), 1, "still in the party");
        t.input(1, ClientInput::WithdrawParty);
        seconds(&mut t, 2);
        assert_eq!(all_companions(&t), 0);
    }

    #[test]
    fn a_solo_hire_stays_when_the_owner_joins_a_party_later() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        t.enter(2, 40, vec![]);
        click(&mut t, "hire:tank");
        t.party(2, 1);
        seconds(&mut t, 3);
        assert_eq!(all_companions(&t), 1);
    }

    #[test]
    fn a_fallen_companion_leaves_the_list_and_room_for_a_new_one() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        for key in ["tank", "healer", "warrior"] {
            click(&mut t, &format!("hire:{key}"));
        }
        let healer = only(&t, NPC_HEALER);
        t.world.do_die(healer, None);
        seconds(&mut t, 2);
        let page = direct(&mut t, "main");
        assert!(!page.contains("Dismiss the Healer"), "{page}");
        assert!(page.contains("(2 of 3)"));
        direct(&mut t, "hire:archer");
        assert_eq!(t.npcs_of(NPC_ARCHER).len(), 1);
    }

    #[test]
    fn the_brain_stops_when_the_last_companion_is_gone() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "dismissall");
        // The timer is cancelled with the squad: a later tick finds nothing and costs nothing.
        seconds(&mut t, 3);
        assert_eq!(all_companions(&t), 0);
        click(&mut t, "hire:tank");
        assert_eq!(all_companions(&t), 1, "a new squad starts its own brain");
    }

    // ---- follow ----

    #[test]
    fn the_companions_follow_their_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:warrior");
        let warrior = only(&t, NPC_WARRIOR);
        t.world.set_position(OWNER, X + 900, Y, Z);
        let far = distance(&t, warrior, OWNER);
        seconds(&mut t, 4);
        let nearer = distance(&t, warrior, OWNER);
        assert!(nearer < far - 200.0, "{far:.0} then {nearer:.0}");
        seconds(&mut t, 6);
        assert!(
            distance(&t, warrior, OWNER) < 350.0,
            "{:.0}",
            distance(&t, warrior, OWNER)
        );
    }

    #[test]
    fn a_companion_left_far_behind_is_brought_to_the_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        t.world.set_position(OWNER, X + 6_000, Y, Z);
        seconds(&mut t, 3);
        assert!(
            distance(&t, tank, OWNER) < 400.0,
            "{:.0}",
            distance(&t, tank, OWNER)
        );
    }

    // ---- combat ----

    #[test]
    fn the_tank_takes_the_monster_from_the_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        let mob = aggressive_mob(&mut t, 150);
        seconds(&mut t, 8);
        assert_eq!(t.world.attack_target_of(mob), Some(tank), "the tank holds it");
        assert!(mob_hp(&t, mob) < MOB_HP, "and hits it back");
    }

    #[test]
    fn the_mage_casts_at_what_the_owner_selects_from_a_distance() {
        let Some(root) = datapack() else { return };
        for level in [25, 60] {
            let mut t = scene(root, level, 10_000_000);
            click(&mut t, "hire:mage");
            let mage = only(&t, NPC_MAGE);
            let mp = t.world.npc(mage).unwrap().core.cur_mp;
            let mob = t.spawn(MONSTER, 300);
            t.world.set_target(OWNER, Some(mob));
            let _ = t.take();
            let mut hit_from = None;
            for _ in 0..80 {
                t.ticks(5);
                if hit_from.is_none() && mob_hp(&t, mob) < MOB_HP {
                    hit_from = Some(distance(&t, mage, mob));
                }
            }
            let hit_from = hit_from.unwrap_or_else(|| panic!("level {level}: the spell never landed"));
            assert!(hit_from > 120.0, "level {level}: {hit_from:.0}");
            assert!(
                t.world.npc(mage).unwrap().core.cur_mp < mp,
                "level {level}: no MP spent"
            );
            assert!(casts(&mut t) > 0, "level {level}");
        }
    }

    #[test]
    fn what_a_mage_kills_pays_its_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        with_experience(root, &mut t);
        click(&mut t, "hire:mage");
        let mage = only(&t, NPC_MAGE);
        let (exp, ..) = reward_state(&t);
        let prize = t.spawn(PRIZE, 200);
        t.world.set_target(OWNER, Some(prize));
        seconds(&mut t, 8);
        assert!(mob_hp(&t, prize) < 60_000.0, "the mage hit it");
        t.kill(mage.0, prize);
        assert!(reward_state(&t).0 > exp, "the experience went to the owner");
    }

    #[test]
    fn the_dagger_hits_what_the_owner_selects_with_a_dagger_in_its_hand() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 10_000_000);
        click(&mut t, "hire:dagger");
        let id = only(&t, NPC_DAGGER);
        let right_hand = t.world.fake_player_info(id).unwrap().paperdoll_item_ids
            [l2_protocol_game::server::charscreen::paperdoll::RHAND];
        assert!(gear_data::WEAPONS.contains(&("dagger", super::look::grade_for(60), right_hand)));
        let mob = t.spawn(MONSTER, 200);
        t.world.set_target(OWNER, Some(mob));
        seconds(&mut t, 8);
        assert!(mob_hp(&t, mob) < MOB_HP, "{}", mob_hp(&t, mob));
    }

    #[test]
    fn a_damage_dealer_hits_what_the_owner_selects() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:warrior");
        let mob = t.spawn(MONSTER, 200);
        seconds(&mut t, 3);
        assert_eq!(mob_hp(&t, mob), MOB_HP, "an unengaged monster is left alone");
        t.world.set_target(OWNER, Some(mob));
        seconds(&mut t, 8);
        assert!(mob_hp(&t, mob) < MOB_HP, "{}", mob_hp(&t, mob));
    }

    #[test]
    fn the_archer_shoots_from_a_distance() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:archer");
        let archer = only(&t, NPC_ARCHER);
        let mob = t.spawn(MONSTER, 450);
        t.world.set_target(OWNER, Some(mob));
        let (mut shot_from, mut closest, mut targeted) = (None, f64::MAX, false);
        for _ in 0..40 {
            t.ticks(10);
            if mob_hp(&t, mob) <= 0.0 {
                break;
            }
            if shot_from.is_none() && mob_hp(&t, mob) < MOB_HP {
                shot_from = Some(distance(&t, archer, mob));
            }
            closest = closest.min(distance(&t, archer, mob));
            targeted |= t.world.attack_target_of(mob) == Some(archer);
        }
        // The first hit lands while the monster is still far away: it is shot, not stabbed.
        let shot_from = shot_from.expect("the archer hit the monster");
        assert!(shot_from > 250.0, "{shot_from:.0}");
        // The monster hates the archer now and walks over to it (a monster follows the target it
        // hates only if it may attack it, `check_target`).
        assert!(targeted, "the monster never went for the archer");
        assert!(
            closest < 150.0,
            "the monster did not come for the archer: {closest:.0}"
        );
    }

    #[test]
    fn the_healer_heals_the_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:healer");
        let hp = |t: &ScriptTest| t.world.player(OWNER).unwrap().core.cur_hp;
        // The test player's maximum is 1.
        t.world.set_current_hp_mp(OWNER, 0.3, 100.0);
        assert!(hp(&t) < 0.5);
        seconds(&mut t, 12);
        assert!(hp(&t) > 0.9, "healed to {}", hp(&t));
    }

    #[test]
    fn the_healer_keeps_the_owner_up_while_a_monster_hits_him() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:healer");
        let healer = only(&t, NPC_HEALER);
        let mp_before = t.world.npc(healer).unwrap().core.cur_mp;
        t.world.set_current_hp_mp(OWNER, 0.4, 100.0);
        seconds(&mut t, 10);
        assert!(t.world.npc(healer).unwrap().core.cur_mp < mp_before, "it cast");
    }

    /// How often the owner saw a spell or a heal start.
    fn casts(t: &mut ScriptTest) -> usize {
        names_to(&t.take(), 1)
            .iter()
            .filter(|n| *n == "MAGIC_SKILL_USE")
            .count()
    }

    #[test]
    fn a_healer_casts_nothing_while_nobody_is_hurt() {
        let Some(root) = datapack() else { return };
        for level in [25, 45, 60, 85] {
            let mut t = scene(root, level, 10_000_000);
            click(&mut t, "hire:tank");
            click(&mut t, "hire:healer");
            click(&mut t, "hire:warrior");
            let _ = t.take();
            seconds(&mut t, 15);
            assert_eq!(casts(&mut t), 0, "level {level}");
        }
    }

    #[test]
    fn a_hired_companion_comes_with_full_hp_and_mp_after_its_buffs() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 10_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        let stats = t.world.stats(tank).unwrap();
        let (max_hp, max_mp) = (f64::from(stats.max_hp()), f64::from(stats.max_mp()));
        let npc = t.world.npc(tank).unwrap();
        assert_eq!((npc.core.cur_hp, npc.core.cur_mp), (max_hp, max_mp));
    }

    #[test]
    fn buffing_again_keeps_the_share_of_hp_of_a_hurt_companion() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 10_000_000);
        click(&mut t, "hire:tank");
        let tank = only(&t, NPC_TANK);
        t.world.stop_all_effects(tank);
        let max = f64::from(t.world.stats(tank).unwrap().max_hp());
        t.world.set_current_hp_mp(tank, max * 0.5, 100.0);
        seconds(&mut t, 2);
        let share = t.world.npc(tank).unwrap().core.cur_hp / f64::from(t.world.stats(tank).unwrap().max_hp());
        assert!((0.48..0.55).contains(&share), "{share}");
    }

    #[test]
    fn a_companion_below_sixty_percent_is_healed_and_one_above_is_not() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        let tank = only(&t, NPC_TANK);
        let max = f64::from(t.world.stats(tank).unwrap().max_hp());

        t.world.set_current_hp_mp(tank, max * 0.7, 100.0);
        let _ = t.take();
        seconds(&mut t, 8);
        assert_eq!(casts(&mut t), 0, "70% is no reason to heal");

        // Measured against the maximum without its buffs, 40% would read as 70%.
        t.world.set_current_hp_mp(tank, max * 0.4, 100.0);
        let _ = t.take();
        seconds(&mut t, 8);
        assert!(casts(&mut t) > 0, "40% is");
    }

    #[test]
    fn a_companion_grows_with_its_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:warrior");
        let id = only(&t, NPC_WARRIOR);
        let before = t.world.stats(id).unwrap().max_hp();
        t.world.player_mut(OWNER).unwrap().core.level = 60;
        seconds(&mut t, 2);
        let stats = t.world.stats(id).unwrap();
        assert_eq!(stats.level(), 60);
        assert!(stats.max_hp() > before, "{} then {}", before, stats.max_hp());
    }

    // ---- who may hurt a companion ----

    #[test]
    fn only_monsters_can_hurt_a_companion() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:warrior");
        let (tank, warrior) = (only(&t, NPC_TANK), only(&t, NPC_WARRIOR));
        let full = |t: &ScriptTest, id| t.world.npc(id).unwrap().core.cur_hp;
        let (tank_hp, warrior_hp) = (full(&t, tank), full(&t, warrior));

        // Another player cannot attack it, not even by force.
        t.enter(2, 40, vec![]);
        t.world.set_target(ObjectId(2), Some(tank));
        for _ in 0..3 {
            t.input(
                2,
                ClientInput::AttackRequest {
                    object_id: tank.0,
                    attack_id: 0,
                },
            );
        }
        seconds(&mut t, 3);
        assert_ne!(t.world.intention_of(ObjectId(2)), l2_world::Intention::Attack);
        assert!(!t.world.is_auto_attackable_by(tank, ObjectId(2)));
        // A companion does not hit another companion, even when it is told to.
        t.world.add_damage_hate(tank, warrior, 0, 1_000);
        t.world.set_intention_attack(warrior, tank);
        seconds(&mut t, 4);
        assert_eq!((full(&t, tank), full(&t, warrior)), (tank_hp, warrior_hp));
        // A monster does, and the tank pays for holding it: below its maximum while the fight lasts.
        // Against its defence a hit is a few points, and its regeneration takes them back within
        // seconds: every tick is looked at.
        let mob = aggressive_mob(&mut t, 150);
        let (mut lowest, mut held) = (1.0_f64, false);
        for _ in 0..400 {
            t.ticks(1);
            let max = f64::from(t.world.stats(tank).unwrap().max_hp());
            lowest = lowest.min(full(&t, tank) / max);
            held |= t.world.attack_target_of(mob) == Some(tank);
        }
        assert!(held, "the tank never held the monster");
        assert!(
            lowest < 1.0,
            "the monster never hit the tank: {lowest}, its HP {}",
            mob_hp(&t, mob)
        );
    }

    #[test]
    fn an_aggressive_monster_leaves_a_companion_alone_until_it_is_provoked() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:warrior");
        let warrior = only(&t, NPC_WARRIOR);
        // The owner is in sight (that wakes the monster) but outside its aggro range of 600; the
        // companion is next to the hunter. Monsters pick fights with players and their servitors
        // only; an NPC is hit back, never started on.
        t.world.set_position(OWNER, X + 800, Y, Z);
        t.world.set_position(warrior, X, Y, Z);
        let hunter = t.world.spawn_npc_at(50_997, X - 100, Y, Z, 0).unwrap();
        let mut went_for_it = false;
        for _ in 0..8 {
            t.ticks(10);
            went_for_it |= t.world.attack_target_of(hunter) == Some(warrior);
        }
        assert!(!went_for_it);
        // Once the companion hits it, the monster fights back.
        t.world.set_target(OWNER, Some(hunter));
        seconds(&mut t, 12);
        assert!(mob_hp(&t, hunter) < MOB_HP, "the companion did not attack");
    }

    // ---- the data ----

    #[test]
    fn the_templates_exist_for_every_role_and_the_gatekeepers_link_to_the_hire() {
        let Some(root) = datapack() else { return };
        let npcs = l2_data::NpcTemplates::load(root, &l2_data::NpcLoadOptions::default());
        for role in Role::ALL {
            let template = npcs
                .get(role.npc_id())
                .unwrap_or_else(|| panic!("{}", role.key()));
            assert_eq!(template.npc_type, "Companion", "{}", role.key());
            assert_eq!(i32::from(template.level), scale::REFERENCE_LEVEL);
            assert!(
                !template.is_aggressive,
                "{}: the script decides whom to fight",
                role.key()
            );
        }
        // Every town gatekeeper's page has the link.
        let town = std::fs::read_dir(root.join("data/teleporters/town")).unwrap();
        let mut linked = 0;
        for entry in town.flatten() {
            let id = entry.path().file_stem().unwrap().to_string_lossy().into_owned();
            let page = std::fs::read_to_string(root.join(format!("data/html/teleporter/{id}.htm"))).unwrap();
            assert!(
                page.contains("bypass -h Script Companions main"),
                "gatekeeper {id}"
            );
            linked += 1;
        }
        assert_eq!(linked, 21);
    }

    #[test]
    fn the_heal_skills_are_in_the_datapack_at_the_magic_levels_the_ladder_says() {
        let Some(root) = datapack() else { return };
        let skills = l2_data::SkillTemplates::load(root, &l2_data::SkillLoadOptions::default());
        let mut count = 0;
        for step in heal::all_steps() {
            let skill = skills
                .get(step.skill_id, step.level)
                .unwrap_or_else(|| panic!("{} level {}", step.skill_id, step.level));
            assert_eq!(
                skill.magic_level, step.magic_level,
                "{} level {}",
                step.skill_id, step.level
            );
            count += 1;
        }
        assert_eq!(count, 18 + 33);
    }

    #[test]
    fn the_spells_are_in_the_datapack_at_the_magic_levels_the_ladder_says() {
        let Some(root) = datapack() else { return };
        let skills = l2_data::SkillTemplates::load(root, &l2_data::SkillLoadOptions::default());
        let mut count = 0;
        for step in spell::all_steps() {
            let skill = skills
                .get(step.skill_id, step.level)
                .unwrap_or_else(|| panic!("{} level {}", step.skill_id, step.level));
            assert_eq!(
                skill.magic_level, step.magic_level,
                "{} level {}",
                step.skill_id, step.level
            );
            count += 1;
        }
        assert_eq!(count, 4 + 28);
    }
}
