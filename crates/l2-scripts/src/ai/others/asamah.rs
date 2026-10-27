//! Asamah, the Elrokian hunter of the Primeval Isle: a first-talk NPC script.
// Java: dist/game/data/scripts/ai/others/Asamah/Asamah.java
//
// The reference port of an NPC script for the batch workers (`docs/design/scripts.md`): it reads
// another quest's state by name, as Java's `player.getQuestState(Q00111_....class.getSimpleName())`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASAMAH: i32 = 32115;
/// Java `Q00111_ElrokianHuntersProof.class.getSimpleName()`.
const ELROKIAN_HUNTERS_PROOF: &str = "Q00111_ElrokianHuntersProof";

/// Java `ai.others.Asamah.Asamah`.
pub struct Asamah;

impl Script for Asamah {
    fn name(&self) -> &'static str {
        "Asamah"
    }

    fn path(&self) -> &'static str {
        "ai/others/Asamah"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ASAMAH]);
    }

    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        if event == "32115-03.htm" || event == "32115-04.htm" {
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let completed = ctx
            .player_quest_state(player, ELROKIAN_HUNTERS_PROOF)
            .is_some_and(|qs| qs.is_completed());
        Some(if completed { "32115-01.htm" } else { "32115-02.htm" }.to_owned())
    }
}
