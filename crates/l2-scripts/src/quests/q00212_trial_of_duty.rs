//! Quest 212: Trial of Duty (Hannavalt in Talking Island's Knight trial, the Knights of Aden).
// Java: dist/game/data/scripts/quests/Q00212_TrialOfDuty/Q00212_TrialOfDuty.java
//
// Pattern: a memo-state chain over many NPCs with kill steps that roll a flag (`qs.set("flag")`)
// and a spirit that spawns at the killer's mob (`docs/design/scripts.md`, "Quest patterns"). The
// memo state is the quest's step counter, the condition is the journal step. One-time:
// `exit_quest(ctx, false, true)`.

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HANNAVALT: i32 = 30109;
const DUSTIN: i32 = 30116;
const SIR_COLLIN_WINDAWOOD: i32 = 30311;
const SIR_ARON_TANFORD: i32 = 30653;
const SIR_KIEL_NIGHTHAWK: i32 = 30654;
const ISAEL_SILVERSHADOW: i32 = 30655;
const SPIRIT_OF_SIR_TALIANUS: i32 = 30656;

// Items
const LETTER_OF_DUSTIN: i32 = 2634;
const KNIGHTS_TEAR: i32 = 2635;
const MIRROR_OF_ORPIC: i32 = 2636;
const TEAR_OF_CONFESSION: i32 = 2637;
/// Java `ItemHolder(2638, 10)`: the report pieces, 10 of them make the report.
const REPORT_PIECE: (i32, i64) = (2638, 10);
const TALIANUSS_REPORT: i32 = 2639;
const TEAR_OF_LOYALTY: i32 = 2640;
/// Java `ItemHolder(2641, 20)`: the militas articles, 20 of them.
const MILITAS_ARTICLE: (i32, i64) = (2641, 20);
// NOTE(java-quirk): `SAINTS_ASHES_URN` is item 2641 too, the same id as `MILITAS_ARTICLE`.
const SAINTS_ASHES_URN: i32 = 2641;
const ATHEBALDTS_SKULL: i32 = 2643;
const ATHEBALDTS_RIBS: i32 = 2644;
const ATHEBALDTS_SHIN: i32 = 2645;
const LETTER_OF_WINDAWOOD: i32 = 2646;
const OLD_KNIGHTS_SWORD: i32 = 3027;

// Monsters
const HANGMAN_TREE: i32 = 20144;
const SKELETON_MARAUDER: i32 = 20190;
const SKELETON_RAIDER: i32 = 20191;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const BREKA_ORC_OVERLORD: i32 = 20270;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const SPIRIT_OF_SIR_HEROD: i32 = 27119;

// Rewards
const MARK_OF_DUTY: i32 = 2633;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Misc
const MIN_LEVEL: i32 = 35;
/// Java `PlayerClass.KNIGHT`.
const KNIGHT: i32 = 4;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00212_TrialOfDuty`.
pub struct Q00212TrialOfDuty;

/// Java `player.isInCategory(CategoryType.KNIGHT_GROUP)` (the loaded `CategoryData`).
fn in_knight_group(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world()
        .is_in_category(player, l2_world::subclass::CategoryType::KnightGroup)
}

/// Java `getActiveWeaponItem()` id of the player (0 when none).
fn active_weapon_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world()
        .stats(player)
        .and_then(|s| s.active_weapon_item().map(|w| w.id))
        .unwrap_or(0)
}

/// Java `rewardDimensionalDiamonds(player)`: `true` when they were given now.
fn reward_dimensional_diamonds(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
        if ctx.player_class_id(player) == KNIGHT {
            ctx.reward_items(player, DIMENSIONAL_DIAMOND, 45);
        } else {
            ctx.reward_items(player, DIMENSIONAL_DIAMOND, 61);
        }
        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
        return true;
    }
    false
}

impl Script for Q00212TrialOfDuty {
    fn name(&self) -> &'static str {
        "Q00212_TrialOfDuty"
    }

    fn path(&self) -> &'static str {
        "quests/Q00212_TrialOfDuty"
    }

    fn id(&self) -> i32 {
        212
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HANNAVALT]);
        s.add_talk_id(&[
            HANNAVALT,
            DUSTIN,
            SIR_COLLIN_WINDAWOOD,
            SIR_ARON_TANFORD,
            SIR_KIEL_NIGHTHAWK,
            ISAEL_SILVERSHADOW,
            SPIRIT_OF_SIR_TALIANUS,
        ]);
        s.add_kill_id(&[
            HANGMAN_TREE,
            SKELETON_MARAUDER,
            SKELETON_RAIDER,
            STRAIN,
            GHOUL,
            BREKA_ORC_OVERLORD,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            SPIRIT_OF_SIR_HEROD,
        ]);
        s.register_quest_items(&[
            LETTER_OF_DUSTIN,
            KNIGHTS_TEAR,
            MIRROR_OF_ORPIC,
            TEAR_OF_CONFESSION,
            REPORT_PIECE.0,
            TALIANUSS_REPORT,
            TEAR_OF_LOYALTY,
            MILITAS_ARTICLE.0,
            SAINTS_ASHES_URN,
            ATHEBALDTS_SKULL,
            ATHEBALDTS_RIBS,
            ATHEBALDTS_SHIN,
            LETTER_OF_WINDAWOOD,
            OLD_KNIGHTS_SWORD,
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

        let mut html = None;
        match event {
            "quest_accept" => {
                if qs.is_created(ctx) && ctx.player_level(player) >= MIN_LEVEL && in_knight_group(ctx, player)
                {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    qs.set_int(ctx, "flag", 0);
                    if reward_dimensional_diamonds(ctx, player) {
                        html = Some("30109-04a.htm".to_owned());
                    } else {
                        html = Some("30109-04.htm".to_owned());
                    }
                }
            }
            "30116-02.html" | "30116-03.html" | "30116-04.html" => {
                if qs.is_memo_state(ctx, 10) && ctx.has_quest_items(player, TEAR_OF_LOYALTY) {
                    html = Some(event.to_owned());
                }
            }
            "30116-05.html" => {
                if qs.is_memo_state(ctx, 10) && ctx.has_quest_items(player, TEAR_OF_LOYALTY) {
                    html = Some(event.to_owned());
                    ctx.take_items(player, TEAR_OF_LOYALTY, -1);
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 14, true);
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        let Some(qs) = qs else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), killer, npc.object_id, true) {
            return;
        }

        match npc.id {
            SKELETON_MARAUDER | SKELETON_RAIDER => {
                if qs.is_memo_state(ctx, 2) {
                    let flag = qs.get_int(ctx, "flag");
                    if ctx.get_random(100) < flag * 10 {
                        ctx.add_spawn_at(SPIRIT_OF_SIR_HEROD, npc.object_id, false, 0);
                        qs.set_int(ctx, "flag", 0);
                    } else {
                        qs.set_int(ctx, "flag", flag + 1);
                    }
                }
            }
            SPIRIT_OF_SIR_HEROD => {
                if qs.is_memo_state(ctx, 2) && active_weapon_id(ctx, killer) == OLD_KNIGHTS_SWORD {
                    ctx.give_items(killer, KNIGHTS_TEAR, 1);
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                }
            }
            STRAIN | GHOUL => {
                if qs.is_memo_state(ctx, 5)
                    && !ctx.has_quest_items(killer, TALIANUSS_REPORT)
                    && ctx.give_item_randomly(
                        killer,
                        Some(npc),
                        REPORT_PIECE.0,
                        1,
                        1,
                        REPORT_PIECE.1,
                        1.0,
                        true,
                    )
                {
                    ctx.take_items(killer, REPORT_PIECE.0, REPORT_PIECE.1);
                    ctx.give_items(killer, TALIANUSS_REPORT, 1);
                    qs.set_cond(ctx, 6, false);
                }
            }
            HANGMAN_TREE => {
                if qs.is_memo_state(ctx, 6) {
                    let flag = qs.get_int(ctx, "flag");
                    if ctx.get_random(100) < (flag - 3) * 33 {
                        ctx.add_spawn_at(SPIRIT_OF_SIR_TALIANUS, npc.object_id, false, 0);
                        qs.set_int(ctx, "flag", 0);
                        qs.set_cond(ctx, 8, true);
                    } else {
                        qs.set_int(ctx, "flag", flag + 1);
                    }
                }
            }
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR
            | LETO_LIZARDMAN_SHAMAN
            | LETO_LIZARDMAN_OVERLORD => {
                if qs.is_memo_state(ctx, 9)
                    && ctx.give_item_randomly(
                        killer,
                        Some(npc),
                        MILITAS_ARTICLE.0,
                        1,
                        1,
                        MILITAS_ARTICLE.1,
                        1.0,
                        true,
                    )
                {
                    qs.set_cond(ctx, 12, false);
                }
            }
            BREKA_ORC_OVERLORD => {
                if qs.is_memo_state(ctx, 11) {
                    if !ctx.has_quest_items(killer, ATHEBALDTS_SKULL) {
                        ctx.give_items(killer, ATHEBALDTS_SKULL, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    } else if !ctx.has_quest_items(killer, ATHEBALDTS_RIBS) {
                        ctx.give_items(killer, ATHEBALDTS_RIBS, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    } else if !ctx.has_quest_items(killer, ATHEBALDTS_SHIN) {
                        ctx.give_items(killer, ATHEBALDTS_SHIN, 1);
                        qs.set_cond(ctx, 15, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);

        match npc.id {
            HANNAVALT => {
                if qs.is_created(ctx) {
                    if !in_knight_group(ctx, talker) {
                        html = "30109-02.html".to_owned();
                    } else if ctx.player_level(talker) < MIN_LEVEL {
                        html = "30109-01.html".to_owned();
                    } else {
                        html = "30109-03.htm".to_owned();
                    }
                } else if qs.is_started(ctx) {
                    match qs.get_memo_state(ctx) {
                        1 => {
                            html = "30109-04.html".to_owned();
                        }
                        14 => {
                            if ctx.has_quest_items(talker, LETTER_OF_DUSTIN) {
                                html = "30109-05.html".to_owned();
                                ctx.take_items(talker, LETTER_OF_DUSTIN, -1);
                                ctx.add_exp_and_sp(talker, 762_576, 49_458);
                                ctx.give_adena(talker, 138_968, true);
                                ctx.give_items(talker, MARK_OF_DUTY, 1);
                                qs.exit_quest(ctx, false, true);
                                ctx.world().send(
                                    talker,
                                    &SocialAction {
                                        object_id: talker.0,
                                        action_id: 3,
                                    },
                                );
                                reward_dimensional_diamonds(ctx, talker);
                            }
                        }
                        _ => {}
                    }
                } else {
                    html = ctx.get_already_completed_msg(talker);
                }
            }
            SIR_ARON_TANFORD => match qs.get_memo_state(ctx) {
                1 => {
                    html = "30653-01.html".to_owned();
                    if !ctx.has_quest_items(talker, OLD_KNIGHTS_SWORD) {
                        ctx.give_items(talker, OLD_KNIGHTS_SWORD, 1);
                    }
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                }
                2 => {
                    if ctx.has_quest_items(talker, OLD_KNIGHTS_SWORD) {
                        html = "30653-02.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_quest_items(talker, KNIGHTS_TEAR) {
                        html = "30653-03.html".to_owned();
                        ctx.take_items_many(talker, -1, &[KNIGHTS_TEAR, OLD_KNIGHTS_SWORD]);
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 4, true);
                    }
                }
                4 => {
                    html = "30653-04.html".to_owned();
                }
                _ => {}
            },
            SIR_KIEL_NIGHTHAWK => match qs.get_memo_state(ctx) {
                4 => {
                    html = "30654-01.html".to_owned();
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                }
                5 => {
                    if !ctx.has_quest_items(talker, TALIANUSS_REPORT) {
                        html = "30654-02.html".to_owned();
                    } else {
                        html = "30654-03.html".to_owned();
                        qs.set_memo_state(ctx, 6);
                        qs.set_cond(ctx, 7, true);
                        ctx.give_items(talker, MIRROR_OF_ORPIC, 1);
                    }
                }
                6 => {
                    if ctx.has_quest_items(talker, MIRROR_OF_ORPIC) {
                        html = "30654-04.html".to_owned();
                    }
                }
                7 => {
                    if ctx.has_quest_items(talker, TEAR_OF_CONFESSION) {
                        html = "30654-05.html".to_owned();
                        ctx.take_items(talker, TEAR_OF_CONFESSION, -1);
                        qs.set_memo_state(ctx, 8);
                        qs.set_cond(ctx, 10, true);
                    }
                }
                8 => {
                    html = "30654-06.html".to_owned();
                }
                _ => {}
            },
            SPIRIT_OF_SIR_TALIANUS => {
                if qs.is_memo_state(ctx, 6)
                    && ctx.has_all_quest_items(talker, &[MIRROR_OF_ORPIC, TALIANUSS_REPORT])
                {
                    html = "30656-01.html".to_owned();
                    ctx.take_items_many(talker, -1, &[MIRROR_OF_ORPIC, TALIANUSS_REPORT]);
                    ctx.give_items(talker, TEAR_OF_CONFESSION, 1);
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 9, true);
                    ctx.delete_npc(npc);
                }
            }
            ISAEL_SILVERSHADOW => match qs.get_memo_state(ctx) {
                8 => {
                    if ctx.player_level(talker) < MIN_LEVEL {
                        html = "30655-01.html".to_owned();
                    } else {
                        html = "30655-02.html".to_owned();
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 11, true);
                    }
                }
                9 => {
                    if ctx.get_quest_items_count(talker, MILITAS_ARTICLE.0) < MILITAS_ARTICLE.1 {
                        html = "30655-03.html".to_owned();
                    } else {
                        html = "30655-04.html".to_owned();
                        ctx.give_items(talker, TEAR_OF_LOYALTY, 1);
                        ctx.take_items(talker, MILITAS_ARTICLE.0, MILITAS_ARTICLE.1);
                        qs.set_memo_state(ctx, 10);
                        qs.set_cond(ctx, 13, true);
                    }
                }
                10 => {
                    if ctx.has_quest_items(talker, TEAR_OF_LOYALTY) {
                        html = "30655-05.html".to_owned();
                    }
                }
                _ => {}
            },
            DUSTIN => match qs.get_memo_state(ctx) {
                10 => {
                    if ctx.has_quest_items(talker, TEAR_OF_LOYALTY) {
                        html = "30116-01.html".to_owned();
                    }
                }
                11 => {
                    if !ctx.has_all_quest_items(talker, &[ATHEBALDTS_SKULL, ATHEBALDTS_RIBS, ATHEBALDTS_SHIN])
                    {
                        html = "30116-06.html".to_owned();
                    } else {
                        html = "30116-07.html".to_owned();
                        ctx.take_items_many(
                            talker,
                            -1,
                            &[ATHEBALDTS_SKULL, ATHEBALDTS_RIBS, ATHEBALDTS_SHIN],
                        );
                        ctx.give_items(talker, SAINTS_ASHES_URN, 1);
                        qs.set_memo_state(ctx, 12);
                        qs.set_cond(ctx, 16, true);
                    }
                }
                12 => {
                    if ctx.has_quest_items(talker, SAINTS_ASHES_URN) {
                        html = "30116-09.html".to_owned();
                    }
                }
                13 => {
                    if ctx.has_quest_items(talker, LETTER_OF_WINDAWOOD) {
                        html = "30116-08.html".to_owned();
                        ctx.take_items(talker, LETTER_OF_WINDAWOOD, -1);
                        ctx.give_items(talker, LETTER_OF_DUSTIN, 1);
                        qs.set_memo_state(ctx, 14);
                        qs.set_cond(ctx, 18, true);
                    }
                }
                14 => {
                    if ctx.has_quest_items(talker, LETTER_OF_DUSTIN) {
                        html = "30116-10.html".to_owned();
                    }
                }
                _ => {}
            },
            SIR_COLLIN_WINDAWOOD => match qs.get_memo_state(ctx) {
                12 => {
                    if ctx.has_quest_items(talker, SAINTS_ASHES_URN) {
                        html = "30311-01.html".to_owned();
                        ctx.take_items(talker, SAINTS_ASHES_URN, -1);
                        ctx.give_items(talker, LETTER_OF_WINDAWOOD, 1);
                        qs.set_memo_state(ctx, 13);
                        qs.set_cond(ctx, 17, true);
                    }
                }
                13 => {
                    if ctx.has_quest_items(talker, LETTER_OF_WINDAWOOD) {
                        html = "30311-02.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_protocol_game::server::charscreen::paperdoll;
    use l2_util::ObjectId as Oid;
    use l2_world::script::player_class;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00212_TrialOfDuty";

    const NPCS: &str = r#"
        <npc id="30109" type="Folk" level="70" name="Hannavalt"/>
        <npc id="30116" type="Folk" level="70" name="Dustin"/>
        <npc id="30311" type="Folk" level="70" name="Sir Collin Windawood"/>
        <npc id="30653" type="Folk" level="70" name="Sir Aron Tanford"/>
        <npc id="30654" type="Folk" level="70" name="Sir Kiel Nighthawk"/>
        <npc id="30655" type="Folk" level="70" name="Isael Silvershadow"/>
        <npc id="30656" type="Folk" level="70" name="Spirit of Sir Talianus"/>
        <npc id="20190" type="Monster" level="30" name="Skeleton Marauder"/>
        <npc id="20191" type="Monster" level="30" name="Skeleton Raider"/>
        <npc id="20200" type="Monster" level="30" name="Strain"/>
        <npc id="20144" type="Monster" level="30" name="Hangman Tree"/>
        <npc id="20270" type="Monster" level="30" name="Breka Orc Overlord"/>
        <npc id="20577" type="Monster" level="30" name="Leto Lizardman"/>
        <npc id="27119" type="Monster" level="30" name="Spirit of Sir Herodd"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="2633" type="EtcItem" name="Mark of Duty"/>
        <item id="2634" type="EtcItem" name="Letter of Dustin"><set name="is_questitem" val="true"/></item>
        <item id="2635" type="EtcItem" name="Knight's Tear"><set name="is_questitem" val="true"/></item>
        <item id="2636" type="EtcItem" name="Mirror of Orpic"><set name="is_questitem" val="true"/></item>
        <item id="2637" type="EtcItem" name="Tear of Confession"><set name="is_questitem" val="true"/></item>
        <item id="2638" type="EtcItem" name="Report Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2639" type="EtcItem" name="Talianus's Report"><set name="is_questitem" val="true"/></item>
        <item id="2640" type="EtcItem" name="Tear of Loyalty"><set name="is_questitem" val="true"/></item>
        <item id="2641" type="EtcItem" name="Militas Article"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2643" type="EtcItem" name="Athebaldt's Skull"><set name="is_questitem" val="true"/></item>
        <item id="2644" type="EtcItem" name="Athebaldt's Ribs"><set name="is_questitem" val="true"/></item>
        <item id="2645" type="EtcItem" name="Athebaldt's Shin"><set name="is_questitem" val="true"/></item>
        <item id="2646" type="EtcItem" name="Letter of Windawood"><set name="is_questitem" val="true"/></item>
        <item id="3027" type="Weapon" name="Old Knight's Sword"><set name="bodypart" val="rhand"/><set name="is_questitem" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Q00212TrialOfDuty)
            .page("30109-03.htm", &["quest_accept"])
            .page("30109-04a.htm", &[])
            .plain(&["30109-01.html", "30109-02.html", "30109-04.html", "30109-05.html"])
            .plain(&["30653-01.html", "30653-02.html", "30653-03.html", "30653-04.html"])
            .plain(&["30654-01.html", "30654-02.html", "30654-03.html", "30654-04.html"])
            .plain(&["30654-05.html", "30654-06.html"])
            .plain(&[
                "30655-01.html",
                "30655-02.html",
                "30655-03.html",
                "30655-04.html",
                "30655-05.html",
            ])
            .plain(&["30656-01.html"])
            .page("30116-01.html", &["30116-05.html"])
            .plain(&["30116-02.html", "30116-03.html", "30116-04.html", "30116-05.html"])
            .plain(&[
                "30116-06.html",
                "30116-07.html",
                "30116-08.html",
                "30116-09.html",
                "30116-10.html",
            ])
            .plain(&["30311-01.html", "30311-02.html"])
            .npcs(&[
                HANNAVALT,
                DUSTIN,
                SIR_COLLIN_WINDAWOOD,
                SIR_ARON_TANFORD,
                SIR_KIEL_NIGHTHAWK,
                ISAEL_SILVERSHADOW,
                SPIRIT_OF_SIR_TALIANUS,
            ])
    }

    struct Npcs {
        hannavalt: Oid,
        aron: Oid,
        kiel: Oid,
        isael: Oid,
        dustin: Oid,
        collin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00212TrialOfDuty);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            hannavalt: t.spawn(HANNAVALT, 20),
            aron: t.spawn(SIR_ARON_TANFORD, 25),
            kiel: t.spawn(SIR_KIEL_NIGHTHAWK, 30),
            isael: t.spawn(ISAEL_SILVERSHADOW, 35),
            dustin: t.spawn(DUSTIN, 40),
            collin: t.spawn(SIR_COLLIN_WINDAWOOD, 45),
        };
        (t, npcs)
    }

    /// A human knight of level 40, with `items`.
    fn enter_knight(t: &mut ScriptTest, items: Vec<l2_world::ItemData>) {
        t.enter_as(1, 40, 0, player_class::KNIGHT, items);
    }

    /// Kills a monster next to the player; it is deleted after the kill.
    fn kill_mob(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 10);
        kill_npc(t, mob);
    }

    fn kill_npc(t: &mut ScriptTest, mob: Oid) {
        t.kill(1, mob);
        t.world.delete_npc(mob);
    }

    /// The `character_quests` writes since the last call.
    fn rows_of(t: &ScriptTest) -> Vec<QuestWrite> {
        t.take_writes()
    }

    /// The sword in the right hand: the weapon the spirit of Sir Herodd needs.
    fn equipped_sword(object_id: i32) -> l2_world::ItemData {
        l2_world::ItemData {
            loc: "PAPERDOLL".to_owned(),
            loc_data: paperdoll::RHAND as i32,
            ..item(object_id, OLD_KNIGHTS_SWORD, 1)
        }
    }

    #[test]
    fn a_character_outside_the_knight_group_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, 0, 0, vec![]);
        assert!(t.quest_page(1, npcs.hannavalt).contains("30109-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_knight_below_level_35_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 34, 0, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.hannavalt).contains("30109-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_diamonds_once_for_a_knight() {
        let (mut t, npcs) = setup();
        enter_knight(&mut t, vec![]);
        assert!(t.quest_page(1, npcs.hannavalt).contains("30109-03.htm"));
        assert!(t.event(1, NAME, "quest_accept").contains("30109-04a.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "flag", "0"),
            ]
        );
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 45);
    }

    #[test]
    fn the_whole_trial_from_the_sword_to_the_mark() {
        let (mut t, npcs) = setup();
        enter_knight(&mut t, vec![equipped_sword(200)]);
        assert!(t.quest_page(1, npcs.hannavalt).contains("30109-03.htm"));
        t.event(1, NAME, "quest_accept");
        t.take_writes();

        // Sir Aron hands out the sword only when the character has none (memo 2).
        assert!(t.quest_page(1, npcs.aron).contains("30653-01.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Skeletons roll for the spirit of Sir Herodd until one spawns (memo 2).
        let mut spirit = None;
        for _ in 0..200 {
            kill_mob(&mut t, SKELETON_MARAUDER);
            if let Some(s) = t.npcs_of(SPIRIT_OF_SIR_HEROD).first() {
                spirit = Some(*s);
                break;
            }
        }
        // The `flag` rows of the rolls are not checked here.
        t.take_writes();
        let spirit = spirit.expect("the spirit spawns from the skeletons");
        // The spirit gives the knight's tear with the old knight's sword in hand (memo 3).
        kill_npc(&mut t, spirit);
        assert_eq!(t.count(1, KNIGHTS_TEAR), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // Sir Aron takes the tear and the sword (memo 4).
        assert!(t.quest_page(1, npcs.aron).contains("30653-03.html"));
        assert_eq!((t.count(1, KNIGHTS_TEAR), t.count(1, OLD_KNIGHTS_SWORD)), (0, 0));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Sir Kiel sends the knight to the strains (memo 5).
        assert!(t.quest_page(1, npcs.kiel).contains("30654-01.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // Ten report pieces from the strains make the report (memo 5, cond 6 with no sound).
        for _ in 0..10 {
            kill_mob(&mut t, STRAIN);
        }
        assert_eq!((t.count(1, REPORT_PIECE.0), t.count(1, TALIANUSS_REPORT)), (0, 1));
        assert_eq!(rows_of(&t), [rows::update(NAME, "cond", "6")]);

        // Sir Kiel takes the report and gives the mirror (memo 6, cond 7).
        assert!(t.quest_page(1, npcs.kiel).contains("30654-03.html"));
        assert_eq!(t.count(1, MIRROR_OF_ORPIC), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "7")
            ]
        );

        // Hangman trees summon the spirit of Sir Talianus (cond 8).
        for _ in 0..200 {
            kill_mob(&mut t, HANGMAN_TREE);
            if !t.npcs_of(SPIRIT_OF_SIR_TALIANUS).is_empty() {
                break;
            }
        }
        let talianus = t.npcs_of(SPIRIT_OF_SIR_TALIANUS)[0];
        assert_eq!(rows_of(&t).last(), Some(&rows::update(NAME, "cond", "8")));

        // The spirit takes the mirror and the report, gives the tear of confession (memo 7).
        assert!(t.quest_page(1, talianus).contains("30656-01.html"));
        assert_eq!(
            (
                t.count(1, MIRROR_OF_ORPIC),
                t.count(1, TALIANUSS_REPORT),
                t.count(1, TEAR_OF_CONFESSION)
            ),
            (0, 0, 1)
        );
        assert!(!t.in_world(talianus));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "9")
            ]
        );

        // Sir Kiel takes the tear of confession (memo 8, cond 10).
        assert!(t.quest_page(1, npcs.kiel).contains("30654-05.html"));
        assert_eq!(t.count(1, TEAR_OF_CONFESSION), 0);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "8"),
                rows::update(NAME, "cond", "10")
            ]
        );

        // Isael sends the knight to the Leto lizardmen (memo 9, cond 11).
        assert!(t.quest_page(1, npcs.isael).contains("30655-02.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "9"),
                rows::update(NAME, "cond", "11")
            ]
        );

        // Twenty militas articles from the lizardmen (cond 12 without sound).
        for _ in 0..20 {
            kill_mob(&mut t, LETO_LIZARDMAN);
        }
        assert_eq!(t.count(1, MILITAS_ARTICLE.0), MILITAS_ARTICLE.1);
        assert_eq!(rows_of(&t).last(), Some(&rows::update(NAME, "cond", "12")));

        // Isael takes the articles and gives the tear of loyalty (memo 10, cond 13).
        assert!(t.quest_page(1, npcs.isael).contains("30655-04.html"));
        assert_eq!(t.count(1, TEAR_OF_LOYALTY), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "10"),
                rows::update(NAME, "cond", "13")
            ]
        );

        // Dustin takes the tear: the page with the links, then the event (memo 11, cond 14).
        assert!(t.quest_page(1, npcs.dustin).contains("30116-01.html"));
        assert!(t.event(1, NAME, "30116-05.html").contains("30116-05.html"));
        assert_eq!(t.count(1, TEAR_OF_LOYALTY), 0);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "11"),
                rows::update(NAME, "cond", "14")
            ]
        );

        // Breka orc overlords drop the three parts of Athebaldt (cond 15 with the shin).
        for _ in 0..3 {
            kill_mob(&mut t, BREKA_ORC_OVERLORD);
        }
        assert_eq!(
            (
                t.count(1, ATHEBALDTS_SKULL),
                t.count(1, ATHEBALDTS_RIBS),
                t.count(1, ATHEBALDTS_SHIN)
            ),
            (1, 1, 1)
        );
        assert_eq!(rows_of(&t), [rows::update(NAME, "cond", "15")]);

        // Dustin takes the parts and gives the urn (memo 12, cond 16).
        assert!(t.quest_page(1, npcs.dustin).contains("30116-07.html"));
        assert_eq!(t.count(1, SAINTS_ASHES_URN), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "12"),
                rows::update(NAME, "cond", "16")
            ]
        );

        // Collin takes the urn and gives the letter (memo 13, cond 17).
        assert!(t.quest_page(1, npcs.collin).contains("30311-01.html"));
        assert_eq!(t.count(1, LETTER_OF_WINDAWOOD), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "13"),
                rows::update(NAME, "cond", "17")
            ]
        );

        // Dustin takes the letter and gives the letter of Dustin (memo 14, cond 18).
        assert!(t.quest_page(1, npcs.dustin).contains("30116-08.html"));
        assert_eq!(t.count(1, LETTER_OF_DUSTIN), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "14"),
                rows::update(NAME, "cond", "18")
            ]
        );

        // Hannavalt: the exp, the adena, the mark, `exitQuest(false, true)` and the social action.
        t.talk(1, npcs.hannavalt);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30109-05.html"));
        let names = names_to(&out, 1);
        assert!(names.contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_DUTY), 1);
        assert_eq!(t.count(1, LETTER_OF_DUSTIN), 0);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 45);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.hannavalt)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_spirit_killed_without_the_sword_gives_nothing() {
        let (mut t, npcs) = setup();
        enter_knight(&mut t, vec![]);
        t.quest_page(1, npcs.hannavalt);
        t.event(1, NAME, "quest_accept");
        t.quest_page(1, npcs.aron);
        t.take_writes();

        let mut spirit = None;
        for _ in 0..200 {
            kill_mob(&mut t, SKELETON_RAIDER);
            if let Some(s) = t.npcs_of(SPIRIT_OF_SIR_HEROD).first() {
                spirit = Some(*s);
                break;
            }
        }
        // The `flag` rows of the rolls are not checked here.
        t.take_writes();
        let spirit = spirit.expect("the spirit spawns from the skeletons");
        kill_npc(&mut t, spirit);
        assert_eq!(t.count(1, KNIGHTS_TEAR), 0);
        assert!(t.take_writes().is_empty());
    }
}
