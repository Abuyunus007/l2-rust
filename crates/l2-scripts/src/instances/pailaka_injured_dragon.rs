//! Pailaka Injured Dragon (the Varka Silenos Barracks), instance 45: Latana, the walls of Varka
//! and the cinematics.
// Java: dist/game/data/scripts/instances/PailakaInjuredDragon/PailakaInjuredDragon.java
//
// Java keeps flags on the NPCs (`npc.getVariables()`, `npc.getScriptValue()`); here they are the
// script's state by object id ([`NpcVars`]).
//
// NOT ported (the world does not have it; see the S-INST-01 report):
// - `onCreatureSee` is ported but the engine never fires it yet (`CreatureSeeTaskManager`), so
//   the Latana scene only starts from `onAttack`.
// - `Latana.setLethalable(false)`: the world's lethal strike only spares raids and doors.

use std::collections::HashMap;

use l2_protocol_game::server::{AutoAttackStart, MagicSkillUse, SkillPosition, SpecialCamera};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

use super::ScriptValues;

// NPCs
const KETRA_ORC_SUPPORTER_2: i32 = 32512;

// Monsters
const LATANA: i32 = 18660;
const LATANA_SKILL_USE: i32 = 18661;
const INJURED_DRAGON_CAMERA_1: i32 = 18603;
const INJURED_DRAGON_CAMERA_2: i32 = 18604;
const INJURED_DRAGON_TARGET: i32 = 18605;

// Animals
const GRAZING_ANTELOPE: i32 = 18637;
const GRAZING_BANDERSNATCH: i32 = 18643;
const GRAZING_FLAVA: i32 = 18647;
const GRAZING_ELDER_ANTELOPE: i32 = 18651;

// Guards
const VARKA_SILENOS_RECRUIT: i32 = 18635;
const VARKA_SILENOS_FOOTMAN: i32 = 18636;
const VARKA_SILENOS_WARRIOR: i32 = 18642;
const VARKA_SILENOS_OFFICER: i32 = 18646;
const VARKA_SILENOS_GREAT_MAGUS: i32 = 18649;
const VARKA_SILENOS_GENERAL: i32 = 18650;
const VARKA_ELITE_GUARD: i32 = 18653;
const VARKA_COMMANDER: i32 = 18654;
const VARKA_HEAD_GUARD: i32 = 18655;
const PROPHET_GUARD: i32 = 18657;

// Wizards
const VARKA_SILENOS_SHAMAN: i32 = 18640;
const VARKA_SILENOS_PRIEST: i32 = 18641;
const VARKA_SILENOS_MEDIUM: i32 = 18644;
const VARKA_SILENOS_MAGUS: i32 = 18645;
const VARKA_SILENOS_SEER: i32 = 18648;
const VARKA_SILENOS_GREAT_SEER: i32 = 18652;
const VARKA_HEAD_MAGUS: i32 = 18656;
const DISCIPLE_OF_PROPHET: i32 = 18658;
const VARKA_PROPHET: i32 = 18659;

// Skills
const ANGER: SkillKey = SkillKey { id: 5718, level: 1 };
const PRESENTATION_THE_RISE_OF_LATANA: SkillKey = SkillKey { id: 5759, level: 1 };
const STUN: SkillKey = SkillKey { id: 5716, level: 1 };
const ELECTRIC_FLAME: SkillKey = SkillKey { id: 5715, level: 1 };
const FIRE_BREATH: SkillKey = SkillKey { id: 5717, level: 1 };

// Drops
const PAILAKA_INSTANT_SHIELD: i32 = 13032;
const QUICK_HEALING_POTION: i32 = 13033;

// Items
const SPEAR_OF_SILENOS: i32 = 13052;
const ENHANCED_SPEAR_OF_SILENOS: i32 = 13053;
const WEAPON_UPGRADE_STAGE_1: i32 = 13056;
const WEAPON_UPGRADE_STAGE_2: i32 = 13057;

// AI Parameters
const HP_ANGER_ACTIVATION: f64 = 0.3;

// Locations
const ENTRY_POINT: (i32, i32, i32) = (125738, -40933, -3770);
const TARGET_POINT: (i32, i32, i32) = (105465, -41817, -1768);

// Misc
const INSTANCE_ID: i32 = 45;
/// Java `Q00144_PailakaInjuredDragon.class.getSimpleName()`.
const Q00144: &str = "Q00144_PailakaInjuredDragon";

// Zones
const NOEXIT_ZONES: [(i32, [i32; 3]); 8] = [
    (200001, [123167, -45743, -3023]),
    (200002, [117783, -46398, -2560]),
    (200003, [116791, -51556, -2584]),
    (200004, [117993, -52505, -2480]),
    (200005, [113226, -44080, -2776]),
    (200006, [110326, -45016, -2444]),
    (200007, [118341, -55951, -2280]),
    (200008, [110127, -41562, -2332]),
];

/// What Java keeps in `npc.getVariables()` of Latana, her camera and the casting marker.
#[derive(Debug, Default)]
struct NpcVars {
    /// `i_ai0`: Anger is up (Latana) / the camera saw a player.
    i_ai0: bool,
    /// `i_ai2`: the scene has started.
    i_ai2: bool,
    /// `cinematic`.
    cinematic: bool,
    /// `c_ai1`: the marker she aims her opening skills at.
    c_ai1: Option<ObjectId>,
    /// `param1`, `param2` of the `LATANA_SKILL_USE` marker: Latana and the player.
    param1: Option<ScriptNpc>,
    param2: Option<ObjectId>,
}

#[derive(Default)]
struct State {
    values: ScriptValues,
    vars: HashMap<ObjectId, NpcVars>,
}

impl State {
    fn vars(&mut self, npc: ObjectId) -> &mut NpcVars {
        self.vars.entry(npc).or_default()
    }
}

/// Java `instances.PailakaInjuredDragon.PailakaInjuredDragon`.
pub struct PailakaInjuredDragon;

impl Script for PailakaInjuredDragon {
    fn name(&self) -> &'static str {
        "PailakaInjuredDragon"
    }

    fn path(&self) -> &'static str {
        "instances/PailakaInjuredDragon"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[
            LATANA,
            LATANA_SKILL_USE,
            INJURED_DRAGON_CAMERA_1,
            INJURED_DRAGON_CAMERA_2,
        ]);
        s.add_creature_see_id(&[LATANA, INJURED_DRAGON_CAMERA_1]);
        s.add_attack_id(&[LATANA]);
        s.add_kill_id(&[LATANA]);
        s.add_kill_id(&[
            GRAZING_ANTELOPE,
            GRAZING_BANDERSNATCH,
            GRAZING_FLAVA,
            GRAZING_ELDER_ANTELOPE,
        ]);
        s.add_kill_id(&[
            VARKA_SILENOS_FOOTMAN,
            VARKA_SILENOS_RECRUIT,
            VARKA_SILENOS_WARRIOR,
            VARKA_ELITE_GUARD,
            VARKA_COMMANDER,
            VARKA_SILENOS_OFFICER,
            VARKA_SILENOS_GREAT_MAGUS,
            VARKA_SILENOS_GENERAL,
            VARKA_PROPHET,
            VARKA_HEAD_GUARD,
            PROPHET_GUARD,
        ]);
        let zones: Vec<i32> = NOEXIT_ZONES.iter().map(|(id, _)| *id).collect();
        s.add_enter_zone_id(&zones);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "enter" {
            if let Some(player) = player {
                enter_instance(ctx, player);
            }
            return None;
        }
        // Every other event is about an NPC (Java dereferences it).
        let npc = npc?;
        match event {
            "LATANA_500" => {
                // She stays on the ground until timer 502, which is the shot that lifts her.
                spawn_intro_target(ctx, npc);
                ctx.start_quest_timer("LATANA_501", 3000, Some(npc), player, false);
            }
            "LATANA_STANCE" => {
                // The client drops the standing pose whenever it redraws the npc,
                //  and the cinematic camera makes it redraw, so the pose has to be reasserted until the cinematic is over.
                keep_upright(ctx, npc);
                if ctx.state::<State>().vars(npc.object_id).cinematic {
                    ctx.start_quest_timer("LATANA_STANCE", 1500, Some(npc), player, false);
                }
            }
            "LATANA_501" => {
                ctx.start_quest_timer("LATANA_502", 3000, Some(npc), player, false);
            }
            "LATANA_502" => {
                // Skill 5759 is the one that lifts her, so she goes into combat here and stays up.
                cast_on_intro_target(ctx, npc, PRESENTATION_THE_RISE_OF_LATANA);
                ctx.world_mut().set_core_ai_disabled(npc.object_id, false);
                if let Some(player) = player {
                    ctx.add_attack_desire(npc, player, 1);
                }
                start_stance_loop(ctx, npc, player);
                ctx.start_quest_timer("LATANA_503", 9700, Some(npc), player, false);
            }
            "LATANA_503" => {
                keep_upright(ctx, npc);
                cast_on_intro_target(ctx, npc, STUN);
                ctx.start_quest_timer("LATANA_504", 6030, Some(npc), player, false);
            }
            "LATANA_504" => {
                keep_upright(ctx, npc);
                ctx.start_quest_timer("LATANA_505", 4000, Some(npc), player, false);
            }
            "LATANA_505" => {
                keep_upright(ctx, npc);
                ctx.start_quest_timer("LATANA_2000", 1000, Some(npc), player, false);
            }
            "LATANA_600" => {
                start_stance_loop(ctx, npc, player);
                spawn_intro_target(ctx, npc);
                ctx.start_quest_timer("LATANA_602", 2000, Some(npc), player, false);
            }
            "LATANA_602" => {
                // Electric Flame is an aura, Latana casts it on herself.
                ctx.cast_skill(npc, npc.object_id, ELECTRIC_FLAME);
                ctx.start_quest_timer("LATANA_603", 2500, Some(npc), player, false);
            }
            "LATANA_603" => {
                cast_on_intro_target(ctx, npc, STUN);
                ctx.start_quest_timer("LATANA_604", 6030, Some(npc), player, false);
            }
            "LATANA_604" => {
                ctx.start_quest_timer("LATANA_2000", 6000, Some(npc), player, false);
            }
            "LATANA_2000" => {
                // Both cinematic paths hand the fight back before this runs, but this loop is the one thing that always reaches her,
                //  so it repeats the handover rather than trust a chain of timers.
                ctx.world_mut().set_core_ai_disabled(npc.object_id, false);
                keep_upright(ctx, npc);

                match player {
                    None => {
                        ctx.start_quest_timer("LATANA_2000", 3000, Some(npc), None, false);
                    }
                    Some(player) => {
                        if distance_2d(ctx, npc.object_id, player) < 100.0 {
                            if ctx.get_random(100) < 30 {
                                ctx.cast_skill(npc, player, ELECTRIC_FLAME);
                            } else {
                                ctx.add_attack_desire(npc, player, 1000);
                            }
                        } else if ctx.get_random(100) < 50 {
                            // TODO: Implement check.
                            // if (npc.inMyTerritory(player)) {
                            let latan_skill_use = ctx.add_spawn_at(LATANA_SKILL_USE, player, false, 0);
                            if let Some(marker) = latan_skill_use {
                                let vars = ctx.state::<State>().vars(marker.object_id);
                                vars.param1 = Some(npc);
                                vars.param2 = Some(player);
                            }
                            // }
                        } else {
                            ctx.cast_skill(npc, player, FIRE_BREATH);
                        }

                        ctx.start_quest_timer("LATANA_2000", 6000, Some(npc), Some(player), false);
                    }
                }
            }
            "LATANA_4000" => {
                ctx.state::<State>().vars(npc.object_id).i_ai0 = false;
            }
            "LATANA_9000" => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.add_spawn(
                        KETRA_ORC_SUPPORTER_2,
                        x + 100,
                        y + 100,
                        z,
                        0,
                        false,
                        0,
                        instance_id,
                    );
                }
                ctx.world_mut().decay(npc.object_id);
            }
            "LATANA_SKILL_USE_1002" => {
                ctx.start_quest_timer("SCE_BOSS_2ND_SKILL", 2000, Some(npc), player, false);
                ctx.start_quest_timer("LATANA_SKILL_USE_2002", 5000, Some(npc), player, false);
            }
            "LATANA_SKILL_USE_2002" => {
                forget(ctx, npc);
                ctx.world_mut().delete_npc(npc.object_id);
            }
            // Cameras
            "INJURED_DRAGON_CAMERA_1_1000" => {
                camera(ctx, player, npc, [600, 200, 5, 0, 10000, -10, 8, 1, 1, 1]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1001", 2000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1001" => {
                camera(ctx, player, npc, [400, 200, 5, 4000, 10000, -10, 8, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1002", 4000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1002" => {
                camera(ctx, player, npc, [300, 195, 4, 1500, 10000, -5, 10, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1003", 1700, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1003" => {
                camera(ctx, player, npc, [130, 2, 5, 0, 10000, 0, 0, 1, 0, 1]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1004", 2000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1004" => {
                camera(ctx, player, npc, [220, 0, 4, 800, 10000, 5, 10, 1, 0, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1005", 2000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1005" => {
                camera(ctx, player, npc, [250, 185, 5, 4000, 10000, -5, 10, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1006", 4000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1006" => {
                camera(ctx, player, npc, [200, 0, 5, 2000, 10000, 0, 25, 1, 0, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_1007", 4530, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_1007" => {
                camera(ctx, player, npc, [300, -3, 5, 3500, 6000, 0, 6, 1, 0, 0]);
                end_cinematic(ctx, npc);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_9999", 10000, Some(npc), None, false);
            }
            "INJURED_DRAGON_CAMERA_1_2000" => {
                camera(ctx, player, npc, [250, 0, 6, 0, 10000, 2, 0, 1, 0, 1]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_2001", 2000, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_2001" => {
                camera(ctx, player, npc, [230, 0, 5, 2000, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_2002", 2500, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_2002" => {
                camera(ctx, player, npc, [180, 175, 2, 1500, 10000, 0, 10, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_1_2003", 1500, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_1_2003" => {
                camera(ctx, player, npc, [300, 180, 5, 1500, 3000, 0, 6, 1, 1, 0]);
                end_cinematic(ctx, npc);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_9999", 3000, Some(npc), None, false);
            }
            "INJURED_DRAGON_CAMERA_9999" => {
                forget(ctx, npc);
                ctx.world_mut().delete_npc(npc.object_id);
            }
            "INJURED_DRAGON_CAMERA_2_1000" => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                let first = ctx
                    .world()
                    .instance_world(instance_id)
                    .and_then(|w| w.allowed.first().copied());
                if let Some(plr) = first {
                    camera(ctx, Some(plr), npc, [450, 200, 3, 0, 10000, -15, 20, 1, 1, 1]);
                    ctx.start_quest_timer("INJURED_DRAGON_CAMERA_2_1001", 100, Some(npc), Some(plr), false);
                }
            }
            "INJURED_DRAGON_CAMERA_2_1001" => {
                camera(ctx, player, npc, [350, 200, 5, 5600, 10000, -15, 10, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_2_1002", 5600, Some(npc), player, false);
            }
            "INJURED_DRAGON_CAMERA_2_1002" => {
                camera(ctx, player, npc, [360, 200, 5, 1000, 2000, -15, 10, 1, 1, 0]);
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_9999", 10000, Some(npc), None, false);
            }
            "SCE_BOSS_2ND_SKILL" => {
                // Latana is the one casting. Retail aims at the marker, but the marker is a non attackable Folk, so the fan would resolve to nothing.
                //  It stands on the player, so aim there instead.
                let (latana, victim) = {
                    let vars = ctx.state::<State>().vars(npc.object_id);
                    (vars.param1, vars.param2)
                };
                if let (Some(latana), Some(victim)) = (latana, victim) {
                    if distance_2d(ctx, latana.object_id, npc.object_id) <= 900.0 {
                        ctx.cast_skill(latana, victim, STUN);
                    }
                }
            }
            "SCE_RATANA_CAMERA_START" => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                ctx.world().instance_world(instance_id)?;

                // The shots are framed from the camera NPC, which stands still, not from the moving boss.
                let camera = ctx
                    .world()
                    .instance_npc_by_template(instance_id, INJURED_DRAGON_CAMERA_1)
                    .map(|object_id| ScriptNpc {
                        object_id,
                        id: INJURED_DRAGON_CAMERA_1,
                    });
                let camera = camera?;

                match ctx.state::<State>().values.get(npc.object_id) {
                    1 => {
                        ctx.start_quest_timer(
                            "INJURED_DRAGON_CAMERA_1_1000",
                            10,
                            Some(camera),
                            player,
                            false,
                        );
                    }
                    2 => {
                        ctx.start_quest_timer(
                            "INJURED_DRAGON_CAMERA_1_2000",
                            10,
                            Some(camera),
                            player,
                            false,
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        None
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        match npc.id {
            LATANA => {
                if !ctx.state::<State>().vars(npc.object_id).i_ai2 && ctx.world().player(creature).is_some() {
                    let player = creature;
                    let Some(qs) = ctx.player_quest_state(player, Q00144) else {
                        return;
                    };

                    if qs.is_cond(4) {
                        ctx.start_quest_timer("LATANA_9000", 1000, Some(npc), Some(player), false);
                        ctx.state::<State>().vars(npc.object_id).i_ai0 = true;
                    } else {
                        ctx.play_sound(player, QuestSound::Bs08A);
                        ctx.state::<State>().values.set(npc.object_id, 1);
                        ctx.start_quest_timer(
                            "SCE_RATANA_CAMERA_START",
                            2000,
                            Some(npc),
                            Some(player),
                            false,
                        );
                        ctx.start_quest_timer("LATANA_500", 1000, Some(npc), Some(player), false);
                        ctx.state::<State>().vars(npc.object_id).i_ai2 = true;
                    }
                }
            }
            INJURED_DRAGON_CAMERA_1 => {
                if ctx.world().player(creature).is_some() && !ctx.state::<State>().vars(npc.object_id).i_ai0 {
                    ctx.state::<State>().vars(npc.object_id).i_ai0 = true;
                }
            }
            _ => {}
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if !ctx.state::<State>().vars(npc.object_id).i_ai2 {
            let Some(qs) = ctx.player_quest_state(player, Q00144) else {
                return;
            };

            if qs.is_cond(4) {
                ctx.start_quest_timer("LATANA_9000", 1000, Some(npc), Some(player), false);
                ctx.state::<State>().vars(npc.object_id).i_ai0 = true;
            } else {
                ctx.play_sound(player, QuestSound::Bs08A);
                ctx.state::<State>().values.set(npc.object_id, 2);
                ctx.start_quest_timer("SCE_RATANA_CAMERA_START", 4000, Some(npc), Some(player), false);
                ctx.start_quest_timer("LATANA_600", 1000, Some(npc), Some(player), false);
                ctx.state::<State>().vars(npc.object_id).i_ai2 = true;
            }
        }

        let low = ctx
            .world()
            .creature_hp(npc.object_id)
            .is_some_and(|(hp, max)| hp < max * HP_ANGER_ACTIVATION);
        if low && !ctx.state::<State>().vars(npc.object_id).i_ai0 {
            ctx.cast_skill(npc, player, ANGER);
            ctx.state::<State>().vars(npc.object_id).i_ai0 = true;
            ctx.start_quest_timer("LATANA_4000", 120000, Some(npc), Some(player), false);
        }
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        let live = {
            let world = ctx.world();
            world.player(creature).is_some() && !world.is_dead(creature) && !world.is_teleporting(creature)
        };
        if live {
            let instance_id = ctx
                .world()
                .instance_world_of(creature)
                .filter(|w| w.template_id == INSTANCE_ID)
                .map(|w| w.instance_id);
            if let Some(instance_id) = instance_id {
                // If a player wants to go by a mob wall without kill it, he will be returned back to a spawn point.
                let zone_teleport = NOEXIT_ZONES
                    .iter()
                    .find(|(id, _)| *id == zone_id)
                    .map(|(_, to)| *to);
                if let Some(zone_teleport) = zone_teleport {
                    let near = ctx.world().visible_objects_in_range(creature, 700);
                    for other in near {
                        let world = ctx.world();
                        if world.npc(other).is_none() {
                            continue;
                        }
                        if world.is_dead(other)
                            || world.is_object_invisible(other)
                            || !world.is_monster_npc(other)
                        {
                            continue;
                        }

                        ctx.world_mut().script_set_intention_idle(creature);
                        ctx.world_mut().set_instance_id(creature, instance_id);
                        let offset = ctx.world().max_offset_on_teleport();
                        ctx.world_mut().tele_to_location(
                            creature,
                            TeleportTarget::at(zone_teleport[0], zone_teleport[1], zone_teleport[2])
                                .with_random_offset(offset),
                        );
                        break;
                    }
                }
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            LATANA => {
                // `npc.setLethalable(false)`: not modelled by the world, see the module notes.
                ctx.world_mut().set_core_ai_disabled(npc.object_id, true);

                // The template carries her retail walk and run speeds, but retail never issues a pursuit: its combat loop only asks for melee once the player is already within 100 units, and no handler delegates to the parent class.
                //  Immobilizing her is how that reads here, and it puts the AI on the movementDisable path, which casts at range and melees what comes close.
                ctx.world_mut().set_immobilized(npc.object_id, true);
            }
            LATANA_SKILL_USE => {
                ctx.start_quest_timer("LATANA_SKILL_USE_1002", 10, Some(npc), None, false);
                let latana = ctx.state::<State>().vars(npc.object_id).param1;
                if let Some(latana) = latana {
                    // `(Creature) latana.getTarget()`.
                    if let Some(target) = ctx.world().creature_target(latana.object_id) {
                        ctx.add_attack_desire(latana, target, 10000000);
                    }
                }
            }
            INJURED_DRAGON_CAMERA_1 => {
                // startQuestTimer("INJURED_DRAGON_CAMERA_1_3000", 10, npc, null);
            }
            INJURED_DRAGON_CAMERA_2 => {
                ctx.start_quest_timer("INJURED_DRAGON_CAMERA_2_1000", 1, Some(npc), None, false);
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            LATANA => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                ctx.add_spawn(
                    INJURED_DRAGON_CAMERA_2,
                    105974,
                    -41794,
                    -1784,
                    32768,
                    false,
                    0,
                    instance_id,
                );
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.add_spawn(
                        KETRA_ORC_SUPPORTER_2,
                        x + 100,
                        y + 100,
                        z,
                        0,
                        false,
                        0,
                        instance_id,
                    );
                }
            }
            GRAZING_ANTELOPE | GRAZING_BANDERSNATCH | GRAZING_FLAVA | GRAZING_ELDER_ANTELOPE => {
                let item = if ctx.get_random_boolean() {
                    PAILAKA_INSTANT_SHIELD
                } else {
                    QUICK_HEALING_POTION
                };
                let count = i64::from(ctx.get_random_range(1, 10));
                ctx.world_mut()
                    .npc_drop_item(npc.object_id, Some(killer), item, count);
            }
            VARKA_SILENOS_FOOTMAN | VARKA_SILENOS_RECRUIT => {
                wall_kill(ctx, npc, killer, Upgrade::Stage1, VARKA_SILENOS_MEDIUM);
            }
            VARKA_SILENOS_WARRIOR => {
                wall_kill(ctx, npc, killer, Upgrade::Stage1, VARKA_SILENOS_PRIEST);
            }
            VARKA_ELITE_GUARD => {
                wall_kill(ctx, npc, killer, Upgrade::Stage1, VARKA_SILENOS_SHAMAN);
            }
            VARKA_COMMANDER | VARKA_SILENOS_OFFICER => {
                wall_kill(ctx, npc, killer, Upgrade::Stage1, VARKA_SILENOS_SEER);
            }
            VARKA_SILENOS_GREAT_MAGUS | VARKA_SILENOS_GENERAL => {
                wall_kill(ctx, npc, killer, Upgrade::Stage2, VARKA_SILENOS_MAGUS);
            }
            VARKA_PROPHET => {
                wall_kill(ctx, npc, killer, Upgrade::Stage2, DISCIPLE_OF_PROPHET);
            }
            VARKA_HEAD_GUARD => {
                wall_kill(ctx, npc, killer, Upgrade::Stage2, VARKA_HEAD_MAGUS);
            }
            PROPHET_GUARD => {
                wall_kill(ctx, npc, killer, Upgrade::Stage2, VARKA_SILENOS_GREAT_SEER);
            }
            _ => {}
        }
    }
}

/// Which weapon upgrade a Varka guard may give.
#[derive(Clone, Copy)]
enum Upgrade {
    Stage1,
    Stage2,
}

/// The common body of the Varka cases of `onKill`: the weapon upgrade (a quarter of the time, for
/// a player with the right spear and without the upgrade), the mage behind, the wall check.
fn wall_kill(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, upgrade: Upgrade, mage_id: i32) {
    let (spear, stage) = match upgrade {
        Upgrade::Stage1 => (SPEAR_OF_SILENOS, WEAPON_UPGRADE_STAGE_1),
        Upgrade::Stage2 => (ENHANCED_SPEAR_OF_SILENOS, WEAPON_UPGRADE_STAGE_2),
    };
    if ctx.has_quest_items(killer, spear) && !ctx.has_quest_items(killer, stage) && ctx.get_random(100) < 25 {
        ctx.give_items(killer, stage, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }

    // Spawns Mage Type silenos behind the one that was killed.
    spawn_mage_behind(ctx, npc, killer, mage_id);

    // Check if all the first row have been killed. Despawn mages.
    check_if_last_in_wall(ctx, npc);
}

/// Java `enterInstance(player, INSTANCE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().enter_instance(
        player,
        INSTANCE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
            }

            world.tele_to_location(
                player,
                TeleportTarget::at(ENTRY_POINT.0, ENTRY_POINT.1, ENTRY_POINT.2).with_instance(instance_id),
            );
        },
    );
}

/// The state of an NPC that is gone.
fn forget(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let state = ctx.state::<State>();
    state.values.forget(npc.object_id);
    state.vars.remove(&npc.object_id);
}

/// `npc.calculateDistance2D(other)`.
fn distance_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some(a), Some(b)) => {
            let (dx, dy) = (f64::from(a.0) - f64::from(b.0), f64::from(a.1) - f64::from(b.1));
            (dx * dx + dy * dy).sqrt()
        }
        _ => f64::MAX,
    }
}

/// `player.sendPacket(new SpecialCamera(npc, force, angle1, angle2, time, range, duration, relYaw,
/// relPitch, isWide, relAngle, unk))` as `[force, angle1, angle2, time, duration, relYaw, relPitch,
/// isWide, relAngle, unk]` (the `range` the constructor ignores is left out).
fn camera(ctx: &mut ScriptCtx<'_>, player: Option<ObjectId>, npc: ScriptNpc, a: [i32; 10]) {
    if let Some(player) = player {
        let packet = SpecialCamera::new(
            npc.object_id.0,
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[5],
            a[6],
            a[7],
            a[8],
            a[9],
        );
        ctx.world().send(player, &packet);
    }
}

// Spawns the marker Latana aims her opening skills at, and remembers it on her.
fn spawn_intro_target(ctx: &mut ScriptCtx<'_>, latana: ScriptNpc) {
    let instance_id = ctx.world().instance_id_of(latana.object_id);
    let target = ctx.add_spawn(
        INJURED_DRAGON_TARGET,
        TARGET_POINT.0,
        TARGET_POINT.1,
        TARGET_POINT.2,
        0,
        false,
        60000,
        instance_id,
    );
    ctx.state::<State>().vars(latana.object_id).c_ai1 = target.map(|t| t.object_id);
}

// The cinematic shots are display casts.
//  The marker is a non attackable Folk, so a real cast would resolve to an empty target list and every target type except AURA is dropped in that case.
fn cast_on_intro_target(ctx: &mut ScriptCtx<'_>, latana: ScriptNpc, skill: SkillKey) {
    let Some(target) = ctx.state::<State>().vars(latana.object_id).c_ai1 else {
        return;
    };
    if !ctx.world().is_object_spawned(target) {
        return;
    }

    ctx.world_mut()
        .set_creature_target(latana.object_id, Some(target));
    let hit_time = ctx.world().skill_hit_time(skill.id, skill.level);
    if let (Some(from), Some(to)) = (
        ctx.world().position_of(latana.object_id),
        ctx.world().position_of(target),
    ) {
        let packet = MagicSkillUse {
            creature_id: latana.object_id.0,
            target_id: target.0,
            skill_id: skill.id,
            skill_level: skill.level,
            hit_time,
            reuse_delay: 0,
            creature: SkillPosition {
                x: from.0,
                y: from.1,
                z: from.2,
            },
            ground_location: None,
            target: SkillPosition {
                x: to.0,
                y: to.1,
                z: to.2,
            },
        };
        ctx.world().broadcast_packet(latana.object_id, &packet);
    }
}

// Latana lies on the ground whenever she is out of combat stance.
//  The task keeps the server from calling the stance off, and the packet is what actually puts the client in the standing pose.
//  AbstractAI.clientStartAutoAttack refuses to send it while the core AI is disabled, hence the direct broadcast.
fn keep_upright(ctx: &mut ScriptCtx<'_>, latana: ScriptNpc) {
    ctx.world_mut().add_attack_stance_task(latana.object_id);
    ctx.world().broadcast_packet(
        latana.object_id,
        &AutoAttackStart {
            target_id: latana.object_id.0,
        },
    );
}

fn start_stance_loop(ctx: &mut ScriptCtx<'_>, latana: ScriptNpc, player: Option<ObjectId>) {
    ctx.state::<State>().vars(latana.object_id).cinematic = true;
    keep_upright(ctx, latana);
    ctx.start_quest_timer("LATANA_STANCE", 1500, Some(latana), player, false);
}

// The last camera shot is where the cinematic really ends, so that is where Latana takes over.
fn end_cinematic(ctx: &mut ScriptCtx<'_>, camera: ScriptNpc) {
    let instance_id = ctx.world().instance_id_of(camera.object_id);
    if ctx.world().instance_world(instance_id).is_none() {
        return;
    }

    let latana = ctx.world().instance_npc_by_template(instance_id, LATANA);
    if let Some(latana) = latana {
        let latana = ScriptNpc {
            object_id: latana,
            id: LATANA,
        };
        ctx.state::<State>().vars(latana.object_id).cinematic = false;
        ctx.world_mut().set_core_ai_disabled(latana.object_id, false);
        keep_upright(ctx, latana);
    }
}

// Spawns Mage Type silenos behind the one that was killed. Aggro against the player that kill the mob.
fn spawn_mage_behind(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, mage_id: i32) {
    let heading = ctx
        .world()
        .npc_spawn_heading(npc.object_id)
        .unwrap_or_else(|| ctx.world().object_heading(npc.object_id));
    // `LocationUtil.convertHeadingToDegree(heading)`.
    let rads = (f64::from(heading) / 182.044444444 + 180.0).to_radians();
    let Some((x, y, z)) = ctx.world().position_of(npc.object_id) else {
        return;
    };
    let mage_x = (f64::from(x) + (150.0 * rads.cos())) as i32;
    let mage_y = (f64::from(y) + (150.0 * rads.sin())) as i32;
    let instance_id = ctx.world().instance_id_of(npc.object_id);
    if let Some(mage_back) = ctx.add_spawn(mage_id, mage_x, mage_y, z, heading, false, 0, instance_id) {
        ctx.world_mut()
            .npc_notify_aggression(mage_back.object_id, player, 1000);
    }
}

/// The monsters of the wall the killed `id` stood in, and the mages behind them.
fn wall_of(id: i32) -> Option<(&'static [i32], &'static [i32])> {
    match id {
        VARKA_SILENOS_FOOTMAN | VARKA_SILENOS_RECRUIT | VARKA_SILENOS_WARRIOR => Some((
            &[
                VARKA_SILENOS_FOOTMAN,
                VARKA_SILENOS_RECRUIT,
                VARKA_SILENOS_WARRIOR,
            ],
            &[VARKA_SILENOS_MEDIUM, VARKA_SILENOS_PRIEST],
        )),
        VARKA_ELITE_GUARD | VARKA_COMMANDER | VARKA_SILENOS_OFFICER => Some((
            &[VARKA_ELITE_GUARD, VARKA_COMMANDER, VARKA_SILENOS_OFFICER],
            &[VARKA_SILENOS_SHAMAN, VARKA_SILENOS_SEER],
        )),
        VARKA_SILENOS_GREAT_MAGUS | VARKA_SILENOS_GENERAL | VARKA_PROPHET => Some((
            &[VARKA_SILENOS_GREAT_MAGUS, VARKA_SILENOS_GENERAL, VARKA_PROPHET],
            &[VARKA_SILENOS_MAGUS, DISCIPLE_OF_PROPHET],
        )),
        VARKA_HEAD_GUARD | PROPHET_GUARD => Some((
            &[VARKA_HEAD_GUARD, PROPHET_GUARD],
            &[VARKA_HEAD_MAGUS, VARKA_SILENOS_GREAT_SEER],
        )),
        _ => None,
    }
}

// This will check if there is other mob alive in this wall of mobs. If all mobs in the first row are dead then despawn the second row mobs, the mages.
fn check_if_last_in_wall(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let knowns: Vec<(ObjectId, i32)> = ctx
        .world()
        .visible_objects_in_range(npc.object_id, 700)
        .into_iter()
        .filter_map(|o| ctx.world().npc(o).map(|n| (o, n.npc_id)))
        .collect();
    let Some((first_row, mages)) = wall_of(npc.id) else {
        // No case of the Java switch: nothing to compare with, and nothing to despawn.
        return;
    };
    for (other, other_id) in &knowns {
        if ctx.world().is_dead(*other) {
            continue;
        }

        if first_row.contains(other_id) {
            return;
        }
    }

    // We did not find any mob on the first row alive, so despawn the second row mobs.
    for (other, other_id) in knowns {
        if ctx.world().is_dead(other) {
            continue;
        }

        if mages.contains(&other_id) {
            ctx.world_mut().abort_cast(other);
            ctx.world_mut().delete_npc(other);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ClientInput, QuestLoad, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z, names_to, player_data};

    const NPCS: &str = r#"
        <npc id="18660" type="Monster" level="60" name="Latana">
            <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
                <vitals hp="1000" hpRegen="2" mp="400" mpRegen="0.9" />
            </stats>
        </npc>
        <npc id="18603" type="Folk" level="60" name="Camera 1"/>
        <npc id="18604" type="Folk" level="60" name="Camera 2"/>
        <npc id="18605" type="Folk" level="60" name="Target"/>
        <npc id="18661" type="Folk" level="60" name="Latana Skill Use"/>
        <npc id="32512" type="Folk" level="60" name="Ketra Orc Supporter"/>
        <npc id="18636" type="Monster" level="60" name="Varka Silenos Footman"/>
        <npc id="18644" type="Monster" level="60" name="Varka Silenos Medium"/>
        <npc id="18637" type="Monster" level="60" name="Grazing Antelope"/>"#;
    const ITEMS: &str = r#"
        <item id="13052" type="EtcItem" name="Spear of Silenos"><set name="is_questitem" val="true"/></item>
        <item id="13056" type="EtcItem" name="Weapon Upgrade Stage 1"><set name="is_questitem" val="true"/></item>
        <item id="13032" type="EtcItem" name="Shield"/>
        <item id="13033" type="EtcItem" name="Healing Potion"/>"#;

    /// Java `Q00144_PailakaInjuredDragon` as far as the instance needs it: a quest of that name.
    struct Quest144;

    impl Script for Quest144 {
        fn name(&self) -> &'static str {
            Q00144
        }
        fn path(&self) -> &'static str {
            "quests/Q00144_PailakaInjuredDragon"
        }
        fn id(&self) -> i32 {
            144
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(PailakaInjuredDragon);
        registry.add(Quest144);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.world.set_instance_data(instance_data(
            &[(45, "Pailaka (Varka Silenos Barracks)")],
            &[(45, r#"<exitPoint x="143420" y="-59058" z="-3472" />"#)],
        ));
        t
    }

    /// Player 1 with Q00144 at `cond` (none: no quest state) enters the instance.
    fn enter(t: &mut ScriptTest, cond: Option<i32>) {
        let mut data = player_data(1, 70, vec![item_row(900, 13052)]);
        if let Some(cond) = cond {
            data.profile.quests = QuestLoad {
                states: vec![(Q00144.to_owned(), Some("Started".to_owned()))],
                variables: vec![(Q00144.to_owned(), "cond".to_owned(), Some(cond.to_string()))],
            };
        }
        t.enter_with(data);
        t.world.run_as_script("PailakaInjuredDragon", |ctx| {
            PailakaInjuredDragon.on_event(ctx, "enter", None, Some(Oid(1)));
        });
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        t.take();
    }

    fn item_row(object_id: i32, item_id: i32) -> l2_world::ItemData {
        crate::testing::item(object_id, item_id, 1)
    }

    fn spawn(t: &mut ScriptTest, id: i32, dx: i32) -> ScriptNpc {
        let object_id = t
            .world
            .script_add_spawn(id, (X + dx, Y, Z), 0, false, 0, 300_000)
            .unwrap();
        ScriptNpc { object_id, id }
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut State) -> R) -> R {
        let mut result = None;
        t.world.run_as_script("PailakaInjuredDragon", |ctx| {
            result = Some(f(ctx.state::<State>()))
        });
        result.unwrap()
    }

    fn run(t: &mut ScriptTest, f: impl FnOnce(&mut ScriptCtx<'_>)) {
        t.world.run_as_script("PailakaInjuredDragon", f);
    }

    #[test]
    fn enter_puts_the_player_into_instance_45_at_the_entry_point() {
        let mut t = setup();
        enter(&mut t, Some(3));
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
        let world = t.world.instance_world(300_000).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (45, vec![Oid(1)]));
        // `teleportPlayer(.., false)`: exactly the entry point (the teleport lifts z by 5).
        assert_eq!(
            t.world.position_of(Oid(1)).map(|p| (p.0, p.1)),
            Some((ENTRY_POINT.0, ENTRY_POINT.1))
        );
    }

    #[test]
    fn the_first_hit_on_latana_starts_the_scene_the_camera_shows_and_latana_takes_over() {
        let mut t = setup();
        enter(&mut t, Some(3));
        let latana = spawn(&mut t, LATANA, 300);
        let camera1 = spawn(&mut t, INJURED_DRAGON_CAMERA_1, 600);
        run(&mut t, |ctx| PailakaInjuredDragon.on_spawn(ctx, latana));

        // A hit without a quest state does nothing at all.
        let mut other = player_data(2, 70, vec![]);
        other.name = "P2".to_owned();
        t.enter_with(other);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(2), 5, false, None);
        });
        assert_eq!(t.world.quest_timer_count(), 0);

        // The player at cond 3: the sound, value 2, the two timers.
        t.take();
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(1), 5, false, None);
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(1), 5, false, None);
        });
        assert_eq!(names_to(&t.take(), 1), ["PLAY_SOUND"]);
        assert_eq!(t.world.quest_timer_count(), 2);
        assert_eq!(state(&mut t, |s| s.values.get(latana.object_id)), 2);
        assert!(state(&mut t, |s| s.vars(latana.object_id).i_ai2));

        // Four seconds in the camera NPC frames the four shots of path 2; they are packets to the player.
        t.ticks(20 * 14);
        let out = t.take();
        let shots = names_to(&out, 1)
            .iter()
            .filter(|n| *n == "SPECIAL_CAMERA")
            .count();
        assert_eq!(shots, 4);
        // The cinematic is over and she was handed the fight; the camera is deleted after 3 s.
        assert!(!state(&mut t, |s| s.vars(latana.object_id).cinematic));
        assert!(t.world.npc(camera1.object_id).is_none());
        // The marker she aimed at is the one at the target point (60 s despawn).
        assert!(state(&mut t, |s| s.vars(latana.object_id).c_ai1.is_some()));
        // The handover loop keeps running.
        assert!(t.world.quest_timer_count() >= 1);
    }

    #[test]
    fn at_cond_4_latana_gives_way_to_the_ketra_supporter() {
        let mut t = setup();
        enter(&mut t, Some(4));
        let latana = spawn(&mut t, LATANA, 300);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(1), 5, false, None);
        });
        assert!(state(&mut t, |s| s.vars(latana.object_id).i_ai0));
        t.ticks(21);
        assert!(!t.world.is_object_spawned(latana.object_id));
        assert_eq!(
            t.world
                .instance(300_000)
                .unwrap()
                .npcs
                .keys()
                .filter(|n| t
                    .world
                    .npc(**n)
                    .is_some_and(|n| n.npc_id == KETRA_ORC_SUPPORTER_2))
                .count(),
            1
        );
    }

    #[test]
    fn below_thirty_percent_latana_casts_anger_once_for_two_minutes() {
        let mut t = setup();
        enter(&mut t, Some(4));
        let latana = spawn(&mut t, LATANA, 300);
        run(&mut t, |ctx| {
            ctx.state::<State>().vars(latana.object_id).i_ai2 = true;
        });
        t.world.set_current_hp(latana.object_id, 200.0, false);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(1), 5, false, None);
            PailakaInjuredDragon.on_attack(ctx, latana, Oid(1), 5, false, None);
        });
        assert!(state(&mut t, |s| s.vars(latana.object_id).i_ai0));
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(120 * 20 + 1);
        assert!(!state(&mut t, |s| s.vars(latana.object_id).i_ai0));
    }

    #[test]
    fn a_footman_drops_the_upgrade_calls_a_mage_and_the_last_one_takes_the_mages_away() {
        let mut t = setup();
        enter(&mut t, Some(3));
        // Enough footmen that the 25% chance gives the upgrade once; it is given only once.
        let a = spawn(&mut t, VARKA_SILENOS_FOOTMAN, 200);
        let b = spawn(&mut t, VARKA_SILENOS_FOOTMAN, 230);
        run(&mut t, |ctx| PailakaInjuredDragon.on_kill(ctx, a, Oid(1), false));
        let medium = |t: &ScriptTest| {
            t.world
                .instance(300_000)
                .unwrap()
                .npcs
                .keys()
                .copied()
                .filter(|n| t.world.npc(*n).is_some_and(|n| n.npc_id == VARKA_SILENOS_MEDIUM))
                .collect::<Vec<_>>()
        };
        // B is alive: the mage stays, and it stands 150 behind A (heading 0 faces +x).
        let mages = medium(&t);
        assert_eq!(mages.len(), 1);
        let (ax, _, _) = t.world.position_of(a.object_id).unwrap();
        let (mx, _, _) = t.world.position_of(mages[0]).unwrap();
        assert_eq!(mx, ax - 150);

        // Both dead: the next kill finds no first row and removes the mages.
        t.world.set_dead(a.object_id, true);
        t.world.set_dead(b.object_id, true);
        run(&mut t, |ctx| PailakaInjuredDragon.on_kill(ctx, b, Oid(1), false));
        assert!(
            mages
                .iter()
                .all(|m| t.world.npc(*m).is_none() || t.world.is_dead(*m))
        );
        // The second kill spawned its own mage behind B, which the same check removed as well.
        assert!(
            medium(&t)
                .iter()
                .all(|m| !t.world.is_object_spawned(*m) || t.world.is_dead(*m))
        );
    }

    #[test]
    fn the_weapon_upgrade_comes_a_quarter_of_the_time() {
        let mut t = setup();
        enter(&mut t, Some(3));
        let mut given = 0;
        for i in 0..40 {
            let footman = spawn(&mut t, VARKA_SILENOS_FOOTMAN, 100 + i);
            run(&mut t, |ctx| {
                PailakaInjuredDragon.on_kill(ctx, footman, Oid(1), false)
            });
            if t.count(1, WEAPON_UPGRADE_STAGE_1) > 0 {
                given = t.count(1, WEAPON_UPGRADE_STAGE_1);
                break;
            }
        }
        // The player has the spear and gets the upgrade (once: `!hasQuestItems`).
        assert_eq!(given, 1);
        for i in 0..20 {
            let footman = spawn(&mut t, VARKA_SILENOS_FOOTMAN, 200 + i);
            run(&mut t, |ctx| {
                PailakaInjuredDragon.on_kill(ctx, footman, Oid(1), false)
            });
        }
        assert_eq!(t.count(1, WEAPON_UPGRADE_STAGE_1), 1);
    }

    #[test]
    fn a_grazing_animal_drops_a_potion() {
        let mut t = setup();
        enter(&mut t, Some(3));
        let antelope = spawn(&mut t, GRAZING_ANTELOPE, 100);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_kill(ctx, antelope, Oid(1), false)
        });
        let items = t.world.ground_item_ids();
        assert!(!items.is_empty());
        let id = t.world.ground_item(items[0]).unwrap().data.item_id;
        assert!(id == PAILAKA_INSTANT_SHIELD || id == QUICK_HEALING_POTION);
    }

    #[test]
    fn a_player_who_walks_into_a_mob_wall_zone_is_put_back() {
        let mut t = setup();
        enter(&mut t, Some(3));
        t.input(1, ClientInput::Appearing);
        let zone = NOEXIT_ZONES[0];
        // Nobody near: nothing happens.
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_enter_zone(ctx, Oid(1), zone.0)
        });
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        assert_eq!(
            t.world.position_of(Oid(1)).map(|p| (p.0, p.1)),
            Some((ENTRY_POINT.0, ENTRY_POINT.1))
        );
        // A live monster near him: sent to the zone's point (within the random offset).
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(300_000));
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        let footman = spawn(&mut t, VARKA_SILENOS_FOOTMAN, 100);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_enter_zone(ctx, Oid(1), zone.0)
        });
        t.ticks(60);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        let offset = t.world.max_offset_on_teleport();
        assert!((x - zone.1[0]).abs() <= offset && (y - zone.1[1]).abs() <= offset);
        // A dead one does not count.
        t.input(1, ClientInput::Appearing);
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(300_000));
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        t.world.set_dead(footman.object_id, true);
        run(&mut t, |ctx| {
            PailakaInjuredDragon.on_enter_zone(ctx, Oid(1), zone.0)
        });
        t.ticks(60);
        assert_eq!(t.world.position_of(Oid(1)).map(|p| (p.0, p.1)), Some((X, Y)));
    }
}
