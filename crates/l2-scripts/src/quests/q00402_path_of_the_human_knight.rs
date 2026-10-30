//! Quest 402: Path of the Human Knight (Sir Klaus Vasper and the six coin masters).
// Java: dist/game/data/scripts/quests/Q00402_PathOfTheHumanKnight/Q00402_PathOfTheHumanKnight.java
//
// Pattern: six coins, each earned at a different guild NPC: the NPC gives a badge (an event), the
// badge's monsters drop their item (a chance or an exact count), then the NPC takes the item and
// gives the coin. Sir Klaus gives the sword for three to five coins from his event pages, or
// for six from his talk (`docs/design/scripts.md`, "Quest patterns"). The quest never calls
// `setCond`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIGH_PRIEST_BIOTIN: i32 = 30031;
const LEVIAN: i32 = 30037;
const CAPTAIN_GILBERT: i32 = 30039;
const HIGH_PRIEST_RAYMOND: i32 = 30289;
const CAPTAIN_BATHIS: i32 = 30332;
const CAPTAIN_BEZIQUE: i32 = 30379;
const SIR_KLAUS_VASPER: i32 = 30417;
const SIR_ARON_TANFORD: i32 = 30653;

// Items
const SQUIRES_MARK: i32 = 1271;
const COIN_OF_LORDS1: i32 = 1162;
const COIN_OF_LORDS2: i32 = 1163;
const COIN_OF_LORDS3: i32 = 1164;
const COIN_OF_LORDS4: i32 = 1165;
const COIN_OF_LORDS5: i32 = 1166;
const COIN_OF_LORDS6: i32 = 1167;
const GLUDIO_GUARDS_1ST_BADGE: i32 = 1168;
const BUGBEAR_NECKLACE: i32 = 1169;
const EINHASADS_1ST_TEMPLE_BADGE: i32 = 1170;
const EINHASAD_CRUCIFIX: i32 = 1171;
const GLUDIO_GUARDS_2ND_BADGE: i32 = 1172;
const VENOMOUS_SPIDERS_LEG: i32 = 1173;
const EINHASADS_2ND_TEMPLE_BADGE: i32 = 1174;
const LIZARDMANS_TOTEM: i32 = 1175;
const GLUDIO_GUARDS_3RD_BADGE: i32 = 1176;
const GIANT_SPIDERS_HUSK: i32 = 1177;
const EINHASADS_3RD_TEMPLE_BADGE: i32 = 1178;
const SKULL_OF_SILENT_HORROR: i32 = 1179;

/// The six coins of the quest.
const COINS: [i32; 6] = [
    COIN_OF_LORDS1,
    COIN_OF_LORDS2,
    COIN_OF_LORDS3,
    COIN_OF_LORDS4,
    COIN_OF_LORDS5,
    COIN_OF_LORDS6,
];

// Reward
const SWORD_OF_RITUAL: i32 = 1161;

// Monsters
const LANGK_LIZARDMAN_WARRIOR: i32 = 20024;
const LANGK_LIZARDMAN_SCOUT: i32 = 20027;
const LANGK_LIZARDMAN: i32 = 20030;
const VENOMOUS_SPIDER: i32 = 20038;
const ARACHNID_TRACKER: i32 = 20043;
const ARACHNID_PREDATOR: i32 = 20050;
const GIANT_SPIDER: i32 = 20103;
const TALON_SPIDER: i32 = 20106;
const BLADE_SPIDER: i32 = 20108;
const SILENT_HORROR: i32 = 20404;
const BUGBEAR_RAIDER: i32 = 20775;

// Quest monster
const UNDEAD_PRIEST: i32 = 27024;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00402_PathOfTheHumanKnight`.
pub struct Q00402PathOfTheHumanKnight;

impl Script for Q00402PathOfTheHumanKnight {
    fn name(&self) -> &'static str {
        "Q00402_PathOfTheHumanKnight"
    }

    fn path(&self) -> &'static str {
        "quests/Q00402_PathOfTheHumanKnight"
    }

    fn id(&self) -> i32 {
        402
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIR_KLAUS_VASPER]);
        s.add_talk_id(&[
            SIR_KLAUS_VASPER,
            HIGH_PRIEST_BIOTIN,
            LEVIAN,
            HIGH_PRIEST_RAYMOND,
            CAPTAIN_GILBERT,
            CAPTAIN_BATHIS,
            CAPTAIN_BEZIQUE,
            SIR_ARON_TANFORD,
        ]);
        s.add_kill_id(&[
            LANGK_LIZARDMAN_WARRIOR,
            LANGK_LIZARDMAN_SCOUT,
            LANGK_LIZARDMAN,
            VENOMOUS_SPIDER,
            ARACHNID_TRACKER,
            ARACHNID_PREDATOR,
            GIANT_SPIDER,
            TALON_SPIDER,
            BLADE_SPIDER,
            SILENT_HORROR,
            BUGBEAR_RAIDER,
            UNDEAD_PRIEST,
        ]);
        s.register_quest_items(&[
            SQUIRES_MARK,
            COIN_OF_LORDS1,
            COIN_OF_LORDS2,
            COIN_OF_LORDS3,
            COIN_OF_LORDS4,
            COIN_OF_LORDS5,
            COIN_OF_LORDS6,
            GLUDIO_GUARDS_1ST_BADGE,
            BUGBEAR_NECKLACE,
            EINHASADS_1ST_TEMPLE_BADGE,
            EINHASAD_CRUCIFIX,
            GLUDIO_GUARDS_2ND_BADGE,
            VENOMOUS_SPIDERS_LEG,
            EINHASADS_2ND_TEMPLE_BADGE,
            LIZARDMANS_TOTEM,
            GLUDIO_GUARDS_3RD_BADGE,
            GIANT_SPIDERS_HUSK,
            EINHASADS_3RD_TEMPLE_BADGE,
            SKULL_OF_SILENT_HORROR,
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
        let coin_count = ctx.get_quest_items_count_of(player, &COINS);

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::FIGHTER {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, SWORD_OF_RITUAL) {
                                "30417-04.htm"
                            } else {
                                "30417-05.htm"
                            }
                        } else {
                            "30417-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::KNIGHT {
                        "30417-02a.htm"
                    } else {
                        "30417-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30417-08.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, SQUIRES_MARK, 1);
                htmltext = Some(event.to_owned());
            }
            "30289-02.html" | "30417-06.html" | "30417-07.htm" | "30417-15.html" => {
                htmltext = Some(event.to_owned());
            }
            "30417-13.html" => {
                if ctx.has_quest_items(player, SQUIRES_MARK) && coin_count == 3 {
                    reward_sword(
                        ctx,
                        player,
                        qs,
                        81_900,
                        [(160_267, 11_576), (228_064, 14_925), (295_862, 18_274)],
                    );
                    htmltext = Some(event.to_owned());
                }
            }
            "30417-14.html" => {
                if ctx.has_quest_items(player, SQUIRES_MARK) && coin_count > 3 && coin_count < 6 {
                    reward_sword(
                        ctx,
                        player,
                        qs,
                        81_900,
                        [(160_267, 11_576), (228_064, 14_925), (295_862, 18_274)],
                    );
                    htmltext = Some(event.to_owned());
                }
            }
            "30031-02.html" => {
                ctx.give_items(player, EINHASADS_3RD_TEMPLE_BADGE, 1);
                htmltext = Some(event.to_owned());
            }
            "30037-02.html" => {
                ctx.give_items(player, EINHASADS_2ND_TEMPLE_BADGE, 1);
                htmltext = Some(event.to_owned());
            }
            "30289-03.html" => {
                ctx.give_items(player, EINHASADS_1ST_TEMPLE_BADGE, 1);
                htmltext = Some(event.to_owned());
            }
            "30039-02.html" => {
                ctx.give_items(player, GLUDIO_GUARDS_3RD_BADGE, 1);
                htmltext = Some(event.to_owned());
            }
            "30379-02.html" => {
                ctx.give_items(player, GLUDIO_GUARDS_2ND_BADGE, 1);
                htmltext = Some(event.to_owned());
            }
            "30332-02.html" => {
                ctx.give_items(player, GLUDIO_GUARDS_1ST_BADGE, 1);
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
            LANGK_LIZARDMAN_WARRIOR | LANGK_LIZARDMAN_SCOUT | LANGK_LIZARDMAN => {
                drop_item(
                    ctx,
                    killer,
                    EINHASADS_2ND_TEMPLE_BADGE,
                    LIZARDMANS_TOTEM,
                    20,
                    Some(5),
                );
            }
            VENOMOUS_SPIDER | ARACHNID_TRACKER | ARACHNID_PREDATOR => {
                drop_item(
                    ctx,
                    killer,
                    GLUDIO_GUARDS_2ND_BADGE,
                    VENOMOUS_SPIDERS_LEG,
                    20,
                    None,
                );
            }
            GIANT_SPIDER | TALON_SPIDER | BLADE_SPIDER => {
                drop_item(
                    ctx,
                    killer,
                    GLUDIO_GUARDS_3RD_BADGE,
                    GIANT_SPIDERS_HUSK,
                    20,
                    Some(4),
                );
            }
            SILENT_HORROR => {
                drop_item(
                    ctx,
                    killer,
                    EINHASADS_3RD_TEMPLE_BADGE,
                    SKULL_OF_SILENT_HORROR,
                    10,
                    Some(4),
                );
            }
            BUGBEAR_RAIDER => {
                drop_item(ctx, killer, GLUDIO_GUARDS_1ST_BADGE, BUGBEAR_NECKLACE, 10, None);
            }
            UNDEAD_PRIEST => {
                drop_item(
                    ctx,
                    killer,
                    EINHASADS_1ST_TEMPLE_BADGE,
                    EINHASAD_CRUCIFIX,
                    12,
                    Some(5),
                );
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == SIR_KLAUS_VASPER {
                htmltext = "30417-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                SIR_KLAUS_VASPER => {
                    let coin_count = ctx.get_quest_items_count_of(player, &COINS);
                    if ctx.has_quest_items(player, SQUIRES_MARK) {
                        if coin_count < 3 {
                            htmltext = "30417-09.html".to_owned();
                        } else if coin_count == 3 {
                            htmltext = "30417-10.html".to_owned();
                        } else if coin_count > 3 && coin_count < 6 {
                            htmltext = "30417-11.html".to_owned();
                        } else {
                            ctx.give_adena(player, 163_800, true);
                            ctx.give_items(player, SWORD_OF_RITUAL, 1);
                            let level = ctx.player_level(player);
                            if level >= 20 {
                                ctx.add_exp_and_sp(player, 320_534, 23_152);
                            } else if level == 19 {
                                ctx.add_exp_and_sp(player, 456_128, 29_850);
                            } else {
                                ctx.add_exp_and_sp(player, 591_724, 36_542);
                            }
                            qs.exit_quest(ctx, false, true);
                            ctx.world_mut().send_social_action(player, 3);
                            htmltext = "30417-12.html".to_owned();
                        }
                    }
                }
                HIGH_PRIEST_BIOTIN => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[EINHASADS_3RD_TEMPLE_BADGE, COIN_OF_LORDS6],
                        )
                    {
                        htmltext = "30031-01.html".to_owned();
                    } else if ctx.has_quest_items(player, EINHASADS_3RD_TEMPLE_BADGE) {
                        if ctx.get_quest_items_count(player, SKULL_OF_SILENT_HORROR) < 10 {
                            htmltext = "30031-03.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS6, 1);
                            ctx.take_items(player, EINHASADS_3RD_TEMPLE_BADGE, 1);
                            ctx.take_items(player, SKULL_OF_SILENT_HORROR, -1);
                            htmltext = "30031-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS6) {
                        htmltext = "30031-05.html".to_owned();
                    }
                }
                LEVIAN => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[EINHASADS_2ND_TEMPLE_BADGE, COIN_OF_LORDS4],
                        )
                    {
                        htmltext = "30037-01.html".to_owned();
                    } else if ctx.has_quest_items(player, EINHASADS_2ND_TEMPLE_BADGE) {
                        if ctx.get_quest_items_count(player, LIZARDMANS_TOTEM) < 20 {
                            htmltext = "30037-03.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS4, 1);
                            ctx.take_items(player, EINHASADS_2ND_TEMPLE_BADGE, 1);
                            ctx.take_items(player, LIZARDMANS_TOTEM, -1);
                            htmltext = "30037-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS4) {
                        htmltext = "30037-05.html".to_owned();
                    }
                }
                HIGH_PRIEST_RAYMOND => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[EINHASADS_1ST_TEMPLE_BADGE, COIN_OF_LORDS2],
                        )
                    {
                        htmltext = "30289-01.html".to_owned();
                    } else if ctx.has_quest_items(player, EINHASADS_1ST_TEMPLE_BADGE) {
                        if ctx.get_quest_items_count(player, EINHASAD_CRUCIFIX) < 12 {
                            htmltext = "30289-04.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS2, 1);
                            ctx.take_items(player, EINHASADS_1ST_TEMPLE_BADGE, 1);
                            ctx.take_items(player, EINHASAD_CRUCIFIX, -1);
                            htmltext = "30289-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS2) {
                        htmltext = "30289-06.html".to_owned();
                    }
                }
                CAPTAIN_GILBERT => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx
                            .has_at_least_one_quest_item(player, &[GLUDIO_GUARDS_3RD_BADGE, COIN_OF_LORDS5])
                    {
                        htmltext = "30039-01.html".to_owned();
                    } else if ctx.has_quest_items(player, GLUDIO_GUARDS_3RD_BADGE) {
                        if ctx.get_quest_items_count(player, GIANT_SPIDERS_HUSK) < 20 {
                            htmltext = "30039-03.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS5, 1);
                            ctx.take_items(player, GLUDIO_GUARDS_3RD_BADGE, 1);
                            ctx.take_items(player, GIANT_SPIDERS_HUSK, -1);
                            htmltext = "30039-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS5) {
                        htmltext = "30039-05.html".to_owned();
                    }
                }
                CAPTAIN_BEZIQUE => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx
                            .has_at_least_one_quest_item(player, &[GLUDIO_GUARDS_2ND_BADGE, COIN_OF_LORDS3])
                    {
                        htmltext = "30379-01.html".to_owned();
                    } else if ctx.has_quest_items(player, GLUDIO_GUARDS_2ND_BADGE) {
                        if ctx.get_quest_items_count(player, VENOMOUS_SPIDERS_LEG) < 20 {
                            htmltext = "30379-03.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS3, 1);
                            ctx.take_items(player, GLUDIO_GUARDS_2ND_BADGE, 1);
                            ctx.take_items(player, VENOMOUS_SPIDERS_LEG, -1);
                            htmltext = "30379-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS3) {
                        htmltext = "30379-05.html".to_owned();
                    }
                }
                CAPTAIN_BATHIS => {
                    if ctx.has_quest_items(player, SQUIRES_MARK)
                        && !ctx
                            .has_at_least_one_quest_item(player, &[GLUDIO_GUARDS_1ST_BADGE, COIN_OF_LORDS1])
                    {
                        htmltext = "30332-01.html".to_owned();
                    } else if ctx.has_quest_items(player, GLUDIO_GUARDS_1ST_BADGE) {
                        if ctx.get_quest_items_count(player, BUGBEAR_NECKLACE) < 10 {
                            htmltext = "30332-03.html".to_owned();
                        } else {
                            ctx.give_items(player, COIN_OF_LORDS1, 1);
                            ctx.take_items(player, GLUDIO_GUARDS_1ST_BADGE, 1);
                            ctx.take_items(player, BUGBEAR_NECKLACE, -1);
                            htmltext = "30332-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, COIN_OF_LORDS1) {
                        htmltext = "30332-05.html".to_owned();
                    }
                }
                SIR_ARON_TANFORD => {
                    if ctx.has_quest_items(player, SQUIRES_MARK) {
                        htmltext = "30653-01.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `drop`-style kill branch: the monster drops `item` for a killer who holds `badge`, up to
/// `limit`; `roll` is `getRandom(10) < roll` (`None` for a sure drop). The sound is the middle
/// one at the limit.
fn drop_item(
    ctx: &mut ScriptCtx<'_>,
    killer: ObjectId,
    badge: i32,
    item: i32,
    limit: i64,
    roll: Option<i32>,
) {
    if !ctx.has_quest_items(killer, badge) || ctx.get_quest_items_count(killer, item) >= limit {
        return;
    }
    if let Some(chance) = roll
        && ctx.get_random(10) >= chance
    {
        return;
    }
    ctx.give_items(killer, item, 1);
    if ctx.get_quest_items_count(killer, item) == limit {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

/// Java's reward of the sword from Klaus' event pages (the same for every level but the exp).
fn reward_sword(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    qs: Qs,
    adena: i64,
    exp_by_level: [(i64, i32); 3],
) {
    ctx.give_adena(player, adena, true);
    ctx.give_items(player, SWORD_OF_RITUAL, 1);
    let level = ctx.player_level(player);
    let (exp, sp) = if level >= 20 {
        exp_by_level[0]
    } else if level == 19 {
        exp_by_level[1]
    } else {
        exp_by_level[2]
    };
    ctx.add_exp_and_sp(player, exp, sp);
    qs.exit_quest(ctx, false, true);
    ctx.world_mut().send_social_action(player, 3);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00402_PathOfTheHumanKnight";

    const NPCS: &str = r#"
        <npc id="30417" type="Folk" level="70" name="Sir Klaus Vasper"/>
        <npc id="30031" type="Folk" level="70" name="High Priest Biotin"/>
        <npc id="30037" type="Folk" level="70" name="Levian"/>
        <npc id="30039" type="Folk" level="70" name="Captain Gilbert"/>
        <npc id="30289" type="Folk" level="70" name="High Priest Raymond"/>
        <npc id="30332" type="Folk" level="70" name="Captain Bathis"/>
        <npc id="30379" type="Folk" level="70" name="Captain Bezique"/>
        <npc id="20030" type="Monster" level="20" name="Langk Lizardman"/>
        <npc id="20038" type="Monster" level="20" name="Venomous Spider"/>
        <npc id="20103" type="Monster" level="20" name="Giant Spider"/>
        <npc id="20404" type="Monster" level="20" name="Silent Horror"/>
        <npc id="20775" type="Monster" level="20" name="Bugbear Raider"/>
        <npc id="27024" type="Monster" level="20" name="Undead Priest"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1161" type="Weapon" name="Sword of Ritual"/>
        <item id="1162" type="EtcItem" name="Coin of Lords 1"><set name="is_questitem" val="true"/></item>
        <item id="1163" type="EtcItem" name="Coin of Lords 2"><set name="is_questitem" val="true"/></item>
        <item id="1164" type="EtcItem" name="Coin of Lords 3"><set name="is_questitem" val="true"/></item>
        <item id="1165" type="EtcItem" name="Coin of Lords 4"><set name="is_questitem" val="true"/></item>
        <item id="1166" type="EtcItem" name="Coin of Lords 5"><set name="is_questitem" val="true"/></item>
        <item id="1167" type="EtcItem" name="Coin of Lords 6"><set name="is_questitem" val="true"/></item>
        <item id="1168" type="EtcItem" name="Gludio Guard's 1st Badge"><set name="is_questitem" val="true"/></item>
        <item id="1169" type="EtcItem" name="Bugbear Necklace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1170" type="EtcItem" name="Einhasad's 1st Temple Badge"><set name="is_questitem" val="true"/></item>
        <item id="1171" type="EtcItem" name="Einhasad Crucifix"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1172" type="EtcItem" name="Gludio Guard's 2nd Badge"><set name="is_questitem" val="true"/></item>
        <item id="1173" type="EtcItem" name="Venomous Spider's Leg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1174" type="EtcItem" name="Einhasad's 2nd Temple Badge"><set name="is_questitem" val="true"/></item>
        <item id="1175" type="EtcItem" name="Lizardman's Totem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1176" type="EtcItem" name="Gludio Guard's 3rd Badge"><set name="is_questitem" val="true"/></item>
        <item id="1177" type="EtcItem" name="Giant Spider's Husk"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1178" type="EtcItem" name="Einhasad's 3rd Temple Badge"><set name="is_questitem" val="true"/></item>
        <item id="1179" type="EtcItem" name="Skull of Silent Horror"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1271" type="EtcItem" name="Squire's Mark"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        klaus: Oid,
        biotin: Oid,
        levian: Oid,
        gilbert: Oid,
        raymond: Oid,
        bathis: Oid,
        bezique: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00402PathOfTheHumanKnight)
            .page("30417-01.htm", &["ACCEPT"])
            .page("30417-05.htm", &["30417-06.html"])
            .page("30417-06.html", &["30417-07.htm"])
            .page("30417-07.htm", &["30417-08.htm"])
            .page("30417-10.html", &["30417-15.html", "30417-13.html"])
            .page("30417-11.html", &["30417-15.html", "30417-14.html"])
            .page("30031-01.html", &["30031-02.html"])
            .page("30037-01.html", &["30037-02.html"])
            .page("30039-01.html", &["30039-02.html"])
            .page("30289-01.html", &["30289-02.html"])
            .page("30289-02.html", &["30289-03.html"])
            .page("30332-01.html", &["30332-02.html"])
            .page("30379-01.html", &["30379-02.html"])
            .plain(&[
                "30417-02.htm",
                "30417-02a.htm",
                "30417-03.htm",
                "30417-04.htm",
                "30417-08.htm",
                "30417-09.html",
                "30417-12.html",
                "30417-13.html",
                "30417-14.html",
                "30417-15.html",
                "30031-02.html",
                "30031-03.html",
                "30031-04.html",
                "30031-05.html",
                "30037-02.html",
                "30037-03.html",
                "30037-04.html",
                "30037-05.html",
                "30039-02.html",
                "30039-03.html",
                "30039-04.html",
                "30039-05.html",
                "30289-03.html",
                "30289-04.html",
                "30289-05.html",
                "30289-06.html",
                "30332-02.html",
                "30332-03.html",
                "30332-04.html",
                "30332-05.html",
                "30379-02.html",
                "30379-03.html",
                "30379-04.html",
                "30379-05.html",
                "30653-01.html",
            ])
            .npcs(&[
                SIR_KLAUS_VASPER,
                HIGH_PRIEST_BIOTIN,
                LEVIAN,
                CAPTAIN_GILBERT,
                HIGH_PRIEST_RAYMOND,
                CAPTAIN_BATHIS,
                CAPTAIN_BEZIQUE,
            ])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00402PathOfTheHumanKnight);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            klaus: t.spawn(SIR_KLAUS_VASPER, 20),
            biotin: t.spawn(HIGH_PRIEST_BIOTIN, 40),
            levian: t.spawn(LEVIAN, 60),
            gilbert: t.spawn(CAPTAIN_GILBERT, 80),
            raymond: t.spawn(HIGH_PRIEST_RAYMOND, 100),
            bathis: t.spawn(CAPTAIN_BATHIS, 120),
            bezique: t.spawn(CAPTAIN_BEZIQUE, 140),
        };
        (t, npcs)
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    /// Takes the knight's squire mark from Klaus (the four pages of his accept).
    fn take_the_mark(t: &mut ScriptTest, npcs: &Npcs) {
        assert!(t.quest_page(1, npcs.klaus).contains("30417-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30417-05.htm"));
        assert!(t.event(1, NAME, "30417-06.html").contains("30417-06.html"));
        assert!(t.event(1, NAME, "30417-07.htm").contains("30417-07.htm"));
        assert!(t.event(1, NAME, "30417-08.htm").contains("30417-08.htm"));
    }

    /// One coin: the NPC's badge events (`path`), the monsters' drops, then the NPC's coin page.
    fn coin(
        t: &mut ScriptTest,
        npc: Oid,
        entry: &str,
        path: &[&str],
        mob: i32,
        item: i32,
        count: i64,
        coin_page: &str,
    ) {
        assert!(t.quest_page(1, npc).contains(entry));
        for event in path {
            assert!(t.event(1, NAME, event).contains(event));
        }
        hunt(t, mob, item, count);
        assert!(t.quest_page(1, npc).contains(coin_page));
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_sword() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.klaus).contains("30417-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30417-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::KNIGHT, vec![]);
        t.quest_page(1, npcs.klaus);
        assert!(t.event(1, NAME, "ACCEPT").contains("30417-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::MAGE, vec![]);
        t.quest_page(1, npcs.klaus);
        assert!(t.event(1, NAME, "ACCEPT").contains("30417-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(5000, 1161, 1)],
        );
        t.quest_page(1, npcs.klaus);
        assert!(t.event(1, NAME, "ACCEPT").contains("30417-04.htm"));
    }

    #[test]
    fn six_coins_from_the_six_masters_then_the_sword() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        take_the_mark(&mut t, &npcs);
        assert_eq!(t.count(1, SQUIRES_MARK), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Klaus with no coin yet.
        assert!(t.quest_page(1, npcs.klaus).contains("30417-09.html"));

        coin(
            &mut t,
            npcs.biotin,
            "30031-01.html",
            &["30031-02.html"],
            20404,
            SKULL_OF_SILENT_HORROR,
            10,
            "30031-04.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS6), 1);
        coin(
            &mut t,
            npcs.levian,
            "30037-01.html",
            &["30037-02.html"],
            20030,
            LIZARDMANS_TOTEM,
            20,
            "30037-04.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS4), 1);
        coin(
            &mut t,
            npcs.raymond,
            "30289-01.html",
            &["30289-02.html", "30289-03.html"],
            27024,
            EINHASAD_CRUCIFIX,
            12,
            "30289-05.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS2), 1);
        coin(
            &mut t,
            npcs.gilbert,
            "30039-01.html",
            &["30039-02.html"],
            20103,
            GIANT_SPIDERS_HUSK,
            20,
            "30039-04.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS5), 1);
        coin(
            &mut t,
            npcs.bezique,
            "30379-01.html",
            &["30379-02.html"],
            20038,
            VENOMOUS_SPIDERS_LEG,
            20,
            "30379-04.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS3), 1);
        coin(
            &mut t,
            npcs.bathis,
            "30332-01.html",
            &["30332-02.html"],
            20775,
            BUGBEAR_NECKLACE,
            10,
            "30332-04.html",
        );
        assert_eq!(t.count(1, COIN_OF_LORDS1), 1);
        assert!(t.take_writes().is_empty());

        // Klaus with six coins: the sword from the talk.
        assert!(t.quest_page(1, npcs.klaus).contains("30417-12.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, SWORD_OF_RITUAL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.klaus).contains("30417-01.htm"));
    }

    #[test]
    fn three_coins_give_the_sword_from_the_event_page() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        take_the_mark(&mut t, &npcs);
        t.take_writes();

        coin(
            &mut t,
            npcs.bathis,
            "30332-01.html",
            &["30332-02.html"],
            20775,
            BUGBEAR_NECKLACE,
            10,
            "30332-04.html",
        );
        coin(
            &mut t,
            npcs.raymond,
            "30289-01.html",
            &["30289-02.html", "30289-03.html"],
            27024,
            EINHASAD_CRUCIFIX,
            12,
            "30289-05.html",
        );
        coin(
            &mut t,
            npcs.bezique,
            "30379-01.html",
            &["30379-02.html"],
            20038,
            VENOMOUS_SPIDERS_LEG,
            20,
            "30379-04.html",
        );
        assert_eq!(
            t.count(1, COIN_OF_LORDS1) + t.count(1, COIN_OF_LORDS2) + t.count(1, COIN_OF_LORDS3),
            3
        );

        // Three coins: Klaus' page 10 links the reward page 13.
        assert!(t.quest_page(1, npcs.klaus).contains("30417-10.html"));
        assert!(t.event(1, NAME, "30417-13.html").contains("30417-13.html"));
        assert_eq!(t.count(1, SWORD_OF_RITUAL), 1);
        assert_eq!(t.count(1, 57), 81_900);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
