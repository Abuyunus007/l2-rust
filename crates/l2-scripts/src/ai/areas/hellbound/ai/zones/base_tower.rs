//! The Base Tower: Guzen's death opens the doors and calls Kendal, the Body Destroyers curse
//! with Death Word.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Zones/BaseTower/BaseTower.java

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::script::player_class;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const GUZEN: i32 = 22362;
const KENDAL: i32 = 32301;
const BODY_DESTROYER: i32 = 22363;

// Skills
const DEATH_WORD: (i32, i32) = (5256, 1);

/// Java `BODY_DESTROYER_TARGET_LIST`.
#[derive(Default)]
struct BaseTowerState {
    body_destroyer_target_list: HashMap<ObjectId, ObjectId>,
}

/// Java `ai.areas.Hellbound.AI.Zones.BaseTower.BaseTower`.
pub struct BaseTower;

impl Script for BaseTower {
    fn name(&self) -> &'static str {
        "BaseTower"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/Zones/BaseTower"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[GUZEN]);
        s.add_kill_id(&[BODY_DESTROYER]);
        s.add_first_talk_id(&[KENDAL]);
        s.add_aggro_range_enter_id(&[BODY_DESTROYER]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let world = ctx.world();
        if world.player_class_equals_or_child_of(player, player_class::HELL_KNIGHT)
            || world.player_class_equals_or_child_of(player, player_class::SOULTAKER)
        {
            return Some("32301-02.htm".to_owned());
        }

        Some("32301-01.htm".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("CLOSE") {
            ctx.close_door(20260004, 0);
        }

        None
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        let first = {
            let list = &mut ctx.state::<BaseTowerState>().body_destroyer_target_list;
            if let std::collections::hash_map::Entry::Vacant(e) = list.entry(npc.object_id) {
                e.insert(player);
                true
            } else {
                false
            }
        };
        if first {
            ctx.world_mut().set_npc_target(npc.object_id, Some(player));
            ctx.world_mut()
                .do_simultaneous_cast(npc.object_id, SkillKey::new(DEATH_WORD.0, DEATH_WORD.1));
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        match npc.id {
            GUZEN => {
                // Should Kendal be despawned before Guzen's spawn? Or it will be crowd of Kendal's
                if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                    let heading = ctx.world().npc_spawn_heading(npc.object_id).unwrap_or(0);
                    let delay = i64::from(ctx.world().npc_spawn_respawn_delay_ms(npc.object_id));
                    ctx.add_spawn(KENDAL, x, y, z, heading, false, delay, 0);
                }
                ctx.open_door(20260003, 0);
                ctx.open_door(20260004, 0);
                ctx.start_quest_timer("CLOSE", 60000, Some(npc), None, false);
            }
            BODY_DESTROYER => {
                if let Some(pl) = ctx
                    .state::<BaseTowerState>()
                    .body_destroyer_target_list
                    .remove(&npc.object_id)
                    && ctx.world().player(pl).is_some()
                    && !ctx.is_dead(pl)
                {
                    ctx.world_mut()
                        .script_stop_skill_effects_removed(pl, DEATH_WORD.0);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="22362" type="Monster" level="80" name="Guzen"/>
        <npc id="22363" type="Monster" level="80" name="Body Destroyer"/>
        <npc id="32301" type="Folk" level="80" name="Kendal"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(BaseTower);
        let pages = Pages::of(&BaseTower).plain(&["32301-01.htm", "32301-02.htm"]);
        ScriptTest::with_pages(registry, NPCS, "", pages)
    }

    #[test]
    fn kendal_talks_by_class() {
        let mut t = setup();
        let kendal = crate::ai::areas::hellbound::testing::spawn(&mut t, KENDAL, 20);
        t.enter_as(1, 80, 0, player_class::DUELIST, vec![]);
        t.talk(1, kendal);
        assert!(last_page(&t.take(), 1).contains("32301-01.htm"));
        t.enter_as(2, 80, 0, player_class::HELL_KNIGHT, vec![]);
        t.talk(2, kendal);
        assert!(last_page(&t.take(), 2).contains("32301-02.htm"));
    }

    #[test]
    fn guzen_s_death_calls_kendal_and_the_close_timer() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        // Kendal appears where Guzen's spawn is: give him one.
        let spawn = t.world.script_add_hellbound_spawn(
            GUZEN,
            (crate::testing::X + 20, crate::testing::Y, crate::testing::Z),
            0,
            3600,
            0,
        );
        let guzen = t.world.script_do_spawn(spawn).unwrap();
        t.kill(1, guzen);
        assert_eq!(t.npcs_of(KENDAL).len(), 1);
        t.world.run_as_script("BaseTower", |ctx| {
            assert!(ctx.has_quest_timer("CLOSE", ctx.script_npc(guzen), None));
        });
    }

    #[test]
    fn a_body_destroyer_picks_one_target_and_forgets_it_at_death() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        let destroyer = crate::ai::areas::hellbound::testing::spawn(&mut t, BODY_DESTROYER, 20);
        t.world.run_as_script("BaseTower", |ctx| {
            let npc = ctx.script_npc(destroyer).unwrap();
            BaseTower.on_aggro_range_enter(ctx, npc, ObjectId(1), false);
            BaseTower.on_aggro_range_enter(ctx, npc, ObjectId(2), false);
            let state = ctx.state::<BaseTowerState>();
            assert_eq!(
                state.body_destroyer_target_list.get(&destroyer),
                Some(&ObjectId(1))
            );
        });
        t.kill(1, destroyer);
        t.world.run_as_script("BaseTower", |ctx| {
            assert!(
                ctx.state::<BaseTowerState>()
                    .body_destroyer_target_list
                    .is_empty()
            );
        });
    }
}
