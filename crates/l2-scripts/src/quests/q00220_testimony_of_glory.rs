//! Quest 220: Testimony of Glory (Prefect Vokian in the Orc Village: the Orc second class change,
//! through Kasman, Manakia, Tanapi, Kakai and the chiefs of the five tribes).
// Java: dist/game/data/scripts/quests/Q00220_TestimonyOfGlory/Q00220_TestimonyOfGlory.java
//
// Pattern: the test chain of the "Testimony of ..." quests (see Q00221 for the item ladders):
// chiefs that call their guards at the player (`addAttackDesire(addSpawn(npc, id, npc, ..))`,
// counted by `getSummonedNpcCount`), radar markers and monsters that talk when they are hit.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::subclass::CategoryType;
use l2_world::{Qs, QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PREFECT_KASMAN: i32 = 30501;
const PREFECT_VOKIAN: i32 = 30514;
const SEER_MANAKIA: i32 = 30515;
const FLAME_LORD_KAKAI: i32 = 30565;
const SEER_TANAPI: i32 = 30571;
const BREKA_CHIEF_VOLTAR: i32 = 30615;
const ENKU_CHIEF_KEPRA: i32 = 30616;
const TUREK_CHIEF_BURAI: i32 = 30617;
const LEUNT_CHIEF_HARAK: i32 = 30618;
const VUKU_CHIEF_DRIKO: i32 = 30619;
const GANDI_CHIEF_CHIANTA: i32 = 30642;

// Items
const VOKIANS_ORDER: i32 = 3204;
const MANASHEN_SHARD: i32 = 3205;
const TYRANT_TALON: i32 = 3206;
const GUARDIAN_BASILISK_FANG: i32 = 3207;
const VOKIANS_ORDER2: i32 = 3208;
const NECKLACE_OF_AUTHORITY: i32 = 3209;
const CHIANTA_1ST_ORDER: i32 = 3210;
const SCEPTER_OF_BREKA: i32 = 3211;
const SCEPTER_OF_ENKU: i32 = 3212;
const SCEPTER_OF_VUKU: i32 = 3213;
const SCEPTER_OF_TUREK: i32 = 3214;
const SCEPTER_OF_TUNATH: i32 = 3215;
const CHIANTA_2ND_ORDER: i32 = 3216;
const CHIANTA_3RD_ORDER: i32 = 3217;
const TAMLIN_ORC_SKULL: i32 = 3218;
const TIMAK_ORC_HEAD: i32 = 3219;
const SCEPTER_BOX: i32 = 3220;
const PASHIKAS_HEAD: i32 = 3221;
const VULTUS_HEAD: i32 = 3222;
const GLOVE_OF_VOLTAR: i32 = 3223;
const ENKU_OVERLORD_HEAD: i32 = 3224;
const GLOVE_OF_KEPRA: i32 = 3225;
const MAKUM_BUGBEAR_HEAD: i32 = 3226;
const GLOVE_OF_BURAI: i32 = 3227;
const MANAKIA_1ST_LETTER: i32 = 3228;
const MANAKIA_2ND_LETTER: i32 = 3229;
const KASMANS_1ST_LETTER: i32 = 3230;
const KASMANS_2ND_LETTER: i32 = 3231;
const KASMANS_3RD_LETTER: i32 = 3232;
const DRIKOS_CONTRACT: i32 = 3233;
const STAKATO_DRONE_HUSK: i32 = 3234;
const TANAPIS_ORDER: i32 = 3235;
const SCEPTER_OF_TANTOS: i32 = 3236;
const RITUAL_BOX: i32 = 3237;

// Reward
const MARK_OF_GLORY: i32 = 3203;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const MARSH_STAKATO_DRONE: i32 = 20234;
const GUARDIAN_BASILISK: i32 = 20550;
const MANASHEN_GARGOYLE: i32 = 20563;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const TIMAK_ORC_SOLDIER: i32 = 20585;
const TIMAK_ORC_WARRIOR: i32 = 20586;
const TIMAK_ORC_SHAMAN: i32 = 20587;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const RAGNA_ORC_OVERLORD: i32 = 20778;
const RAGNA_ORC_SEER: i32 = 20779;

// Quest Monster
const PASHIKA_SON_OF_VOLTAR: i32 = 27080;
const VULTUS_SON_OF_VOLTAR: i32 = 27081;
const ENKU_ORC_OVERLORD: i32 = 27082;
const MAKUM_BUGBEAR_THUG: i32 = 27083;
const REVENANT_OF_TANTOS_CHIEF: i32 = 27086;

// Misc
const MIN_LEVEL: i32 = 37;

fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

/// Java `hasQuestItems(player, a, b, ..)`: all of them.
fn all(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(player, items)
}

/// Java `hasAtLeastOneQuestItem(player, a, b, ..)`.
fn any(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(player, items)
}

/// Java `addAttackDesire(addSpawn(npc, id, npc, true, 200000), player)`: a guard of the chief,
/// counted in the chief's summoned NPCs, that attacks the player at once.
fn call_guard(ctx: &mut ScriptCtx<'_>, chief: ScriptNpc, guard_id: i32, player: ObjectId) {
    let Some((x, y, z)) = ctx.world().position_of(chief.object_id) else {
        return;
    };
    let heading = ctx.heading_of(chief.object_id);
    if let Some(guard) = ctx.add_spawn_summoned(chief, guard_id, x, y, z, heading, true, 200_000, 0) {
        ctx.add_attack_desire(guard, player, 999);
    }
}

/// Java `Q00220_TestimonyOfGlory`.
pub struct Q00220TestimonyOfGlory;

impl Script for Q00220TestimonyOfGlory {
    fn name(&self) -> &'static str {
        "Q00220_TestimonyOfGlory"
    }

    fn path(&self) -> &'static str {
        "quests/Q00220_TestimonyOfGlory"
    }

    fn id(&self) -> i32 {
        220
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PREFECT_VOKIAN]);
        s.add_talk_id(&[
            PREFECT_VOKIAN,
            PREFECT_KASMAN,
            SEER_MANAKIA,
            FLAME_LORD_KAKAI,
            SEER_TANAPI,
            BREKA_CHIEF_VOLTAR,
            ENKU_CHIEF_KEPRA,
            TUREK_CHIEF_BURAI,
            LEUNT_CHIEF_HARAK,
            VUKU_CHIEF_DRIKO,
            GANDI_CHIEF_CHIANTA,
        ]);
        s.add_kill_id(&[
            TYRANT,
            TYRANT_KINGPIN,
            MARSH_STAKATO_DRONE,
            GUARDIAN_BASILISK,
            MANASHEN_GARGOYLE,
            TIMAK_ORC,
            TIMAK_ORC_ARCHER,
            TIMAK_ORC_SOLDIER,
            TIMAK_ORC_WARRIOR,
            TIMAK_ORC_SHAMAN,
            TIMAK_ORC_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            RAGNA_ORC_OVERLORD,
            RAGNA_ORC_SEER,
            PASHIKA_SON_OF_VOLTAR,
            VULTUS_SON_OF_VOLTAR,
            ENKU_ORC_OVERLORD,
            MAKUM_BUGBEAR_THUG,
            REVENANT_OF_TANTOS_CHIEF,
        ]);
        s.add_attack_id(&[RAGNA_ORC_OVERLORD, RAGNA_ORC_SEER, REVENANT_OF_TANTOS_CHIEF]);
        s.register_quest_items(&[
            VOKIANS_ORDER,
            MANASHEN_SHARD,
            TYRANT_TALON,
            GUARDIAN_BASILISK_FANG,
            VOKIANS_ORDER2,
            NECKLACE_OF_AUTHORITY,
            CHIANTA_1ST_ORDER,
            SCEPTER_OF_BREKA,
            SCEPTER_OF_ENKU,
            SCEPTER_OF_VUKU,
            SCEPTER_OF_TUREK,
            SCEPTER_OF_TUNATH,
            CHIANTA_2ND_ORDER,
            CHIANTA_3RD_ORDER,
            TAMLIN_ORC_SKULL,
            TIMAK_ORC_HEAD,
            SCEPTER_BOX,
            PASHIKAS_HEAD,
            VULTUS_HEAD,
            GLOVE_OF_VOLTAR,
            ENKU_OVERLORD_HEAD,
            GLOVE_OF_KEPRA,
            MAKUM_BUGBEAR_HEAD,
            GLOVE_OF_BURAI,
            MANAKIA_1ST_LETTER,
            MANAKIA_2ND_LETTER,
            KASMANS_1ST_LETTER,
            KASMANS_2ND_LETTER,
            KASMANS_3RD_LETTER,
            DRIKOS_CONTRACT,
            STAKATO_DRONE_HUSK,
            TANAPIS_ORDER,
            SCEPTER_OF_TANTOS,
            RITUAL_BOX,
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
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, VOKIANS_ORDER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 109);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30514-05a.htm".to_owned());
                    } else {
                        htmltext = Some("30514-05.htm".to_owned());
                    }
                }
            }
            "30514-04.htm" | "30514-07.html" | "30571-02.html" | "30615-03.html" | "30616-03.html"
            | "30642-02.html" | "30642-06.html" | "30642-08.html" => {
                htmltext = Some(event.to_owned());
            }
            "30501-02.html" => {
                if has(ctx, player, SCEPTER_OF_VUKU) {
                    htmltext = Some(event.to_owned());
                } else if !any(ctx, player, &[SCEPTER_OF_VUKU, KASMANS_1ST_LETTER]) {
                    ctx.give_items(player, KASMANS_1ST_LETTER, 1);
                    ctx.add_radar(player, -2150, 124443, -3724);
                    htmltext = Some("30501-03.html".to_owned());
                } else if !has(ctx, player, SCEPTER_OF_VUKU)
                    && any(ctx, player, &[KASMANS_1ST_LETTER, DRIKOS_CONTRACT])
                {
                    ctx.add_radar(player, -2150, 124443, -3724);
                    htmltext = Some("30501-04.html".to_owned());
                }
            }
            "30501-05.html" => {
                if has(ctx, player, SCEPTER_OF_TUREK) {
                    htmltext = Some(event.to_owned());
                } else if !any(ctx, player, &[SCEPTER_OF_TUREK, KASMANS_2ND_LETTER]) {
                    ctx.give_items(player, KASMANS_2ND_LETTER, 1);
                    ctx.add_radar(player, -94294, 110818, -3563);
                    htmltext = Some("30501-06.html".to_owned());
                } else if !has(ctx, player, SCEPTER_OF_TUREK) && has(ctx, player, KASMANS_2ND_LETTER) {
                    ctx.add_radar(player, -94294, 110818, -3563);
                    htmltext = Some("30501-07.html".to_owned());
                }
            }
            "30501-08.html" => {
                if has(ctx, player, SCEPTER_OF_TUNATH) {
                    htmltext = Some(event.to_owned());
                } else if !any(ctx, player, &[SCEPTER_OF_TUNATH, KASMANS_3RD_LETTER]) {
                    ctx.give_items(player, KASMANS_3RD_LETTER, 1);
                    ctx.add_radar(player, -55217, 200628, -3724);
                    htmltext = Some("30501-09.html".to_owned());
                } else if !has(ctx, player, SCEPTER_OF_TUNATH) && has(ctx, player, KASMANS_3RD_LETTER) {
                    ctx.add_radar(player, -55217, 200628, -3724);
                    htmltext = Some("30501-10.html".to_owned());
                }
            }
            "30515-04.html" => {
                if !has(ctx, player, SCEPTER_OF_BREKA) && has(ctx, player, MANAKIA_1ST_LETTER) {
                    ctx.add_radar(player, 80100, 119991, -2264);
                    htmltext = Some(event.to_owned());
                } else if has(ctx, player, SCEPTER_OF_BREKA) {
                    htmltext = Some("30515-02.html".to_owned());
                } else if !any(ctx, player, &[SCEPTER_OF_BREKA, MANAKIA_1ST_LETTER]) {
                    ctx.give_items(player, MANAKIA_1ST_LETTER, 1);
                    ctx.add_radar(player, 80100, 119991, -2264);
                    htmltext = Some("30515-03.html".to_owned());
                }
            }
            "30515-05.html" => {
                if has(ctx, player, SCEPTER_OF_ENKU) {
                    htmltext = Some(event.to_owned());
                } else if !any(ctx, player, &[SCEPTER_OF_ENKU, MANAKIA_2ND_LETTER]) {
                    ctx.give_items(player, MANAKIA_2ND_LETTER, 1);
                    ctx.add_radar(player, 12805, 189249, -3616);
                    htmltext = Some("30515-06.html".to_owned());
                } else if !has(ctx, player, SCEPTER_OF_ENKU) && has(ctx, player, MANAKIA_2ND_LETTER) {
                    ctx.add_radar(player, 12805, 189249, -3616);
                    htmltext = Some("30515-07.html".to_owned());
                }
            }
            "30571-03.html" => {
                if has(ctx, player, SCEPTER_BOX) {
                    ctx.take_items(player, SCEPTER_BOX, 1);
                    ctx.give_items(player, TANAPIS_ORDER, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30615-04.html" => {
                if has(ctx, player, MANAKIA_1ST_LETTER) {
                    ctx.give_items(player, GLOVE_OF_VOLTAR, 1);
                    ctx.take_items(player, MANAKIA_1ST_LETTER, 1);
                    let chief = npc?;
                    call_guard(ctx, chief, PASHIKA_SON_OF_VOLTAR, player);
                    call_guard(ctx, chief, VULTUS_SON_OF_VOLTAR, player);
                    htmltext = Some(event.to_owned());
                }
            }
            "30616-04.html" => {
                if has(ctx, player, MANAKIA_2ND_LETTER) {
                    ctx.give_items(player, GLOVE_OF_KEPRA, 1);
                    ctx.take_items(player, MANAKIA_2ND_LETTER, 1);
                    let chief = npc?;
                    for _ in 0..4 {
                        call_guard(ctx, chief, ENKU_ORC_OVERLORD, player);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30617-03.html" => {
                if has(ctx, player, KASMANS_2ND_LETTER) {
                    ctx.give_items(player, GLOVE_OF_BURAI, 1);
                    ctx.take_items(player, KASMANS_2ND_LETTER, 1);
                    let chief = npc?;
                    call_guard(ctx, chief, MAKUM_BUGBEAR_THUG, player);
                    call_guard(ctx, chief, MAKUM_BUGBEAR_THUG, player);
                    htmltext = Some(event.to_owned());
                }
            }
            "30618-03.html" => {
                if has(ctx, player, KASMANS_3RD_LETTER) {
                    ctx.give_items(player, SCEPTER_OF_TUNATH, 1);
                    ctx.take_items(player, KASMANS_3RD_LETTER, 1);
                    if all(
                        ctx,
                        player,
                        &[
                            SCEPTER_OF_TUREK,
                            SCEPTER_OF_ENKU,
                            SCEPTER_OF_BREKA,
                            SCEPTER_OF_VUKU,
                        ],
                    ) {
                        qs.set_cond(ctx, 5, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30619-03.html" => {
                if has(ctx, player, KASMANS_1ST_LETTER) {
                    ctx.give_items(player, DRIKOS_CONTRACT, 1);
                    ctx.take_items(player, KASMANS_1ST_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30642-03.html" => {
                if has(ctx, player, VOKIANS_ORDER2) {
                    ctx.take_items(player, VOKIANS_ORDER2, 1);
                    ctx.give_items(player, CHIANTA_1ST_ORDER, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30642-07.html" => {
                if all(
                    ctx,
                    player,
                    &[
                        CHIANTA_1ST_ORDER,
                        SCEPTER_OF_BREKA,
                        SCEPTER_OF_VUKU,
                        SCEPTER_OF_TUREK,
                        SCEPTER_OF_TUNATH,
                        SCEPTER_OF_ENKU,
                    ],
                ) {
                    ctx.take_items(player, CHIANTA_1ST_ORDER, 1);
                    ctx.take_items(player, SCEPTER_OF_BREKA, 1);
                    ctx.take_items(player, SCEPTER_OF_ENKU, 1);
                    ctx.take_items(player, SCEPTER_OF_VUKU, 1);
                    ctx.take_items(player, SCEPTER_OF_TUREK, 1);
                    ctx.take_items(player, SCEPTER_OF_TUNATH, 1);
                    ctx.take_items(player, MANAKIA_1ST_LETTER, 1);
                    ctx.take_items(player, MANAKIA_2ND_LETTER, 1);
                    ctx.take_items(player, KASMANS_1ST_LETTER, 1);
                    ctx.give_items(player, CHIANTA_3RD_ORDER, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
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
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match npc.id {
            RAGNA_ORC_OVERLORD | RAGNA_ORC_SEER => match ctx.world().npc_script_value(npc.object_id) {
                0 => {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("lastAttacker", attacker.0);
                    if !has(ctx, attacker, SCEPTER_OF_TANTOS) {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IsItALackeyOfKakai,
                            &[],
                        );
                        ctx.set_script_value(npc, 1);
                    }
                }
                1 => ctx.set_script_value(npc, 2),
                _ => {}
            },
            REVENANT_OF_TANTOS_CHIEF => match ctx.world().npc_script_value(npc.object_id) {
                0 => {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("lastAttacker", attacker.0);
                    if !has(ctx, attacker, SCEPTER_OF_TANTOS) {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::HowRegretfulUnjustDishonor,
                            &[],
                        );
                        ctx.set_script_value(npc, 1);
                    }
                }
                1 => {
                    if !has(ctx, attacker, SCEPTER_OF_TANTOS)
                        && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc) / 3)
                    {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IndignantAndUnfairDeath,
                            &[],
                        );
                        ctx.set_script_value(npc, 2);
                    }
                }
                _ => {}
            },
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
        match npc.id {
            TYRANT | TYRANT_KINGPIN => vokian_item(
                ctx,
                qs,
                killer,
                TYRANT_TALON,
                (MANASHEN_SHARD, GUARDIAN_BASILISK_FANG),
            ),
            GUARDIAN_BASILISK => {
                vokian_item(
                    ctx,
                    qs,
                    killer,
                    GUARDIAN_BASILISK_FANG,
                    (MANASHEN_SHARD, TYRANT_TALON),
                );
            }
            MANASHEN_GARGOYLE => {
                vokian_item(
                    ctx,
                    qs,
                    killer,
                    MANASHEN_SHARD,
                    (TYRANT_TALON, GUARDIAN_BASILISK_FANG),
                );
            }
            MARSH_STAKATO_DRONE => {
                if !has(ctx, killer, SCEPTER_OF_VUKU)
                    && all(
                        ctx,
                        killer,
                        &[NECKLACE_OF_AUTHORITY, CHIANTA_1ST_ORDER, DRIKOS_CONTRACT],
                    )
                    && ctx.get_quest_items_count(killer, STAKATO_DRONE_HUSK) < 30
                {
                    // NOTE(java-quirk): Java compares the count of TYRANT_TALON (not of the husk)
                    // with 29 to pick the "middle" sound.
                    if ctx.get_quest_items_count(killer, TYRANT_TALON) == 29 {
                        ctx.give_items(killer, STAKATO_DRONE_HUSK, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.give_items(killer, STAKATO_DRONE_HUSK, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TIMAK_ORC | TIMAK_ORC_ARCHER | TIMAK_ORC_SOLDIER | TIMAK_ORC_WARRIOR | TIMAK_ORC_SHAMAN
            | TIMAK_ORC_OVERLORD => {
                if all(ctx, killer, &[NECKLACE_OF_AUTHORITY, CHIANTA_3RD_ORDER])
                    && ctx.get_quest_items_count(killer, TIMAK_ORC_HEAD) < 20
                {
                    // NOTE(java-quirk): Java compares the count of MANASHEN_SHARD (not of the
                    // heads) with 19, so the 20th head is not the one that sets cond 7.
                    if ctx.get_quest_items_count(killer, MANASHEN_SHARD) == 19 {
                        ctx.give_items(killer, TIMAK_ORC_HEAD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.get_quest_items_count(killer, TAMLIN_ORC_SKULL) >= 20 {
                            qs.set_cond(ctx, 7, false);
                        }
                    } else {
                        ctx.give_items(killer, TIMAK_ORC_HEAD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                if all(ctx, killer, &[NECKLACE_OF_AUTHORITY, CHIANTA_3RD_ORDER])
                    && ctx.get_quest_items_count(killer, TAMLIN_ORC_SKULL) < 20
                {
                    if ctx.get_quest_items_count(killer, TAMLIN_ORC_SKULL) == 19 {
                        ctx.give_items(killer, TAMLIN_ORC_SKULL, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.get_quest_items_count(killer, TIMAK_ORC_HEAD) >= 20 {
                            qs.set_cond(ctx, 7, false);
                        }
                    } else {
                        ctx.give_items(killer, TAMLIN_ORC_SKULL, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            RAGNA_ORC_OVERLORD | RAGNA_ORC_SEER => {
                if has(ctx, killer, TANAPIS_ORDER) && !has(ctx, killer, SCEPTER_OF_TANTOS) {
                    ctx.add_spawn_at(REVENANT_OF_TANTOS_CHIEF, npc.object_id, true, 200_000);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::TooLate, &[]);
                }
            }
            PASHIKA_SON_OF_VOLTAR => voltar_head(ctx, killer, PASHIKAS_HEAD, VULTUS_HEAD),
            VULTUS_SON_OF_VOLTAR => voltar_head(ctx, killer, VULTUS_HEAD, PASHIKAS_HEAD),
            ENKU_ORC_OVERLORD => {
                if all(
                    ctx,
                    killer,
                    &[NECKLACE_OF_AUTHORITY, CHIANTA_1ST_ORDER, GLOVE_OF_KEPRA],
                ) && ctx.get_quest_items_count(killer, ENKU_OVERLORD_HEAD) < 4
                {
                    if ctx.get_quest_items_count(killer, ENKU_OVERLORD_HEAD) == 3 {
                        ctx.give_items(killer, ENKU_OVERLORD_HEAD, 1);
                        ctx.take_items(killer, GLOVE_OF_KEPRA, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.give_items(killer, ENKU_OVERLORD_HEAD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MAKUM_BUGBEAR_THUG => {
                if all(
                    ctx,
                    killer,
                    &[NECKLACE_OF_AUTHORITY, CHIANTA_1ST_ORDER, GLOVE_OF_BURAI],
                ) && ctx.get_quest_items_count(killer, MAKUM_BUGBEAR_HEAD) < 2
                {
                    if ctx.get_quest_items_count(killer, MAKUM_BUGBEAR_HEAD) == 1 {
                        ctx.give_items(killer, MAKUM_BUGBEAR_HEAD, 1);
                        ctx.take_items(killer, GLOVE_OF_BURAI, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.give_items(killer, MAKUM_BUGBEAR_HEAD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            REVENANT_OF_TANTOS_CHIEF => {
                if has(ctx, killer, TANAPIS_ORDER) && !has(ctx, killer, SCEPTER_OF_TANTOS) {
                    ctx.give_items(killer, SCEPTER_OF_TANTOS, 1);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::ILlGetRevengeSomeday, &[]);
                    qs.set_cond(ctx, 10, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == PREFECT_VOKIAN {
                if ctx.player_race(player) == race::ORC {
                    let level_ok = ctx.player_level(player) >= MIN_LEVEL;
                    if level_ok && ctx.world().is_in_category(player, CategoryType::Orc2ndGroup) {
                        htmltext = "30514-03.htm".to_owned();
                    } else if level_ok {
                        htmltext = "30514-01a.html".to_owned();
                    } else {
                        htmltext = "30514-02.html".to_owned();
                    }
                } else {
                    htmltext = "30514-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            if let Some(page) = started_page(ctx, qs, npc, player) {
                htmltext = page.to_owned();
            }
        } else if state == COMPLETED && npc.id == PREFECT_VOKIAN {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// A monster of Vokian's order: 10 of the item (middle sound at the 10th, quiet cond 2 when the
/// other two are complete as well).
fn vokian_item(ctx: &mut ScriptCtx<'_>, qs: Qs, killer: ObjectId, item: i32, others: (i32, i32)) {
    if has(ctx, killer, VOKIANS_ORDER) && ctx.get_quest_items_count(killer, item) < 10 {
        if ctx.get_quest_items_count(killer, item) == 9 {
            ctx.give_items(killer, item, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            if ctx.get_quest_items_count(killer, others.0) >= 10
                && ctx.get_quest_items_count(killer, others.1) >= 10
            {
                qs.set_cond(ctx, 2, false);
            }
        } else {
            ctx.give_items(killer, item, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// Pashika and Vultus: the head of the one killed; the glove goes with the second head.
fn voltar_head(ctx: &mut ScriptCtx<'_>, killer: ObjectId, head: i32, other_head: i32) {
    if all(
        ctx,
        killer,
        &[NECKLACE_OF_AUTHORITY, CHIANTA_1ST_ORDER, GLOVE_OF_VOLTAR],
    ) && !has(ctx, killer, head)
    {
        if has(ctx, killer, other_head) {
            ctx.give_items(killer, head, 1);
            ctx.take_items(killer, GLOVE_OF_VOLTAR, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
        } else {
            ctx.give_items(killer, head, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// The `STARTED` branch of Java's `onTalk`: the page (or none) of an NPC for the items held.
fn started_page(ctx: &mut ScriptCtx<'_>, qs: Qs, npc: ScriptNpc, p: ObjectId) -> Option<&'static str> {
    let necklace_first = all(ctx, p, &[NECKLACE_OF_AUTHORITY, CHIANTA_1ST_ORDER]);
    let late = any(ctx, p, &[CHIANTA_2ND_ORDER, CHIANTA_3RD_ORDER, SCEPTER_BOX]);
    match npc.id {
        PREFECT_VOKIAN => {
            if has(ctx, p, VOKIANS_ORDER) {
                if ctx.get_quest_items_count(p, MANASHEN_SHARD) >= 10
                    && ctx.get_quest_items_count(p, TYRANT_TALON) >= 10
                    && ctx.get_quest_items_count(p, GUARDIAN_BASILISK_FANG) >= 10
                {
                    ctx.take_items(p, VOKIANS_ORDER, 1);
                    ctx.take_items(p, MANASHEN_SHARD, -1);
                    ctx.take_items(p, TYRANT_TALON, -1);
                    ctx.take_items(p, GUARDIAN_BASILISK_FANG, -1);
                    ctx.give_items(p, VOKIANS_ORDER2, 1);
                    ctx.give_items(p, NECKLACE_OF_AUTHORITY, 1);
                    qs.set_cond(ctx, 3, true);
                    return Some("30514-08.html");
                }
                return Some("30514-06.html");
            } else if all(ctx, p, &[VOKIANS_ORDER2, NECKLACE_OF_AUTHORITY]) {
                return Some("30514-09.html");
            } else if !has(ctx, p, NECKLACE_OF_AUTHORITY) && any(ctx, p, &[VOKIANS_ORDER2, SCEPTER_BOX]) {
                return Some("30514-10.html");
            }
        }
        PREFECT_KASMAN => {
            if necklace_first {
                return Some("30501-01.html");
            } else if late {
                return Some("30501-11.html");
            }
        }
        SEER_MANAKIA => {
            if necklace_first {
                return Some("30515-01.html");
            } else if late {
                return Some("30515-08.html");
            }
        }
        FLAME_LORD_KAKAI => {
            if !has(ctx, p, RITUAL_BOX) && any(ctx, p, &[SCEPTER_BOX, TANAPIS_ORDER]) {
                return Some("30565-01.html");
            } else if has(ctx, p, RITUAL_BOX) {
                ctx.give_adena(p, 262_720, true);
                ctx.give_items(p, MARK_OF_GLORY, 1);
                ctx.add_exp_and_sp(p, 1_448_226, 96_648);
                qs.exit_quest(ctx, false, true);
                ctx.send_social_action(p, 3);
                return Some("30565-02.html");
            }
        }
        SEER_TANAPI => {
            if has(ctx, p, SCEPTER_BOX) {
                return Some("30571-01.html");
            } else if has(ctx, p, TANAPIS_ORDER) {
                if !has(ctx, p, SCEPTER_OF_TANTOS) {
                    return Some("30571-04.html");
                }
                ctx.take_items(p, TANAPIS_ORDER, 1);
                ctx.take_items(p, SCEPTER_OF_TANTOS, 1);
                ctx.give_items(p, RITUAL_BOX, 1);
                qs.set_cond(ctx, 11, true);
                return Some("30571-05.html");
            } else if has(ctx, p, RITUAL_BOX) {
                return Some("30571-06.html");
            }
        }
        BREKA_CHIEF_VOLTAR => {
            if necklace_first {
                if !any(
                    ctx,
                    p,
                    &[
                        SCEPTER_OF_BREKA,
                        MANAKIA_1ST_LETTER,
                        GLOVE_OF_VOLTAR,
                        PASHIKAS_HEAD,
                        VULTUS_HEAD,
                    ],
                ) {
                    return Some("30615-01.html");
                } else if has(ctx, p, MANAKIA_1ST_LETTER) {
                    ctx.remove_radar(p, 80100, 119991, -2264);
                    return Some("30615-02.html");
                } else if !has(ctx, p, SCEPTER_OF_BREKA)
                    && has(ctx, p, GLOVE_OF_VOLTAR)
                    && ctx.get_quest_items_count(p, PASHIKAS_HEAD) + ctx.get_quest_items_count(p, VULTUS_HEAD)
                        < 2
                {
                    if ctx.summoned_npc_count(npc) < 2 {
                        call_guard(ctx, npc, PASHIKA_SON_OF_VOLTAR, p);
                        call_guard(ctx, npc, VULTUS_SON_OF_VOLTAR, p);
                    }
                    return Some("30615-05.html");
                } else if all(ctx, p, &[PASHIKAS_HEAD, VULTUS_HEAD]) {
                    ctx.give_items(p, SCEPTER_OF_BREKA, 1);
                    ctx.take_items(p, PASHIKAS_HEAD, 1);
                    ctx.take_items(p, VULTUS_HEAD, 1);
                    if all(
                        ctx,
                        p,
                        &[
                            SCEPTER_OF_ENKU,
                            SCEPTER_OF_VUKU,
                            SCEPTER_OF_TUREK,
                            SCEPTER_OF_TUNATH,
                        ],
                    ) {
                        qs.set_cond(ctx, 5, true);
                    }
                    return Some("30615-06.html");
                } else if has(ctx, p, SCEPTER_OF_BREKA) {
                    return Some("30615-07.html");
                }
            } else if late {
                return Some("30615-08.html");
            }
        }
        ENKU_CHIEF_KEPRA => {
            if necklace_first {
                if !any(ctx, p, &[SCEPTER_OF_ENKU, MANAKIA_2ND_LETTER, GLOVE_OF_KEPRA])
                    && ctx.get_quest_items_count(p, ENKU_OVERLORD_HEAD) < 4
                {
                    return Some("30616-01.html");
                } else if has(ctx, p, MANAKIA_2ND_LETTER) {
                    ctx.remove_radar(p, 12805, 189249, -3616);
                    return Some("30616-02.html");
                } else if has(ctx, p, GLOVE_OF_KEPRA) && ctx.get_quest_items_count(p, ENKU_OVERLORD_HEAD) < 4
                {
                    if ctx.summoned_npc_count(npc) < 5 {
                        call_guard(ctx, npc, ENKU_ORC_OVERLORD, p);
                    }
                    return Some("30616-05.html");
                } else if ctx.get_quest_items_count(p, ENKU_OVERLORD_HEAD) >= 4 {
                    ctx.give_items(p, SCEPTER_OF_ENKU, 1);
                    ctx.take_items(p, ENKU_OVERLORD_HEAD, -1);
                    if all(
                        ctx,
                        p,
                        &[
                            SCEPTER_OF_BREKA,
                            SCEPTER_OF_VUKU,
                            SCEPTER_OF_TUREK,
                            SCEPTER_OF_TUNATH,
                        ],
                    ) {
                        qs.set_cond(ctx, 5, true);
                    }
                    return Some("30616-06.html");
                } else if has(ctx, p, SCEPTER_OF_ENKU) {
                    return Some("30616-07.html");
                }
            } else if late {
                return Some("30616-08.html");
            }
        }
        TUREK_CHIEF_BURAI => {
            if necklace_first {
                if !any(
                    ctx,
                    p,
                    &[
                        SCEPTER_OF_TUREK,
                        KASMANS_2ND_LETTER,
                        GLOVE_OF_BURAI,
                        MAKUM_BUGBEAR_HEAD,
                    ],
                ) {
                    return Some("30617-01.html");
                } else if has(ctx, p, KASMANS_2ND_LETTER) {
                    ctx.remove_radar(p, -94294, 110818, -3563);
                    return Some("30617-02.html");
                } else if has(ctx, p, GLOVE_OF_BURAI) {
                    if ctx.summoned_npc_count(npc) < 3 {
                        call_guard(ctx, npc, MAKUM_BUGBEAR_THUG, p);
                        call_guard(ctx, npc, MAKUM_BUGBEAR_THUG, p);
                    }
                    return Some("30617-04.html");
                } else if ctx.get_quest_items_count(p, MAKUM_BUGBEAR_HEAD) >= 2 {
                    ctx.give_items(p, SCEPTER_OF_TUREK, 1);
                    ctx.take_items(p, MAKUM_BUGBEAR_HEAD, -1);
                    if all(
                        ctx,
                        p,
                        &[
                            SCEPTER_OF_ENKU,
                            SCEPTER_OF_BREKA,
                            SCEPTER_OF_VUKU,
                            SCEPTER_OF_TUNATH,
                        ],
                    ) {
                        qs.set_cond(ctx, 5, true);
                    }
                    return Some("30617-05.html");
                } else if has(ctx, p, SCEPTER_OF_TUREK) {
                    return Some("30617-06.html");
                }
            } else if has(ctx, p, NECKLACE_OF_AUTHORITY) && late {
                return Some("30617-07.html");
            }
        }
        LEUNT_CHIEF_HARAK => {
            if necklace_first {
                if !any(ctx, p, &[SCEPTER_OF_TUNATH, KASMANS_3RD_LETTER]) {
                    return Some("30618-01.html");
                } else if !has(ctx, p, SCEPTER_OF_TUNATH) && has(ctx, p, KASMANS_3RD_LETTER) {
                    ctx.remove_radar(p, -55217, 200628, -3724);
                    return Some("30618-02.html");
                } else if has(ctx, p, SCEPTER_OF_TUNATH) {
                    return Some("30618-04.html");
                }
            } else if late {
                return Some("30618-05.html");
            }
        }
        VUKU_CHIEF_DRIKO => {
            if necklace_first {
                if !any(ctx, p, &[SCEPTER_OF_VUKU, KASMANS_1ST_LETTER, DRIKOS_CONTRACT]) {
                    return Some("30619-01.html");
                } else if !has(ctx, p, SCEPTER_OF_VUKU) && has(ctx, p, KASMANS_1ST_LETTER) {
                    ctx.remove_radar(p, -2150, 124443, -3724);
                    return Some("30619-02.html");
                } else if !has(ctx, p, SCEPTER_OF_VUKU) && has(ctx, p, DRIKOS_CONTRACT) {
                    if ctx.get_quest_items_count(p, STAKATO_DRONE_HUSK) < 30 {
                        return Some("30619-04.html");
                    }
                    ctx.give_items(p, SCEPTER_OF_VUKU, 1);
                    ctx.take_items(p, DRIKOS_CONTRACT, 1);
                    ctx.take_items(p, STAKATO_DRONE_HUSK, -1);
                    if all(
                        ctx,
                        p,
                        &[
                            SCEPTER_OF_TUREK,
                            SCEPTER_OF_ENKU,
                            SCEPTER_OF_BREKA,
                            SCEPTER_OF_TUNATH,
                        ],
                    ) {
                        qs.set_cond(ctx, 5, true);
                    }
                    return Some("30619-05.html");
                } else if has(ctx, p, SCEPTER_OF_VUKU) {
                    return Some("30619-06.html");
                }
            } else if has(ctx, p, NECKLACE_OF_AUTHORITY) && late {
                return Some("30619-07.html");
            }
        }
        GANDI_CHIEF_CHIANTA => {
            if all(ctx, p, &[NECKLACE_OF_AUTHORITY, VOKIANS_ORDER2]) {
                return Some("30642-01.html");
            } else if necklace_first {
                let scepters = ctx.get_quest_items_count(p, SCEPTER_OF_BREKA)
                    + ctx.get_quest_items_count(p, SCEPTER_OF_VUKU)
                    + ctx.get_quest_items_count(p, SCEPTER_OF_TUREK)
                    + ctx.get_quest_items_count(p, SCEPTER_OF_TUNATH)
                    + ctx.get_quest_items_count(p, SCEPTER_OF_ENKU);
                if scepters < 5 {
                    return Some("30642-04.html");
                } else if all(
                    ctx,
                    p,
                    &[
                        SCEPTER_OF_BREKA,
                        SCEPTER_OF_VUKU,
                        SCEPTER_OF_TUREK,
                        SCEPTER_OF_TUNATH,
                        SCEPTER_OF_ENKU,
                    ],
                ) {
                    return Some("30642-05.html");
                }
            } else if all(ctx, p, &[NECKLACE_OF_AUTHORITY, CHIANTA_2ND_ORDER]) {
                ctx.give_items(p, CHIANTA_3RD_ORDER, 1);
                ctx.take_items(p, CHIANTA_2ND_ORDER, 1);
                return Some("30642-09.html");
            } else if all(ctx, p, &[NECKLACE_OF_AUTHORITY, CHIANTA_3RD_ORDER]) {
                if ctx.get_quest_items_count(p, TAMLIN_ORC_SKULL) >= 20
                    && ctx.get_quest_items_count(p, TIMAK_ORC_HEAD) >= 20
                {
                    ctx.take_items(p, NECKLACE_OF_AUTHORITY, 1);
                    ctx.take_items(p, CHIANTA_3RD_ORDER, 1);
                    ctx.take_items(p, TAMLIN_ORC_SKULL, -1);
                    ctx.take_items(p, TIMAK_ORC_HEAD, -1);
                    ctx.give_items(p, SCEPTER_BOX, 1);
                    qs.set_cond(ctx, 8, true);
                    return Some("30642-11.html");
                }
                return Some("30642-10.html");
            } else if has(ctx, p, SCEPTER_BOX) {
                return Some("30642-12.html");
            } else if any(ctx, p, &[TANAPIS_ORDER, RITUAL_BOX]) {
                return Some("30642-13.html");
            }
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00220_TestimonyOfGlory";

    const NPCS: &str = r#"
        <npc id="30514" type="Folk" level="70" name="Prefect Vokian"/>
        <npc id="30501" type="Folk" level="70" name="Prefect Kasman"/>
        <npc id="30565" type="Folk" level="70" name="Flame Lord Kakai"/>
        <npc id="30615" type="Folk" level="70" name="Breka Chief Voltar"/>
        <npc id="27080" type="Monster" level="40" name="Pashika"/>
        <npc id="27081" type="Monster" level="40" name="Vultus"/>
        <npc id="20563" type="Monster" level="40" name="Manashen Gargoyle"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3203" type="EtcItem" name="Mark of Glory"><set name="is_questitem" val="true"/></item>
        <item id="3204" type="EtcItem" name="Vokian's Order"><set name="is_questitem" val="true"/></item>
        <item id="3205" type="EtcItem" name="Manashen Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3206" type="EtcItem" name="Tyrant Talon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3207" type="EtcItem" name="Guardian Basilisk Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3208" type="EtcItem" name="Vokian's Order 2"><set name="is_questitem" val="true"/></item>
        <item id="3209" type="EtcItem" name="Necklace of Authority"><set name="is_questitem" val="true"/></item>
        <item id="3210" type="EtcItem" name="Chianta 1st Order"><set name="is_questitem" val="true"/></item>
        <item id="3221" type="EtcItem" name="Pashika's Head"><set name="is_questitem" val="true"/></item>
        <item id="3222" type="EtcItem" name="Vultus Head"><set name="is_questitem" val="true"/></item>
        <item id="3223" type="EtcItem" name="Glove of Voltar"><set name="is_questitem" val="true"/></item>
        <item id="3228" type="EtcItem" name="Manakia 1st Letter"><set name="is_questitem" val="true"/></item>
        <item id="3237" type="EtcItem" name="Ritual Box"><set name="is_questitem" val="true"/></item>
        <item id="3211" type="EtcItem" name="Scepter of Breka"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00220TestimonyOfGlory)
            .plain(&[
                "30514-01.html",
                "30514-01a.html",
                "30514-02.html",
                "30514-05.htm",
                "30514-05a.htm",
                "30514-06.html",
                "30514-08.html",
                "30565-02.html",
            ])
            .page("30514-03.htm", &["30514-04.htm"])
            .page("30514-04.htm", &["ACCEPT"])
            .page("30615-02.html", &["30615-03.html"])
            .page("30615-03.html", &["30615-04.html"])
            .plain(&["30615-04.html", "30615-05.html", "30615-06.html"])
            .npcs(&[
                PREFECT_VOKIAN,
                PREFECT_KASMAN,
                FLAME_LORD_KAKAI,
                BREKA_CHIEF_VOLTAR,
            ])
    }

    struct Npcs {
        vokian: Oid,
        kakai: Oid,
        voltar: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00220TestimonyOfGlory);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            vokian: t.spawn(PREFECT_VOKIAN, 20),
            kakai: t.spawn(FLAME_LORD_KAKAI, 30),
            voltar: t.spawn(BREKA_CHIEF_VOLTAR, 40),
        };
        (t, npcs)
    }

    /// Started through the script's own `ACCEPT` (the start page needs the ORC_2ND_GROUP
    /// category, which the test world has not).
    fn start_by_hand(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), true);
            let out = Q00220TestimonyOfGlory.on_event(ctx, "ACCEPT", None, Some(Oid(1)));
            assert_eq!(out.as_deref(), Some("30514-05a.htm"));
        });
    }

    #[test]
    fn the_start_is_gated_by_race_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.vokian).contains("30514-01.html"));
        t.enter_as(2, 36, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.vokian).contains("30514-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accept_gives_the_order_and_109_diamonds() {
        let (mut t, _npcs) = setup();
        t.enter_as(1, 40, race::ORC, player_class::ORC_FIGHTER, vec![]);
        start_by_hand(&mut t);
        assert_eq!(t.count(1, VOKIANS_ORDER), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 109);
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
    }

    #[test]
    fn the_gargoyle_shards_stop_at_ten_and_vokian_hands_over_the_necklace() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::ORC, player_class::ORC_FIGHTER, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), TYRANT_TALON, 10);
            ctx.give_items(Oid(1), GUARDIAN_BASILISK_FANG, 10);
        });
        t.take_writes();
        for _ in 0..11 {
            let gargoyle = t.spawn(MANASHEN_GARGOYLE, 60);
            t.kill(1, gargoyle);
            t.world.delete_npc(gargoyle);
        }
        assert_eq!(t.count(1, MANASHEN_SHARD), 10);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));

        assert!(t.quest_page(1, npcs.vokian).contains("30514-08.html"));
        assert_eq!(t.count(1, NECKLACE_OF_AUTHORITY), 1);
        assert_eq!(t.count(1, VOKIANS_ORDER2), 1);
    }

    #[test]
    fn voltar_calls_his_sons_and_the_second_head_gives_the_glove_back() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::ORC, player_class::ORC_FIGHTER, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), NECKLACE_OF_AUTHORITY, 1);
            ctx.give_items(Oid(1), CHIANTA_1ST_ORDER, 1);
            ctx.give_items(Oid(1), MANAKIA_1ST_LETTER, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.voltar).contains("30615-02.html"));
        assert!(t.event(1, NAME, "30615-03.html").contains("30615-03.html"));
        assert!(t.event(1, NAME, "30615-04.html").contains("30615-04.html"));
        assert_eq!(t.count(1, GLOVE_OF_VOLTAR), 1);
        let sons = [t.npcs_of(PASHIKA_SON_OF_VOLTAR), t.npcs_of(VULTUS_SON_OF_VOLTAR)];
        assert_eq!(sons[0].len(), 1);
        assert_eq!(sons[1].len(), 1);
        t.kill(1, sons[0][0]);
        t.kill(1, sons[1][0]);
        assert_eq!(t.count(1, PASHIKAS_HEAD), 1);
        assert_eq!(t.count(1, VULTUS_HEAD), 1);
        assert_eq!(t.count(1, GLOVE_OF_VOLTAR), 0);
    }

    #[test]
    fn kakai_pays_the_reward_for_the_ritual_box() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::ORC, player_class::ORC_FIGHTER, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RITUAL_BOX, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.kakai).contains("30565-02.html"));
        assert_eq!(t.count(1, MARK_OF_GLORY), 1);
        assert_eq!(t.count(1, 57), 262_720);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.vokian)
                .contains("This quest has already been completed.")
        );
    }
}
