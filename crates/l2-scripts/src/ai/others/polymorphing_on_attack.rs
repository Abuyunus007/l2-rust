//! Monsters that turn into a stronger form when hurt.
// Java: dist/game/data/scripts/ai/others/PolymorphingOnAttack.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

/// Java `MOBSPAWNS`: id -> (spawned id, HP percent, chance, text group).
const MOBSPAWNS: [(i32, [i32; 4]); 16] = [
    (21258, [21259, 100, 100, -1]), // Fallen Orc Shaman -> Sharp Talon Tiger (always polymorphs)
    (21261, [21262, 100, 20, 0]),   // Ol Mahum Transcender 1st stage
    (21262, [21263, 100, 10, 1]),   // Ol Mahum Transcender 2nd stage
    (21263, [21264, 100, 5, 2]),    // Ol Mahum Transcender 3rd stage
    (21265, [21271, 100, 33, 0]),   // Cave Ant Larva -> Cave Ant
    (21266, [21269, 100, 100, -1]), // Cave Ant Larva -> Cave Ant (always polymorphs)
    (21267, [21270, 100, 100, -1]), // Cave Ant Larva -> Cave Ant Soldier (always polymorphs)
    (21271, [21272, 66, 10, 1]),    // Cave Ant -> Cave Ant Soldier
    (21272, [21273, 33, 5, 2]),     // Cave Ant Soldier -> Cave Noble Ant
    (21521, [21522, 100, 30, -1]),  // Claws of Splendor
    (21524, [21525, 100, 30, -1]),  // Blade of Splendor
    (21527, [21528, 100, 30, -1]),  // Anger of Splendor
    (21531, [21658, 100, 30, -1]),  // Punishment of Splendor
    (21533, [21534, 100, 30, -1]),  // Alliance of Splendor
    (21537, [21538, 100, 30, -1]),  // Fang of Splendor
    (21539, [21540, 100, 30, -1]),  // Wailing of Splendor
];

const MOBTEXTS: [[NpcStringId; 3]; 3] = [
    [
        NpcStringId::EnoughFoolingAroundGetReadyToDie,
        NpcStringId::YouIdiotIVeJustBeenToyingWithYou,
        NpcStringId::NowTheFunStarts,
    ],
    [
        NpcStringId::IMustAdmitNoOneMakesMyBloodBoilQuiteLikeYouDo,
        NpcStringId::NowTheBattleBegins,
        NpcStringId::WitnessMyTruePower,
    ],
    [
        NpcStringId::PrepareToDie,
        NpcStringId::ILlDoubleMyStrength,
        NpcStringId::YouHaveMoreSkillThanIThought,
    ],
];

/// Java `ai.others.PolymorphingOnAttack`.
pub struct PolymorphingOnAttack;

impl Script for PolymorphingOnAttack {
    fn name(&self) -> &'static str {
        "PolymorphingOnAttack"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = MOBSPAWNS.iter().map(|&(k, _)| k).collect();
        s.add_attack_id(&ids);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.world().is_object_spawned(npc.object_id) && !ctx.is_dead(npc.object_id) {
            let Some(&(_, tmp)) = MOBSPAWNS.iter().find(|&&(k, _)| k == npc.id) else {
                return;
            };
            if ctx.npc_current_hp(npc) <= (f64::from(ctx.npc_max_hp(npc)) * f64::from(tmp[1])) / 100.0
                && ctx.get_random(100) < tmp[2]
            {
                if tmp[3] >= 0 {
                    let texts = &MOBTEXTS[tmp[3] as usize];
                    let index = ctx.get_random(texts.len() as i32) as usize;
                    // NOTE(java-quirk): Java sends a `CreatureSay` (the chat window packet) built
                    // from the NPC; it is shown here as the NPC's speech (`NpcSay`).
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, texts[index], &[]);
                }

                // The new NPC stands where the old one was, so read that before it is deleted.
                let at = ctx.npc_position(npc);
                ctx.delete_npc(npc);
                let Some((x, y, z, heading)) = at else {
                    return;
                };
                let new_npc = ctx.add_spawn(tmp[0], x, y, z + 20, heading, false, 0, 0);
                let original_attacker = if is_summon {
                    ctx.summon_of(attacker)
                } else {
                    Some(attacker)
                };
                if let (Some(new_npc), Some(target)) = (new_npc, original_attacker) {
                    ctx.add_attack_desire(new_npc, target, 500);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    // Fallen Orc Shaman (always polymorphs) and the Sharp Talon Tiger.
    const NPC_DEFS: &str = r#"<npc id="21258" type="Monster" level="50" name="Shaman"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="21259" type="Monster" level="50" name="Tiger"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_hurt_shaman_becomes_the_tiger() {
        let mut registry = ScriptRegistry::new();
        registry.add(PolymorphingOnAttack);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let shaman = t.spawn(21258, 20);
        t.enter(1, 60, vec![]);
        t.world.script_on_attackable_attack(shaman, ObjectId(1), 10, None);
        t.world.script_drain();
        assert!(!t.in_world(shaman));
        let tigers = t.npcs_of(21259);
        assert_eq!(tigers.len(), 1);
        t.world.delete_npc(tigers[0]);
    }

    #[test]
    fn the_table_matches_java() {
        assert_eq!(MOBSPAWNS.len(), 16);
        assert_eq!(MOBSPAWNS[8], (21272, [21273, 33, 5, 2]));
    }
}
