//! Pailaka Devil's Legacy (Devil's Isle), instance 44: Lematan, his followers and the powder kegs.
// Java: dist/game/data/scripts/instances/PailakaDevilsLegacy/PailakaDevilsLegacy.java
//
// Java keeps Lematan and the follower list as objects in the `InstanceWorld` parameters
// (`world.setParameter("lematanNpc", npc)`); here the parameters are strings, so the NPCs are
// object ids ("lematanNpc": one id, "followerslist": ids separated by commas).

use l2_protocol_game::server::{MagicSkillUse, SkillPosition};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

use super::ScriptValues;

// NPCs
const LEMATAN: i32 = 18633; // Lematan
const SURVIVOR: i32 = 32498; // Devil's Isle Survivor
const FOLLOWERS: i32 = 18634; // Lematan's Follower
const POWDER_KEG: i32 = 18622; // Powder Keg
const TREASURE_BOX: i32 = 32495; // Treasure Chest
const ADVENTURER2: i32 = 32511; // Dwarf Adventurer

// Items
const ANTIDOTE_POTION: i32 = 13048; // Pailaka Antidote
const DIVINE_POTION: i32 = 13049; // Divine Soul
const PAILAKA_KEY: i32 = 13150; // Pailaka All-Purpose Key
const SHIELD: i32 = 13032; // Pailaka Instant Shield
const DEFENCE_POTION: i32 = 13059; // Long-Range Defense Increasing Potion
const HEALING_POTION: i32 = 13033; // Quick Healing Potion

// Skills
const ENERGY: SkillKey = SkillKey { id: 5712, level: 1 }; // Energy Ditch
const BOOM: SkillKey = SkillKey { id: 5714, level: 1 }; // Boom Up
const AV_TELEPORT: SkillKey = SkillKey { id: 4671, level: 1 }; // AV - Teleport
const WATER_CANNON: SkillKey = SkillKey { id: 5708, level: 1 }; // Water Cannon
const WHIRLPOOL: SkillKey = SkillKey { id: 5709, level: 1 }; // Whirlpool
const TRIPLE_SWORD: SkillKey = SkillKey { id: 5710, level: 1 }; // Triple Sword
const POWER_OF_RAGE: SkillKey = SkillKey { id: 5711, level: 1 }; // Power of Rage
const SUMMON_PRESENTATION: SkillKey = SkillKey { id: 5756, level: 1 }; // Presentation - Summon Follower

// Locations
const TELEPORT: (i32, i32, i32) = (76427, -219045, -3780);
/// x, y, z, heading.
const LEMATAN_SPAWN: (i32, i32, i32, i32) = (88108, -209252, -3744, 6425);
const LEMATAN_PORT_POINT: (i32, i32, i32) = (86116, -209117, -3774);
const LEMATAN_PORT: (i32, i32, i32) = (85000, -208699, -3336);
const ADVENTURER_LOC: (i32, i32, i32, i32) = (84983, -208736, -3336, 49915);
const FOLLOWERS_LOC: [(i32, i32, i32, i32); 8] = [
    (85067, -208943, -3336, 20106),
    (84904, -208944, -3336, 10904),
    (85062, -208538, -3336, 44884),
    (84897, -208542, -3336, 52973),
    (84808, -208633, -3339, 65039),
    (84808, -208856, -3339, 0),
    (85144, -208855, -3341, 33380),
    (85139, -208630, -3339, 31777),
];

// Misc
const TEMPLATE_ID: i32 = 44;
const ZONE: i32 = 20109;
const ZONE_EXIT: i32 = 200000;
const TIGRESS_LEVEL1: i32 = 14916;
const TIGRESS_LEVEL2: i32 = 14917;

/// Java `Q00129_PailakaDevilsLegacy.class.getSimpleName()`.
const Q00129: &str = "Q00129_PailakaDevilsLegacy";

/// The `Npc._scriptValue` of Lematan, the kegs and the chests.
#[derive(Default)]
struct State {
    values: ScriptValues,
}

/// Java `instances.PailakaDevilsLegacy.PailakaDevilsLegacy`.
pub struct PailakaDevilsLegacy;

impl Script for PailakaDevilsLegacy {
    fn name(&self) -> &'static str {
        "PailakaDevilsLegacy"
    }

    fn path(&self) -> &'static str {
        "instances/PailakaDevilsLegacy"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[SURVIVOR]);
        s.add_attack_id(&[POWDER_KEG, TREASURE_BOX, LEMATAN]);
        s.add_kill_id(&[LEMATAN]);
        s.add_spawn_id(&[FOLLOWERS]);
        s.add_enter_zone_id(&[ZONE]);
        s.add_exit_zone_id(&[ZONE_EXIT]);
        s.add_move_finished_id(&[LEMATAN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `InstanceManager.getInstance().getWorld(npc)`.
        let instance_id = npc.map_or(0, |n| ctx.world().instance_id_of(n.object_id));
        let in_world = npc.is_some_and(|n| ctx.world().instance_world_of(n.object_id).is_some());
        let mut htmltext = None;
        if event == "enter" {
            let player = player?;
            let qs = ctx.quest_state_of(Q00129, player, false);
            enter_instance(ctx, player);
            // NOTE(java-quirk): `qs` is null for a player without the quest and Java throws here
            // (after he was already put into the instance).
            let qs = qs?;
            if qs.is_cond(ctx, 1) {
                qs.set_cond(ctx, 2, true);
                htmltext = Some("32498-01.htm".to_owned());
            } else {
                htmltext = Some("32498-02.htm".to_owned());
            }
        } else if in_world {
            let npc = npc?;
            match event {
                "FOLLOWER_CAST" => {
                    let lematan = lematan_of(ctx, instance_id);
                    if let Some(lematan) = lematan.filter(|l| !ctx.world().is_dead(l.object_id)) {
                        for follower in followers_of(ctx, instance_id) {
                            ctx.cast_skill(follower, lematan.object_id, ENERGY);
                        }

                        ctx.start_quest_timer("FOLLOWER_CAST", 15000, Some(lematan), None, false);
                    }
                }
                "LEMATAN_GROUND_SKILLS" => {
                    if ctx.world().is_dead(npc.object_id) || !ctx.state::<State>().values.is(npc.object_id, 0)
                    {
                        return htmltext;
                    }

                    // Water Cannon lands on three attacks in ten, the core AI swings on the rest.
                    let ground_target = ctx.world_mut().aggro_most_hated(npc.object_id);
                    if let Some(ground_target) = ground_target {
                        if ctx.get_random(100) < 30 {
                            ctx.cast_skill(npc, ground_target, WATER_CANNON);
                        }
                    }

                    ctx.start_quest_timer("LEMATAN_GROUND_SKILLS", 5000, Some(npc), None, false);
                }
                "LEMATAN_SHIP_SKILLS" => {
                    if ctx.world().is_dead(npc.object_id) {
                        return htmltext;
                    }

                    // He fights only on the deck, so a target that has drawn away drags him back aboard.
                    let ship_target = ctx.world_mut().aggro_most_hated(npc.object_id);
                    if let Some(target) = ship_target {
                        if distance_3d(ctx, npc.object_id, target) > 800.0 {
                            tele_npc(ctx, npc, LEMATAN_PORT, instance_id);
                        }
                    }

                    // Power of Rage buffs Lematan himself and is rolled before the attack skills.
                    if ctx.get_random(100) < 10 {
                        ctx.cast_skill(npc, npc.object_id, POWER_OF_RAGE);
                    }

                    // Half the ticks cast, and that cast is an even split between the two ship skills.
                    if let Some(target) = ship_target {
                        if ctx.get_random(100) < 50 {
                            let skill = if ctx.get_random(2) == 0 {
                                WHIRLPOOL
                            } else {
                                TRIPLE_SWORD
                            };
                            ctx.cast_skill(npc, target, skill);
                        }
                    }

                    ctx.start_quest_timer("LEMATAN_SHIP_SKILLS", 15000, Some(npc), None, false);
                }
                "LEMATAN_MOVE_RETRY" => {
                    if !ctx.world().is_dead(npc.object_id) && ctx.state::<State>().values.is(npc.object_id, 1)
                    {
                        let world = ctx.world_mut();
                        world.set_npc_running(npc.object_id);
                        world.npc_move_to_location(
                            npc.object_id,
                            LEMATAN_PORT_POINT.0,
                            LEMATAN_PORT_POINT.1,
                            LEMATAN_PORT_POINT.2,
                        );
                        ctx.start_quest_timer("LEMATAN_MOVE_RETRY", 5000, Some(npc), None, false);
                    }
                }
                "LEMATAN_TELEPORT" => {
                    if ctx.world().is_dead(npc.object_id) {
                        return htmltext;
                    }

                    ctx.world_mut().clear_aggro_list(npc.object_id);
                    ctx.world_mut().set_core_ai_disabled(npc.object_id, false);
                    tele_npc(ctx, npc, LEMATAN_PORT, instance_id);
                    // `npc.getVariables().set("ON_SHIP", 1)`: nothing in the datapack reads it.
                    ctx.world_mut().set_npc_spawn_point(
                        npc.object_id,
                        LEMATAN_PORT.0,
                        LEMATAN_PORT.1,
                        LEMATAN_PORT.2,
                    );
                    ctx.cast_skill(npc, npc.object_id, SUMMON_PRESENTATION);
                    let mut followers = followers_of(ctx, instance_id);
                    for loc in FOLLOWERS_LOC {
                        let follower =
                            ctx.add_spawn(FOLLOWERS, loc.0, loc.1, loc.2, loc.3, false, 0, instance_id);
                        if let Some(follower) = follower {
                            ctx.world_mut().set_core_ai_disabled(follower.object_id, true);
                            ctx.world_mut().set_immobilized(follower.object_id, true);
                            followers.push(follower);
                        }
                    }

                    set_followers(ctx, instance_id, &followers);
                    let lematan = lematan_of(ctx, instance_id);
                    ctx.start_quest_timer("FOLLOWER_CAST", 4000, lematan, None, false);
                    ctx.start_quest_timer("LEMATAN_SHIP_SKILLS", 10, Some(npc), None, false);
                }
                "TELEPORT" => {
                    if let Some(player) = player {
                        // NOTE(java-quirk): `player.teleToLocation(TELEPORT)` takes the instance id of
                        // the `Location`, which is 0: the player lands at the entry point of the open
                        // world, not inside the instance.
                        ctx.world_mut().tele_to_location(
                            player,
                            TeleportTarget::at(TELEPORT.0, TELEPORT.1, TELEPORT.2).with_instance(0),
                        );
                    }
                }
                "DELETE" => {
                    ctx.state::<State>().values.forget(npc.object_id);
                    ctx.world_mut().delete_npc(npc.object_id);
                }
                _ => {}
            }
        }

        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.world().instance_world_of(npc.object_id).is_none() {
            return;
        }
        match npc.id {
            POWDER_KEG => {
                if damage > 0 && ctx.state::<State>().values.is(npc.object_id, 0) {
                    let monsters: Vec<ObjectId> = ctx
                        .world()
                        .visible_objects_in_range(npc.object_id, 600)
                        .into_iter()
                        .filter(|o| ctx.world().is_monster_npc(*o))
                        .collect();
                    for monster in monsters {
                        ctx.world_mut().add_damage_hate(monster, npc.object_id, 0, 999);
                        ctx.world_mut().set_intention_attack(monster, npc.object_id);
                        let amount = 500 + ctx.get_random_range(0, 200);
                        // The skill `BOOM` of Java's `reduceCurrentHp(.., npc, BOOM.getSkill())` only
                        // marks the damage as a skill hit.
                        ctx.world_mut()
                            .reduce_current_hp(monster, f64::from(amount), Some(npc.object_id));
                    }
                    ctx.world_mut().do_cast(npc.object_id, BOOM);
                    ctx.state::<State>().values.set(npc.object_id, 1);
                    ctx.start_quest_timer("DELETE", 2000, Some(npc), None, false);
                }
            }
            LEMATAN => {
                if ctx.state::<State>().values.is(npc.object_id, 0)
                    && !ctx.has_quest_timer("LEMATAN_GROUND_SKILLS", Some(npc), None)
                {
                    ctx.start_quest_timer("LEMATAN_GROUND_SKILLS", 1000, Some(npc), None, false);
                }

                let low = ctx
                    .world()
                    .creature_hp(npc.object_id)
                    .is_some_and(|(hp, max)| hp < max * 0.5);
                if ctx.state::<State>().values.is(npc.object_id, 0) && low {
                    ctx.world_mut().set_core_ai_disabled(npc.object_id, true);
                    ctx.state::<State>().values.set(npc.object_id, 1);
                    let world = ctx.world_mut();
                    world.set_npc_running(npc.object_id);
                    world.npc_move_to_location(
                        npc.object_id,
                        LEMATAN_PORT_POINT.0,
                        LEMATAN_PORT_POINT.1,
                        LEMATAN_PORT_POINT.2,
                    );
                    ctx.start_quest_timer("LEMATAN_MOVE_RETRY", 5000, Some(npc), None, false);
                }
            }
            TREASURE_BOX => {
                if ctx.state::<State>().values.is(npc.object_id, 0) {
                    match ctx.get_random(7) {
                        0 | 1 => {
                            let count = i64::from(ctx.get_random_range(1, 10));
                            drop_item(ctx, npc, attacker, ANTIDOTE_POTION, count);
                        }
                        2 => {
                            let count = i64::from(ctx.get_random_range(1, 5));
                            drop_item(ctx, npc, attacker, DIVINE_POTION, count);
                        }
                        3 => {
                            let count = i64::from(ctx.get_random_range(1, 2));
                            drop_item(ctx, npc, attacker, PAILAKA_KEY, count);
                        }
                        4 => {
                            let count = i64::from(ctx.get_random_range(1, 7));
                            drop_item(ctx, npc, attacker, DEFENCE_POTION, count);
                        }
                        5 => {
                            let count = i64::from(ctx.get_random_range(1, 10));
                            drop_item(ctx, npc, attacker, SHIELD, count);
                        }
                        6 => {
                            let count = i64::from(ctx.get_random_range(1, 10));
                            drop_item(ctx, npc, attacker, HEALING_POTION, count);
                        }
                        _ => {}
                    }

                    ctx.state::<State>().values.set(npc.object_id, 1);
                    ctx.start_quest_timer("DELETE", 3000, Some(npc), Some(attacker), false);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        for follower in followers_of(ctx, instance_id) {
            ctx.world_mut().delete_npc(follower.object_id);
        }

        if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
            world.parameters.remove("followerslist");
        }
        let instance_of_npc = ctx.world().instance_id_of(npc.object_id);
        ctx.add_spawn(
            ADVENTURER2,
            ADVENTURER_LOC.0,
            ADVENTURER_LOC.1,
            ADVENTURER_LOC.2,
            ADVENTURER_LOC.3,
            false,
            0,
            instance_of_npc,
        );
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if is_live_player(ctx, creature) {
            let instance_id = ctx
                .world()
                .instance_world_of(creature)
                .filter(|w| w.template_id == TEMPLATE_ID)
                .map(|w| w.instance_id);
            if let Some(instance_id) = instance_id {
                let lematan = lematan_of(ctx, instance_id);
                ctx.start_quest_timer("TELEPORT", 1000, lematan, Some(creature), false);
            }
        }
    }

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if ctx.world().player(creature).is_none() {
            return;
        }
        let Some(summon) = ctx.world().summon_of(creature) else {
            return;
        };
        let Some(template_id) = ctx.world().summon_npc_id(summon) else {
            return;
        };
        if template_id == TIGRESS_LEVEL1 || template_id == TIGRESS_LEVEL2 {
            ctx.world_mut().discard_summon(creature, summon);
        }
    }

    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if ctx.state::<State>().values.is(npc.object_id, 1) {
            ctx.state::<State>().values.set(npc.object_id, 2);
            if let Some(packet) = magic_skill_use(ctx, npc.object_id, npc.object_id, AV_TELEPORT, 2000) {
                ctx.world().broadcast_packet(npc.object_id, &packet);
            }
            ctx.start_quest_timer("LEMATAN_TELEPORT", 2000, Some(npc), None, false);
        }
    }
}

/// `creature.isPlayer() && !creature.isDead() && !creature.isTeleporting() &&
/// creature.asPlayer().isOnline()`.
fn is_live_player(ctx: &ScriptCtx<'_>, creature: ObjectId) -> bool {
    let world = ctx.world();
    world.player(creature).is_some() && !world.is_dead(creature) && !world.is_teleporting(creature)
}

/// `npc.dropItem(player, itemId, count)`.
fn drop_item(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, item_id: i32, count: i64) {
    ctx.world_mut()
        .npc_drop_item(npc.object_id, Some(player), item_id, count);
}

/// `npc.calculateDistance3D(other)`.
fn distance_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some(a), Some(b)) => {
            let (dx, dy, dz) = (
                f64::from(a.0) - f64::from(b.0),
                f64::from(a.1) - f64::from(b.1),
                f64::from(a.2) - f64::from(b.2),
            );
            (dx * dx + dy * dy + dz * dz).sqrt()
        }
        _ => f64::MAX,
    }
}

/// `npc.teleToLocation(location, instanceId, 0)`.
fn tele_npc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, to: (i32, i32, i32), instance_id: i32) {
    ctx.world_mut().tele_to_location(
        npc.object_id,
        TeleportTarget::at(to.0, to.1, to.2).with_instance(instance_id),
    );
}

/// `new MagicSkillUse(caster, target, skillId, skillLevel, hitTime, 0)`.
fn magic_skill_use(
    ctx: &ScriptCtx<'_>,
    caster: ObjectId,
    target: ObjectId,
    skill: SkillKey,
    hit_time: i32,
) -> Option<MagicSkillUse> {
    let position = |id: ObjectId| {
        ctx.world()
            .position_of(id)
            .map(|(x, y, z)| SkillPosition { x, y, z })
    };
    Some(MagicSkillUse {
        creature_id: caster.0,
        target_id: target.0,
        skill_id: skill.id,
        skill_level: skill.level,
        hit_time,
        reuse_delay: 0,
        creature: position(caster)?,
        ground_location: None,
        target: position(target)?,
    })
}

/// `world.getParameters().getObject("lematanNpc", Npc.class)`.
fn lematan_of(ctx: &ScriptCtx<'_>, instance_id: i32) -> Option<ScriptNpc> {
    let id: i32 = ctx
        .world()
        .instance_world(instance_id)?
        .parameters
        .get("lematanNpc")?
        .parse()
        .ok()?;
    Some(ScriptNpc {
        object_id: ObjectId(id),
        id: LEMATAN,
    })
}

/// `world.getParameters().getList("followerslist", Npc.class, new ArrayList<>())`.
fn followers_of(ctx: &ScriptCtx<'_>, instance_id: i32) -> Vec<ScriptNpc> {
    let Some(list) = ctx
        .world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get("followerslist"))
    else {
        return Vec::new();
    };
    list.split(',')
        .filter_map(|id| id.parse::<i32>().ok())
        .map(|id| ScriptNpc {
            object_id: ObjectId(id),
            id: FOLLOWERS,
        })
        .collect()
}

/// `world.setParameter("followerslist", followerslist)`.
fn set_followers(ctx: &mut ScriptCtx<'_>, instance_id: i32, followers: &[ScriptNpc]) {
    let list = followers
        .iter()
        .map(|f| f.object_id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.parameters.insert("followerslist".to_owned(), list);
    }
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let offset = ctx.world().max_offset_on_teleport();
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
                let lematan = world.script_add_spawn(
                    LEMATAN,
                    (LEMATAN_SPAWN.0, LEMATAN_SPAWN.1, LEMATAN_SPAWN.2),
                    LEMATAN_SPAWN.3,
                    false,
                    0,
                    instance_id,
                );
                if let (Some(lematan), Some(instance_world)) =
                    (lematan, world.instance_world_mut(instance_id))
                {
                    instance_world
                        .parameters
                        .insert("lematanNpc".to_owned(), lematan.0.to_string());
                }
            }
            world.tele_to_location(
                player,
                TeleportTarget::at(TELEPORT.0, TELEPORT.1, TELEPORT.2)
                    .with_instance(instance_id)
                    .with_random_offset(offset),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ClientInput, QuestLoad, QuestWrite, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z, pages_to, player_data};

    const NPCS: &str = r#"
        <npc id="32498" type="Folk" level="70" name="Devil's Isle Survivor"/>
        <npc id="32495" type="Folk" level="70" name="Treasure Chest"/>
        <npc id="32511" type="Folk" level="70" name="Dwarf Adventurer"/>
        <npc id="18633" level="60" type="Monster" name="Lematan">
            <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
                <vitals hp="1000" hpRegen="2" mp="400" mpRegen="0.9" />
                <speed><walk ground="20" /><run ground="50" /></speed>
            </stats>
        </npc>
        <npc id="18634" level="60" type="Monster" name="Lematan's Follower"/>
        <npc id="18622" level="60" type="Monster" name="Powder Keg"/>
        <npc id="18700" level="60" type="Monster" name="Guard">
            <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
                <vitals hp="5000" hpRegen="2" mp="400" mpRegen="0.9" />
            </stats>
        </npc>"#;
    const ITEMS: &str = r#"
        <item id="13048" type="EtcItem" name="Pailaka Antidote"/>
        <item id="13049" type="EtcItem" name="Divine Soul"/>
        <item id="13150" type="EtcItem" name="Pailaka Key"/>
        <item id="13032" type="EtcItem" name="Shield"/>
        <item id="13059" type="EtcItem" name="Defence Potion"/>
        <item id="13033" type="EtcItem" name="Healing Potion"/>"#;
    const DIR: &str = "data/scripts/instances/PailakaDevilsLegacy";

    /// Java `Q00129_PailakaDevilsLegacy` as far as the instance needs it: a quest of that name.
    struct Quest129;

    impl Script for Quest129 {
        fn name(&self) -> &'static str {
            Q00129
        }
        fn path(&self) -> &'static str {
            "quests/Q00129_PailakaDevilsLegacy"
        }
        fn id(&self) -> i32 {
            129
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(PailakaDevilsLegacy);
        registry.add(Quest129);
        let page1 = format!("{DIR}/32498-01.htm");
        let page2 = format!("{DIR}/32498-02.htm");
        let pages = [
            (
                "data/html/default/32498.htm",
                r#"<html><body>Survivor <a action="bypass Script PailakaDevilsLegacy enter">Enter</a></body></html>"#,
            ),
            (page1.as_str(), "<html><body>32498-01</body></html>"),
            (page2.as_str(), "<html><body>32498-02</body></html>"),
        ];
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        t.world.set_instance_data(instance_data(
            &[(44, "Pailaka (Devil's Isle)")],
            &[(44, r#"<exitPoint x="85039" y="147543" z="-3400" />"#)],
        ));
        let survivor = t.spawn(SURVIVOR, 20);
        (t, survivor)
    }

    /// A character with Q00129 started at `cond`.
    fn with_quest(t: &mut ScriptTest, id: i32, cond: i32) {
        let mut data = player_data(id, 40, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(Q00129.to_owned(), Some("Started".to_owned()))],
            variables: vec![(Q00129.to_owned(), "cond".to_owned(), Some(cond.to_string()))],
        };
        t.enter_with(data);
    }

    fn enter(t: &mut ScriptTest, survivor: Oid) {
        t.talk(1, survivor);
        t.bypass(1, "Script PailakaDevilsLegacy enter");
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
    }

    fn lematan(t: &ScriptTest) -> ScriptNpc {
        let id: i32 = t.world.instance_world(300_000).unwrap().parameters["lematanNpc"]
            .parse()
            .unwrap();
        ScriptNpc {
            object_id: Oid(id),
            id: LEMATAN,
        }
    }

    fn cond_write(value: &str) -> QuestWrite {
        QuestWrite::UpdateVar {
            char_id: 1,
            name: Q00129.to_owned(),
            var: "cond".to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn enter_spawns_lematan_and_moves_the_quest_to_cond_2() {
        let (mut t, survivor) = setup();
        with_quest(&mut t, 1, 1);
        t.take();
        enter(&mut t, survivor);

        // The instance of template 44 allows the player and holds Lematan at his place.
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
        let world = t.world.instance_world(300_000).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (44, vec![Oid(1)]));
        let lematan = lematan(&t);
        assert_eq!(t.world.npc(lematan.object_id).unwrap().npc_id, LEMATAN);
        assert_eq!(t.world.instance_id_of(lematan.object_id), 300_000);
        assert_eq!(
            t.world.position_of(lematan.object_id).map(|p| (p.0, p.1)),
            Some((LEMATAN_SPAWN.0, LEMATAN_SPAWN.1))
        );
        // `qs.setCond(2, true)` and the page 32498-01.
        assert_eq!(t.take_writes(), [cond_write("2")]);
        assert!(
            pages_to(&t.take(), 1)
                .last()
                .is_some_and(|p| p.contains("32498-01"))
        );

        // At cond 2 the survivor says 32498-02 and the same instance takes him in.
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(0));
        t.ticks(60);
        t.take();
        enter(&mut t, survivor);
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
        assert!(t.take_writes().is_empty());
        assert!(
            pages_to(&t.take(), 1)
                .last()
                .is_some_and(|p| p.contains("32498-02"))
        );
    }

    #[test]
    fn lematan_sails_off_at_half_health_and_calls_his_followers() {
        let (mut t, survivor) = setup();
        with_quest(&mut t, 1, 1);
        enter(&mut t, survivor);
        let lematan = lematan(&t);

        // Above half: only the ground skills loop starts, once.
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_attack(ctx, lematan, Oid(1), 5, false, None);
            PailakaDevilsLegacy.on_attack(ctx, lematan, Oid(1), 5, false, None);
        });
        assert_eq!(t.world.quest_timer_count(), 1);

        // Below half: core AI off, value 1, the walk to the port point and its retry.
        t.world.set_current_hp(lematan.object_id, 400.0, false);
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_attack(ctx, lematan, Oid(1), 5, false, None);
            assert!(ctx.has_quest_timer("LEMATAN_MOVE_RETRY", Some(lematan), None));
        });

        // He arrives: the teleport skill shows and the teleport follows two seconds later.
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_move_finished(ctx, lematan);
            assert!(ctx.has_quest_timer("LEMATAN_TELEPORT", Some(lematan), None));
            // A second arrival does nothing: the value is 2 now.
            PailakaDevilsLegacy.on_move_finished(ctx, lematan);
        });
        t.ticks(41);
        let world = t.world.instance_world(300_000).unwrap();
        let followers = world.parameters["followerslist"].split(',').count();
        assert_eq!(followers, 8);
        let (x, y, _) = t.world.position_of(lematan.object_id).unwrap();
        assert!((x - LEMATAN_PORT.0).abs() < 50 && (y - LEMATAN_PORT.1).abs() < 50);

        // Killing him removes the followers and brings the Dwarf Adventurer.
        let ids: Vec<Oid> = world.parameters["followerslist"]
            .split(',')
            .map(|i| Oid(i.parse().unwrap()))
            .collect();
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_kill(ctx, lematan, Oid(1), false);
        });
        assert!(ids.iter().all(|f| t.world.npc(*f).is_none()));
        assert!(
            !t.world
                .instance_world(300_000)
                .unwrap()
                .parameters
                .contains_key("followerslist")
        );
        assert!(
            t.world
                .instance(300_000)
                .unwrap()
                .npcs
                .keys()
                .any(|n| t.world.npc(*n).is_some_and(|n| n.npc_id == ADVENTURER2))
        );
    }

    #[test]
    fn a_powder_keg_hurts_the_monsters_around_it_once() {
        let (mut t, survivor) = setup();
        with_quest(&mut t, 1, 1);
        enter(&mut t, survivor);
        let spawn = |t: &mut ScriptTest, id: i32, dx: i32| {
            t.world
                .script_add_spawn(id, (X + dx, Y, Z), 0, false, 0, 300_000)
                .unwrap()
        };
        let keg = spawn(&mut t, POWDER_KEG, 100);
        let guard = spawn(&mut t, 18700, 150);
        let far = spawn(&mut t, 18700, 2000);
        let npc = ScriptNpc {
            object_id: keg,
            id: POWDER_KEG,
        };
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_attack(ctx, npc, Oid(1), 0, false, None);
        });
        assert_eq!(t.world.quest_timer_count(), 0, "no damage, no explosion");
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_attack(ctx, npc, Oid(1), 7, false, None);
            PailakaDevilsLegacy.on_attack(ctx, npc, Oid(1), 7, false, None);
        });
        let hurt = |id: Oid| 5000.0 - t.world.creature_hp(id).unwrap().0;
        assert!((500.0..=700.0).contains(&hurt(guard)), "{}", hurt(guard));
        assert_eq!(hurt(far), 0.0);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(41);
        assert!(t.world.npc(keg).is_none());
    }

    #[test]
    fn a_treasure_box_drops_once_and_is_deleted() {
        let (mut t, survivor) = setup();
        with_quest(&mut t, 1, 1);
        enter(&mut t, survivor);
        let boxes: Vec<Oid> = (0..8)
            .map(|i| {
                t.world
                    .script_add_spawn(TREASURE_BOX, (X + 100 + 10 * i, Y, Z), 0, false, 0, 300_000)
                    .unwrap()
            })
            .collect();
        for b in &boxes {
            let npc = ScriptNpc {
                object_id: *b,
                id: TREASURE_BOX,
            };
            t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
                PailakaDevilsLegacy.on_attack(ctx, npc, Oid(1), 3, false, None);
                PailakaDevilsLegacy.on_attack(ctx, npc, Oid(1), 3, false, None);
            });
        }
        assert_eq!(t.world.quest_timer_count(), 8);
        assert!(!t.world.ground_item_ids().is_empty());
        t.ticks(61);
        assert!(boxes.iter().all(|b| t.world.npc(*b).is_none()));
    }

    #[test]
    fn entering_the_zone_sends_the_player_back_to_the_entry_point_of_the_open_world() {
        let (mut t, survivor) = setup();
        with_quest(&mut t, 1, 1);
        enter(&mut t, survivor);
        t.world.run_as_script("PailakaDevilsLegacy", |ctx| {
            PailakaDevilsLegacy.on_enter_zone(ctx, Oid(1), ZONE);
        });
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(21);
        // NOTE(java-quirk): instance 0.
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (TELEPORT.0, TELEPORT.1));
    }
}
