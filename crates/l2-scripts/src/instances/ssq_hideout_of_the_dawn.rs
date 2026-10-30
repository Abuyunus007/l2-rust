//! Hideout of the Dawn (instance 113), the Seven Signs quest 198 (Embryo).
// Java: dist/game/data/scripts/instances/SSQHideoutOfTheDawn/SSQHideoutOfTheDawn.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const WOOD: i32 = 32593;
const JAINA: i32 = 32617;

// Location
const WOOD_LOC: (i32, i32, i32) = (-23758, -8959, -5384);
const JAINA_LOC: (i32, i32, i32) = (147072, 23743, -1984);

// Misc
const TEMPLATE_ID: i32 = 113;

/// Java `instances.SSQHideoutOfTheDawn.SSQHideoutOfTheDawn`.
pub struct SSQHideoutOfTheDawn;

impl Script for SSQHideoutOfTheDawn {
    fn name(&self) -> &'static str {
        "SSQHideoutOfTheDawn"
    }

    fn path(&self) -> &'static str {
        "instances/SSQHideoutOfTheDawn"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        // NOTE(java-quirk): Jaina has a first-talk listener but `onFirstTalk` is not overridden,
        // so her page (32617.html) is never shown by the script.
        s.add_first_talk_id(&[JAINA]);
        s.add_start_npc(&[WOOD]);
        s.add_talk_id(&[WOOD, JAINA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "32617-01.html" | "32617-02a.html" => {
                htmltext = Some(event.to_owned());
            }
            "32617-02.html" => {
                // `player.getObjectId()` of a null player is a NullPointerException.
                let player = player?;
                ctx.world_mut().set_instance_id(player, 0);
                // `teleToLocation(JAINA_LOC, true)`: the Location carries instance 0.
                let offset = ctx.world().max_offset_on_teleport();
                ctx.world_mut().tele_to_location(
                    player,
                    TeleportTarget::at(JAINA_LOC.0, JAINA_LOC.1, JAINA_LOC.2)
                        .with_instance(0)
                        .with_random_offset(offset),
                );
                htmltext = Some(event.to_owned());
            }
            "32593-01.html" => {
                let player = player?;
                enter_instance(ctx, player);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance && let Some(instance_world) = world.instance_world_mut(instance_id) {
                instance_world.add_allowed(player);
            }
            // `teleportPlayer(player, WOOD_LOC, id, false)`.
            world.tele_to_location(
                player,
                TeleportTarget::at(WOOD_LOC.0, WOOD_LOC.1, WOOD_LOC.2).with_instance(instance_id),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32617" type="Folk" level="80" name="Jaina"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SSQHideoutOfTheDawn);
        let pages = Pages::of(&SSQHideoutOfTheDawn)
            .page("32593-01.html", &["32617-02.html"])
            .plain(&["32617-01.html", "32617-02.html", "32617-02a.html"])
            .npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(113, "Hideout of the Dawn")],
            &[(113, r#"<exitPoint x="147072" y="23743" z="-1984" />"#)],
        ));
        t
    }

    fn event(t: &mut ScriptTest, name: &str) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("SSQHideoutOfTheDawn", |ctx| {
            result = SSQHideoutOfTheDawn.on_event(ctx, name, None, Some(Oid(1)));
        });
        t.ticks(60);
        result
    }

    #[test]
    fn the_pages_come_back_and_wood_leads_in_and_jaina_out() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        assert_eq!(event(&mut t, "32617-01.html").as_deref(), Some("32617-01.html"));
        assert_eq!(event(&mut t, "32617-02a.html").as_deref(), Some("32617-02a.html"));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);

        // Enter: a new instance of template 113, the player allowed and at Wood's place.
        assert_eq!(event(&mut t, "32593-01.html").as_deref(), Some("32593-01.html"));
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (113, vec![Oid(1)]));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (WOOD_LOC.0, WOOD_LOC.1));

        // Jaina: out of the instance, to her place (within the teleport offset).
        assert_eq!(event(&mut t, "32617-02.html").as_deref(), Some("32617-02.html"));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        let offset = t.world.max_offset_on_teleport();
        assert!((x - JAINA_LOC.0).abs() <= offset && (y - JAINA_LOC.1).abs() <= offset);
        // Still allowed: a second visit goes back into the same instance.
        event(&mut t, "32593-01.html");
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }

    #[test]
    fn an_unknown_event_gives_nothing() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        assert_eq!(event(&mut t, "other"), None);
    }
}
