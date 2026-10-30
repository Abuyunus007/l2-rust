//! Elcadia's Tent (instance 158), the Seven Signs quests 10292 to 10296.
// Java: dist/game/data/scripts/instances/SSQElcadiasTent/SSQElcadiasTent.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const ELCADIA: i32 = 32784;
const GRUFF_LOOKING_MAN: i32 = 32862;

// Locations
const START_LOC: (i32, i32, i32) = (89797, -238081, -9632);
const EXIT_LOC: (i32, i32, i32) = (43347, -87923, -2820);

// Misc
const TEMPLATE_ID: i32 = 158;

// Quests it reads (`Class.getSimpleName()`).
const Q10292: &str = "Q10292_SevenSignsGirlOfDoubt";
const Q10293: &str = "Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom";
const Q10294: &str = "Q10294_SevenSignsToTheMonasteryOfSilence";
const Q10296: &str = "Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal";

/// Java `instances.SSQElcadiasTent.SSQElcadiasTent`.
pub struct SSQElcadiasTent;

impl Script for SSQElcadiasTent {
    fn name(&self) -> &'static str {
        "SSQElcadiasTent"
    }

    fn path(&self) -> &'static str {
        "instances/SSQElcadiasTent"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[GRUFF_LOOKING_MAN, ELCADIA]);
        s.add_start_npc(&[GRUFF_LOOKING_MAN, ELCADIA]);
        s.add_talk_id(&[GRUFF_LOOKING_MAN, ELCADIA]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if npc.id == GRUFF_LOOKING_MAN {
            let qs10292 = ctx.player_quest_state(talker, Q10292);
            let qs10293 = ctx.player_quest_state(talker, Q10293);
            let qs10294 = ctx.player_quest_state(talker, Q10294);
            let qs10296 = ctx.player_quest_state(talker, Q10296);
            let allowed = qs10292.is_some_and(|q| q.memo_state() > 1 && q.memo_state() < 9)
                || (qs10292.is_some_and(|q| q.is_completed()) && qs10293.is_none())
                || qs10293.is_some_and(|q| q.is_started())
                || (qs10293.is_some_and(|q| q.is_completed()) && qs10294.is_none())
                || qs10296.is_some_and(|q| q.memo_state() > 2 && q.memo_state() < 4);
            if allowed {
                enter_instance(ctx, talker);
            } else {
                return Some("32862-01.html".to_owned());
            }
        } else {
            // `world.removeAllowed(talker)`: a null world is a NullPointerException, which stops here.
            let instance_id = ctx.world().player_instance_world(talker).map(|w| w.instance_id)?;
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.remove_allowed(talker);
            }
            ctx.world_mut().set_instance_id(talker, 0);
            // `teleToLocation(EXIT_LOC)`: the Location carries instance 0.
            ctx.world_mut().tele_to_location(
                talker,
                TeleportTarget::at(EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2).with_instance(0),
            );
        }
        // `super.onTalk(npc, talker)`.
        None
    }
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance && let Some(instance_world) = world.instance_world_mut(instance_id) {
                instance_world.add_allowed(player);
            }
            // `teleportPlayer(player, START_LOC, id, false)`.
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{COMPLETED, STARTED};
    use l2_world::{ScriptRegistry, TeleportTarget};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest, X, Y, Z};

    const NPCS: &str = r#"
        <npc id="32784" type="Folk" level="80" name="Elcadia"/>
        <npc id="32862" type="Folk" level="80" name="Gruff-looking Man"/>"#;

    /// A stand-in for a quest this script reads: only its name matters.
    struct Quest(&'static str);

    impl Script for Quest {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Standin"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        for name in [Q10292, Q10293, Q10294, Q10296] {
            registry.add(Quest(name));
        }
        registry.add(SSQElcadiasTent);
        let pages = Pages::of(&SSQElcadiasTent).plain(&["32862-01.html"]).npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(158, "Elcadia's Tent")],
            &[(158, r#"<exitPoint x="43347" y="-87923" z="-2820" />"#)],
        ));
        let man = t.spawn(GRUFF_LOOKING_MAN, 20);
        let elcadia = t.spawn(ELCADIA, 40);
        (t, man, elcadia)
    }

    fn set_memo(t: &mut ScriptTest, quest: &'static str, state: u8, memo: i32) {
        t.world.run_as_script(quest, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, state);
            qs.set_memo_state(ctx, memo);
        });
        t.take_writes();
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let id = t.world.npc(npc).unwrap().npc_id;
        let mut result = None;
        t.world.run_as_script("SSQElcadiasTent", |ctx| {
            result = SSQElcadiasTent.on_talk(ctx, ScriptNpc { object_id: npc, id }, Oid(1));
        });
        t.ticks(60);
        result
    }

    #[test]
    fn without_the_right_quest_the_man_sends_the_player_away() {
        let (mut t, man, _) = setup();
        t.enter(1, 80, vec![]);
        assert_eq!(talk(&mut t, man), Some("32862-01.html".to_owned()));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        // Q10292 at memo 9 is outside 2..8.
        set_memo(&mut t, Q10292, STARTED, 9);
        assert_eq!(talk(&mut t, man), Some("32862-01.html".to_owned()));
    }

    #[test]
    fn the_quest_states_that_open_the_tent_make_an_instance_and_elcadia_leads_out() {
        let (mut t, man, elcadia) = setup();
        t.enter(1, 80, vec![]);
        set_memo(&mut t, Q10292, STARTED, 3);
        assert_eq!(talk(&mut t, man), None);
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (158, vec![Oid(1)]));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));

        // Elcadia takes the player out and he is not allowed any more.
        assert_eq!(talk(&mut t, elcadia), None);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (EXIT_LOC.0, EXIT_LOC.1));
        assert!(t.world.instance_world(instance_id).unwrap().allowed.is_empty());
    }

    #[test]
    fn the_other_quest_conditions() {
        let (mut t, man, _) = setup();
        t.enter(1, 80, vec![]);
        // Q10292 completed and no Q10293.
        set_memo(&mut t, Q10292, COMPLETED, 0);
        assert_eq!(talk(&mut t, man), None);
        assert_ne!(t.world.instance_id_of(Oid(1)), 0);
        // Out again; Q10293 completed is not enough once Q10294 exists.
        t.world
            .tele_to_location(Oid(1), TeleportTarget::at(X, Y, Z).with_instance(0));
        t.ticks(60);
        set_memo(&mut t, Q10293, COMPLETED, 0);
        set_memo(&mut t, Q10294, COMPLETED, 0);
        set_memo(&mut t, Q10292, STARTED, 1);
        assert_eq!(talk(&mut t, man), Some("32862-01.html".to_owned()));
        // Q10296 at memo 3.
        set_memo(&mut t, Q10296, STARTED, 3);
        assert_eq!(talk(&mut t, man), None);
    }
}
