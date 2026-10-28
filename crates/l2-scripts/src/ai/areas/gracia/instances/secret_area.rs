//! The Secret Area in the Keucereus Fortress: Ginby takes a character in, Lelrikia back out.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/SecretArea/SecretArea.java

use l2_util::ObjectId;
use l2_world::teleport::TeleportTarget;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GINBY: i32 = 32566;
const LELRIKIA: i32 = 32567;

// Locations
const TELEPORTS: [(i32, i32, i32); 2] = [(-23758, -8959, -5384), (-185057, 242821, 1576)];

// Misc
const TEMPLATE_ID: i32 = 117;
const ENTER: usize = 0;
const EXIT: usize = 1;

/// Java `ai.areas.Gracia.instances.SecretArea.SecretArea`.
pub struct SecretArea;

impl Script for SecretArea {
    fn name(&self) -> &'static str {
        "SecretArea"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/instances/SecretArea"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GINBY]);
        s.add_talk_id(&[GINBY]);
        s.add_talk_id(&[LELRIKIA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);
        if npc.id == GINBY && event.eq_ignore_ascii_case("enter") {
            // `enterInstance(player, TEMPLATE_ID)`; `onEnterInstance` adds the player to the world on
            // the first entrance and teleports him to the entrance of the instance.
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

                    let (x, y, z) = TELEPORTS[ENTER];
                    world.tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(instance_id));
                },
            );
            return Some("32566-01.html".to_owned());
        }

        if npc.id == LELRIKIA && event.eq_ignore_ascii_case("exit") {
            ctx.teleport_player(
                player,
                TELEPORTS[EXIT].0,
                TELEPORTS[EXIT].1,
                TELEPORTS[EXIT].2,
                0,
                false,
            );
            return Some("32567-01.html".to_owned());
        }

        Some(ctx.get_no_quest_msg(player))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32566" type="Folk" level="70" name="Ginby"/>
        <npc id="32567" type="Folk" level="70" name="Lelrikia"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/instances/SecretArea";

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(SecretArea);
        let ginby_page =
            r#"<html><body>ginby <a action="bypass -h Script SecretArea enter">x</a></body></html>"#;
        let lelrikia_page =
            r#"<html><body>lelrikia <a action="bypass -h Script SecretArea exit">x</a></body></html>"#;
        let pages = [
            ("data/html/default/32566.htm".to_owned(), ginby_page.to_owned()),
            ("data/html/default/32567.htm".to_owned(), lelrikia_page.to_owned()),
            (
                format!("{DIR}/32566-01.html"),
                "<html><body>32566-01</body></html>".to_owned(),
            ),
            (
                format!("{DIR}/32567-01.html"),
                "<html><body>32567-01</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let ginby = t.spawn(GINBY, 20);
        let lelrikia = t.spawn(LELRIKIA, 25);
        t.enter(1, 80, vec![]);
        (t, ginby, lelrikia)
    }

    #[test]
    fn ginby_takes_a_character_into_a_new_instance_of_template_117() {
        let (mut t, ginby, _) = setup();
        t.talk(1, ginby);
        t.take();
        t.bypass(1, "Script SecretArea enter");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32566-01"));
        assert!(
            names_to(&out, 1).iter().any(|n| n.contains("TELEPORT")),
            "{:?}",
            names_to(&out, 1)
        );
        // The character is allowed into a world of the instance made for him.
        let world = t.world.player_instance_world(ObjectId(1)).unwrap();
        assert!(world.is_allowed(ObjectId(1)));
        let instance_id = world.instance_id;
        assert!(t.world.instance(instance_id).is_some());
        // Asking again goes back into the same instance (not a first entrance).
        t.bypass(1, "Script SecretArea enter");
        assert_eq!(
            t.world.player_instance_world(ObjectId(1)).unwrap().instance_id,
            instance_id
        );
    }

    #[test]
    fn lelrikia_takes_a_character_out() {
        let (mut t, _, lelrikia) = setup();
        t.talk(1, lelrikia);
        t.take();
        t.bypass(1, "Script SecretArea exit");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32567-01"));
        assert!(names_to(&out, 1).iter().any(|n| n.contains("TELEPORT")));
    }
}
