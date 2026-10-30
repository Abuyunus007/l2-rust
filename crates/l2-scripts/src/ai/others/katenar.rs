//! Katenar, the Kamael who waits for the soul breaker candidate in "Certified Soul Breaker".
// Java: dist/game/data/scripts/ai/others/Katenar/Katenar.java
//
// Q00065 (S-QR-xx) spawns him and writes `npc0` (the NPC that spawned him) and `player0` (the
// player he waits for) to his variables as object ids, the way Q00184 does for its alarm.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KATENAR: i32 = 32242;

// Item
const SEALED_DOCUMENT: i32 = 9803;

/// Java `Q00065_CertifiedSoulBreaker.class.getSimpleName()`.
const CERTIFIED_SOUL_BREAKER: &str = "Q00065_CertifiedSoulBreaker";

/// Java `npc.getVariables().getObject(name, ...)`: the object id a quest stored (0 = unset).
fn object_var(ctx: &ScriptCtx<'_>, npc: ObjectId, name: &str) -> Option<ObjectId> {
    let id = ctx.world().npc_variables(npc)?.get_long(name, 0);
    i32::try_from(id).ok().filter(|id| *id != 0).map(ObjectId)
}

/// Java `Katenar` (`ai.others.Katenar`).
pub struct Katenar;

impl Script for Katenar {
    fn name(&self) -> &'static str {
        "Katenar"
    }

    fn path(&self) -> &'static str {
        "ai/others/Katenar"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KATENAR]);
        s.add_talk_id(&[KATENAR]);
        s.add_first_talk_id(&[KATENAR]);
        s.add_spawn_id(&[KATENAR]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): `npc.getVariables()` throws for a null NPC; nothing happens then.
        let npc = npc?;
        let npc0 = object_var(ctx, npc.object_id, "npc0");
        let spawned = ctx
            .world_mut()
            .npc_variables_mut(npc.object_id)
            .get_bool("SPAWNED", false);
        match event {
            "CREATED_50" => {
                if let Some(npc0) = npc0
                    && !spawned
                {
                    ctx.world_mut().npc_variables_mut(npc0).set_bool("SPAWNED", false);
                }
                ctx.delete_npc(npc);
            }
            "GOOD_LUCK" => {
                // NOTE(java-quirk): a missing quest state throws in Java.
                let qs = ctx.quest_state_of(CERTIFIED_SOUL_BREAKER, player?, false)?;
                if qs.is_memo_state(ctx, 14) {
                    if let Some(npc0) = npc0
                        && !spawned
                    {
                        ctx.world_mut().npc_variables_mut(npc0).set_bool("SPAWNED", false);
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::GoodLuck, &[]);
                    }
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        // NOTE(java-quirk): a missing quest state throws in Java (no page).
        let qs = ctx.quest_state_of(CERTIFIED_SOUL_BREAKER, talker, false)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let memo_state = qs.get_memo_state(ctx);
        if memo_state == 12 {
            htmltext = "32242-01.html".to_owned();
        } else if memo_state == 13 {
            let player = object_var(ctx, npc.object_id, "player0");
            if player == Some(talker) {
                qs.set_memo_state(ctx, 14);
                qs.set_cond(ctx, 13, true);
                htmltext = "32242-02.html".to_owned();
            } else {
                qs.set_memo_state(ctx, 14);
                qs.set_cond(ctx, 13, true);
                htmltext = "32242-03.html".to_owned();
            }

            // NOTE(java-quirk): the document goes to `player0`, not to the talker; without a
            // `player0` Java throws.
            if let Some(player) = player
                && !ctx.has_quest_items(player, SEALED_DOCUMENT)
            {
                ctx.give_items(player, SEALED_DOCUMENT, 1);
            }
        } else if memo_state == 14 {
            htmltext = "32242-04.html".to_owned();
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("CREATED_50", 50_000, Some(npc), None, false);
        if object_var(ctx, npc.object_id, "player0").is_some() {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::IAmLate, &[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32242" type="Folk" level="70" name="Katenar"/>"#;
    const ITEMS: &str = r#"<item id="9803" type="EtcItem" name="Sealed Document"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00065_CertifiedSoulBreaker` as far as Katenar reads it: a quest of that name.
    struct CertifiedSoulBreaker;

    impl Script for CertifiedSoulBreaker {
        fn name(&self) -> &'static str {
            CERTIFIED_SOUL_BREAKER
        }
        fn path(&self) -> &'static str {
            "quests/Q00065_CertifiedSoulBreaker"
        }
        fn id(&self) -> i32 {
            65
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Katenar);
        registry.add(CertifiedSoulBreaker);
        let pages =
            Pages::of(&Katenar).plain(&["32242-01.html", "32242-02.html", "32242-03.html", "32242-04.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let katenar = t.spawn(KATENAR, 20);
        (t, katenar)
    }

    /// Starts the stand-in quest of the player (when `start`) and sets its memo state.
    fn memo(t: &mut ScriptTest, player: i32, start: bool, memo_state: i32) {
        t.world.run_as_script(CERTIFIED_SOUL_BREAKER, |ctx| {
            let qs = ctx.get_quest_state(ObjectId(player), true).unwrap();
            if start {
                qs.start_quest(ctx);
            }
            qs.set_memo_state(ctx, memo_state);
        });
        t.take_writes();
    }

    fn page(t: &mut ScriptTest, player: i32, katenar: ObjectId) -> String {
        t.take();
        t.talk(player, katenar);
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn katenar_gives_the_document_to_player0_and_moves_the_memo_on() {
        let (mut t, katenar) = setup();
        t.enter(1, 70, vec![]);
        t.enter(2, 70, vec![]);
        // Q00065 stores who he waits for.
        t.world.npc_variables_mut(katenar).set_long("player0", 1);
        memo(&mut t, 1, true, 12);
        assert!(page(&mut t, 1, katenar).contains("32242-01.html"));

        memo(&mut t, 1, false, 13);
        assert!(page(&mut t, 1, katenar).contains("32242-02.html"));
        assert_eq!(t.count(1, SEALED_DOCUMENT), 1);
        assert!(page(&mut t, 1, katenar).contains("32242-04.html"));

        // Another talker gets the other page, the document still goes to player0.
        memo(&mut t, 2, true, 13);
        assert!(page(&mut t, 2, katenar).contains("32242-03.html"));
        assert_eq!(t.count(1, SEALED_DOCUMENT), 1);
        assert_eq!(t.count(2, SEALED_DOCUMENT), 0);
    }

    #[test]
    fn katenar_says_he_is_late_and_leaves_after_50_seconds() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        let katenar = t.spawn(KATENAR, 30);
        t.world.npc_variables_mut(katenar).set_long("player0", 1);
        t.take();
        // The variables are set after the spawn: run the hook again, as the engine does at a spawn.
        t.world.run_as_script("Katenar", |ctx| {
            let npc = ctx.script_npc(katenar).unwrap();
            Katenar.on_spawn(ctx, npc);
        });
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
        assert!(t.in_world(katenar));
        t.wait_ms(51_000);
        assert!(!t.in_world(katenar));
    }

    #[test]
    fn good_luck_waits_for_memo_state_14() {
        let (mut t, katenar) = setup();
        t.enter(1, 70, vec![]);
        memo(&mut t, 1, true, 13);
        let event = |t: &mut ScriptTest| {
            t.world.run_as_script("Katenar", |ctx| {
                let npc = ctx.script_npc(katenar).unwrap();
                Katenar.on_event(ctx, "GOOD_LUCK", Some(npc), Some(ObjectId(1)));
            });
        };
        event(&mut t);
        assert!(t.in_world(katenar));
        memo(&mut t, 1, false, 14);
        event(&mut t);
        assert!(!t.in_world(katenar));
    }
}
