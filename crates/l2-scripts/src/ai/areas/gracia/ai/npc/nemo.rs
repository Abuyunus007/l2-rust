//! Nemo, the Great Magician of Oren in the Seed of Annihilation: the Maguen Plasma Collector and a
//! practice Maguen.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/Nemo/Nemo.java

use std::collections::{HashMap, HashSet};

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NEMO: i32 = 32735; // Nemo
const MAGUEN: i32 = 18839; // Wild Maguen

// Items
const COLLECTOR: i32 = 15487; // Maguen Plasma Collector

// Misc
const MAXIMUM_MAGUEN: i32 = 18; // Maximum maguens in one time

/// Java's `npc.getScriptValue()` of Nemo, the player variable `TEST_MAGUEN` and the
/// `SPAWNED_NPC` variable of the practice Maguens.
#[derive(Default)]
struct NemoState {
    script_value: HashMap<ObjectId, i32>,
    /// Players with `getVariables().getInt("TEST_MAGUEN", 0) == 1`.
    // NOTE(java-quirk): Java stores this in the player's variables, which survive a logout; a
    // player who leaves during the practice stays blocked there. Here it is forgotten with the
    // server (the player variables are not in the world yet).
    practicing: HashSet<ObjectId>,
    /// Maguen -> the Nemo that summoned it.
    spawned_npc: HashMap<ObjectId, ObjectId>,
}

/// Java `ai.areas.Gracia.AI.NPC.Nemo.Nemo`.
pub struct Nemo;

impl Script for Nemo {
    fn name(&self) -> &'static str {
        "Nemo"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/Nemo"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NEMO]);
        s.add_first_talk_id(&[NEMO]);
        s.add_talk_id(&[NEMO]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "32735-01.html" => {
                htmltext = Some(event.to_owned());
            }
            "giveCollector" => {
                let player = player?;
                if ctx.has_quest_items(player, COLLECTOR) {
                    htmltext = Some("32735-03.html".to_owned());
                } else if !ctx.world().script_is_inventory_under_90(player, false) {
                    htmltext = Some("32735-04.html".to_owned());
                } else {
                    htmltext = Some("32735-02.html".to_owned());
                    ctx.give_items(player, COLLECTOR, 1);
                }
            }
            "summonMaguen" => {
                let (npc, player) = (npc?, player?);
                let state = ctx.state::<NemoState>();
                let practicing = state.practicing.contains(&player);
                let value = state.script_value.get(&npc.object_id).copied().unwrap_or(0);
                if !practicing && value < MAXIMUM_MAGUEN {
                    let maguen = ctx.add_spawn_at(MAGUEN, npc.object_id, true, 60000)?;
                    let state = ctx.state::<NemoState>();
                    state.spawned_npc.insert(maguen.object_id, npc.object_id);
                    state.practicing.insert(player);
                    state.script_value.insert(npc.object_id, value + 1);
                    let name = ctx.player_name(player);
                    let world = ctx.world_mut();
                    world.script_set_npc_title(maguen.object_id, &name);
                    world.script_set_running(maguen.object_id, true);
                    world.script_start_follow(maguen.object_id, player);
                    world.broadcast_status_update(maguen.object_id);
                    ctx.show_on_screen_npc_string(player, NpcStringId::MaguenAppearance, 2, 4000, &[]);
                    // `SUMMON_PLAYER`, `TEST_MAGUEN` and `DIST_CHECK_TIMER` of the Maguen script.
                    ctx.notify_event_of("Maguen", "PRACTICE_MAGUEN", Some(maguen), Some(player));
                    htmltext = Some("32735-05.html".to_owned());
                } else {
                    htmltext = Some("32735-06.html".to_owned());
                }
            }
            "DECREASE_COUNT" => {
                // `npc` is the practice Maguen here.
                let (npc, player) = (npc?, player?);
                let state = ctx.state::<NemoState>();
                if let Some(spawned) = state.spawned_npc.remove(&npc.object_id) {
                    let value = state.script_value.get(&spawned).copied().unwrap_or(0);
                    if value > 0 {
                        state.practicing.remove(&player);
                        state.script_value.insert(spawned, value - 1);
                    }
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::gracia::ai::maguen::Maguen;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32735" type="Folk" level="70" name="Nemo"/>
        <npc id="18839" type="Monster" level="80" name="Wild Maguen"/>"#;
    const ITEMS: &str = r#"<item id="15487" type="EtcItem" name="Maguen Plasma Collector"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/Nemo";

    fn setup(items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Nemo);
        registry.add(Maguen);
        let links: String = ["giveCollector", "summonMaguen"]
            .iter()
            .map(|e| format!(r#"<a action="bypass -h Script Nemo {e}">x</a>"#))
            .collect();
        let mut pages = vec![(format!("{DIR}/32735.html"), format!("<html><body>32735 {links}</body></html>"))];
        for n in 1..=6 {
            pages.push((format!("{DIR}/32735-0{n}.html"), format!("<html><body>32735-0{n} {links}</body></html>")));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(NEMO, 20);
        t.enter(1, 80, items);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32735 "));
        (t, npc)
    }

    #[test]
    fn the_collector_is_given_once() {
        let (mut t, _) = setup(vec![]);
        t.bypass(1, "Script Nemo giveCollector");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32735-02"));
        assert_eq!(t.count(1, COLLECTOR), 1);
        t.bypass(1, "Script Nemo giveCollector");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32735-03"));
        assert_eq!(t.count(1, COLLECTOR), 1);
    }

    #[test]
    fn one_practice_maguen_per_character_until_it_is_gone() {
        let (mut t, _) = setup(vec![item(1000, 15487, 1)]);
        let before = t.world.npc_count();
        t.bypass(1, "Script Nemo summonMaguen");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32735-05"));
        assert!(names_to(&out, 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        assert_eq!(t.world.npc_count(), before + 1);
        // A second one is refused while the first lives.
        t.bypass(1, "Script Nemo summonMaguen");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32735-06"));
        assert_eq!(t.world.npc_count(), before + 1);
        // After its dance it dies and the character may practice again.
        t.ticks(600);
        t.bypass(1, "Script Nemo summonMaguen");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32735-05"));
    }
}
