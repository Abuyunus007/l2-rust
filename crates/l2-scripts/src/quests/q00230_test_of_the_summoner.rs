//! Quest 230: Test of the Summoner (High Summoner Galatea in Talking Island: the Wizard, Elven
//! Wizard and Dark Wizard second class change, through Lara and six summoners whose duel
//! monsters fight the player's servitor).
// Java: dist/game/data/scripts/quests/Q00230_TestOfTheSummoner/Q00230_TestOfTheSummoner.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222), with duel monsters that
// react to the player's summon (`onAttack` with `isSummon`, the `ATTACKER` NPC variable, the
// `DESPAWN` and `KILLED_ATTACKER` NPC timers) and the six summoners who hand out the duels.
// The six summoners differ only in the crystal set (index n: items 3360 + 5 * (n - 1) + 0..4 =
// starting, in progress, foul, defeat, victory), so one function writes their pages.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{Qs, QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const GROCER_LARA: i32 = 30063;
const HIGH_SUMMONER_GALATEA: i32 = 30634;
const SUMMONER_ALMORS: i32 = 30635;
const SUMMONER_CAMONIELL: i32 = 30636;
const SUMMONER_BELTHUS: i32 = 30637;
const SUMMONER_BASILLA: i32 = 30638;
const SUMMONER_CELESTIEL: i32 = 30639;
const SUMMONER_BRYNTHEA: i32 = 30640;

// Items
const LETOLIZARDMAN_AMULET: i32 = 3337;
const SAC_OF_REDSPORES: i32 = 3338;
const KARULBUGBEAR_TOTEM: i32 = 3339;
const SHARDS_OF_MANASHEN: i32 = 3340;
const BREKAORC_TOTEM: i32 = 3341;
const CRIMSON_BLOODSTONE: i32 = 3342;
const TALONS_OF_TYRANT: i32 = 3343;
const WINGS_OF_DRONEANT: i32 = 3344;
const TUSK_OF_WINDSUS: i32 = 3345;
const FANGS_OF_WYRM: i32 = 3346;
const LARAS_1ST_LIST: i32 = 3347;
const LARAS_2ND_LIST: i32 = 3348;
const LARAS_3RD_LIST: i32 = 3349;
const LARAS_4TH_LIST: i32 = 3350;
const LARAS_5TH_LIST: i32 = 3351;
const GALATEAS_LETTER: i32 = 3352;
const BEGINNERS_ARCANA: i32 = 3353;
const ALMORS_ARCANA: i32 = 3354;
const CAMONIELL_ARCANA: i32 = 3355;
const BELTHUS_ARCANA: i32 = 3356;
const BASILLIA_ARCANA: i32 = 3357;
const CELESTIEL_ARCANA: i32 = 3358;
const BRYNTHEA_ARCANA: i32 = 3359;

/// The crystals of duel `n` (1 to 6): starting, in progress, foul, defeat, victory.
const fn crystals(n: i32) -> [i32; 5] {
    let first = 3360 + 5 * (n - 1);
    [first, first + 1, first + 2, first + 3, first + 4]
}

// Reward
const MARK_OF_SUMMONER: i32 = 3336;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const WYRM: i32 = 20176;
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const BREKA_ORC: i32 = 20267;
const BREKA_ORC_ARCHER: i32 = 20268;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const BREKA_ORC_WARRIOR: i32 = 20271;
const FETTERED_SOUL: i32 = 20552;
const WINDSUS: i32 = 20553;
const GIANT_FUNGUS: i32 = 20555;
const MANASHEN_GARGOYLE: i32 = 20563;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const KARUL_BUGBEAR: i32 = 20600;

// Quest Monster
const PAKO_THE_CAT: i32 = 27102;
const UNICORN_RACER: i32 = 27103;
const SHADOW_TUREN: i32 = 27104;
const MIMI_THE_CAT: i32 = 27105;
const UNICORN_PHANTASM: i32 = 27106;
const SILHOUETTE_TILFO: i32 = 27107;

// Skill
const REDUCTION_IN_RECOVERY_TIME: (i32, i32) = (4126, 1);

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `MonsterData` plus the duel number: (monster, duel, what it says at the start, what it
/// says when it loses).
fn duel_monster(npc_id: i32) -> Option<(i32, NpcStringId, NpcStringId)> {
    match npc_id {
        PAKO_THE_CAT => Some((1, NpcStringId::Whhiisshh, NpcStringId::IMSorryLord)),
        UNICORN_RACER => Some((3, NpcStringId::StartDuel, NpcStringId::ILose)),
        SHADOW_TUREN => Some((5, NpcStringId::SoShallWeStart, NpcStringId::UghILost)),
        MIMI_THE_CAT => Some((2, NpcStringId::WhishFight, NpcStringId::LostSorryLord)),
        UNICORN_PHANTASM => Some((4, NpcStringId::StartDuel, NpcStringId::ILose)),
        SILHOUETTE_TILFO => Some((
            6,
            NpcStringId::ILlWalkAllOverYou,
            NpcStringId::UghCanThisBeHappening,
        )),
        _ => None,
    }
}

/// The summoner of a duel: (NPC id, page prefix is the NPC id, duel number, the arcana he gives).
fn summoner(npc_id: i32) -> Option<(i32, i32)> {
    match npc_id {
        SUMMONER_ALMORS => Some((1, ALMORS_ARCANA)),
        SUMMONER_BASILLA => Some((2, BASILLIA_ARCANA)),
        SUMMONER_CAMONIELL => Some((3, CAMONIELL_ARCANA)),
        SUMMONER_CELESTIEL => Some((4, CELESTIEL_ARCANA)),
        SUMMONER_BELTHUS => Some((5, BELTHUS_ARCANA)),
        SUMMONER_BRYNTHEA => Some((6, BRYNTHEA_ARCANA)),
        _ => None,
    }
}

const ARCANAS: [i32; 6] = [
    ALMORS_ARCANA,
    BASILLIA_ARCANA,
    CAMONIELL_ARCANA,
    CELESTIEL_ARCANA,
    BELTHUS_ARCANA,
    BRYNTHEA_ARCANA,
];

/// Java `Q00230_TestOfTheSummoner`.
pub struct Q00230TestOfTheSummoner;

impl Script for Q00230TestOfTheSummoner {
    fn name(&self) -> &'static str {
        "Q00230_TestOfTheSummoner"
    }

    fn path(&self) -> &'static str {
        "quests/Q00230_TestOfTheSummoner"
    }

    fn id(&self) -> i32 {
        230
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIGH_SUMMONER_GALATEA]);
        s.add_talk_id(&[
            HIGH_SUMMONER_GALATEA,
            GROCER_LARA,
            SUMMONER_ALMORS,
            SUMMONER_CAMONIELL,
            SUMMONER_BELTHUS,
            SUMMONER_BASILLA,
            SUMMONER_CELESTIEL,
            SUMMONER_BRYNTHEA,
        ]);
        s.add_kill_id(&[
            NOBLE_ANT,
            NOBLE_ANT_LEADER,
            WYRM,
            TYRANT,
            TYRANT_KINGPIN,
            BREKA_ORC,
            BREKA_ORC_ARCHER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            BREKA_ORC_WARRIOR,
            FETTERED_SOUL,
            WINDSUS,
            GIANT_FUNGUS,
            MANASHEN_GARGOYLE,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            KARUL_BUGBEAR,
        ]);
        s.add_kill_id(&[
            PAKO_THE_CAT,
            UNICORN_RACER,
            SHADOW_TUREN,
            MIMI_THE_CAT,
            UNICORN_PHANTASM,
            SILHOUETTE_TILFO,
        ]);
        s.add_attack_id(&[
            PAKO_THE_CAT,
            UNICORN_RACER,
            SHADOW_TUREN,
            MIMI_THE_CAT,
            UNICORN_PHANTASM,
            SILHOUETTE_TILFO,
        ]);
        let mut items = vec![
            LETOLIZARDMAN_AMULET,
            SAC_OF_REDSPORES,
            KARULBUGBEAR_TOTEM,
            SHARDS_OF_MANASHEN,
            BREKAORC_TOTEM,
            CRIMSON_BLOODSTONE,
            TALONS_OF_TYRANT,
            WINGS_OF_DRONEANT,
            TUSK_OF_WINDSUS,
            FANGS_OF_WYRM,
            LARAS_1ST_LIST,
            LARAS_2ND_LIST,
            LARAS_3RD_LIST,
            LARAS_4TH_LIST,
            LARAS_5TH_LIST,
            GALATEAS_LETTER,
            BEGINNERS_ARCANA,
            ALMORS_ARCANA,
            CAMONIELL_ARCANA,
            BELTHUS_ARCANA,
            BASILLIA_ARCANA,
            CELESTIEL_ARCANA,
            BRYNTHEA_ARCANA,
        ];
        for n in 1..=6 {
            items.extend(crystals(n));
        }
        s.register_quest_items(&items);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "DESPAWN" => {
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
            }
            "KILLED_ATTACKER" => {
                if let Some(npc) = npc {
                    let summon = ctx
                        .world()
                        .npc_variables(npc.object_id)
                        .map_or(0, |v| v.get_int("ATTACKER", 0));
                    if summon != 0 && ctx.is_dead(ObjectId(summon)) {
                        ctx.delete_npc(npc);
                    } else {
                        ctx.start_quest_timer("KILLED_ATTACKER", 5000, Some(npc), None, false);
                    }
                }
            }
            _ => {}
        }

        // For NPC-only timers, player is null and no further checks or actions are required.
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, GALATEAS_LETTER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 122);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30634-08a.htm".to_owned());
                    } else {
                        htmltext = Some("30634-08.htm".to_owned());
                    }
                }
            }
            "30634-04.htm" | "30634-05.htm" | "30634-06.htm" | "30634-07.htm" | "30634-11.html"
            | "30634-11a.html" | "30634-11b.html" | "30634-11c.html" | "30634-11d.html" => {
                htmltext = Some(event.to_owned());
            }
            "30063-02.html" => {
                give_lara_list(ctx, player);
                qs.set_cond(ctx, 2, true);
                ctx.take_items(player, GALATEAS_LETTER, 1);
                htmltext = Some(event.to_owned());
            }
            "30063-04.html" => {
                give_lara_list(ctx, player);
                htmltext = Some(event.to_owned());
            }
            // The six summoners: "<id>-03.html" asks for the beginner's arcana, "<id>-04.html"
            // starts the duel.
            _ => {
                let id = event.get(..5).and_then(|p| p.parse::<i32>().ok());
                let step = event.get(5..);
                if let (Some(id), Some(step)) = (id, step)
                    && let Some((duel, _)) = summoner(id)
                {
                    if step == "-03.html" {
                        if ctx.has_quest_items(player, BEGINNERS_ARCANA) {
                            htmltext = Some(event.to_owned());
                        } else {
                            htmltext = Some(format!("{id}-02.html"));
                        }
                    } else if step == "-04.html" {
                        let npc = npc?;
                        let [starting, _, foul, defeat, _] = crystals(duel);
                        ctx.add_skill_cast_desire(
                            npc,
                            player,
                            SkillKey::new(REDUCTION_IN_RECOVERY_TIME.0, REDUCTION_IN_RECOVERY_TIME.1),
                            1_000_000,
                        );
                        ctx.take_items(player, BEGINNERS_ARCANA, 1);
                        ctx.give_items(player, starting, 1);
                        ctx.take_items(player, foul, 1);
                        ctx.take_items(player, defeat, 1);
                        htmltext = Some(event.to_owned());
                    }
                }
            }
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some((duel, start_say, _)) = duel_monster(npc.id) else {
            return;
        };
        let [starting, in_progress, foul, _, _] = crystals(duel);
        let summon = ctx.summon_of(attacker);
        match ctx.world().npc_script_value(npc.object_id) {
            0 => {
                if is_summon {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("ATTACKER", summon.map_or(0, |s| s.0));
                    ctx.set_script_value(npc, 1);
                    ctx.start_quest_timer("DESPAWN", 120_000, Some(npc), None, false);
                    ctx.start_quest_timer("KILLED_ATTACKER", 5000, Some(npc), None, false);

                    let started = ctx
                        .get_quest_state(attacker, false)
                        .is_some_and(|qs| qs.is_started(ctx));
                    if ctx.has_quest_items(attacker, starting) && started {
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, start_say, &[]);
                        ctx.take_items(attacker, starting, -1);
                        ctx.give_items(attacker, in_progress, 1);
                        // Java passes `attacker.getSummon()` (not null: `isSummon` is true).
                        if let Some(summon) = summon {
                            ctx.add_attack_desire(npc, summon, 100_000);
                        }
                    }
                }
            }
            1 => {
                let stored = ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .map_or(0, |v| v.get_int("ATTACKER", 0));
                if !is_summon || summon.map_or(0, |s| s.0) != stored {
                    let started = ctx
                        .get_quest_state(attacker, false)
                        .is_some_and(|qs| qs.is_started(ctx));
                    if !ctx.has_quest_items(attacker, starting)
                        && ctx.has_quest_items(attacker, in_progress)
                        && started
                    {
                        ctx.set_script_value(npc, 2);
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::RuleViolation, &[]);
                        ctx.take_items(attacker, in_progress, -1);
                        ctx.give_items(attacker, foul, 1);
                        ctx.take_items(attacker, starting, -1);
                    }
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        // (list the player needs, item, amount per kill); Java `giveItemRandomly(killer, npc, item,
        // amount, 30, 1, true)`.
        let drop: Option<(i32, i32, i64)> = match npc.id {
            NOBLE_ANT | NOBLE_ANT_LEADER => Some((LARAS_5TH_LIST, WINGS_OF_DRONEANT, 2)),
            WYRM => Some((LARAS_5TH_LIST, FANGS_OF_WYRM, 3)),
            TYRANT | TYRANT_KINGPIN => Some((LARAS_4TH_LIST, TALONS_OF_TYRANT, 3)),
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_WARRIOR => Some((LARAS_3RD_LIST, BREKAORC_TOTEM, 1)),
            BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => Some((LARAS_3RD_LIST, BREKAORC_TOTEM, 2)),
            FETTERED_SOUL => Some((LARAS_3RD_LIST, CRIMSON_BLOODSTONE, 6)),
            WINDSUS => Some((LARAS_4TH_LIST, TUSK_OF_WINDSUS, 3)),
            GIANT_FUNGUS => Some((LARAS_1ST_LIST, SAC_OF_REDSPORES, 2)),
            MANASHEN_GARGOYLE => Some((LARAS_2ND_LIST, SHARDS_OF_MANASHEN, 2)),
            LETO_LIZARDMAN | LETO_LIZARDMAN_ARCHER | LETO_LIZARDMAN_SOLDIER | LETO_LIZARDMAN_WARRIOR => {
                Some((LARAS_1ST_LIST, LETOLIZARDMAN_AMULET, 1))
            }
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => {
                Some((LARAS_1ST_LIST, LETOLIZARDMAN_AMULET, 2))
            }
            KARUL_BUGBEAR => Some((LARAS_2ND_LIST, KARULBUGBEAR_TOTEM, 2)),
            _ => None,
        };
        if let Some((list, item, amount)) = drop {
            if !ctx.has_quest_items(killer, GALATEAS_LETTER) && ctx.has_quest_items(killer, list) {
                ctx.give_item_randomly(killer, Some(npc), item, amount, amount, 30, 1.0, true);
            }
        } else if let Some((duel, _, lose_say)) = duel_monster(npc.id) {
            let [_, in_progress, _, _, victory] = crystals(duel);
            if ctx.has_quest_items(killer, in_progress) {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, lose_say, &[]);
                ctx.take_items(killer, in_progress, 1);
                ctx.give_items(killer, victory, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == HIGH_SUMMONER_GALATEA {
                let class = ctx.player_class_id(player);
                if class == player_class::WIZARD
                    || class == player_class::ELVEN_WIZARD
                    || class == player_class::DARK_WIZARD
                {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "30634-03.htm".to_owned();
                    } else {
                        htmltext = "30634-02.html".to_owned();
                    }
                } else {
                    htmltext = "30634-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                HIGH_SUMMONER_GALATEA => {
                    if ctx.has_quest_items(player, GALATEAS_LETTER) {
                        htmltext = "30634-09.html".to_owned();
                    } else {
                        let all_arcanas = ctx.has_all_quest_items(player, &ARCANAS);
                        let beginner = ctx.has_quest_items(player, BEGINNERS_ARCANA);
                        if !all_arcanas && !beginner {
                            htmltext = "30634-10.html".to_owned();
                        } else if !all_arcanas && beginner {
                            htmltext = "30634-11.html".to_owned();
                        } else if all_arcanas {
                            ctx.give_adena(player, 300_960, true);
                            ctx.give_items(player, MARK_OF_SUMMONER, 1);
                            ctx.add_exp_and_sp(player, 1_664_494, 114_220);
                            qs.exit_quest(ctx, false, true);
                            ctx.send_social_action(player, 3);
                            htmltext = "30634-12.html".to_owned();
                        }
                    }
                }
                GROCER_LARA => {
                    if let Some(page) = lara_page(ctx, qs, player) {
                        htmltext = page.to_owned();
                    }
                }
                _ => {
                    if let Some(page) = summoner_page(ctx, qs, npc.id, player) {
                        htmltext = page;
                    }
                }
            }
        } else if state == COMPLETED && npc.id == HIGH_SUMMONER_GALATEA {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// Lara's list: one of five by `getRandom(5)`.
fn give_lara_list(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let list = match ctx.get_random(5) {
        0 => LARAS_1ST_LIST,
        1 => LARAS_2ND_LIST,
        2 => LARAS_3RD_LIST,
        3 => LARAS_4TH_LIST,
        _ => LARAS_5TH_LIST,
    };
    ctx.give_items(player, list, 1);
}

/// Grocer Lara in a started quest: the list, what it needs (30 of two items) and the pages.
fn lara_page(ctx: &mut ScriptCtx<'_>, qs: Qs, p: ObjectId) -> Option<&'static str> {
    if ctx.has_quest_items(p, GALATEAS_LETTER) {
        return Some("30063-01.html");
    }
    let lists: [(i32, i32, i32, &'static str, &'static str); 5] = [
        (
            LARAS_1ST_LIST,
            LETOLIZARDMAN_AMULET,
            SAC_OF_REDSPORES,
            "30063-05.html",
            "30063-06.html",
        ),
        (
            LARAS_2ND_LIST,
            KARULBUGBEAR_TOTEM,
            SHARDS_OF_MANASHEN,
            "30063-07.html",
            "30063-08.html",
        ),
        (
            LARAS_3RD_LIST,
            BREKAORC_TOTEM,
            CRIMSON_BLOODSTONE,
            "30063-09.html",
            "30063-10.html",
        ),
        (
            LARAS_4TH_LIST,
            TALONS_OF_TYRANT,
            TUSK_OF_WINDSUS,
            "30063-11.html",
            "30063-12.html",
        ),
        (
            LARAS_5TH_LIST,
            WINGS_OF_DRONEANT,
            FANGS_OF_WYRM,
            "30063-13.html",
            "30063-14.html",
        ),
    ];
    if !ctx.has_at_least_one_quest_item(p, &lists.map(|l| l.0)) {
        return Some("30063-03.html");
    }
    for (list, first, second, not_yet, done) in lists {
        if ctx.has_quest_items(p, list) {
            if ctx.get_quest_items_count(p, first) >= 30 && ctx.get_quest_items_count(p, second) >= 30 {
                ctx.take_items(p, first, -1);
                ctx.take_items(p, second, -1);
                ctx.take_items(p, list, 1);
                ctx.give_items(p, BEGINNERS_ARCANA, 2);
                qs.set_cond(ctx, 3, true);
                return Some(done);
            }
            return Some(not_yet);
        }
    }
    None
}

/// One of the six summoners in a started quest (pages `<id>-01` to `<id>-10`).
fn summoner_page(ctx: &mut ScriptCtx<'_>, qs: Qs, npc_id: i32, p: ObjectId) -> Option<String> {
    let (duel, arcana) = summoner(npc_id)?;
    if ctx.has_quest_items(p, arcana) {
        return Some(format!("{npc_id}-10.html"));
    }
    let [starting, in_progress, foul, defeat, victory] = crystals(duel);
    let h = |ctx: &ScriptCtx<'_>, item: i32| ctx.has_quest_items(p, item);
    let page = |n: &str| Some(format!("{npc_id}-{n}.html"));
    if !ctx.has_at_least_one_quest_item(p, &[starting, in_progress, foul, defeat, victory]) {
        page("01")
    } else if !ctx.has_at_least_one_quest_item(p, &[starting, in_progress, foul, victory]) && h(ctx, defeat) {
        page("05")
    } else if !ctx.has_at_least_one_quest_item(p, &[starting, in_progress, defeat, victory]) && h(ctx, foul) {
        page("06")
    } else if !ctx.has_at_least_one_quest_item(p, &[starting, in_progress, foul, defeat]) && h(ctx, victory) {
        ctx.give_items(p, arcana, 1);
        ctx.take_items(p, victory, 1);
        let others: Vec<i32> = ARCANAS.iter().copied().filter(|&a| a != arcana).collect();
        if ctx.has_all_quest_items(p, &others) {
            qs.set_cond(ctx, 4, true);
        }
        page("07")
    } else if !ctx.has_at_least_one_quest_item(p, &[in_progress, foul, defeat, victory]) && h(ctx, starting) {
        page("08")
    } else if !ctx.has_at_least_one_quest_item(p, &[starting, foul, defeat, victory]) && h(ctx, in_progress) {
        page("09")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00230_TestOfTheSummoner";

    const NPCS: &str = r#"
        <npc id="30634" type="Folk" level="70" name="High Summoner Galatea"/>
        <npc id="30063" type="Folk" level="70" name="Grocer Lara"/>
        <npc id="30635" type="Folk" level="70" name="Summoner Almors"/>
        <npc id="20555" type="Monster" level="40" name="Giant Fungus"/>
        <npc id="27102" type="Monster" level="40" name="Pako the Cat"/>"#;

    fn items() -> String {
        let mut items = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3336" type="EtcItem" name="Mark of Summoner"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in 3337..=3359 {
            items.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q230 {id}"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#
            ));
        }
        for id in 3360..=3389 {
            items.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q230 crystal {id}"><set name="is_questitem" val="true"/></item>"#
            ));
        }
        items
    }

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00230TestOfTheSummoner)
            .plain(&[
                "30634-01.html",
                "30634-02.html",
                "30634-08.htm",
                "30634-08a.htm",
                "30634-12.html",
            ])
            .page("30634-03.htm", &["30634-04.htm"])
            .page("30634-04.htm", &["30634-05.htm"])
            .page("30634-05.htm", &["30634-06.htm"])
            .page("30634-06.htm", &["30634-07.htm"])
            .page("30634-07.htm", &["ACCEPT"])
            .page("30063-01.html", &["30063-02.html"])
            .plain(&[
                "30063-02.html",
                "30063-03.html",
                "30063-04.html",
                "30063-05.html",
                "30063-06.html",
            ])
            .page("30635-01.html", &["30635-03.html"])
            .page("30635-03.html", &["30635-04.html"])
            .plain(&[
                "30635-02.html",
                "30635-04.html",
                "30635-07.html",
                "30635-08.html",
                "30635-09.html",
            ])
            .npcs(&[HIGH_SUMMONER_GALATEA, GROCER_LARA, SUMMONER_ALMORS])
    }

    struct Npcs {
        galatea: Oid,
        lara: Oid,
        almors: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00230TestOfTheSummoner);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages());
        let npcs = Npcs {
            galatea: t.spawn(HIGH_SUMMONER_GALATEA, 20),
            lara: t.spawn(GROCER_LARA, 30),
            almors: t.spawn(SUMMONER_ALMORS, 40),
        };
        (t, npcs)
    }

    /// Galatea's pages from the first to the accept.
    fn start(t: &mut ScriptTest, galatea: Oid) {
        t.quest_page(1, galatea);
        for page in [
            "30634-04.htm",
            "30634-05.htm",
            "30634-06.htm",
            "30634-07.htm",
            "ACCEPT",
        ] {
            t.event(1, NAME, page);
        }
    }

    #[test]
    fn the_start_is_gated_by_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.galatea).contains("30634-01.html"));
        t.enter_as(2, 38, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(2, npcs.galatea).contains("30634-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accept_gives_the_letter_and_122_diamonds_and_lara_gives_a_list() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.galatea);
        assert_eq!(t.count(1, GALATEAS_LETTER), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 122);
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));

        assert!(t.quest_page(1, npcs.lara).contains("30063-01.html"));
        assert!(t.event(1, NAME, "30063-02.html").contains("30063-02.html"));
        assert_eq!(t.count(1, GALATEAS_LETTER), 0);
        let lists: i64 = (LARAS_1ST_LIST..=LARAS_5TH_LIST).map(|l| t.count(1, l)).sum();
        assert_eq!(lists, 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
    }

    #[test]
    fn the_fungus_drops_spores_for_the_first_list_up_to_thirty() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.galatea);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), GALATEAS_LETTER, 1);
            ctx.give_items(Oid(1), LARAS_1ST_LIST, 1);
        });
        for _ in 0..16 {
            let fungus = t.spawn(GIANT_FUNGUS, 60);
            t.kill(1, fungus);
            t.world.delete_npc(fungus);
        }
        assert_eq!(t.count(1, SAC_OF_REDSPORES), 30);
    }

    #[test]
    fn almors_hands_out_the_duel_and_the_summoners_pet_wins_it() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.galatea);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), GALATEAS_LETTER, 1);
            ctx.give_items(Oid(1), BEGINNERS_ARCANA, 2);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.almors).contains("30635-01.html"));
        assert!(t.event(1, NAME, "30635-03.html").contains("30635-03.html"));
        assert!(t.event(1, NAME, "30635-04.html").contains("30635-04.html"));
        assert_eq!(t.count(1, BEGINNERS_ARCANA), 1);
        assert_eq!(t.count(1, crystals(1)[0]), 1);

        // Pako the Cat is hit by the summon: the duel starts (the summon is faked: the hook is
        // called with `is_summon` and no summon in the test world).
        let pako = t.spawn(PAKO_THE_CAT, 60);
        t.world.run_as_script(NAME, |ctx| {
            let n = ScriptNpc {
                object_id: pako,
                id: PAKO_THE_CAT,
            };
            Q00230TestOfTheSummoner.on_attack(ctx, n, Oid(1), 1, true, None);
        });
        assert_eq!(t.count(1, crystals(1)[0]), 0);
        assert_eq!(t.count(1, crystals(1)[1]), 1);
        // The pet kills it: the victory crystal.
        t.kill(1, pako);
        assert_eq!(t.count(1, crystals(1)[1]), 0);
        assert_eq!(t.count(1, crystals(1)[4]), 1);

        // Almors swaps the crystal for his arcana.
        assert!(t.quest_page(1, npcs.almors).contains("30635-07.html"));
        assert_eq!(t.count(1, ALMORS_ARCANA), 1);
        assert_eq!(t.count(1, crystals(1)[4]), 0);
    }

    #[test]
    fn galatea_pays_the_reward_for_the_six_arcanas() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.galatea);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), GALATEAS_LETTER, 1);
            for arcana in ARCANAS {
                ctx.give_items(Oid(1), arcana, 1);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.galatea).contains("30634-12.html"));
        assert_eq!(t.count(1, MARK_OF_SUMMONER), 1);
        assert_eq!(t.count(1, 57), 300_960);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.galatea)
                .contains("This quest has already been completed.")
        );
    }
}
