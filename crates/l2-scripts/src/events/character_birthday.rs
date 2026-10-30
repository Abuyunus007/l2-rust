//! Character Birthday: the gatekeepers call Alegria, who swaps the Adventurer's Hat for the birthday hat.
// Java: dist/game/data/scripts/events/CharacterBirthday/CharacterBirthday.java
//
// Not a `LongTimeEvent` (it has no `config.xml`): it extends `Script` and is always on.
//
// Left out: `onDailyReset` (the birthday mail). It needs a `SELECT .. FROM characters` for the
// characters whose birthday was in the last 24 hours and `MailManager.sendMessage` with an
// attached item (Alegria as the sender). The world has neither, and the engine cannot run a
// database query for a script (D-15): its own card (see the report of S-EVENT-01).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ALEGRIA: i32 = 32600;
const GATEKEEPERS: [i32; 19] = [
    30006, 30059, 30080, 30134, 30146, 30177, 30233, 30256, 30320, 30540, 30576, 30836, 30848, 30878, 30899,
    31275, 31320, 31964, 32163,
];

/// Java `private static int SPAWNS`: the Alegrias standing now.
#[derive(Default)]
struct Spawns(i32);

/// Java `events.CharacterBirthday.CharacterBirthday`.
pub struct CharacterBirthday;

impl Script for CharacterBirthday {
    fn name(&self) -> &'static str {
        "CharacterBirthday"
    }

    fn path(&self) -> &'static str {
        "events/CharacterBirthday"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALEGRIA]);
        s.add_start_npc(&GATEKEEPERS);
        s.add_talk_id(&[ALEGRIA]);
        s.add_talk_id(&GATEKEEPERS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = Some(event.to_owned());
        if event.eq_ignore_ascii_case("despawn_npc") {
            let npc = npc?;
            ctx.do_die(npc, player);
            ctx.shared_state::<Spawns>().0 -= 1;

            htmltext = None;
        } else if event.eq_ignore_ascii_case("change") {
            // Change Hat
            if ctx.has_quest_items(player, 10250) {
                ctx.take_items(player, 10250, 1); // Adventurer Hat (Event)
                ctx.give_items(player, 21594, 1); // Birthday Hat
                htmltext = None; // FIXME: Probably has html

                // Despawn npc
                let npc = npc?;
                ctx.do_die(npc, player);
                ctx.shared_state::<Spawns>().0 -= 1;
            } else {
                htmltext = Some("32600-nohat.htm".to_owned());
            }
        }

        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.shared_state::<Spawns>().0 >= 3 {
            return Some("busy.htm".to_owned());
        }

        if !ctx.check_if_in_range(10, npc.object_id, player, true) {
            let (x, y, z) = ctx.world().position_of(player)?;
            let spawned = ctx.add_spawn(32600, x + 10, y + 10, z + 20, 0, false, 0, 0);
            ctx.start_quest_timer("despawn_npc", 180_000, spawned, Some(player), false);
            ctx.shared_state::<Spawns>().0 += 1;
        } else {
            return Some("tooclose.htm".to_owned());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"
        <npc id="30006" type="Folk" level="70" name="Gatekeeper"/>
        <npc id="32600" type="Folk" level="70" name="Alegria"/>"#;
    const ITEMS: &str = r#"
        <item id="10250" type="Armor" name="Adventurer's Hat (Event)"/>
        <item id="21594" type="Armor" name="Birthday Hat"/>"#;
    const GATEKEEPER: i32 = 30006;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(CharacterBirthday);
        let pages = Pages::of(&CharacterBirthday)
            .plain(&["busy.htm", "tooclose.htm", "32600-nohat.htm"])
            .npcs(&[GATEKEEPER, ALEGRIA]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    fn talk_to(t: &mut ScriptTest, gatekeeper: ObjectId) -> Option<String> {
        let mut page = None;
        t.world.run_as_script("CharacterBirthday", |ctx| {
            let npc = ctx.script_npc(gatekeeper).unwrap();
            page = CharacterBirthday.on_talk(ctx, npc, ObjectId(1));
        });
        page
    }

    #[test]
    fn a_gatekeeper_far_enough_calls_alegria_and_three_is_the_most() {
        let mut t = setup();
        t.enter(1, 40, vec![]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        for _ in 0..3 {
            assert_eq!(talk_to(&mut t, gatekeeper), None);
        }
        assert_eq!(t.npcs_of(ALEGRIA).len(), 3);
        assert_eq!(talk_to(&mut t, gatekeeper).as_deref(), Some("busy.htm"));
        assert_eq!(t.npcs_of(ALEGRIA).len(), 3);
    }

    #[test]
    fn a_gatekeeper_too_close_does_not() {
        let mut t = setup();
        t.enter(1, 40, vec![]);
        let gatekeeper = t.spawn(GATEKEEPER, 5);
        assert_eq!(talk_to(&mut t, gatekeeper).as_deref(), Some("tooclose.htm"));
        assert!(t.npcs_of(ALEGRIA).is_empty());
    }

    #[test]
    fn alegria_leaves_after_three_minutes() {
        let mut t = setup();
        t.enter(1, 40, vec![]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        talk_to(&mut t, gatekeeper);
        let alegria = t.npcs_of(ALEGRIA)[0];
        t.wait_ms(180_100);
        assert!(!t.in_world(alegria) || t.world.is_dead(alegria));
        // The counter went down: three more can be called.
        for _ in 0..3 {
            assert_eq!(talk_to(&mut t, gatekeeper), None);
        }
    }

    #[test]
    fn the_hat_is_changed_for_the_birthday_hat() {
        let mut t = setup();
        t.enter(1, 40, vec![item(1000, 10250, 1)]);
        let gatekeeper = t.spawn(GATEKEEPER, 20);
        talk_to(&mut t, gatekeeper);
        let alegria = ScriptNpc {
            object_id: t.npcs_of(ALEGRIA)[0],
            id: ALEGRIA,
        };
        t.world.run_as_script("CharacterBirthday", |ctx| {
            assert_eq!(
                CharacterBirthday.on_event(ctx, "change", Some(alegria), Some(ObjectId(1))),
                None
            );
        });
        assert_eq!((t.count(1, 10250), t.count(1, 21594)), (0, 1));

        // Without the hat: the page says so.
        t.enter(2, 40, vec![]);
        t.world.run_as_script("CharacterBirthday", |ctx| {
            assert_eq!(
                CharacterBirthday
                    .on_event(ctx, "change", Some(alegria), Some(ObjectId(2)))
                    .as_deref(),
                Some("32600-nohat.htm")
            );
        });
    }
}
