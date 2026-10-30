//! Quest 224: Test of Sagittarius (Guild President Bernard in Gludio: the Rogue, Elven Scout and
//! Assassin second class change, a crescent moon bow at the end).
// Java: dist/game/data/scripts/quests/Q00224_TestOfSagittarius/Q00224_TestOfSagittarius.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222), with letters handed from NPC
// to NPC on the memo state and a kill step that moves the condition on its ninth and tenth kill.
// FOLLOW-UP(engine): the killing blow weapon is not tracked (`npc.getKillingBlowWeapon()`), so the
// last step never sees the crescent moon bow and the serpent demon always respawns.

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PREFECT_VOKIAN: i32 = 30514;
const SAGITTARIUS_HAMIL: i32 = 30626;
const SIR_ARON_TANFORD: i32 = 30653;
const GUILD_PRESIDENT_BERNARD: i32 = 30702;
const MAGISTER_GAUEN: i32 = 30717;

// Items
const WOODEN_ARROW: i32 = 17;
const CRESCENT_MOON_BOW: i32 = 3028;
const BERNARDS_INTRODUCTION: i32 = 3294;
const HAMILS_1ST_LETTER: i32 = 3295;
const HAMILS_2ND_LETTER: i32 = 3296;
const HAMILS_3RD_LETTER: i32 = 3297;
const HUNTERS_1ST_RUNE: i32 = 3298;
const HUNTERS_2ND_RUNE: i32 = 3299;
const TALISMAN_OF_KADESH: i32 = 3300;
const TALISMAN_OF_SNAKE: i32 = 3301;
const MITHRIL_CLIP: i32 = 3302;
const STAKATO_CHITIN: i32 = 3303;
const REINFORCED_BOWSTRING: i32 = 3304;
const MANASHENS_HORN: i32 = 3305;
const BLOOD_OF_LIZARDMAN: i32 = 3306;

// Reward
const MARK_OF_SAGITTARIUS: i32 = 3293;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const ANT: i32 = 20079;
const ANT_CAPTAIN: i32 = 20080;
const ANT_OVERSEER: i32 = 20081;
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const ROAD_SCAVENGER: i32 = 20551;
const MANASHEN_GARGOYLE: i32 = 20563;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;

// Quest monster
const SERPENT_DEMON_KADESH: i32 = 27090;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00224_TestOfSagittarius`.
pub struct Q00224TestOfSagittarius;

impl Script for Q00224TestOfSagittarius {
    fn name(&self) -> &'static str {
        "Q00224_TestOfSagittarius"
    }

    fn path(&self) -> &'static str {
        "quests/Q00224_TestOfSagittarius"
    }

    fn id(&self) -> i32 {
        224
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUILD_PRESIDENT_BERNARD]);
        s.add_talk_id(&[
            GUILD_PRESIDENT_BERNARD,
            PREFECT_VOKIAN,
            SAGITTARIUS_HAMIL,
            SIR_ARON_TANFORD,
            MAGISTER_GAUEN,
        ]);
        s.add_kill_id(&[
            ANT,
            ANT_CAPTAIN,
            ANT_OVERSEER,
            ANT_RECRUIT,
            ANT_PATROL,
            ANT_GUARD,
            NOBLE_ANT,
            NOBLE_ANT_LEADER,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_SPIDER,
            MARSH_STAKATO_DRONE,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            ROAD_SCAVENGER,
            MANASHEN_GARGOYLE,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            SERPENT_DEMON_KADESH,
        ]);
        s.register_quest_items(&[
            CRESCENT_MOON_BOW,
            BERNARDS_INTRODUCTION,
            HAMILS_1ST_LETTER,
            HAMILS_2ND_LETTER,
            HAMILS_3RD_LETTER,
            HUNTERS_1ST_RUNE,
            HUNTERS_2ND_RUNE,
            TALISMAN_OF_KADESH,
            TALISMAN_OF_SNAKE,
            MITHRIL_CLIP,
            STAKATO_CHITIN,
            REINFORCED_BOWSTRING,
            MANASHENS_HORN,
            BLOOD_OF_LIZARDMAN,
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
                    qs.set_memo_state(ctx, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, BERNARDS_INTRODUCTION, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 96);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30702-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30702-04.htm".to_owned());
                    }
                }
            }
            "30514-02.html" => {
                if ctx.has_quest_items(player, HAMILS_2ND_LETTER) {
                    ctx.take_items(player, HAMILS_2ND_LETTER, 1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30626-02.html" | "30626-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "30626-03.html" => {
                if ctx.has_quest_items(player, BERNARDS_INTRODUCTION) {
                    ctx.take_items(player, BERNARDS_INTRODUCTION, 1);
                    ctx.give_items(player, HAMILS_1ST_LETTER, 1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30626-07.html" => {
                if ctx.get_quest_items_count(player, HUNTERS_1ST_RUNE) >= 10 {
                    ctx.give_items(player, HAMILS_2ND_LETTER, 1);
                    ctx.take_items(player, HUNTERS_1ST_RUNE, -1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30653-02.html" => {
                if ctx.has_quest_items(player, HAMILS_1ST_LETTER) {
                    ctx.take_items(player, HAMILS_1ST_LETTER, 1);
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                ANT | ANT_CAPTAIN | ANT_OVERSEER | ANT_RECRUIT | ANT_PATROL | ANT_GUARD | NOBLE_ANT
                | NOBLE_ANT_LEADER => {
                    if qs.is_memo_state(ctx, 3) && ctx.get_quest_items_count(killer, HUNTERS_1ST_RUNE) < 10 {
                        if ctx.get_quest_items_count(killer, HUNTERS_1ST_RUNE) == 9 {
                            ctx.give_items(killer, HUNTERS_1ST_RUNE, 1);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 4, true);
                        } else {
                            ctx.give_items(killer, HUNTERS_1ST_RUNE, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                    if qs.is_memo_state(ctx, 10) && !ctx.has_quest_items(killer, STAKATO_CHITIN) {
                        if ctx.has_all_quest_items(
                            killer,
                            &[MITHRIL_CLIP, REINFORCED_BOWSTRING, MANASHENS_HORN],
                        ) {
                            ctx.give_items(killer, STAKATO_CHITIN, 1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.give_items(killer, STAKATO_CHITIN, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                MARSH_SPIDER => {
                    if qs.is_memo_state(ctx, 10) && !ctx.has_quest_items(killer, REINFORCED_BOWSTRING) {
                        if ctx.has_all_quest_items(killer, &[MITHRIL_CLIP, MANASHENS_HORN, STAKATO_CHITIN]) {
                            ctx.give_items(killer, REINFORCED_BOWSTRING, 1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.give_items(killer, REINFORCED_BOWSTRING, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => {
                    if qs.is_memo_state(ctx, 6) && ctx.get_quest_items_count(killer, HUNTERS_2ND_RUNE) < 10 {
                        if ctx.get_quest_items_count(killer, HUNTERS_2ND_RUNE) == 9 {
                            ctx.give_items(killer, HUNTERS_2ND_RUNE, 1);
                            ctx.give_items(killer, TALISMAN_OF_SNAKE, 1);
                            qs.set_memo_state(ctx, 7);
                            qs.set_cond(ctx, 7, true);
                        } else {
                            ctx.give_items(killer, HUNTERS_2ND_RUNE, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                ROAD_SCAVENGER => {
                    if qs.is_memo_state(ctx, 10) && !ctx.has_quest_items(killer, MITHRIL_CLIP) {
                        if ctx.has_all_quest_items(
                            killer,
                            &[REINFORCED_BOWSTRING, MANASHENS_HORN, STAKATO_CHITIN],
                        ) {
                            ctx.give_items(killer, MITHRIL_CLIP, 1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.give_items(killer, MITHRIL_CLIP, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                MANASHEN_GARGOYLE => {
                    if qs.is_memo_state(ctx, 10) && !ctx.has_quest_items(killer, MANASHENS_HORN) {
                        if ctx.has_all_quest_items(
                            killer,
                            &[MITHRIL_CLIP, REINFORCED_BOWSTRING, STAKATO_CHITIN],
                        ) {
                            ctx.give_items(killer, MANASHENS_HORN, 1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.give_items(killer, MANASHENS_HORN, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                LETO_LIZARDMAN
                | LETO_LIZARDMAN_ARCHER
                | LETO_LIZARDMAN_SOLDIER
                | LETO_LIZARDMAN_WARRIOR
                | LETO_LIZARDMAN_SHAMAN
                | LETO_LIZARDMAN_OVERLORD => {
                    if qs.is_memo_state(ctx, 13)
                        && ctx.get_quest_items_count(killer, BLOOD_OF_LIZARDMAN) < 140
                    {
                        let blood = ctx.get_quest_items_count(killer, BLOOD_OF_LIZARDMAN);
                        if (blood - 10) * 5 > i64::from(ctx.get_random(100)) {
                            // The serpent demon comes out of the lizardman that rolled it.
                            ctx.add_spawn_at(SERPENT_DEMON_KADESH, npc.object_id, true, 300_000);
                            ctx.take_items(killer, BLOOD_OF_LIZARDMAN, -1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestBeforeBattle);
                        } else {
                            ctx.give_items(killer, BLOOD_OF_LIZARDMAN, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                SERPENT_DEMON_KADESH => {
                    if qs.is_memo_state(ctx, 13) && !ctx.has_quest_items(killer, TALISMAN_OF_KADESH) {
                        // FOLLOW-UP(engine): `npc.getKillingBlowWeapon() == CRESCENT_MOON_BOW`. The
                        // weapon of the killing blow is not kept, so it reads as no weapon.
                        if killing_blow_weapon(npc) == CRESCENT_MOON_BOW {
                            ctx.give_items(killer, TALISMAN_OF_KADESH, 1);
                            qs.set_memo_state(ctx, 14);
                            qs.set_cond(ctx, 14, true);
                        } else {
                            ctx.add_spawn_at(SERPENT_DEMON_KADESH, npc.object_id, true, 300_000);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == GUILD_PRESIDENT_BERNARD {
                if matches!(
                    ctx.player_class_id(player),
                    player_class::ROGUE | player_class::ELVEN_SCOUT | player_class::ASSASSIN
                ) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "30702-03.htm".to_owned();
                    } else {
                        htmltext = "30702-01.html".to_owned();
                    }
                } else {
                    htmltext = "30702-02.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                GUILD_PRESIDENT_BERNARD => {
                    if ctx.has_quest_items(player, BERNARDS_INTRODUCTION) {
                        htmltext = "30702-05.html".to_owned();
                    }
                }
                PREFECT_VOKIAN => match memo_state {
                    5 => {
                        if ctx.has_quest_items(player, HAMILS_2ND_LETTER) {
                            htmltext = "30514-01.html".to_owned();
                        }
                    }
                    6 => htmltext = "30514-03.html".to_owned(),
                    7 => {
                        if ctx.has_quest_items(player, TALISMAN_OF_SNAKE) {
                            ctx.take_items(player, TALISMAN_OF_SNAKE, 1);
                            qs.set_memo_state(ctx, 8);
                            qs.set_cond(ctx, 8, true);
                            htmltext = "30514-04.html".to_owned();
                        }
                    }
                    8 => htmltext = "30514-05.html".to_owned(),
                    _ => {}
                },
                SAGITTARIUS_HAMIL => match memo_state {
                    1 => {
                        if ctx.has_quest_items(player, BERNARDS_INTRODUCTION) {
                            htmltext = "30626-01.html".to_owned();
                        }
                    }
                    2 => {
                        if ctx.has_quest_items(player, HAMILS_1ST_LETTER) {
                            htmltext = "30626-04.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.get_quest_items_count(player, HUNTERS_1ST_RUNE) == 10 {
                            htmltext = "30626-05.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_quest_items(player, HAMILS_2ND_LETTER) {
                            htmltext = "30626-08.html".to_owned();
                        }
                    }
                    8 => {
                        ctx.give_items(player, HAMILS_3RD_LETTER, 1);
                        ctx.take_items(player, HUNTERS_2ND_RUNE, -1);
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30626-09.html".to_owned();
                    }
                    9 => {
                        if ctx.has_quest_items(player, HAMILS_3RD_LETTER) {
                            htmltext = "30626-10.html".to_owned();
                        }
                    }
                    12 => {
                        if ctx.has_quest_items(player, CRESCENT_MOON_BOW) {
                            qs.set_cond(ctx, 13, true);
                            qs.set_memo_state(ctx, 13);
                            htmltext = "30626-11.html".to_owned();
                        }
                    }
                    13 => htmltext = "30626-12.html".to_owned(),
                    14 => {
                        if ctx.has_quest_items(player, TALISMAN_OF_KADESH) {
                            ctx.give_adena(player, 161_806, true);
                            ctx.give_items(player, MARK_OF_SAGITTARIUS, 1);
                            ctx.add_exp_and_sp(player, 894_888, 61_408);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                player,
                                &SocialAction {
                                    object_id: player.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30626-13.html".to_owned();
                        }
                    }
                    _ => {}
                },
                SIR_ARON_TANFORD => {
                    if memo_state == 2 {
                        if ctx.has_quest_items(player, HAMILS_1ST_LETTER) {
                            htmltext = "30653-01.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        htmltext = "30653-03.html".to_owned();
                    }
                }
                MAGISTER_GAUEN => match memo_state {
                    9 => {
                        if ctx.has_quest_items(player, HAMILS_3RD_LETTER) {
                            ctx.take_items(player, HAMILS_3RD_LETTER, 1);
                            qs.set_memo_state(ctx, 10);
                            qs.set_cond(ctx, 10, true);
                            htmltext = "30717-01.html".to_owned();
                        }
                    }
                    10 => htmltext = "30717-03.html".to_owned(),
                    12 => htmltext = "30717-04.html".to_owned(),
                    11 => {
                        if ctx.has_all_quest_items(
                            player,
                            &[STAKATO_CHITIN, MITHRIL_CLIP, REINFORCED_BOWSTRING, MANASHENS_HORN],
                        ) {
                            ctx.give_items(player, WOODEN_ARROW, 10);
                            ctx.give_items(player, CRESCENT_MOON_BOW, 1);
                            ctx.take_items(player, MITHRIL_CLIP, 1);
                            ctx.take_items(player, STAKATO_CHITIN, 1);
                            ctx.take_items(player, REINFORCED_BOWSTRING, 1);
                            ctx.take_items(player, MANASHENS_HORN, 1);
                            qs.set_memo_state(ctx, 12);
                            qs.set_cond(ctx, 12, true);
                            htmltext = "30717-02.html".to_owned();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        } else if state == COMPLETED && npc.id == GUILD_PRESIDENT_BERNARD {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// FOLLOW-UP(engine): Java `npc.getKillingBlowWeapon()` (the item id of the weapon that made the
/// killing blow, 0 without one). The world does not keep it yet, so this is always 0.
fn killing_blow_weapon(_npc: ScriptNpc) -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{Qs, player_class};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00224_TestOfSagittarius";

    const NPCS: &str = r#"
        <npc id="30514" type="Folk" level="70" name="Prefect Vokian"/>
        <npc id="30626" type="Folk" level="70" name="Sagittarius Hamil"/>
        <npc id="30653" type="Folk" level="70" name="Sir Aron Tanford"/>
        <npc id="30702" type="Folk" level="70" name="Guild President Bernard"/>
        <npc id="30717" type="Folk" level="70" name="Magister Gauen"/>
        <npc id="20079" type="Monster" level="40" name="Ant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17" type="EtcItem" name="Wooden Arrow"><set name="is_stackable" val="true"/></item>
        <item id="3028" type="Weapon" name="Crescent Moon Bow"><set name="is_questitem" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3293" type="EtcItem" name="Mark of Sagittarius"><set name="is_questitem" val="true"/></item>
        <item id="3294" type="EtcItem" name="Bernard's Introduction"><set name="is_questitem" val="true"/></item>
        <item id="3295" type="EtcItem" name="Hamil's 1st Letter"><set name="is_questitem" val="true"/></item>
        <item id="3296" type="EtcItem" name="Hamil's 2nd Letter"><set name="is_questitem" val="true"/></item>
        <item id="3297" type="EtcItem" name="Hamil's 3rd Letter"><set name="is_questitem" val="true"/></item>
        <item id="3298" type="EtcItem" name="Hunter's 1st Rune"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3299" type="EtcItem" name="Hunter's 2nd Rune"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3300" type="EtcItem" name="Talisman of Kadesh"><set name="is_questitem" val="true"/></item>
        <item id="3301" type="EtcItem" name="Talisman of Snake"><set name="is_questitem" val="true"/></item>
        <item id="3302" type="EtcItem" name="Mithril Clip"><set name="is_questitem" val="true"/></item>
        <item id="3303" type="EtcItem" name="Stakato Chitin"><set name="is_questitem" val="true"/></item>
        <item id="3304" type="EtcItem" name="Reinforced Bowstring"><set name="is_questitem" val="true"/></item>
        <item id="3305" type="EtcItem" name="Manashen's Horn"><set name="is_questitem" val="true"/></item>
        <item id="3306" type="EtcItem" name="Blood of Lizardman"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00224TestOfSagittarius)
            .plain(&[
                "30702-01.html",
                "30702-02.html",
                "30702-04.htm",
                "30702-04a.htm",
                "30702-05.html",
            ])
            .page("30702-03.htm", &["ACCEPT"])
            .page("30514-01.html", &["30514-02.html"])
            .plain(&["30514-02.html", "30514-03.html", "30514-04.html", "30514-05.html"])
            .page("30626-01.html", &["30626-02.html"])
            .page("30626-02.html", &["30626-03.html"])
            .plain(&["30626-03.html"])
            .plain(&["30626-04.html", "30626-08.html", "30626-09.html", "30626-10.html"])
            .page("30626-05.html", &["30626-06.html"])
            .page("30626-06.html", &["30626-07.html"])
            .plain(&["30626-07.html", "30626-11.html", "30626-12.html", "30626-13.html"])
            .page("30653-01.html", &["30653-02.html"])
            .plain(&["30653-02.html", "30653-03.html"])
            .plain(&["30717-01.html", "30717-02.html", "30717-03.html", "30717-04.html"])
            .npcs(&[
                GUILD_PRESIDENT_BERNARD,
                PREFECT_VOKIAN,
                SAGITTARIUS_HAMIL,
                SIR_ARON_TANFORD,
                MAGISTER_GAUEN,
            ])
    }

    struct Npcs {
        bernard: Oid,
        hamil: Oid,
        aron: Oid,
        vokian: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00224TestOfSagittarius);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            bernard: t.spawn(GUILD_PRESIDENT_BERNARD, 20),
            hamil: t.spawn(SAGITTARIUS_HAMIL, 30),
            aron: t.spawn(SIR_ARON_TANFORD, 40),
            vokian: t.spawn(PREFECT_VOKIAN, 50),
        };
        (t, npcs)
    }

    /// Runs `f` on the quest state of player 1 (memo states and items a test reaches by hand).
    fn with_state(t: &mut ScriptTest, f: impl FnOnce(&mut ScriptCtx<'_>, Qs)) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                f(ctx, qs);
            }
        });
    }

    #[test]
    fn only_the_rogues_and_scouts_of_level_39_are_taken() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, 0, player_class::ROGUE, vec![]);
        assert!(t.quest_page(1, npcs.bernard).contains("30702-01.html"));
        t.enter_as(2, 40, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.bernard).contains("30702-02.html"));
        t.enter_as(3, 39, 0, player_class::ELVEN_SCOUT, vec![]);
        assert!(t.quest_page(3, npcs.bernard).contains("30702-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_introduction_and_the_diamonds() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.bernard);
        assert!(t.event(1, NAME, "ACCEPT").contains("30702-04a.htm"));
        assert_eq!(t.count(1, BERNARDS_INTRODUCTION), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 96);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
    }

    #[test]
    fn the_letters_go_from_hamil_to_aron_and_the_hunters_runes_come_in() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.bernard);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();

        // Hamil takes the introduction and gives the first letter (memo 2, cond 2).
        assert!(t.quest_page(1, npcs.hamil).contains("30626-01.html"));
        assert!(t.event(1, NAME, "30626-02.html").contains("30626-02.html"));
        assert!(t.event(1, NAME, "30626-03.html").contains("30626-03.html"));
        assert_eq!(t.count(1, HAMILS_1ST_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Aron takes the first letter (memo 3, cond 3); ants give the runes from then on.
        assert!(t.quest_page(1, npcs.aron).contains("30653-01.html"));
        assert!(t.event(1, NAME, "30653-02.html").contains("30653-02.html"));
        assert_eq!(t.count(1, HAMILS_1ST_LETTER), 0);
        t.take_writes();
        for _ in 0..9 {
            let ant = t.spawn(ANT, 70);
            t.kill(1, ant);
            t.world.delete_npc(ant);
        }
        assert_eq!(t.count(1, HUNTERS_1ST_RUNE), 9);
        // The tenth rune completes the hunt: memo 4, cond 4.
        let ant = t.spawn(ANT, 70);
        t.kill(1, ant);
        t.world.delete_npc(ant);
        assert_eq!(t.count(1, HUNTERS_1ST_RUNE), 10);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "memoState", "4")));
        assert!(writes.contains(&rows::update(NAME, "cond", "4")));
    }

    #[test]
    fn the_second_letter_is_given_for_ten_runes() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.bernard);
        t.event(1, NAME, "ACCEPT");
        // Hand the step over by hand: memo 4 with the ten runes (the hunt is tested above).
        with_state(&mut t, |ctx, qs| {
            qs.set_memo_state(ctx, 4);
            ctx.give_items(Oid(1), HUNTERS_1ST_RUNE, 10);
        });
        t.take_writes();

        assert!(t.quest_page(1, npcs.hamil).contains("30626-05.html"));
        assert!(t.event(1, NAME, "30626-06.html").contains("30626-06.html"));
        assert!(t.event(1, NAME, "30626-07.html").contains("30626-07.html"));
        assert_eq!(
            (t.count(1, HUNTERS_1ST_RUNE), t.count(1, HAMILS_2ND_LETTER)),
            (0, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483631"),
            ]
        );
        // Vokian takes the second letter (memo 6, cond 6).
        assert!(t.quest_page(1, npcs.vokian).contains("30514-01.html"));
        assert!(t.event(1, NAME, "30514-02.html").contains("30514-02.html"));
        assert_eq!(t.count(1, HAMILS_2ND_LETTER), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "6"),
                rows::update(NAME, "__compltdStateFlags", "-2147483599"),
            ]
        );
    }

    #[test]
    fn the_reward_is_the_mark_after_the_talisman_of_kadesh() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.bernard);
        t.event(1, NAME, "ACCEPT");
        // Hamil's last step (memo 14) with the talisman in hand.
        with_state(&mut t, |ctx, qs| {
            qs.set_memo_state(ctx, 14);
            ctx.give_items(Oid(1), TALISMAN_OF_KADESH, 1);
        });
        t.take_writes();
        t.take();
        t.talk(1, npcs.hamil);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30626-13.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_SAGITTARIUS), 1);
        assert_eq!(t.count(1, 57), 161_806);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.bernard)
                .contains("This quest has already been completed.")
        );
    }
}
