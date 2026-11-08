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
mod class_skills;
mod classes;
mod gear_data;
mod heal;
mod look;
mod pages;
mod role;
mod scale;
mod skills;
mod spell;

use std::collections::{BTreeMap, BTreeSet};

use l2_util::ObjectId;
use l2_world::custom::CompanionCombatMode;
use l2_world::{
    CompanionSetup, Intention, PartyId, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, ZoneId,
};

use self::brain::{Action, Enemy, Memory, Pos, Unit, View, think};
use self::classes::{CompanionClass, choices};
use self::heal::heal_for;
use self::pages::{MainPage, Mode, main_page};
use self::role::Role;
use self::scale::{Multipliers, scale_for};
use self::skills::Kind;
use self::spell::spell_for;
use super::class_buffer::{buff_target, grouped};
use super::class_buffer_data::group_by_key;

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
const SLOTS: [(i32, i32); 8] = [
    (80, 60),
    (-80, 60),
    (0, -90),
    (100, -40),
    (-100, -40),
    (0, 110),
    (60, -100),
    (-60, -100),
];

/// The price of one companion for a player of `level` (`CompanionFeePerLevel` of `L2R.ini`).
fn hire_fee(ctx: &ScriptCtx<'_>, level: i32) -> i64 {
    i64::from(level.max(0)) * ctx.world().l2r_settings().companions.fee_per_level
}

/// The most companions a player may have at once (`CompanionMaxCount` of `L2R.ini`).
fn max_companions(ctx: &ScriptCtx<'_>) -> usize {
    ctx.world().l2r_settings().companions.max_count.max(1)
}

/// A hired companion.
struct Member {
    npc: ScriptNpc,
    /// Its class; the role comes with it.
    class: &'static CompanionClass,
    role: Role,
    /// The level it was made for, to see when its owner has grown.
    level: i32,
    /// The item id of the weapon it holds: its attack.
    weapon: i32,
    /// Its owner's client knows it (its `CharInfo` went out with a world tick after the spawn).
    /// Only then may it join his party window: a client takes no party member it does not know
    /// yet, so a companion added in the same tick it was made never showed there.
    known: bool,
    memory: Memory,
    /// When each of its class skills may be used again (world time in milliseconds).
    ready_at: BTreeMap<i32, u64>,
    /// The buffs that did not take (a stronger one of the same kind is on): not tried again.
    buff_skip: BTreeSet<i32>,
    /// A summoner's servitor.
    pet: Option<Pet>,
    /// When a summoner may call its servitor again.
    summon_at: u64,
    /// The grade of gear it wears (it is dressed anew when the owner reaches the next one).
    grade: usize,
}

/// A summoner's servitor: a companion NPC shown with the servitor's model, that fights like a
/// warrior.
struct Pet {
    npc: ScriptNpc,
    memory: Memory,
}

/// The template of a servitor (`data/stats/npcs/50000-50099.xml`).
const NPC_SERVITOR: i32 = 50029;
/// How long a summoner waits to call its servitor again after it fell.
const RESUMMON_MS: u64 = 30_000;
/// The least time between two uses of one class skill: a skill with no reuse time is not cast
/// every second.
const MIN_SKILL_GAP_MS: u64 = 2_000;

/// What the owner's party window shows of a squad.
struct Window {
    /// His party when the window was built.
    party: Option<PartyId>,
    /// The companions in it, with the names they are shown with (a companion that is gone is
    /// deleted from the window by its name).
    shown: Vec<(ObjectId, String)>,
}

/// The companions of one player.
struct Squad {
    /// The party the owner was in when he hired the first one. The squad ends when he is not in it.
    party: Option<PartyId>,
    members: Vec<Member>,
    /// What the owner's party window shows now, if anything.
    window: Option<Window>,
    /// The owner was told that his party window has no room for all of them.
    told_full: bool,
    /// When the companions start a fight (`CompanionCombatMode` of `L2R.ini`, `.companion`).
    mode: CompanionCombatMode,
    /// What was last saved for the owner's next login (written again only when it changes).
    saved: String,
    /// When a healer may offer the dead owner a resurrection again.
    revive_at: u64,
}

/// The player variable that keeps the companions over a logout.
const SAVE_VARIABLE: &str = "Companions";
/// How long a healer waits before it offers a resurrection again.
const REVIVE_AGAIN_MS: u64 = 30_000;

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

/// The monsters around `owner` that are alive. None in a peace zone: nobody may fight there.
fn enemies_near(ctx: &ScriptCtx<'_>, owner: ObjectId) -> Vec<Enemy> {
    let world = ctx.world();
    if world.is_inside_zone(owner, ZoneId::Peace) {
        return Vec::new();
    }
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
                // A hit, a shot or a skill of the owner puts him on the monster's list.
                hit_by_leader: world.aggro_list(id).iter().any(|aggro| aggro.attacker == owner),
            })
        })
        .collect()
}

/// The factors of `L2R.ini` on the companions' values.
fn multipliers(ctx: &ScriptCtx<'_>) -> Multipliers {
    let settings = &ctx.world().l2r_settings().companions;
    Multipliers {
        p_atk: settings.p_atk_multiplier,
        m_atk: settings.m_atk_multiplier,
        hp: settings.hp_multiplier,
    }
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
        if let Some(pet) = member.pet {
            ctx.delete_npc(pet.npc);
        }
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
fn hire(ctx: &mut ScriptCtx<'_>, owner: ObjectId, class: &'static CompanionClass) -> Result<(), String> {
    let role = class.role;
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
    let max = max_companions(ctx);
    if count >= max {
        return Err(format!("You can have {max} companions at most."));
    }
    // A party window holds nine; a companion with no place in it would fight unseen.
    if ctx.world().l2r_settings().companions.auto_join_party
        && ctx.world().companion_window_room(owner) <= count
    {
        return Err("Your party is full: there is no room for a companion in your party window.".to_owned());
    }
    let fee = hire_fee(ctx, level);
    if ctx.adena_of(owner) < fee {
        return Err(format!("A companion costs {} Adena.", grouped(fee)));
    }
    // Only the classes of the owner's own class tier (1st from 20, 2nd from 40, 3rd from 76).
    if !choices(level).iter().any(|c| c.id == class.id) {
        return Err(format!("You cannot hire a {} at level {level}.", class.name));
    }
    let look = look::roll(class, level, &mut |bound| ctx.get_random(bound));
    let member = spawn_member(ctx, owner, class, look, count)
        .ok_or_else(|| "The companion cannot come to this place.".to_owned())?;
    // The fee is checked above; if the purchase still fails the NPC goes again.
    if !ctx.world_mut().reduce_adena(owner, fee, true) {
        ctx.delete_npc(member.npc);
        return Err(format!("A companion costs {} Adena.", grouped(fee)));
    }
    let _ = role;
    join_squad(ctx, owner, member);
    Ok(())
}

/// A companion of `class` with `look` next to `owner` (in the `slot`-th place around him): made
/// for him and at his strength from its first moment, and buffed. `None`: it cannot be made here.
fn spawn_member(
    ctx: &mut ScriptCtx<'_>,
    owner: ObjectId,
    class: &'static CompanionClass,
    look: l2_world::CompanionLook,
    slot: usize,
) -> Option<Member> {
    let level = ctx.player_level(owner);
    let (x, y, z) = ctx.world().position_of(owner)?;
    let (dx, dy) = SLOTS[slot % SLOTS.len()];
    let (heading, instance) = (ctx.heading_of(owner), ctx.world().instance_id_of(owner));
    // The figure, the strength and the owner are born with the NPC: nobody sees it as anything
    // else first, and its kills pay the owner from its first blow.
    let weapon = look.right_hand;
    let setup = CompanionSetup {
        owner,
        scale: Some(scale_for(level, weapon, multipliers(ctx))),
        look: Some(look),
        display: None,
    };
    let role = class.role;
    let object_id =
        ctx.world_mut()
            .spawn_companion(role.npc_id(), (x + dx, y + dy, z), heading, instance, setup)?;
    let member = Member {
        npc: ScriptNpc {
            object_id,
            id: role.npc_id(),
        },
        class,
        role,
        level,
        weapon,
        known: false,
        memory: Memory::default(),
        ready_at: BTreeMap::new(),
        buff_skip: BTreeSet::new(),
        pet: None,
        summon_at: 0,
        grade: look::grade_for(level),
    };
    buff_itself(ctx, &member);
    Some(member)
}

/// Puts a new member into the owner's squad (a new squad when he had none), shows it in his
/// party window, saves the squad for his next login and starts the brain.
fn join_squad(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: Member) {
    let party = ctx.world().party_of(owner);
    let mode = ctx.world().l2r_settings().companions.combat_mode;
    let state = ctx.state::<State>();
    let mut squad = state.squads.remove(&owner).unwrap_or(Squad {
        party,
        members: Vec::new(),
        window: None,
        told_full: false,
        mode,
        saved: String::new(),
        revive_at: 0,
    });
    squad.members.push(member);
    // He sees the new companion in his party window at once.
    show_in_party_window(ctx, owner, &mut squad);
    save_squad(ctx, owner, &mut squad);
    ctx.state::<State>().squads.insert(owner, squad);
    ctx.start_quest_timer(TIMER, TICK_MS, None, Some(owner), true);
}

/// What is kept of a squad over a logout: the mode, then one figure per companion.
fn squad_text(ctx: &ScriptCtx<'_>, squad: &Squad) -> String {
    let mode = match squad.mode {
        CompanionCombatMode::Aggressive => "aggressive",
        CompanionCombatMode::Passive => "passive",
    };
    let figures = squad.members.iter().filter_map(|member| {
        let npc = ctx.world().npc(member.npc.object_id)?;
        Some(look::Saved::of(npc.look.as_ref()?).to_text())
    });
    std::iter::once(mode.to_owned())
        .chain(figures)
        .collect::<Vec<_>>()
        .join("|")
}

/// Writes the squad into the owner's player variable when it changed (an empty squad clears it).
fn save_squad(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: &mut Squad) {
    let text = if squad.members.is_empty() {
        String::new()
    } else {
        squad_text(ctx, squad)
    };
    if text != squad.saved {
        ctx.set_player_variable(owner, SAVE_VARIABLE, &text);
        squad.saved = text;
    }
}

/// At login: the companions he had come back, free, at his level now.
fn restore_squad(ctx: &mut ScriptCtx<'_>, owner: ObjectId) {
    let Some(text) = ctx
        .player_variable(owner, SAVE_VARIABLE)
        .filter(|t| !t.is_empty())
    else {
        return;
    };
    if ctx.state::<State>().squads.contains_key(&owner) || ctx.player_level(owner) < MIN_OWNER_LEVEL {
        return;
    }
    let mut parts = text.split('|');
    let mode = match parts.next() {
        Some("aggressive") => CompanionCombatMode::Aggressive,
        _ => CompanionCombatMode::Passive,
    };
    let level = ctx.player_level(owner);
    let max = max_companions(ctx);
    for (slot, saved) in parts.filter_map(look::Saved::from_text).take(max).enumerate() {
        let Some(class) = classes::by_id(saved.class_id) else {
            continue;
        };
        let look = look::restore(&saved, class, level, &mut |bound| ctx.get_random(bound));
        if let Some(member) = spawn_member(ctx, owner, class, look, slot) {
            join_squad(ctx, owner, member);
        }
    }
    if let Some(squad) = ctx.state::<State>().squads.get_mut(&owner) {
        squad.mode = mode;
    }
}

/// A healer of the squad offers the dead owner a resurrection (the dialog a player's
/// resurrection brings), with the animation of its skill; again after a while if he says no.
fn offer_resurrection(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: &mut Squad, now_ms: u64) {
    if now_ms < squad.revive_at {
        return;
    }
    for member in &squad.members {
        let npc = member.npc;
        if ctx.world().is_casting_now(npc.object_id) || ctx.is_dead(npc.object_id) {
            continue;
        }
        let Some(skill) = skills::of_kind(member.class.id, member.level, Kind::Resurrect)
            .into_iter()
            .next()
        else {
            continue;
        };
        let Some(info) = ctx.world().companion_skill(SkillKey::new(skill.id, skill.level)) else {
            continue;
        };
        ctx.broadcast_magic_skill_use(
            npc.object_id,
            npc.object_id,
            owner,
            skill.id,
            skill.level,
            info.hit_time,
            0,
        );
        ctx.world_mut()
            .revive_request(owner, npc.object_id, false, info.power.round() as i32);
        squad.revive_at = now_ms + REVIVE_AGAIN_MS;
        return;
    }
}

/// The owner reached a new class tier or a new grade: the companion moves up its class line
/// (Monk, Tyrant, Grand Khavatari) and gets the gear of the new grade. `true` when its figure
/// changed.
fn move_up(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &mut Member, level: i32) -> bool {
    let tier = classes::tier_for(level);
    let mut class = member.class;
    if class.tier < tier
        && let Some(next) = classes::successor(class, tier, &mut |bound| ctx.get_random(bound))
    {
        class = next;
    }
    let grade = look::grade_for(level);
    if class.id == member.class.id && grade == member.grade {
        return false;
    }
    let id = member.npc.object_id;
    let Some(mut figure) = ctx.world().npc(id).and_then(|n| n.look.clone()) else {
        return false;
    };
    if class.id != member.class.id {
        ctx.send_message(
            owner,
            &format!("Your {} is now a {}.", member.class.name, class.name),
        );
        member.ready_at.clear();
        member.buff_skip.clear();
    }
    look::dress(&mut figure, class, level, &mut |bound| ctx.get_random(bound));
    member.class = class;
    member.grade = grade;
    member.weapon = figure.right_hand;
    ctx.world_mut().set_companion_look(id, figure);
    let scale = scale_for(level, member.weapon, multipliers(ctx));
    ctx.world_mut().set_npc_scale(id, scale);
    true
}

/// One second of an owner's squad.
fn tick(ctx: &mut ScriptCtx<'_>, owner: ObjectId) {
    let Some(mut squad) = ctx.state::<State>().squads.remove(&owner) else {
        ctx.cancel_quest_timer(TIMER, None, Some(owner));
        return;
    };
    // A dead owner keeps them: they guard him and come after him when he goes to town.
    let owner_here = ctx.world().player(owner).is_some();
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
        // Not in his party any more: they are gone for good, also at the next login.
        if owner_here {
            ctx.set_player_variable(owner, SAVE_VARIABLE, "");
        }
        dismiss(ctx, owner, squad);
        return;
    }
    // The fallen and the vanished are not replaced; their corpses decay on their own. Their
    // servitors go with them.
    let (alive, fallen): (Vec<Member>, Vec<Member>) = std::mem::take(&mut squad.members)
        .into_iter()
        .partition(|member| {
            let id = member.npc.object_id;
            ctx.world().npc(id).is_some_and(|npc| !npc.decayed) && !ctx.is_dead(id)
        });
    for pet in fallen.into_iter().filter_map(|m| m.pet) {
        ctx.delete_npc(pet.npc);
    }
    squad.members = alive;
    if squad.members.is_empty() {
        if squad.window.is_some() {
            ctx.world().clear_companion_window(owner);
        }
        ctx.cancel_quest_timer(TIMER, None, Some(owner));
        return;
    }
    // A second has passed since the last of them was made: the world has shown each to the
    // players around it.
    for member in &mut squad.members {
        member.known = true;
    }
    run(ctx, owner, &mut squad);
    if ctx.is_dead(owner) {
        let now_ms = u64::try_from(ctx.world().world_ms()).unwrap_or(0);
        offer_resurrection(ctx, owner, &mut squad, now_ms);
    }
    show_in_party_window(ctx, owner, &mut squad);
    save_squad(ctx, owner, &mut squad);
    ctx.state::<State>().squads.insert(owner, squad);
}

/// The companions in the party window of their owner, the way a real party is shown: the window
/// is sent whole when it opens, when his party changes and after a teleport; a companion that
/// comes or goes is added or deleted on its own; every second the HP and MP of each.
fn show_in_party_window(ctx: &mut ScriptCtx<'_>, owner: ObjectId, squad: &mut Squad) {
    let world = ctx.world();
    let wanted: Vec<ObjectId> = if world.l2r_settings().companions.auto_join_party {
        let room = world.companion_window_room(owner);
        let all: Vec<ObjectId> = squad
            .members
            .iter()
            .filter(|m| m.known)
            .map(|m| m.npc.object_id)
            .collect();
        let shown: Vec<ObjectId> = all.iter().copied().take(room).collect();
        if shown.len() < all.len() {
            if !squad.told_full {
                ctx.send_message(
                    owner,
                    &format!(
                        "Your party is full: {} of your companions are not shown in the party window.",
                        all.len() - shown.len()
                    ),
                );
                squad.told_full = true;
            }
        } else {
            squad.told_full = false;
        }
        shown
    } else {
        Vec::new()
    };
    let world = ctx.world();
    if wanted.is_empty() {
        if squad.window.take().is_some() {
            world.clear_companion_window(owner);
        }
        return;
    }
    let party = world.party_of(owner);
    let name = |id: ObjectId| world.companion_name(id).unwrap_or_default();
    match &mut squad.window {
        Some(window) if window.party == party => {
            for (id, name) in window.shown.iter().filter(|(id, _)| !wanted.contains(id)) {
                world.remove_from_companion_window(owner, *id, name);
            }
            window.shown.retain(|(id, _)| wanted.contains(id));
            for id in &wanted {
                if !window.shown.iter().any(|(shown, _)| shown == id) {
                    world.add_to_companion_window(owner, *id);
                    window.shown.push((*id, name(*id)));
                }
            }
            for (id, _) in &window.shown {
                world.update_companion_window(owner, *id);
            }
        }
        _ => {
            world.send_companion_window(owner, &wanted);
            squad.window = Some(Window {
                party,
                shown: wanted.iter().map(|id| (*id, name(*id))).collect(),
            });
        }
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
    let mut moved = false;
    let mut redress = false;
    for member in &mut squad.members {
        let id = member.npc.object_id;
        if member.level != level {
            let scale = scale_for(level, member.weapon, multipliers(ctx));
            ctx.world_mut().set_npc_scale(id, scale);
            member.level = level;
            member.buff_skip.clear();
            // A new class shows a new class in the party window: it is built anew.
            if move_up(ctx, owner, member, level) {
                redress = true;
            }
        }
        moved |= catch_up(ctx, owner, member);
        if !is_buffed(ctx, member) {
            buff_itself(ctx, member);
        }
        if member.role == Role::Summoner {
            keep_pet(ctx, owner, member, level, now_ms);
        }
        for unit in [Some(id), member.pet.as_ref().map(|p| p.npc.object_id)]
            .into_iter()
            .flatten()
        {
            if let Some(pos) = pos_of(ctx, unit) {
                allies.push(Unit {
                    id: unit,
                    pos,
                    hp: npc_hp(ctx, unit),
                });
            }
        }
    }
    // The owner went far (a teleport, most likely), or a companion changed its class: his
    // client builds the window anew.
    if moved || redress {
        squad.window = None;
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
            aggressive: squad.mode == CompanionCombatMode::Aggressive,
            allies: &allies,
            enemies: &enemies,
        };
        let action = think(member.role.behavior(), &view, &mut member.memory);
        act(ctx, owner, member, action, now_ms);
        if member.role == Role::Buffer && member.memory.mode == brain::Mode::Follow {
            buff_the_party(ctx, owner, member, now_ms);
        }
        // The servitor fights beside its summoner, on the same picture.
        if let Some(pet) = member.pet.as_mut()
            && let Some(me) = allies.iter().find(|a| a.id == pet.npc.object_id).copied()
        {
            let view = View { me, mp: 1.0, ..view };
            match think(&PET, &view, &mut pet.memory) {
                Action::Attack(target) => attack(ctx, pet.npc, target),
                Action::Follow => follow(ctx, owner, pet.npc),
                _ => {}
            }
        }
    }
}

/// A servitor fights like a warrior.
static PET: brain::DamageDealer = brain::DamageDealer { ranged: false };

/// A summoner without its servitor calls it: the strongest summon skill it knows gives the model;
/// its strength is a warrior's of the owner's level.
fn keep_pet(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &mut Member, level: i32, now_ms: u64) {
    if let Some(pet) = &member.pet {
        let gone =
            ctx.world().npc(pet.npc.object_id).is_none_or(|n| n.decayed) || ctx.is_dead(pet.npc.object_id);
        if !gone {
            return;
        }
        if let Some(pet) = member.pet.take() {
            ctx.delete_npc(pet.npc);
        }
        member.summon_at = now_ms + RESUMMON_MS;
    }
    if now_ms < member.summon_at {
        return;
    }
    let Some(summon) = skills::of_kind(member.class.id, member.level, Kind::Summon)
        .into_iter()
        .next()
    else {
        return;
    };
    let Some(model) = skills::summon_npc(summon.id, summon.level) else {
        return;
    };
    let Some((x, y, z)) = ctx.world().position_of(member.npc.object_id) else {
        return;
    };
    let weapon = scale::pet_weapon(level);
    let setup = CompanionSetup {
        owner,
        scale: Some(scale_for(level, weapon, multipliers(ctx))),
        look: None,
        display: Some(model),
    };
    let (heading, instance) = (ctx.heading_of(owner), ctx.world().instance_id_of(owner));
    if let Some(object_id) =
        ctx.world_mut()
            .spawn_companion(NPC_SERVITOR, (x + 40, y + 40, z), heading, instance, setup)
    {
        member.pet = Some(Pet {
            npc: ScriptNpc {
                object_id,
                id: NPC_SERVITOR,
            },
            memory: Memory::default(),
        });
    }
    member.summon_at = now_ms + RESUMMON_MS;
}

/// A buffer at peace gives the owner, his party around him and the companions the buffs of its
/// class they lack, one buff a second.
fn buff_the_party(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &mut Member, now_ms: u64) {
    let npc = member.npc;
    if ctx.world().is_casting_now(npc.object_id) {
        return;
    }
    let mut targets = vec![owner];
    let near = |ctx: &ScriptCtx<'_>, id: ObjectId| matches!((pos_of(ctx, id), pos_of(ctx, npc.object_id)), (Some(a), Some(b)) if a.distance(b) <= 900.0);
    for friend in ctx.world().party_members_of(owner) {
        if friend != owner && !ctx.is_dead(friend) && near(ctx, friend) {
            targets.push(friend);
        }
    }
    targets.push(npc.object_id);
    // The owner first, then his party, then the companions.
    let buffs = skills::of_kind(member.class.id, member.level, Kind::Buff);
    for target in targets {
        for buff in &buffs {
            if member.buff_skip.contains(&buff.id)
                || member.ready_at.get(&buff.id).is_some_and(|at| *at > now_ms)
                || ctx.world().script_buff_skill_level(target, buff.id).is_some()
            {
                continue;
            }
            let key = SkillKey::new(buff.id, buff.level);
            // The buff lands at once (as the class buffer gives it) and is shown as a short cast:
            // a real cast takes seconds per buff, and a buffer of level 45 knows about twenty.
            ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, target, buff.id, buff.level, 500, 0);
            ctx.world_mut().apply_effects(npc.object_id, target, key);
            if ctx.world().script_buff_skill_level(target, buff.id).is_none() {
                // A stronger buff of the same kind is on: this one is not tried again.
                member.buff_skip.insert(buff.id);
            }
            return;
        }
    }
}

/// Casts the strongest ready class skill of `kind` at `target`. `false`: none was ready (or the
/// companion lacks the MP); `true` also while it is casting already.
fn use_skill(
    ctx: &mut ScriptCtx<'_>,
    member: &mut Member,
    kind: Kind,
    target: ObjectId,
    now_ms: u64,
) -> bool {
    let npc = member.npc;
    if ctx.world().is_casting_now(npc.object_id) {
        return true;
    }
    let mp = ctx.world().npc(npc.object_id).map_or(0.0, |n| n.core.cur_mp);
    for skill in skills::of_kind(member.class.id, member.level, kind) {
        if member.ready_at.get(&skill.id).is_some_and(|at| *at > now_ms) {
            continue;
        }
        let key = SkillKey::new(skill.id, skill.level);
        let Some(info) = ctx.world().companion_skill(key) else {
            continue;
        };
        if f64::from(info.mp) > mp {
            continue;
        }
        // The intention, not a plain cast: the companion walks into range first.
        ctx.world_mut()
            .set_intention_cast(npc.object_id, key, Some(target));
        let reuse = u64::try_from(info.reuse_ms).unwrap_or(0).max(MIN_SKILL_GAP_MS);
        member.ready_at.insert(skill.id, now_ms + reuse);
        return true;
    }
    false
}

/// A companion that is far away or in another instance is put next to its owner. `true` when it
/// was moved.
fn catch_up(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &Member) -> bool {
    let id = member.npc.object_id;
    let (Some(here), Some(there)) = (pos_of(ctx, id), pos_of(ctx, owner)) else {
        return false;
    };
    let same_instance = ctx.world().instance_id_of(id) == ctx.world().instance_id_of(owner);
    if same_instance && here.distance(there) <= brain::CATCH_UP_DISTANCE {
        return false;
    }
    let Some((x, y, z)) = ctx.world().position_of(owner) else {
        return false;
    };
    let slot = SLOTS[usize::try_from(id.0).unwrap_or(0) % SLOTS.len()];
    let target = l2_world::TeleportTarget::at(x + slot.0, y + slot.1, z)
        .with_instance(ctx.world().instance_id_of(owner));
    ctx.world_mut().tele_to_location(id, target);
    true
}

/// `.companion aggressive` / `.companion passive` (event `mode:<word>`): when the companions of
/// `owner` start a fight. What the player is told.
fn set_mode(ctx: &mut ScriptCtx<'_>, owner: ObjectId, word: &str) -> String {
    let wanted = match word.trim().to_ascii_lowercase().as_str() {
        "aggressive" => Some(CompanionCombatMode::Aggressive),
        "passive" => Some(CompanionCombatMode::Passive),
        _ => None,
    };
    let default = ctx.world().l2r_settings().companions.combat_mode;
    let Some(squad) = ctx.state::<State>().squads.get_mut(&owner) else {
        return "You have no companions. Hire them at a gatekeeper.".to_owned();
    };
    if let Some(mode) = wanted {
        squad.mode = mode;
    }
    let now = squad.mode;
    let text = match now {
        CompanionCombatMode::Aggressive => "aggressive: they attack what you target",
        CompanionCombatMode::Passive => "passive: they attack what you target once you attacked it",
    };
    match wanted {
        Some(_) => format!("Your companions are {text}."),
        None => format!(
            "Your companions are {text}. Use .companion aggressive or .companion passive (default: {}).",
            match default {
                CompanionCombatMode::Aggressive => "aggressive",
                CompanionCombatMode::Passive => "passive",
            }
        ),
    }
}

/// Carries out what a companion decided: with a skill of its class where it has one.
fn act(ctx: &mut ScriptCtx<'_>, owner: ObjectId, member: &mut Member, action: Action, now_ms: u64) {
    let npc = member.npc;
    match action {
        Action::Hold => {}
        Action::Follow => follow(ctx, owner, npc),
        Action::Attack(target) => {
            if !use_skill(ctx, member, Kind::Strike, target, now_ms) {
                attack(ctx, npc, target);
            }
        }
        Action::Taunt(target) => {
            // The monster now hates the companion more than anybody.
            ctx.world_mut()
                .add_damage_hate(target, npc.object_id, 0, TAUNT_HATE);
            if !use_skill(ctx, member, Kind::Taunt, target, now_ms) {
                attack(ctx, npc, target);
            }
        }
        Action::Heal(ally) => {
            if !use_skill(ctx, member, Kind::Heal, ally, now_ms)
                && let Some(step) = heal_for(member.level)
                && !ctx.world().is_casting_now(npc.object_id)
            {
                ctx.cast_skill_at(npc, Some(ally), SkillKey::new(step.skill_id, step.level));
            }
        }
        Action::Cast(target) if use_skill(ctx, member, Kind::Nuke, target, now_ms) => {}
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

/// The class a hire link names: a class id, or a role (`hire:tank`: the first class of that role
/// in the owner's tier).
fn class_of_key(level: i32, key: &str) -> Option<&'static CompanionClass> {
    match key.parse::<i32>() {
        Ok(id) => classes::by_id(id),
        Err(_) => {
            let role = Role::from_key(key)?;
            // Below level 20 the hire itself says why it cannot be.
            choices(level.max(MIN_OWNER_LEVEL))
                .into_iter()
                .find(|c| c.role == role)
        }
    }
}

/// The main page for `owner`, with `note` on top.
fn page(ctx: &mut ScriptCtx<'_>, owner: ObjectId, mode: Mode, note: Option<&str>) -> String {
    let level = ctx.player_level(owner);
    let squad: Vec<&'static str> = ctx
        .state::<State>()
        .squads
        .get(&owner)
        .map(|s| s.members.iter().map(|m| m.class.name).collect())
        .unwrap_or_default();
    let choices = choices(level);
    main_page(&MainPage {
        mode,
        fee: hire_fee(ctx, level),
        fee_per_level: ctx.world().l2r_settings().companions.fee_per_level,
        choices: &choices,
        squad: &squad,
        max: max_companions(ctx),
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
        s.set_on_enter_world();
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
        // `p-...` comes from `.companion` anywhere: its page links back to the command.
        let (mode, click) = match event.strip_prefix("p-") {
            Some(click) => (Mode::Command, click),
            None => (Mode::Npc, event),
        };
        let (command, key) = click.split_once([':', ' ']).unwrap_or((click, ""));
        let key = key.trim();
        if matches!(command, "aggressive" | "passive" | "mode") {
            let word = if command == "mode" { key } else { command };
            let answer = set_mode(ctx, owner, word);
            ctx.send_message(owner, &answer);
            return None;
        }
        let note = match command {
            "hire" => match class_of_key(ctx.player_level(owner), key) {
                Some(class) => match hire(ctx, owner, class) {
                    Ok(()) => Some(format!("Your {} joins you.", class.name)),
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
                        if let Some(pet) = member.pet {
                            ctx.delete_npc(pet.npc);
                        }
                        ctx.delete_npc(member.npc);
                        if ctx
                            .state::<State>()
                            .squads
                            .get(&owner)
                            .is_some_and(|s| s.members.is_empty())
                        {
                            dismiss_all(ctx, owner);
                        }
                        Some(format!("The {} leaves.", member.class.name))
                    }
                    None => None,
                }
            }
            "dismissall" => {
                let had = dismiss_all(ctx, owner);
                if had {
                    ctx.set_player_variable(owner, SAVE_VARIABLE, "");
                }
                had.then(|| "Your companions leave.".to_owned())
            }
            _ => None,
        };
        Some(page(ctx, owner, mode, note.as_deref()))
    }

    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        // What they are is saved (it is, with every change); they leave with him and come back
        // at his next login.
        if let Some(mut squad) = ctx.state::<State>().squads.remove(&player) {
            save_squad(ctx, player, &mut squad);
            dismiss(ctx, player, squad);
        }
    }

    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        restore_squad(ctx, player);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use l2_world::script::{player_class, race};
    use l2_world::{ClientInput, ScriptRegistry};

    use super::role::{NPC_ARCHER, NPC_BUFFER, NPC_DAGGER, NPC_HEALER, NPC_MAGE, NPC_TANK, NPC_WARRIOR};
    use super::*;
    use crate::custom::class_buffer_data::GROUPS;
    use l2_protocol_game::server::{
        PartySmallWindowAdd, PartySmallWindowAll, PartySmallWindowDelete, PartySmallWindowUpdate,
    };

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
        // The class skills of every companion class.
        ids.extend(
            class_skills::CLASS_SKILLS
                .iter()
                .flat_map(|(_, skills)| skills.iter().map(|(id, _, _)| *id)),
        );
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
        // Adena, and the weapons the companions hold (their attack type is their weapon's).
        let weapons: BTreeSet<i32> = gear_data::WEAPONS.iter().map(|(_, _, id)| *id).collect();
        let items = format!("{ADENA_ITEM}{}", crate::testing::item_xml(root, &weapons));
        t.set_skills(&items, &skill_xml(root, &needed_skills()));
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
        let event = by_class(t, event);
        t.event(1, "Companions", &event)
    }

    /// `hire:tank` as the page has it: the first tank class of the owner's tier (`hire:4`).
    fn by_class(t: &ScriptTest, event: &str) -> String {
        match event.strip_prefix("hire:") {
            Some(key) if key.parse::<i32>().is_err() => {
                let level = t.world.player(OWNER).unwrap().core.level;
                let class = class_of_key(level, key).unwrap_or_else(|| panic!("no {key} at level {level}"));
                format!("hire:{}", class.id)
            }
            _ => event.to_owned(),
        }
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

    /// The owner types `text` in the chat.
    fn say(t: &mut ScriptTest, text: &str) {
        t.input(
            1,
            ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        );
    }

    /// The owner attacks `mob` (he is on its hate list): a passive companion joins in now.
    fn owner_hits(t: &mut ScriptTest, mob: ObjectId) {
        t.world.add_damage_hate(mob, OWNER, 1, 1);
    }

    /// The texts of the system messages the owner got (UTF-16 in the packet: both alignments).
    fn texts_to_owner(sent: &Sent) -> Vec<String> {
        let utf16 = |bytes: &[u8]| {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        };
        sent.iter()
            .filter(|(to, body)| *to == 1 && name_of(body) == "SYSTEM_MESSAGE")
            .map(|(_, body)| format!("{} {}", utf16(body), utf16(&body[1..])))
            .collect()
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
                let class = classes::by_id(info.class_id).expect("a class of the catalog");
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
                    gear_data::WEAPONS.contains(&(class.weapon.key(), grade, worn(paperdoll::RHAND))),
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
                    class.weapon.with_shield(),
                    "{role:?}: the shield"
                );
            }
        }
    }

    #[test]
    fn a_companion_is_its_class_and_the_names_and_sexes_vary() {
        let Some(root) = datapack() else { return };
        let mut names = BTreeSet::new();
        let mut sexes = BTreeSet::new();
        let mut t = scene(root, 60, 100_000_000);
        for class in choices(60) {
            let page = click(&mut t, &format!("hire:{}", class.id));
            assert!(
                page.contains(&format!("Your {} joins you.", class.name)),
                "{}: {page}",
                class.name
            );
            let id = only(&t, class.role.npc_id());
            let info = t.world.fake_player_info(id).unwrap();
            assert_eq!(
                (info.class_id, info.race),
                (class.id, class.race),
                "{}",
                class.name
            );
            names.insert(info.name.clone());
            sexes.insert(info.is_female);
            click(&mut t, "dismissall");
        }
        assert!(names.len() >= 8, "{names:?}");
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
        let event = by_class(t, event);
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
    fn every_further_companion_is_added_to_the_open_window_after_the_client_knows_it() {
        use l2_protocol_game::server::CharInfo;

        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        let mut hired = Vec::new();
        for (key, npc) in [("tank", NPC_TANK), ("healer", NPC_HEALER), ("mage", NPC_MAGE)] {
            let _ = t.take();
            press(&mut t, &format!("hire:{key}"));
            let id = only(&t, npc);
            hired.push(id.0);
            seconds(&mut t, 2);
            let sent: Vec<(String, Vec<u8>)> = t
                .take()
                .into_iter()
                .filter(|(to, _)| *to == 1)
                .map(|(_, body)| (name_of(&body), body))
                .collect();
            let names: Vec<&str> = sent.iter().map(|(n, _)| n.as_str()).collect();
            // The client is told who it is first: a party member it does not know is dropped.
            let shown = sent
                .iter()
                .position(|(n, body)| n == "CHAR_INFO" && decode::<CharInfo>(body).object_id == id.0)
                .unwrap_or_else(|| panic!("{key}: never shown: {names:?}"));
            let joined = if hired.len() == 1 {
                // The window opens: emptied, then whole, as for a player who joins a party.
                let all = names
                    .iter()
                    .position(|n| *n == "PARTY_SMALL_WINDOW_ALL")
                    .unwrap_or_else(|| panic!("{key}: {names:?}"));
                assert_eq!(names[all - 1], "PARTY_SMALL_WINDOW_DELETE_ALL");
                let window = decode::<PartySmallWindowAll>(&sent[all].1);
                let ids: Vec<i32> = window.members.iter().map(|m| m.vitals.object_id).collect();
                assert_eq!(ids, hired);
                all
            } else {
                // Later ones join it as a member who joins a party is shown to the others.
                assert!(!names.contains(&"PARTY_SMALL_WINDOW_ALL"), "{key}: {names:?}");
                let adds: Vec<(usize, PartySmallWindowAdd)> = sent
                    .iter()
                    .enumerate()
                    .filter(|(_, (n, _))| n == "PARTY_SMALL_WINDOW_ADD")
                    .map(|(i, (_, body))| (i, decode::<PartySmallWindowAdd>(body)))
                    .collect();
                assert_eq!(adds.len(), 1, "{key}: {names:?}");
                let (at, add) = &adds[0];
                assert_eq!(add.member.object_id, id.0);
                assert_eq!(add.leader_object_id, OWNER.0);
                assert_eq!(add.member.name, t.world.fake_player_info(id).unwrap().name);
                *at
            };
            assert!(
                shown < joined,
                "{key}: CharInfo at {shown}, the window at {joined}: {names:?}"
            );
        }
    }

    #[test]
    fn a_full_party_refuses_a_companion_and_says_so() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        // Eight friends and the owner: nine, the most a party window holds.
        for friend in 2..=9 {
            t.enter_as(friend, 40, race::HUMAN, player_class::DUELIST, vec![]);
            t.party(1, friend);
        }
        assert_eq!(t.world.party_members_of(OWNER).len(), 9);
        let adena = t.count(1, 57);
        let answer = direct(&mut t, "hire:tank");
        assert!(answer.contains("Your party is full"), "{answer}");
        assert_eq!(
            (all_companions(&t), t.count(1, 57)),
            (0, adena),
            "nothing hired, nothing paid"
        );
    }

    #[test]
    fn companions_beyond_the_room_of_the_window_are_left_out_with_a_message() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        for friend in 2..=6 {
            t.enter_as(friend, 40, race::HUMAN, player_class::DUELIST, vec![]);
            t.party(1, friend);
        }
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        click(&mut t, "hire:warrior");
        assert_eq!(all_companions(&t), 3, "six players and three companions fit");
        seconds(&mut t, 1);
        // Two more friends: room for one companion.
        for friend in 7..=8 {
            t.enter_as(friend, 40, race::HUMAN, player_class::DUELIST, vec![]);
            t.party(1, friend);
        }
        let _ = t.take();
        seconds(&mut t, 2);
        let sent = t.take();
        assert!(
            texts_to_owner(&sent)
                .iter()
                .any(|s| s.contains("2 of your companions are not shown")),
            "{:?}",
            texts_to_owner(&sent)
        );
        // The last two hired leave the window; the tank keeps its place.
        let deleted: Vec<i32> = window_packets(&sent)
            .iter()
            .filter(|(n, _)| n == "PARTY_SMALL_WINDOW_DELETE")
            .map(|(_, body)| decode::<PartySmallWindowDelete>(body).object_id)
            .collect();
        assert_eq!(deleted, vec![only(&t, NPC_HEALER).0, only(&t, NPC_WARRIOR).0]);
        assert_eq!(all_companions(&t), 3, "they still follow and fight");
    }

    #[test]
    fn without_auto_join_the_window_stays_shut() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        let mut settings = t.world.l2r_settings().clone();
        settings.companions.auto_join_party = false;
        t.world.set_l2r_settings(settings);
        let _ = t.take();
        press(&mut t, "hire:tank");
        press(&mut t, "hire:healer");
        seconds(&mut t, 2);
        assert!(window_packets(&t.take()).is_empty());
        assert_eq!(all_companions(&t), 2, "they still come and fight");
    }

    #[test]
    fn after_a_teleport_the_window_is_built_anew() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        seconds(&mut t, 1);
        let _ = t.take();
        t.world.set_position(OWNER, X + 6_000, Y, Z);
        seconds(&mut t, 2);
        let window = last_window(&t.take()).expect("sent whole after the companions were brought over");
        assert_eq!(window.members.len(), 2);
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
        let tank_name = t.world.fake_player_info(tank).unwrap().name;
        seconds(&mut t, 1);
        let _ = t.take();
        t.world.do_die(tank, None);
        seconds(&mut t, 2);
        let packets = window_packets(&t.take());
        let deleted: Vec<PartySmallWindowDelete> = packets
            .iter()
            .filter(|(n, _)| n == "PARTY_SMALL_WINDOW_DELETE")
            .map(|(_, body)| decode::<PartySmallWindowDelete>(body))
            .collect();
        assert_eq!(deleted.len(), 1);
        assert_eq!((deleted[0].object_id, &deleted[0].name), (tank.0, &tank_name));
        assert!(!packets.iter().any(|(n, _)| n == "PARTY_SMALL_WINDOW_ALL"));

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
    fn the_window_is_closed_when_the_companions_leave_with_their_owner_out_of_the_party() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        t.enter(2, 40, vec![]);
        t.party(2, 1);
        click(&mut t, "hire:tank");
        seconds(&mut t, 1);
        let _ = t.take();
        t.input(1, ClientInput::WithdrawParty);
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
        assert!(page.contains("Your Paladin joins you."), "{page}");
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
    fn the_price_and_the_most_companions_come_from_l2r_ini() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        let mut settings = t.world.l2r_settings().clone();
        settings.companions.fee_per_level = 1_000;
        settings.companions.max_count = 5;
        t.world.set_l2r_settings(settings);
        let page = direct(&mut t, "main");
        assert!(
            page.contains("40,000 Adena") && page.contains("1,000 for every level"),
            "{page}"
        );
        assert!(page.contains("You can have 5,"), "{page}");
        for key in ["tank", "healer", "warrior", "archer", "mage"] {
            click(&mut t, &format!("hire:{key}"));
        }
        assert_eq!(all_companions(&t), 5);
        assert_eq!(t.count(1, 57), 10_000_000 - 5 * 40_000, "40 x 1,000 each");
        assert!(direct(&mut t, "hire:dagger").contains("5 companions at most"));
        assert_eq!(all_companions(&t), 5);
        // They stand apart, not on top of each other.
        let places: BTreeSet<(i32, i32)> = [NPC_TANK, NPC_HEALER, NPC_WARRIOR, NPC_ARCHER, NPC_MAGE]
            .iter()
            .map(|npc| {
                let (x, y, _) = t.world.position_of(only(&t, *npc)).unwrap();
                (x, y)
            })
            .collect();
        assert_eq!(places.len(), 5);
    }

    #[test]
    fn the_command_opens_the_page_anywhere_and_hires_from_it() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        let _ = t.take();
        say(&mut t, ".companion");
        t.ticks(2);
        let pages = crate::testing::pages_to(&t.take(), 1);
        let page = pages.last().expect("the page was shown");
        assert!(
            page.contains("bypass -h voice .companion hire 48\">Tyrant</a>"),
            "{page}"
        );
        assert!(!page.contains("Script Companions"));
        say(&mut t, ".companion hire 48");
        t.ticks(2);
        assert_eq!(t.npcs_of(NPC_WARRIOR).len(), 1);
        let info = t.world.fake_player_info(only(&t, NPC_WARRIOR)).unwrap();
        assert_eq!(info.class_id, 48, "a Tyrant");
        // A class of another tier is refused.
        say(&mut t, ".companion hire 114");
        t.ticks(2);
        let pages = crate::testing::pages_to(&t.take(), 1);
        assert!(
            pages
                .last()
                .unwrap()
                .contains("You cannot hire a Grand Khavatari at level 45.")
        );
        assert_eq!(all_companions(&t), 1);
    }

    /// The skills `caster` started, by id, in what the owner was sent.
    fn skills_cast(sent: &Sent, caster: ObjectId) -> Vec<i32> {
        use l2_protocol_game::server::MagicSkillUse;
        sent.iter()
            .filter(|(to, body)| *to == 1 && name_of(body) == "MAGIC_SKILL_USE")
            .map(|(_, body)| decode::<MagicSkillUse>(body))
            .filter(|m| m.creature_id == caster.0)
            .map(|m| m.skill_id)
            .collect()
    }

    /// The class skills of `class` of `kind` at `level`, by id.
    fn class_skill_ids(class: i32, level: i32, kind: Kind) -> Vec<i32> {
        skills::of_kind(class, level, kind).iter().map(|k| k.id).collect()
    }

    /// A companion of class `class` next to an owner of `level`, and a monster he has hit.
    fn fight_with(root: &Path, class: i32, level: i32) -> (ScriptTest, ObjectId, ObjectId) {
        let mut t = scene(root, level, 10_000_000);
        let page = click(&mut t, &format!("hire:{class}"));
        assert!(page.contains("joins you"), "{page}");
        let role = classes::by_id(class).unwrap().role;
        let id = only(&t, role.npc_id());
        let mob = t.spawn(MONSTER, 150);
        t.world.set_target(OWNER, Some(mob));
        owner_hits(&mut t, mob);
        t.world.set_intention_attack(mob, OWNER);
        let _ = t.take();
        (t, id, mob)
    }

    #[test]
    fn a_dagger_strikes_with_the_skills_of_its_class() {
        let Some(root) = datapack() else { return };
        let (mut t, rogue, mob) = fight_with(root, 7, 30);
        seconds(&mut t, 10);
        let cast = skills_cast(&t.take(), rogue);
        let strikes = class_skill_ids(7, 30, Kind::Strike);
        assert!(
            cast.iter().any(|id| strikes.contains(id)),
            "cast {cast:?}, its strikes {strikes:?}"
        );
        assert!(mob_hp(&t, mob) < MOB_HP);
    }

    #[test]
    fn a_monk_strikes_with_its_fists_although_its_template_holds_a_sword() {
        let Some(root) = datapack() else { return };
        // The warrior template's weapon is a sword; the Monk's skills want fists. The weapon of
        // the figure counts.
        let (mut t, monk, _) = fight_with(root, 47, 30);
        assert_eq!(
            t.world.stats(monk).unwrap().attack_type(),
            l2_data::item::WeaponType::DualFist,
            "it fights with the fists it is shown with"
        );
        seconds(&mut t, 10);
        let cast = skills_cast(&t.take(), monk);
        let strikes = class_skill_ids(47, 30, Kind::Strike);
        assert!(!strikes.is_empty());
        assert!(
            cast.iter().any(|id| strikes.contains(id)),
            "cast {cast:?}, its strikes {strikes:?}"
        );
    }

    #[test]
    fn a_mage_casts_the_spells_of_its_class() {
        let Some(root) = datapack() else { return };
        let (mut t, wizard, mob) = fight_with(root, 11, 30);
        seconds(&mut t, 12);
        let cast = skills_cast(&t.take(), wizard);
        let nukes = class_skill_ids(11, 30, Kind::Nuke);
        assert!(
            cast.iter().any(|id| nukes.contains(id)),
            "cast {cast:?}, its nukes {nukes:?}"
        );
        assert!(mob_hp(&t, mob) < MOB_HP);
    }

    #[test]
    fn a_knight_taunts_with_its_class_skill() {
        let Some(root) = datapack() else { return };
        let (mut t, knight, mob) = fight_with(root, 4, 35);
        seconds(&mut t, 8);
        let cast = skills_cast(&t.take(), knight);
        let taunts = class_skill_ids(4, 35, Kind::Taunt);
        assert!(!taunts.is_empty());
        assert!(
            cast.iter().any(|id| taunts.contains(id)),
            "cast {cast:?}, its taunts {taunts:?}"
        );
        assert_eq!(t.world.attack_target_of(mob), Some(knight), "the knight holds it");
    }

    #[test]
    fn a_buffer_gives_the_owner_the_buffs_of_its_class() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:17");
        let prophet = only(&t, NPC_BUFFER);
        let buffs = class_skill_ids(17, 45, Kind::Buff);
        assert!(buffs.contains(&1068), "a Prophet has Might: {buffs:?}");
        // One buff a second, the strongest first: about 20 of them at level 45.
        seconds(&mut t, 25);
        let on_owner: Vec<i32> = buffs
            .iter()
            .copied()
            .filter(|id| t.world.script_buff_skill_level(OWNER, *id).is_some())
            .collect();
        assert!(on_owner.len() >= 15, "the owner got {on_owner:?} of {buffs:?}");
        // Might, or Greater Might, which takes its place.
        assert!(
            [1068, 1388]
                .iter()
                .any(|id| t.world.script_buff_skill_level(OWNER, *id).is_some()),
            "Might"
        );
        // A fight stops the buffing (a monster on the prophet: the test owner has 1 HP).
        let _ = t.take();
        let mob = t.spawn(MONSTER, 150);
        t.world.add_damage_hate(mob, prophet, 1, 100);
        t.world.set_intention_attack(mob, prophet);
        seconds(&mut t, 1);
        let _ = t.take();
        seconds(&mut t, 3);
        let cast = skills_cast(&t.take(), prophet);
        assert!(
            cast.iter().all(|id| !buffs.contains(id)),
            "buffs in a fight: {cast:?}"
        );
        let _ = mob;
    }

    #[test]
    fn a_summoner_calls_its_servitor_which_fights_and_goes_with_it() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:14");
        seconds(&mut t, 2);
        let pets = t.npcs_of(NPC_SERVITOR);
        assert_eq!(pets.len(), 1, "the Warlock called its servitor");
        let pet = pets[0];
        let summon = skills::of_kind(14, 45, Kind::Summon)[0];
        let model = skills::summon_npc(summon.id, summon.level).unwrap();
        assert_eq!(
            t.world.npc(pet).unwrap().display,
            Some(model),
            "shown as the servitor"
        );
        assert!(
            t.world.npc_info(pet, OWNER).is_some(),
            "an NPC to the clients, not a player"
        );
        assert_eq!(
            t.world.companion_owner(pet),
            Some(OWNER),
            "its kills pay the owner"
        );
        // It is no member of the party window.
        let window = window_packets(&t.take());
        assert!(
            window
                .iter()
                .all(|(_, body)| !body.windows(4).any(|w| w == pet.0.to_le_bytes()))
        );
        // It fights.
        let mob = t.spawn(MONSTER, 150);
        t.world.set_target(OWNER, Some(mob));
        owner_hits(&mut t, mob);
        seconds(&mut t, 8);
        assert!(mob_hp(&t, mob) < MOB_HP);
        // It goes with its summoner.
        click(&mut t, "dismissall");
        assert!(t.npcs_of(NPC_SERVITOR).is_empty());
    }

    #[test]
    fn a_fallen_servitor_is_called_again_later() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:14");
        seconds(&mut t, 2);
        let pet = t.npcs_of(NPC_SERVITOR)[0];
        t.world.do_die(pet, None);
        seconds(&mut t, 5);
        assert!(
            t.npcs_of(NPC_SERVITOR)
                .iter()
                .all(|p| *p == pet || t.world.is_dead(*p)),
            "not at once"
        );
        seconds(&mut t, 30);
        let alive: Vec<ObjectId> = t
            .npcs_of(NPC_SERVITOR)
            .into_iter()
            .filter(|p| !t.world.is_dead(*p))
            .collect();
        assert_eq!(alive.len(), 1, "called again after the wait");
    }

    #[test]
    fn a_healer_heals_with_the_heals_of_its_class() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 50, 10_000_000);
        click(&mut t, "hire:5");
        click(&mut t, "hire:16");
        let (knight, bishop) = (only(&t, NPC_TANK), only(&t, NPC_HEALER));
        let max = f64::from(t.world.stats(knight).unwrap().max_hp());
        t.world.set_current_hp_mp(knight, max * 0.3, 100.0);
        let _ = t.take();
        seconds(&mut t, 8);
        let cast = skills_cast(&t.take(), bishop);
        let heals = class_skill_ids(16, 50, Kind::Heal);
        assert!(
            cast.iter().any(|id| heals.contains(id)),
            "cast {cast:?}, its heals {heals:?}"
        );
    }

    /// The owner's saved companions (the player variable).
    fn saved(t: &mut ScriptTest) -> String {
        let mut value = None;
        t.world.run_as_script("Companions", |ctx| {
            value = ctx.player_variable(OWNER, SAVE_VARIABLE);
        });
        value.unwrap_or_default()
    }

    #[test]
    fn the_companions_come_back_free_at_the_next_login() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:48");
        click(&mut t, "hire:16");
        say(&mut t, ".companion aggressive");
        seconds(&mut t, 1);
        let before: Vec<(i32, String)> = [NPC_WARRIOR, NPC_HEALER]
            .iter()
            .map(|npc| {
                let info = t.world.fake_player_info(only(&t, *npc)).unwrap();
                (info.class_id, info.name)
            })
            .collect();
        let text = saved(&mut t);
        assert!(
            text.starts_with("aggressive|48,") && text.contains("|16,"),
            "{text}"
        );
        t.input(1, ClientInput::Logout);
        seconds(&mut t, 2);
        assert_eq!(all_companions(&t), 0, "they leave with him");

        // He comes back (the variable is what the database keeps for him).
        t.enter_as(
            1,
            45,
            race::HUMAN,
            player_class::DUELIST,
            vec![item(100, 57, 1_000)],
        );
        t.world.run_as_script("Companions", |ctx| {
            ctx.set_player_variable(OWNER, SAVE_VARIABLE, &text);
            Companions.on_enter_world(ctx, OWNER);
        });
        seconds(&mut t, 1);
        let after: Vec<(i32, String)> = [NPC_WARRIOR, NPC_HEALER]
            .iter()
            .map(|npc| {
                let info = t.world.fake_player_info(only(&t, *npc)).unwrap();
                (info.class_id, info.name)
            })
            .collect();
        assert_eq!(after, before, "the same companions");
        assert_eq!(t.count(1, 57), 1_000, "free");
        let mut mode = None;
        t.world.run_as_script("Companions", |ctx| {
            mode = ctx.state::<State>().squads.get(&OWNER).map(|s| s.mode);
        });
        assert_eq!(mode, Some(CompanionCombatMode::Aggressive), "the mode comes back");
    }

    #[test]
    fn dismissing_all_forgets_them_for_the_next_login() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:48");
        assert!(!saved(&mut t).is_empty());
        click(&mut t, "dismissall");
        assert_eq!(saved(&mut t), "");
    }

    #[test]
    fn a_companion_moves_up_its_class_line_with_its_owner() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 39, 10_000_000);
        click(&mut t, "hire:47");
        let monk = only(&t, NPC_WARRIOR);
        let face = t.world.fake_player_info(monk).unwrap();
        assert_eq!(face.class_id, 47);
        let _ = t.take();
        t.world.player_mut(OWNER).unwrap().core.level = 40;
        seconds(&mut t, 2);
        let info = t.world.fake_player_info(monk).unwrap();
        assert_eq!(info.class_id, 48, "a Tyrant now");
        assert_eq!(
            (info.name, info.is_female, info.hair_style),
            (face.name, face.is_female, face.hair_style),
            "the same face"
        );
        let fist = info.paperdoll_item_ids[l2_protocol_game::server::charscreen::paperdoll::RHAND];
        assert!(
            gear_data::WEAPONS.contains(&("fist", 1, fist)),
            "a C grade fist weapon: {fist}"
        );
        let sent = t.take();
        assert!(
            texts_to_owner(&sent)
                .iter()
                .any(|s| s.contains("Your Monk is now a Tyrant."))
        );
        assert!(names_to(&sent, 1).iter().any(|n| n == "CHAR_INFO"), "shown anew");
        // At 76 the Grand Khavatari.
        t.world.player_mut(OWNER).unwrap().core.level = 76;
        seconds(&mut t, 2);
        assert_eq!(t.world.fake_player_info(monk).unwrap().class_id, 114);
    }

    #[test]
    fn the_player_can_dismiss_one_or_all() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:healer");
        click(&mut t, "hire:warrior");
        let page = click(&mut t, "dismiss:1");
        assert!(page.contains("The Bishop leaves."), "{page}");
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
    fn the_companions_stay_and_guard_when_the_owner_dies_and_follow_him_to_town() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:tank");
        click(&mut t, "hire:warrior");
        let warrior = only(&t, NPC_WARRIOR);
        t.world.do_die(OWNER, None);
        seconds(&mut t, 3);
        assert_eq!(all_companions(&t), 2, "they stay");
        // A monster that goes for them is fought.
        let mob = t.spawn(MONSTER, 150);
        t.world.add_damage_hate(mob, warrior, 1, 100);
        t.world.set_intention_attack(mob, warrior);
        seconds(&mut t, 6);
        assert!(mob_hp(&t, mob) < MOB_HP, "they defend themselves");
        // He goes to town: they come after him.
        t.world.do_revive(OWNER);
        t.world.set_position(OWNER, X + 8_000, Y, Z);
        seconds(&mut t, 3);
        assert!(
            distance(&t, warrior, OWNER) < 400.0,
            "{:.0}",
            distance(&t, warrior, OWNER)
        );
    }

    #[test]
    fn a_healer_offers_the_dead_owner_a_resurrection() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 45, 10_000_000);
        click(&mut t, "hire:16");
        assert!(
            !skills::of_kind(16, 45, Kind::Resurrect).is_empty(),
            "a Bishop can raise the dead"
        );
        let bishop = only(&t, NPC_HEALER);
        let _ = t.take();
        t.world.do_die(OWNER, None);
        seconds(&mut t, 3);
        let sent = t.take();
        let names = names_to(&sent, 1);
        assert!(
            names.iter().any(|n| n == "CONFIRM_DLG"),
            "the owner is asked: {names:?}"
        );
        let resurrection = class_skill_ids(16, 45, Kind::Resurrect);
        assert!(
            skills_cast(&sent, bishop)
                .iter()
                .any(|id| resurrection.contains(id))
        );
        // He says yes.
        t.world.revive_answer(OWNER, 1);
        t.ticks(2);
        assert!(!t.world.is_dead(OWNER), "raised");
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
            owner_hits(&mut t, mob);
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
        owner_hits(&mut t, prize);
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
        owner_hits(&mut t, mob);
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
        // Passive (the default of L2R.ini): selecting it is not enough.
        t.world.set_target(OWNER, Some(mob));
        seconds(&mut t, 3);
        assert_eq!(
            mob_hp(&t, mob),
            MOB_HP,
            "a passive companion waits for its owner's first blow"
        );
        owner_hits(&mut t, mob);
        seconds(&mut t, 8);
        assert!(mob_hp(&t, mob) < MOB_HP, "{}", mob_hp(&t, mob));
    }

    #[test]
    fn an_aggressive_companion_attacks_what_the_owner_selects_and_the_command_switches_it() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:warrior");
        let _ = t.take();
        say(&mut t, ".companion aggressive");
        t.ticks(2);
        assert!(texts_to_owner(&t.take()).iter().any(|s| s.contains("aggressive")));
        let mob = t.spawn(MONSTER, 200);
        t.world.set_target(OWNER, Some(mob));
        seconds(&mut t, 8);
        assert!(
            mob_hp(&t, mob) < MOB_HP,
            "an aggressive companion goes for the selection"
        );

        say(&mut t, ".companion passive");
        let second = t.spawn(MONSTER, -200);
        t.world.set_target(OWNER, Some(second));
        seconds(&mut t, 4);
        assert_eq!(mob_hp(&t, second), MOB_HP, "passive again");
    }

    #[test]
    fn nobody_fights_in_a_peace_zone() {
        use std::sync::Arc;
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        t.world.set_zone_data(
            Arc::new(l2_data::Zones::load(root)),
            Arc::new(l2_data::MapRegions::load(root)),
        );
        click(&mut t, "hire:warrior");
        say(&mut t, ".companion aggressive");
        let warrior = only(&t, NPC_WARRIOR);
        // The middle of Giran.
        let (x, y, z) = (83_400, 147_943, -3_404);
        t.world.set_position(OWNER, x, y, z);
        t.world.set_position(warrior, x + 50, y, z);
        t.world.revalidate_zone(OWNER, true);
        assert!(
            t.world.is_inside_zone(OWNER, ZoneId::Peace),
            "Giran is a peace zone"
        );
        let mob = t.world.spawn_npc_at(MONSTER, x + 150, y, z, 0).unwrap();
        t.world.set_target(OWNER, Some(mob));
        owner_hits(&mut t, mob);
        seconds(&mut t, 6);
        assert_eq!(mob_hp(&t, mob), MOB_HP, "the companion fought in town");
    }

    #[test]
    fn the_archer_shoots_from_a_distance() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 40, 10_000_000);
        click(&mut t, "hire:archer");
        let archer = only(&t, NPC_ARCHER);
        let mob = t.spawn(MONSTER, 450);
        t.world.set_target(OWNER, Some(mob));
        owner_hits(&mut t, mob);
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

    /// P. Atk., M. Atk. and critical rate of a companion without its buffs.
    fn plain_attack(t: &mut ScriptTest, id: ObjectId) -> (f64, f64, i32) {
        t.world.stop_all_effects(id);
        let s = t.world.stats(id).unwrap();
        (s.p_atk(None), s.m_atk(None, None), s.critical_hit(None, None))
    }

    #[test]
    fn a_companion_hits_like_a_player_of_its_level_with_its_weapon() {
        let Some(root) = datapack() else { return };
        for level in [20, 40, 60, 85] {
            let mut t = scene(root, level, 10_000_000);
            for role in [Role::Warrior, Role::Archer, Role::Mage] {
                click(&mut t, &format!("hire:{}", role.key()));
                let id = only(&t, role.npc_id());
                let weapon = t.world.fake_player_info(id).unwrap().paperdoll_item_ids
                    [l2_protocol_game::server::charscreen::paperdoll::RHAND];
                let (weapon_p, weapon_m) = scale::weapon_attack(weapon);
                let (p_atk, m_atk, crit) = plain_attack(&mut t, id);
                // A player: weapon times the level modifier, M. Atk. times its square (the stat
                // bonuses of the template's base stats are about 1). Never the template's values
                // times the level modifier again, which made a level 60 warrior hit ten times as
                // hard as a player.
                let level_mod = f64::from(level + 89) / 100.0;
                let p_ratio = p_atk / (f64::from(weapon_p) * level_mod);
                assert!(
                    (0.8..1.25).contains(&p_ratio),
                    "{role:?} {level}: {p_atk} with {weapon_p}"
                );
                if role == Role::Mage {
                    let m_ratio = m_atk / (f64::from(weapon_m) * level_mod * level_mod);
                    assert!(
                        (0.8..1.5).contains(&m_ratio),
                        "{role:?} {level}: {m_atk} with {weapon_m}"
                    );
                }
                // A player's critical rate with that weapon (the template value is times 10).
                assert!(crit <= 150, "{role:?} {level}: critical rate {crit}");
            }
        }
    }

    #[test]
    fn the_multipliers_of_l2r_ini_change_attack_and_hp() {
        let Some(root) = datapack() else { return };
        let mut t = scene(root, 60, 10_000_000);
        click(&mut t, "hire:warrior");
        let plain = only(&t, NPC_WARRIOR);
        let weapon = t.world.npc(plain).unwrap().look.as_ref().unwrap().right_hand;
        let (p_atk, ..) = plain_attack(&mut t, plain);
        let hp = t.world.stats(plain).unwrap().max_hp();
        click(&mut t, "dismissall");

        let mut settings = t.world.l2r_settings().clone();
        settings.companions.p_atk_multiplier = 0.5;
        settings.companions.hp_multiplier = 2.0;
        t.world.set_l2r_settings(settings);
        // The same weapon for a fair comparison: hire until the dice give it again.
        for _ in 0..200 {
            click(&mut t, "hire:warrior");
            let id = only(&t, NPC_WARRIOR);
            if t.world.npc(id).unwrap().look.as_ref().unwrap().right_hand == weapon {
                let (half, ..) = plain_attack(&mut t, id);
                assert!((half / p_atk - 0.5).abs() < 0.02, "{p_atk} then {half}");
                let double = t.world.stats(id).unwrap().max_hp();
                assert!(
                    (f64::from(double) / f64::from(hp) - 2.0).abs() < 0.01,
                    "{hp} then {double}"
                );
                return;
            }
            click(&mut t, "dismissall");
        }
        panic!("the dice never gave the weapon {weapon} again");
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
        owner_hits(&mut t, hunter);
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
