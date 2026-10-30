//! Quest 25: Hiding Behind the Truth (Benedict, Agripel, the wizard, Lidia's maid, three bookshelves, a coffin).
// Java: dist/game/data/scripts/quests/Q00025_HidingBehindTheTruth/Q00025_HidingBehindTheTruth.java
//
// Pattern: a long memo-state chain (`docs/design/scripts.md`, "Quest patterns"). The Java NPC
// variables `Q00025` (the player who is fighting a Triol pawn at a bookshelf; the bookshelf a pawn
// came from) live in `ctx.state`, keyed by object id.

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const HIGH_PRIEST_AGRIPEL: i32 = 31348;
const PRIEST_BENEDICT: i32 = 31349;
const MYSTERIOUS_WIZARD: i32 = 31522;
const TOMBSTONE: i32 = 31531;
const MAID_OF_LIDIA: i32 = 31532;
const BROKEN_BOOKSHELF2: i32 = 31533;
const BROKEN_BOOKSHELF3: i32 = 31534;
const BROKEN_BOOKSHELF4: i32 = 31535;
const COFFIN: i32 = 31536;

// Mobs
const TRIOL_PAWN: i32 = 27218;

// Items
const MAP_FOREST_OF_THE_DEAD: i32 = 7063;
const CONTRACT: i32 = 7066;
const LIDAS_DRESS: i32 = 7155;
const TOTEM_DOLL2: i32 = 7156;
const GEMSTONE_KEY: i32 = 7157;
const TOTEM_DOLL3: i32 = 7158;

// Rewards
const NECKLACE_OF_BLESSING: i32 = 936;
const EARING_OF_BLESSING: i32 = 874;
const RING_OF_BLESSING: i32 = 905;

// Misc
const MIN_LEVEL: i32 = 66;
const COFFIN_LOC: (i32, i32, i32) = (60104, -35820, -681);
/// Java `Q00024_InhabitantsOfTheForestOfTheDead.class.getSimpleName()`.
const INHABITANTS: &str = "Q00024_InhabitantsOfTheForestOfTheDead";

/// Java `TRIOL_PAWN_LOC.get(bookshelfId)`.
fn triol_pawn_loc(bookshelf: i32) -> Option<(i32, i32, i32)> {
    match bookshelf {
        BROKEN_BOOKSHELF2 => Some((47142, -35941, -1623)),
        BROKEN_BOOKSHELF3 => Some((50055, -47020, -3396)),
        BROKEN_BOOKSHELF4 => Some((59712, -47568, -2720)),
        _ => None,
    }
}

/// The Java NPC variable `Q00025`: on a bookshelf the player id of its pawn's opponent (0 = free),
/// on a pawn the bookshelf it came from.
#[derive(Default)]
struct HidingVars {
    desk_owner: HashMap<ObjectId, i32>,
    pawn_desk: HashMap<ObjectId, ObjectId>,
}

/// Java `Q00025_HidingBehindTheTruth`.
pub struct Q00025HidingBehindTheTruth;

impl Script for Q00025HidingBehindTheTruth {
    fn name(&self) -> &'static str {
        "Q00025_HidingBehindTheTruth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00025_HidingBehindTheTruth"
    }

    fn id(&self) -> i32 {
        25
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PRIEST_BENEDICT]);
        s.add_talk_id(&[
            HIGH_PRIEST_AGRIPEL,
            PRIEST_BENEDICT,
            MYSTERIOUS_WIZARD,
            TOMBSTONE,
            MAID_OF_LIDIA,
            BROKEN_BOOKSHELF2,
            BROKEN_BOOKSHELF3,
            BROKEN_BOOKSHELF4,
            COFFIN,
        ]);
        s.register_quest_items(&[GEMSTONE_KEY, CONTRACT, TOTEM_DOLL3, TOTEM_DOLL2, LIDAS_DRESS]);
        s.add_attack_id(&[TRIOL_PAWN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java: a null player makes `getNoQuestMsg(player)` / the quest state throw.
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match event {
            "31349-06.html" | "31349-07.html" | "31349-08.html" | "31349-09.html" | "31522-08.html"
            | "31522-09.html" | "31522-07.html" | "31522-11.html" | "31348-04.html" | "31348-05.html"
            | "31348-06.html" | "31348-11.html" | "31348-07.html" | "31348-12.html" | "31348-14.html"
            | "31532-04.html" | "31532-05.html" | "31532-06.html" | "31532-14.html" | "31532-15.html"
            | "31532-16.html" | "31532-19.html" | "31532-20.html" => {
                htmltext = event.to_owned();
            }
            _ => {
                // NOTE(java-quirk): every other branch uses `qs` unchecked; without a quest state
                // Java throws a NullPointerException, which `notifyEvent` swallows (null).
                let qs = qs?;
                match event {
                    "31349-03.html" => {
                        let q24_completed = ctx
                            .player_quest_state(player, INHABITANTS)
                            .is_some_and(|q| q.is_completed());
                        if qs.is_created(ctx) && q24_completed && ctx.player_level(player) >= MIN_LEVEL {
                            qs.set_memo_state(ctx, 1);
                            qs.start_quest(ctx);
                            htmltext = event.to_owned();
                        }
                    }
                    "31349-05.html" => {
                        if qs.is_memo_state(ctx, 1) {
                            if ctx.has_quest_items(player, TOTEM_DOLL2) {
                                htmltext = "31349-04.html".to_owned();
                            } else {
                                qs.set_cond(ctx, 2, true);
                                htmltext = event.to_owned();
                            }
                        }
                    }
                    "31349-10.html" => {
                        if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(player, TOTEM_DOLL2) {
                            qs.set_memo_state(ctx, 2);
                            qs.set_cond(ctx, 4, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31522-04.html" => {
                        if qs.is_memo_state(ctx, 6) && ctx.has_quest_items(player, GEMSTONE_KEY) {
                            qs.set_memo_state(ctx, 7);
                            qs.set_memo_state_ex(ctx, 1, 20);
                            qs.set_cond(ctx, 6, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31522-10.html" => {
                        if qs.is_memo_state(ctx, 16) {
                            qs.set_memo_state(ctx, 19);
                            htmltext = event.to_owned();
                        }
                    }
                    "31522-13.html" => {
                        if qs.is_memo_state(ctx, 19) {
                            qs.set_memo_state(ctx, 20);
                            qs.set_cond(ctx, 16, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31522-16.html" => {
                        if qs.is_memo_state(ctx, 24) {
                            ctx.take_items(player, MAP_FOREST_OF_THE_DEAD, -1);
                            ctx.reward_items(player, EARING_OF_BLESSING, 1);
                            ctx.reward_items(player, NECKLACE_OF_BLESSING, 1);
                            ctx.add_exp_and_sp(player, 572_277, 53_750);
                            qs.exit_quest(ctx, false, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-02.html" => {
                        if qs.is_memo_state(ctx, 2) {
                            ctx.take_items(player, TOTEM_DOLL2, -1);
                            qs.set_memo_state(ctx, 3);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-08.html" => {
                        if qs.is_memo_state(ctx, 3) {
                            ctx.give_items(player, GEMSTONE_KEY, 1);
                            qs.set_memo_state(ctx, 6);
                            qs.set_cond(ctx, 5, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-10.html" => {
                        if qs.is_memo_state(ctx, 20) && ctx.has_quest_items(player, TOTEM_DOLL3) {
                            ctx.take_items(player, TOTEM_DOLL3, -1);
                            qs.set_memo_state(ctx, 21);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-13.html" => {
                        if qs.is_memo_state(ctx, 21) {
                            qs.set_memo_state(ctx, 22);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-16.html" => {
                        if qs.is_memo_state(ctx, 22) {
                            qs.set_memo_state(ctx, 23);
                            qs.set_cond(ctx, 17, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31348-17.html" => {
                        if qs.is_memo_state(ctx, 22) {
                            qs.set_memo_state(ctx, 24);
                            qs.set_cond(ctx, 18, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31533-04.html" => {
                        let npc = npc?;
                        if qs.get_memo_state_ex(ctx, npc.id) != 0 {
                            htmltext = "31533-03.html".to_owned();
                        } else if ctx.get_random(60) > qs.get_memo_state_ex(ctx, 1) {
                            let ex = qs.get_memo_state_ex(ctx, 1);
                            qs.set_memo_state_ex(ctx, 1, ex + 20);
                            qs.set_memo_state_ex(ctx, npc.id, 1);
                            htmltext = "31533-03.html".to_owned();
                        } else {
                            qs.set_memo_state(ctx, 8);
                            htmltext = event.to_owned();
                            ctx.play_sound(player, QuestSound::AmdsoundHorror02);
                        }
                    }
                    "31533-05.html" => {
                        if qs.is_memo_state(ctx, 8) {
                            let npc = npc?;
                            if !ctx.has_quest_items(player, TOTEM_DOLL3) {
                                let desk_owner = ctx
                                    .state::<HidingVars>()
                                    .desk_owner
                                    .get(&npc.object_id)
                                    .copied()
                                    .unwrap_or(0);
                                if desk_owner == 0 {
                                    ctx.state::<HidingVars>()
                                        .desk_owner
                                        .insert(npc.object_id, player.0);
                                    if let Some((x, y, z)) = triol_pawn_loc(npc.id)
                                        && let Some(triyol) =
                                            ctx.add_spawn(TRIOL_PAWN, x, y, z, 0, true, 0, 0)
                                    {
                                        ctx.state::<HidingVars>()
                                            .pawn_desk
                                            .insert(triyol.object_id, npc.object_id);
                                        ctx.set_script_value(triyol, player.0);
                                        ctx.start_quest_timer(
                                            "SAY_TRIYOL",
                                            500,
                                            Some(triyol),
                                            Some(player),
                                            false,
                                        );
                                        ctx.start_quest_timer(
                                            "DESPAWN_TRIYOL",
                                            120_000,
                                            Some(triyol),
                                            Some(player),
                                            false,
                                        );
                                        ctx.set_intention_attack(triyol, player);
                                    }
                                    htmltext = event.to_owned();
                                    qs.set_cond(ctx, 7, false);
                                } else if desk_owner == player.0 {
                                    htmltext = "31533-06.html".to_owned();
                                } else {
                                    htmltext = "31533-07.html".to_owned();
                                }
                            } else {
                                htmltext = "31533-08.html".to_owned();
                            }
                        }
                    }
                    "31533-09.html" => {
                        if qs.is_memo_state(ctx, 8)
                            && ctx.has_all_quest_items(player, &[TOTEM_DOLL3, GEMSTONE_KEY])
                        {
                            ctx.give_items(player, CONTRACT, 1);
                            ctx.take_items(player, GEMSTONE_KEY, -1);
                            qs.set_memo_state(ctx, 9);
                            qs.set_cond(ctx, 9, false);
                            htmltext = event.to_owned();
                        }
                    }
                    "SAY_TRIYOL" => {
                        let npc = npc?;
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::General,
                            NpcStringId::ThatBoxWasSealedByMyMasterS1DonTTouchIt,
                            &[&name],
                        );
                    }
                    "DESPAWN_TRIYOL" => {
                        let npc = npc?;
                        release_desk(ctx, npc);
                        ctx.delete_npc(npc);
                    }
                    "31532-02.html" => {
                        if qs.is_memo_state(ctx, 9) && ctx.has_quest_items(player, CONTRACT) {
                            ctx.take_items(player, CONTRACT, -1);
                            qs.set_memo_state(ctx, 10);
                            htmltext = event.to_owned();
                        }
                    }
                    "31532-07.html" => {
                        if qs.is_memo_state(ctx, 10) {
                            qs.set_memo_state(ctx, 11);
                            ctx.play_sound(player, QuestSound::SkillsoundHorror1);
                            qs.set_cond(ctx, 11, false);
                            htmltext = event.to_owned();
                        }
                    }
                    "31532-11.html" => {
                        if qs.is_memo_state(ctx, 13) {
                            let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                            if memo_state_ex <= 3 {
                                qs.set_memo_state_ex(ctx, 1, memo_state_ex + 1);
                                ctx.play_sound(player, QuestSound::ChrsoundFdelfCry);
                                htmltext = event.to_owned();
                            } else {
                                qs.set_memo_state(ctx, 14);
                                htmltext = "31532-12.html".to_owned();
                            }
                        }
                    }
                    "31532-17.html" => {
                        if qs.is_memo_state(ctx, 14) {
                            qs.set_memo_state(ctx, 15);
                            htmltext = event.to_owned();
                        }
                    }
                    "31532-21.html" => {
                        if qs.is_memo_state(ctx, 15) {
                            qs.set_memo_state(ctx, 16);
                            qs.set_cond(ctx, 15, false);
                            htmltext = event.to_owned();
                        }
                    }
                    "31532-25.html" => {
                        if qs.is_memo_state(ctx, 23) {
                            ctx.take_items(player, MAP_FOREST_OF_THE_DEAD, -1);
                            ctx.reward_items(player, EARING_OF_BLESSING, 1);
                            ctx.reward_items(player, RING_OF_BLESSING, 2);
                            ctx.add_exp_and_sp(player, 572_277, 53_750);
                            qs.exit_quest(ctx, false, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "31531-02.html" => {
                        if qs.is_memo_state(ctx, 11) {
                            if let Some(coffin) =
                                ctx.add_spawn(COFFIN, COFFIN_LOC.0, COFFIN_LOC.1, COFFIN_LOC.2, 0, true, 0, 0)
                            {
                                ctx.start_quest_timer(
                                    "DESPAWN_BOX",
                                    20_000,
                                    Some(coffin),
                                    Some(player),
                                    false,
                                );
                            }
                            qs.set_cond(ctx, 12, true);
                            htmltext = event.to_owned();
                        }
                    }
                    "DESPAWN_BOX" => {
                        let npc = npc?;
                        ctx.delete_npc(npc);
                    }
                    _ => {}
                }
            }
        }
        Some(htmltext)
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
        if ctx.npc_current_hp(npc) <= 0.30 * f64::from(ctx.npc_max_hp(npc)) {
            // NOTE(java-quirk): `qs` is used unchecked; without a quest state Java throws here.
            let Some(qs) = ctx.get_quest_state(attacker, false) else {
                return;
            };
            if qs.is_memo_state(ctx, 8)
                && !ctx.has_quest_items(attacker, TOTEM_DOLL3)
                && attacker.0 == ctx.get_script_value(npc)
            {
                ctx.give_items(attacker, TOTEM_DOLL3, 1);
                qs.set_cond(ctx, 8, true);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::General,
                    NpcStringId::YouVeEndedMyImmortalLifeYouReProtectedByTheFeudalLordArenTYou,
                    &[],
                );
                release_desk(ctx, npc);
                ctx.delete_npc(npc);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == PRIEST_BENEDICT {
                    let q24_completed = ctx
                        .player_quest_state(talker, INHABITANTS)
                        .is_some_and(|q| q.is_completed());
                    if q24_completed && ctx.player_level(talker) >= MIN_LEVEL {
                        htmltext = "31349-01.htm".to_owned();
                    } else {
                        htmltext = "31349-02.html".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                PRIEST_BENEDICT => match qs.get_memo_state(ctx) {
                    1 => htmltext = "31349-03a.html".to_owned(),
                    2 => htmltext = "31349-11.html".to_owned(),
                    _ => {}
                },
                MYSTERIOUS_WIZARD => match qs.get_memo_state(ctx) {
                    1 => {
                        if !ctx.has_quest_items(talker, TOTEM_DOLL2) {
                            ctx.give_items(talker, TOTEM_DOLL2, 1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "31522-01.html".to_owned();
                        } else {
                            htmltext = "31522-02.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.has_quest_items(talker, GEMSTONE_KEY) {
                            htmltext = "31522-03.html".to_owned();
                        }
                    }
                    9 => {
                        if ctx.has_quest_items(talker, CONTRACT) {
                            qs.set_cond(ctx, 10, true);
                            htmltext = "31522-06.html".to_owned();
                        }
                    }
                    16 => htmltext = "31522-06a.html".to_owned(),
                    19 => htmltext = "31522-12.html".to_owned(),
                    20 => htmltext = "31522-14.html".to_owned(),
                    24 => htmltext = "31522-15.html".to_owned(),
                    23 => htmltext = "31522-15a.html".to_owned(),
                    memo => {
                        if memo % 100 == 7 {
                            htmltext = "31522-05.html".to_owned();
                        }
                    }
                },
                HIGH_PRIEST_AGRIPEL => match qs.get_memo_state(ctx) {
                    2 => htmltext = "31348-01.html".to_owned(),
                    3 => htmltext = "31348-03.html".to_owned(),
                    6 => htmltext = "31348-08a.html".to_owned(),
                    20 => {
                        if ctx.has_quest_items(talker, TOTEM_DOLL3) {
                            htmltext = "31348-09.html".to_owned();
                        }
                    }
                    21 => htmltext = "31348-10a.html".to_owned(),
                    22 => htmltext = "31348-15.html".to_owned(),
                    23 => htmltext = "31348-18.html".to_owned(),
                    24 => htmltext = "31348-19.html".to_owned(),
                    _ => {}
                },
                BROKEN_BOOKSHELF2 | BROKEN_BOOKSHELF3 | BROKEN_BOOKSHELF4 => {
                    let memo = qs.get_memo_state(ctx);
                    if memo % 100 == 7 {
                        htmltext = "31533-01.html".to_owned();
                    } else if memo % 100 >= 9 {
                        htmltext = "31533-02.html".to_owned();
                    } else if memo == 8 {
                        htmltext = "31533-04.html".to_owned();
                    }
                }
                MAID_OF_LIDIA => match qs.get_memo_state(ctx) {
                    9 => {
                        if ctx.has_quest_items(talker, CONTRACT) {
                            htmltext = "31532-01.html".to_owned();
                        }
                    }
                    10 => htmltext = "31532-03.html".to_owned(),
                    11 => {
                        ctx.play_sound(talker, QuestSound::SkillsoundHorror1);
                        htmltext = "31532-08.html".to_owned();
                    }
                    12 => {
                        if ctx.has_quest_items(talker, LIDAS_DRESS) {
                            ctx.take_items(talker, LIDAS_DRESS, -1);
                            qs.set_memo_state(ctx, 13);
                            qs.set_cond(ctx, 14, true);
                            htmltext = "31532-09.html".to_owned();
                        }
                    }
                    13 => {
                        qs.set_memo_state_ex(ctx, 1, 0);
                        ctx.play_sound(talker, QuestSound::ChrsoundFdelfCry);
                        htmltext = "31532-10.html".to_owned();
                    }
                    14 => htmltext = "31532-13.html".to_owned(),
                    15 => htmltext = "31532-18.html".to_owned(),
                    16 => htmltext = "31532-22.html".to_owned(),
                    23 => htmltext = "31532-23.html".to_owned(),
                    24 => htmltext = "31532-24.html".to_owned(),
                    _ => {}
                },
                TOMBSTONE => match qs.get_memo_state(ctx) {
                    11 => htmltext = "31531-01.html".to_owned(),
                    12 => htmltext = "31531-03.html".to_owned(),
                    _ => {}
                },
                COFFIN => {
                    if qs.is_memo_state(ctx, 11) {
                        ctx.give_items(talker, LIDAS_DRESS, 1);
                        ctx.cancel_quest_timer("DESPAWN_BOX", Some(npc), Some(talker));
                        ctx.start_quest_timer("DESPAWN_BOX", 3000, Some(npc), Some(talker), false);
                        qs.set_memo_state(ctx, 12);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "31536-01.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == PRIEST_BENEDICT {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `brokenDesk.getVariables().set("Q00025", 0)` for the bookshelf a pawn came from.
fn release_desk(ctx: &mut ScriptCtx<'_>, pawn: ScriptNpc) {
    let desk = ctx.state::<HidingVars>().pawn_desk.remove(&pawn.object_id);
    if let Some(desk) = desk {
        ctx.state::<HidingVars>().desk_owner.insert(desk, 0);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00025_HidingBehindTheTruth";

    const NPCS: &str = r#"
        <npc id="31348" type="Folk" level="70" name="Agripel"/>
        <npc id="31349" type="Folk" level="70" name="Benedict"/>
        <npc id="31522" type="Folk" level="70" name="Mysterious Wizard"/>
        <npc id="31531" type="Folk" level="70" name="Tombstone"/>
        <npc id="31532" type="Folk" level="70" name="Maid of Lidia"/>
        <npc id="31533" type="Folk" level="70" name="Bookshelf"/>
        <npc id="31536" type="Folk" level="70" name="Coffin"/>
        <npc id="27218" type="Monster" level="60" name="Triol Pawn"/>"#;

    const ITEMS: &str = r#"
        <item id="874" type="EtcItem" name="Earing of Blessing"/>
        <item id="936" type="EtcItem" name="Necklace of Blessing"/>
        <item id="7063" type="EtcItem" name="Map"/>
        <item id="7066" type="EtcItem" name="Contract"><set name="is_questitem" val="true"/></item>
        <item id="7155" type="EtcItem" name="Lida's Dress"><set name="is_questitem" val="true"/></item>
        <item id="7156" type="EtcItem" name="Totem Doll"><set name="is_questitem" val="true"/></item>
        <item id="7157" type="EtcItem" name="Gemstone Key"><set name="is_questitem" val="true"/></item>
        <item id="7158" type="EtcItem" name="Totem Doll 3"><set name="is_questitem" val="true"/></item>"#;

    /// A stand-in for a quest this one reads.
    struct Other(&'static str, i32);

    impl Script for Other {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Other"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Other(INHABITANTS, 24));
        registry.add(Q00025HidingBehindTheTruth);
        let pages = Pages::of(&Q00025HidingBehindTheTruth)
            .page("31349-01.htm", &["31349-03.html"])
            .plain(&["31349-02.html", "31349-03.html"])
            .npcs(&[PRIEST_BENEDICT, MYSTERIOUS_WIZARD, HIGH_PRIEST_AGRIPEL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let benedict = t.spawn(PRIEST_BENEDICT, 20);
        (t, benedict)
    }

    fn finish_inhabitants(t: &mut ScriptTest) {
        t.world.run_as_script(INHABITANTS, |ctx| {
            if let Some(qs) = ctx.quest_state_of(INHABITANTS, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00025HidingBehindTheTruth.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00025HidingBehindTheTruth.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn writes(t: &ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    fn started(t: &mut ScriptTest, benedict: Oid) {
        t.enter(1, 66, vec![]);
        finish_inhabitants(t);
        t.quest_page(1, benedict);
        t.event(1, NAME, "31349-03.html");
        t.take_writes();
    }

    #[test]
    fn it_needs_level_66_and_the_inhabitants() {
        let (mut t, benedict) = setup();
        t.enter(1, 66, vec![]);
        assert!(t.quest_page(1, benedict).contains("31349-02.html"));

        let (mut t, benedict) = setup();
        t.enter(1, 65, vec![]);
        finish_inhabitants(&mut t);
        assert!(t.quest_page(1, benedict).contains("31349-02.html"));
        // The refused start answers the no-quest page and writes nothing.
        assert!(!t.event(1, NAME, "31349-03.html").contains("31349-03.html"));
        assert!(writes(&t).is_empty());
    }

    #[test]
    fn the_start_the_doll_the_key_and_the_wizard() {
        let (mut t, benedict) = setup();
        t.enter(1, 66, vec![]);
        finish_inhabitants(&mut t);
        assert!(t.quest_page(1, benedict).contains("31349-01.htm"));
        assert!(t.event(1, NAME, "31349-03.html").contains("31349-03.html"));
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // Cond 2, then the wizard gives the doll (cond 3), Benedict sends it to Agripel (cond 4).
        assert_eq!(
            ev(&mut t, "31349-05.html", None).as_deref(),
            Some("31349-05.html")
        );
        let wizard = t.spawn(MYSTERIOUS_WIZARD, 40);
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-01.html"));
        assert_eq!(t.count(1, TOTEM_DOLL2), 1);
        assert_eq!(
            ev(&mut t, "31349-05.html", None).as_deref(),
            Some("31349-04.html")
        );
        assert_eq!(
            ev(&mut t, "31349-10.html", None).as_deref(),
            Some("31349-10.html")
        );
        let agripel = t.spawn(HIGH_PRIEST_AGRIPEL, 60);
        assert_eq!(talk(&mut t, agripel).as_deref(), Some("31348-01.html"));
        assert_eq!(
            ev(&mut t, "31348-02.html", None).as_deref(),
            Some("31348-02.html")
        );
        assert_eq!(t.count(1, TOTEM_DOLL2), 0);
        assert_eq!(
            ev(&mut t, "31348-08.html", None).as_deref(),
            Some("31348-08.html")
        );
        assert_eq!(t.count(1, GEMSTONE_KEY), 1);
        writes(&t);
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-03.html"));
        assert_eq!(
            ev(&mut t, "31522-04.html", None).as_deref(),
            Some("31522-04.html")
        );
        assert_eq!(
            writes(&t),
            [
                rows::update(NAME, "memoState", "7"),
                rows::insert(NAME, "memoStateEx1", "20"),
                rows::update(NAME, "cond", "6"),
            ]
        );
    }

    #[test]
    fn the_triol_pawn_gives_the_doll_and_the_wizard_pays() {
        let (mut t, benedict) = setup();
        started(&mut t, benedict);
        memo(&mut t, 8);
        let shelf = t.spawn(BROKEN_BOOKSHELF2, 40);

        // The pawn is spawned for this player; a second player id is refused.
        assert_eq!(
            ev(&mut t, "31533-05.html", Some(shelf)).as_deref(),
            Some("31533-05.html")
        );
        let pawn = t.npcs_of(TRIOL_PAWN)[0];
        assert_eq!(
            ev(&mut t, "31533-05.html", Some(shelf)).as_deref(),
            Some("31533-06.html")
        );

        // A hit at full health does nothing; a hit under 30% gives the doll and removes the pawn.
        let hit = |t: &mut ScriptTest| {
            t.world.run_as_script(NAME, |ctx| {
                if let Some(npc) = ctx.script_npc(pawn) {
                    Q00025HidingBehindTheTruth.on_attack(ctx, npc, Oid(1), 1, false, None);
                }
            });
        };
        hit(&mut t);
        assert_eq!(t.count(1, TOTEM_DOLL3), 0);
        t.world.npc_mut(pawn).unwrap().core.cur_hp = 0.0;
        hit(&mut t);
        assert_eq!(t.count(1, TOTEM_DOLL3), 1);
        assert!(!t.in_world(pawn));
        // The bookshelf is free again.
        assert_eq!(
            ev(&mut t, "31533-05.html", Some(shelf)).as_deref(),
            Some("31533-08.html")
        );

        // The contract: key and doll needed.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), GEMSTONE_KEY, 1));
        writes(&t);
        assert_eq!(
            ev(&mut t, "31533-09.html", None).as_deref(),
            Some("31533-09.html")
        );
        assert_eq!(t.count(1, CONTRACT), 1);

        // The reward at memo 24 from the wizard.
        memo(&mut t, 24);
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), MAP_FOREST_OF_THE_DEAD, 1));
        assert_eq!(
            ev(&mut t, "31522-16.html", None).as_deref(),
            Some("31522-16.html")
        );
        assert_eq!(
            (t.count(1, EARING_OF_BLESSING), t.count(1, NECKLACE_OF_BLESSING)),
            (1, 1)
        );
        assert_eq!(t.count(1, MAP_FOREST_OF_THE_DEAD), 0);
        let w = writes(&t);
        assert!(w.contains(&rows::delete(NAME, false)));
        assert!(w.contains(&rows::update(NAME, "<state>", "Completed")));
    }
}
