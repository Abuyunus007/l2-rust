//! The delevel manager NPC (`DelevelManager.ini`): takes an item and puts the player back to
//! the start of the level below.
// Java: dist/game/data/scripts/custom/DelevelManager/DelevelManager.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

/// Java `custom.DelevelManager.DelevelManager`.
pub struct DelevelManager;

impl Script for DelevelManager {
    fn name(&self) -> &'static str {
        "DelevelManager"
    }

    fn path(&self) -> &'static str {
        "custom/DelevelManager"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The Java constructor registers `DelevelManagerConfig.DELEVEL_MANAGER_NPCID`, also when the
    // manager is switched off.
    fn setup_with(&self, s: &mut ScriptSetup, world: &l2_world::World) {
        let npc_id = world.custom_script_settings().delevel.npc_id;
        s.add_start_npc(&[npc_id]);
        s.add_talk_id(&[npc_id]);
        s.add_first_talk_id(&[npc_id]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let settings = ctx.world().custom_script_settings().delevel.clone();
        if !settings.enabled {
            return None;
        }
        let player = player?;

        if event == "delevel" {
            let level = ctx.player_level(player);
            if level <= settings.minimum_delevel {
                return Some("1002000-2.htm".to_owned());
            }

            if ctx.get_quest_items_count(player, settings.item_id) >= i64::from(settings.item_count) {
                ctx.take_items(player, settings.item_id, i64::from(settings.item_count));
                let exp = ctx.world().player(player).map_or(0, |p| p.data.exp);
                let remove = exp - ctx.world().experience_for_level(level - 1);
                ctx.world_mut().remove_exp_and_sp(player, remove, 0, true);
                ctx.broadcast_user_info(player);
                return Some("1002000.htm".to_owned());
            }

            return Some("1002000-1.htm".to_owned());
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("1002000.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::{CustomScriptSettings, DelevelManagerSettings};

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPC: i32 = 1_002_000;
    const NPCS: &str = r#"<npc id="1002000" type="Folk" level="70" name="Delevel"/>"#;
    const ITEMS: &str = r#"<item id="4356" type="EtcItem" name="Gold Coin"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/custom/DelevelManager";

    fn pages() -> Vec<(String, String)> {
        [
            ("1002000.htm", r#"<html><body>start <a action="bypass -h Script DelevelManager delevel">go</a></body></html>"#),
            ("1002000-1.htm", "<html><body>no item</body></html>"),
            ("1002000-2.htm", "<html><body>too low</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect()
    }

    fn scene(enabled: bool) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(DelevelManager);
        let pages = pages();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                delevel: DelevelManagerSettings {
                    enabled,
                    ..DelevelManagerSettings::default()
                },
                ..CustomScriptSettings::default()
            });
        });
        t.enter(1, 40, vec![item(100, 4356, 5)]);
        let npc = t.spawn(NPC, 20);
        (t, npc)
    }

    #[test]
    fn the_defaults_are_javas_and_off() {
        let d = DelevelManagerSettings::default();
        assert!(!d.enabled);
        assert_eq!((d.npc_id, d.item_id, d.item_count, d.minimum_delevel), (1_002_000, 4356, 2, 20));
    }

    #[test]
    fn switched_off_the_link_does_nothing() {
        let (mut t, npc) = scene(false);
        t.talk(1, npc);
        let out = t.take();
        assert!(pages_to(&out, 1)[0].contains("start"));
        t.bypass(1, "Script DelevelManager delevel");
        let out = t.take();
        assert!(pages_to(&out, 1).is_empty());
        assert_eq!(t.count(1, 4356), 5);
        assert_eq!(t.world.player(ObjectId(1)).unwrap().core.level, 40);
    }

    #[test]
    fn switched_on_it_takes_the_item_and_loses_the_exp_of_the_level() {
        let (mut t, npc) = scene(true);
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script DelevelManager delevel");
        let out = t.take();
        assert!(pages_to(&out, 1).iter().any(|p| p.contains("start")));
        assert_eq!(t.count(1, 4356), 3);
    }

    #[test]
    fn too_low_a_level_and_a_missing_item_answer_with_their_pages() {
        let (mut t, npc) = scene(true);
        t.world.run_as_script("DelevelManager", |ctx| {
            ctx.take_items(ObjectId(1), 4356, -1);
        });
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script DelevelManager delevel");
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("no item")));

        let mut registry = ScriptRegistry::new();
        registry.add(DelevelManager);
        let pages = pages();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut low = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                delevel: DelevelManagerSettings {
                    enabled: true,
                    ..DelevelManagerSettings::default()
                },
                ..CustomScriptSettings::default()
            });
        });
        low.enter(1, 20, vec![item(100, 4356, 5)]);
        let npc = low.spawn(NPC, 20);
        low.talk(1, npc);
        low.take();
        low.bypass(1, "Script DelevelManager delevel");
        assert!(pages_to(&low.take(), 1).iter().any(|p| p.contains("too low")));
        assert_eq!(low.count(1, 4356), 5);
    }
}
