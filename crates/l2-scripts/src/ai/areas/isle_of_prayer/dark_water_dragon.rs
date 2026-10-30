//! The Dark Water Dragon: shades on the first hit and at half hp, Fafurion Kindred when it dies.
// Java: dist/game/data/scripts/ai/areas/IsleOfPrayer/DarkWaterDragon.java

use std::collections::{HashMap, HashSet};

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const DRAGON: i32 = 22267;
const SHADE1: i32 = 22268;
const SHADE2: i32 = 22269;
const FAFURION: i32 = 18482;
const DETRACTOR1: i32 = 22270;
const DETRACTOR2: i32 = 22271;
/// The template whose drop list Fafurion pays (`NpcData.getTemplate(18485)`).
const FAFURION_REWARD: i32 = 18485;

/// The three `static` sets of the Java class: the instances being tracked, those whose second
/// shades were spawned, and the player that killed the dragon of a Fafurion.
#[derive(Default)]
struct DragonState {
    /// `SECOND_SPAWN`: used to track if second Shades were already spawned.
    second_spawn: HashSet<ObjectId>,
    /// `MY_TRACKING_SET`: used to track instances of npcs.
    my_tracking_set: HashSet<ObjectId>,
    /// `ID_MAP`: used to track instances of npcs.
    id_map: HashMap<ObjectId, ObjectId>,
}

/// Java `ai.areas.IsleOfPrayer.DarkWaterDragon`.
pub struct DarkWaterDragon;

fn cancel(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, names: &[&str]) {
    for name in names {
        ctx.cancel_quest_timer(name, Some(npc), None);
    }
}

fn spawn_shade(ctx: &mut ScriptCtx<'_>, attacker: ObjectId, npc_id: i32, x: i32, y: i32, z: i32) {
    if let Some(shade) = ctx.add_spawn(npc_id, x, y, z, 0, false, 0, 0) {
        // `setRunning(); addDamageHate(attacker, 0, 999); setIntentionAttack(attacker)`.
        ctx.add_attack_desire(shade, attacker, 999);
    }
}

impl Script for DarkWaterDragon {
    fn name(&self) -> &'static str {
        "DarkWaterDragon"
    }

    fn path(&self) -> &'static str {
        "ai/areas/IsleOfPrayer"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let mobs = [DRAGON, SHADE1, SHADE2, FAFURION, DETRACTOR1, DETRACTOR2];
        s.add_kill_id(&mobs);
        s.add_attack_id(&mobs);
        s.add_spawn_id(&mobs);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        // The constructor clears both sets.
        let state = ctx.state::<DragonState>();
        state.my_tracking_set.clear();
        state.second_spawn.clear();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc {
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            match event.to_ascii_lowercase().as_str() {
                // Timers to start timers "1" to "4": a detractor every 40 seconds.
                "first_spawn" => ctx.start_quest_timer("1", 40000, Some(npc), None, true),
                "second_spawn" => ctx.start_quest_timer("2", 40000, Some(npc), None, true),
                "third_spawn" => ctx.start_quest_timer("3", 40000, Some(npc), None, true),
                "fourth_spawn" => ctx.start_quest_timer("4", 40000, Some(npc), None, true),
                "1" => {
                    ctx.add_spawn(DETRACTOR1, x + 100, y + 100, z, 0, false, 40000, 0);
                }
                "2" => {
                    ctx.add_spawn(DETRACTOR2, x + 100, y - 100, z, 0, false, 40000, 0);
                }
                "3" => {
                    ctx.add_spawn(DETRACTOR1, x - 100, y + 100, z, 0, false, 40000, 0);
                }
                "4" => {
                    ctx.add_spawn(DETRACTOR2, x - 100, y - 100, z, 0, false, 40000, 0);
                }
                // Fafurion Kindred disappears and drops reward.
                "fafurion_despawn" => {
                    cancel(ctx, npc, &["fafurion_poison", "1", "2", "3", "4"]);
                    let state = ctx.state::<DragonState>();
                    state.my_tracking_set.remove(&npc.object_id);
                    let removed = state.id_map.remove(&npc.object_id);
                    if let Some(removed) = removed {
                        ctx.world_mut().do_item_drop_template(
                            npc.object_id,
                            Some(FAFURION_REWARD),
                            Some(removed),
                        );
                    }
                    ctx.delete_npc(npc);
                }
                // Reduces Fafurions hp like it is poisoned.
                "fafurion_poison" => {
                    if ctx.npc_current_hp(npc) <= 500.0 {
                        cancel(
                            ctx,
                            npc,
                            &[
                                "fafurion_despawn",
                                "first_spawn",
                                "second_spawn",
                                "third_spawn",
                                "fourth_spawn",
                                "1",
                                "2",
                                "3",
                                "4",
                            ],
                        );
                        let state = ctx.state::<DragonState>();
                        state.my_tracking_set.remove(&npc.object_id);
                        state.id_map.remove(&npc.object_id);
                    }
                    // The poison kills Fafurion if he is not healed.
                    ctx.world_mut()
                        .reduce_current_hp(npc.object_id, 500.0, Some(npc.object_id));
                }
                _ => {}
            }
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if npc.id != DRAGON {
            return;
        }
        let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
        // `isSummon ? attacker.getSummon() : attacker`.
        let original = if is_summon {
            ctx.summon_of(attacker).unwrap_or(attacker)
        } else {
            attacker
        };
        // This allows to handle multiple instances of npc.
        if !ctx
            .state::<DragonState>()
            .my_tracking_set
            .contains(&npc.object_id)
        {
            ctx.state::<DragonState>().my_tracking_set.insert(npc.object_id);
            // Spawn first 5 shades on first attack on Dark Water Dragon.
            spawn_shade(ctx, original, SHADE1, x + 100, y + 100, z);
            spawn_shade(ctx, original, SHADE2, x + 100, y - 100, z);
            spawn_shade(ctx, original, SHADE1, x - 100, y + 100, z);
            spawn_shade(ctx, original, SHADE2, x - 100, y - 100, z);
            spawn_shade(ctx, original, SHADE1, x - 150, y + 150, z);
        } else if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) / 2.0
            && !ctx.state::<DragonState>().second_spawn.contains(&npc.object_id)
        {
            ctx.state::<DragonState>().second_spawn.insert(npc.object_id);
            // Spawn second 5 shades on half hp of on Dark Water Dragon.
            spawn_shade(ctx, original, SHADE2, x + 100, y + 100, z);
            spawn_shade(ctx, original, SHADE1, x + 100, y - 100, z);
            spawn_shade(ctx, original, SHADE2, x - 100, y + 100, z);
            spawn_shade(ctx, original, SHADE1, x - 100, y - 100, z);
            spawn_shade(ctx, original, SHADE2, x - 150, y + 150, z);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == DRAGON {
            let state = ctx.state::<DragonState>();
            state.my_tracking_set.remove(&npc.object_id);
            state.second_spawn.remove(&npc.object_id);
            // Spawns Fafurion Kindred when Dark Water Dragon is dead.
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            if let Some(faf) = ctx.add_spawn(FAFURION, x, y, z, 0, false, 0, 0) {
                ctx.state::<DragonState>().id_map.insert(faf.object_id, killer);
            }
        } else if npc.id == FAFURION {
            cancel(
                ctx,
                npc,
                &[
                    "fafurion_poison",
                    "fafurion_despawn",
                    "first_spawn",
                    "second_spawn",
                    "third_spawn",
                    "fourth_spawn",
                    "1",
                    "2",
                    "3",
                    "4",
                ],
            );
            let state = ctx.state::<DragonState>();
            state.my_tracking_set.remove(&npc.object_id);
            state.id_map.remove(&npc.object_id);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == FAFURION
            && !ctx
                .state::<DragonState>()
                .my_tracking_set
                .contains(&npc.object_id)
        {
            ctx.state::<DragonState>().my_tracking_set.insert(npc.object_id);
            // Spawn 4 Detractors on spawn of Fafurion.
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            ctx.add_spawn(DETRACTOR2, x + 100, y + 100, z, 0, false, 40000, 0);
            ctx.add_spawn(DETRACTOR1, x + 100, y - 100, z, 0, false, 40000, 0);
            ctx.add_spawn(DETRACTOR2, x - 100, y + 100, z, 0, false, 40000, 0);
            ctx.add_spawn(DETRACTOR1, x - 100, y - 100, z, 0, false, 40000, 0);
            ctx.start_quest_timer("first_spawn", 2000, Some(npc), None, false);
            ctx.start_quest_timer("second_spawn", 4000, Some(npc), None, false);
            ctx.start_quest_timer("third_spawn", 8000, Some(npc), None, false);
            ctx.start_quest_timer("fourth_spawn", 10000, Some(npc), None, false);
            ctx.start_quest_timer("fafurion_poison", 3000, Some(npc), None, true);
            ctx.start_quest_timer("fafurion_despawn", 120000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22267" type="Monster" level="70" name="Dark Water Dragon"><stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="22268" type="Monster" level="70" name="Shade"><stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="22269" type="Monster" level="70" name="Shade"><stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="22270" type="Monster" level="70" name="Detractor"><stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="22271" type="Monster" level="70" name="Detractor"><stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="18482" type="Monster" level="70" name="Fafurion Kindred"><stats><vitals hp="100000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(DarkWaterDragon);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        let dragon = t.spawn(DRAGON, 500);
        t.ticks(2);
        (
            t,
            ScriptNpc {
                object_id: dragon,
                id: DRAGON,
            },
        )
    }

    #[test]
    fn the_first_hit_calls_five_shades_once() {
        let (mut t, dragon) = setup();
        for _ in 0..2 {
            t.world.run_as_script("DarkWaterDragon", |ctx| {
                DarkWaterDragon.on_attack(ctx, dragon, ObjectId(1), 5, false, None);
            });
        }
        assert_eq!(t.npcs_of(SHADE1).len() + t.npcs_of(SHADE2).len(), 5);
        for id in t.npcs_of(SHADE1).into_iter().chain(t.npcs_of(SHADE2)) {
            t.world.delete_npc(id);
        }
    }

    #[test]
    fn the_dead_dragon_leaves_fafurion_with_detractors_who_goes_away_later() {
        let (mut t, dragon) = setup();
        t.world.run_as_script("DarkWaterDragon", |ctx| {
            DarkWaterDragon.on_kill(ctx, dragon, ObjectId(1), false);
        });
        t.ticks(2);
        let fafurion = t.npcs_of(FAFURION);
        assert_eq!(fafurion.len(), 1);
        assert_eq!(t.npcs_of(DETRACTOR1).len() + t.npcs_of(DETRACTOR2).len(), 4);
        // Another pair after the 2 s timer starts the 40 s ones, and the 2 min timer ends it.
        t.wait_ms(121_000);
        assert!(!t.in_world(fafurion[0]));
        for id in t.npcs_of(DETRACTOR1).into_iter().chain(t.npcs_of(DETRACTOR2)) {
            t.world.delete_npc(id);
        }
    }
}
