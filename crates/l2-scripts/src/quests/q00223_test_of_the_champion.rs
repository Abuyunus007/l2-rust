//! Quest 223: Test of the Champion (Veteran Ascalon in Gludio: the Warrior and Orc Raider second
//! class change, through Trader Groot, Captain Mouen and Mason).
// Java: dist/game/data/scripts/quests/Q00223_TestOfTheChampion/Q00223_TestOfTheChampion.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222), with monsters that call
// their own helpers when they are first hit (`onAttack` on the NPC's script value and the
// `lastAttacker` variable of the NPC).

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const TRADER_GROOT: i32 = 30093;
const CAPTAIN_MOUEN: i32 = 30196;
const VETERAN_ASCALON: i32 = 30624;
const MASON: i32 = 30625;

// Items
const ASCALONS_1ST_LETTER: i32 = 3277;
const MASONS_LETTER: i32 = 3278;
const IRON_ROSE_RING: i32 = 3279;
const ASCALONS_2ND_LETTER: i32 = 3280;
const WHITE_ROSE_INSIGNIA: i32 = 3281;
const GROOTS_LETTER: i32 = 3282;
const ASCALONS_3RD_LETTER: i32 = 3283;
const MOUENS_1ST_ORDER: i32 = 3284;
const MOUENS_2ND_ORDER: i32 = 3285;
const MOUENS_LETTER: i32 = 3286;
const HARPYS_EGG: i32 = 3287;
const MEDUSA_VENOM: i32 = 3288;
const WINDSUS_BILE: i32 = 3289;
const BLOODY_AXE_HEAD: i32 = 3290;
const ROAD_RATMAN_HEAD: i32 = 3291;
const LETO_LIZARDMAN_FANG: i32 = 3292;

// Reward
const MARK_OF_CHAMPION: i32 = 3276;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const HARPY: i32 = 20145;
const MEDUSA: i32 = 20158;
const ROAD_SCAVENGER: i32 = 20551;
const WINDSUS: i32 = 20553;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OCERLORD: i32 = 20582;
const BLOODY_AXE_ELITE: i32 = 20780;

// Quest monsters
const HARPY_MATRIARCH: i32 = 27088;
const ROAD_COLLECTOR: i32 = 27089;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00223_TestOfTheChampion`.
pub struct Q00223TestOfTheChampion;

impl Script for Q00223TestOfTheChampion {
    fn name(&self) -> &'static str {
        "Q00223_TestOfTheChampion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00223_TestOfTheChampion"
    }

    fn id(&self) -> i32 {
        223
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VETERAN_ASCALON]);
        s.add_talk_id(&[VETERAN_ASCALON, TRADER_GROOT, CAPTAIN_MOUEN, MASON]);
        s.add_kill_id(&[
            HARPY,
            MEDUSA,
            WINDSUS,
            ROAD_SCAVENGER,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OCERLORD,
            BLOODY_AXE_ELITE,
            HARPY_MATRIARCH,
            ROAD_COLLECTOR,
        ]);
        s.add_attack_id(&[HARPY, ROAD_SCAVENGER, BLOODY_AXE_ELITE]);
        s.register_quest_items(&[
            ASCALONS_1ST_LETTER,
            MASONS_LETTER,
            IRON_ROSE_RING,
            ASCALONS_2ND_LETTER,
            WHITE_ROSE_INSIGNIA,
            GROOTS_LETTER,
            ASCALONS_3RD_LETTER,
            MOUENS_1ST_ORDER,
            MOUENS_2ND_ORDER,
            MOUENS_LETTER,
            HARPYS_EGG,
            MEDUSA_VENOM,
            WINDSUS_BILE,
            BLOODY_AXE_HEAD,
            ROAD_RATMAN_HEAD,
            LETO_LIZARDMAN_FANG,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
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
                    ctx.give_items(player, ASCALONS_1ST_LETTER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        if ctx.player_class_id(player) == player_class::WARRIOR {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 72);
                        } else {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 64);
                        }
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30624-06a.htm".to_owned());
                    } else {
                        htmltext = Some("30624-06.htm".to_owned());
                    }
                }
            }
            "30624-05.htm" | "30196-02.html" | "30625-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30624-10.html" => {
                if ctx.has_quest_items(player, MASONS_LETTER) {
                    ctx.take_items(player, MASONS_LETTER, 1);
                    ctx.give_items(player, ASCALONS_2ND_LETTER, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30624-14.html" => {
                if ctx.has_quest_items(player, GROOTS_LETTER) {
                    ctx.take_items(player, GROOTS_LETTER, 1);
                    ctx.give_items(player, ASCALONS_3RD_LETTER, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30093-02.html" => {
                if ctx.has_quest_items(player, ASCALONS_2ND_LETTER) {
                    ctx.take_items(player, ASCALONS_2ND_LETTER, 1);
                    ctx.give_items(player, WHITE_ROSE_INSIGNIA, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30196-03.html" => {
                if ctx.has_quest_items(player, ASCALONS_3RD_LETTER) {
                    ctx.take_items(player, ASCALONS_3RD_LETTER, 1);
                    ctx.give_items(player, MOUENS_1ST_ORDER, 1);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30196-06.html" => {
                if ctx.get_quest_items_count(player, ROAD_RATMAN_HEAD) >= 10 {
                    ctx.take_items(player, MOUENS_1ST_ORDER, 1);
                    ctx.give_items(player, MOUENS_2ND_ORDER, 1);
                    ctx.take_items(player, ROAD_RATMAN_HEAD, -1);
                    qs.set_cond(ctx, 12, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30625-03.html" => {
                if ctx.has_quest_items(player, ASCALONS_1ST_LETTER) {
                    ctx.take_items(player, ASCALONS_1ST_LETTER, 1);
                    ctx.give_items(player, IRON_ROSE_RING, 1);
                    qs.set_cond(ctx, 2, true);
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
        // Java `npc.getScriptValue()`: 0 at the first hit (the helper may come), 1 after it, 2 later.
        match npc.id {
            HARPY => match ctx.world().npc_script_value(npc.object_id) {
                0 => {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("lastAttacker", attacker.0);
                    if ctx.has_quest_items(attacker, WHITE_ROSE_INSIGNIA)
                        && ctx.get_quest_items_count(attacker, HARPYS_EGG) < 30
                        && ctx.get_random_boolean()
                    {
                        if ctx.get_random(10) < 7 {
                            spawn_attacking(ctx, HARPY_MATRIARCH, npc, attacker, 1);
                        } else {
                            spawn_attacking(ctx, HARPY_MATRIARCH, npc, attacker, 2);
                        }
                    }
                    ctx.world_mut().set_npc_script_value(npc.object_id, 1);
                }
                1 => ctx.world_mut().set_npc_script_value(npc.object_id, 2),
                _ => {}
            },
            ROAD_SCAVENGER => match ctx.world().npc_script_value(npc.object_id) {
                0 => {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("lastAttacker", attacker.0);
                    if ctx.has_quest_items(attacker, MOUENS_1ST_ORDER)
                        && ctx.get_quest_items_count(attacker, ROAD_RATMAN_HEAD) < 10
                        && ctx.get_random_boolean()
                    {
                        if ctx.get_random(10) < 7 {
                            spawn_attacking(ctx, ROAD_COLLECTOR, npc, attacker, 1);
                        } else {
                            spawn_attacking(ctx, ROAD_COLLECTOR, npc, attacker, 2);
                        }
                    }
                    ctx.world_mut().set_npc_script_value(npc.object_id, 1);
                }
                1 => ctx.world_mut().set_npc_script_value(npc.object_id, 2),
                _ => {}
            },
            BLOODY_AXE_ELITE => match ctx.world().npc_script_value(npc.object_id) {
                0 => {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("lastAttacker", attacker.0);
                    if ctx.has_quest_items(attacker, IRON_ROSE_RING)
                        && ctx.get_quest_items_count(attacker, BLOODY_AXE_HEAD) < 10
                        && ctx.get_random_boolean()
                    {
                        spawn_attacking(ctx, BLOODY_AXE_ELITE, npc, attacker, 1);
                    }
                    ctx.world_mut().set_npc_script_value(npc.object_id, 1);
                }
                1 => ctx.world_mut().set_npc_script_value(npc.object_id, 2),
                _ => {}
            },
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                HARPY | HARPY_MATRIARCH => {
                    if ctx.has_quest_items(killer, WHITE_ROSE_INSIGNIA)
                        && ctx.get_quest_items_count(killer, HARPYS_EGG) < 30
                    {
                        if ctx.get_quest_items_count(killer, HARPYS_EGG) >= 28 {
                            ctx.give_items(killer, HARPYS_EGG, 2);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, MEDUSA_VENOM) >= 30
                                && ctx.get_quest_items_count(killer, WINDSUS_BILE) >= 30
                            {
                                qs.set_cond(ctx, 7, false);
                            }
                        } else {
                            ctx.give_items(killer, HARPYS_EGG, 2);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                MEDUSA => {
                    if ctx.has_quest_items(killer, WHITE_ROSE_INSIGNIA)
                        && ctx.get_quest_items_count(killer, MEDUSA_VENOM) < 30
                    {
                        if ctx.get_quest_items_count(killer, MEDUSA_VENOM) >= 27 {
                            ctx.give_items(killer, MEDUSA_VENOM, 3);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, HARPYS_EGG) >= 30
                                && ctx.get_quest_items_count(killer, WINDSUS_BILE) >= 30
                            {
                                qs.set_cond(ctx, 7, false);
                            }
                        } else {
                            ctx.give_items(killer, MEDUSA_VENOM, 3);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                WINDSUS => {
                    if ctx.has_quest_items(killer, WHITE_ROSE_INSIGNIA)
                        && ctx.get_quest_items_count(killer, WINDSUS_BILE) < 30
                    {
                        if ctx.get_quest_items_count(killer, WINDSUS_BILE) >= 27 {
                            ctx.give_items(killer, WINDSUS_BILE, 3);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, HARPYS_EGG) >= 30
                                && ctx.get_quest_items_count(killer, MEDUSA_VENOM) >= 30
                            {
                                qs.set_cond(ctx, 7, false);
                            }
                        } else {
                            ctx.give_items(killer, WINDSUS_BILE, 3);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                ROAD_SCAVENGER | ROAD_COLLECTOR => {
                    if ctx.has_quest_items(killer, MOUENS_1ST_ORDER)
                        && ctx.get_quest_items_count(killer, ROAD_RATMAN_HEAD) < 10
                    {
                        if ctx.get_quest_items_count(killer, ROAD_RATMAN_HEAD) >= 9 {
                            ctx.give_items(killer, ROAD_RATMAN_HEAD, 1);
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.give_items(killer, ROAD_RATMAN_HEAD, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                LETO_LIZARDMAN
                | LETO_LIZARDMAN_ARCHER
                | LETO_LIZARDMAN_SOLDIER
                | LETO_LIZARDMAN_WARRIOR
                | LETO_LIZARDMAN_SHAMAN
                | LETO_LIZARDMAN_OCERLORD => {
                    if ctx.has_quest_items(killer, MOUENS_2ND_ORDER)
                        && ctx.get_quest_items_count(killer, LETO_LIZARDMAN_FANG) < 10
                    {
                        if ctx.get_quest_items_count(killer, LETO_LIZARDMAN_FANG) >= 9 {
                            ctx.give_items(killer, LETO_LIZARDMAN_FANG, 1);
                            qs.set_cond(ctx, 13, true);
                        } else {
                            ctx.give_items(killer, LETO_LIZARDMAN_FANG, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                BLOODY_AXE_ELITE => {
                    if ctx.has_quest_items(killer, IRON_ROSE_RING)
                        && ctx.get_quest_items_count(killer, BLOODY_AXE_HEAD) < 10
                    {
                        if ctx.get_quest_items_count(killer, BLOODY_AXE_HEAD) >= 9 {
                            ctx.give_items(killer, BLOODY_AXE_HEAD, 1);
                            qs.set_cond(ctx, 3, true);
                        } else {
                            ctx.give_items(killer, BLOODY_AXE_HEAD, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == VETERAN_ASCALON {
                let class = ctx.player_class_id(player);
                if class == player_class::WARRIOR || class == player_class::ORC_RAIDER {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if class == player_class::WARRIOR {
                            htmltext = "30624-03.htm".to_owned();
                        } else {
                            htmltext = "30624-04.html".to_owned();
                        }
                    } else {
                        htmltext = "30624-01.html".to_owned();
                    }
                } else {
                    htmltext = "30624-02.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                VETERAN_ASCALON => {
                    if ctx.has_quest_items(player, ASCALONS_1ST_LETTER) {
                        htmltext = "30624-07.html".to_owned();
                    } else if ctx.has_quest_items(player, IRON_ROSE_RING) {
                        htmltext = "30624-08.html".to_owned();
                    } else if ctx.has_quest_items(player, MASONS_LETTER) {
                        htmltext = "30624-09.html".to_owned();
                    } else if ctx.has_quest_items(player, ASCALONS_2ND_LETTER) {
                        htmltext = "30624-11.html".to_owned();
                    } else if ctx.has_quest_items(player, WHITE_ROSE_INSIGNIA) {
                        htmltext = "30624-12.html".to_owned();
                    } else if ctx.has_quest_items(player, GROOTS_LETTER) {
                        htmltext = "30624-13.html".to_owned();
                    } else if ctx.has_quest_items(player, ASCALONS_3RD_LETTER) {
                        htmltext = "30624-15.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[MOUENS_1ST_ORDER, MOUENS_2ND_ORDER]) {
                        htmltext = "30624-16.html".to_owned();
                    } else if ctx.has_quest_items(player, MOUENS_LETTER) {
                        ctx.give_adena(player, 229_764, true);
                        ctx.give_items(player, MARK_OF_CHAMPION, 1);
                        ctx.add_exp_and_sp(player, 1_270_742, 87_200);
                        qs.exit_quest(ctx, false, true);
                        ctx.world().send(
                            player,
                            &SocialAction {
                                object_id: player.0,
                                action_id: 3,
                            },
                        );
                        htmltext = "30624-17.html".to_owned();
                    }
                }
                TRADER_GROOT => {
                    if ctx.has_quest_items(player, ASCALONS_2ND_LETTER) {
                        htmltext = "30093-01.html".to_owned();
                    } else if ctx.has_quest_items(player, WHITE_ROSE_INSIGNIA) {
                        if ctx.get_quest_items_count(player, HARPYS_EGG) >= 30
                            && ctx.get_quest_items_count(player, MEDUSA_VENOM) >= 30
                            && ctx.get_quest_items_count(player, WINDSUS_BILE) >= 30
                        {
                            ctx.take_items(player, WHITE_ROSE_INSIGNIA, 1);
                            ctx.give_items(player, GROOTS_LETTER, 1);
                            ctx.take_items(player, HARPYS_EGG, -1);
                            ctx.take_items(player, MEDUSA_VENOM, -1);
                            ctx.take_items(player, WINDSUS_BILE, -1);
                            qs.set_cond(ctx, 8, true);
                            htmltext = "30093-04.html".to_owned();
                        } else {
                            htmltext = "30093-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, GROOTS_LETTER) {
                        htmltext = "30093-05.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ASCALONS_3RD_LETTER,
                            MOUENS_1ST_ORDER,
                            MOUENS_2ND_ORDER,
                            MOUENS_LETTER,
                        ],
                    ) {
                        htmltext = "30093-06.html".to_owned();
                    }
                }
                CAPTAIN_MOUEN => {
                    if ctx.has_quest_items(player, ASCALONS_3RD_LETTER) {
                        htmltext = "30196-01.html".to_owned();
                    } else if ctx.has_quest_items(player, MOUENS_1ST_ORDER) {
                        if ctx.get_quest_items_count(player, ROAD_RATMAN_HEAD) < 10 {
                            htmltext = "30196-04.html".to_owned();
                        } else {
                            htmltext = "30196-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, MOUENS_2ND_ORDER) {
                        if ctx.get_quest_items_count(player, LETO_LIZARDMAN_FANG) < 10 {
                            htmltext = "30196-07.html".to_owned();
                        } else {
                            ctx.take_items(player, MOUENS_2ND_ORDER, 1);
                            ctx.give_items(player, MOUENS_LETTER, 1);
                            ctx.take_items(player, LETO_LIZARDMAN_FANG, -1);
                            qs.set_cond(ctx, 14, true);
                            htmltext = "30196-08.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, MOUENS_LETTER) {
                        htmltext = "30196-09.html".to_owned();
                    }
                }
                MASON => {
                    if ctx.has_quest_items(player, ASCALONS_1ST_LETTER) {
                        htmltext = "30625-01.html".to_owned();
                    } else if ctx.has_quest_items(player, IRON_ROSE_RING) {
                        if ctx.get_quest_items_count(player, BLOODY_AXE_HEAD) < 10 {
                            htmltext = "30625-04.html".to_owned();
                        } else {
                            ctx.give_items(player, MASONS_LETTER, 1);
                            ctx.take_items(player, IRON_ROSE_RING, 1);
                            ctx.take_items(player, BLOODY_AXE_HEAD, -1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30625-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, MASONS_LETTER) {
                        htmltext = "30625-06.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ASCALONS_2ND_LETTER,
                            WHITE_ROSE_INSIGNIA,
                            GROOTS_LETTER,
                            ASCALONS_3RD_LETTER,
                            MOUENS_1ST_ORDER,
                            MOUENS_2ND_ORDER,
                            MOUENS_LETTER,
                        ],
                    ) {
                        htmltext = "30625-07.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == VETERAN_ASCALON {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// Java `addAttackDesire(addSpawn(id, npc, true, 0, false), attacker)`, `count` times: the helper
/// spawns at the attacked NPC and runs at the attacker.
fn spawn_attacking(ctx: &mut ScriptCtx<'_>, npc_id: i32, npc: ScriptNpc, attacker: ObjectId, count: i32) {
    for _ in 0..count {
        if let Some(helper) = ctx.add_spawn_at(npc_id, npc.object_id, true, 0) {
            ctx.add_attack_desire(helper, attacker, 999);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00223_TestOfTheChampion";

    const NPCS: &str = r#"
        <npc id="30093" type="Folk" level="70" name="Trader Groot"/>
        <npc id="30196" type="Folk" level="70" name="Captain Mouen"/>
        <npc id="30624" type="Folk" level="70" name="Veteran Ascalon"/>
        <npc id="30625" type="Folk" level="70" name="Mason"/>
        <npc id="20780" type="Monster" level="40" name="Bloody Axe Elite"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3276" type="EtcItem" name="Mark of Champion"><set name="is_questitem" val="true"/></item>
        <item id="3277" type="EtcItem" name="Ascalon's 1st Letter"><set name="is_questitem" val="true"/></item>
        <item id="3278" type="EtcItem" name="Mason's Letter"><set name="is_questitem" val="true"/></item>
        <item id="3279" type="EtcItem" name="Iron Rose Ring"><set name="is_questitem" val="true"/></item>
        <item id="3280" type="EtcItem" name="Ascalon's 2nd Letter"><set name="is_questitem" val="true"/></item>
        <item id="3281" type="EtcItem" name="White Rose Insignia"><set name="is_questitem" val="true"/></item>
        <item id="3282" type="EtcItem" name="Groot's Letter"><set name="is_questitem" val="true"/></item>
        <item id="3283" type="EtcItem" name="Ascalon's 3rd Letter"><set name="is_questitem" val="true"/></item>
        <item id="3284" type="EtcItem" name="Mouen's 1st Order"><set name="is_questitem" val="true"/></item>
        <item id="3285" type="EtcItem" name="Mouen's 2nd Order"><set name="is_questitem" val="true"/></item>
        <item id="3286" type="EtcItem" name="Mouen's Letter"><set name="is_questitem" val="true"/></item>
        <item id="3287" type="EtcItem" name="Harpy's Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3288" type="EtcItem" name="Medusa's Venom"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3289" type="EtcItem" name="Windsus' Bile"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3290" type="EtcItem" name="Bloody Axe Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3291" type="EtcItem" name="Road Ratman Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3292" type="EtcItem" name="Leto Lizardman Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00223TestOfTheChampion)
            .plain(&[
                "30624-01.html",
                "30624-02.html",
                "30624-04.html",
                "30624-06.htm",
                "30624-06a.htm",
            ])
            .page("30624-03.htm", &["30624-05.htm"])
            .page("30624-05.htm", &["ACCEPT"])
            .plain(&[
                "30624-07.html",
                "30624-08.html",
                "30624-10.html",
                "30624-14.html",
                "30624-17.html",
            ])
            .page("30624-09.html", &["30624-10.html"])
            .page("30624-13.html", &["30624-14.html"])
            .page("30625-01.html", &["30625-02.html"])
            .page("30625-02.html", &["30625-03.html"])
            .plain(&["30625-03.html", "30625-04.html", "30625-05.html"])
            .page("30093-01.html", &["30093-02.html"])
            .plain(&[
                "30093-02.html",
                "30093-03.html",
                "30093-04.html",
                "30093-05.html",
                "30093-06.html",
            ])
            .page("30196-01.html", &["30196-02.html"])
            .page("30196-02.html", &["30196-03.html"])
            .plain(&[
                "30196-03.html",
                "30196-04.html",
                "30196-07.html",
                "30196-08.html",
                "30196-09.html",
            ])
            .page("30196-05.html", &["30196-06.html"])
            .plain(&["30196-06.html"])
            .npcs(&[TRADER_GROOT, CAPTAIN_MOUEN, VETERAN_ASCALON, MASON])
    }

    struct Npcs {
        ascalon: Oid,
        mason: Oid,
        groot: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00223TestOfTheChampion);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            ascalon: t.spawn(VETERAN_ASCALON, 20),
            mason: t.spawn(MASON, 30),
            groot: t.spawn(TRADER_GROOT, 40),
        };
        (t, npcs)
    }

    /// Gives the player items by hand (the steps the tests do not walk).
    fn give(t: &mut ScriptTest, items: &[i32], count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            for &item in items {
                ctx.give_items(Oid(1), item, count);
            }
        });
    }

    #[test]
    fn only_warriors_and_orc_raiders_of_level_39_are_taken() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, npcs.ascalon).contains("30624-01.html"));
        t.enter_as(2, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.ascalon).contains("30624-02.html"));
        t.enter_as(3, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(3, npcs.ascalon).contains("30624-03.htm"));
        t.enter_as(4, 39, race::ORC, player_class::ORC_RAIDER, vec![]);
        assert!(t.quest_page(4, npcs.ascalon).contains("30624-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_warrior_accepts_and_gets_the_first_letter_and_the_diamonds() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, npcs.ascalon);
        assert!(t.event(1, NAME, "30624-05.htm").contains("30624-05.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30624-06a.htm"));
        assert_eq!(t.count(1, ASCALONS_1ST_LETTER), 1);
        // A warrior gets 72 diamonds (other classes 64).
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 72);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
    }

    #[test]
    fn the_axe_elites_and_the_mason_make_the_ring_and_the_letter() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, npcs.ascalon);
        t.event(1, NAME, "30624-05.htm");
        t.event(1, NAME, "ACCEPT");
        t.take_writes();

        // Mason takes the first letter and gives the iron rose ring (cond 2).
        assert!(t.quest_page(1, npcs.mason).contains("30625-01.html"));
        assert!(t.event(1, NAME, "30625-02.html").contains("30625-02.html"));
        assert!(t.event(1, NAME, "30625-03.html").contains("30625-03.html"));
        assert_eq!(
            (t.count(1, ASCALONS_1ST_LETTER), t.count(1, IRON_ROSE_RING)),
            (0, 1)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));

        // Ten axe heads: the tenth elite moves to cond 3.
        for _ in 0..10 {
            let elite = t.spawn(BLOODY_AXE_ELITE, 60);
            t.kill(1, elite);
            t.world.delete_npc(elite);
        }
        assert_eq!(t.count(1, BLOODY_AXE_HEAD), 10);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));

        // Mason takes the heads and the ring and gives the letter (cond 4).
        assert!(t.quest_page(1, npcs.mason).contains("30625-05.html"));
        assert_eq!(
            (
                t.count(1, IRON_ROSE_RING),
                t.count(1, MASONS_LETTER),
                t.count(1, BLOODY_AXE_HEAD)
            ),
            (0, 1, 0)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "4")));
    }

    #[test]
    fn groot_takes_the_insignia_with_the_eggs_venom_and_bile() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, npcs.ascalon);
        t.event(1, NAME, "30624-05.htm");
        t.event(1, NAME, "ACCEPT");
        // Ascalon's second letter is in hand (the first step is tested above).
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), ASCALONS_1ST_LETTER, -1);
            ctx.give_items(Oid(1), ASCALONS_2ND_LETTER, 1);
        });
        t.take_writes();

        // Groot takes the second letter and gives the white rose insignia (cond 6).
        assert!(t.quest_page(1, npcs.groot).contains("30093-01.html"));
        assert!(t.event(1, NAME, "30093-02.html").contains("30093-02.html"));
        assert_eq!(
            (t.count(1, ASCALONS_2ND_LETTER), t.count(1, WHITE_ROSE_INSIGNIA)),
            (0, 1)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "6")));

        // With 30 eggs, 30 venom and 30 bile, Groot takes them and the insignia (cond 8).
        give(&mut t, &[HARPYS_EGG, MEDUSA_VENOM, WINDSUS_BILE], 30);
        assert!(t.quest_page(1, npcs.groot).contains("30093-04.html"));
        assert_eq!(
            (
                t.count(1, WHITE_ROSE_INSIGNIA),
                t.count(1, HARPYS_EGG),
                t.count(1, MEDUSA_VENOM),
                t.count(1, WINDSUS_BILE),
                t.count(1, GROOTS_LETTER),
            ),
            (0, 0, 0, 0, 1)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
    }

    #[test]
    fn mouen_and_ascalon_end_with_the_mark_when_the_letter_is_in_hand() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, npcs.ascalon);
        t.event(1, NAME, "30624-05.htm");
        t.event(1, NAME, "ACCEPT");
        // Mouen's letter is the last thing the test needs; the steps above are tested on their own.
        give(&mut t, &[MOUENS_LETTER], 1);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), ASCALONS_1ST_LETTER, -1);
        });
        t.take_writes();
        t.take();

        t.talk(1, npcs.ascalon);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30624-17.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_CHAMPION), 1);
        assert_eq!(t.count(1, 57), 229_764);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::delete(NAME, false)));
        assert!(writes.contains(&rows::update(NAME, "<state>", "Completed")));
        assert!(
            t.quest_page(1, npcs.ascalon)
                .contains("This quest has already been completed.")
        );
    }
}
