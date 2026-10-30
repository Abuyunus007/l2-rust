//! Quest 232: Test of the Lord (the Orc Shaman's second class change: Flame Lord Kakai, the
//! tribe chiefs, Seer Somak and Manakia, Trader Jakal, Blacksmith Sumari and Ancestor Martankus).
// Java: dist/game/data/scripts/quests/Q00232_TestOfTheLord/Q00232_TestOfTheLord.java
//
// Pattern: five ordeals run in parallel on quest items (no memo state), then Martankus' charm;
// the last NPC summons the First Orc with `addSpawn(npc, ...)` (the summoner's list).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class, race};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SEER_SOMAK: i32 = 30510;
const SEER_MANAKIA: i32 = 30515;
const TRADER_JAKAL: i32 = 30558;
const BLACKSMITH_SUMARI: i32 = 30564;
const FLAME_LORD_KAKAI: i32 = 30565;
const ATUBA_CHIEF_VARKEES: i32 = 30566;
const NERUGA_CHIEF_TANTUS: i32 = 30567;
const URUTU_CHIEF_HATOS: i32 = 30568;
const DUDA_MARA_CHIEF_TAKUNA: i32 = 30641;
const GANDI_CHIEF_CHIANTA: i32 = 30642;
const FIRST_ORC: i32 = 30643;
const ANCESTOR_MARTANKUS: i32 = 30649;

// Items
const ADENA: i32 = 57;
const BONE_ARROW: i32 = 1341;
const ORDEAL_NECKLACE: i32 = 3391;
const VARKEES_CHARM: i32 = 3392;
const TANTUS_CHARM: i32 = 3393;
const HATOS_CHARM: i32 = 3394;
const TAKUNA_CHARM: i32 = 3395;
const CHIANTA_CHARM: i32 = 3396;
const MANAKIAS_ORDERS: i32 = 3397;
const BREKA_ORC_FANG: i32 = 3398;
const MANAKIAS_AMULET: i32 = 3399;
const HUGE_ORC_FANG: i32 = 3400;
const SUMARIS_LETTER: i32 = 3401;
const URUTU_BLADE: i32 = 3402;
const TIMAK_ORC_SKULL: i32 = 3403;
const SWORD_INTO_SKULL: i32 = 3404;
const NERUGA_AXE_BLADE: i32 = 3405;
const AXE_OF_CEREMONY: i32 = 3406;
const MARSH_SPIDER_FEELER: i32 = 3407;
const MARSH_SPIDER_FEET: i32 = 3408;
const HANDIWORK_SPIDER_BROOCH: i32 = 3409;
const ENCHANTED_MONSTER_CORNEA: i32 = 3410;
const MONSTER_EYE_WOODCARVING: i32 = 3411;
const BEAR_FANG_NECKLACE: i32 = 3412;
const MARTANKUS_CHARM: i32 = 3413;
const RAGNA_ORC_HEAD: i32 = 3414;
const RAGNA_CHIEF_NOTICE: i32 = 3415;
const IMMORTAL_FLAME: i32 = 3416;

// Reward
const MARK_OF_LORD: i32 = 3390;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const MARSH_SPIDER: i32 = 20233;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const TIMAK_ORC_SOLDIER: i32 = 20585;
const TIMAK_ORC_WARRIOR: i32 = 20586;
const TIMAK_ORC_SHAMAN: i32 = 20587;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const RAGNA_ORC_OVERLORD: i32 = 20778;
const RAGNA_ORC_SEER: i32 = 20779;

// Misc
const MIN_LEVEL: i32 = 39;

// Locations
const FIRST_ORC_SPAWN: (i32, i32, i32) = (21036, -107690, -3038);

/// Java `Q00232_TestOfTheLord`.
pub struct Q00232TestOfTheLord;

fn has(ctx: &ScriptCtx<'_>, p: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(p, item)
}

fn all(ctx: &ScriptCtx<'_>, p: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(p, items)
}

fn any(ctx: &ScriptCtx<'_>, p: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(p, items)
}

/// Java `giveItems(killer, item, amount)` and the middle sound at the limit, else the item sound.
fn collect(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32, amount: i64, limit: i64) {
    ctx.give_items(killer, item, amount);
    if ctx.get_quest_items_count(killer, item) >= limit {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

fn spawn_first_orc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    if ctx.summoned_npc_count(npc) < 1 {
        let (x, y, z) = FIRST_ORC_SPAWN;
        ctx.add_spawn_summoned(npc, FIRST_ORC, x, y, z, 0, false, 10_000, 0);
    }
}

impl Script for Q00232TestOfTheLord {
    fn name(&self) -> &'static str {
        "Q00232_TestOfTheLord"
    }

    fn path(&self) -> &'static str {
        "quests/Q00232_TestOfTheLord"
    }

    fn id(&self) -> i32 {
        232
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FLAME_LORD_KAKAI]);
        s.add_talk_id(&[
            FLAME_LORD_KAKAI,
            SEER_SOMAK,
            SEER_MANAKIA,
            TRADER_JAKAL,
            BLACKSMITH_SUMARI,
            ATUBA_CHIEF_VARKEES,
            NERUGA_CHIEF_TANTUS,
            URUTU_CHIEF_HATOS,
            DUDA_MARA_CHIEF_TAKUNA,
            GANDI_CHIEF_CHIANTA,
            FIRST_ORC,
            ANCESTOR_MARTANKUS,
        ]);
        s.add_kill_id(&[
            MARSH_SPIDER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            ENCHANTED_MONSTEREYE,
            TIMAK_ORC,
            TIMAK_ORC_ARCHER,
            TIMAK_ORC_SOLDIER,
            TIMAK_ORC_WARRIOR,
            TIMAK_ORC_SHAMAN,
            TIMAK_ORC_OVERLORD,
            RAGNA_ORC_OVERLORD,
            RAGNA_ORC_SEER,
        ]);
        s.register_quest_items(&[
            ORDEAL_NECKLACE,
            VARKEES_CHARM,
            TANTUS_CHARM,
            HATOS_CHARM,
            TAKUNA_CHARM,
            CHIANTA_CHARM,
            MANAKIAS_ORDERS,
            BREKA_ORC_FANG,
            MANAKIAS_AMULET,
            HUGE_ORC_FANG,
            SUMARIS_LETTER,
            URUTU_BLADE,
            TIMAK_ORC_SKULL,
            SWORD_INTO_SKULL,
            NERUGA_AXE_BLADE,
            AXE_OF_CEREMONY,
            MARSH_SPIDER_FEELER,
            MARSH_SPIDER_FEET,
            HANDIWORK_SPIDER_BROOCH,
            ENCHANTED_MONSTER_CORNEA,
            MONSTER_EYE_WOODCARVING,
            BEAR_FANG_NECKLACE,
            MARTANKUS_CHARM,
            RAGNA_ORC_HEAD,
            RAGNA_CHIEF_NOTICE,
            IMMORTAL_FLAME,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, ORDEAL_NECKLACE, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 92);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30565-05b.htm".to_owned());
                    } else {
                        htmltext = Some("30565-05.htm".to_owned());
                    }
                }
            }
            "30565-05a.html" | "30558-03a.html" | "30643-02.html" | "30643-03.html" | "30649-02.html"
            | "30649-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30565-08.html" => {
                if has(ctx, player, HUGE_ORC_FANG) {
                    ctx.take_items(player, ORDEAL_NECKLACE, 1);
                    ctx.take_items(player, HUGE_ORC_FANG, 1);
                    ctx.take_items(player, SWORD_INTO_SKULL, 1);
                    ctx.take_items(player, AXE_OF_CEREMONY, 1);
                    ctx.take_items(player, HANDIWORK_SPIDER_BROOCH, 1);
                    ctx.take_items(player, MONSTER_EYE_WOODCARVING, 1);
                    ctx.give_items(player, BEAR_FANG_NECKLACE, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30558-02.html" => {
                if ctx.get_quest_items_count(player, ADENA) >= 1000 {
                    ctx.take_items(player, ADENA, 1000);
                    ctx.give_items(player, NERUGA_AXE_BLADE, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30566-02.html" => {
                ctx.give_items(player, VARKEES_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30567-02.html" => {
                ctx.give_items(player, TANTUS_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30568-02.html" => {
                ctx.give_items(player, HATOS_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30641-02.html" => {
                ctx.give_items(player, TAKUNA_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30642-02.html" => {
                ctx.give_items(player, CHIANTA_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30649-04.html" => {
                if has(ctx, player, BEAR_FANG_NECKLACE) {
                    ctx.take_items(player, BEAR_FANG_NECKLACE, 1);
                    ctx.give_items(player, MARTANKUS_CHARM, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30649-07.html" => {
                if let Some(npc) = npc {
                    spawn_first_orc(ctx, npc);
                }
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            MARSH_SPIDER => {
                if all(ctx, killer, &[ORDEAL_NECKLACE, TAKUNA_CHARM])
                    && !has(ctx, killer, HANDIWORK_SPIDER_BROOCH)
                {
                    if ctx.get_quest_items_count(killer, MARSH_SPIDER_FEELER) < 10 {
                        collect(ctx, killer, MARSH_SPIDER_FEELER, 2, 10);
                    } else if ctx.get_quest_items_count(killer, MARSH_SPIDER_FEET) < 10 {
                        collect(ctx, killer, MARSH_SPIDER_FEET, 2, 10);
                    }
                }
            }
            BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => {
                if all(ctx, killer, &[ORDEAL_NECKLACE, VARKEES_CHARM, MANAKIAS_ORDERS])
                    && !any(ctx, killer, &[HUGE_ORC_FANG, MANAKIAS_AMULET])
                    && ctx.get_quest_items_count(killer, BREKA_ORC_FANG) < 20
                {
                    collect(ctx, killer, BREKA_ORC_FANG, 2, 20);
                }
            }
            ENCHANTED_MONSTEREYE => {
                if all(ctx, killer, &[ORDEAL_NECKLACE, CHIANTA_CHARM])
                    && !has(ctx, killer, MONSTER_EYE_WOODCARVING)
                    && ctx.get_quest_items_count(killer, ENCHANTED_MONSTER_CORNEA) < 20
                {
                    collect(ctx, killer, ENCHANTED_MONSTER_CORNEA, 1, 20);
                }
            }
            TIMAK_ORC | TIMAK_ORC_ARCHER | TIMAK_ORC_SOLDIER | TIMAK_ORC_WARRIOR | TIMAK_ORC_SHAMAN
            | TIMAK_ORC_OVERLORD => {
                if all(ctx, killer, &[ORDEAL_NECKLACE, HATOS_CHARM])
                    && !has(ctx, killer, SWORD_INTO_SKULL)
                    && ctx.get_quest_items_count(killer, TIMAK_ORC_SKULL) < 10
                {
                    collect(ctx, killer, TIMAK_ORC_SKULL, 1, 10);
                }
            }
            RAGNA_ORC_OVERLORD | RAGNA_ORC_SEER => {
                if has(ctx, killer, MARTANKUS_CHARM) {
                    if !has(ctx, killer, RAGNA_CHIEF_NOTICE) {
                        ctx.give_items(killer, RAGNA_CHIEF_NOTICE, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else if !has(ctx, killer, RAGNA_ORC_HEAD) {
                        ctx.give_items(killer, RAGNA_ORC_HEAD, 1);
                        qs.set_cond(ctx, 5, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        let p = player;
        if state == CREATED {
            if npc.id == FLAME_LORD_KAKAI {
                htmltext = if ctx.player_race(player) != race::ORC {
                    "30565-01.html"
                } else if ctx.player_class_id(player) != player_class::ORC_SHAMAN {
                    "30565-02.html"
                } else if ctx.player_level(player) < MIN_LEVEL {
                    "30565-03.html"
                } else {
                    "30565-04.htm"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                FLAME_LORD_KAKAI => {
                    if has(ctx, p, ORDEAL_NECKLACE) {
                        if all(
                            ctx,
                            p,
                            &[
                                HUGE_ORC_FANG,
                                SWORD_INTO_SKULL,
                                AXE_OF_CEREMONY,
                                MONSTER_EYE_WOODCARVING,
                                HANDIWORK_SPIDER_BROOCH,
                            ],
                        ) {
                            htmltext = "30565-07.html".to_owned();
                        } else {
                            htmltext = "30565-06.html".to_owned();
                        }
                    } else if has(ctx, p, BEAR_FANG_NECKLACE) {
                        htmltext = "30565-09.html".to_owned();
                    } else if has(ctx, p, MARTANKUS_CHARM) {
                        htmltext = "30565-10.html".to_owned();
                    } else if has(ctx, p, IMMORTAL_FLAME) {
                        ctx.give_adena(p, 161_806, true);
                        ctx.give_items(p, MARK_OF_LORD, 1);
                        ctx.add_exp_and_sp(p, 894_888, 61_408);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(p, 3);
                        htmltext = "30565-11.html".to_owned();
                    }
                }
                SEER_SOMAK => {
                    if all(ctx, p, &[ORDEAL_NECKLACE, HATOS_CHARM, SUMARIS_LETTER])
                        && !any(ctx, p, &[SWORD_INTO_SKULL, URUTU_BLADE])
                    {
                        ctx.take_items(p, SUMARIS_LETTER, 1);
                        ctx.give_items(p, URUTU_BLADE, 1);
                        htmltext = "30510-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HATOS_CHARM, URUTU_BLADE])
                        && !any(ctx, p, &[SWORD_INTO_SKULL, SUMARIS_LETTER])
                    {
                        htmltext = "30510-02.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, SWORD_INTO_SKULL])
                        && !any(ctx, p, &[HATOS_CHARM, URUTU_BLADE, SUMARIS_LETTER])
                    {
                        htmltext = "30510-03.html".to_owned();
                    }
                }
                SEER_MANAKIA => {
                    if all(ctx, p, &[ORDEAL_NECKLACE, VARKEES_CHARM])
                        && !any(ctx, p, &[HUGE_ORC_FANG, MANAKIAS_AMULET, MANAKIAS_ORDERS])
                    {
                        ctx.give_items(p, MANAKIAS_ORDERS, 1);
                        htmltext = "30515-01.html".to_owned();
                    } else if all(ctx, p, &[VARKEES_CHARM, ORDEAL_NECKLACE, MANAKIAS_ORDERS])
                        && !any(ctx, p, &[HUGE_ORC_FANG, MANAKIAS_AMULET])
                    {
                        if ctx.get_quest_items_count(p, BREKA_ORC_FANG) < 20 {
                            htmltext = "30515-02.html".to_owned();
                        } else {
                            ctx.take_items(p, MANAKIAS_ORDERS, 1);
                            ctx.take_items(p, BREKA_ORC_FANG, -1);
                            ctx.give_items(p, MANAKIAS_AMULET, 1);
                            htmltext = "30515-03.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, VARKEES_CHARM, MANAKIAS_AMULET])
                        && !any(ctx, p, &[HUGE_ORC_FANG, MANAKIAS_ORDERS])
                    {
                        htmltext = "30515-04.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HUGE_ORC_FANG])
                        && !any(ctx, p, &[VARKEES_CHARM, MANAKIAS_AMULET, MANAKIAS_ORDERS])
                    {
                        htmltext = "30515-05.html".to_owned();
                    }
                }
                TRADER_JAKAL => {
                    if all(ctx, p, &[ORDEAL_NECKLACE, TANTUS_CHARM])
                        && !any(ctx, p, &[AXE_OF_CEREMONY, NERUGA_AXE_BLADE])
                    {
                        if ctx.get_quest_items_count(p, ADENA) >= 1000 {
                            htmltext = "30558-01.html".to_owned();
                        } else {
                            htmltext = "30558-03.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, TANTUS_CHARM, NERUGA_AXE_BLADE])
                        && !has(ctx, p, AXE_OF_CEREMONY)
                    {
                        htmltext = "30558-04.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, AXE_OF_CEREMONY]) && !has(ctx, p, TANTUS_CHARM) {
                        htmltext = "30558-05.html".to_owned();
                    }
                }
                BLACKSMITH_SUMARI => {
                    if all(ctx, p, &[HATOS_CHARM, ORDEAL_NECKLACE])
                        && !any(ctx, p, &[SWORD_INTO_SKULL, URUTU_BLADE, SUMARIS_LETTER])
                    {
                        ctx.give_items(p, SUMARIS_LETTER, 1);
                        htmltext = "30564-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HATOS_CHARM, SUMARIS_LETTER])
                        && !any(ctx, p, &[SWORD_INTO_SKULL, URUTU_BLADE])
                    {
                        htmltext = "30564-02.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HATOS_CHARM, URUTU_BLADE])
                        && !any(ctx, p, &[SUMARIS_LETTER, SWORD_INTO_SKULL])
                    {
                        htmltext = "30564-03.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, SWORD_INTO_SKULL])
                        && !any(ctx, p, &[HATOS_CHARM, URUTU_BLADE, SUMARIS_LETTER])
                    {
                        htmltext = "30564-04.html".to_owned();
                    }
                }
                ATUBA_CHIEF_VARKEES => {
                    if has(ctx, p, ORDEAL_NECKLACE) && !any(ctx, p, &[HUGE_ORC_FANG, VARKEES_CHARM]) {
                        htmltext = "30566-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, VARKEES_CHARM])
                        && !any(ctx, p, &[HUGE_ORC_FANG, MANAKIAS_AMULET])
                    {
                        htmltext = "30566-03.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, VARKEES_CHARM, MANAKIAS_AMULET])
                        && !has(ctx, p, HUGE_ORC_FANG)
                    {
                        ctx.take_items(p, VARKEES_CHARM, 1);
                        ctx.take_items(p, MANAKIAS_AMULET, 1);
                        ctx.give_items(p, HUGE_ORC_FANG, 1);
                        if all(
                            ctx,
                            p,
                            &[
                                AXE_OF_CEREMONY,
                                SWORD_INTO_SKULL,
                                HANDIWORK_SPIDER_BROOCH,
                                MONSTER_EYE_WOODCARVING,
                            ],
                        ) {
                            qs.set_cond(ctx, 2, true);
                        }
                        htmltext = "30566-04.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HUGE_ORC_FANG]) && !has(ctx, p, VARKEES_CHARM) {
                        htmltext = "30566-05.html".to_owned();
                    }
                }
                NERUGA_CHIEF_TANTUS => {
                    if has(ctx, p, ORDEAL_NECKLACE) && !any(ctx, p, &[AXE_OF_CEREMONY, TANTUS_CHARM]) {
                        htmltext = "30567-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, TANTUS_CHARM]) && !has(ctx, p, AXE_OF_CEREMONY) {
                        if !has(ctx, p, NERUGA_AXE_BLADE) || ctx.get_quest_items_count(p, BONE_ARROW) < 1000 {
                            htmltext = "30567-03.html".to_owned();
                        } else {
                            ctx.take_items(p, BONE_ARROW, 1000);
                            ctx.take_items(p, TANTUS_CHARM, 1);
                            ctx.take_items(p, NERUGA_AXE_BLADE, 1);
                            ctx.give_items(p, AXE_OF_CEREMONY, 1);
                            if all(
                                ctx,
                                p,
                                &[
                                    HUGE_ORC_FANG,
                                    SWORD_INTO_SKULL,
                                    HANDIWORK_SPIDER_BROOCH,
                                    MONSTER_EYE_WOODCARVING,
                                ],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30567-04.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, AXE_OF_CEREMONY]) && !has(ctx, p, TANTUS_CHARM) {
                        htmltext = "30567-05.html".to_owned();
                    }
                }
                URUTU_CHIEF_HATOS => {
                    if has(ctx, p, ORDEAL_NECKLACE) && !any(ctx, p, &[SWORD_INTO_SKULL, HATOS_CHARM]) {
                        htmltext = "30568-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HATOS_CHARM]) && !has(ctx, p, SWORD_INTO_SKULL) {
                        if has(ctx, p, URUTU_BLADE) && ctx.get_quest_items_count(p, TIMAK_ORC_SKULL) >= 10 {
                            ctx.take_items(p, HATOS_CHARM, 1);
                            ctx.take_items(p, URUTU_BLADE, 1);
                            ctx.take_items(p, TIMAK_ORC_SKULL, -1);
                            ctx.give_items(p, SWORD_INTO_SKULL, 1);
                            if all(
                                ctx,
                                p,
                                &[
                                    HUGE_ORC_FANG,
                                    AXE_OF_CEREMONY,
                                    HANDIWORK_SPIDER_BROOCH,
                                    MONSTER_EYE_WOODCARVING,
                                ],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30568-04.html".to_owned();
                        } else {
                            htmltext = "30568-03.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, SWORD_INTO_SKULL]) && !has(ctx, p, HATOS_CHARM) {
                        htmltext = "30568-05.html".to_owned();
                    }
                }
                DUDA_MARA_CHIEF_TAKUNA => {
                    if has(ctx, p, ORDEAL_NECKLACE) && !any(ctx, p, &[HANDIWORK_SPIDER_BROOCH, TAKUNA_CHARM])
                    {
                        htmltext = "30641-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, TAKUNA_CHARM])
                        && !has(ctx, p, HANDIWORK_SPIDER_BROOCH)
                    {
                        if ctx.get_quest_items_count(p, MARSH_SPIDER_FEELER) >= 10
                            && ctx.get_quest_items_count(p, MARSH_SPIDER_FEET) >= 10
                        {
                            ctx.take_items(p, TAKUNA_CHARM, 1);
                            ctx.take_items(p, MARSH_SPIDER_FEELER, -1);
                            ctx.take_items(p, MARSH_SPIDER_FEET, -1);
                            ctx.give_items(p, HANDIWORK_SPIDER_BROOCH, 1);
                            if all(
                                ctx,
                                p,
                                &[
                                    HUGE_ORC_FANG,
                                    AXE_OF_CEREMONY,
                                    SWORD_INTO_SKULL,
                                    MONSTER_EYE_WOODCARVING,
                                ],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30641-04.html".to_owned();
                        } else {
                            htmltext = "30641-03.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, HANDIWORK_SPIDER_BROOCH])
                        && !has(ctx, p, TAKUNA_CHARM)
                    {
                        htmltext = "30641-05.html".to_owned();
                    }
                }
                GANDI_CHIEF_CHIANTA => {
                    if has(ctx, p, ORDEAL_NECKLACE) && !any(ctx, p, &[MONSTER_EYE_WOODCARVING, CHIANTA_CHARM])
                    {
                        htmltext = "30642-01.html".to_owned();
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, CHIANTA_CHARM])
                        && !has(ctx, p, MONSTER_EYE_WOODCARVING)
                    {
                        if ctx.get_quest_items_count(p, ENCHANTED_MONSTER_CORNEA) < 20 {
                            htmltext = "30642-03.html".to_owned();
                        } else {
                            ctx.take_items(p, CHIANTA_CHARM, 1);
                            ctx.take_items(p, ENCHANTED_MONSTER_CORNEA, -1);
                            ctx.give_items(p, MONSTER_EYE_WOODCARVING, 1);
                            if all(
                                ctx,
                                p,
                                &[
                                    HUGE_ORC_FANG,
                                    AXE_OF_CEREMONY,
                                    SWORD_INTO_SKULL,
                                    HANDIWORK_SPIDER_BROOCH,
                                ],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30642-04.html".to_owned();
                        }
                    } else if all(ctx, p, &[ORDEAL_NECKLACE, MONSTER_EYE_WOODCARVING])
                        && !has(ctx, p, CHIANTA_CHARM)
                    {
                        htmltext = "30642-05.html".to_owned();
                    }
                }
                FIRST_ORC => {
                    if any(ctx, p, &[MARTANKUS_CHARM, IMMORTAL_FLAME]) {
                        qs.set_cond(ctx, 7, true);
                        htmltext = "30643-01.html".to_owned();
                    }
                }
                ANCESTOR_MARTANKUS => {
                    if has(ctx, p, BEAR_FANG_NECKLACE) {
                        htmltext = "30649-01.html".to_owned();
                    } else if has(ctx, p, MARTANKUS_CHARM)
                        && !any(ctx, p, &[RAGNA_CHIEF_NOTICE, RAGNA_ORC_HEAD])
                    {
                        htmltext = "30649-05.html".to_owned();
                    } else if all(ctx, p, &[MARTANKUS_CHARM, RAGNA_CHIEF_NOTICE, RAGNA_ORC_HEAD]) {
                        ctx.take_items(p, MARTANKUS_CHARM, 1);
                        ctx.take_items(p, RAGNA_ORC_HEAD, 1);
                        ctx.take_items(p, RAGNA_CHIEF_NOTICE, 1);
                        ctx.give_items(p, IMMORTAL_FLAME, 1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30649-06.html".to_owned();
                    } else if has(ctx, p, IMMORTAL_FLAME) {
                        spawn_first_orc(ctx, npc);
                        htmltext = "30649-08.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == FLAME_LORD_KAKAI {
            htmltext = ctx.get_already_completed_msg(player);
        }

        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00232_TestOfTheLord";

    const NPCS: &str = r#"
        <npc id="30565" type="Folk" level="70" name="Flame Lord Kakai"/>
        <npc id="30566" type="Folk" level="70" name="Varkees"/>
        <npc id="30649" type="Folk" level="70" name="Martankus"/>
        <npc id="30643" type="Folk" level="70" name="First Orc"/>
        <npc id="20233" type="Monster" level="40" name="Marsh Spider"/>
        <npc id="20778" type="Monster" level="40" name="Ragna Orc Overlord"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3390" type="EtcItem" name="Mark of Lord"><set name="is_questitem" val="true"/></item>
        <item id="3391" type="EtcItem" name="Ordeal Necklace"><set name="is_questitem" val="true"/></item>
        <item id="3392" type="EtcItem" name="Varkees Charm"><set name="is_questitem" val="true"/></item>
        <item id="3395" type="EtcItem" name="Takuna Charm"><set name="is_questitem" val="true"/></item>
        <item id="3400" type="EtcItem" name="Huge Orc Fang"><set name="is_questitem" val="true"/></item>
        <item id="3404" type="EtcItem" name="Sword into Skull"><set name="is_questitem" val="true"/></item>
        <item id="3406" type="EtcItem" name="Axe of Ceremony"><set name="is_questitem" val="true"/></item>
        <item id="3407" type="EtcItem" name="Marsh Spider Feeler"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3408" type="EtcItem" name="Marsh Spider Feet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3409" type="EtcItem" name="Handiwork Spider Brooch"><set name="is_questitem" val="true"/></item>
        <item id="3411" type="EtcItem" name="Monster Eye Woodcarving"><set name="is_questitem" val="true"/></item>
        <item id="3412" type="EtcItem" name="Bear Fang Necklace"><set name="is_questitem" val="true"/></item>
        <item id="3413" type="EtcItem" name="Martankus Charm"><set name="is_questitem" val="true"/></item>
        <item id="3414" type="EtcItem" name="Ragna Orc Head"><set name="is_questitem" val="true"/></item>
        <item id="3415" type="EtcItem" name="Ragna Chief Notice"><set name="is_questitem" val="true"/></item>
        <item id="3416" type="EtcItem" name="Immortal Flame"><set name="is_questitem" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Q00232TestOfTheLord)
            .plain(&["30565-01.html", "30565-02.html", "30565-03.html"])
            .page("30565-04.htm", &["ACCEPT"])
            .page("30565-05b.htm", &["30565-05a.html"])
            .plain(&[
                "30565-05.htm",
                "30565-05a.html",
                "30565-06.html",
                "30565-09.html",
                "30565-10.html",
            ])
            .page("30565-07.html", &["30565-08.html"])
            .plain(&["30565-08.html", "30565-11.html"])
            .page("30566-01.html", &["30566-02.html"])
            .plain(&["30566-02.html", "30566-03.html", "30566-04.html", "30566-05.html"])
            .page("30649-01.html", &["30649-02.html"])
            .page("30649-02.html", &["30649-03.html"])
            .page("30649-03.html", &["30649-04.html"])
            .plain(&["30649-04.html", "30649-05.html", "30649-07.html", "30649-08.html"])
            .page("30649-06.html", &["30649-07.html"])
            .page("30643-01.html", &["30643-02.html"])
            .plain(&["30643-02.html", "30643-03.html"])
            .npcs(&[
                FLAME_LORD_KAKAI,
                ATUBA_CHIEF_VARKEES,
                ANCESTOR_MARTANKUS,
                FIRST_ORC,
            ])
    }

    struct Npcs {
        kakai: Oid,
        varkees: Oid,
        martankus: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00232TestOfTheLord);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            kakai: t.spawn(FLAME_LORD_KAKAI, 20),
            varkees: t.spawn(ATUBA_CHIEF_VARKEES, 40),
            martankus: t.spawn(ANCESTOR_MARTANKUS, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_orc_shaman_of_level_39_is_offered_the_test() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, 0, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(1, npcs.kakai).contains("30565-01.html"));
        t.enter_as(2, 40, 3, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.kakai).contains("30565-02.html"));
        t.enter_as(3, 38, 3, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(3, npcs.kakai).contains("30565-03.html"));
        t.enter_as(4, 39, 3, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(4, npcs.kakai).contains("30565-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_necklace_and_the_diamonds_once() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.kakai);
        assert!(t.event(1, NAME, "ACCEPT").contains("30565-05b.htm"));
        assert_eq!(
            (t.count(1, ORDEAL_NECKLACE), t.count(1, DIMENSIONAL_DIAMOND)),
            (1, 92)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_chief_hands_out_a_charm_and_the_spider_drops_come_in_twos() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.kakai);
        t.event(1, NAME, "ACCEPT");
        assert!(t.quest_page(1, npcs.varkees).contains("30566-01.html"));
        assert!(t.event(1, NAME, "30566-02.html").contains("30566-02.html"));
        assert_eq!(t.count(1, VARKEES_CHARM), 1);
        // Marsh spiders drop only with Takuna's charm.
        let spider = t.spawn(MARSH_SPIDER, 90);
        t.kill(1, spider);
        assert_eq!(t.count(1, MARSH_SPIDER_FEELER), 0);
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), TAKUNA_CHARM, 1));
        let spider2 = t.spawn(MARSH_SPIDER, 100);
        t.kill(1, spider2);
        assert_eq!(t.count(1, MARSH_SPIDER_FEELER), 2);
    }

    #[test]
    fn the_flame_is_carried_to_the_first_orc_and_ends_with_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.kakai);
        t.event(1, NAME, "ACCEPT");
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), HUGE_ORC_FANG, 1);
            ctx.give_items(Oid(1), SWORD_INTO_SKULL, 1);
            ctx.give_items(Oid(1), AXE_OF_CEREMONY, 1);
            ctx.give_items(Oid(1), HANDIWORK_SPIDER_BROOCH, 1);
            ctx.give_items(Oid(1), MONSTER_EYE_WOODCARVING, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.kakai).contains("30565-07.html"));
        assert!(t.event(1, NAME, "30565-08.html").contains("30565-08.html"));
        assert_eq!(
            (t.count(1, ORDEAL_NECKLACE), t.count(1, BEAR_FANG_NECKLACE)),
            (0, 1)
        );
        // cond 2 was skipped: `setCond` records it in `__compltdStateFlags`.
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643")
            ]
        );

        // Martankus swaps the necklace for his charm; the Ragna orcs drop notice then head.
        assert!(t.quest_page(1, npcs.martankus).contains("30649-01.html"));
        t.event(1, NAME, "30649-02.html");
        t.event(1, NAME, "30649-03.html");
        t.event(1, NAME, "30649-04.html");
        assert_eq!(t.count(1, MARTANKUS_CHARM), 1);
        for dx in [110, 120] {
            let orc = t.spawn(RAGNA_ORC_OVERLORD, dx);
            t.kill(1, orc);
        }
        assert_eq!(
            (t.count(1, RAGNA_CHIEF_NOTICE), t.count(1, RAGNA_ORC_HEAD)),
            (1, 1)
        );
        assert!(t.quest_page(1, npcs.martankus).contains("30649-06.html"));
        assert_eq!(t.count(1, IMMORTAL_FLAME), 1);

        // `addSpawn(npc, FIRST_ORC, ...)` once: the summoner's list keeps it.
        assert!(t.event(1, NAME, "30649-07.html").contains("30649-07.html"));
        assert_eq!(t.npcs_of(FIRST_ORC).len(), 1);
        t.event(1, NAME, "30649-07.html");
        assert_eq!(t.npcs_of(FIRST_ORC).len(), 1);

        t.take_writes();
        t.take();
        t.talk(1, npcs.kakai);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30565-11.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!((t.count(1, MARK_OF_LORD), t.count(1, 57)), (1, 161_806));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
