//! Quest 236: Seeds of Chaos (the Kamael's Star of Destiny quest: Hierarch Kekropus, Kurstin,
//! the Mysterious Wizard, Katenar and Harkilgamed who are summoned next to the player, the Rock,
//! Rodenpicula and Mother Nornil).
// Java: dist/game/data/scripts/quests/Q00236_SeedsOfChaos/Q00236_SeedsOfChaos.java
//
// Pattern: the memo state drives the quest; the summoned helpers (Katenar, Harkilgamed) keep the
// player and their "owner" NPC in the NPC variables (`player0`, `npc0`, `SPAWNED`, `PLAYER_ID`),
// and a 120 s timer despawns them (see `q00184_art_of_persuasion.rs` for the same variables).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KURSTIN: i32 = 31387;
const MYSTERIOU_WIZARD: i32 = 31522;
const HIERARCH_KEKROPUS: i32 = 32138;
const VICE_HIERARCH_MAO: i32 = 32190;
const KATENAR: i32 = 32235;
const HARKILGAMED: i32 = 32236;
const RODENPICULA: i32 = 32237;
const ROCK: i32 = 32238;
const MOTHER_NORNIL: i32 = 32239;
const KATENAR_A: i32 = 32332;
const KATENAR_B: i32 = 32333;
const HARKILGAMED_A: i32 = 32334;

// Items
const STAR_OF_DESTINY: i32 = 5011;
const SHINING_MEDALLION: i32 = 9743;
const BLOOD_JEWEL: i32 = 9744;
const BLACK_ECHO_CRYSTAL: i32 = 9745;

// Reward
const SCROLL_ENCHANT_WEAPON_A_GRADE: i32 = 729;

// Monster
const NEEDLE_STAKATO_DRONE: i32 = 21516;
const SHOUT_OF_SPLENDOR: i32 = 21532;
const ALLIANCE_OF_SPLENDOR: i32 = 21533;
const ALLIANCE_OF_SPLENDOR_1: i32 = 21534;
const SIGNET_OF_SPLENDOR: i32 = 21535;
const CROWN_OF_SPLENDOR: i32 = 21536;
const FANG_OF_SPLENDOR: i32 = 21537;
const FANG_OF_SPLENDOR_1: i32 = 21538;
const WAILINGOF_SPLENDOR: i32 = 21539;
const WAILINGOF_SPLENDOR_1: i32 = 21540;
const VAMPIRE_WIZARD: i32 = 21588;
const VAMPIRE_WIZARD_A: i32 = 21589;

// Misc
const MIN_LEVEL: i32 = 75;

// Java quest name of `Q00025_HidingBehindTheTruth.class.getSimpleName()`.
const HIDING_BEHIND_THE_TRUTH: &str = "Q00025_HidingBehindTheTruth";

/// Java `Q00236_SeedsOfChaos`.
pub struct Q00236SeedsOfChaos;

/// Java `npc.getVariables().getObject(name, ...)`: the object id stored by `put_link`, 0 for none.
fn link(ctx: &ScriptCtx<'_>, npc: ObjectId, name: &str) -> i64 {
    ctx.world().npc_variables(npc).map_or(0, |v| v.get_long(name, 0))
}

/// Java `npc0.getVariables().set("SPAWNED", value)`.
fn set_spawned(ctx: &mut ScriptCtx<'_>, npc: i64, value: bool) {
    if npc != 0 {
        ctx.world_mut()
            .npc_variables_mut(ObjectId(npc as i32))
            .set_bool("SPAWNED", value);
    }
}

/// The shared 120 s timers of the helpers: the owner may spawn a new one, the helper leaves.
fn helper_timeout(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, say: NpcStringId, needs_player: bool) {
    let c0 = link(ctx, npc.object_id, "player0");
    let npc0 = link(ctx, npc.object_id, "npc0");
    if npc0 != 0
        && ctx
            .world()
            .npc_variables(ObjectId(npc0 as i32))
            .is_some_and(|v| v.get_bool("SPAWNED", false))
    {
        set_spawned(ctx, npc0, false);
        if !needs_player || c0 != 0 {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, say, &[]);
        }
    }
    ctx.delete_npc(npc);
}

/// The "owner" NPC's `SPAWNED` / `PLAYER_ID` handshake and the spawn of the helper (Java passes
/// `+10` as the heading next to the player and 0 at the Rock); returns the page to show.
fn summon_helper(
    ctx: &mut ScriptCtx<'_>,
    owner: ScriptNpc,
    player: ObjectId,
    helper: i32,
    at: Option<(i32, i32, i32)>,
    heading: i32,
    pages: (&str, &str, &str),
) -> Option<String> {
    let (spawn_page, same_player_page, other_player_page) = pages;
    let (spawned, player_id) = {
        let v = ctx.world_mut().npc_variables_mut(owner.object_id);
        (v.get_bool("SPAWNED", false), v.get_int("PLAYER_ID", 0))
    };
    if !spawned {
        let v = ctx.world_mut().npc_variables_mut(owner.object_id);
        v.set_bool("SPAWNED", true);
        v.set_int("PLAYER_ID", player.0);
        let (x, y, z) = at.or_else(|| ctx.world().position_of(player))?;
        let spawned = ctx.add_spawn(helper, x, y, z, heading, false, 0, 0);
        if let Some(h) = spawned {
            let hv = ctx.world_mut().npc_variables_mut(h.object_id);
            hv.set_long("npc0", i64::from(owner.object_id.0));
            hv.set_long("player0", i64::from(player.0));
        }
        Some(spawn_page.to_owned())
    } else if player_id == player.0 {
        Some(same_player_page.to_owned())
    } else {
        Some(other_player_page.to_owned())
    }
}

/// The despawn events of the helpers: only the player who was met, then the helper says goodbye.
fn helper_despawn(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, say: NpcStringId) {
    let c0 = link(ctx, npc.object_id, "player0");
    let npc0 = link(ctx, npc.object_id, "npc0");
    if i64::from(player.0) == c0 {
        set_spawned(ctx, npc0, false);
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, say, &[]);
        ctx.delete_npc(npc);
    }
}

impl Script for Q00236SeedsOfChaos {
    fn name(&self) -> &'static str {
        "Q00236_SeedsOfChaos"
    }

    fn path(&self) -> &'static str {
        "quests/Q00236_SeedsOfChaos"
    }

    fn id(&self) -> i32 {
        236
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIERARCH_KEKROPUS]);
        s.add_talk_id(&[
            HIERARCH_KEKROPUS,
            KURSTIN,
            MYSTERIOU_WIZARD,
            VICE_HIERARCH_MAO,
            KATENAR,
            HARKILGAMED,
            RODENPICULA,
            ROCK,
            MOTHER_NORNIL,
            KATENAR_A,
            KATENAR_B,
            HARKILGAMED_A,
        ]);
        s.add_kill_id(&[
            NEEDLE_STAKATO_DRONE,
            SHOUT_OF_SPLENDOR,
            ALLIANCE_OF_SPLENDOR,
            ALLIANCE_OF_SPLENDOR_1,
            SIGNET_OF_SPLENDOR,
            CROWN_OF_SPLENDOR,
            FANG_OF_SPLENDOR,
            FANG_OF_SPLENDOR_1,
            WAILINGOF_SPLENDOR,
            WAILINGOF_SPLENDOR_1,
            VAMPIRE_WIZARD,
            VAMPIRE_WIZARD_A,
        ]);
        s.add_spawn_id(&[KATENAR, HARKILGAMED, KATENAR_A, KATENAR_B, HARKILGAMED_A]);
        s.register_quest_items(&[SHINING_MEDALLION, BLOOD_JEWEL, BLACK_ECHO_CRYSTAL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // The 120 s timers of the helpers (`super.onEvent` answers null).
        match event {
            "KATENAR_120" | "KATENAR_A_120" | "KATENAR_B_120" => {
                let npc = npc?;
                helper_timeout(ctx, npc, NpcStringId::HmmWhereDidMyFriendGo, true);
                return None;
            }
            "HARKILGAMED_120" | "HARKILGAMED_A_120" => {
                let npc = npc?;
                helper_timeout(ctx, npc, NpcStringId::GraaahWeReBeingAttacked, false);
                return None;
            }
            _ => {}
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32138-04.htm" => {
                if qs.is_created(ctx) {
                    qs.set_memo_state(ctx, 1);
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL
                    && ctx.player_race(player) == race::KAMAEL
                    && ctx.has_quest_items(player, STAR_OF_DESTINY)
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-12.html" => {
                if qs.is_memo_state(ctx, 30) {
                    qs.set_memo_state(ctx, 40);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31387-03.html" => {
                if qs.is_memo_state(ctx, 11) {
                    ctx.take_items(player, BLOOD_JEWEL, -1);
                    qs.set_memo_state(ctx, 12);
                    htmltext = Some(event.to_owned());
                }
            }
            "31387-05a.html" => {
                if qs.is_memo_state(ctx, 12) {
                    let q25_completed = ctx
                        .player_quest_state(player, HIDING_BEHIND_THE_TRUTH)
                        .is_some_and(|q25| q25.is_completed());
                    if q25_completed {
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31387-05b.html".to_owned());
                    }
                }
            }
            "31387-10.html" => {
                if qs.is_memo_state(ctx, 12) {
                    qs.set_memo_state(ctx, 20);
                    qs.set_memo_state_ex(ctx, 1, 1);
                    qs.set_cond(ctx, 11, true);
                    htmltext = Some(event.to_owned());
                } else if qs.is_memo_state(ctx, 20) && qs.get_memo_state_ex(ctx, 1) == 1 {
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-04a.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-05a.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-05b.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-09a.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, BLACK_ECHO_CRYSTAL) {
                    ctx.take_items(player, BLACK_ECHO_CRYSTAL, -1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-12a.html" => {
                if qs.is_memo_state(ctx, 6)
                    && let Some(npc) = npc
                {
                    // Java adds the offset to the player's x and y.
                    let at = ctx
                        .world()
                        .position_of(player)
                        .map(|(x, y, z)| (x + 10, y + 10, z));
                    htmltext = summon_helper(
                        ctx,
                        npc,
                        player,
                        KATENAR,
                        at,
                        10,
                        (event, "31522-13a.html", "31522-14a.html"),
                    );
                }
            }
            "31522-09b.html" => {
                if qs.is_memo_state(ctx, 3)
                    && qs.get_memo_state_ex(ctx, 1) == 2
                    && let Some(npc) = npc
                {
                    let at = ctx
                        .world()
                        .position_of(player)
                        .map(|(x, y, z)| (x + 10, y + 10, z));
                    htmltext = summon_helper(
                        ctx,
                        npc,
                        player,
                        KATENAR_A,
                        at,
                        10,
                        (event, "31522-10b.html", "31522-11b.html"),
                    );
                }
            }
            "31522-14b.html" => {
                if qs.is_memo_state(ctx, 7)
                    && ctx.has_quest_items(player, BLOOD_JEWEL)
                    && let Some(npc) = npc
                {
                    let at = ctx
                        .world()
                        .position_of(player)
                        .map(|(x, y, z)| (x + 10, y + 10, z));
                    htmltext = summon_helper(
                        ctx,
                        npc,
                        player,
                        KATENAR_B,
                        at,
                        10,
                        (event, "31522-15b.html", "31522-15bz.html"),
                    );
                }
            }
            "32235-09a.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 20);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32236-07.html" => {
                if qs.is_memo_state(ctx, 20) {
                    let c0 = npc.map_or(0, |n| link(ctx, n.object_id, "player0"));
                    if c0 != 0 {
                        qs.set_memo_state(ctx, 21);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        qs.set_cond(ctx, 12, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "32237-10.html" => {
                if qs.is_memo_state(ctx, 40) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32237-11.html" => {
                if qs.is_memo_state(ctx, 40) {
                    qs.set_memo_state(ctx, 42);
                    qs.set_cond(ctx, 17, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32237-13.html" => {
                if qs.is_memo_state(ctx, 43) {
                    qs.set_memo_state(ctx, 44);
                    qs.set_cond(ctx, 19, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32237-17.html" => {
                if qs.is_memo_state(ctx, 45) {
                    ctx.give_items(player, SCROLL_ENCHANT_WEAPON_A_GRADE, 1);
                    ctx.take_items(player, STAR_OF_DESTINY, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32238-02.html" => {
                if qs.is_memo_state(ctx, 20)
                    && let Some(npc) = npc
                {
                    htmltext = summon_helper(
                        ctx,
                        npc,
                        player,
                        HARKILGAMED,
                        Some((71722, -78853, -4464)),
                        0,
                        (event, "32238-03.html", "32238-04z.html"),
                    );
                }
            }
            "32238-06.html" => {
                if qs.is_memo_state(ctx, 22)
                    && let Some(npc) = npc
                {
                    htmltext = summon_helper(
                        ctx,
                        npc,
                        player,
                        HARKILGAMED_A,
                        Some((71722, -78853, -4464)),
                        0,
                        (event, "32238-07.html", "32238-08.html"),
                    );
                }
            }
            "32239-04.html" => {
                if qs.is_memo_state(ctx, 42) {
                    qs.set_memo_state(ctx, 43);
                    qs.set_cond(ctx, 18, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32239-08.html" => {
                if qs.is_memo_state(ctx, 44) {
                    qs.set_memo_state(ctx, 45);
                    qs.set_cond(ctx, 20, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32332-05b.html" => {
                if qs.is_memo_state(ctx, 3) && qs.get_memo_state_ex(ctx, 1) == 2 {
                    let c0 = npc.map_or(0, |n| link(ctx, n.object_id, "player0"));
                    if c0 != 0 {
                        qs.set_memo_state(ctx, 7);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        qs.set_cond(ctx, 8, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "32334-17.html" => {
                if qs.is_memo_state(ctx, 22) {
                    let c0 = npc.map_or(0, |n| link(ctx, n.object_id, "player0"));
                    if c0 != 0 {
                        ctx.take_items(player, SHINING_MEDALLION, -1);
                        qs.set_memo_state(ctx, 30);
                        qs.set_cond(ctx, 14, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "KEITNAR_DESPAWN" => {
                if qs.is_memo_state(ctx, 20)
                    && qs.get_memo_state_ex(ctx, 1) == 0
                    && let Some(npc) = npc
                {
                    helper_despawn(ctx, npc, player, NpcStringId::BestOfLuckWithYourFutureEndeavours);
                }
            }
            "HARKILGAMED_DESPAWN" => {
                if qs.is_memo_state(ctx, 21)
                    && let Some(npc) = npc
                {
                    helper_despawn(ctx, npc, player, NpcStringId::InThatCaseIWishYouGoodLuck);
                }
            }
            "KEITNAR_A_DESPAWN" => {
                if let Some(npc) = npc {
                    helper_despawn(ctx, npc, player, NpcStringId::BestOfLuckWithYourFutureEndeavours);
                }
            }
            "KEITNAR_B_DESPAWN" => {
                if qs.is_memo_state(ctx, 11)
                    && ctx.has_quest_items(player, BLOOD_JEWEL)
                    && let Some(npc) = npc
                {
                    helper_despawn(ctx, npc, player, NpcStringId::BestOfLuckWithYourFutureEndeavours);
                }
            }
            "HARKILGAMED_A_DESPAWN" => {
                if qs.is_memo_state(ctx, 30)
                    && let Some(npc) = npc
                {
                    helper_despawn(ctx, npc, player, NpcStringId::SafeTravels);
                }
            }
            "31387-02.html" | "31387-04.html" | "31387-06.html" | "31387-07.html" | "31387-08.html"
            | "31522-02.html" | "31522-03.html" | "31522-04b.html" | "31522-08a.html" | "31522-11a.html"
            | "32138-02.html" | "32138-07.html" | "32138-08.html" | "32138-09.html" | "32138-10.html"
            | "32138-11.html" | "32235-02a.html" | "32235-03a.html" | "32235-04a.html" | "32235-05a.html"
            | "32235-06a.html" | "32235-08a.html" | "32236-03.html" | "32236-04.html" | "32236-05.html"
            | "32236-06.html" | "32237-02.html" | "32237-03.html" | "32237-04.html" | "32237-05.html"
            | "32237-06.html" | "32237-07.html" | "32237-08.html" | "32237-16.html" | "32237-18.html"
            | "32239-03.html" | "32239-07.html" | "32332-02b.html" | "32332-03b.html" | "32332-04b.html"
            | "32334-10.html" | "32334-11.html" | "32334-12.html" | "32334-13.html" | "32334-14.html"
            | "32334-15.html" | "32334-16.html" => {
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
            NEEDLE_STAKATO_DRONE => {
                if qs.is_memo_state(ctx, 2)
                    && !ctx.has_quest_items(killer, BLACK_ECHO_CRYSTAL)
                    && ctx.get_random(100) < 20
                {
                    ctx.give_items(killer, BLACK_ECHO_CRYSTAL, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            SHOUT_OF_SPLENDOR
            | ALLIANCE_OF_SPLENDOR
            | ALLIANCE_OF_SPLENDOR_1
            | SIGNET_OF_SPLENDOR
            | CROWN_OF_SPLENDOR
            | FANG_OF_SPLENDOR
            | FANG_OF_SPLENDOR_1
            | WAILINGOF_SPLENDOR
            | WAILINGOF_SPLENDOR_1 => {
                if qs.is_memo_state(ctx, 21)
                    && ctx.get_quest_items_count(killer, SHINING_MEDALLION) < 62
                    && ctx.get_random(100) < 70
                {
                    ctx.give_items(killer, SHINING_MEDALLION, 1);
                    if ctx.get_quest_items_count(killer, SHINING_MEDALLION) == 62 {
                        qs.set_memo_state(ctx, 22);
                        qs.set_cond(ctx, 13, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            VAMPIRE_WIZARD | VAMPIRE_WIZARD_A => {
                if qs.is_memo_state(ctx, 7)
                    && !ctx.has_quest_items(killer, BLOOD_JEWEL)
                    && ctx.get_random(100) < 8
                {
                    ctx.give_items(killer, BLOOD_JEWEL, 1);
                    qs.set_cond(ctx, 9, true);
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
            if npc.id == HIERARCH_KEKROPUS {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    if ctx.player_race(player) == race::KAMAEL {
                        if ctx.has_quest_items(player, STAR_OF_DESTINY) {
                            "32138-01.htm"
                        } else {
                            "32138-01x.html"
                        }
                    } else {
                        "32138-01y.html"
                    }
                } else {
                    "32138-01z.html"
                }
                .to_owned();
            }
        } else if state == STARTED {
            let memo = qs.get_memo_state(ctx);
            let memo_ex1 = qs.get_memo_state_ex(ctx, 1);
            match npc.id {
                HIERARCH_KEKROPUS => match memo {
                    1 => htmltext = "32138-05.html".to_owned(),
                    30 => htmltext = "32138-06.html".to_owned(),
                    40 => htmltext = "32138-13.html".to_owned(),
                    _ => {}
                },
                KURSTIN => match memo {
                    11 => htmltext = "31387-01.html".to_owned(),
                    12 => htmltext = "31387-04.html".to_owned(),
                    20 => {
                        if memo_ex1 == 1 {
                            htmltext = "31387-11.html".to_owned();
                        }
                    }
                    _ => {}
                },
                MYSTERIOU_WIZARD => match memo {
                    1 => htmltext = "31522-01.html".to_owned(),
                    2 => {
                        if !ctx.has_quest_items(player, BLACK_ECHO_CRYSTAL) {
                            htmltext = "31522-06a.html".to_owned();
                        } else {
                            htmltext = "31522-07a.html".to_owned();
                        }
                    }
                    6 => htmltext = "31522-10a.html".to_owned(),
                    20 => {
                        if memo_ex1 == 0 {
                            htmltext = "31522-15a.html".to_owned();
                        }
                    }
                    3 => {
                        if memo_ex1 == 0 {
                            qs.set_memo_state_ex(ctx, 1, 1);
                            htmltext = "31522-06b.html".to_owned();
                        } else if memo_ex1 == 1 {
                            qs.set_memo_state_ex(ctx, 1, 2);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "31522-07b.html".to_owned();
                        } else if memo_ex1 == 2 {
                            htmltext = "31522-08b.html".to_owned();
                        }
                    }
                    7 => {
                        if !ctx.has_quest_items(player, BLOOD_JEWEL) {
                            htmltext = "31522-12b.html".to_owned();
                        } else {
                            htmltext = "31522-13b.html".to_owned();
                        }
                    }
                    11 => htmltext = "31522-16b.html".to_owned(),
                    _ => {}
                },
                VICE_HIERARCH_MAO => {
                    if (40..=45).contains(&memo) {
                        htmltext = "32190-01.html".to_owned();
                    }
                }
                KATENAR => match memo {
                    6 => {
                        let c0 = link(ctx, npc.object_id, "player0");
                        let npc0 = link(ctx, npc.object_id, "npc0");
                        set_spawned(ctx, npc0, false);
                        if i64::from(player.0) == c0 {
                            htmltext = "32235-01a.html".to_owned();
                        } else {
                            htmltext = "32235-01z.html".to_owned();
                        }
                    }
                    20 => {
                        if memo_ex1 == 0 {
                            htmltext = "32235-09z.html".to_owned();
                        }
                    }
                    _ => {}
                },
                HARKILGAMED => match memo {
                    20 => {
                        let c0 = link(ctx, npc.object_id, "player0");
                        let npc0 = link(ctx, npc.object_id, "npc0");
                        set_spawned(ctx, npc0, false);
                        if i64::from(player.0) == c0 {
                            htmltext = "32236-01.html".to_owned();
                        } else {
                            htmltext = "32236-02.html".to_owned();
                        }
                    }
                    21 => htmltext = "32236-07z.html".to_owned(),
                    22 => htmltext = "32236-08z.html".to_owned(),
                    _ => {}
                },
                RODENPICULA => match memo {
                    40 => htmltext = "32237-01.html".to_owned(),
                    42 => htmltext = "32237-11a.html".to_owned(),
                    43 => htmltext = "32237-12.html".to_owned(),
                    44 => htmltext = "32237-14.html".to_owned(),
                    45 => htmltext = "32237-15.html".to_owned(),
                    _ => {}
                },
                ROCK => match memo {
                    20 => htmltext = "32238-01.html".to_owned(),
                    21 => htmltext = "32238-04.html".to_owned(),
                    22 => htmltext = "32238-05.html".to_owned(),
                    30 => htmltext = "32238-09.html".to_owned(),
                    _ => {}
                },
                MOTHER_NORNIL => match memo {
                    40 => htmltext = "32239-01.html".to_owned(),
                    42 => htmltext = "32239-02.html".to_owned(),
                    43 => htmltext = "32239-05.html".to_owned(),
                    44 => htmltext = "32239-06.html".to_owned(),
                    45 => htmltext = "32239-09.html".to_owned(),
                    _ => {}
                },
                KATENAR_A => match memo {
                    3 => {
                        if memo_ex1 == 2 {
                            let c0 = link(ctx, npc.object_id, "player0");
                            let npc0 = link(ctx, npc.object_id, "npc0");
                            set_spawned(ctx, npc0, false);
                            if i64::from(player.0) == c0 {
                                htmltext = "32332-01b.html".to_owned();
                            } else {
                                htmltext = "32332-01z.html".to_owned();
                            }
                        }
                    }
                    7 => {
                        if !ctx.has_quest_items(player, BLOOD_JEWEL) {
                            htmltext = "32332-05z.html".to_owned();
                        }
                    }
                    _ => {}
                },
                KATENAR_B => match memo {
                    7 => {
                        if ctx.has_quest_items(player, BLOOD_JEWEL) {
                            let c0 = link(ctx, npc.object_id, "player0");
                            let npc0 = link(ctx, npc.object_id, "npc0");
                            set_spawned(ctx, npc0, false);
                            // NOTE(java-quirk): both branches do the same; only the page differs.
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 10, true);
                            if i64::from(player.0) == c0 {
                                htmltext = "32333-06bz.html".to_owned();
                            } else {
                                htmltext = "32333-06b.html".to_owned();
                            }
                        }
                    }
                    11 => htmltext = "32333-06b.html".to_owned(),
                    _ => {}
                },
                HARKILGAMED_A => match memo {
                    22 => {
                        let c0 = link(ctx, npc.object_id, "player0");
                        let npc0 = link(ctx, npc.object_id, "npc0");
                        set_spawned(ctx, npc0, false);
                        if i64::from(player.0) == c0 {
                            htmltext = "32334-08.html".to_owned();
                        } else {
                            htmltext = "32334-09.html".to_owned();
                        }
                    }
                    30 => htmltext = "32334-18.html".to_owned(),
                    _ => {}
                },
                _ => {}
            }
        } else if state == COMPLETED && npc.id == HIERARCH_KEKROPUS {
            htmltext = ctx.get_already_completed_msg(player);
        }

        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            KATENAR => {
                let c0 = link(ctx, npc.object_id, "player0");
                ctx.start_quest_timer("KATENAR_120", 120_000, Some(npc), None, false);
                if c0 != 0 {
                    let name = ctx.player_name(ObjectId(c0 as i32));
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::S1FinallyWeMeet, &[&name]);
                }
            }
            HARKILGAMED => {
                ctx.start_quest_timer("HARKILGAMED_120", 120_000, Some(npc), None, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::HmmIsSomeoneApproaching,
                    &[],
                );
            }
            KATENAR_A => {
                let c0 = link(ctx, npc.object_id, "player0");
                ctx.start_quest_timer("KATENAR_A_120", 120_000, Some(npc), None, false);
                if c0 != 0 {
                    let name = ctx.player_name(ObjectId(c0 as i32));
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::S1DidYouWaitForLong,
                        &[&name],
                    );
                }
            }
            KATENAR_B => {
                let c0 = link(ctx, npc.object_id, "player0");
                ctx.start_quest_timer("KATENAR_B_120", 120_000, Some(npc), None, false);
                if c0 != 0 {
                    let name = ctx.player_name(ObjectId(c0 as i32));
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::DidYouBringWhatIAskedS1,
                        &[&name],
                    );
                }
            }
            HARKILGAMED_A => {
                let c0 = link(ctx, npc.object_id, "player0");
                ctx.start_quest_timer("HARKILGAMED_A_120", 120_000, Some(npc), None, false);
                if c0 != 0 {
                    let name = ctx.player_name(ObjectId(c0 as i32));
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::S1HasEverythingBeenFound,
                        &[&name],
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00236_SeedsOfChaos";
    const KAMAEL: i32 = 5;

    const NPCS: &str = r#"
        <npc id="32138" type="Folk" level="70" name="Hierarch Kekropus"/>
        <npc id="31522" type="Folk" level="70" name="Mysterious Wizard"/>
        <npc id="32235" type="Folk" level="70" name="Katenar"/>
        <npc id="32237" type="Folk" level="70" name="Rodenpicula"/>
        <npc id="21516" type="Monster" level="70" name="Needle Stakato Drone"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="729" type="EtcItem" name="Scroll: Enchant Weapon (A)"/>
        <item id="5011" type="EtcItem" name="Star of Destiny"/>
        <item id="9743" type="EtcItem" name="Shining Medallion"><set name="is_questitem" val="true"/></item>
        <item id="9744" type="EtcItem" name="Blood Jewel"><set name="is_questitem" val="true"/></item>
        <item id="9745" type="EtcItem" name="Black Echo Crystal"><set name="is_questitem" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Q00236SeedsOfChaos)
            .plain(&[
                "32138-01x.html",
                "32138-01y.html",
                "32138-01z.html",
                "32138-05.html",
            ])
            .page("32138-01.htm", &["32138-02.html"])
            .page("32138-02.html", &["32138-03.htm"])
            .page("32138-03.htm", &["32138-04.htm"])
            .plain(&["32138-04.htm"])
            .page("31522-01.html", &["31522-02.html"])
            .page("31522-02.html", &["31522-03.html"])
            .page("31522-03.html", &["31522-04a.html"])
            .page("31522-04a.html", &["31522-05a.html"])
            .plain(&[
                "31522-05a.html",
                "31522-06a.html",
                "31522-13a.html",
                "31522-14a.html",
            ])
            .page("31522-10a.html", &["31522-11a.html"])
            .page("31522-11a.html", &["31522-12a.html"])
            .plain(&["31522-12a.html", "32235-01a.html", "32235-01z.html"])
            .page("32237-15.html", &["32237-16.html"])
            .page("32237-16.html", &["32237-17.html"])
            .plain(&["32237-17.html"])
            .npcs(&[HIERARCH_KEKROPUS, MYSTERIOU_WIZARD, KATENAR, RODENPICULA])
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00236SeedsOfChaos);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let kekropus = t.spawn(HIERARCH_KEKROPUS, 20);
        let wizard = t.spawn(MYSTERIOU_WIZARD, 40);
        let rodenpicula = t.spawn(RODENPICULA, 60);
        (t, kekropus, wizard, rodenpicula)
    }

    fn kamael(t: &mut ScriptTest, id: i32, level: i32) {
        t.enter_as(
            id,
            level,
            KAMAEL,
            player_class::FIGHTER,
            vec![item(10 * id, STAR_OF_DESTINY, 1)],
        );
    }

    fn set_memo(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, memo);
            }
        });
    }

    #[test]
    fn only_a_kamael_of_level_75_with_the_star_is_offered_the_quest() {
        let (mut t, kekropus, _, _) = setup();
        kamael(&mut t, 1, 74);
        assert!(t.quest_page(1, kekropus).contains("32138-01z.html"));
        t.enter_as(2, 80, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, kekropus).contains("32138-01y.html"));
        t.enter_as(3, 80, KAMAEL, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(3, kekropus).contains("32138-01x.html"));
        kamael(&mut t, 4, 75);
        assert!(t.quest_page(4, kekropus).contains("32138-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_starts_with_memo_1_and_the_wizard_opens_the_first_path() {
        let (mut t, kekropus, wizard, _) = setup();
        kamael(&mut t, 1, 75);
        t.quest_page(1, kekropus);
        t.event(1, NAME, "32138-02.html");
        t.event(1, NAME, "32138-03.htm");
        assert!(t.event(1, NAME, "32138-04.htm").contains("32138-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kekropus).contains("32138-05.html"));
        assert!(t.quest_page(1, wizard).contains("31522-01.html"));
        t.event(1, NAME, "31522-02.html");
        t.event(1, NAME, "31522-03.html");
        t.event(1, NAME, "31522-04a.html");
        t.take_writes();
        assert!(t.event(1, NAME, "31522-05a.html").contains("31522-05a.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // The black echo crystal drops from the stakato drones only at memo 2.
        for i in 0..200 {
            if t.count(1, BLACK_ECHO_CRYSTAL) > 0 {
                break;
            }
            let drone = t.spawn(NEEDLE_STAKATO_DRONE, 100 + i);
            t.kill(1, drone);
        }
        assert_eq!(t.count(1, BLACK_ECHO_CRYSTAL), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));
    }

    #[test]
    fn katenar_is_summoned_once_and_leaves_after_120_seconds() {
        let (mut t, kekropus, wizard, _) = setup();
        kamael(&mut t, 1, 75);
        t.quest_page(1, kekropus);
        t.event(1, NAME, "32138-02.html");
        t.event(1, NAME, "32138-03.htm");
        t.event(1, NAME, "32138-04.htm");
        set_memo(&mut t, 6);
        assert!(t.quest_page(1, wizard).contains("31522-10a.html"));
        t.event(1, NAME, "31522-11a.html");
        assert!(t.event(1, NAME, "31522-12a.html").contains("31522-12a.html"));
        let katenars = t.npcs_of(KATENAR);
        assert_eq!(katenars.len(), 1);
        // The wizard remembers the player: `SPAWNED` is set on his own variables.
        assert!(
            t.world
                .npc_variables(wizard)
                .is_some_and(|v| v.get_bool("SPAWNED", false) && v.get_int("PLAYER_ID", 0) == 1)
        );
        // Katenar greets the player who summoned him.
        assert!(t.quest_page(1, katenars[0]).contains("32235-01a.html"));
        assert!(
            t.world
                .npc_variables(wizard)
                .is_some_and(|v| !v.get_bool("SPAWNED", false))
        );

        t.wait_ms(120_000);
        assert!(t.npcs_of(KATENAR).is_empty());
    }

    #[test]
    fn rodenpicula_gives_the_scroll_and_takes_the_star() {
        let (mut t, kekropus, _, rodenpicula) = setup();
        kamael(&mut t, 1, 75);
        t.quest_page(1, kekropus);
        t.event(1, NAME, "32138-02.html");
        t.event(1, NAME, "32138-03.htm");
        t.event(1, NAME, "32138-04.htm");
        set_memo(&mut t, 45);
        t.take_writes();
        assert!(t.quest_page(1, rodenpicula).contains("32237-15.html"));
        t.event(1, NAME, "32237-16.html");
        assert!(t.event(1, NAME, "32237-17.html").contains("32237-17.html"));
        assert_eq!(
            (
                t.count(1, SCROLL_ENCHANT_WEAPON_A_GRADE),
                t.count(1, STAR_OF_DESTINY)
            ),
            (1, 0)
        );
        assert_eq!(
            t.take_writes().last(),
            Some(&rows::update(NAME, "<state>", "Completed"))
        );
        assert!(
            t.quest_page(1, kekropus)
                .contains("This quest has already been completed.")
        );
    }
}
