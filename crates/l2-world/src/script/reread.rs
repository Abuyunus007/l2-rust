//! The D-50 re-read of `global_variables` before a boss or event script decides (S-API-04).
// Java: managers/GlobalVariablesManager.java (the table the Java GS shares, D-39)
//
// # Why
// With `CycleDatabaseWrites` off (D-53, D-54) this server never writes the global rows, and the
// Java GS (the real boss and event cycle) keeps changing them. A script that decides on one
// (`if (GlobalVariablesManager.getInstance().getLong("...Respawn", 0) > now)`) must see the
// current row, so it asks for a re-read first and decides in the continuation:
//
// ```ignore
// ctx.reread_global_variables(&["AntharasRespawn"], "decide", Some(npc), Some(player));
// // ... later, with the rows back, the world calls:
// fn on_event(&self, ctx, "decide", npc, player) { /* ctx.global_variables() is current */ }
// ```
//
// The continuation is the script's own `on_event` with the event name the script chose (its
// result is shown to the player like any `onEvent`), so no closure has to wait in the world.
// The read goes out through the outbox and the answer comes back as a world command (D-15):
// `PersistCommand::GlobalVariablesRereadRequest` and `WorldCommand::GlobalVariablesReread`.
//
// - With the switch ON this server is the only writer: its copy is the truth, nothing is read,
//   and the continuation runs after the current command.
// - A failed read refuses the action (D-50): the continuation does not run;
//   `Script::on_global_variables_reread_failed` is told instead (default: a log line).
// - A name the table no longer has is removed from the copy; a row the Java GS changed replaces
//   the copy. Neither marks the variable as changed, so nothing is written back.
// - The continuation is bound to the script, the NPC and the player by id; if the player left
//   meanwhile the script still runs with the id.

use std::collections::HashMap;

use l2_util::ObjectId;
use tracing::warn;

use super::api::ScriptNpc;
use super::ctx::ScriptCtx;
use super::notify::PendingEvent;
use super::setup::ScriptId;
use crate::outbox::PersistCommand;
use crate::world::World;

/// A re-read waiting for its rows.
#[derive(Debug, Clone)]
pub(crate) struct Reread {
    script: ScriptId,
    names: Vec<String>,
    event: String,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
}

/// The re-reads in flight.
#[derive(Debug, Default)]
pub(crate) struct Rereads {
    next_token: u64,
    waiting: HashMap<u64, Reread>,
}

impl Rereads {
    /// The number of re-reads waiting for their rows (tests).
    pub(crate) fn len(&self) -> usize {
        self.waiting.len()
    }
}

impl<'w> ScriptCtx<'w> {
    /// Re-reads the `global_variables` rows `names` (D-50) and then runs this script's
    /// `on_event(event, npc, player)`. See the module notes.
    pub fn reread_global_variables(
        &mut self,
        names: &[&str],
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        let script = self.script;
        let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
        let world = &mut *self.world;
        if world.scripts.settings.cycle_database_writes {
            world.scripts.pending.push_back(PendingEvent::RereadDone {
                script,
                event: event.to_owned(),
                npc,
                player,
                ok: true,
            });
            return;
        }
        let rereads = &mut world.scripts.rereads;
        rereads.next_token += 1;
        let token = rereads.next_token;
        rereads.waiting.insert(
            token,
            Reread {
                script,
                names: names.clone(),
                event: event.to_owned(),
                npc,
                player,
            },
        );
        world
            .outbox
            .persist(PersistCommand::GlobalVariablesRereadRequest { token, names });
    }
}

impl World {
    /// The answer of a re-read (`WorldCommand::GlobalVariablesReread`): the rows found, or
    /// `Err` when the read failed.
    pub(crate) fn on_global_variables_reread(
        &mut self,
        token: u64,
        result: Result<Vec<(String, Option<String>)>, ()>,
    ) {
        let Some(reread) = self.scripts.rereads.waiting.remove(&token) else {
            warn!("Global variables re-read {token} is not waiting for an answer.");
            return;
        };
        let ok = match result {
            Ok(rows) => {
                self.scripts.globals.apply_reread(&reread.names, rows);
                true
            }
            Err(()) => false,
        };
        self.scripts.pending.push_back(PendingEvent::RereadDone {
            script: reread.script,
            event: reread.event,
            npc: reread.npc,
            player: reread.player,
            ok,
        });
    }

    /// The queued end of a re-read: the continuation, or the failure.
    pub(crate) fn dispatch_reread_done(
        &mut self,
        script: ScriptId,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
        ok: bool,
    ) {
        if ok {
            self.script_notify_event(script, event, npc, player);
        } else {
            self.with_script(script, |s, ctx| {
                s.on_global_variables_reread_failed(ctx, event, npc, player)
            });
        }
    }

    /// The number of re-reads waiting for their rows (tests).
    pub fn script_rereads_waiting(&self) -> usize {
        self.scripts.rereads.len()
    }
}
