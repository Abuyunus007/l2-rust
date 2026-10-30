//! Quest 417: Path of the Scavenger (Pipi, the dwarven fighter's way to the scavenger class).
// Java: dist/game/data/scripts/quests/Q00417_PathOfTheScavenger/Q00417_PathOfTheScavenger.java
//
// Pattern: a class-change path quest with a long NPC chain; spoiled kills of the first attacker
// (script value 2) drop the quest items, a bear spawns a quest bear (`FLAG` counts up to it).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const WAREHOUSE_KEEPER_RAUT: i32 = 30316;
const TRADER_SHARI: i32 = 30517;
const TRADER_MION: i32 = 30519;
const COLLECTOR_PIPI: i32 = 30524;
const HEAD_BLACKSMITH_BRONK: i32 = 30525;
const PRIEST_OF_THE_EARTH_ZIMENF: i32 = 30538;
const MASTER_TOMA: i32 = 30556;
const TORAI: i32 = 30557;
const WAREHOUSE_CHIEF_YASENI: i32 = 31958;

// Items
const PIPPIS_LETTER_OF_RECOMMENDATION: i32 = 1643;
const ROUTS_TELEPORT_SCROLL: i32 = 1644;
const SUCCUBUS_UNDIES: i32 = 1645;
const MIONS_LETTER: i32 = 1646;
const BRONKS_INGOT: i32 = 1647;
const SHARIS_AXE: i32 = 1648;
const ZIMENFS_POTION: i32 = 1649;
const BRONKS_PAY: i32 = 1650;
const SHARIS_PAY: i32 = 1651;
const ZIMENFS_PAY: i32 = 1652;
const BEAR_PICTURE: i32 = 1653;
const TARANTULA_PICTURE: i32 = 1654;
const HONEY_JAR: i32 = 1655;
const BEAD: i32 = 1656;
const BEAD_PARCEL: i32 = 1657;
const BEAD_PARCEL2: i32 = 8543;

// Reward
const RING_OF_RAVEN: i32 = 1642;

// Monster
const HUNTER_TARANTULA: i32 = 20403;
const PLUNDER_TARANTULA: i32 = 20508;
const HUNTER_BEAR: i32 = 20777;

// Quest Monster
const HONEY_BEAR: i32 = 27058;

// Skill
const SPOIL: i32 = 254;

// Misc
const MIN_LEVEL: i32 = 18;
const FIRST_ATTACKER: &str = "FIRST_ATTACKER";
const FLAG: &str = "FLAG";

/// Java `Q00417_PathOfTheScavenger`.
pub struct Q00417PathOfTheScavenger;

/// The three helpers' tasks Mion hands out: Zimenf's potion, Shari's axe or Bronk's ingot.
fn give_random_errand(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<String> {
    match ctx.get_random(3) {
        0 => {
            ctx.give_items(player, ZIMENFS_POTION, 1);
            Some("30519-02.html".to_owned())
        }
        1 => {
            ctx.give_items(player, SHARIS_AXE, 1);
            Some("30519-03.html".to_owned())
        }
        2 => {
            ctx.give_items(player, BRONKS_INGOT, 1);
            Some("30519-04.html".to_owned())
        }
        _ => None,
    }
}

/// Java `npc.getVariables().getInt(FIRST_ATTACKER)` (0 without variables).
fn first_attacker_of(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> i32 {
    ctx.world()
        .npc_variables(npc.object_id)
        .map_or(0, |v| v.get_int(FIRST_ATTACKER, 0))
}

/// Java `giveAdena` + `giveItems(RING_OF_RAVEN)` + `addExpAndSp` by level + `exitQuest(false,
/// true)` + the bow: the two endings differ in the numbers only.
fn finish(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    qs: l2_world::script::Qs,
    adena: i64,
    (high, mid, low): ((i64, i32), (i64, i32), (i64, i32)),
) {
    ctx.give_adena(player, adena, true);
    ctx.give_items(player, RING_OF_RAVEN, 1);
    let level = ctx.player_level(player);
    let (exp, sp) = if level >= 20 {
        high
    } else if level == 19 {
        mid
    } else {
        low
    };
    ctx.add_exp_and_sp(player, exp, sp);
    qs.exit_quest(ctx, false, true);
    ctx.world_mut().send_social_action(player, 3);
}

impl Script for Q00417PathOfTheScavenger {
    fn name(&self) -> &'static str {
        "Q00417_PathOfTheScavenger"
    }

    fn path(&self) -> &'static str {
        "quests/Q00417_PathOfTheScavenger"
    }

    fn id(&self) -> i32 {
        417
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[COLLECTOR_PIPI]);
        s.add_talk_id(&[
            COLLECTOR_PIPI,
            WAREHOUSE_KEEPER_RAUT,
            TRADER_MION,
            TRADER_SHARI,
            HEAD_BLACKSMITH_BRONK,
            PRIEST_OF_THE_EARTH_ZIMENF,
            MASTER_TOMA,
            TORAI,
            WAREHOUSE_CHIEF_YASENI,
        ]);
        s.add_attack_id(&[HUNTER_TARANTULA, PLUNDER_TARANTULA, HUNTER_BEAR, HONEY_BEAR]);
        s.add_kill_id(&[HUNTER_TARANTULA, PLUNDER_TARANTULA, HUNTER_BEAR, HONEY_BEAR]);
        s.register_quest_items(&[
            PIPPIS_LETTER_OF_RECOMMENDATION,
            ROUTS_TELEPORT_SCROLL,
            SUCCUBUS_UNDIES,
            MIONS_LETTER,
            BRONKS_INGOT,
            SHARIS_AXE,
            ZIMENFS_POTION,
            BRONKS_PAY,
            SHARIS_PAY,
            ZIMENFS_PAY,
            BEAR_PICTURE,
            TARANTULA_PICTURE,
            HONEY_JAR,
            BEAD,
            BEAD_PARCEL,
            BEAD_PARCEL2,
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
        match event {
            "ACCEPT" => {
                let class = ctx.player_class_id(player);
                if class == player_class::DWARVEN_FIGHTER {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, RING_OF_RAVEN) {
                            Some("30524-04.htm".to_owned())
                        } else {
                            qs.start_quest(ctx);
                            qs.set_memo_state_ex(ctx, 1, 0);
                            ctx.give_items(player, PIPPIS_LETTER_OF_RECOMMENDATION, 1);
                            Some("30524-05.htm".to_owned())
                        }
                    } else {
                        Some("30524-02.htm".to_owned())
                    }
                } else if class == player_class::SCAVENGER {
                    Some("30524-02a.htm".to_owned())
                } else {
                    Some("30524-08.htm".to_owned())
                }
            }
            "30524-03.html" | "30557-02.html" | "30519-06.html" => Some(event.to_owned()),
            "reply_1" => {
                if ctx.has_quest_items(player, PIPPIS_LETTER_OF_RECOMMENDATION) {
                    ctx.take_items(player, PIPPIS_LETTER_OF_RECOMMENDATION, 1);
                    return give_random_errand(ctx, player);
                }
                None
            }
            "30519-07.html" => {
                let ex = qs.get_memo_state_ex(ctx, 1);
                qs.set_memo_state_ex(ctx, 1, ex + 1);
                Some(event.to_owned())
            }
            "reply_2" => match ctx.get_random(2) {
                0 => Some("30519-06.html".to_owned()),
                1 => Some("30519-11.html".to_owned()),
                _ => None,
            },
            "reply_3" => {
                let ex = qs.get_memo_state_ex(ctx, 1);
                if ex % 10 < 2 {
                    qs.set_memo_state_ex(ctx, 1, ex + 1);
                    Some("30519-07.html".to_owned())
                } else if ex % 10 == 2 && qs.is_memo_state(ctx, 0) {
                    Some("30519-07.html".to_owned())
                } else if ex % 10 == 2 && qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state_ex(ctx, 1, ex + 1);
                    Some("30519-09.html".to_owned())
                } else if ex % 10 >= 3 && qs.is_memo_state(ctx, 1) {
                    ctx.give_items(player, MIONS_LETTER, 1);
                    ctx.take_items(player, SHARIS_AXE, 1);
                    ctx.take_items(player, ZIMENFS_POTION, 1);
                    ctx.take_items(player, BRONKS_INGOT, 1);
                    qs.set_cond(ctx, 4, true);
                    Some("30519-10.html".to_owned())
                } else {
                    None
                }
            }
            "reply_4" => {
                ctx.take_items(player, ZIMENFS_PAY, 1);
                ctx.take_items(player, SHARIS_PAY, 1);
                ctx.take_items(player, BRONKS_PAY, 1);
                give_random_errand(ctx, player)
            }
            "30556-05b.html" => {
                if ctx.has_quest_items(player, TARANTULA_PICTURE)
                    && ctx.get_quest_items_count(player, BEAD) >= 20
                {
                    ctx.take_items(player, TARANTULA_PICTURE, 1);
                    ctx.take_items(player, BEAD, -1);
                    ctx.give_items(player, BEAD_PARCEL, 1);
                    qs.set_cond(ctx, 9, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30556-06b.html" => {
                if ctx.has_quest_items(player, TARANTULA_PICTURE)
                    && ctx.get_quest_items_count(player, BEAD) >= 20
                {
                    ctx.take_items(player, TARANTULA_PICTURE, 1);
                    ctx.take_items(player, BEAD, -1);
                    ctx.give_items(player, BEAD_PARCEL2, 1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 12, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30316-02.html" => {
                if ctx.has_quest_items(player, BEAD_PARCEL) {
                    ctx.take_items(player, BEAD_PARCEL, 1);
                    ctx.give_items(player, ROUTS_TELEPORT_SCROLL, 1);
                    qs.set_cond(ctx, 10, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30316-03.html" => {
                if ctx.has_quest_items(player, BEAD_PARCEL) {
                    ctx.give_items(player, ROUTS_TELEPORT_SCROLL, 1);
                    ctx.take_items(player, BEAD_PARCEL, 1);
                    qs.set_cond(ctx, 10, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30557-03.html" => {
                if ctx.has_quest_items(player, ROUTS_TELEPORT_SCROLL) {
                    ctx.take_items(player, ROUTS_TELEPORT_SCROLL, 1);
                    ctx.give_items(player, SUCCUBUS_UNDIES, 1);
                    qs.set_cond(ctx, 11, true);
                    // Java calls `npc.deleteMe()` unchecked; the event always comes from Torai.
                    if let Some(npc) = npc {
                        ctx.delete_npc(npc);
                    }
                    return Some(event.to_owned());
                }
                None
            }
            "31958-02.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, BEAD_PARCEL2) {
                    finish(
                        ctx,
                        player,
                        qs,
                        163_800,
                        ((320_534, 35_412), (456_128, 42_110), (591_724, 48_808)),
                    );
                    return Some(event.to_owned());
                }
                None
            }
            _ => None,
        }
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
        if !qs.is_some_and(|qs| qs.is_started(ctx)) {
            return;
        }
        match npc.id {
            HUNTER_BEAR => match ctx.get_script_value(npc) {
                0 => {
                    ctx.set_script_value(npc, 1);
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int(FIRST_ATTACKER, attacker.0);
                }
                1 => {
                    if first_attacker_of(ctx, npc) != attacker.0 {
                        ctx.set_script_value(npc, 2);
                    }
                }
                _ => {}
            },
            HUNTER_TARANTULA | PLUNDER_TARANTULA | HONEY_BEAR => {
                if ctx.is_script_value(npc, 0) {
                    ctx.set_script_value(npc, 1);
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int(FIRST_ATTACKER, attacker.0);
                }

                // TODO(java): Java says this should be the skill parameter and not the last skill cast.
                if ctx
                    .world()
                    .script_last_skill_cast(attacker)
                    .is_some_and(|s| s.id == SPOIL)
                {
                    ctx.set_script_value(npc, 2);
                    // Reset last skill cast.
                    ctx.world_mut().script_clear_last_skill_cast(attacker);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && ctx.is_attackable(npc))
        {
            return;
        }
        let first_attacker = killer.0 == first_attacker_of(ctx, npc);
        match npc.id {
            HUNTER_BEAR => {
                if ctx.is_script_value(npc, 1)
                    && first_attacker
                    && ctx.has_quest_items(killer, BEAR_PICTURE)
                    && ctx.get_quest_items_count(killer, HONEY_JAR) < 5
                {
                    let flag = qs.get_int(ctx, FLAG);
                    if flag > 0 && ctx.get_random(100) < 20 * flag {
                        // `addSpawn(HONEY_BEAR, npc, true, 0, true)`: the summon-spawn flag only
                        // plays the spawn effect.
                        ctx.add_spawn_at(HONEY_BEAR, npc.object_id, true, 0);
                        qs.set_int(ctx, FLAG, 0);
                    } else {
                        qs.set_int(ctx, FLAG, flag + 1);
                    }
                }
            }
            HONEY_BEAR => {
                if ctx.is_script_value(npc, 2)
                    && first_attacker
                    && ctx.world().is_spoiled(npc.object_id)
                    && ctx.has_quest_items(killer, BEAR_PICTURE)
                    && ctx.give_item_randomly(killer, Some(npc), HONEY_JAR, 1, 1, 5, 1.0, true)
                {
                    qs.set_cond(ctx, 6, false);
                }
            }
            HUNTER_TARANTULA | PLUNDER_TARANTULA => {
                if ctx.is_script_value(npc, 2)
                    && first_attacker
                    && ctx.world().is_spoiled(npc.object_id)
                    && ctx.has_quest_items(killer, TARANTULA_PICTURE)
                    && ctx.give_item_randomly(killer, Some(npc), BEAD, 1, 1, 20, 1.0, true)
                {
                    qs.set_cond(ctx, 8, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED | COMPLETED => {
                if npc.id == COLLECTOR_PIPI {
                    htmltext = "30524-01.htm".to_owned();
                }
            }
            STARTED => match npc.id {
                COLLECTOR_PIPI => {
                    htmltext = if ctx.has_quest_items(player, PIPPIS_LETTER_OF_RECOMMENDATION) {
                        "30524-06.html"
                    } else {
                        "30524-07.html"
                    }
                    .to_owned();
                }
                TRADER_MION => {
                    let ex = qs.get_memo_state_ex(ctx, 1);
                    if ctx.has_quest_items(player, PIPPIS_LETTER_OF_RECOMMENDATION) {
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30519-01.html".to_owned();
                    } else if ctx.get_quest_items_count(player, SHARIS_AXE)
                        + ctx.get_quest_items_count(player, BRONKS_INGOT)
                        + ctx.get_quest_items_count(player, ZIMENFS_POTION)
                        == 1
                    {
                        if ex % 10 == 0 {
                            htmltext = "30519-05.html".to_owned();
                        } else if ex % 10 > 0 {
                            htmltext = "30519-08.html".to_owned();
                        }
                    } else if ctx.get_quest_items_count(player, SHARIS_PAY)
                        + ctx.get_quest_items_count(player, BRONKS_PAY)
                        + ctx.get_quest_items_count(player, ZIMENFS_PAY)
                        == 1
                    {
                        if ex < 50 {
                            htmltext = "30519-12.html".to_owned();
                        } else {
                            ctx.give_items(player, MIONS_LETTER, 1);
                            ctx.take_items(player, SHARIS_PAY, 1);
                            ctx.take_items(player, ZIMENFS_PAY, 1);
                            ctx.take_items(player, BRONKS_PAY, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30519-15.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, MIONS_LETTER) {
                        htmltext = "30519-13.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            BEAR_PICTURE,
                            TARANTULA_PICTURE,
                            BEAD_PARCEL,
                            ROUTS_TELEPORT_SCROLL,
                            SUCCUBUS_UNDIES,
                        ],
                    ) {
                        htmltext = "30519-14.html".to_owned();
                    }
                }
                TRADER_SHARI => {
                    htmltext = errand_helper(
                        ctx,
                        player,
                        qs,
                        (SHARIS_AXE, SHARIS_PAY),
                        ["30517-01.html", "30517-02.html", "30517-03.html"],
                    )
                    .unwrap_or(htmltext);
                }
                HEAD_BLACKSMITH_BRONK => {
                    htmltext = errand_helper(
                        ctx,
                        player,
                        qs,
                        (BRONKS_INGOT, BRONKS_PAY),
                        ["30525-01.html", "30525-02.html", "30525-03.html"],
                    )
                    .unwrap_or(htmltext);
                }
                PRIEST_OF_THE_EARTH_ZIMENF => {
                    htmltext = errand_helper(
                        ctx,
                        player,
                        qs,
                        (ZIMENFS_POTION, ZIMENFS_PAY),
                        ["30538-01.html", "30538-02.html", "30538-03.html"],
                    )
                    .unwrap_or(htmltext);
                }
                MASTER_TOMA => {
                    if ctx.has_quest_items(player, MIONS_LETTER) {
                        ctx.take_items(player, MIONS_LETTER, 1);
                        ctx.give_items(player, BEAR_PICTURE, 1);
                        qs.set_cond(ctx, 5, true);
                        qs.set_int(ctx, FLAG, 0);
                        htmltext = "30556-01.html".to_owned();
                    } else if ctx.has_quest_items(player, BEAR_PICTURE) {
                        if ctx.get_quest_items_count(player, HONEY_JAR) < 5 {
                            htmltext = "30556-02.html".to_owned();
                        } else {
                            ctx.take_items(player, BEAR_PICTURE, 1);
                            ctx.give_items(player, TARANTULA_PICTURE, 1);
                            ctx.take_items(player, HONEY_JAR, -1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30556-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, TARANTULA_PICTURE) {
                        htmltext = if ctx.get_quest_items_count(player, BEAD) < 20 {
                            "30556-04.html"
                        } else {
                            "30556-05a.html"
                        }
                        .to_owned();
                    } else if ctx.has_quest_items(player, BEAD_PARCEL)
                        && !ctx.has_quest_items(player, BEAD_PARCEL2)
                    {
                        htmltext = "30556-06a.html".to_owned();
                    } else if ctx.has_quest_items(player, BEAD_PARCEL2)
                        && !ctx.has_quest_items(player, BEAD_PARCEL)
                        && qs.is_memo_state(ctx, 2)
                    {
                        htmltext = "30556-06c.html".to_owned();
                    } else if ctx
                        .has_at_least_one_quest_item(player, &[ROUTS_TELEPORT_SCROLL, SUCCUBUS_UNDIES])
                    {
                        htmltext = "30556-07.html".to_owned();
                    }
                }
                WAREHOUSE_KEEPER_RAUT => {
                    if ctx.has_quest_items(player, BEAD_PARCEL) {
                        htmltext = "30316-01.html".to_owned();
                    } else if ctx.has_quest_items(player, ROUTS_TELEPORT_SCROLL) {
                        htmltext = "30316-04.html".to_owned();
                    } else if ctx.has_quest_items(player, SUCCUBUS_UNDIES) {
                        finish(
                            ctx,
                            player,
                            qs,
                            81_900,
                            ((160_267, 17_706), (228_064, 21_055), (295_862, 24_404)),
                        );
                        htmltext = "30316-05.html".to_owned();
                    }
                }
                TORAI => {
                    if ctx.has_quest_items(player, ROUTS_TELEPORT_SCROLL) {
                        htmltext = "30557-01.html".to_owned();
                    }
                }
                WAREHOUSE_CHIEF_YASENI => {
                    if ctx.has_quest_items(player, BEAD_PARCEL2)
                        && !ctx.has_quest_items(player, BEAD_PARCEL)
                        && qs.is_memo_state(ctx, 2)
                    {
                        htmltext = "31958-01.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

/// Shari, Bronk and Zimenf are the same helper: they swap Mion's errand item for a payment and
/// count `memoStateEx1` up by 10; the second payment also moves the quest to cond 3.
fn errand_helper(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    qs: l2_world::script::Qs,
    (errand, pay): (i32, i32),
    [first, second, again]: [&str; 3],
) -> Option<String> {
    if ctx.has_quest_items(player, errand) {
        let ex = qs.get_memo_state_ex(ctx, 1);
        if ex < 20 {
            ctx.take_items(player, errand, 1);
            ctx.give_items(player, pay, 1);
            qs.set_memo_state_ex(ctx, 1, ex + 10);
            Some(first.to_owned())
        } else {
            ctx.take_items(player, errand, 1);
            ctx.give_items(player, pay, 1);
            qs.set_memo_state(ctx, 1);
            qs.set_memo_state_ex(ctx, 1, ex + 10);
            qs.set_cond(ctx, 3, true);
            Some(second.to_owned())
        }
    } else if ctx.has_quest_items(player, pay) {
        Some(again.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00417_PathOfTheScavenger";

    const NPCS: &str = r#"
        <npc id="30316" type="Folk" level="70" name="Raut"/>
        <npc id="30519" type="Folk" level="70" name="Mion"/>
        <npc id="30524" type="Folk" level="70" name="Pipi"/>
        <npc id="30556" type="Folk" level="70" name="Toma"/>
        <npc id="30557" type="Folk" level="70" name="Torai"/>
        <npc id="31958" type="Folk" level="70" name="Yaseni"/>
        <npc id="20777" type="Monster" level="20" name="Hunter Bear"/>
        <npc id="27058" type="Monster" level="20" name="Honey Bear"/>"#;

    fn items() -> String {
        let mut out = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
            <item id="1642" type="Armor" name="Ring of Raven"><set name="bodypart" val="rfinger;lfinger"/></item>"#,
        );
        for id in (1643..=1657).chain([8543]) {
            out.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#
            ));
        }
        out
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00417PathOfTheScavenger);
        let pages = Pages::of(&Q00417PathOfTheScavenger)
            .page("30524-01.htm", &["ACCEPT"])
            .page("30519-01.html", &["reply_1"])
            .page("30316-01.html", &["30316-02.html", "30316-03.html"])
            .page("30557-01.html", &["30557-02.html"])
            .page("30557-02.html", &["30557-03.html"])
            .page("30556-05a.html", &["30556-05b.html", "30556-06b.html"])
            .page("31958-01.html", &["31958-02.html"])
            .plain(&[
                "30524-02.htm",
                "30524-02a.htm",
                "30524-04.htm",
                "30524-05.htm",
                "30524-06.html",
                "30524-07.html",
                "30524-08.htm",
                "30519-02.html",
                "30519-03.html",
                "30519-04.html",
                "30519-15.html",
                "30316-02.html",
                "30316-05.html",
                "30556-01.html",
                "30556-02.html",
                "30556-03.html",
                "30556-04.html",
                "30556-05b.html",
                "30556-06b.html",
                "30557-03.html",
                "31958-02.html",
            ])
            .npcs(&[
                COLLECTOR_PIPI,
                TRADER_MION,
                WAREHOUSE_KEEPER_RAUT,
                MASTER_TOMA,
                TORAI,
                WAREHOUSE_CHIEF_YASENI,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let pipi = t.spawn(COLLECTOR_PIPI, 20);
        (t, pipi)
    }

    /// The rows written since the last call, without the step-flag row the engine adds when a
    /// quest jumps over conds (`__compltdStateFlags`).
    fn writes(t: &mut ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    fn enter_fighter(t: &mut ScriptTest, level: i32, items: Vec<l2_world::ItemData>) {
        t.enter_as(1, level, race::DWARF, player_class::DWARVEN_FIGHTER, items);
    }

    /// Starts the quest at Pipi (the letter of recommendation comes).
    fn accept(t: &mut ScriptTest, pipi: Oid) {
        assert!(t.quest_page(1, pipi).contains("30524-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30524-05.htm"));
        writes(t);
    }

    #[test]
    fn only_a_dwarven_fighter_of_level_18_starts() {
        let (mut t, pipi) = setup();
        enter_fighter(&mut t, 17, vec![]);
        assert!(t.quest_page(1, pipi).contains("30524-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30524-02.htm"));
        t.enter_as(2, 20, race::DWARF, player_class::SCAVENGER, vec![]);
        t.quest_page(2, pipi);
        assert!(t.event(2, NAME, "ACCEPT").contains("30524-02a.htm"));
        t.enter_as(3, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(3, pipi);
        assert!(t.event(3, NAME, "ACCEPT").contains("30524-08.htm"));
        assert!(writes(&mut t).is_empty());
    }

    #[test]
    fn accepting_gives_the_letter_and_mion_hands_out_an_errand() {
        let (mut t, pipi) = setup();
        enter_fighter(&mut t, 18, vec![]);
        assert!(t.quest_page(1, pipi).contains("30524-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30524-05.htm"));
        assert_eq!(
            writes(&mut t),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoStateEx1", "0")
            ]
        );
        assert_eq!(t.count(1, PIPPIS_LETTER_OF_RECOMMENDATION), 1);
        assert!(t.quest_page(1, pipi).contains("30524-06.html"));
        let mion = t.spawn(TRADER_MION, 25);
        assert!(t.quest_page(1, mion).contains("30519-01.html"));
        let page = t.event(1, NAME, "reply_1");
        assert!(
            ["30519-02.html", "30519-03.html", "30519-04.html"]
                .iter()
                .any(|p| page.contains(p))
        );
        assert_eq!(t.count(1, PIPPIS_LETTER_OF_RECOMMENDATION), 0);
        assert_eq!(
            t.count(1, ZIMENFS_POTION) + t.count(1, SHARIS_AXE) + t.count(1, BRONKS_INGOT),
            1
        );
    }

    #[test]
    fn the_pictures_the_beads_and_the_scroll_lead_to_the_small_reward() {
        let (mut t, pipi) = setup();
        enter_fighter(&mut t, 20, vec![item(100, MIONS_LETTER, 1)]);
        accept(&mut t, pipi);
        let toma = t.spawn(MASTER_TOMA, 25);
        assert!(t.quest_page(1, toma).contains("30556-01.html"));
        assert_eq!((t.count(1, MIONS_LETTER), t.count(1, BEAR_PICTURE)), (0, 1));
        assert_eq!(
            writes(&mut t),
            [rows::update(NAME, "cond", "5"), rows::insert(NAME, "FLAG", "0")]
        );
        assert!(t.quest_page(1, toma).contains("30556-02.html"));
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), HONEY_JAR, 5));
        assert!(t.quest_page(1, toma).contains("30556-03.html"));
        assert_eq!((t.count(1, TARANTULA_PICTURE), t.count(1, HONEY_JAR)), (1, 0));
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), BEAD, 20));
        assert!(t.quest_page(1, toma).contains("30556-05a.html"));
        assert!(t.event(1, NAME, "30556-05b.html").contains("30556-05b.html"));
        assert_eq!((t.count(1, BEAD_PARCEL), t.count(1, BEAD)), (1, 0));
        writes(&mut t);

        let raut = t.spawn(WAREHOUSE_KEEPER_RAUT, 30);
        assert!(t.quest_page(1, raut).contains("30316-01.html"));
        assert!(t.event(1, NAME, "30316-02.html").contains("30316-02.html"));
        assert_eq!(writes(&mut t), [rows::update(NAME, "cond", "10")]);
        let torai = t.spawn(TORAI, 35);
        assert!(t.quest_page(1, torai).contains("30557-01.html"));
        assert!(t.event(1, NAME, "30557-02.html").contains("30557-02.html"));
        assert!(t.event(1, NAME, "30557-03.html").contains("30557-03.html"));
        assert_eq!(t.count(1, SUCCUBUS_UNDIES), 1);
        assert!(!t.in_world(torai));
        writes(&mut t);

        t.take();
        assert!(t.quest_page(1, raut).contains("30316-05.html"));
        assert_eq!((t.count(1, RING_OF_RAVEN), t.count(1, 57)), (1, 81_900));
        assert_eq!(
            writes(&mut t),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn the_bead_parcel_to_yaseni_pays_double() {
        let (mut t, pipi) = setup();
        enter_fighter(
            &mut t,
            20,
            vec![item(100, TARANTULA_PICTURE, 1), item(101, BEAD, 20)],
        );
        accept(&mut t, pipi);
        let toma = t.spawn(MASTER_TOMA, 25);
        assert!(t.quest_page(1, toma).contains("30556-05a.html"));
        assert!(t.event(1, NAME, "30556-06b.html").contains("30556-06b.html"));
        assert_eq!(
            writes(&mut t),
            [
                rows::insert(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "12")
            ]
        );
        let yaseni = t.spawn(WAREHOUSE_CHIEF_YASENI, 30);
        assert!(t.quest_page(1, yaseni).contains("31958-01.html"));
        assert!(t.event(1, NAME, "31958-02.html").contains("31958-02.html"));
        assert_eq!((t.count(1, RING_OF_RAVEN), t.count(1, 57)), (1, 163_800));
        assert_eq!(
            writes(&mut t),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn a_hunter_bear_kill_counts_up_to_a_honey_bear() {
        let (mut t, pipi) = setup();
        enter_fighter(&mut t, 20, vec![item(100, BEAR_PICTURE, 1)]);
        accept(&mut t, pipi);
        for kills in 1..60 {
            let bear = t.spawn(HUNTER_BEAR, 60);
            let npc = ScriptNpc {
                object_id: bear,
                id: HUNTER_BEAR,
            };
            t.world.run_as_script(NAME, |ctx| {
                Q00417PathOfTheScavenger.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
            t.kill(1, bear);
            if !t.npcs_of(HONEY_BEAR).is_empty() {
                // The first kill only sets FLAG 1; the spawn needs 20% per FLAG on the later ones.
                assert!(kills >= 2, "{kills}");
                return;
            }
        }
        panic!("no honey bear after 60 kills");
    }
}
