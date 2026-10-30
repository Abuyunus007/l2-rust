//! Pagan Keys: the Zombie Worker and the Triols drop the keys of the Pagan Temple.
// Java: dist/game/data/scripts/ai/areas/PaganTemple/PaganKeys.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Items
const ANTEROOM_KEY: i32 = 8273;
const CHAPEL_KEY: i32 = 8274;
const KEY_OF_DARKNESS: i32 = 8275;

// NPCs
const ZOMBIE_WORKER: i32 = 22140;
const TRIOLS_LAYPERSON: i32 = 22142;
const TRIOLS_PRIEST: i32 = 22168;

// Misc
const ANTEROOM_KEY_CHANCE: i32 = 10;
const CHAPEL_KEY_CHANCE: i32 = 10;
const KEY_OF_DARKNESS_CHANCE: i32 = 10;

/// Java `ai.areas.PaganTemple.PaganKeys`.
pub struct PaganKeys;

impl Script for PaganKeys {
    fn name(&self) -> &'static str {
        "PaganKeys"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PaganTemple"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[ZOMBIE_WORKER, TRIOLS_LAYPERSON, TRIOLS_PRIEST]);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let (key, chance) = match npc.id {
            ZOMBIE_WORKER => (ANTEROOM_KEY, ANTEROOM_KEY_CHANCE),
            TRIOLS_LAYPERSON => (CHAPEL_KEY, CHAPEL_KEY_CHANCE),
            TRIOLS_PRIEST => (KEY_OF_DARKNESS, KEY_OF_DARKNESS_CHANCE),
            _ => return,
        };
        if ctx.get_random(100) < chance {
            if ctx.auto_loot() {
                ctx.give_items(killer, key, 1);
            } else {
                ctx.drop_item(npc, Some(killer), key, 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22140" type="Monster" level="30" name="Zombie Worker"/>
        <npc id="22168" type="Monster" level="30" name="Triol's Priest"/>"#;
    const ITEMS: &str = r#"
        <item id="8273" type="EtcItem" name="Anteroom Key"/>
        <item id="8275" type="EtcItem" name="Key of Darkness"/>"#;

    #[test]
    fn a_kill_drops_the_key_on_the_ground_with_the_10_percent_chance() {
        let mut registry = ScriptRegistry::new();
        registry.add(PaganKeys);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 75, vec![]);
        // The world RNG is seeded: kill until the key lands on the ground (AutoLoot is off).
        let mut kills = 0;
        while t
            .world
            .nearest_ground_item(ObjectId(1), ANTEROOM_KEY, 2_000)
            .is_none()
            && kills < 500
        {
            let mob = t.spawn(ZOMBIE_WORKER, 20);
            t.kill(1, mob);
            kills += 1;
        }
        assert!(
            t.world
                .nearest_ground_item(ObjectId(1), ANTEROOM_KEY, 2_000)
                .is_some()
        );
        assert_eq!(t.count(1, ANTEROOM_KEY), 0);
    }
}
