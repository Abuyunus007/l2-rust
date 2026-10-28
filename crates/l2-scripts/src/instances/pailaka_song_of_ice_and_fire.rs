//! Pailaka Song of Ice and Fire (the Forgotten Temple), instance 43.
// Java: dist/game/data/scripts/instances/PailakaSongOfIceAndFire/PailakaSongOfIceAndFire.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

use super::ScriptValues;

// NPCs
const ADLER1: i32 = 32497;
const GARGOS: i32 = 18607;
const BLOOM: i32 = 18616;
const BOTTLE: i32 = 32492;
const BRAZIER: i32 = 32493;

// Items
const FIRE_ENHANCER: i32 = 13040;
const WATER_ENHANCER: i32 = 13041;
const SHIELD_POTION: i32 = 13032;
const HEAL_POTION: i32 = 13033;

// Location
const TELEPORT: (i32, i32, i32) = (-52875, 188232, -4696);

// Misc
const TEMPLATE_ID: i32 = 43;
const ZONE: i32 = 20108;

/// The `Npc._scriptValue` of the bottles and braziers (Java keeps it on the NPC).
#[derive(Default)]
struct State {
    values: ScriptValues,
}

/// Java `instances.PailakaSongOfIceAndFire.PailakaSongOfIceAndFire`.
pub struct PailakaSongOfIceAndFire;

impl Script for PailakaSongOfIceAndFire {
    fn name(&self) -> &'static str {
        "PailakaSongOfIceAndFire"
    }

    fn path(&self) -> &'static str {
        "instances/PailakaSongOfIceAndFire"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ADLER1]);
        s.add_talk_id(&[ADLER1]);
        s.add_attack_id(&[BOTTLE, BRAZIER]);
        s.add_exit_zone_id(&[ZONE]);
        s.add_spawn_id(&[BLOOM]);
        s.add_kill_id(&[BLOOM]);
        s.add_creature_see_id(&[GARGOS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "enter" => {
                if let Some(player) = player {
                    enter_instance(ctx, player);
                }
            }
            "GARGOS_LAUGH" => {
                if let Some(npc) = npc {
                    ctx.broadcast_npc_say(npc, ChatType::NpcShout, NpcStringId::OhhOhOh, &[]);
                }
            }
            "TELEPORT" => {
                if let Some(player) = player {
                    let instance_id = ctx.world().instance_id_of(player);
                    ctx.teleport_player(player, TELEPORT.0, TELEPORT.1, TELEPORT.2, instance_id, true);
                }
            }
            "DELETE" => {
                if let Some(npc) = npc {
                    ctx.state::<State>().values.forget(npc.object_id);
                    ctx.world_mut().delete_npc(npc.object_id);
                }
            }
            "BLOOM_TIMER" => {
                let delay = i64::from(ctx.get_random_range(2, 4)) * 60 * 1000;
                ctx.start_quest_timer("BLOOM_TIMER2", delay, npc, None, false);
            }
            "BLOOM_TIMER2" => {
                if let Some(npc) = npc {
                    let invisible = ctx.world().is_object_invisible(npc.object_id);
                    ctx.world_mut().set_invisible(npc.object_id, !invisible);
                    ctx.start_quest_timer("BLOOM_TIMER", 5000, Some(npc), None, false);
                }
            }
            _ => {}
        }
        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if damage > 0 && ctx.state::<State>().values.is(npc.object_id, 0) {
            match ctx.get_random(6) {
                0 => {
                    if npc.id == BOTTLE {
                        let count = i64::from(ctx.get_random_range(1, 6));
                        drop_item(ctx, npc, attacker, WATER_ENHANCER, count);
                    }
                }
                1 => {
                    if npc.id == BRAZIER {
                        let count = i64::from(ctx.get_random_range(1, 6));
                        drop_item(ctx, npc, attacker, FIRE_ENHANCER, count);
                    }
                }
                2 | 3 => {
                    let count = i64::from(ctx.get_random_range(1, 10));
                    drop_item(ctx, npc, attacker, SHIELD_POTION, count);
                }
                4 | 5 => {
                    let count = i64::from(ctx.get_random_range(1, 10));
                    drop_item(ctx, npc, attacker, HEAL_POTION, count);
                }
                _ => {}
            }

            ctx.state::<State>().values.set(npc.object_id, 1);
            ctx.start_quest_timer("DELETE", 3000, Some(npc), None, false);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let item = if ctx.get_random_boolean() {
            SHIELD_POTION
        } else {
            HEAL_POTION
        };
        let count = i64::from(ctx.get_random_range(1, 7));
        drop_item(ctx, npc, killer, item, count);
    }

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if is_live_player(ctx, creature) {
            let in_pailaka = ctx
                .world()
                .instance_world_of(creature)
                .is_some_and(|w| w.template_id == TEMPLATE_ID);
            if in_pailaka {
                ctx.start_quest_timer("TELEPORT", 1000, None, Some(creature), false);
            }
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() && ctx.state::<State>().values.is(npc.object_id, 0) {
            ctx.state::<State>().values.set(npc.object_id, 1);
            ctx.start_quest_timer("GARGOS_LAUGH", 1000, Some(npc), Some(creature), false);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_invisible(npc.object_id, true);
        ctx.start_quest_timer("BLOOM_TIMER", 1000, Some(npc), None, false);
    }
}

/// `creature.isPlayer() && !creature.isDead() && !creature.isTeleporting() &&
/// creature.asPlayer().isOnline()`.
fn is_live_player(ctx: &ScriptCtx<'_>, creature: ObjectId) -> bool {
    let world = ctx.world();
    world.player(creature).is_some() && !world.is_dead(creature) && !world.is_teleporting(creature)
}

/// `npc.dropItem(player, itemId, count)`.
fn drop_item(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, item_id: i32, count: i64) {
    ctx.world_mut()
        .npc_drop_item(npc.object_id, Some(player), item_id, count);
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let offset = ctx.world().max_offset_on_teleport();
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
            }
            world.tele_to_location(
                player,
                TeleportTarget::at(TELEPORT.0, TELEPORT.1, TELEPORT.2)
                    .with_instance(instance_id)
                    .with_random_offset(offset),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ClientInput, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z};

    const NPCS: &str = r#"
        <npc id="32497" type="Folk" level="70" name="Adler"/>
        <npc id="32492" type="Folk" level="70" name="Crystal Water Bottle"/>
        <npc id="32493" type="Folk" level="70" name="Burning Brazier"/>
        <npc id="18616" type="Folk" level="70" name="Bloom"/>"#;
    const ITEMS: &str = r#"
        <item id="13040" type="EtcItem" name="Fire Enhancer"/>
        <item id="13041" type="EtcItem" name="Water Enhancer"/>
        <item id="13032" type="EtcItem" name="Pailaka Instant Shield"/>
        <item id="13033" type="EtcItem" name="Quick Healing Potion"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(PailakaSongOfIceAndFire);
        let pages = [(
            "data/html/default/32497.htm",
            r#"<html><body>Adler <a action="bypass Script PailakaSongOfIceAndFire enter">Enter</a></body></html>"#,
        )];
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        t.world.set_instance_data(instance_data(
            &[(43, "Pailaka (Forgotten Temple)"), (44, "Pailaka (Devil's Isle)")],
            &[(43, r#"<exitPoint x="-80666" y="151321" z="-3040" />"#), (44, "")],
        ));
        let adler = t.spawn(ADLER1, 20);
        (t, adler)
    }

    fn position(t: &ScriptTest, player: i32) -> (i32, i32, i32) {
        t.world.position_of(Oid(player)).unwrap()
    }

    #[test]
    fn enter_makes_an_instance_and_a_second_enter_goes_back_into_it() {
        let (mut t, adler) = setup();
        t.enter(1, 40, vec![]);
        t.talk(1, adler);
        t.bypass(1, "Script PailakaSongOfIceAndFire enter");
        t.ticks(60);

        // A new dynamic instance of template 43 that allows the player.
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (43, vec![Oid(1)]));
        // `teleportPlayer(player, TELEPORT, instanceId)`: within MaxOffsetOnTeleport of the point.
        let (x, y, _) = position(&t, 1);
        let offset = t.world.max_offset_on_teleport();
        assert!((x - TELEPORT.0).abs() <= offset && (y - TELEPORT.1).abs() <= offset);

        // Out again, then "enter" once more: the same instance, no second one.
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(0));
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        t.talk(1, adler);
        t.bypass(1, "Script PailakaSongOfIceAndFire enter");
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }

    #[test]
    fn a_bottle_drops_once_and_goes_after_three_seconds() {
        let (mut t, _) = setup();
        t.enter(1, 40, vec![]);
        let bottles: Vec<Oid> = (0..16).map(|i| t.spawn(BOTTLE, 40 + i)).collect();
        for bottle in &bottles {
            let npc = ScriptNpc {
                object_id: *bottle,
                id: BOTTLE,
            };
            t.world.run_as_script("PailakaSongOfIceAndFire", |ctx| {
                // `damage > 0 && npc.isScriptValue(0)`; the second hit does nothing.
                PailakaSongOfIceAndFire.on_attack(ctx, npc, Oid(1), 10, false, None);
                PailakaSongOfIceAndFire.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
        }
        // One DELETE timer per bottle, not two.
        assert_eq!(t.world.quest_timer_count(), 16);
        let dropped = t.world.ground_item_ids();
        // 4 of 6 rolls always drop (a non stackable item `count` times), 1 of 6 for a bottle.
        assert!(!dropped.is_empty());
        for id in dropped {
            let item = t.world.ground_item(id).unwrap().data.item_id;
            assert!([WATER_ENHANCER, SHIELD_POTION, HEAL_POTION].contains(&item), "{item}");
        }
        t.ticks(59);
        assert!(bottles.iter().all(|b| t.world.npc(*b).is_some()));
        t.ticks(2);
        assert!(bottles.iter().all(|b| t.world.npc(*b).is_none()));
    }

    #[test]
    fn a_hit_of_no_damage_leaves_the_brazier_alone() {
        let (mut t, _) = setup();
        t.enter(1, 40, vec![]);
        let brazier = t.spawn(BRAZIER, 40);
        let npc = ScriptNpc {
            object_id: brazier,
            id: BRAZIER,
        };
        t.world.run_as_script("PailakaSongOfIceAndFire", |ctx| {
            PailakaSongOfIceAndFire.on_attack(ctx, npc, Oid(1), 0, false, None);
        });
        assert_eq!(t.world.quest_timer_count(), 0);
        assert!(t.world.ground_item_ids().is_empty());
    }

    #[test]
    fn bloom_hides_on_spawn_and_shows_up_again_after_two_to_four_minutes() {
        let (mut t, _) = setup();
        t.enter(1, 40, vec![]);
        let bloom = t.spawn(BLOOM, 40);
        let npc = ScriptNpc {
            object_id: bloom,
            id: BLOOM,
        };
        // `spawn_npc_at` is a GM spawn; the test tells the script the way the world does.
        t.world.run_as_script("PailakaSongOfIceAndFire", |ctx| {
            PailakaSongOfIceAndFire.on_spawn(ctx, npc);
        });
        assert!(t.world.is_object_invisible(bloom));
        // BLOOM_TIMER after a second, BLOOM_TIMER2 after 2 to 4 minutes: visible.
        t.ticks(20);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(4 * 60 * 20);
        assert!(!t.world.is_object_invisible(bloom));
    }

    #[test]
    fn a_player_who_leaves_the_zone_is_teleported_back_after_a_second() {
        let (mut t, adler) = setup();
        t.enter(1, 40, vec![]);
        t.talk(1, adler);
        t.bypass(1, "Script PailakaSongOfIceAndFire enter");
        t.ticks(60);
        // The client loaded the target (`Appearing`): the player is not teleporting any more.
        t.input(1, ClientInput::Appearing);
        t.world.run_as_script("PailakaSongOfIceAndFire", |ctx| {
            PailakaSongOfIceAndFire.on_exit_zone(ctx, Oid(1), ZONE);
        });
        assert_eq!(t.world.quest_timer_count(), 1);
        // Teleport away inside the instance, then the timer brings him back.
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(300_000));
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        assert_eq!(t.world.quest_timer_count(), 0);
        let (x, y, _) = position(&t, 1);
        let offset = t.world.max_offset_on_teleport();
        assert!((x - TELEPORT.0).abs() <= offset && (y - TELEPORT.1).abs() <= offset);

        // A player outside any instance is left alone.
        t.enter(2, 40, vec![]);
        t.world.run_as_script("PailakaSongOfIceAndFire", |ctx| {
            PailakaSongOfIceAndFire.on_exit_zone(ctx, Oid(2), ZONE);
        });
        assert_eq!(t.world.quest_timer_count(), 0);
    }
}
