//! The Star Stones of the Seed of Annihilation: the collection skill (932) gathers a red, blue or
//! green Energy Star Stone.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/StarStones.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const MOBS: [i32; 9] = [18684, 18685, 18686, 18687, 18688, 18689, 18690, 18691, 18692];

const COLLECTION_RATE: i32 = 1;

/// Java `ai.areas.Gracia.AI.StarStones`.
pub struct StarStones;

impl Script for StarStones {
    fn name(&self) -> &'static str {
        "StarStones"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&MOBS);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if skill.id == 932 {
            let item_id = match npc.id {
                // give Red item
                18684..=18686 => 14009,
                // give Blue item
                18687..=18689 => 14010,
                // give Green item
                18690..=18692 => 14011,
                // unknown npc!
                _ => return,
            };

            if ctx.get_random(100) < 33 {
                ctx.world().send(
                    caster,
                    &SystemMessage::new(SystemMessageId::YourCollectionHasSucceeded),
                );
                let count = ctx.get_random_range(COLLECTION_RATE + 1, 2 * COLLECTION_RATE);
                ctx.give_items(caster, item_id, i64::from(count));
            } else if (skill.level == 1 && ctx.get_random(100) < 15)
                || (skill.level == 2 && ctx.get_random(100) < 50)
                || (skill.level == 3 && ctx.get_random(100) < 75)
            {
                ctx.world().send(
                    caster,
                    &SystemMessage::new(SystemMessageId::YourCollectionHasSucceeded),
                );
                let count = ctx.get_random_range(1, COLLECTION_RATE);
                ctx.give_items(caster, item_id, i64::from(count));
            } else {
                ctx.world().send(
                    caster,
                    &SystemMessage::new(SystemMessageId::TheCollectionHasFailed),
                );
            }

            ctx.world_mut().delete_npc(npc.object_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18684" type="Monster" level="80" name="Red Star Stone"/>
        <npc id="18690" type="Monster" level="80" name="Green Star Stone"/>"#;
    const ITEMS: &str = r#"<item id="14009" type="EtcItem" name="Red"><set name="is_stackable" val="true"/></item>
        <item id="14011" type="EtcItem" name="Green"><set name="is_stackable" val="true"/></item>"#;

    fn collect(npc_id: i32, skill: SkillKey) -> (i64, i64, bool, usize) {
        let mut registry = ScriptRegistry::new();
        registry.add(StarStones);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 80, vec![]);
        let stone = t.spawn(npc_id, 20);
        t.take();
        t.world.run_as_script("StarStones", |ctx| {
            let n = ctx.script_npc(stone).unwrap();
            StarStones.on_skill_see(ctx, n, ObjectId(1), skill, &[], false);
        });
        let messages = names_to(&t.take(), 1)
            .iter()
            .filter(|n| *n == "SYSTEM_MESSAGE")
            .count();
        (
            t.count(1, 14009),
            t.count(1, 14011),
            t.world.npc(stone).is_none(),
            messages,
        )
    }

    #[test]
    fn a_collection_skill_removes_the_stone_and_may_give_its_color() {
        for _ in 0..20 {
            let (red, green, gone, messages) = collect(18684, SkillKey::new(932, 3));
            assert!(gone);
            assert_eq!(green, 0);
            // 33%: 2 stones (the range is 2..2); then 75%: 1 stone; else nothing.
            assert!(red == 0 || red == 1 || red == 2, "{red}");
            assert!(messages >= 1);
        }
        let (red, green, gone, _) = collect(18690, SkillKey::new(932, 3));
        assert!(gone);
        assert_eq!(red, 0);
        assert!(green <= 2);
    }

    #[test]
    fn other_skills_leave_the_stone() {
        let (red, green, gone, messages) = collect(18684, SkillKey::new(1000, 1));
        assert_eq!((red, green, messages), (0, 0, 0));
        assert!(!gone);
    }
}
