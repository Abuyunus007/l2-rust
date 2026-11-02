//! The nobless master NPC (`NoblessMaster.ini`): noblesse for a level and an item.
// Java: dist/game/data/scripts/custom/NoblessMaster/NoblessMaster.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Item
const NOBLESS_TIARA: i32 = 7694;

/// Java `custom.NoblessMaster.NoblessMaster`.
pub struct NoblessMaster;

impl Script for NoblessMaster {
    fn name(&self) -> &'static str {
        "NoblessMaster"
    }

    fn path(&self) -> &'static str {
        "custom/NoblessMaster"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The Java constructor registers `NoblessMasterConfig.NOBLESS_MASTER_NPCID`, also when the
    // master is switched off.
    fn setup_with(&self, s: &mut ScriptSetup, world: &l2_world::World) {
        let npc_id = world.custom_script_settings().nobless.npc_id;
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
        let settings = ctx.world().custom_script_settings().nobless.clone();
        if !settings.enabled {
            return None;
        }
        let player = player?;

        if event == "noblesse" {
            if ctx.world().is_noble(player) {
                return Some("1003000-3.htm".to_owned());
            }

            if ctx.player_level(player) < settings.level_requirement {
                return Some("1003000-2.htm".to_owned());
            }

            if settings.item_count > 0 {
                if ctx.get_quest_items_count(player, settings.item_id) < settings.item_count {
                    let msg = SystemMessage::new(SystemMessageId::S2UnitSOfTheItemS1IsAreRequired)
                        .with(SystemMessageParam::ItemName(settings.item_id))
                        .with(SystemMessageParam::LongNumber(settings.item_count));
                    ctx.world().send(player, &msg);
                    return Some("1003000-4.htm".to_owned());
                }

                ctx.take_items(player, settings.item_id, settings.item_count);
            }

            if settings.reward_tiara {
                ctx.give_items(player, NOBLESS_TIARA, 1);
            }

            ctx.set_noble(player, true);
            ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
            return Some("1003000-1.htm".to_owned());
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("1003000.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::{CustomScriptSettings, NoblessMasterSettings};

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPC: i32 = 1_003_000;
    const NPCS: &str = r#"<npc id="1003000" type="Folk" level="70" name="Nobless"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7694" type="Armor" name="Tiara"/>"#;
    const DIR: &str = "data/scripts/custom/NoblessMaster";

    fn scene(settings: NoblessMasterSettings, level: i32, adena: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(NoblessMaster);
        let pages: Vec<(String, String)> = [
            ("1003000.htm", r#"<html><body>start <a action="bypass -h Script NoblessMaster noblesse">go</a></body></html>"#),
            ("1003000-1.htm", "<html><body>done</body></html>"),
            ("1003000-2.htm", "<html><body>level</body></html>"),
            ("1003000-3.htm", "<html><body>already</body></html>"),
            ("1003000-4.htm", "<html><body>item</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                nobless: settings,
                ..CustomScriptSettings::default()
            });
        });
        t.enter(1, level, vec![item(100, 57, adena)]);
        let npc = t.spawn(NPC, 20);
        (t, npc)
    }

    fn click(t: &mut ScriptTest, npc: ObjectId) -> Vec<String> {
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script NoblessMaster noblesse");
        pages_to(&t.take(), 1)
    }

    #[test]
    fn the_defaults_are_javas_and_off() {
        let d = NoblessMasterSettings::default();
        assert!(!d.enabled);
        assert_eq!((d.npc_id, d.level_requirement, d.item_id, d.item_count), (1_003_000, 80, 57, 0));
        assert!(!d.reward_tiara);
    }

    #[test]
    fn switched_off_nothing_changes() {
        let (mut t, npc) = scene(NoblessMasterSettings::default(), 85, 0);
        assert!(click(&mut t, npc).is_empty());
        assert!(!t.world.is_noble(ObjectId(1)));
    }

    #[test]
    fn switched_on_it_makes_a_noble_and_hands_the_tiara() {
        let settings = NoblessMasterSettings {
            enabled: true,
            item_count: 100,
            reward_tiara: true,
            ..NoblessMasterSettings::default()
        };
        let (mut t, npc) = scene(settings, 85, 500);
        let pages = click(&mut t, npc);
        assert!(pages.iter().any(|p| p.contains("done")), "{pages:?}");
        assert!(t.world.is_noble(ObjectId(1)));
        assert_eq!(t.count(1, 57), 400);
        assert_eq!(t.count(1, 7694), 1);
        // A noble is turned away.
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script NoblessMaster noblesse");
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("already")));
    }

    #[test]
    fn a_low_level_or_a_missing_item_is_refused() {
        let enabled = NoblessMasterSettings {
            enabled: true,
            item_count: 100,
            ..NoblessMasterSettings::default()
        };
        let (mut t, npc) = scene(enabled.clone(), 50, 500);
        assert!(click(&mut t, npc).iter().any(|p| p.contains("level")));
        let (mut t, npc) = scene(enabled, 85, 50);
        assert!(click(&mut t, npc).iter().any(|p| p.contains("item")));
        assert!(!t.world.is_noble(ObjectId(1)));
        assert_eq!(t.count(1, 57), 50);
    }
}
