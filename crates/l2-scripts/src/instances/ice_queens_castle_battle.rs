//! Ice Queen's Castle (Normal Battle) instance zone: the fight against Freya, in the normal
//! (139) and the hardcore (144) version.
// Java: dist/game/data/scripts/instances/IceQueensCastleBattle/IceQueensCastleBattle.java
//
// Java keeps the state of a run in the parameters of the `InstanceWorld` (`isHardCore`,
// `playersInside`, `controller`, `freya`, `canSpawnMobs`, `isSupportActive`); so does this port,
// as strings, because `onEnterInstance` runs inside `World::enter_instance` without a
// `ScriptCtx`. An NPC in a parameter is its object id.

use l2_protocol_game::server::{
    ActionFailed, ChatType, ExChangeClientEffectInfo, ExSendUiEvent, OnEventTrigger, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::mount::MountType;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget, World};

const Q10286_REUNION_WITH_SIRRA: &str = "Q10286_ReunionWithSirra";

// NPCs
const FREYA_THRONE: i32 = 29177; // First freya
const FREYA_SPELLING: i32 = 29178; // Second freya
const FREYA_STAND_EASY: i32 = 29179; // Last freya - Easy mode
const FREYA_STAND_HARD: i32 = 29180; // Last freya - Hardcore mode
const INVISIBLE_NPC: i32 = 18919;
const KNIGHT_EASY: i32 = 18855; // Archery Knight - Easy mode
const KNIGHT_HARD: i32 = 18856; // Archery Knight - Hardcore mode
const GLACIER: i32 = 18853; // Glacier
const BREATH: i32 = 18854; // Archer's Breath
const GLAKIAS_EASY: i32 = 25699; // Glakias (Archery Knight Captain) - Easy mode
const GLAKIAS_HARD: i32 = 25700; // Glakias (Archery Knight Captain) - Hardcore mode
const SIRRA: i32 = 32762; // Sirra
const JINIA: i32 = 32781; // Jinia
const SUPP_JINIA: i32 = 18850; // Jinia
const SUPP_KEGOR: i32 = 18851; // Kegor

// Skills
const ETERNAL_BLIZZARD: SkillKey = SkillKey { id: 6274, level: 1 }; // Eternal Blizzard
const ETERNAL_BLIZZARD_HARD: SkillKey = SkillKey { id: 6275, level: 1 }; // Eternal Blizzard Hard
const ETERNAL_BLIZZARD_FORCE: SkillKey = SkillKey { id: 6697, level: 1 }; // Eternal Blizzard Force
const BREATH_OF_ICE_PALACE: SkillKey = SkillKey { id: 6299, level: 1 }; // Breath of Ice Palace - Ice Storm
const SELF_DESTRUCTION: SkillKey = SkillKey { id: 6300, level: 1 }; // Self-Destruction
const JINIAS_PRAYER: SkillKey = SkillKey { id: 6288, level: 1 }; // Jinia's Prayer
const KEGORS_COURAGE: SkillKey = SkillKey { id: 6289, level: 1 }; // Kegor's Courage
const COLD_MANAS_FRAGMENT: SkillKey = SkillKey { id: 6301, level: 1 }; // Cold Mana's Fragment
const NPC_CANCEL_PC_TARGET: SkillKey = SkillKey { id: 4618, level: 1 }; // NPC Cancel PC Target
const POWER_STRIKE: SkillKey = SkillKey { id: 6293, level: 1 }; // Power Strike
const POINT_TARGET: SkillKey = SkillKey { id: 6295, level: 1 }; // Point Target
const CYLINDER_THROW: SkillKey = SkillKey { id: 6297, level: 1 }; // Cylinder Throw
const LEADERS_ROAR: SkillKey = SkillKey { id: 6294, level: 1 }; // Leader's Roar
const RUSH: SkillKey = SkillKey { id: 6296, level: 1 }; // Rush
const HINDER_STRIDER: SkillKey = SkillKey { id: 4258, level: 1 }; // Hinder Strider
const ICE_BALL: SkillKey = SkillKey { id: 6278, level: 1 }; // Ice Ball
const SUMMON_SPIRITS: SkillKey = SkillKey { id: 6277, level: 1 }; // Summon Spirits
const ATTACK_NEARBY_RANGE: SkillKey = SkillKey { id: 6279, level: 1 }; // Attack Nearby Range
const REFLECT_MAGIC: SkillKey = SkillKey { id: 6282, level: 1 }; // Reflect Magic
const RAGE_OF_ICE: SkillKey = SkillKey { id: 6285, level: 1 }; // Rage of Ice
const FREYA_BLESS: SkillKey = SkillKey { id: 6284, level: 1 }; // Freya's Bless

// Locations
/// `(x, y, z)` and, where Java has it, the heading.
type Loc = (i32, i32, i32);
const FREYA_SPAWN: (Loc, i32) = ((114720, -117085, -11088), 15956);
const FREYA_SPELLING_SPAWN: (Loc, i32) = ((114723, -117502, -10672), 15956);
const FREYA_CORPSE: Loc = (114767, -114795, -11200);
const MIDDLE_POINT: Loc = (114730, -114805, -11200);
const KEGOR_FINISH: Loc = (114659, -114796, -11205);
const GLAKIAS_SPAWN: (Loc, i32) = ((114707, -114799, -11199), 15956);
const SUPP_JINIA_SPAWN: Loc = (114751, -114781, -11205);
const SUPP_KEGOR_SPAWN: Loc = (114659, -114796, -11205);
const BATTLE_PORT: Loc = (114694, -113700, -11200);
const CONTROLLER_LOC: Loc = (114394, -112383, -11200);
const ENTER_LOC: [Loc; 6] = [
    (114185, -112435, -11210),
    (114183, -112280, -11210),
    (114024, -112435, -11210),
    (114024, -112278, -11210),
    (113865, -112435, -11210),
    (113865, -112276, -11210),
];
const STATUES_LOC: [(Loc, i32); 16] = [
    ((113845, -116091, -11168), 8264),
    ((113381, -115622, -11168), 8264),
    ((113380, -113978, -11168), -8224),
    ((113845, -113518, -11168), -8224),
    ((115591, -113516, -11168), -24504),
    ((116053, -113981, -11168), -24504),
    ((116061, -115611, -11168), 24804),
    ((115597, -116080, -11168), 24804),
    ((112942, -115480, -10960), 52),
    ((112940, -115146, -10960), 52),
    ((112945, -114453, -10960), 52),
    ((112945, -114123, -10960), 52),
    ((116497, -114117, -10960), 32724),
    ((116499, -114454, -10960), 32724),
    ((116501, -115145, -10960), 32724),
    ((116502, -115473, -10960), 32724),
];
const KNIGHTS_LOC: [(Loc, i32); 3] = [
    ((114502, -115315, -11205), 15451),
    ((114937, -115323, -11205), 18106),
    ((114722, -115185, -11205), 16437),
];

// Misc
const MAX_PLAYERS: usize = 27;
const MIN_PLAYERS: usize = 10;
const MIN_LEVEL: i32 = 82;
const GLAKIAS_KILL_TIMER: i64 = 6; // Harcore Glakias Kill Timer in minutes
const TEMPLATE_ID_EASY: i32 = 139; // Ice Queen's Castle
const TEMPLATE_ID_HARD: i32 = 144; // Ice Queen's Castle (Epic)
const DOOR_ID: i32 = 23140101;
const EMMITERS: [i32; 7] = [
    23140202, 23140204, 23140206, 23140208, 23140212, 23140214, 23140216,
];

// Movie ids (`Movie.getClientId()`).
const SC_BOSS_FREYA_OPENING: i32 = 15;
const SC_BOSS_FREYA_PHASECH_A: i32 = 16;
const SC_BOSS_FREYA_PHASECH_B: i32 = 17;
const SC_BOSS_KEGOR_INTRUSION: i32 = 18;
const SC_BOSS_FREYA_ENDING_A: i32 = 19;
const SC_BOSS_FREYA_ENDING_B: i32 = 20;
const SC_BOSS_FREYA_DEFEAT: i32 = 22;
const SC_ICE_HEAVYKNIGHT_SPAWN: i32 = 23;

/// Java `instances.IceQueensCastleBattle.IceQueensCastleBattle`.
pub struct IceQueensCastleBattle;

impl Script for IceQueensCastleBattle {
    fn name(&self) -> &'static str {
        "IceQueensCastleBattle"
    }

    fn path(&self) -> &'static str {
        "instances/IceQueensCastleBattle"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIRRA, SUPP_KEGOR, SUPP_JINIA]);
        s.add_first_talk_id(&[SUPP_KEGOR, SUPP_JINIA]);
        s.add_talk_id(&[SIRRA, JINIA, SUPP_KEGOR]);
        s.add_attack_id(&[
            FREYA_THRONE,
            FREYA_STAND_EASY,
            FREYA_STAND_HARD,
            GLAKIAS_EASY,
            GLAKIAS_HARD,
            GLACIER,
            BREATH,
            KNIGHT_EASY,
            KNIGHT_HARD,
        ]);
        s.add_kill_id(&[
            GLAKIAS_EASY,
            GLAKIAS_HARD,
            FREYA_STAND_EASY,
            FREYA_STAND_HARD,
            KNIGHT_EASY,
            KNIGHT_HARD,
            GLACIER,
            BREATH,
        ]);
        s.add_spell_finished_id(&[GLACIER, BREATH]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "enterEasy" {
            enter_instance(ctx, player?, TEMPLATE_ID_EASY);
        } else if event == "enterHardcore" {
            enter_instance(ctx, player?, TEMPLATE_ID_HARD);
        } else {
            let npc = npc?;
            let instance_id = ctx
                .world()
                .instance_world_of(npc.object_id)
                .map(|w| w.instance_id);
            if let Some(instance_id) = instance_id {
                let controller = param_npc(ctx, instance_id, "controller");
                let freya = param_npc(ctx, instance_id, "freya");
                let hard = param_bool(ctx.world(), instance_id, "isHardCore", false);
                match event {
                    "openDoor" => {
                        if ctx.is_script_value(npc, 0) {
                            ctx.set_script_value(npc, 1);
                            ctx.open_door(DOOR_ID, instance_id);
                            let control = spawn(ctx, INVISIBLE_NPC, CONTROLLER_LOC, 0, instance_id);
                            for (loc, heading) in STATUES_LOC {
                                if loc.2 == -11168 {
                                    spawn(ctx, INVISIBLE_NPC, loc, heading, instance_id);
                                }
                            }

                            if !hard {
                                for players in players_inside(ctx.world(), instance_id) {
                                    if ctx.world().player(players).is_some()
                                        && !ctx.world().is_dead(players)
                                        && ctx.world().instance_id_of(players) == instance_id
                                    {
                                        // NOTE(java-quirk): the quest state asked is the one of
                                        // `player` (who clicked), not of `players`, once for every
                                        // player inside.
                                        let qs = player.and_then(|p| {
                                            ctx.quest_state_of(Q10286_REUNION_WITH_SIRRA, p, false)
                                        });
                                        if let Some(qs) = qs
                                            && qs.state(ctx) == STARTED
                                            && qs.is_cond(ctx, 5)
                                        {
                                            qs.set_cond(ctx, 6, true);
                                        }
                                    }
                                }
                            }

                            if let Some(control) = control {
                                param_set(
                                    ctx.world_mut(),
                                    instance_id,
                                    "controller",
                                    control.object_id.0.to_string(),
                                );
                            }
                            ctx.start_quest_timer("STAGE_1_MOVIE", 60000, control, None, false);
                        }
                    }
                    "portInside" => {
                        let player = player?;
                        teleport_player(ctx, player, BATTLE_PORT, instance_id, true);
                    }
                    "killFreya" => {
                        let player = player?;
                        let qs = ctx.quest_state_of(Q10286_REUNION_WITH_SIRRA, player, false);
                        if let Some(qs) = qs
                            && qs.state(ctx) == STARTED
                            && qs.is_cond(ctx, 6)
                        {
                            qs.set_memo_state(ctx, 10);
                            qs.set_cond(ctx, 7, true);
                        }

                        // NOTE(java-quirk): `world.getNpc(SUPP_KEGOR)` and `freya` are used unchecked.
                        let kegor = ctx.world().instance_npc_by_template(instance_id, SUPP_KEGOR)?;
                        ctx.world_mut().delete_npc(kegor);
                        ctx.world_mut().decay(freya?.object_id);
                        manage_movie(ctx, instance_id, SC_BOSS_FREYA_ENDING_B);
                        ctx.cancel_quest_timer("FINISH_WORLD", controller, None);
                        ctx.start_quest_timer("FINISH_WORLD", 58500, controller, None, false);
                    }
                    "18851-01.html" => {
                        return Some(event.to_owned());
                    }
                    "STAGE_1_MOVIE" => {
                        ctx.close_door(DOOR_ID, instance_id);
                        set_status(ctx, instance_id, 1);
                        manage_movie(ctx, instance_id, SC_BOSS_FREYA_OPENING);
                        ctx.start_quest_timer("STAGE_1_START", 53500, controller, None, false);
                    }
                    "STAGE_1_START" => {
                        let frey = spawn(ctx, FREYA_THRONE, FREYA_SPAWN.0, FREYA_SPAWN.1, instance_id)?;
                        ctx.world_mut().set_mortal(frey.object_id, false);
                        manage_screen_msg(ctx, instance_id, NpcStringId::BeginStage1);
                        ctx.start_quest_timer("CAST_BLIZZARD", 50000, controller, None, false);
                        param_set(
                            ctx.world_mut(),
                            instance_id,
                            "freya",
                            frey.object_id.0.to_string(),
                        );
                        ctx.start_quest_timer("STAGE_1_SPAWN", 2000, Some(frey), None, false);
                    }
                    "STAGE_1_SPAWN" => {
                        ctx.notify_event("START_SPAWN", controller, None);
                    }
                    "STAGE_1_FINISH" => {
                        if let Some(freya) = freya {
                            param_remove(ctx.world_mut(), instance_id, "freya");
                            ctx.world_mut().delete_npc(freya.object_id);
                            manage_despawn_minions(ctx, instance_id);
                            manage_movie(ctx, instance_id, SC_BOSS_FREYA_PHASECH_A);
                            ctx.start_quest_timer("STAGE_1_PAUSE", 24100 - 1000, controller, None, false);
                        }
                    }
                    "STAGE_1_PAUSE" => {
                        let frey = spawn(
                            ctx,
                            FREYA_SPELLING,
                            FREYA_SPELLING_SPAWN.0,
                            FREYA_SPELLING_SPAWN.1,
                            instance_id,
                        )?;
                        ctx.world_mut().set_invul(frey.object_id, true);
                        ctx.world_mut().set_npc_random_walking(frey.object_id, false);
                        ctx.world_mut().set_core_ai_disabled(frey.object_id, true);
                        manage_timer(ctx, instance_id, 60, NpcStringId::TimeRemainingUntilNextBattle);
                        set_status(ctx, instance_id, 2);
                        param_set(
                            ctx.world_mut(),
                            instance_id,
                            "freya",
                            frey.object_id.0.to_string(),
                        );
                        ctx.start_quest_timer("STAGE_2_START", 60000, controller, None, false);
                    }
                    "STAGE_2_START" => {
                        param_set(ctx.world_mut(), instance_id, "canSpawnMobs", "true".to_owned());
                        ctx.notify_event("START_SPAWN", controller, None);
                        manage_screen_msg(ctx, instance_id, NpcStringId::BeginStage2);
                        if hard {
                            ctx.start_quest_timer(
                                "STAGE_2_FAILED",
                                GLAKIAS_KILL_TIMER * 60 * 1000,
                                controller,
                                None,
                                false,
                            );
                            manage_timer(ctx, instance_id, 360, NpcStringId::BattleEndLimitTime);
                            let end = ctx.now_ms() + GLAKIAS_KILL_TIMER * 60 * 1000;
                            // NOTE(java-quirk): `controller` is used unchecked.
                            ctx.world_mut()
                                .npc_variables_mut(controller?.object_id)
                                .set_long("TIMER_END", end);
                        }
                    }
                    "STAGE_2_MOVIE" => {
                        manage_movie(ctx, instance_id, SC_ICE_HEAVYKNIGHT_SPAWN);
                        ctx.start_quest_timer("STAGE_2_GLAKIAS", 7000, controller, None, false);
                    }
                    "STAGE_2_GLAKIAS" => {
                        for (loc, heading) in STATUES_LOC {
                            if loc.2 == -10960 {
                                let statue = spawn(ctx, INVISIBLE_NPC, loc, heading, instance_id);
                                ctx.start_quest_timer("SPAWN_KNIGHT", 5000, statue, None, false);
                            }
                        }

                        let glakias = spawn(
                            ctx,
                            if hard { GLAKIAS_HARD } else { GLAKIAS_EASY },
                            GLAKIAS_SPAWN.0,
                            GLAKIAS_SPAWN.1,
                            instance_id,
                        );
                        ctx.start_quest_timer("LEADER_DELAY", 5000, glakias, None, false);
                        if hard {
                            ctx.start_quest_timer("SHOW_GLAKIAS_TIMER", 3000, controller, None, false);
                        }
                    }
                    "STAGE_2_FAILED" => {
                        manage_movie(ctx, instance_id, SC_BOSS_FREYA_DEFEAT);
                        ctx.start_quest_timer("STAGE_2_FAILED2", 22000, Some(npc), None, false);
                    }
                    "STAGE_2_FAILED2" => {
                        ctx.world_mut().finish_instance(instance_id);
                    }
                    "STAGE_3_MOVIE" => {
                        ctx.world_mut().delete_npc(freya?.object_id);
                        manage_movie(ctx, instance_id, SC_BOSS_FREYA_PHASECH_B);
                        ctx.start_quest_timer("STAGE_3_START", 21500, controller, None, false);
                    }
                    "STAGE_3_START" => {
                        for players in players_inside(ctx.world(), instance_id) {
                            if ctx.world().player(players).is_some() {
                                ctx.world()
                                    .broadcast_player_packet(players, &ExChangeClientEffectInfo::STATIC_FREYA_DESTROYED);

                                for emmiter_id in EMMITERS {
                                    ctx.world().send(
                                        players,
                                        &OnEventTrigger {
                                            emitter_id: emmiter_id,
                                            enabled: true,
                                        },
                                    );
                                }
                            }
                        }

                        let frey = spawn(
                            ctx,
                            if hard { FREYA_STAND_HARD } else { FREYA_STAND_EASY },
                            FREYA_SPAWN.0,
                            FREYA_SPAWN.1,
                            instance_id,
                        )?;
                        set_status(ctx, instance_id, 4);
                        param_set(ctx.world_mut(), instance_id, "canSpawnMobs", "true".to_owned());
                        param_set(
                            ctx.world_mut(),
                            instance_id,
                            "freya",
                            frey.object_id.0.to_string(),
                        );
                        // NOTE(java-quirk): `controller` is used unchecked.
                        let controller_id = controller?.object_id;
                        ctx.world_mut()
                            .npc_variables_mut(controller_id)
                            .set_int("FREYA_MOVE", 0);
                        ctx.notify_event("START_SPAWN", controller, None);
                        ctx.start_quest_timer("START_MOVE", 10000, controller, None, false);
                        ctx.start_quest_timer("CAST_BLIZZARD", 50000, controller, None, false);
                        manage_screen_msg(ctx, instance_id, NpcStringId::BeginStage3);
                        if hard {
                            ctx.world_mut().do_cast(frey.object_id, RAGE_OF_ICE);
                            ctx.start_quest_timer("FREYA_BUFF", 15000, controller, None, false);
                        }
                    }
                    "FREYA_BUFF" => {
                        ctx.world_mut().do_cast(freya?.object_id, FREYA_BLESS);
                        ctx.start_quest_timer("FREYA_BUFF", 15000, controller, None, false);
                    }
                    "START_MOVE" => {
                        let freya_move = ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .map_or(0, |v| v.get_int("FREYA_MOVE", 0));
                        if freya_move == 0 {
                            ctx.world_mut()
                                .npc_variables_mut(controller?.object_id)
                                .set_int("FREYA_MOVE", 1);
                            let freya = freya?;
                            if !ctx.is_in_combat(freya.object_id) {
                                ctx.world_mut().set_npc_running(freya.object_id);
                                move_to(ctx, freya.object_id, MIDDLE_POINT);
                            }
                        }
                    }
                    "CAST_BLIZZARD" => {
                        if let Some(freya) = freya
                            && !ctx.world().is_invul(freya.object_id)
                        {
                            let controller_id = controller?.object_id;
                            let blizzard_force_count = ctx
                                .world()
                                .npc_variables(controller_id)
                                .map_or(0, |v| v.get_int("BLIZZARD_FORCE_COUNT", 0));
                            if hard
                                && blizzard_force_count < 4
                                && ctx.npc_current_hp(freya)
                                    < f64::from(ctx.npc_max_hp(freya))
                                        * (0.8 - (0.2 * f64::from(blizzard_force_count)))
                            {
                                ctx.world_mut()
                                    .npc_variables_mut(controller_id)
                                    .set_int("BLIZZARD_FORCE_COUNT", blizzard_force_count + 1);
                                ctx.world_mut().do_cast(freya.object_id, ETERNAL_BLIZZARD_FORCE);
                                ctx.start_quest_timer("MANA_BURN", 7000, controller, None, false);
                                manage_screen_msg(
                                    ctx,
                                    instance_id,
                                    NpcStringId::MagicPowerSoStrongThatItCouldMakeYouLoseYourMindCanBeFeltFromSomewhere,
                                );
                            } else {
                                let skill = if hard {
                                    ETERNAL_BLIZZARD_HARD
                                } else {
                                    ETERNAL_BLIZZARD
                                };
                                ctx.world_mut().do_cast(freya.object_id, skill);
                                manage_screen_msg(
                                    ctx,
                                    instance_id,
                                    NpcStringId::StrongMagicPowerCanBeFeltFromSomewhere,
                                );
                            }
                        }

                        let time = i64::from(if hard {
                            ctx.get_random_range(35, 40)
                        } else {
                            ctx.get_random_range(55, 60)
                        }) * 1000;
                        ctx.start_quest_timer("CAST_BLIZZARD", time, controller, None, false);
                        for minion in
                            instance_npcs(ctx, instance_id, &[BREATH, GLACIER, KNIGHT_EASY, KNIGHT_HARD])
                        {
                            if !ctx.world().is_dead(minion) && !ctx.is_in_combat(minion) {
                                manage_random_attack(ctx, instance_id, minion);
                            }
                        }
                    }
                    "SPAWN_SUPPORT" => {
                        for players in players_inside(ctx.world(), instance_id) {
                            ctx.world_mut().set_invul(players, false);
                        }

                        // NOTE(java-quirk): `freya` is used unchecked.
                        let freya = freya?;
                        ctx.world_mut().set_invul(freya.object_id, false);
                        ctx.world_mut().set_core_ai_disabled(freya.object_id, false);
                        manage_screen_msg(ctx, instance_id, NpcStringId::BeginStage4);

                        let jinia = spawn(ctx, SUPP_JINIA, SUPP_JINIA_SPAWN, 0, instance_id)?;
                        ctx.world_mut().set_npc_running(jinia.object_id);
                        ctx.world_mut().set_invul(jinia.object_id, true);
                        ctx.set_can_return_to_spawn_point(jinia, false);

                        let kegor = spawn(ctx, SUPP_KEGOR, SUPP_KEGOR_SPAWN, 0, instance_id)?;
                        ctx.world_mut().set_npc_running(kegor.object_id);
                        ctx.world_mut().set_invul(kegor.object_id, true);
                        ctx.set_can_return_to_spawn_point(kegor, false);

                        ctx.start_quest_timer("ATTACK_FREYA", 5000, Some(jinia), None, false);
                        ctx.start_quest_timer("ATTACK_FREYA", 5000, Some(kegor), None, false);
                        ctx.start_quest_timer("GIVE_SUPPORT", 1000, controller, None, false);
                    }
                    "GIVE_SUPPORT" => {
                        if param_bool(ctx.world(), instance_id, "isSupportActive", false) {
                            let jinia = ctx.world().instance_npc_by_template(instance_id, SUPP_JINIA)?;
                            ctx.world_mut().do_cast(jinia, JINIAS_PRAYER);
                            let kegor = ctx.world().instance_npc_by_template(instance_id, SUPP_KEGOR)?;
                            ctx.world_mut().do_cast(kegor, KEGORS_COURAGE);
                            ctx.start_quest_timer("GIVE_SUPPORT", 25000, controller, None, false);
                        }
                    }
                    "FINISH_STAGE" => {
                        let jinia = ctx.world().instance_npc_by_template(instance_id, SUPP_JINIA)?;
                        ctx.world_mut().delete_npc(jinia);
                        let freya = freya?.object_id;
                        let freya_instance = ctx.world().instance_id_of(freya);
                        ctx.world_mut().tele_to_location(
                            freya,
                            TeleportTarget::at(FREYA_CORPSE.0, FREYA_CORPSE.1, FREYA_CORPSE.2)
                                .with_instance(freya_instance),
                        );
                        let kegor = ctx.world().instance_npc_by_template(instance_id, SUPP_KEGOR)?;
                        let kegor_instance = ctx.world().instance_id_of(kegor);
                        ctx.world_mut().tele_to_location(
                            kegor,
                            TeleportTarget::at(KEGOR_FINISH.0, KEGOR_FINISH.1, KEGOR_FINISH.2)
                                .with_instance(kegor_instance),
                        );
                    }
                    "START_SPAWN" => {
                        for statue in knight_statues(ctx, instance_id, controller) {
                            ctx.notify_event("SPAWN_KNIGHT", script_npc(ctx, statue), None);
                        }

                        for (loc, heading) in KNIGHTS_LOC {
                            let id = if hard { KNIGHT_HARD } else { KNIGHT_EASY };
                            if let Some(knight) = spawn(ctx, id, loc, heading, instance_id) {
                                ctx.world_mut().set_core_ai_disabled(knight.object_id, true);
                                ctx.world_mut().set_npc_display_effect(knight.object_id, 1);
                                ctx.world_mut()
                                    .set_npc_spawn_point(knight.object_id, loc.0, loc.1, loc.2);
                                let delay = i64::from(ctx.get_random_range(2, 5)) * 1000;
                                ctx.start_quest_timer("ICE_RUPTURE", delay, Some(knight), None, false);
                            }
                        }

                        let status = ctx.world().instance_world(instance_id).map_or(0, |w| w.status());
                        for _ in 0..status {
                            ctx.notify_event("SPAWN_GLACIER", controller, None);
                        }
                    }
                    "SPAWN_KNIGHT" => {
                        if param_bool(ctx.world(), instance_id, "canSpawnMobs", true) {
                            let loc = random_middle_point(ctx);
                            let (position, heading) = location_of(ctx, npc.object_id);
                            let id = if hard { KNIGHT_HARD } else { KNIGHT_EASY };
                            if let Some(knight) = spawn(ctx, id, position, heading, instance_id) {
                                ctx.world_mut()
                                    .npc_variables_mut(knight.object_id)
                                    .set_int("SPAWNED_NPC", npc.object_id.0);
                                ctx.world_mut().set_core_ai_disabled(knight.object_id, true);
                                ctx.set_immobilized(knight, true);
                                ctx.world_mut().set_npc_display_effect(knight.object_id, 1);
                                ctx.world_mut()
                                    .set_npc_spawn_point(knight.object_id, loc.0, loc.1, loc.2);

                                let time = i64::from(if hard {
                                    ctx.get_random_range(5, 10)
                                } else {
                                    ctx.get_random_range(15, 20)
                                }) * 1000;
                                ctx.start_quest_timer("ICE_RUPTURE", time, Some(knight), None, false);
                            }
                        }
                    }
                    "SPAWN_GLACIER" => {
                        if param_bool(ctx.world(), instance_id, "canSpawnMobs", true) {
                            let loc = random_middle_point(ctx);
                            if let Some(glacier) = spawn(ctx, GLACIER, loc, 0, instance_id) {
                                ctx.world_mut().set_npc_display_effect(glacier.object_id, 1);
                                ctx.world_mut().set_core_ai_disabled(glacier.object_id, true);
                                ctx.set_immobilized(glacier, true);
                                ctx.start_quest_timer("CHANGE_STATE", 1400, Some(glacier), None, false);
                            }
                        }
                    }
                    "ICE_RUPTURE" => {
                        if ctx.world().is_core_ai_disabled(npc.object_id) {
                            ctx.world_mut().set_core_ai_disabled(npc.object_id, false);
                            ctx.set_immobilized(npc, false);
                            ctx.world_mut().set_npc_display_effect(npc.object_id, 2);
                            manage_random_attack(ctx, instance_id, npc.object_id);
                        }
                    }
                    "FIND_TARGET" => {
                        manage_random_attack(ctx, instance_id, npc.object_id);
                    }
                    "CHANGE_STATE" => {
                        ctx.world_mut().set_npc_display_effect(npc.object_id, 2);
                        ctx.start_quest_timer("CAST_SKILL", 20000, Some(npc), None, false);
                    }
                    "CAST_SKILL" => {
                        if ctx.is_script_value(npc, 0) && !ctx.world().is_dead(npc.object_id) {
                            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                            ctx.world_mut().do_cast(npc.object_id, COLD_MANAS_FRAGMENT);
                            ctx.set_script_value(npc, 1);
                        }
                    }
                    "SUICIDE" => {
                        ctx.world_mut().set_npc_display_effect(npc.object_id, 3);
                        ctx.world_mut().set_mortal(npc.object_id, true);
                        ctx.world_mut().do_die(npc.object_id, None);
                    }
                    "BLIZZARD" => {
                        let count = var_int(ctx, npc.object_id, "SUICIDE_COUNT", 0) + 1;
                        set_var_int(ctx, npc.object_id, "SUICIDE_COUNT", count);
                        if var_int(ctx, npc.object_id, "SUICIDE_ON", 0) == 0 {
                            if count == 2 {
                                ctx.start_quest_timer("ELEMENTAL_SUICIDE", 20000, Some(npc), None, false);
                            } else {
                                if ctx
                                    .world_mut()
                                    .check_do_cast_conditions_of(npc.object_id, BREATH_OF_ICE_PALACE)
                                    && !ctx.world().is_casting_now(npc.object_id)
                                {
                                    ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                                    ctx.world_mut().do_cast(npc.object_id, BREATH_OF_ICE_PALACE);
                                }

                                ctx.start_quest_timer("BLIZZARD", 20000, Some(npc), None, false);
                            }
                        }
                    }
                    "ELEMENTAL_SUICIDE" => {
                        ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                        ctx.world_mut().do_cast(npc.object_id, SELF_DESTRUCTION);
                    }
                    "ELEMENTAL_KILLED" => {
                        if var_int(ctx, npc.object_id, "SUICIDE_ON", 0) == 1 {
                            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                            ctx.world_mut().do_cast(npc.object_id, SELF_DESTRUCTION);
                        }
                    }
                    "ATTACK_FREYA" => {
                        let skill = ctx.world().npc_template_skill_param(npc.object_id, "Skill01_ID");
                        // NOTE(java-quirk): `freya` and `skill` are used unchecked.
                        let freya = freya?;
                        if ctx.is_inside_radius_2d(npc.object_id, freya.object_id, 100) {
                            let skill = skill?;
                            if ctx.world_mut().check_do_cast_conditions_of(npc.object_id, skill)
                                && !ctx.world().is_casting_now(npc.object_id)
                            {
                                ctx.world_mut()
                                    .set_npc_target(npc.object_id, Some(freya.object_id));
                                ctx.world_mut().do_cast(npc.object_id, skill);
                                ctx.start_quest_timer("ATTACK_FREYA", 20000, Some(npc), None, false);
                            } else {
                                ctx.start_quest_timer("ATTACK_FREYA", 5000, Some(npc), None, false);
                            }
                        } else {
                            ctx.world_mut()
                                .script_start_follow(npc.object_id, freya.object_id);
                            ctx.start_quest_timer("ATTACK_FREYA", 5000, Some(npc), None, false);
                        }
                    }
                    "FINISH_WORLD" => {
                        if let Some(freya) = freya {
                            ctx.world_mut().decay(freya.object_id);
                        }

                        for players in players_inside(ctx.world(), instance_id) {
                            if ctx.world().player(players).is_some() {
                                ctx.world()
                                    .broadcast_player_packet(players, &ExChangeClientEffectInfo::STATIC_FREYA_DEFAULT);
                            }
                        }

                        ctx.world_mut().finish_instance(instance_id);
                    }
                    "LEADER_RANGEBUFF" => {
                        if ctx
                            .world_mut()
                            .check_do_cast_conditions_of(npc.object_id, LEADERS_ROAR)
                            && !ctx.world().is_casting_now(npc.object_id)
                        {
                            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                            ctx.world_mut().do_cast(npc.object_id, LEADERS_ROAR);
                        } else {
                            ctx.start_quest_timer("LEADER_RANGEBUFF", 30000, Some(npc), None, false);
                        }
                    }
                    "LEADER_RANDOMIZE" => {
                        ctx.world_mut().clear_aggro_list(npc.object_id);

                        for character in ctx.world().visible_objects_in_range(npc.object_id, 1000) {
                            if ctx.world().player(character).is_some() {
                                let hate = i64::from(ctx.get_random_range(10000, 20000));
                                ctx.world_mut().add_damage_hate(npc.object_id, character, 0, hate);
                            }
                        }
                        ctx.start_quest_timer("LEADER_RANDOMIZE", 25000, Some(npc), None, false);
                    }
                    "LEADER_DASH" => {
                        let most_hated = ctx.most_hated(npc);
                        // `getRandomBoolean()` is rolled first and always.
                        if ctx.get_random_boolean()
                            && !ctx.world().is_casting_now(npc.object_id)
                            && let Some(most_hated) = most_hated
                            && !ctx.world().is_dead(most_hated)
                            && ctx
                                .distance_3d(npc.object_id, most_hated)
                                .is_some_and(|d| d < 1000.0)
                        {
                            ctx.world_mut().set_npc_target(npc.object_id, Some(most_hated));
                            ctx.world_mut().do_cast(npc.object_id, RUSH);
                        }

                        ctx.start_quest_timer("LEADER_DASH", 10000, Some(npc), None, false);
                    }
                    "LEADER_DESTROY" => {
                        if var_int(ctx, npc.object_id, "OFF_SHOUT", 0) == 0 {
                            manage_screen_msg(
                                ctx,
                                instance_id,
                                NpcStringId::TheSpaceFeelsLikeItsGraduallyStartingToShake,
                            );

                            match ctx.get_random(4) {
                                0 => {
                                    ctx.broadcast_npc_say(
                                        npc,
                                        ChatType::Shout,
                                        NpcStringId::ArcherGiveYourBreathForTheIntruder,
                                        &[],
                                    );
                                }
                                1 => {
                                    ctx.broadcast_npc_say(
                                        npc,
                                        ChatType::Shout,
                                        NpcStringId::MyKnightsShowYourLoyalty,
                                        &[],
                                    );
                                }
                                2 => {
                                    ctx.broadcast_npc_say(
                                        npc,
                                        ChatType::Shout,
                                        NpcStringId::ICanTakeItNoLonger,
                                        &[],
                                    );
                                }
                                3 => {
                                    ctx.broadcast_npc_say(
                                        npc,
                                        ChatType::Shout,
                                        NpcStringId::ArcherHeedMyCall,
                                        &[],
                                    );
                                    for _ in 0..3 {
                                        let (position, heading) = location_of(ctx, npc.object_id);
                                        let Some(breath) = ctx.add_spawn(
                                            BREATH,
                                            position.0,
                                            position.1,
                                            position.2,
                                            heading,
                                            true,
                                            0,
                                            instance_id,
                                        ) else {
                                            continue;
                                        };
                                        ctx.world_mut().set_npc_running(breath.object_id);
                                        if let Some(most_hated) = ctx.most_hated(npc) {
                                            ctx.world_mut().add_damage_hate(
                                                breath.object_id,
                                                most_hated,
                                                0,
                                                999,
                                            );
                                            ctx.world_mut()
                                                .set_intention_attack(breath.object_id, most_hated);
                                        }
                                        ctx.start_quest_timer("BLIZZARD", 20000, Some(breath), None, false);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "LEADER_DELAY" => {
                        if var_int(ctx, npc.object_id, "DELAY_VAL", 0) == 0 {
                            set_var_int(ctx, npc.object_id, "DELAY_VAL", 1);
                        }
                    }
                    "SHOW_GLAKIAS_TIMER" => {
                        let end = ctx
                            .world()
                            .npc_variables(controller?.object_id)
                            .map_or(0, |v| v.get_long("TIMER_END", 0));
                        let time = ((end - ctx.now_ms()) / 1000) as i32;
                        manage_timer(ctx, instance_id, time, NpcStringId::BattleEndLimitTime);
                    }
                    "MANA_BURN" => {
                        for temp in players_inside(ctx.world(), instance_id) {
                            if ctx.world().player(temp).is_some()
                                && ctx.world().instance_id_of(temp) == instance_id
                            {
                                ctx.world_mut().set_current_mp(temp, 0.0, false);
                                ctx.world_mut().broadcast_status_update(temp);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let instance_id = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id);
        if let Some(instance_id) = instance_id {
            if npc.id == SUPP_JINIA {
                ctx.world().send(player, &ActionFailed);
                return None;
            } else if npc.id == SUPP_KEGOR {
                if param_bool(ctx.world(), instance_id, "isSupportActive", false) {
                    ctx.world().send(player, &ActionFailed);
                    return None;
                }

                return Some("18851.html".to_owned());
            }
        }

        ctx.world().send(player, &ActionFailed);
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        let controller = param_npc(ctx, instance_id, "controller");
        let freya = param_npc(ctx, instance_id, "freya");
        let id = npc.object_id;
        match npc.id {
            FREYA_THRONE => {
                // NOTE(java-quirk): `controller` and `freya` are used unchecked.
                let (Some(controller), Some(freya)) = (controller, freya) else {
                    return;
                };
                let status = ctx.world().instance_world(instance_id).map_or(0, |w| w.status());
                if var_int(ctx, controller.object_id, "FREYA_MOVE", 0) == 0 && status == 1 {
                    set_var_int(ctx, controller.object_id, "FREYA_MOVE", 1);
                    if !ctx.is_in_combat(freya.object_id) {
                        manage_screen_msg(ctx, instance_id, NpcStringId::FreyaHasStartedToMove);
                        ctx.world_mut().set_npc_running(freya.object_id);
                        move_to(ctx, freya.object_id, MIDDLE_POINT);
                    }
                }

                if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.02 {
                    ctx.notify_event("STAGE_1_FINISH", Some(controller), None);
                    ctx.cancel_quest_timer("CAST_BLIZZARD", Some(controller), None);
                } else {
                    hinder_strider(ctx, npc, attacker);

                    let most_hated = ctx.most_hated(npc);
                    let can_reach_most_hated = most_hated.is_some_and(|m| {
                        !ctx.world().is_dead(m) && ctx.distance_3d(id, m).is_some_and(|d| d <= 800.0)
                    });
                    if ctx.get_random(10000) < 3333 {
                        ranged_skill(ctx, npc, attacker, most_hated, can_reach_most_hated, ICE_BALL);
                    } else if ctx.get_random(10000) < 800 {
                        ranged_skill(
                            ctx,
                            npc,
                            attacker,
                            most_hated,
                            can_reach_most_hated,
                            SUMMON_SPIRITS,
                        );
                    } else if ctx.get_random(10000) < 1500
                        && !ctx.is_affected_by_skill(id, ATTACK_NEARBY_RANGE.id)
                        && ctx
                            .world_mut()
                            .check_do_cast_conditions_of(id, ATTACK_NEARBY_RANGE)
                        && !ctx.world().is_casting_now(id)
                    {
                        ctx.world_mut().set_npc_target(id, Some(id));
                        ctx.world_mut().do_cast(id, ATTACK_NEARBY_RANGE);
                    }
                }
            }
            FREYA_STAND_EASY | FREYA_STAND_HARD => {
                // NOTE(java-quirk): `controller` and `freya` are used unchecked.
                let (Some(controller), Some(freya)) = (controller, freya) else {
                    return;
                };
                if var_int(ctx, controller.object_id, "FREYA_MOVE", 0) == 0 {
                    set_var_int(ctx, controller.object_id, "FREYA_MOVE", 1);
                    if !ctx.is_in_combat(freya.object_id) {
                        ctx.world_mut().set_npc_running(freya.object_id);
                        move_to(ctx, freya.object_id, MIDDLE_POINT);
                    }
                }

                if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.2
                    && !param_bool(ctx.world(), instance_id, "isSupportActive", false)
                {
                    param_set(ctx.world_mut(), instance_id, "isSupportActive", "true".to_owned());
                    ctx.world_mut().set_invul(freya.object_id, true);
                    ctx.world_mut().set_core_ai_disabled(freya.object_id, true);
                    for players in players_inside(ctx.world(), instance_id) {
                        ctx.world_mut().set_invul(players, true);
                        ctx.world_mut().abort_attack(players);
                    }

                    manage_movie(ctx, instance_id, SC_BOSS_KEGOR_INTRUSION);
                    ctx.start_quest_timer("SPAWN_SUPPORT", 27000, Some(controller), None, false);
                }

                hinder_strider(ctx, npc, attacker);

                let most_hated = ctx.most_hated(npc);
                let can_reach_most_hated = most_hated.is_some_and(|m| {
                    !ctx.world().is_dead(m) && ctx.distance_3d(id, m).is_some_and(|d| d <= 800.0)
                });
                if ctx.get_random(10000) < 3333 {
                    ranged_skill(ctx, npc, attacker, most_hated, can_reach_most_hated, ICE_BALL);
                } else if ctx.get_random(10000) < 1333 {
                    ranged_skill(
                        ctx,
                        npc,
                        attacker,
                        most_hated,
                        can_reach_most_hated,
                        SUMMON_SPIRITS,
                    );
                } else if ctx.get_random(10000) < 1500 {
                    if !ctx.is_affected_by_skill(id, ATTACK_NEARBY_RANGE.id)
                        && ctx
                            .world_mut()
                            .check_do_cast_conditions_of(id, ATTACK_NEARBY_RANGE)
                        && !ctx.world().is_casting_now(id)
                    {
                        ctx.world_mut().set_npc_target(id, Some(id));
                        ctx.world_mut().do_cast(id, ATTACK_NEARBY_RANGE);
                    }
                } else if ctx.get_random(10000) < 1333
                    && !ctx.is_affected_by_skill(id, REFLECT_MAGIC.id)
                    && ctx.world_mut().check_do_cast_conditions_of(id, REFLECT_MAGIC)
                    && !ctx.world().is_casting_now(id)
                {
                    ctx.world_mut().set_npc_target(id, Some(id));
                    ctx.world_mut().do_cast(id, REFLECT_MAGIC);
                }
            }
            GLACIER => {
                if ctx.is_script_value(npc, 0)
                    && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.5
                {
                    ctx.world_mut().set_npc_target(id, Some(attacker));
                    ctx.world_mut().do_cast(id, COLD_MANAS_FRAGMENT);
                    ctx.set_script_value(npc, 1);
                }
            }
            BREATH => {
                if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc) / 20)
                    && var_int(ctx, id, "SUICIDE_ON", 0) == 0
                {
                    set_var_int(ctx, id, "SUICIDE_ON", 1);
                    ctx.start_quest_timer("ELEMENTAL_KILLED", 1000, Some(npc), None, false);
                }
            }
            KNIGHT_EASY | KNIGHT_HARD => {
                if ctx.world().is_core_ai_disabled(npc.object_id) {
                    manage_random_attack(ctx, instance_id, id);
                    ctx.world_mut().set_core_ai_disabled(id, false);
                    ctx.set_immobilized(npc, false);
                    ctx.world_mut().set_npc_display_effect(id, 2);
                    ctx.cancel_quest_timer("ICE_RUPTURE", Some(npc), None);
                }
            }
            GLAKIAS_EASY | GLAKIAS_HARD => {
                if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.02 {
                    if var_int(ctx, id, "OFF_SHOUT", 0) == 0 {
                        set_var_int(ctx, id, "OFF_SHOUT", 1);
                        set_var_int(ctx, id, "DELAY_VAL", 2);
                        ctx.world_mut().set_npc_target(id, Some(attacker));
                        ctx.world_mut().do_cast(id, NPC_CANCEL_PC_TARGET);
                    } else if var_int(ctx, id, "OFF_SHOUT", 0) == 1 {
                        ctx.world_mut().set_npc_target(id, Some(attacker));
                        ctx.world_mut().do_cast(id, NPC_CANCEL_PC_TARGET);
                    }
                } else if var_int(ctx, id, "OFF_SHOUT", 0) == 0 && var_int(ctx, id, "DELAY_VAL", 0) == 1 {
                    let most_hated = ctx.most_hated(npc);
                    let can_reach_most_hated = most_hated.is_some_and(|m| {
                        !ctx.world().is_dead(m) && ctx.distance_3d(id, m).is_some_and(|d| d < 1000.0)
                    });
                    if var_int(ctx, id, "TIMER_ON", 0) == 0 {
                        set_var_int(ctx, id, "TIMER_ON", 1);
                        let delay = i64::from(ctx.get_random_range(5, 30)) * 1000;
                        ctx.start_quest_timer("LEADER_RANGEBUFF", delay, Some(npc), None, false);
                        ctx.start_quest_timer("LEADER_RANDOMIZE", 25000, Some(npc), None, false);
                        ctx.start_quest_timer("LEADER_DASH", 5000, Some(npc), None, false);
                        ctx.start_quest_timer("LEADER_DESTROY", 60000, Some(npc), None, false);
                    }

                    if ctx.get_random(10000) < 2500 {
                        if ctx.get_random(10000) < 2500 {
                            if ctx.world_mut().check_do_cast_conditions_of(id, POWER_STRIKE)
                                && !ctx.world().is_casting_now(id)
                            {
                                ctx.world_mut().set_npc_target(id, Some(attacker));
                                ctx.world_mut().do_cast(id, POWER_STRIKE);
                            }
                        } else if ctx.world_mut().check_do_cast_conditions_of(id, POWER_STRIKE)
                            && !ctx.world().is_casting_now(id)
                            && can_reach_most_hated
                        {
                            let target = ctx.most_hated(npc);
                            ctx.world_mut().set_npc_target(id, target);
                            ctx.world_mut().do_cast(id, POWER_STRIKE);
                        }
                    } else if ctx.get_random(10000) < 1500 {
                        if ctx.get_random_boolean() {
                            if ctx.world_mut().check_do_cast_conditions_of(id, POINT_TARGET)
                                && !ctx.world().is_casting_now(id)
                            {
                                ctx.world_mut().set_npc_target(id, Some(attacker));
                                ctx.world_mut().do_cast(id, POINT_TARGET);
                            }
                        } else if ctx.world_mut().check_do_cast_conditions_of(id, POINT_TARGET)
                            && !ctx.world().is_casting_now(id)
                            && can_reach_most_hated
                        {
                            let target = ctx.most_hated(npc);
                            ctx.world_mut().set_npc_target(id, target);
                            ctx.world_mut().do_cast(id, POINT_TARGET);
                        }
                    } else if ctx.get_random(10000) < 1500 {
                        if ctx.get_random_boolean() {
                            if ctx.world_mut().check_do_cast_conditions_of(id, CYLINDER_THROW)
                                && !ctx.world().is_casting_now(id)
                            {
                                ctx.world_mut().set_npc_target(id, Some(attacker));
                                ctx.world_mut().do_cast(id, CYLINDER_THROW);
                            }
                        } else if ctx.world_mut().check_do_cast_conditions_of(id, CYLINDER_THROW)
                            && !ctx.world().is_casting_now(id)
                            && can_reach_most_hated
                        {
                            let target = ctx.most_hated(npc);
                            ctx.world_mut().set_npc_target(id, target);
                            ctx.world_mut().do_cast(id, CYLINDER_THROW);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        match npc.id {
            GLACIER => {
                if skill == COLD_MANAS_FRAGMENT {
                    if ctx.get_random(100) < 75 {
                        let (position, heading) = location_of(ctx, npc.object_id);
                        let breath = ctx.add_spawn(
                            BREATH,
                            position.0,
                            position.1,
                            position.2,
                            heading,
                            false,
                            0,
                            instance_id,
                        );
                        if let Some(breath) = breath {
                            if let Some(player) = player {
                                ctx.world_mut().set_npc_running(breath.object_id);
                                ctx.world_mut().add_damage_hate(breath.object_id, player, 0, 999);
                                ctx.world_mut().set_intention_attack(breath.object_id, player);
                            } else {
                                manage_random_attack(ctx, instance_id, breath.object_id);
                            }

                            ctx.start_quest_timer("BLIZZARD", 20000, Some(breath), None, false);
                        }
                    }

                    ctx.notify_event("SUICIDE", Some(npc), None);
                }
            }
            BREATH => {
                if skill == SELF_DESTRUCTION {
                    ctx.world_mut().do_die(npc.object_id, None);
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
        let controller = param_npc(ctx, instance_id, "controller");
        let hard = param_bool(ctx.world(), instance_id, "isHardCore", false);
        let status = ctx.world().instance_world(instance_id).map_or(0, |w| w.status());
        match npc.id {
            GLAKIAS_EASY | GLAKIAS_HARD => {
                manage_despawn_minions(ctx, instance_id);
                manage_timer(ctx, instance_id, 60, NpcStringId::TimeRemainingUntilNextBattle);
                ctx.cancel_quest_timer("STAGE_2_FAILED", controller, None);
                ctx.start_quest_timer("STAGE_3_MOVIE", 60000, controller, None, false);
            }
            FREYA_STAND_EASY | FREYA_STAND_HARD => {
                param_set(
                    ctx.world_mut(),
                    instance_id,
                    "isSupportActive",
                    "false".to_owned(),
                );
                manage_movie(ctx, instance_id, SC_BOSS_FREYA_ENDING_A);
                manage_despawn_minions(ctx, instance_id);
                ctx.world_mut().cancel_npc_decay_task(npc.object_id);
                let jinia = ctx
                    .world()
                    .instance_npc_by_template(instance_id, SUPP_JINIA)
                    .and_then(|n| script_npc(ctx, n));
                let kegor = ctx
                    .world()
                    .instance_npc_by_template(instance_id, SUPP_KEGOR)
                    .and_then(|n| script_npc(ctx, n));
                ctx.cancel_quest_timer("ATTACK_FREYA", jinia, None);
                ctx.cancel_quest_timer("ATTACK_FREYA", kegor, None);
                ctx.cancel_quest_timer("GIVE_SUPPORT", controller, None);
                ctx.cancel_quest_timer("CAST_BLIZZARD", controller, None);
                ctx.cancel_quest_timer("FREYA_BUFF", controller, None);
                ctx.start_quest_timer("FINISH_STAGE", 16000, controller, None, false);
                ctx.start_quest_timer("FINISH_WORLD", 300000, controller, None, false);
            }
            KNIGHT_EASY | KNIGHT_HARD => {
                let spawned_by = ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .map(|v| v.get_int("SPAWNED_NPC", 0))
                    .filter(|id| *id != 0)
                    .map(ObjectId)
                    .and_then(|id| script_npc(ctx, id));
                // NOTE(java-quirk): `controller` is used unchecked.
                let Some(controller) = controller else {
                    return;
                };
                let mut knight_count = var_int(ctx, controller.object_id, "KNIGHT_COUNT", 0);

                if var_int(ctx, controller.object_id, "FREYA_MOVE", 0) == 0 && status == 1 {
                    set_var_int(ctx, controller.object_id, "FREYA_MOVE", 1);
                    let Some(freya) = param_npc(ctx, instance_id, "freya") else {
                        return;
                    };
                    if !ctx.is_in_combat(freya.object_id) {
                        manage_screen_msg(ctx, instance_id, NpcStringId::FreyaHasStartedToMove);
                        ctx.world_mut().set_npc_running(freya.object_id);
                        move_to(ctx, freya.object_id, MIDDLE_POINT);
                    }
                }

                if knight_count < 10 && status == 2 {
                    knight_count += 1;
                    set_var_int(ctx, controller.object_id, "KNIGHT_COUNT", knight_count);
                    if knight_count == 10 {
                        ctx.notify_event("STAGE_2_MOVIE", Some(controller), None);
                        set_status(ctx, instance_id, 3);
                    }
                }

                if spawned_by.is_some() {
                    let time = i64::from(if hard {
                        ctx.get_random_range(30, 60)
                    } else {
                        ctx.get_random_range(50, 60)
                    }) * 1000;
                    ctx.start_quest_timer("SPAWN_KNIGHT", time, spawned_by, None, false);
                }
            }
            GLACIER => {
                let delay = i64::from(ctx.get_random_range(30, 60)) * 1000;
                ctx.start_quest_timer("SPAWN_GLACIER", delay, controller, None, false);
            }
            _ => {}
        }
    }
}

/// `npc.getAI()` hinders a Strider rider: the three-line block both Freyas share.
fn hinder_strider(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, attacker: ObjectId) {
    let id = npc.object_id;
    if ctx.world().mount_type_of(attacker) == MountType::Strider
        && !ctx.is_affected_by_skill(attacker, HINDER_STRIDER.id)
        && !ctx.world().is_casting_now(id)
        && !ctx.world_mut().is_skill_disabled_of(id, HINDER_STRIDER)
    {
        ctx.world_mut().set_npc_target(id, Some(attacker));
        ctx.world_mut().do_cast(id, HINDER_STRIDER);
    }
}

/// The Ice Ball / Summon Spirits blocks of Freya: half the time at the attacker (within 800),
/// half the time at the most hated one (when it is within 800).
fn ranged_skill(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    attacker: ObjectId,
    most_hated: Option<ObjectId>,
    can_reach_most_hated: bool,
    skill: SkillKey,
) {
    let id = npc.object_id;
    if ctx.get_random_boolean() {
        if ctx.distance_3d(id, attacker).is_some_and(|d| d <= 800.0)
            && ctx.world_mut().check_do_cast_conditions_of(id, skill)
            && !ctx.world().is_casting_now(id)
        {
            ctx.world_mut().set_npc_target(id, Some(attacker));
            ctx.world_mut().do_cast(id, skill);
        }
    } else if can_reach_most_hated
        && ctx.world_mut().check_do_cast_conditions_of(id, skill)
        && !ctx.world().is_casting_now(id)
    {
        ctx.world_mut().set_npc_target(id, most_hated);
        ctx.world_mut().do_cast(id, skill);
    }
}

// ---------------------------------------------------------------------- instance parameters

fn param(world: &World, instance_id: i32, key: &str) -> Option<String> {
    world
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(key).cloned())
}

/// `world.getParameters().getBoolean(key, default)`.
fn param_bool(world: &World, instance_id: i32, key: &str, default: bool) -> bool {
    param(world, instance_id, key).map_or(default, |v| v == "true")
}

/// `world.setParameter(key, value)`.
fn param_set(world: &mut World, instance_id: i32, key: &str, value: String) {
    if let Some(w) = world.instance_world_mut(instance_id) {
        w.parameters.insert(key.to_owned(), value);
    }
}

/// `world.setParameter(key, null)`.
fn param_remove(world: &mut World, instance_id: i32, key: &str) {
    if let Some(w) = world.instance_world_mut(instance_id) {
        w.parameters.remove(key);
    }
}

/// `params.getObject(key, Npc.class)`.
fn param_npc(ctx: &ScriptCtx<'_>, instance_id: i32, key: &str) -> Option<ScriptNpc> {
    let id = ObjectId(param(ctx.world(), instance_id, key)?.parse().ok()?);
    script_npc(ctx, id)
}

/// `params.getList("playersInside", Player.class)`.
fn players_inside(world: &World, instance_id: i32) -> Vec<ObjectId> {
    param(world, instance_id, "playersInside")
        .map(|list| {
            list.split(',')
                .filter_map(|id| id.parse().ok())
                .map(ObjectId)
                .collect()
        })
        .unwrap_or_default()
}

fn set_status(ctx: &mut ScriptCtx<'_>, instance_id: i32, status: i32) {
    if let Some(w) = ctx.world_mut().instance_world_mut(instance_id) {
        w.set_status(status);
    }
}

// ---------------------------------------------------------------------- npcs

/// The `ScriptNpc` of an NPC in the world.
fn script_npc(ctx: &ScriptCtx<'_>, id: ObjectId) -> Option<ScriptNpc> {
    ctx.world().npc(id).map(|n| ScriptNpc {
        object_id: id,
        id: n.npc_id,
    })
}

/// `addSpawn(npcId, loc, false, 0, isSummonSpawn, instanceId)`.
fn spawn(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    loc: Loc,
    heading: i32,
    instance_id: i32,
) -> Option<ScriptNpc> {
    ctx.add_spawn(npc_id, loc.0, loc.1, loc.2, heading, false, 0, instance_id)
}

/// `npc.getLocation()`: the place and the heading.
fn location_of(ctx: &ScriptCtx<'_>, id: ObjectId) -> (Loc, i32) {
    (
        ctx.world().position_of(id).unwrap_or_default(),
        ctx.world().heading_of(id),
    )
}

/// `npc.getAI().setIntentionMoveTo(loc)`.
fn move_to(ctx: &mut ScriptCtx<'_>, id: ObjectId, loc: Loc) {
    ctx.world_mut().npc_move_to_location(id, loc.0, loc.1, loc.2);
}

/// `new Location(MIDDLE_POINT.getX() + getRandom(-1000, 1000), MIDDLE_POINT.getY() +
/// getRandom(-1000, 1000), MIDDLE_POINT.getZ())`.
fn random_middle_point(ctx: &mut ScriptCtx<'_>) -> Loc {
    let x = MIDDLE_POINT.0 + ctx.get_random_range(-1000, 1000);
    let y = MIDDLE_POINT.1 + ctx.get_random_range(-1000, 1000);
    (x, y, MIDDLE_POINT.2)
}

/// `npc.getVariables().getInt(name, default)`.
fn var_int(ctx: &ScriptCtx<'_>, npc: ObjectId, name: &str, default: i32) -> i32 {
    ctx.world()
        .npc_variables(npc)
        .map_or(default, |v| v.get_int(name, default))
}

/// `npc.getVariables().set(name, value)`.
fn set_var_int(ctx: &mut ScriptCtx<'_>, npc: ObjectId, name: &str, value: i32) {
    ctx.world_mut().npc_variables_mut(npc).set_int(name, value);
}

/// `world.getNpcs(ids)`: the NPCs of the instance with one of these templates.
fn instance_npcs(ctx: &ScriptCtx<'_>, instance_id: i32, ids: &[i32]) -> Vec<ObjectId> {
    let world = ctx.world();
    let Some(instance) = world.instance(instance_id) else {
        return Vec::new();
    };
    instance
        .npcs
        .keys()
        .copied()
        .filter(|n| world.npc(*n).is_some_and(|npc| ids.contains(&npc.npc_id)))
        .collect()
}

/// Java `getKnightStatues(world)`: the invisible NPCs but the controller.
fn knight_statues(ctx: &ScriptCtx<'_>, instance_id: i32, controller: Option<ScriptNpc>) -> Vec<ObjectId> {
    let mut invis = instance_npcs(ctx, instance_id, &[INVISIBLE_NPC]);
    invis.retain(|n| Some(*n) != controller.map(|c| c.object_id));
    invis
}

// ---------------------------------------------------------------------- the manage* helpers

/// Java `manageRandomAttack(world, mob)`.
fn manage_random_attack(ctx: &mut ScriptCtx<'_>, instance_id: i32, mob: ObjectId) {
    let players: Vec<ObjectId> = players_inside(ctx.world(), instance_id)
        .into_iter()
        .filter(|p| {
            let world = ctx.world();
            world.player(*p).is_some()
                && !world.is_dead(*p)
                && world.instance_id_of(*p) == instance_id
                && !world.is_object_invisible(*p)
        })
        .collect();
    // NOTE(java-quirk): `Collections.shuffle` uses an unseeded `java.util.Random`; the world RNG
    // draws the target here.
    let target = if players.is_empty() {
        None
    } else {
        let index = usize::try_from(ctx.get_random(players.len() as i32)).unwrap_or(0);
        players.get(index).copied()
    };
    if let Some(target) = target {
        ctx.world_mut().add_damage_hate(mob, target, 0, 999);
        ctx.world_mut().set_npc_running(mob);
        ctx.world_mut().set_intention_attack(mob, target);
    } else if let Some(mob) = script_npc(ctx, mob) {
        ctx.start_quest_timer("FIND_TARGET", 10000, Some(mob), None, false);
    }
}

/// Java `manageDespawnMinions(world)`.
fn manage_despawn_minions(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    param_set(ctx.world_mut(), instance_id, "canSpawnMobs", "false".to_owned());
    for mob in instance_npcs(ctx, instance_id, &[BREATH, GLACIER, KNIGHT_EASY, KNIGHT_HARD]) {
        if !ctx.world().is_dead(mob) && ctx.world().is_monster_npc(mob) {
            ctx.world_mut().do_die(mob, None);
        }
    }
}

/// Java `manageTimer(world, time, npcStringId)`.
fn manage_timer(ctx: &mut ScriptCtx<'_>, instance_id: i32, time: i32, npc_string_id: NpcStringId) {
    for players in players_inside(ctx.world(), instance_id) {
        if ctx.world().player(players).is_some() && ctx.world().instance_id_of(players) == instance_id {
            ctx.world().send(
                players,
                &ExSendUiEvent {
                    object_id: players.0,
                    hide: false,
                    count_up: false,
                    start_time: time,
                    end_time: 0,
                    npc_string_id: npc_string_id.id(),
                    params: Vec::new(),
                },
            );
        }
    }
}

/// Java `manageScreenMsg(world, stringId)`.
fn manage_screen_msg(ctx: &mut ScriptCtx<'_>, instance_id: i32, string_id: NpcStringId) {
    for players in players_inside(ctx.world(), instance_id) {
        if ctx.world().player(players).is_some() && ctx.world().instance_id_of(players) == instance_id {
            ctx.show_on_screen_npc_string(players, string_id, 2, 6000, &[]);
        }
    }
}

/// Java `manageMovie(world, movie)`.
fn manage_movie(ctx: &mut ScriptCtx<'_>, instance_id: i32, movie: i32) {
    for player in players_inside(ctx.world(), instance_id) {
        if ctx.world().player(player).is_some() && ctx.world().instance_id_of(player) == instance_id {
            ctx.play_movie(player, movie);
        }
    }
}

/// Java `teleportPlayer(player, loc, instanceId[, allowRandomOffset])`.
fn teleport_player(ctx: &mut ScriptCtx<'_>, player: ObjectId, loc: Loc, instance_id: i32, offset: bool) {
    ctx.teleport_player(player, loc.0, loc.1, loc.2, instance_id, offset);
}

// ---------------------------------------------------------------------- entering

/// Java `checkConditions(player)`.
fn check_conditions(world: &World, player: ObjectId, is_gm: bool) -> bool {
    if is_gm {
        return true;
    }

    let Some(party) = world.party_of(player) else {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    };

    let Some(channel) = world.command_channel_of_party(party) else {
        world.send(
            player,
            &SystemMessage::new(
                SystemMessageId::YouCannotEnterBecauseYouAreNotAssociatedWithTheCurrentCommandChannel,
            ),
        );
        return false;
    };

    if world.command_channel_leader(channel) != Some(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    let members = world.command_channel_members(channel);
    if members.len() < MIN_PLAYERS || members.len() > MAX_PLAYERS {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    // `party.broadcastPacket(sm)`: the party of the player, not the channel.
    let broadcast = |sm: &SystemMessage| {
        for member in world.party_members(party) {
            world.send(member, sm);
        }
    };
    for channel_member in members {
        let name = world
            .player(channel_member)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let level = world.player(channel_member).map_or(0, |p| p.core.level);
        if level < MIN_LEVEL {
            broadcast(
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        } else if !world.check_if_in_range(1000, player, channel_member, true) {
            broadcast(
                &SystemMessage::new(
                    SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        } else if world.instance_clock_ms() < world.instance_time(channel_member, TEMPLATE_ID_EASY)
            || world.instance_clock_ms() < world.instance_time(channel_member, TEMPLATE_ID_HARD)
        {
            broadcast(
                &SystemMessage::new(SystemMessageId::C1MayNotReEnterYet)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
    }

    true
}

/// Java `enterInstance(player, templateId)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32) {
    let is_gm = ctx.is_gm(player);
    ctx.world_mut().enter_instance(
        player,
        template_id,
        |world, player| check_conditions(world, player, is_gm),
        on_enter_instance,
    );
}

/// Java `onEnterInstance(player, world, firstEntrance)`.
fn on_enter_instance(world: &mut World, player: ObjectId, instance_id: i32, first_entrance: bool) {
    if first_entrance {
        let template = world.instance_world(instance_id).map_or(0, |w| w.template_id);
        param_set(
            world,
            instance_id,
            "isHardCore",
            (template == TEMPLATE_ID_HARD).to_string(),
        );

        let mut players_inside = Vec::new();
        match world.party_of(player) {
            None => {
                players_inside.push(player);
                manage_player_enter(world, player, instance_id);
            }
            Some(party) => {
                let members = match world.command_channel_of_party(party) {
                    Some(channel) => world.command_channel_members(channel),
                    None => world.party_members(party),
                };
                for member in members {
                    players_inside.push(member);
                    manage_player_enter(world, member, instance_id);
                }
            }
        }

        for players in &players_inside {
            if world.player(*players).is_some() {
                world.broadcast_player_packet(*players, &ExChangeClientEffectInfo::STATIC_FREYA_DEFAULT);

                for emmiter_id in EMMITERS {
                    world.send(
                        *players,
                        &OnEventTrigger {
                            emitter_id: emmiter_id,
                            enabled: false,
                        },
                    );
                }
            }
        }

        let list: Vec<String> = players_inside.iter().map(|p| p.0.to_string()).collect();
        param_set(world, instance_id, "playersInside", list.join(","));
    } else {
        let status = world.instance_world(instance_id).map_or(0, |w| w.status());
        let loc = if status == 4 {
            BATTLE_PORT
        } else {
            random_enter_loc(world)
        };
        let offset = world.max_offset_on_teleport();
        world.tele_to_location(
            player,
            TeleportTarget::at(loc.0, loc.1, loc.2)
                .with_instance(instance_id)
                .with_random_offset(offset),
        );
    }
}

/// `getRandomEntry(ENTER_LOC)`.
fn random_enter_loc(world: &mut World) -> Loc {
    let index = usize::try_from(world.rng().below(ENTER_LOC.len() as i32)).unwrap_or(0);
    ENTER_LOC[index.min(ENTER_LOC.len() - 1)]
}

/// Java `managePlayerEnter(player, world)`.
fn manage_player_enter(world: &mut World, player: ObjectId, instance_id: i32) {
    if let Some(instance_world) = world.instance_world_mut(instance_id) {
        instance_world.add_allowed(player);
    }
    let loc = random_enter_loc(world);
    world.tele_to_location(
        player,
        TeleportTarget::at(loc.0, loc.1, loc.2).with_instance(instance_id),
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{InstanceWorld, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NAME: &str = "IceQueensCastleBattle";
    const NPCS: &str = r#"
        <npc id="29177" type="GrandBoss" level="85" name="Freya"/>
        <npc id="29178" type="GrandBoss" level="85" name="Freya Spelling"/>
        <npc id="29179" type="GrandBoss" level="85" name="Freya Stand Easy"/>
        <npc id="29180" type="GrandBoss" level="85" name="Freya Stand Hard"/>
        <npc id="18919" type="Monster" level="85" name="Invisible"/>
        <npc id="18855" type="Monster" level="85" name="Archery Knight"/>
        <npc id="18856" type="Monster" level="85" name="Archery Knight Hard"/>
        <npc id="18853" type="Monster" level="85" name="Glacier"/>
        <npc id="18854" type="Monster" level="85" name="Archer's Breath"/>
        <npc id="25699" type="RaidBoss" level="85" name="Glakias"/>
        <npc id="25700" type="RaidBoss" level="85" name="Glakias Hard"/>
        <npc id="32762" type="Folk" level="85" name="Sirra"/>
        <npc id="18850" type="Folk" level="85" name="Jinia"/>
        <npc id="18851" type="Folk" level="85" name="Kegor"/>"#;

    /// Reunion with Sirra: a script of that name.
    struct StandIn;
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            Q10286_REUNION_WITH_SIRRA
        }
        fn path(&self) -> &'static str {
            "quests/Q10286_ReunionWithSirra"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn);
        registry.add(IceQueensCastleBattle);
        let pages = Pages::of(&IceQueensCastleBattle).plain(&["18851.html", "18851-01.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(139, "Ice Queen's Castle"), (144, "Ice Queen's Castle (Epic)")],
            &[
                (139, r#"<exitPoint x="113883" y="-108777" z="-848" />"#),
                (144, r#"<exitPoint x="113883" y="-108777" z="-848" />"#),
            ],
        ));
        t
    }

    /// An instance made the way `enterInstance` makes it (the command channel check is the
    /// engine's), with `player` inside.
    fn make_instance(t: &mut ScriptTest, player: i32, template_id: i32) -> i32 {
        let instance_id = t.world.create_dynamic_instance(template_id);
        t.world
            .add_instance_world(InstanceWorld::new(instance_id, template_id));
        on_enter_instance(&mut t.world, Oid(player), instance_id, true);
        t.ticks(60);
        instance_id
    }

    fn event(t: &mut ScriptTest, name: &str, npc: Oid, id: i32, player: Option<i32>) {
        let npc = ScriptNpc { object_id: npc, id };
        t.world.run_as_script(NAME, |ctx| {
            IceQueensCastleBattle.on_event(ctx, name, Some(npc), player.map(Oid));
        });
    }

    fn kill(t: &mut ScriptTest, npc: Oid, id: i32) {
        let npc = ScriptNpc { object_id: npc, id };
        t.world.run_as_script(NAME, |ctx| {
            IceQueensCastleBattle.on_kill(ctx, npc, Oid(1), false);
        });
    }

    fn inside(t: &ScriptTest, id: i32, instance_id: i32) -> Vec<Oid> {
        t.npcs_of(id)
            .into_iter()
            .filter(|n| t.world.instance_id_of(*n) == instance_id)
            .collect()
    }

    fn status(t: &ScriptTest, instance_id: i32) -> i32 {
        t.world.instance_world(instance_id).unwrap().status()
    }

    #[test]
    fn a_player_without_a_command_channel_is_refused() {
        let mut t = setup();
        t.enter(1, 85, vec![]);
        t.enter(2, 85, vec![]);
        t.take();
        // Not in a party.
        t.world.run_as_script(NAME, |ctx| {
            IceQueensCastleBattle.on_event(ctx, "enterEasy", None, Some(Oid(1)));
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(!t.take().is_empty());
        // In a party, but no command channel.
        t.party(1, 2);
        t.take();
        t.world.run_as_script(NAME, |ctx| {
            IceQueensCastleBattle.on_event(ctx, "enterHardcore", None, Some(Oid(1)));
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(!t.take().is_empty());
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);
    }

    #[test]
    fn the_first_entrance_marks_the_mode_and_lists_the_players() {
        let mut t = setup();
        t.enter(1, 85, vec![]);
        let hard = make_instance(&mut t, 1, TEMPLATE_ID_HARD);
        assert_eq!(param(&t.world, hard, "isHardCore").as_deref(), Some("true"));
        assert_eq!(players_inside(&t.world, hard), [Oid(1)]);
        assert_eq!(t.world.instance_world(hard).unwrap().allowed, [Oid(1)]);
        assert_eq!(t.world.instance_id_of(Oid(1)), hard);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!(ENTER_LOC.iter().any(|(ex, ey, _)| (*ex, *ey) == (x, y)));

        let mut t = setup();
        t.enter(1, 85, vec![]);
        let easy = make_instance(&mut t, 1, TEMPLATE_ID_EASY);
        assert_eq!(param(&t.world, easy, "isHardCore").as_deref(), Some("false"));
    }

    #[test]
    fn the_normal_battle_goes_through_its_stages() {
        let mut t = setup();
        t.enter(1, 85, vec![]);
        let instance_id = make_instance(&mut t, 1, TEMPLATE_ID_EASY);

        // Sirra's page: the first click opens the door and starts the controller.
        let sirra = t.spawn(SIRRA, 20);
        t.world.set_instance_id(sirra, instance_id);
        event(&mut t, "openDoor", sirra, SIRRA, Some(1));
        event(&mut t, "openDoor", sirra, SIRRA, Some(1));
        let controller = inside(&t, INVISIBLE_NPC, instance_id);
        // The controller and the eight statues of the floor.
        assert_eq!(controller.len(), 1 + 8);
        let controller = Oid(param(&t.world, instance_id, "controller")
            .unwrap()
            .parse()
            .unwrap());
        assert_eq!(t.world.quest_timer_count(), 1);

        // Stage 1: after the movie Freya stands there and the knights come.
        t.wait_ms(60_100);
        assert_eq!(status(&t, instance_id), 1);
        t.wait_ms(53_500 + 2_500);
        let freya = inside(&t, FREYA_THRONE, instance_id);
        assert_eq!(freya.len(), 1);
        assert_eq!(inside(&t, KNIGHT_EASY, instance_id).len(), 3 + 8);
        assert_eq!(inside(&t, GLACIER, instance_id).len(), 1);
        // Stop everything the monsters would do to the test player.
        for id in [KNIGHT_EASY, GLACIER, BREATH] {
            for mob in inside(&t, id, instance_id) {
                t.world.set_core_ai_disabled(mob, true);
            }
        }

        // 2% of Freya's health left: the first phase ends.
        event(&mut t, "STAGE_1_FINISH", controller, INVISIBLE_NPC, None);
        assert!(!t.in_world(freya[0]));
        assert_eq!(
            param(&t.world, instance_id, "canSpawnMobs").as_deref(),
            Some("false")
        );
        t.wait_ms(24_000);
        assert_eq!(status(&t, instance_id), 2);
        let spelling = inside(&t, FREYA_SPELLING, instance_id);
        assert_eq!(spelling.len(), 1);
        assert!(t.world.is_invul(spelling[0]));

        // Stage 2: a minute later the knights come again; ten dead knights call Glakias.
        t.wait_ms(60_100);
        assert_eq!(
            param(&t.world, instance_id, "canSpawnMobs").as_deref(),
            Some("true")
        );
        let knights = inside(&t, KNIGHT_EASY, instance_id);
        let living: Vec<Oid> = knights.into_iter().filter(|k| !t.world.is_dead(*k)).collect();
        assert!(living.len() >= 10);
        for knight in living.iter().take(10) {
            kill(&mut t, *knight, KNIGHT_EASY);
        }
        assert_eq!(status(&t, instance_id), 3);
        t.wait_ms(7_000 + 5_100);
        let glakias = inside(&t, GLAKIAS_EASY, instance_id);
        assert_eq!(glakias.len(), 1);

        // Glakias dies: Freya's second form leaves after a minute, the last one arrives.
        kill(&mut t, glakias[0], GLAKIAS_EASY);
        t.wait_ms(60_100);
        assert!(!t.in_world(spelling[0]));
        t.wait_ms(21_600);
        assert_eq!(status(&t, instance_id), 4);
        let last = inside(&t, FREYA_STAND_EASY, instance_id);
        assert_eq!(last.len(), 1);

        // The end: Freya's death starts the ending; the instance finishes after five minutes.
        let before = t.world.instance(instance_id).unwrap().empty_destroy_time_ms;
        assert_ne!(before, 0);
        kill(&mut t, last[0], FREYA_STAND_EASY);
        assert_eq!(
            param(&t.world, instance_id, "isSupportActive").as_deref(),
            Some("false")
        );
        t.wait_ms(300_100);
        assert_eq!(
            t.world.instance(instance_id).map(|i| i.empty_destroy_time_ms),
            Some(0)
        );
    }

    #[test]
    fn the_hardcore_second_stage_has_a_time_limit() {
        let mut t = setup();
        t.enter(1, 85, vec![]);
        let instance_id = make_instance(&mut t, 1, TEMPLATE_ID_HARD);
        let controller = t
            .world
            .script_add_spawn(INVISIBLE_NPC, CONTROLLER_LOC, 0, false, 0, instance_id)
            .unwrap();
        param_set(&mut t.world, instance_id, "controller", controller.0.to_string());
        let before = t.world.quest_timer_count();
        event(&mut t, "STAGE_2_START", controller, INVISIBLE_NPC, None);
        // STAGE_2_FAILED (six minutes) is a new timer; the end time is kept on the controller.
        assert!(t.world.quest_timer_count() > before);
        assert!(
            t.world
                .npc_variables(controller)
                .unwrap()
                .get_long("TIMER_END", 0)
                > 0
        );
    }
}
