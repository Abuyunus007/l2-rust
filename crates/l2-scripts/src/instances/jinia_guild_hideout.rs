//! The shared body of `JiniaGuildHideout1..4`: Rafforty sends a player of the Sirra quest chain
//! into the hideout instance. The four Java scripts differ in the quest, the condition on it and
//! the template only.
// Java: dist/game/data/scripts/instances/JiniaGuildHideout{1,2,3,4}/JiniaGuildHideout{1..4}.java

use l2_util::ObjectId;
use l2_world::{ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPC
pub(crate) const RAFFORTY: i32 = 32020;

// Location
const START_LOC: (i32, i32, i32) = (-23530, -8963, -5413);

/// How a hideout looks at the quest state of the player.
#[derive(Clone, Copy)]
pub(crate) enum Condition {
    /// `qs.isCond(n)`.
    Cond(i32),
    /// `qs.isMemoState(n)`.
    MemoState(i32),
}

/// What differs between the four hideouts.
pub(crate) struct HideoutData {
    /// `Qxxxxx.class.getSimpleName()`.
    pub quest: &'static str,
    pub condition: Condition,
    pub template_id: i32,
}

pub(crate) const PATH: &str = "instances/JiniaGuildHideout";

pub(crate) fn setup(s: &mut ScriptSetup) {
    s.add_start_npc(&[RAFFORTY]);
    s.add_talk_id(&[RAFFORTY]);
}

/// Java `onTalk`: the quest at the right step enters the instance and moves the quest on.
pub(crate) fn on_talk(
    d: &HideoutData,
    ctx: &mut ScriptCtx<'_>,
    _npc: ScriptNpc,
    talker: ObjectId,
) -> Option<String> {
    let qs = ctx.quest_state_of(d.quest, talker, false);
    if let Some(qs) = qs
        && match d.condition {
            Condition::Cond(n) => qs.is_cond(ctx, n),
            Condition::MemoState(n) => qs.is_memo_state(ctx, n),
        }
    {
        enter_instance(ctx, talker, d.template_id);
        qs.set_cond(ctx, 2, true);
    }

    // `super.onTalk(npc, talker)`: `Quest.onTalk` returns null.
    None
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32) {
    ctx.world_mut().enter_instance(
        player,
        template_id,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
            }
            // `teleportPlayer(player, START_LOC, world.getInstanceId(), false)`.
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
        },
    );
}

/// Writes the `impl Script` of one hideout:
/// `jinia_script!(JiniaGuildHideout1, "JiniaGuildHideout1", DATA);`.
macro_rules! jinia_script {
    ($ty:ident, $name:literal, $data:expr) => {
        impl l2_world::Script for $ty {
            fn name(&self) -> &'static str {
                $name
            }

            fn path(&self) -> &'static str {
                $crate::instances::jinia_guild_hideout::PATH
            }

            fn setup(&self, s: &mut l2_world::ScriptSetup) {
                $crate::instances::jinia_guild_hideout::setup(s);
            }

            fn on_talk(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                talker: l2_util::ObjectId,
            ) -> Option<String> {
                $crate::instances::jinia_guild_hideout::on_talk(&$data, ctx, npc, talker)
            }
        }
    };
}
pub(crate) use jinia_script;

/// The test every hideout runs: no quest, no entry; the right step enters and moves the quest to
/// cond 2; the wrong step does not enter again.
#[cfg(test)]
pub(crate) fn check_hideout(script: impl l2_world::Script + 'static, data: &HideoutData) {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry};

    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    /// The quest of the chain: a script of that name.
    struct StandIn(&'static str);
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/StandIn"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    let script_name = script.name();
    let pages = Pages::of(&script).file(
        "data/html/default/32020.htm",
        &format!(r#"<html><body><a action="bypass -h Script {script_name}">Let me in</a></body></html>"#),
    );
    let mut registry = ScriptRegistry::new();
    registry.add(StandIn(data.quest));
    registry.add(script);
    let mut t = ScriptTest::with_pages(
        registry,
        r#"<npc id="32020" type="Folk" level="70" name="Rafforty"/>"#,
        "",
        pages,
    );
    t.world.set_instance_data(instance_data(
        &[(data.template_id, "Jinia Guild Hideout")],
        &[(data.template_id, "")],
    ));
    let rafforty = t.spawn(RAFFORTY, 20);
    t.enter(1, 82, vec![]);

    // Without the quest: nothing happens.
    t.talk(1, rafforty);
    t.bypass(1, &format!("Script {}", script_name));
    t.ticks(60);
    assert_eq!(t.world.instance_id_of(Oid(1)), 0);

    // The quest at the right step: into a new instance, and the quest goes to cond 2.
    let condition = data.condition;
    t.world.run_as_script(data.quest, |ctx| {
        let qs = ctx.get_quest_state(Oid(1), true).unwrap();
        qs.start_quest(ctx);
        if let Condition::MemoState(n) = condition {
            qs.set_memo_state(ctx, n);
        }
    });
    t.take_writes();
    t.talk(1, rafforty);
    t.bypass(1, &format!("Script {}", script_name));
    t.ticks(60);
    let instance_id = t.world.instance_id_of(Oid(1));
    assert!(instance_id > 0);
    let world = t.world.instance_world(instance_id).unwrap();
    assert_eq!(
        (world.template_id, world.allowed.clone()),
        (data.template_id, vec![Oid(1)])
    );
    // The teleport lifts z by a few units (Java `Player.teleToLocation`).
    let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
    assert_eq!((x, y), (START_LOC.0, START_LOC.1));
    let mut cond = 0;
    t.world.run_as_script(data.quest, |ctx| {
        cond = ctx.get_quest_state(Oid(1), false).unwrap().get_cond(ctx);
    });
    assert_eq!(cond, 2);

    // The wrong step now: no second instance.
    t.world.run_as_script(data.quest, |ctx| {
        let qs = ctx.get_quest_state(Oid(1), false).unwrap();
        qs.set_cond(ctx, 3, false);
        qs.set_memo_state(ctx, 0);
    });
    t.talk(1, rafforty);
    t.bypass(1, &format!("Script {}", script_name));
    t.ticks(60);
    assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
}
