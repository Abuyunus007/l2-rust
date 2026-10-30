//! Nornil's Garden Quest instant zone, template 12: the Kamael of `Q00236_SeedsOfChaos` meets
//! Mother Nornil.
// Java: dist/game/data/scripts/instances/NornilsGardenQuest/NornilsGardenQuest.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

const Q00236_SEEDS_OF_CHAOS: &str = "Q00236_SeedsOfChaos";

// NPCs
const RODENPICULA: i32 = 32237;
const MOTHER_NORNIL: i32 = 32239;

// Location
const ENTER_LOC: (i32, i32, i32) = (-119538, 87177, -12592);

// Misc
const TEMPLATE_ID: i32 = 12;

/// Java `instances.NornilsGardenQuest.NornilsGardenQuest`.
pub struct NornilsGardenQuest;

impl Script for NornilsGardenQuest {
    fn name(&self) -> &'static str {
        "NornilsGardenQuest"
    }

    fn path(&self) -> &'static str {
        "instances/NornilsGardenQuest"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RODENPICULA, MOTHER_NORNIL]);
        s.add_talk_id(&[RODENPICULA, MOTHER_NORNIL]);
        s.add_first_talk_id(&[RODENPICULA, MOTHER_NORNIL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        let q236 = ctx.quest_state_of(Q00236_SEEDS_OF_CHAOS, player, false);
        match event {
            "enter" => {
                if check_conditions(ctx, player) {
                    // `player.getLocation()`: the place, the heading and the instance.
                    let origin = {
                        let (x, y, z) = ctx.world().position_of(player).unwrap_or_default();
                        let heading = ctx.world().heading_of(player);
                        let instance = ctx.world().instance_id_of(player);
                        format!("{x};{y};{z};{heading};{instance}")
                    };
                    enter_instance(ctx, player);
                    // NOTE(java-quirk): `getPlayerWorld(player)` is dereferenced unchecked.
                    let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id)?;
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.parameters.insert("ORIGIN_LOC".to_owned(), origin);
                    }
                    if let Some(q236) = q236 {
                        q236.set_cond(ctx, 16, true);
                    }
                    htmltext = Some("32190-02.html".to_owned());
                } else {
                    htmltext = Some("32190-03.html".to_owned());
                }
            }
            "exit" => {
                if let Some(q236) = q236
                    && q236.is_completed(ctx)
                {
                    // NOTE(java-quirk): `getPlayerWorld(player)` is dereferenced unchecked.
                    let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id)?;
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.remove_allowed(player);
                    }
                    ctx.world_mut().finish_instance_in(instance_id, 5000);
                    ctx.world_mut().set_instance_id(player, 0);
                    let origin = ctx
                        .world()
                        .instance_world(instance_id)
                        .and_then(|w| w.parameters.get("ORIGIN_LOC"))
                        .and_then(|v| parse_location(v));
                    if let Some((x, y, z, instance)) = origin {
                        // `teleToLocation(loc)`: the location's own instance, no random offset.
                        ctx.teleport_player(player, x, y, z, instance, false);
                    }
                    htmltext = Some("32239-03.html".to_owned());
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        let completed = ctx
            .quest_state_of(Q00236_SEEDS_OF_CHAOS, player, false)
            .is_some_and(|q| q.is_completed(ctx));
        match npc.id {
            RODENPICULA => {
                htmltext = Some(if completed { "32237-02.html" } else { "32237-01.html" }.to_owned());
            }
            MOTHER_NORNIL => {
                htmltext = Some(if completed { "32239-02.html" } else { "32239-01.html" }.to_owned());
            }
            _ => {}
        }

        htmltext
    }
}

/// `x;y;z;heading;instance` of `ORIGIN_LOC` (the heading is kept for completeness: the world
/// teleport has no heading).
fn parse_location(value: &str) -> Option<(i32, i32, i32, i32)> {
    let mut parts = value.split(';').map(|p| p.parse::<i32>().ok());
    let x = parts.next()??;
    let y = parts.next()??;
    let z = parts.next()??;
    let _heading = parts.next()??;
    let instance = parts.next()??;
    Some((x, y, z, instance))
}

/// Java `checkConditions(player)`: the seeds of chaos at memo state 40 to 45.
fn check_conditions(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.player_quest_state(player, Q00236_SEEDS_OF_CHAOS)
        .is_some_and(|qs| (40..=45).contains(&qs.memo_state()))
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    // `checkConditions` of a new world is the one the caller just made.
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
                TeleportTarget::at(ENTER_LOC.0, ENTER_LOC.1, ENTER_LOC.2).with_instance(instance_id),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest, X, Y};

    const NPCS: &str = r#"
        <npc id="32237" type="Folk" level="70" name="Rodenpicula"/>
        <npc id="32239" type="Folk" level="70" name="Mother Nornil"/>"#;

    /// Seeds of Chaos: a script of that name.
    struct StandIn;
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            Q00236_SEEDS_OF_CHAOS
        }
        fn path(&self) -> &'static str {
            "quests/Q00236_SeedsOfChaos"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn);
        registry.add(NornilsGardenQuest);
        let pages = Pages::of(&NornilsGardenQuest)
            .plain(&["32237-01.html", "32237-02.html", "32239-01.html", "32239-03.html"])
            .page("32239-02.html", &["exit"])
            .plain(&["32190-02.html", "32190-03.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(12, "Nornil's Garden")],
            &[(12, r#"<exitPoint x="-111000" y="86000" z="-3000" />"#)],
        ));
        let rodenpicula = t.spawn(RODENPICULA, 20);
        let nornil = t.spawn(MOTHER_NORNIL, 30);
        (t, rodenpicula, nornil)
    }

    fn quest_at(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(Q00236_SEEDS_OF_CHAOS, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, memo);
        });
        t.take_writes();
    }

    fn page(t: &mut ScriptTest) -> String {
        crate::testing::pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    #[test]
    fn the_first_talk_depends_on_the_quest_being_completed() {
        let (mut t, rodenpicula, _) = setup();
        t.enter(1, 82, vec![]);
        t.talk(1, rodenpicula);
        t.ticks(5);
        assert!(page(&mut t).contains("32237-01.html"));
        t.world.run_as_script(Q00236_SEEDS_OF_CHAOS, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.talk(1, rodenpicula);
        t.ticks(5);
        assert!(page(&mut t).contains("32237-02.html"));
    }

    #[test]
    fn enter_needs_memo_state_40_to_45_and_remembers_where_the_player_came_from() {
        let (mut t, _, _) = setup();
        t.enter(1, 82, vec![]);
        // No state: the refusal page, nothing else.
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            let page = NornilsGardenQuest.on_event(ctx, "enter", None, Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("32190-03.html"));
        });
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);

        quest_at(&mut t, 46);
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            let page = NornilsGardenQuest.on_event(ctx, "enter", None, Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("32190-03.html"));
        });

        t.world.run_as_script(Q00236_SEEDS_OF_CHAOS, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 40);
        });
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            let page = NornilsGardenQuest.on_event(ctx, "enter", None, Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("32190-02.html"));
        });
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(1));
        assert!(instance_id > 0);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (12, vec![Oid(1)]));
        assert_eq!(world.parameters.get("ORIGIN_LOC").map(|v| v.starts_with(&format!("{X};{Y};"))), Some(true));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (ENTER_LOC.0, ENTER_LOC.1));
        let mut cond = 0;
        t.world.run_as_script(Q00236_SEEDS_OF_CHAOS, |ctx| {
            cond = ctx.get_quest_state(Oid(1), false).unwrap().get_cond(ctx);
        });
        assert_eq!(cond, 16);
    }

    #[test]
    fn exit_after_the_quest_brings_the_player_back_and_ends_the_instance() {
        let (mut t, _, _) = setup();
        t.enter(1, 82, vec![]);
        quest_at(&mut t, 40);
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            NornilsGardenQuest.on_event(ctx, "enter", None, Some(Oid(1)));
        });
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(1));

        // Not completed yet: `exit` does nothing.
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            assert_eq!(NornilsGardenQuest.on_event(ctx, "exit", None, Some(Oid(1))), None);
        });
        assert_eq!(t.world.instance_id_of(Oid(1)), instance_id);

        t.world.run_as_script(Q00236_SEEDS_OF_CHAOS, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.world.run_as_script("NornilsGardenQuest", |ctx| {
            let page = NornilsGardenQuest.on_event(ctx, "exit", None, Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("32239-03.html"));
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (X, Y));
        // The world is gone with the instance or no longer allows the player.
        assert!(t.world.instance_world(instance_id).is_none_or(|w| w.allowed.is_empty()));
    }
}
