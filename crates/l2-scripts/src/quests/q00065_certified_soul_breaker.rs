//! Quest 65: Certified Soul Breaker (Grand Master Vitus, Kekropus, Casca, Holst, Katenar, ...).
// Java: dist/game/data/scripts/quests/Q00065_CertifiedSoulBreaker/Q00065_CertifiedSoulBreaker.java
//
// Pattern: a kill and delivery chain with the memo state as the step counter, sibling of
// `q00064_certified_berserker.rs`; the diamond reward is paid once per character. The guardian
// angel, the cargo box and Katenar (`ai/others/katenar.rs`) keep the player and the NPC they
// belong to on their variables as object ids (the way Q00184 does for its alarm).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{Qs, race};
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CAPTAIN_LUCAS: i32 = 30071;
const JACOB: i32 = 30073;
const GUARD_HARLAN: i32 = 30074;
const GUARD_XABER: i32 = 30075;
const GUARD_LIAM: i32 = 30076;
const GUARD_VESA: i32 = 30123;
const GUARD_ZEROME: i32 = 30124;
const WHARF_MANAGER_FELTON: i32 = 30879;
const KEKROPUS: i32 = 32138;
const VICE_HIERARCH_CASCA: i32 = 32139;
const GRAND_MASTER_HOLST: i32 = 32199;
const GRAND_MASTER_VITUS: i32 = 32213;
const GRAND_MASTER_MELDINA: i32 = 32214;
const KATENAR: i32 = 32242;
const CARGO_BOX: i32 = 32243;
const SUSPICIOUS_MAN: i32 = 32244;

// Items
const SEALED_DOCUMENT: i32 = 9803;
const WYRM_HEART: i32 = 9804;
const KEKROPUS_RECOMMENDATION: i32 = 9805;

// Reward
const DIMENSIONAL_DIAMOND: i32 = 7562;
const SOUL_BREAKER_CERTIFICATE: i32 = 9806;

// Monster
const WYRM: i32 = 20176;

// Quest Monster
const GUARDIAN_ANGEL: i32 = 27332;

// Misc
const MIN_LEVEL: i32 = 39;
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

// Locations
const SUSPICIOUS_SPAWN: (i32, i32, i32) = (16489, 146249, -3112);
const MOVE_TO: (i32, i32, i32) = (16490, 145839, -3080);

/// Java `npc.getVariables().getObject(name, ...)`: the object id a script stored (0 = unset).
fn object_var(ctx: &ScriptCtx<'_>, npc: ObjectId, name: &str) -> Option<ObjectId> {
    let id = ctx.world().npc_variables(npc)?.get_long(name, 0);
    i32::try_from(id).ok().filter(|id| *id != 0).map(ObjectId)
}

/// Java `npc.getVariables().set(name, object)`.
fn set_object_var(ctx: &mut ScriptCtx<'_>, npc: ObjectId, name: &str, object: ObjectId) {
    ctx.world_mut()
        .npc_variables_mut(npc)
        .set_long(name, i64::from(object.0));
}

/// Java `npc.getVariables().getBoolean("SPAWNED", false)`.
fn spawned(ctx: &ScriptCtx<'_>, npc: ObjectId) -> bool {
    ctx.world()
        .npc_variables(npc)
        .is_some_and(|v| v.get_bool("SPAWNED", false))
}

/// The cargo box is taken: `SPAWNED = true`, `PLAYER_ID = player`.
fn claim_box(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
    vars.set_bool("SPAWNED", true);
    vars.set_int("PLAYER_ID", player.0);
}

/// Java `npc.getVariables().getInt("PLAYER_ID")`.
fn box_owner(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> i32 {
    ctx.world()
        .npc_variables(npc.object_id)
        .map_or(0, |v| v.get_int("PLAYER_ID", 0))
}

/// The six guards share one block in Java, only the numbers differ. At memo state `from` the
/// first guard visited sets `memoStateEx(1)` to its `flag` (page 0), a repeated visit shows page 1,
/// and the second guard (it finds `other` set by the first one) moves to memo state `to` with
/// cond `to` (page 2); at memo state `to` the guard shows `after`.
struct Guard {
    from: i32,
    to: i32,
    flag: i32,
    other: i32,
    pages: [&'static str; 3],
    after: &'static str,
}

fn visit_guard(ctx: &mut ScriptCtx<'_>, qs: Qs, memo_state: i32, g: &Guard, htmltext: &mut String) {
    if memo_state == g.from {
        let ex = qs.get_memo_state_ex(ctx, 1);
        if ex == 0 {
            qs.set_memo_state_ex(ctx, 1, g.flag);
            *htmltext = g.pages[0].to_owned();
        } else if ex == g.flag {
            *htmltext = g.pages[1].to_owned();
        } else if ex == g.other {
            qs.set_memo_state(ctx, g.to);
            qs.set_memo_state_ex(ctx, 1, 0);
            qs.set_cond(ctx, g.to, true);
            *htmltext = g.pages[2].to_owned();
        }
    } else if memo_state == g.to {
        *htmltext = g.after.to_owned();
    }
}

/// Java `Q00065_CertifiedSoulBreaker`.
pub struct Q00065CertifiedSoulBreaker;

impl Script for Q00065CertifiedSoulBreaker {
    fn name(&self) -> &'static str {
        "Q00065_CertifiedSoulBreaker"
    }

    fn path(&self) -> &'static str {
        "quests/Q00065_CertifiedSoulBreaker"
    }

    fn id(&self) -> i32 {
        65
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GRAND_MASTER_VITUS]);
        s.add_talk_id(&[
            GRAND_MASTER_VITUS,
            CAPTAIN_LUCAS,
            JACOB,
            GUARD_HARLAN,
            GUARD_XABER,
            GUARD_LIAM,
            GUARD_VESA,
            GUARD_ZEROME,
            WHARF_MANAGER_FELTON,
            KEKROPUS,
            VICE_HIERARCH_CASCA,
            GRAND_MASTER_HOLST,
            GRAND_MASTER_MELDINA,
            KATENAR,
            CARGO_BOX,
            SUSPICIOUS_MAN,
        ]);
        s.add_kill_id(&[WYRM, GUARDIAN_ANGEL]);
        s.add_spawn_id(&[GUARDIAN_ANGEL, SUSPICIOUS_MAN]);
        s.register_quest_items(&[SEALED_DOCUMENT, WYRM_HEART, KEKROPUS_RECOMMENDATION]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN_5" {
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            return None;
        } else if event == "DESPAWN_70" {
            // NOTE(java-quirk): `npc.getVariables()` throws for a null NPC; nothing happens then.
            let npc = npc?;
            let npc0 = object_var(ctx, npc.object_id, "npc0");
            let c0 = object_var(ctx, npc.object_id, "player0");
            if let Some(npc0) = npc0
                && spawned(ctx, npc0)
            {
                ctx.world_mut().npc_variables_mut(npc0).set_bool("SPAWNED", false);
                if let Some(c0) = c0 {
                    let name = ctx.player_name(c0);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::S1IWillBeBackSoonStayThereAndDonTYouDareWanderOff,
                        &[&name],
                    );
                }
            }
            ctx.delete_npc(npc);
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 47);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("32213-05.htm".to_owned());
                    } else {
                        htmltext = Some("32213-06.htm".to_owned());
                    }
                }
            }
            "32213-09.html" => htmltext = Some(event.to_owned()),
            "32213-04.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL
                    && ctx
                        .world()
                        .is_in_category(player, CategoryType::KamaelSecondClassGroup)
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "30071-02.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30879-02.html" => {
                if qs.is_memo_state(ctx, 11) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30879-03.html" => {
                if qs.is_memo_state(ctx, 11) {
                    qs.set_memo_state(ctx, 12);
                    qs.set_cond(ctx, 12, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-02.html" | "32138-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-04.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-07.html" => {
                if qs.is_memo_state(ctx, 21) {
                    qs.set_memo_state(ctx, 22);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-10.html" | "32138-11.html" => {
                if qs.is_memo_state(ctx, 23) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-12.html" => {
                if qs.is_memo_state(ctx, 23) {
                    ctx.take_items(player, WYRM_HEART, -1);
                    ctx.give_items(player, KEKROPUS_RECOMMENDATION, 1);
                    qs.set_memo_state(ctx, 24);
                    qs.set_cond(ctx, 17, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32139-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32139-04.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32139-07.html" => {
                if qs.is_memo_state(ctx, 14) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32139-08.html" => {
                if qs.is_memo_state(ctx, 14) {
                    ctx.take_items(player, SEALED_DOCUMENT, -1);
                    qs.set_memo_state(ctx, 21);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32199-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    // `addSpawn(npc, SUSPICIOUS_MAN, SUSPICIOUS_SPAWN, false, 0)`: Holst is the summoner.
                    if let Some(holst) = npc {
                        let (x, y, z) = SUSPICIOUS_SPAWN;
                        ctx.add_spawn_summoned(holst, SUSPICIOUS_MAN, x, y, z, 0, false, 0, 0);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "32214-02.html" => {
                if qs.is_memo_state(ctx, 10) {
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 11, true);
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
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            WYRM => {
                if qs.is_memo_state(ctx, 22)
                    && ctx.give_item_randomly(killer, Some(npc), WYRM_HEART, 1, 1, 10, 0.20, true)
                {
                    qs.set_memo_state(ctx, 23);
                    qs.set_cond(ctx, 16, true);
                }
            }
            GUARDIAN_ANGEL => {
                let c0 = object_var(ctx, npc.object_id, "player0");
                let npc0 = object_var(ctx, npc.object_id, "npc0");
                if Some(killer) == c0 {
                    if qs.is_memo_state(ctx, 12) {
                        let (x, y, z) = ctx.world().position_of(killer).unwrap_or_default();
                        if let Some(katenar) = ctx.add_spawn(KATENAR, x + 20, y + 20, z, 0, false, 0, 0) {
                            set_object_var(ctx, katenar.object_id, "player0", killer);
                            // NOTE(java-quirk): Katenar's `npc0` is the angel, not the cargo box.
                            set_object_var(ctx, katenar.object_id, "npc0", npc.object_id);
                        }
                        qs.set_memo_state(ctx, 13);
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::GrrIVeBeenHit, &[]);
                    }
                } else {
                    if let Some(npc0) = npc0
                        && spawned(ctx, npc0)
                    {
                        ctx.world_mut().npc_variables_mut(npc0).set_bool("SPAWNED", false);
                    }
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::GrrWhoAreYouAndWhyHaveYouStoppedMe,
                        &[],
                    );
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == GRAND_MASTER_VITUS {
                if ctx.player_race(player) == race::KAMAEL {
                    if ctx.player_level(player) >= MIN_LEVEL
                        && ctx
                            .world()
                            .is_in_category(player, CategoryType::KamaelSecondClassGroup)
                    {
                        htmltext = "32213-01.htm".to_owned();
                    } else {
                        htmltext = "32213-03.html".to_owned();
                    }
                } else {
                    htmltext = "32213-02.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                GRAND_MASTER_VITUS => {
                    if memo_state == 1 {
                        htmltext = "32213-07.html".to_owned();
                    } else if memo_state > 1 && memo_state < 24 {
                        htmltext = "32213-08.html".to_owned();
                    } else if memo_state == 24 {
                        ctx.give_adena(player, 71_194, true);
                        ctx.give_items(player, SOUL_BREAKER_CERTIFICATE, 1);
                        ctx.add_exp_and_sp(player, 393_750, 27_020);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(player, 3);
                        htmltext = "32213-10.html".to_owned();
                    }
                }
                CAPTAIN_LUCAS => {
                    if memo_state == 7 {
                        htmltext = "30071-01.html".to_owned();
                    } else if memo_state == 8 {
                        htmltext = "30071-03.html".to_owned();
                    }
                }
                JACOB => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 6,
                        to: 7,
                        flag: 10,
                        other: 1,
                        pages: ["30073-01.html", "30073-01a.html", "30073-02.html"],
                        after: "30073-03.html",
                    },
                    &mut htmltext,
                ),
                GUARD_HARLAN => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 6,
                        to: 7,
                        flag: 1,
                        other: 10,
                        pages: ["30074-01.html", "30074-01a.html", "30074-02.html"],
                        after: "30074-03.html",
                    },
                    &mut htmltext,
                ),
                GUARD_XABER => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 8,
                        to: 9,
                        flag: 1,
                        other: 10,
                        pages: ["30075-01.html", "30075-01a.html", "30075-02.html"],
                        after: "30075-03.html",
                    },
                    &mut htmltext,
                ),
                GUARD_LIAM => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 8,
                        to: 9,
                        flag: 10,
                        other: 1,
                        pages: ["30076-01.html", "30076-01a.html", "30076-02.html"],
                        after: "30076-03.html",
                    },
                    &mut htmltext,
                ),
                // NOTE(java-quirk): Vesa and Zerome show the same page on the first and the repeated visit.
                GUARD_VESA => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 9,
                        to: 10,
                        flag: 10,
                        other: 1,
                        pages: ["30123-01.html", "30123-01.html", "30123-02.html"],
                        after: "30123-03.html",
                    },
                    &mut htmltext,
                ),
                GUARD_ZEROME => visit_guard(
                    ctx,
                    qs,
                    memo_state,
                    &Guard {
                        from: 9,
                        to: 10,
                        flag: 1,
                        other: 10,
                        pages: ["30124-01.html", "30124-01.html", "30124-02.html"],
                        after: "30124-03.html",
                    },
                    &mut htmltext,
                ),
                WHARF_MANAGER_FELTON => {
                    if memo_state == 11 {
                        htmltext = "30879-01.html".to_owned();
                    } else if memo_state == 12 {
                        htmltext = "30879-04.html".to_owned();
                    }
                }
                KEKROPUS => {
                    if memo_state == 1 {
                        let name = ctx.player_name(player);
                        return ctx
                            .get_htm(player, "32138-01.html")
                            .map(|page| page.replace("%name1%", &name));
                    } else if memo_state == 2 {
                        htmltext = "32138-05.html".to_owned();
                    } else if memo_state == 21 {
                        htmltext = "32138-06.html".to_owned();
                    } else if memo_state == 22 {
                        htmltext = "32138-08.html".to_owned();
                    } else if memo_state == 23 {
                        htmltext = "32138-09.html".to_owned();
                    } else if memo_state == 24 {
                        htmltext = "32138-13.html".to_owned();
                    }
                }
                VICE_HIERARCH_CASCA => {
                    if memo_state == 2 {
                        htmltext = "32139-01.html".to_owned();
                    } else if memo_state == 3 {
                        htmltext = "32139-03.html".to_owned();
                    } else if memo_state == 4 {
                        htmltext = "32139-05.html".to_owned();
                    } else if memo_state == 14 {
                        htmltext = "32139-06.html".to_owned();
                    } else if memo_state == 21 {
                        htmltext = "32139-09.html".to_owned();
                    }
                }
                GRAND_MASTER_HOLST => {
                    if memo_state == 4 {
                        htmltext = "32199-01.html".to_owned();
                    } else if memo_state == 5 {
                        qs.set_memo_state(ctx, 6);
                        qs.set_memo_state_ex(ctx, 1, 0); // Custom line
                        qs.set_cond(ctx, 6, true);
                        htmltext = "32199-03.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "32199-04.html".to_owned();
                    }
                }
                GRAND_MASTER_MELDINA => {
                    if memo_state == 10 {
                        htmltext = "32214-01.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "32214-03.html".to_owned();
                    }
                }
                CARGO_BOX => {
                    if memo_state == 12 {
                        if !spawned(ctx, npc.object_id) {
                            claim_box(ctx, npc, player);
                            if let Some(angel) =
                                ctx.add_spawn(GUARDIAN_ANGEL, 36110, 191921, -3712, 0, true, 0, 0)
                            {
                                set_object_var(ctx, angel.object_id, "npc0", npc.object_id);
                                set_object_var(ctx, angel.object_id, "player0", player);
                                ctx.add_attack_desire(angel, player, 999);
                            }
                            htmltext = "32243-01.html".to_owned();
                        } else if box_owner(ctx, npc) == player.0 {
                            htmltext = "32243-03.html".to_owned();
                        } else {
                            htmltext = "32243-02.html".to_owned();
                        }
                    } else if memo_state == 13 {
                        if !spawned(ctx, npc.object_id) {
                            claim_box(ctx, npc, player);
                            if let Some(katenar) =
                                ctx.add_spawn(KATENAR, 36110, 191921, -3712, 0, false, 0, 0)
                            {
                                set_object_var(ctx, katenar.object_id, "player0", player);
                                set_object_var(ctx, katenar.object_id, "npc0", npc.object_id);
                            }
                            htmltext = "32243-06.html".to_owned();
                        } else if box_owner(ctx, npc) == player.0 {
                            htmltext = "32243-04.html".to_owned();
                        } else {
                            htmltext = "32243-05.html".to_owned();
                        }
                    } else if memo_state == 14 {
                        htmltext = "32243-07.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == GRAND_MASTER_VITUS {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == SUSPICIOUS_MAN {
            ctx.start_quest_timer("DESPAWN_5", 5000, Some(npc), None, false);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::DratsHowCouldIBeSoWrong,
                &[],
            );
            ctx.set_running(npc);
            ctx.npc_move_to(npc, MOVE_TO.0, MOVE_TO.1, MOVE_TO.2);
        } else if npc.id == GUARDIAN_ANGEL {
            let c0 = object_var(ctx, npc.object_id, "player0");
            ctx.start_quest_timer("DESPAWN_70", 70_000, Some(npc), None, false);
            if let Some(c0) = c0 {
                let name = ctx.player_name(c0);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::S1StepBackFromTheConfoundedBoxIWillTakeItMyself,
                    &[&name],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00065_CertifiedSoulBreaker";

    const NPCS: &str = r#"
        <npc id="30073" type="Folk" level="70" name="Jacob"/>
        <npc id="30074" type="Folk" level="70" name="Harlan"/>
        <npc id="32138" type="Folk" level="70" name="Kekropus"/>
        <npc id="32199" type="Folk" level="70" name="Holst"/>
        <npc id="32213" type="Folk" level="70" name="Vitus"/>
        <npc id="32243" type="Folk" level="70" name="Cargo Box"/>
        <npc id="32244" type="Folk" level="70" name="Suspicious Man"/>
        <npc id="20176" type="Monster" level="40" name="Wyrm"/>
        <npc id="27332" type="Monster" level="40" name="Guardian Angel"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="9804" type="EtcItem" name="Wyrm Heart"><set name="is_questitem" val="true"/></item>
        <item id="9805" type="EtcItem" name="Kekropus' Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="9806" type="EtcItem" name="Soul Breaker Certificate"/>"#;

    struct Npcs {
        vitus: Oid,
        kekropus: Oid,
        jacob: Oid,
        harlan: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00065CertifiedSoulBreaker);
        let pages = Pages::of(&Q00065CertifiedSoulBreaker)
            .page("32213-01.htm", &["32213-04.htm"])
            .page("32213-04.htm", &["ACCEPT"])
            .plain(&[
                "32213-02.html",
                "32213-03.html",
                "32213-05.htm",
                "32213-06.htm",
                "32213-10.html",
                "30073-01.html",
                "30073-01a.html",
                "30073-02.html",
                "30074-01.html",
                "30074-02.html",
                "32138-04.html",
                "32243-01.html",
            ])
            .file(
                "data/scripts/quests/Q00065_CertifiedSoulBreaker/32138-01.html",
                "<html><body>Hello %name1%</body></html>",
            )
            .npcs(&[GRAND_MASTER_VITUS, KEKROPUS, JACOB, GUARD_HARLAN, CARGO_BOX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            vitus: t.spawn(GRAND_MASTER_VITUS, 20),
            kekropus: t.spawn(KEKROPUS, 40),
            jacob: t.spawn(JACOB, 60),
            harlan: t.spawn(GUARD_HARLAN, 80),
        };
        (t, npcs)
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00065CertifiedSoulBreaker.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00065CertifiedSoulBreaker.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_kamael_second_class_of_level_39_starts_it() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::KAMAEL, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.vitus).contains("32213-03.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, npcs.vitus).contains("32213-03.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.vitus).contains("32213-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_pays_47_diamonds_once_and_kekropus_greets_by_name() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::WARDER, vec![]);
        assert!(t.quest_page(1, npcs.vitus).contains("32213-01.htm"));
        assert!(t.event(1, NAME, "32213-04.htm").contains("32213-04.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("32213-05.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 47);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        let page = talk(&mut t, npcs.kekropus).unwrap();
        assert!(page.contains("Hello") && !page.contains("%name1%"));
        assert_eq!(
            ev(&mut t, "32138-04.html", None).as_deref(),
            Some("32138-04.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
    }

    #[test]
    fn the_guards_of_a_pair_need_both_visits() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::TROOPER, vec![]);
        t.quest_page(1, npcs.vitus);
        t.event(1, NAME, "32213-04.htm");
        t.event(1, NAME, "ACCEPT");
        memo(&mut t, 6);
        assert_eq!(talk(&mut t, npcs.jacob).as_deref(), Some("30073-01.html"));
        assert_eq!(talk(&mut t, npcs.jacob).as_deref(), Some("30073-01a.html"));
        assert_eq!(talk(&mut t, npcs.harlan).as_deref(), Some("30074-02.html"));
        assert_eq!(talk(&mut t, npcs.jacob).as_deref(), Some("30073-03.html"));
        assert!(t.take_writes().contains(&rows::update(NAME, "memoState", "7")));
    }

    #[test]
    fn the_certificate_pays_and_the_cargo_box_calls_the_angel() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::TROOPER, vec![]);
        t.quest_page(1, npcs.vitus);
        t.event(1, NAME, "32213-04.htm");
        t.event(1, NAME, "ACCEPT");

        let cargo = t.spawn(CARGO_BOX, 100);
        memo(&mut t, 12);
        assert_eq!(talk(&mut t, cargo).as_deref(), Some("32243-01.html"));
        assert_eq!(t.npcs_of(GUARDIAN_ANGEL).len(), 1);
        assert_eq!(talk(&mut t, cargo).as_deref(), Some("32243-03.html"));
        for angel in t.npcs_of(GUARDIAN_ANGEL) {
            t.world.delete_npc(angel);
        }

        memo(&mut t, 24);
        assert_eq!(talk(&mut t, npcs.vitus).as_deref(), Some("32213-10.html"));
        assert_eq!(t.count(1, SOUL_BREAKER_CERTIFICATE), 1);
        assert_eq!(t.count(1, 57), 71_194);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert_eq!(
            w,
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
