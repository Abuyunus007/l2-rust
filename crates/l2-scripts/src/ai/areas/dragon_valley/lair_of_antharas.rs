//! Lair of Antharas AI: the Dragon Knights rise again, the guards return home, Knoriks shouts.
// Java: dist/game/data/scripts/ai/areas/DragonValley/LairOfAntharas.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TimerParams};

// NPC
const KNORIKS: i32 = 22857;
const DRAGON_KNIGHT: i32 = 22844;
const DRAGON_KNIGHT2: i32 = 22845;
const ELITE_DRAGON_KNIGHT: i32 = 22846;

const DRAGON_GUARD: i32 = 22852;
const DRAGON_MAGE: i32 = 22853;

// Misc
const KNIGHT_CHANCE: i32 = 30;
const KNORIKS_CHANCE: i32 = 60;
const KNORIKS_CHANCE2: i32 = 50;

/// Java `ai.areas.DragonValley.LairOfAntharas`.
pub struct LairOfAntharas;

impl Script for LairOfAntharas {
    fn name(&self) -> &'static str {
        "LairOfAntharas"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DragonValley"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[DRAGON_KNIGHT, DRAGON_KNIGHT2, DRAGON_GUARD, DRAGON_MAGE]);
        s.add_spawn_id(&[DRAGON_KNIGHT, DRAGON_KNIGHT2, DRAGON_GUARD, DRAGON_MAGE]);
        s.add_move_finished_id(&[DRAGON_GUARD, DRAGON_MAGE]);
        s.add_aggro_range_enter_id(&[KNORIKS]);
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _event: &str,
        _params: Option<&TimerParams>,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        if let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
            && ctx.world().is_object_spawned(npc.object_id)
        {
            if !ctx.is_in_combat(npc.object_id) {
                let at = ctx.npc_position(npc);
                let spawn = ctx.npc_spawn_point(npc);
                let far = match (at, spawn) {
                    (Some((x, y, _, _)), Some((sx, sy, _))) => {
                        let (dx, dy) = (f64::from(x - sx), f64::from(y - sy));
                        (dx * dx + dy * dy).sqrt() > 10.0
                    }
                    _ => false,
                };
                if far {
                    ctx.return_home(npc);
                } else if let (Some((_, _, _, heading)), Some(spawn_heading)) =
                    (at, ctx.world().npc_spawn_heading(npc.object_id))
                {
                    if heading != spawn_heading {
                        ctx.set_heading_and_validate(npc, spawn_heading);
                    }
                }
            }

            ctx.add_timer(
                &format!("CHECK_HOME_{}", npc.object_id.0),
                None,
                10_000,
                Some(npc),
                None,
            );
        }
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.is_script_value(npc, 0) && (ctx.get_random(100) < KNORIKS_CHANCE) {
            if ctx.get_random(100) < KNORIKS_CHANCE2 {
                ctx.set_script_value(npc, 1);
            }

            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::WhoSThereIfYouDisturbTheTemperOfTheGreatLandDragonAntharasIWillNeverForgiveYou,
                &[],
            );
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            DRAGON_KNIGHT => {
                if ctx.get_random(100) > KNIGHT_CHANCE {
                    let new_knight = ctx.add_spawn_at(DRAGON_KNIGHT2, npc.object_id, false, 0);
                    ctx.delete_npc(npc);
                    if let Some(new_knight) = new_knight {
                        ctx.broadcast_npc_say(
                            new_knight,
                            ChatType::NpcShout,
                            NpcStringId::ThoseWhoSetFootInThisPlaceShallNotLeaveAlive,
                            &[],
                        );
                        ctx.add_attack_desire(new_knight, killer, 999);
                    }
                }
            }
            DRAGON_KNIGHT2 => {
                if ctx.get_random(100) > KNIGHT_CHANCE {
                    let elite_knight = ctx.add_spawn_at(ELITE_DRAGON_KNIGHT, npc.object_id, false, 0);
                    ctx.delete_npc(npc);
                    if let Some(elite_knight) = elite_knight {
                        ctx.broadcast_npc_say(
                            elite_knight,
                            ChatType::NpcShout,
                            NpcStringId::IfYouWishToSeeHellIWillGrantYouYourWish,
                            &[],
                        );
                        ctx.add_attack_desire(elite_knight, killer, 999);
                    }
                }
            }
            DRAGON_GUARD | DRAGON_MAGE => {
                // NOTE(java-quirk): the timer is named "CHECK_HOME_<objectId>" in `onSpawn`
                // (and is a `getTimers()` timer), so this quest-timer cancel never finds it.
                ctx.cancel_quest_timer("CHECK_HOME", Some(npc), None);
            }
            _ => {}
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_on_kill_delay(npc.object_id, 0);
        if (npc.id == DRAGON_GUARD) || (npc.id == DRAGON_MAGE) {
            ctx.set_random_walking(npc, false);
            ctx.add_timer(
                &format!("CHECK_HOME_{}", npc.object_id.0),
                None,
                10_000,
                Some(npc),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22844" type="Monster" level="80" name="Dragon Knight"/>
        <npc id="22845" type="Monster" level="80" name="Dragon Knight"/>"#;

    #[test]
    fn a_dead_dragon_knight_is_replaced_by_the_next_rank_in_70_percent_of_the_kills() {
        let mut registry = ScriptRegistry::new();
        registry.add(LairOfAntharas);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let mut kills = 0;
        while t.npcs_of(DRAGON_KNIGHT2).is_empty() && kills < 60 {
            let knight = t.spawn(DRAGON_KNIGHT, 20);
            t.kill(1, knight);
            kills += 1;
        }
        let next = t.npcs_of(DRAGON_KNIGHT2);
        assert!(!next.is_empty(), "no Dragon Knight rose after {kills} kills");
    }
}
