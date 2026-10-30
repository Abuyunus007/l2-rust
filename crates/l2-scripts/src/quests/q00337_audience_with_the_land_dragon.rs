//! Quest 337: Audience With The Land Dragon (Gabrielle in Gludio; the Harit lizardmen, the marsh
//! monsters, the caves, the abyssal jewels of Antharas and their guardians).
// Java: dist/game/data/scripts/quests/Q00337_AudienceWithTheLandDragon/Q00337_AudienceWithTheLandDragon.java
//
// Pattern: the memo state is a four-digit code (one digit per errand: 1 = Kranrot and Hamrut,
// 2 = marsh, 3 = totem, 4 = remains; hundreds, tens and units of 2xxxx) that the errand NPCs
// add to; the jewel fights (`onAttack`) spawn guardians and drop fragments; the timers
// `DESPAWN` and `DESPAWN_240` remove the jewels.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const WAREHOUSE_CHIEF_MOKE: i32 = 30498;
const BLACKSMITH_HELTON: i32 = 30678;
const PREFECT_CHAKIRIS: i32 = 30705;
const MAGISTER_KAIENA: i32 = 30720;
const GABRIELLE: i32 = 30753;
const ANTHARAS_WATCHMAN_GILMORE: i32 = 30754;
const ANTHARAS_WATCHMAN_THEODRIC: i32 = 30755;
const MASTER_KENDRA: i32 = 30851;
const HIGH_PRIEST_ORVEN: i32 = 30857;

// Items
const FEATHER_OF_GABRIELLE: i32 = 3852;
const MARSH_STALKER_HORN: i32 = 3853;
const MARSH_DRAKE_TALONS: i32 = 3854;
const KRANROT_SKIN: i32 = 3855;
const HAMRUT_LEG: i32 = 3856;
const REMAINS_OF_SACRAFICE: i32 = 3857;
const TOTEM_OF_LAND_DRAGON: i32 = 3858;
const FRAGMENT_OF_ABYSS_JEWEL_1ST: i32 = 3859;
const FRAGMENT_OF_ABYSS_JEWEL_2ND: i32 = 3860;
const FRAGMENT_OF_ABYSS_JEWEL_3RD: i32 = 3861;
const MARA_FANG: i32 = 3862;
const MUSFEL_FANG: i32 = 3863;
const MARK_OF_WATCHMAN: i32 = 3864;
const HERALD_OF_SLAYER: i32 = 3890;

// Reward
const PORTAL_STONE: i32 = 3865;

// Monster
const BLOOD_QUEEN: i32 = 18001;
const CAVE_MAIDEN: i32 = 20134;
const CAVE_KEEPER: i32 = 20246;
const CAVE_KEEPER_HOLD: i32 = 20277;
const CAVE_MAIDEN_HOLD: i32 = 20287;
const HARIT_LIZARDMAN_SHAMAN: i32 = 20644;
const HARIT_LIZARDMAN_MATRIARCH: i32 = 20645;
const HAMRUT: i32 = 20649;
const KRANROT: i32 = 20650;
const MARSH_STALKER: i32 = 20679;
const MARSH_DRAKE: i32 = 20680;

// Quest Monster
const ABYSSAL_JEWEL_1: i32 = 27165;
const ABYSSAL_JEWEL_2: i32 = 27166;
const ABYSSAL_JEWEL_3: i32 = 27167;
const JEWEL_GUARDIAN_MARA: i32 = 27168;
const JEWEL_GUARDIAN_MUSFEL: i32 = 27169;
const JEWEL_GUARDIAN_PYTON: i32 = 27170;
const GHOST_OF_OFFERING: i32 = 27171;
const HARIT_LIZARDMAN_ZEALOT: i32 = 27172;

// Misc
const MIN_LEVEL: i32 = 50;

/// Java `Q00337_AudienceWithTheLandDragon`.
pub struct Q00337AudienceWithTheLandDragon;

impl Script for Q00337AudienceWithTheLandDragon {
    fn name(&self) -> &'static str {
        "Q00337_AudienceWithTheLandDragon"
    }

    fn path(&self) -> &'static str {
        "quests/Q00337_AudienceWithTheLandDragon"
    }

    fn id(&self) -> i32 {
        337
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GABRIELLE]);
        s.add_talk_id(&[
            GABRIELLE,
            WAREHOUSE_CHIEF_MOKE,
            BLACKSMITH_HELTON,
            PREFECT_CHAKIRIS,
            MAGISTER_KAIENA,
            ANTHARAS_WATCHMAN_GILMORE,
            ANTHARAS_WATCHMAN_THEODRIC,
            MASTER_KENDRA,
            HIGH_PRIEST_ORVEN,
        ]);
        s.add_kill_id(&[
            BLOOD_QUEEN,
            CAVE_MAIDEN,
            CAVE_KEEPER,
            CAVE_KEEPER_HOLD,
            CAVE_MAIDEN_HOLD,
            HARIT_LIZARDMAN_SHAMAN,
            HARIT_LIZARDMAN_MATRIARCH,
            HAMRUT,
            KRANROT,
            MARSH_STALKER,
            MARSH_DRAKE,
            ABYSSAL_JEWEL_1,
            ABYSSAL_JEWEL_2,
            ABYSSAL_JEWEL_3,
            JEWEL_GUARDIAN_MARA,
            JEWEL_GUARDIAN_MUSFEL,
            JEWEL_GUARDIAN_PYTON,
            GHOST_OF_OFFERING,
            HARIT_LIZARDMAN_ZEALOT,
        ]);
        s.add_attack_id(&[ABYSSAL_JEWEL_1, ABYSSAL_JEWEL_2, ABYSSAL_JEWEL_3]);
        s.register_quest_items(&[
            FEATHER_OF_GABRIELLE,
            MARSH_STALKER_HORN,
            MARSH_DRAKE_TALONS,
            KRANROT_SKIN,
            HAMRUT_LEG,
            REMAINS_OF_SACRAFICE,
            TOTEM_OF_LAND_DRAGON,
            FRAGMENT_OF_ABYSS_JEWEL_1ST,
            FRAGMENT_OF_ABYSS_JEWEL_2ND,
            FRAGMENT_OF_ABYSS_JEWEL_3RD,
            MARA_FANG,
            MUSFEL_FANG,
            MARK_OF_WATCHMAN,
            HERALD_OF_SLAYER,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // The jewels and their guardians go when their timers say so (`npc.deleteMe()`).
        if event == "DESPAWN" || event == "DESPAWN_240" {
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30753-05.htm" => {
                if qs.is_created(ctx) {
                    ctx.give_items(player, FEATHER_OF_GABRIELLE, 1);
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 20000);
                    htmltext = Some(event.to_owned());
                }
            }
            "30753-09.html" => {
                ctx.take_items(player, MARK_OF_WATCHMAN, -1);
                qs.set_memo_state(ctx, 40000);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "30754-03.html" => {
                qs.set_memo_state(ctx, 70000);
                qs.set_cond(ctx, 4, true);
                htmltext = Some(event.to_owned());
            }
            "30755-05.html" => {
                if qs.is_memo_state(ctx, 70000) && ctx.has_quest_items(player, FRAGMENT_OF_ABYSS_JEWEL_3RD) {
                    ctx.give_items(player, PORTAL_STONE, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30498-02.html" | "30678-01a.html" | "30753-01a.html" | "30753-03.htm" | "30753-04.htm"
            | "30753-06a.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
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
        let qs = ctx.get_quest_state(attacker, false);
        let Some(qs) = qs.filter(|qs| qs.is_started(ctx)) else {
            return;
        };
        let cur_hp = ctx.npc_current_hp(npc);
        let max_hp = f64::from(ctx.npc_max_hp(npc));

        match npc.id {
            ABYSSAL_JEWEL_1 => {
                if qs.is_memo_state(ctx, 40000) || qs.is_memo_state(ctx, 40001) {
                    if cur_hp < max_hp * 0.8
                        && ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .is_none_or(|v| v.get_int("i_quest0", 0) == 0)
                    {
                        for _ in 0..20 {
                            if let Some(guardian) =
                                ctx.add_spawn_at(JEWEL_GUARDIAN_MARA, npc.object_id, true, 180_000)
                            {
                                ctx.add_attack_desire(guardian, attacker, 999);
                            }
                        }
                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_int("i_quest0", 1);
                        ctx.start_quest_timer("DESPAWN", 900_000, Some(npc), Some(attacker), false);
                    }

                    if cur_hp < max_hp * 0.4 && !ctx.has_quest_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_1ST) {
                        ctx.give_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_1ST, 1);
                        ctx.play_sound(attacker, QuestSound::ItemsoundQuestItemget);
                        ctx.start_quest_timer("DESPAWN_240", 240_000, Some(npc), Some(attacker), false);
                    }
                }

                if cur_hp < max_hp * 0.1 {
                    ctx.delete_npc(npc);
                }
            }
            ABYSSAL_JEWEL_2 => {
                if qs.is_memo_state(ctx, 40000) || qs.is_memo_state(ctx, 40010) {
                    if cur_hp < max_hp * 0.8
                        && ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .is_none_or(|v| v.get_int("i_quest0", 0) == 0)
                    {
                        for _ in 0..20 {
                            if let Some(guardian) =
                                ctx.add_spawn_at(JEWEL_GUARDIAN_MUSFEL, npc.object_id, true, 180_000)
                            {
                                ctx.add_attack_desire(guardian, attacker, 999);
                            }
                        }
                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_int("i_quest0", 1);
                        ctx.start_quest_timer("DESPAWN", 900_000, Some(npc), Some(attacker), false);
                    }

                    if cur_hp < max_hp * 0.4 && !ctx.has_quest_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_2ND) {
                        ctx.give_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_2ND, 1);
                        ctx.play_sound(attacker, QuestSound::ItemsoundQuestItemget);
                        ctx.start_quest_timer("DESPAWN_240", 240_000, Some(npc), Some(attacker), false);
                    }
                }

                if cur_hp < max_hp * 0.1 {
                    ctx.delete_npc(npc);
                }
            }
            ABYSSAL_JEWEL_3 => {
                if qs.is_memo_state(ctx, 70000) {
                    if cur_hp < max_hp * 0.8
                        && ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .is_none_or(|v| v.get_int("i_quest0", 0) == 0)
                    {
                        for _ in 0..4 {
                            if let Some(guardian) =
                                ctx.add_spawn_at(JEWEL_GUARDIAN_PYTON, npc.object_id, true, 180_000)
                            {
                                ctx.add_attack_desire(guardian, attacker, 999);
                            }
                        }
                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_int("i_quest0", 1);
                    }

                    if cur_hp < max_hp * 0.4 && !ctx.has_quest_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_3RD) {
                        ctx.give_items(attacker, FRAGMENT_OF_ABYSS_JEWEL_3RD, 1);
                        ctx.play_sound(attacker, QuestSound::ItemsoundQuestItemget);
                    }
                }

                if cur_hp < max_hp * 0.1 {
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        let Some(qs) = qs.filter(|qs| qs.is_started(ctx)) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        match npc.id {
            BLOOD_QUEEN => match qs.get_memo_state(ctx) {
                21011 | 21010 | 21001 | 21000 | 20011 | 20010 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, REMAINS_OF_SACRAFICE) {
                        for _ in 0..8 {
                            ctx.add_spawn_at(GHOST_OF_OFFERING, npc.object_id, true, 180_000);
                        }
                    }
                }
                _ => {}
            },
            CAVE_MAIDEN | CAVE_KEEPER | CAVE_KEEPER_HOLD | CAVE_MAIDEN_HOLD => {
                if qs.is_memo_state(ctx, 70000)
                    && !ctx.has_quest_items(killer, FRAGMENT_OF_ABYSS_JEWEL_3RD)
                    && ctx.get_random(5) == 0
                {
                    ctx.add_spawn_at(ABYSSAL_JEWEL_3, npc.object_id, true, 180_000);
                }
            }
            HARIT_LIZARDMAN_SHAMAN => match qs.get_memo_state(ctx) {
                21110 | 21100 | 21010 | 21000 | 20110 | 20100 | 20010 | 20000 => {
                    if !ctx.has_quest_items(killer, TOTEM_OF_LAND_DRAGON) {
                        for _ in 0..3 {
                            if let Some(zealot) =
                                ctx.add_spawn_at(HARIT_LIZARDMAN_ZEALOT, npc.object_id, true, 180_000)
                            {
                                ctx.add_attack_desire(zealot, killer, 999);
                            }
                        }
                    }
                }
                _ => {}
            },
            HARIT_LIZARDMAN_MATRIARCH => match qs.get_memo_state(ctx) {
                21110 | 21100 | 21010 | 21000 | 20110 | 20100 | 20010 | 20000 => {
                    if !ctx.has_quest_items(killer, TOTEM_OF_LAND_DRAGON) && ctx.get_random(5) == 0 {
                        for _ in 0..3 {
                            if let Some(zealot) =
                                ctx.add_spawn_at(HARIT_LIZARDMAN_ZEALOT, npc.object_id, true, 180_000)
                            {
                                ctx.add_attack_desire(zealot, killer, 999);
                            }
                        }
                    }
                }
                _ => {}
            },
            HAMRUT => match qs.get_memo_state(ctx) {
                21101 | 21100 | 21001 | 21000 | 20101 | 20100 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, HAMRUT_LEG) {
                        ctx.give_items(killer, HAMRUT_LEG, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            KRANROT => match qs.get_memo_state(ctx) {
                21101 | 21100 | 21001 | 21000 | 20101 | 20100 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, KRANROT_SKIN) {
                        ctx.give_items(killer, KRANROT_SKIN, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            MARSH_STALKER => match qs.get_memo_state(ctx) {
                20111 | 20110 | 20101 | 20100 | 20011 | 20010 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, MARSH_STALKER_HORN) {
                        ctx.give_items(killer, MARSH_STALKER_HORN, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            MARSH_DRAKE => match qs.get_memo_state(ctx) {
                20111 | 20110 | 20101 | 20100 | 20011 | 20010 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, MARSH_DRAKE_TALONS) {
                        ctx.give_items(killer, MARSH_DRAKE_TALONS, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            JEWEL_GUARDIAN_MARA => {
                if (qs.is_memo_state(ctx, 40000) || qs.is_memo_state(ctx, 40001))
                    && !ctx.has_quest_items(killer, MARA_FANG)
                {
                    ctx.give_items(killer, MARA_FANG, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            JEWEL_GUARDIAN_MUSFEL => {
                if (qs.is_memo_state(ctx, 40000) || qs.is_memo_state(ctx, 40010))
                    && !ctx.has_quest_items(killer, MUSFEL_FANG)
                {
                    ctx.give_items(killer, MUSFEL_FANG, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            GHOST_OF_OFFERING => match qs.get_memo_state(ctx) {
                21011 | 21010 | 21001 | 21000 | 20011 | 20010 | 20001 | 20000 => {
                    if !ctx.has_quest_items(killer, REMAINS_OF_SACRAFICE) {
                        ctx.give_items(killer, REMAINS_OF_SACRAFICE, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            HARIT_LIZARDMAN_ZEALOT => match qs.get_memo_state(ctx) {
                21110 | 21100 | 21010 | 21000 | 20110 | 20100 | 20010 | 20000 => {
                    if !ctx.has_quest_items(killer, TOTEM_OF_LAND_DRAGON) {
                        ctx.give_items(killer, TOTEM_OF_LAND_DRAGON, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == GABRIELLE {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30753-01.htm"
                } else {
                    "30753-02.htm"
                }
                .to_owned();
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                GABRIELLE => {
                    if (20000..30000).contains(&memo_state) {
                        htmltext = "30753-06.html".to_owned();
                    } else if memo_state == 30000 {
                        htmltext = "30753-08.html".to_owned();
                    } else if (40000..50000).contains(&memo_state) {
                        htmltext = "30753-10.html".to_owned();
                    } else if memo_state == 50000 {
                        ctx.take_items(player, FEATHER_OF_GABRIELLE, -1);
                        ctx.take_items(player, MARK_OF_WATCHMAN, -1);
                        ctx.give_items(player, HERALD_OF_SLAYER, 1);
                        qs.set_memo_state(ctx, 60000);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30753-11.html".to_owned();
                    } else if memo_state == 60000 {
                        htmltext = "30753-12.html".to_owned();
                    } else if memo_state == 70000 {
                        htmltext = "30753-13.html".to_owned();
                    }
                }
                WAREHOUSE_CHIEF_MOKE => {
                    if memo_state == 40000 || memo_state == 40001 {
                        if ctx.has_all_quest_items(player, &[FRAGMENT_OF_ABYSS_JEWEL_1ST, MARA_FANG]) {
                            ctx.take_items(player, FRAGMENT_OF_ABYSS_JEWEL_1ST, -1);
                            ctx.take_items(player, MARA_FANG, -1);
                            ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                            if qs.get_memo_state(ctx) == 40001 {
                                qs.set_memo_state(ctx, 50000);
                            } else {
                                qs.set_memo_state(ctx, 40010);
                            }

                            htmltext = "30498-03.html".to_owned();
                        } else {
                            htmltext = "30498-01.html".to_owned();
                        }
                    } else if memo_state == 40010 {
                        htmltext = "30498-04.html".to_owned();
                    } else if memo_state >= 50000 {
                        htmltext = "30498-05.html".to_owned();
                    }
                }
                BLACKSMITH_HELTON => {
                    if memo_state == 40000 || memo_state == 40010 {
                        if ctx.has_all_quest_items(player, &[FRAGMENT_OF_ABYSS_JEWEL_2ND, MUSFEL_FANG]) {
                            ctx.take_items(player, FRAGMENT_OF_ABYSS_JEWEL_2ND, -1);
                            ctx.take_items(player, MUSFEL_FANG, -1);
                            ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                            if qs.get_memo_state(ctx) == 40010 {
                                qs.set_memo_state(ctx, 50000);
                            } else {
                                qs.set_memo_state(ctx, 40001);
                            }

                            htmltext = "30678-02.html".to_owned();
                        } else {
                            htmltext = "30678-01.html".to_owned();
                        }
                    } else if memo_state == 40001 {
                        htmltext = "30678-03.html".to_owned();
                    } else if memo_state >= 50000 {
                        htmltext = "30678-04.html".to_owned();
                    }
                }
                PREFECT_CHAKIRIS => {
                    match qs.get_memo_state(ctx) {
                        21101 | 21000 | 21100 | 21001 | 20101 | 20100 | 20001 | 20000 => {
                            if ctx.has_all_quest_items(player, &[KRANROT_SKIN, HAMRUT_LEG]) {
                                ctx.take_items(player, KRANROT_SKIN, -1);
                                ctx.take_items(player, HAMRUT_LEG, -1);
                                ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                                if qs.get_memo_state(ctx) + 10 == 21111 {
                                    qs.set_memo_state(ctx, 30000);
                                } else {
                                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + 10);
                                }

                                htmltext = "30705-02.html".to_owned();
                            } else {
                                htmltext = "30705-01.html".to_owned();
                            }
                        }
                        21110 | 21011 | 21010 | 20111 | 20110 | 20011 | 20010 => {
                            htmltext = "30705-03.html".to_owned();
                        }
                        _ => {}
                    }

                    if memo_state >= 30000 {
                        htmltext = "30705-04.html".to_owned();
                    }
                }
                MAGISTER_KAIENA => {
                    match qs.get_memo_state(ctx) {
                        20111 | 20110 | 20101 | 20100 | 20010 | 20011 | 20001 | 20000 => {
                            if ctx.has_all_quest_items(player, &[MARSH_STALKER_HORN, MARSH_DRAKE_TALONS]) {
                                ctx.take_items(player, MARSH_STALKER_HORN, -1);
                                ctx.take_items(player, MARSH_DRAKE_TALONS, -1);
                                ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                                if qs.get_memo_state(ctx) + 1000 == 21111 {
                                    qs.set_memo_state(ctx, 30000);
                                } else {
                                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + 1000);
                                }

                                htmltext = "30720-02.html".to_owned();
                            } else {
                                htmltext = "30720-01.html".to_owned();
                            }
                        }
                        21110 | 21101 | 21100 | 21011 | 21010 | 21001 | 21000 => {
                            htmltext = "30720-03.html".to_owned();
                        }
                        _ => {}
                    }

                    if memo_state >= 30000 {
                        htmltext = "30720-04.html".to_owned();
                    }
                }
                ANTHARAS_WATCHMAN_GILMORE => {
                    if memo_state < 60000 {
                        htmltext = "30754-01.html".to_owned();
                    } else if memo_state == 60000 {
                        htmltext = "30754-02.html".to_owned();
                    } else if memo_state == 70000 {
                        htmltext = if ctx.has_quest_items(player, FRAGMENT_OF_ABYSS_JEWEL_3RD) {
                            "30754-05.html"
                        } else {
                            "30754-04.html"
                        }
                        .to_owned();
                    }
                }
                ANTHARAS_WATCHMAN_THEODRIC => {
                    if memo_state < 60000 {
                        htmltext = "30755-01.html".to_owned();
                    } else if memo_state == 60000 {
                        htmltext = "30755-02.html".to_owned();
                    } else if memo_state == 70000 {
                        htmltext = if !ctx.has_quest_items(player, FRAGMENT_OF_ABYSS_JEWEL_3RD) {
                            "30755-03.html"
                        } else {
                            "30755-04.html"
                        }
                        .to_owned();
                    }
                }
                MASTER_KENDRA => {
                    match qs.get_memo_state(ctx) {
                        21110 | 21100 | 21010 | 21000 | 20110 | 20100 | 20010 | 20000 => {
                            if !ctx.has_quest_items(player, TOTEM_OF_LAND_DRAGON) {
                                htmltext = "30851-01.html".to_owned();
                            } else {
                                ctx.take_items(player, TOTEM_OF_LAND_DRAGON, -1);
                                ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                                if qs.get_memo_state(ctx) + 1 == 21111 {
                                    qs.set_memo_state(ctx, 30000);
                                } else {
                                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + 1);
                                }

                                htmltext = "30851-02.html".to_owned();
                            }
                        }
                        21101 | 21011 | 21001 | 20111 | 20101 | 20011 | 20001 => {
                            htmltext = "30851-03.html".to_owned();
                        }
                        _ => {}
                    }

                    if memo_state >= 30000 {
                        htmltext = "30851-04.html".to_owned();
                    }
                }
                HIGH_PRIEST_ORVEN => {
                    match qs.get_memo_state(ctx) {
                        21011 | 21010 | 21001 | 21000 | 20011 | 20010 | 20001 | 20000 => {
                            if !ctx.has_quest_items(player, REMAINS_OF_SACRAFICE) {
                                htmltext = "30857-01.html".to_owned();
                            } else {
                                ctx.take_items(player, REMAINS_OF_SACRAFICE, -1);
                                ctx.give_items(player, MARK_OF_WATCHMAN, 1);
                                if qs.get_memo_state(ctx) + 100 == 21111 {
                                    qs.set_memo_state(ctx, 30000);
                                } else {
                                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + 100);
                                }

                                htmltext = "30857-02.html".to_owned();
                            }
                        }
                        21110 | 21101 | 21100 | 20111 | 20110 | 20101 | 20100 => {
                            htmltext = "30857-03.html".to_owned();
                        }
                        _ => {}
                    }

                    if memo_state >= 30000 {
                        htmltext = "30857-04.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}
#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00337_AudienceWithTheLandDragon";

    const NPCS: &str = r#"
        <npc id="30753" type="Folk" level="60" name="Gabrielle"/>
        <npc id="30720" type="Folk" level="60" name="Magister Kaiena"/>
        <npc id="30851" type="Folk" level="60" name="Master Kendra"/>
        <npc id="30755" type="Folk" level="60" name="Antharas Watchman Theodric"/>
        <npc id="20644" type="Monster" level="50" name="Harit Lizardman Shaman"/>
        <npc id="20679" type="Monster" level="50" name="Marsh Stalker"/>
        <npc id="20680" type="Monster" level="50" name="Marsh Drake"/>
        <npc id="27165" type="Monster" level="50" name="Abyssal Jewel"/>
        <npc id="27172" type="Monster" level="50" name="Harit Lizardman Zealot"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3852" type="EtcItem" name="Feather of Gabrielle"><set name="is_questitem" val="true"/></item>
        <item id="3853" type="EtcItem" name="Marsh Stalker Horn"><set name="is_questitem" val="true"/></item>
        <item id="3854" type="EtcItem" name="Marsh Drake Talons"><set name="is_questitem" val="true"/></item>
        <item id="3858" type="EtcItem" name="Totem of Land Dragon"><set name="is_questitem" val="true"/></item>
        <item id="3861" type="EtcItem" name="Fragment of Abyss Jewel 3rd"><set name="is_questitem" val="true"/></item>
        <item id="3864" type="EtcItem" name="Mark of Watchman"><set name="is_questitem" val="true"/></item>
        <item id="3865" type="EtcItem" name="Portal Stone"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00337AudienceWithTheLandDragon);
        let pages = Pages::of(&Q00337AudienceWithTheLandDragon)
            .page("30753-01.htm", &["30753-01a.html"])
            .plain(&["30753-01a.html"])
            .page("30753-02.htm", &["30753-03.htm"])
            .page("30753-03.htm", &["30753-04.htm"])
            .page("30753-04.htm", &["30753-05.htm"])
            .page("30753-05.htm", &["30753-06a.html"])
            .page("30753-06.html", &["30753-06a.html"])
            .plain(&["30753-06a.html", "30753-08.html", "30753-10.html"])
            .plain(&["30753-11.html", "30753-12.html", "30753-13.html"])
            .plain(&["30720-01.html", "30720-02.html", "30720-03.html", "30720-04.html"])
            .plain(&["30851-01.html", "30851-02.html", "30851-03.html", "30851-04.html"])
            .page("30755-04.html", &["30755-05.html"])
            .plain(&["30755-01.html", "30755-02.html", "30755-03.html", "30755-05.html"])
            .plain(&[
                "30754-01.html",
                "30754-02.html",
                "30754-03.html",
                "30754-04.html",
                "30754-05.html",
            ])
            .npcs(&[
                GABRIELLE,
                MAGISTER_KAIENA,
                MASTER_KENDRA,
                ANTHARAS_WATCHMAN_THEODRIC,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gabrielle = t.spawn(GABRIELLE, 20);
        let kaiena = t.spawn(MAGISTER_KAIENA, 40);
        let kendra = t.spawn(MASTER_KENDRA, 60);
        (t, gabrielle, kaiena, kendra)
    }

    /// Gabrielle's accept (level 50): the feather, cond 1 and memo 20000.
    fn start(t: &mut ScriptTest, gabrielle: Oid) {
        assert!(t.quest_page(1, gabrielle).contains("30753-02.htm"));
        assert!(t.event(1, NAME, "30753-03.htm").contains("30753-03.htm"));
        assert!(t.event(1, NAME, "30753-04.htm").contains("30753-04.htm"));
        assert!(t.event(1, NAME, "30753-05.htm").contains("30753-05.htm"));
    }

    #[test]
    fn a_character_below_level_fifty_is_sent_away() {
        let (mut t, gabrielle, ..) = setup();
        t.enter(1, 49, vec![]);
        assert!(t.quest_page(1, gabrielle).contains("30753-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn gabrielle_starts_the_quest_with_the_feather_and_memo_20000() {
        let (mut t, gabrielle, ..) = setup();
        t.enter(1, 50, vec![]);
        assert!(t.quest_page(1, gabrielle).contains("30753-02.htm"));
        assert!(t.event(1, NAME, "30753-03.htm").contains("30753-03.htm"));
        assert!(t.event(1, NAME, "30753-04.htm").contains("30753-04.htm"));
        t.take_writes();
        assert!(t.event(1, NAME, "30753-05.htm").contains("30753-05.htm"));
        assert_eq!(t.count(1, FEATHER_OF_GABRIELLE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "20000"),
            ]
        );
        assert!(t.quest_page(1, gabrielle).contains("30753-06.html"));
    }

    #[test]
    fn the_marsh_horn_and_talons_go_to_kaiena_for_the_mark() {
        let (mut t, gabrielle, kaiena, _kendra) = setup();
        t.enter(1, 50, vec![]);
        start(&mut t, gabrielle);
        t.take_writes();

        // A marsh stalker and a marsh drake each drop their part once.
        let stalker = t.spawn(MARSH_STALKER, 60);
        t.kill(1, stalker);
        assert_eq!(t.count(1, MARSH_STALKER_HORN), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        let drake = t.spawn(MARSH_DRAKE, 60);
        t.kill(1, drake);
        assert_eq!(t.count(1, MARSH_DRAKE_TALONS), 1);
        // Another horn is not wanted.
        let stalker = t.spawn(MARSH_STALKER, 60);
        t.kill(1, stalker);
        assert_eq!(t.count(1, MARSH_STALKER_HORN), 1);

        // Kaiena takes both: the mark and memo 21000 (memo + 1000).
        assert!(t.quest_page(1, kaiena).contains("30720-02.html"));
        assert_eq!(t.count(1, MARSH_STALKER_HORN), 0);
        assert_eq!(t.count(1, MARSH_DRAKE_TALONS), 0);
        assert_eq!(t.count(1, MARK_OF_WATCHMAN), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "21000")]);
        // Kaiena has nothing more to do now.
        assert!(t.quest_page(1, kaiena).contains("30720-03.html"));
    }

    #[test]
    fn kaiena_without_the_horn_and_talons_sends_the_player_out() {
        let (mut t, gabrielle, kaiena, _kendra) = setup();
        t.enter(1, 50, vec![]);
        start(&mut t, gabrielle);
        t.take_writes();
        assert!(t.quest_page(1, kaiena).contains("30720-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_shamans_kill_calls_three_zealots_and_a_zealot_drops_the_totem() {
        let (mut t, gabrielle, ..) = setup();
        t.enter(1, 50, vec![]);
        start(&mut t, gabrielle);
        // Memo 21000: the shaman's kill calls three zealots.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 21000);
        });
        t.take_writes();
        let shaman = t.spawn(HARIT_LIZARDMAN_SHAMAN, 60);
        t.kill(1, shaman);
        let zealots = t.npcs_of(HARIT_LIZARDMAN_ZEALOT);
        assert_eq!(zealots.len(), 3);
        // A zealot drops the totem; the others still attack the player, so they go.
        t.kill(1, zealots[0]);
        assert_eq!(t.count(1, TOTEM_OF_LAND_DRAGON), 1);
        for other in &zealots[1..] {
            t.world.delete_npc(*other);
        }
    }

    #[test]
    fn kendra_takes_the_totem_for_the_mark_and_memo_21001() {
        let (mut t, _gabrielle, _kaiena, kendra) = setup();
        t.enter(1, 50, vec![item(100, TOTEM_OF_LAND_DRAGON, 1)]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, 21000);
        });
        t.take_writes();
        assert!(t.quest_page(1, kendra).contains("30851-02.html"));
        assert_eq!(t.count(1, TOTEM_OF_LAND_DRAGON), 0);
        assert_eq!(t.count(1, MARK_OF_WATCHMAN), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "21001")]);
    }

    #[test]
    fn the_last_fragment_gives_the_portal_stone_and_ends_the_quest() {
        let (mut t, gabrielle, ..) = setup();
        let theodric = t.spawn(ANTHARAS_WATCHMAN_THEODRIC, 80);
        t.enter(1, 50, vec![]);
        start(&mut t, gabrielle);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 70000);
            ctx.give_items(Oid(1), FRAGMENT_OF_ABYSS_JEWEL_3RD, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, theodric).contains("30755-04.html"));
        t.take();
        t.bypass(1, &format!("Script {NAME} 30755-05.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30755-05.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, PORTAL_STONE), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_jewel_timer_removes_the_jewel() {
        let (mut t, gabrielle, ..) = setup();
        t.enter(1, 50, vec![]);
        start(&mut t, gabrielle);
        let jewel = t.spawn(ABYSSAL_JEWEL_1, 60);
        assert!(t.in_world(jewel));
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(jewel).unwrap();
            Q00337AudienceWithTheLandDragon.on_event(ctx, "DESPAWN", Some(npc), Some(Oid(1)));
        });
        assert!(!t.in_world(jewel));
    }
}
