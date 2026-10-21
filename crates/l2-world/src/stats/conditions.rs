//! Condition evaluation: the `Condition` trees of `l2-data` tested against the world.
// Java: mechanics/conditions/*.java (testImpl of each class)
//
// Java tests a condition with `(effector, effected, skill, item)`; the stat functions always pass
// `item = null`. Here the effector is a [`StatView`], the effected an optional object id.
//
// # What is decided
// A kind is evaluated when the world models the state it reads. Some state cannot exist in the
// world yet and is therefore known: nobody is transformed, mounted, flying, in a vehicle, in the
// olympiad, in an instance, or has an agathion or a summon. The effects, sitting, the PvP flag,
// the charges and the souls are the world's (`effects/`, M5-EFX-02).
// Kinds that read state the Java server has but the world does not model (the halls and forts
// of clans, sieges and territory wars, heroes, parties,
// categories, the instance type tree, the `Can...` checks, random chance in a read-only stat
// calculation, positions relative to the target's heading) evaluate to `false` and log one
// warning per kind: a stat bonus never applies by accident.

use std::sync::atomic::{AtomicBool, Ordering};

use l2_data::item::{ArmorType, BodyPart};
use l2_data::npc::NpcRace;
use l2_data::stats::PlayerState;
use l2_data::{Condition, ItemTemplate};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use tracing::warn;

use super::CalcSkill;
use super::creature_stat::StatView;
use crate::zones::ZoneId;

/// What a condition is tested against (Java's `testImpl` arguments).
pub(crate) struct CondEnv<'a, 'w> {
    /// `effector`: the creature the stat belongs to.
    pub actor: &'a StatView<'w>,
    /// `effected`.
    pub target: Option<ObjectId>,
    /// `skill`.
    pub skill: Option<&'a CalcSkill>,
}

/// The kinds that are not decided yet, each with its own "already warned" flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum NotModelled {
    Hero,
    SiegeZone,
    SiegeSide,
    ClanHall,
    Fort,
    Category,
    CanAction,
    TargetNpcType,
    MyPartyExceptMe,
    GameChance,
    Behind,
    Front,
    InstanceWorld,
    NpcZones,
    MinDistance,
    TargetMissing,
}

const NOT_MODELLED_KINDS: usize = NotModelled::TargetMissing as usize + 1;

/// One flag per [`NotModelled`] kind: whether its warning was logged. Only the log line is
/// deduplicated; this is not game state.
static WARNED: [AtomicBool; NOT_MODELLED_KINDS] = [const { AtomicBool::new(false) }; NOT_MODELLED_KINDS];

/// `false`, with a warning the first time a kind comes up.
fn not_modelled(kind: NotModelled, what: &str) -> bool {
    if !WARNED[kind as usize].swap(true, Ordering::Relaxed) {
        warn!(
            "Condition {what} reads state the world does not model yet; it evaluates to false (logged once)."
        );
    }
    false
}

/// Java `Condition.test(effector, effected, skill)`.
pub(crate) fn test(cond: &Condition, env: &CondEnv<'_, '_>) -> bool {
    let a = env.actor;
    match cond {
        Condition::And(all) => all.iter().all(|c| test(c, env)),
        Condition::Or(any) => any.iter().any(|c| test(c, env)),
        Condition::Not(c) => !test(c, env),

        // ---- <player ...> ----
        Condition::PlayerRace { races } => a
            .player()
            .and_then(|p| NpcRace::ALL.get(usize::try_from(p.data.race).ok()?))
            .is_some_and(|race| races.contains(race)),
        Condition::PlayerLevel { level } => a.level() >= *level,
        Condition::PlayerLevelRange { levels } => {
            let level = a.level();
            level >= levels[0] && level <= levels[1]
        }
        Condition::PlayerState { check, required } => player_state(*check, *required, env),
        Condition::PlayerIsHero { .. } => {
            a.is_player() && not_modelled(NotModelled::Hero, "player isHero (the heroes table)")
        }
        // `player != null && (player.getPvpFlag() > 0) == value`.
        Condition::PlayerIsPvpFlagged { value } => {
            a.is_player() && (a.world().pvp_flag_of(a.object_id()) > 0) == *value
        }
        // Nobody is transformed: `getTransformationId()` is 0.
        Condition::PlayerTransformationId { id } => a.is_player() && *id == 0,
        // Nobody is mounted (`getMountType()` is NONE).
        Condition::PlayerIsRidingStrider { value } => a.is_player() && !*value,
        Condition::PlayerHp { hp } => (a.core().cur_hp * 100.0) / f64::from(a.max_hp()) <= f64::from(*hp),
        Condition::PlayerMp { mp } => (a.core().cur_mp * 100.0) / f64::from(a.max_mp()) <= f64::from(*mp),
        Condition::PlayerCp { cp } => (a.core().cur_cp * 100.0) / f64::from(a.max_cp()) >= f64::from(*cp),
        // `_value == (byte) getExpertiseLevel()`: the level of skill 239 (Expertise).
        Condition::PlayerGrade { value } => a
            .player()
            .is_some_and(|_| *value == i32::from(a.skill_level(239) as i8)),
        Condition::PlayerPkCount { pk } => a.player().is_some_and(|p| p.data.pk_kills <= *pk),
        Condition::SiegeZone { .. } => not_modelled(NotModelled::SiegeZone, "siegeZone"),
        Condition::PlayerSiegeSide { .. } => {
            a.is_player() && not_modelled(NotModelled::SiegeSide, "player siegeSide")
        }
        Condition::PlayerCharges { charges } => {
            a.is_player() && a.world().charges_of(a.object_id()) >= *charges
        }
        Condition::PlayerSouls { souls } => {
            a.is_player() && a.world().charged_souls_of(a.object_id()) >= *souls
        }
        Condition::PlayerWeight { weight } => match a.player() {
            Some(_) if a.max_load() > 0 => {
                let proc = (a.current_load() - a.bonus_weight_penalty()) * 100 / a.max_load();
                // Java's `getDietMode()` (the `//diet` admin command) is not modelled: off.
                proc < *weight
            }
            _ => true,
        },
        Condition::PlayerInvSize { size } => match a.player() {
            Some(_) => a.non_quest_size() <= a.inventory_limit() - size,
            None => true,
        },
        // `player.isClanLeader() == _value` (M6-CLAN-01).
        Condition::PlayerIsClanLeader { value } => {
            a.is_player() && a.world().is_clan_leader(a.object_id()) == *value
        }
        // `ConditionPlayerPledgeClass`: a clan member whose pledge class reaches the value, or
        // the leader (`-1` asks for the leader).
        Condition::PlayerPledgeClass { pledge_class } => {
            let world = a.world();
            let id = a.object_id();
            if !a.is_player() || world.clan_of(id).is_none() {
                false
            } else {
                let leader = world.is_clan_leader(id);
                if *pledge_class == -1 && !leader {
                    false
                } else {
                    leader || world.pledge_class_of(id) >= *pledge_class
                }
            }
        }
        // Without a clan: `[0]` asks for "no clan hall". A clan's hall comes with the clan hall
        // tables (M7).
        Condition::PlayerHasClanHall { clan_hall } => match a.player() {
            None => false,
            Some(_) if a.world().clan_of(a.object_id()).is_none() => {
                clan_hall.len() == 1 && clan_hall[0] == 0
            }
            Some(_) => not_modelled(
                NotModelled::ClanHall,
                "player clanHall of a clan (clan halls, M7)",
            ),
        },
        // Without a clan: `0` asks for "no fortress". A clan's fortress comes with the forts (M7).
        Condition::PlayerHasFort { fort } => match a.player() {
            None => false,
            Some(_) if a.world().clan_of(a.object_id()).is_none() => *fort == 0,
            Some(_) => not_modelled(NotModelled::Fort, "player fort of a clan (fortresses, M7)"),
        },
        // `ConditionPlayerHasCastle`: the clan's castle (`clan_data.hasCastle`); `-1` is any.
        Condition::PlayerHasCastle { castle } => match a.world().clan_of(a.object_id()) {
            _ if !a.is_player() => false,
            None => *castle == 0,
            Some(clan) if *castle == -1 => clan.castle_id > 0,
            Some(clan) => clan.castle_id == *castle,
        },
        Condition::PlayerSex { sex } => a.player().is_some_and(|p| i32::from(p.data.sex != 0) == *sex),
        // `!isPlayer() || isFlyingMounted() == value`: nobody rides a wyvern.
        Condition::PlayerFlyMounted { value } => !a.is_player() || !*value,
        // `player == null ? true : isInVehicle() == value`: there are no boats or airships.
        Condition::PlayerVehicleMounted { value } => !a.is_player() || !*value,
        Condition::PlayerLandingZone { value } => match a.player() {
            Some(p) => p.zones.is_inside(ZoneId::Landing) == *value,
            None => not_modelled(NotModelled::NpcZones, "landingZone of an NPC (NPC zones)"),
        },
        // `getBuffInfoBySkillId(id)` found, and its level at least `effect_lvl` (or any).
        Condition::PlayerActiveEffectId {
            effect_id,
            effect_lvl,
        } => active_effect(a.world(), a.object_id(), *effect_id, *effect_lvl),
        Condition::PlayerActiveSkillId {
            skill_id,
            skill_level,
        } => a
            .skills()
            .any(|(id, level)| id == *skill_id && (*skill_level == -1 || *skill_level <= level)),
        Condition::PlayerClassIdRestriction { class_ids } => {
            a.player().is_some_and(|p| class_ids.contains(&p.data.class_id))
        }
        // `player == null ? true : isSubClassActive() == value`.
        Condition::PlayerSubclass { value } => a.player().is_none_or(|_| a.is_subclass_active() == *value),
        // `getInstanceId() <= 0` is false; instance worlds are not modelled.
        Condition::PlayerInstanceId { .. } => match a.player() {
            None => false,
            Some(_) if a.instance_id() <= 0 => false,
            Some(_) => not_modelled(NotModelled::InstanceWorld, "player instanceId (instance worlds)"),
        },
        Condition::PlayerAgathionId { agathion_id } => a.is_player() && *agathion_id == 0,
        Condition::PlayerCloakStatus { value } => a.is_player() && a.can_equip_cloak() == *value,
        // `_controlItemIds == null` (the list `0`) is true; otherwise nobody has a pet.
        Condition::PlayerHasPet { control_item_ids } => {
            control_item_ids.len() == 1 && control_item_ids[0] == 0
        }
        Condition::PlayerServitorNpcId { .. } => false,
        Condition::PlayerRangeFromNpc {
            npc_ids,
            radius,
            value,
        } => {
            let mut exists = false;
            if !npc_ids.is_empty() && *radius > 0 {
                let world = a.world();
                exists = world
                    .visible_objects_in_range(a.object_id(), *radius)
                    .into_iter()
                    .filter_map(|id| world.npcs.get(&id))
                    .any(|npc| npc_ids.contains(&npc.npc_id));
            }
            exists == *value
        }
        Condition::PlayerCallPc { .. }
        | Condition::PlayerCanCreateBase { .. }
        | Condition::PlayerCanCreateOutpost { .. }
        | Condition::PlayerCanEscape { .. }
        | Condition::PlayerCanRefuelAirship { .. }
        | Condition::PlayerCanResurrect { .. }
        | Condition::PlayerCanSummon { .. }
        | Condition::PlayerCanSummonSiegeGolem { .. }
        | Condition::PlayerCanTakeCastle
        | Condition::PlayerCanTakeFort { .. }
        | Condition::PlayerCanTransform { .. }
        | Condition::PlayerCanUntransform { .. } => {
            not_modelled(NotModelled::CanAction, "player can... (the action checks of M5)")
        }
        // Java `ConditionPlayerCanSweep`: `_value == canSweep`. Like Java's `testImpl` it tells
        // the sweeper why it cannot (one message per failed check).
        Condition::PlayerCanSweep { value } => {
            let world = a.world();
            let can_sweep = env
                .skill
                .and_then(|s| {
                    let skills = world.skill_table()?;
                    let template = skills.get(s.id, s.level)?;
                    let check = world.can_sweep_check(a.object_id(), template);
                    for id in &check.messages {
                        world.send(a.object_id(), &l2_protocol_game::server::SystemMessage::new(*id));
                    }
                    Some(check.can_sweep)
                })
                .unwrap_or(false);
            *value == can_sweep
        }
        Condition::PlayerInsideZoneId { zones } => a
            .player()
            .is_some_and(|p| p.zones.members().iter().any(|(id, _)| zones.contains(id))),
        // `getBuffInfoByAbnormalType(type)` found, and `level` at least its abnormal level.
        Condition::PlayerCheckAbnormal { abnormal_type, level } => a
            .world()
            .effect_list(a.object_id())
            .and_then(|l| l.info_by_abnormal_type(*abnormal_type))
            .is_some_and(|info| *level == -1 || *level >= info.facts.abnormal_level),
        Condition::CategoryType { .. } => not_modelled(NotModelled::Category, "categoryType (CategoryData)"),

        // ---- <target ...> ----
        Condition::TargetAggro { is_aggro } => match target(env) {
            Some(t) => match t.npc() {
                // `Monster.isAggressive()`: the template flag, unless a PASSIVE effect.
                Some(npc) if npc.class.is_monster() => {
                    (t.npc_template().is_some_and(|tpl| tpl.is_aggressive)
                        && !a
                            .world()
                            .is_affected(t.object_id(), crate::effects::effect_flag::PASSIVE))
                        == *is_aggro
                }
                Some(_) => false,
                None => t.player().is_some_and(|p| p.data.karma > 0),
            },
            None => false,
        },
        Condition::TargetLevel { level } => target(env).is_some_and(|t| t.level() >= *level),
        Condition::TargetLevelRange { levels } => target(env).is_some_and(|t| {
            let level = t.level();
            level >= levels[0] && level <= levels[1]
        }),
        Condition::TargetMyPartyExceptMe { .. } => {
            // Java also sends the player a system message from here; parties are M5.
            not_modelled(NotModelled::MyPartyExceptMe, "target myPartyExceptMe (parties)")
        }
        Condition::TargetPlayable => target(env).is_some_and(|t| t.is_player()),
        Condition::TargetClassIdRestriction { class_ids } => target(env)
            .and_then(|t| t.player().map(|p| p.data.class_id))
            .is_some_and(|class| class_ids.contains(&class)),
        // `effected.getEffectList().getBuffInfoBySkillId(id)`.
        Condition::TargetActiveEffectId {
            effect_id,
            effect_lvl,
        } => match env.target {
            Some(t) => active_effect(a.world(), t, *effect_id, *effect_lvl),
            None => not_modelled(NotModelled::TargetMissing, "target ... without a target"),
        },
        Condition::TargetActiveSkillId {
            skill_id,
            skill_level,
        } => match target(env) {
            Some(t) => t
                .skills()
                .any(|(id, level)| id == *skill_id && (*skill_level == -1 || *skill_level <= level)),
            None => not_modelled(NotModelled::TargetMissing, "target ... without a target"),
        },
        // `(effected.getAbnormalVisualEffects() & abnormalId) != 0`.
        Condition::TargetAbnormal { abnormal_id } => match env.target {
            Some(t) => a.world().abnormal_visual_effects(t).0 & *abnormal_id != 0,
            None => not_modelled(NotModelled::TargetMissing, "target ... without a target"),
        },
        // `calculateDistance3D >= distance && GeoEngine.canSeeTarget(effector, effected)`: the
        // creature-to-creature line of sight (eye heights, doors) comes with combat.
        Condition::MinDistance { .. } => match target(env) {
            Some(_) => not_modelled(NotModelled::MinDistance, "minDistance (line of sight)"),
            None => false,
        },
        Condition::TargetRace { race } => match target(env) {
            Some(t) => t.race() == Some(*race),
            // Java dereferences the null target.
            None => not_modelled(NotModelled::TargetMissing, "target ... without a target"),
        },
        Condition::TargetUsesWeaponKind { weapon_mask } => target(env)
            .and_then(|t| t.active_weapon_item())
            .and_then(ItemTemplate::weapon)
            .is_some_and(|w| w.weapon_type.mask() & weapon_mask != 0),
        // `(target == null || !isPlayer()) ? effected : effector.getTarget()`.
        Condition::TargetNpcId { npc_ids } => {
            let subject = match a.player() {
                Some(p) => p.target,
                None => env.target,
            };
            subject.is_some_and(|id| {
                let world = a.world();
                if let Some(npc) = world.npcs.get(&id) {
                    npc_ids.contains(&npc.npc_id)
                } else if let Some(door) = world.doors.get(&id) {
                    npc_ids.contains(&door.door_id)
                } else {
                    false
                }
            })
        }
        Condition::TargetNpcType { .. } => match target(env) {
            Some(_) => not_modelled(
                NotModelled::TargetNpcType,
                "target npcType (the InstanceType tree)",
            ),
            None => false,
        },
        Condition::TargetWeight { weight } => target(env).is_some_and(|t| {
            // `getDietMode()` is not modelled: off.
            t.player().is_some()
                && t.max_load() > 0
                && (t.current_load() - t.bonus_weight_penalty()) * 100 / t.max_load() < *weight
        }),
        Condition::TargetInvSize { size } => target(env)
            .is_some_and(|t| t.player().is_some() && t.non_quest_size() <= t.inventory_limit() - size),

        // ---- <using ...> ----
        Condition::UsingItemType { mask } => using_item_type(*mask, a),
        Condition::UsingSlotType { mask } => {
            a.player().is_some()
                && a.active_weapon_item()
                    .is_some_and(|w| w.body_part.mask() & mask != 0)
        }
        Condition::UsingSkill { skill_id } => env.skill.is_some_and(|s| s.id == *skill_id),
        Condition::SlotItemId {
            slot,
            item_id,
            enchant_level,
        } => match a.player() {
            None => false,
            Some(_) => match usize::try_from(*slot).ok().and_then(|s| a.paperdoll(s)) {
                None => *item_id == 0,
                Some(item) => item.item_id == *item_id && item.enchant_level >= *enchant_level,
            },
        },
        Condition::ChangeWeapon { required } => match a.player() {
            None => false,
            // `isEnchanting()` (an enchant scroll in use) is not modelled: never.
            Some(_) if *required => a
                .active_weapon_item()
                .and_then(ItemTemplate::weapon)
                .is_some_and(|w| w.change_weapon_id != 0),
            Some(_) => true,
        },

        // ---- <game ...> ----
        Condition::WithSkill { skill } => env.skill.is_some() == *skill,
        // `GameTimeTaskManager.isNight()`; with the game clock off (tests) it is day.
        Condition::GameTime { required } => a.world().game_clock_night.unwrap_or(false) == *required,
        // `Rnd.get(100) < chance` needs the world RNG, which a read-only stat calculation cannot
        // draw from.
        Condition::GameChance { .. } => not_modelled(NotModelled::GameChance, "game chance (random)"),
    }
}

/// The effected creature as a view.
fn target<'w>(env: &CondEnv<'_, 'w>) -> Option<StatView<'w>> {
    env.actor.other(env.target?)
}

/// `ConditionPlayerState`.
/// `ConditionPlayerActiveEffectId` / `ConditionTargetActiveEffectId`: the creature has the skill
/// as an effect, at least at the level asked for (`-1`: any).
fn active_effect(world: &crate::world::World, id: ObjectId, effect_id: i32, effect_lvl: i32) -> bool {
    world
        .effect_list(id)
        .and_then(|l| l.info_by_skill_id(effect_id))
        .is_some_and(|info| effect_lvl == -1 || effect_lvl <= info.skill().level)
}

fn player_state(check: PlayerState, required: bool, env: &CondEnv<'_, '_>) -> bool {
    let a = env.actor;
    let sitting = a.world().is_sitting(a.object_id());
    match check {
        PlayerState::Resting => {
            if a.is_player() {
                sitting == required
            } else {
                !required
            }
        }
        PlayerState::Moving => a.is_moving() == required,
        PlayerState::Running => a.is_moving() == required && a.is_running() == required,
        PlayerState::Standing => {
            if a.is_player() {
                required != (sitting || a.is_moving())
            } else {
                required != a.is_moving()
            }
        }
        // Nothing flies yet (`_isFlying` is set by wyverns, airships and flying transformations).
        PlayerState::Flying => !required,
        PlayerState::Behind => not_modelled(NotModelled::Behind, "player behind"),
        PlayerState::Front => not_modelled(NotModelled::Front, "player front"),
        PlayerState::Chaotic => match a.player() {
            Some(p) => (p.data.karma > 0) == required,
            None => !required,
        },
        // Nobody is in the olympiad.
        PlayerState::Olympiad => !required,
    }
}

/// `ConditionUsingItemType`.
fn using_item_type(mask: i32, a: &StatView<'_>) -> bool {
    let armor = mask & (ArmorType::Magic.mask() | ArmorType::Light.mask() | ArmorType::Heavy.mask()) != 0;
    if a.player().is_none() {
        return !armor && (mask & a.attack_type().mask()) != 0;
    }
    if armor {
        let none = ArmorType::None.mask();
        let chest = a.paperdoll_template(paperdoll::CHEST);
        let legs = a.paperdoll_template(paperdoll::LEGS);
        // A paperdoll item without a template (unknown item id) counts as worn with mask 0.
        let worn = |slot| a.paperdoll(slot).is_some();
        let item_mask = |t: Option<&ItemTemplate>| t.map_or(0, ItemTemplate::item_mask);
        match (worn(paperdoll::CHEST), worn(paperdoll::LEGS)) {
            (false, false) => return (none & mask) == none,
            (true, false) => {
                if chest.is_some_and(|t| t.body_part == BodyPart::FullArmor) {
                    return (mask & item_mask(chest)) != 0;
                }
                return (none & mask) == none;
            }
            (true, true) => {
                let chest_mask = item_mask(chest);
                if chest_mask == item_mask(legs) {
                    return (mask & chest_mask) != 0;
                }
                return (none & mask) == none;
            }
            // Legs without a chest: Java falls through to the worn mask.
            (false, true) => {}
        }
    }
    (mask & a.weared_mask()) != 0
}
