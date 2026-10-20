//! The `OpenChest` effect: a chest within a few levels of the caster breaks open and drops its
//! real loot; one too strong for the caster turns on it.
// Java: dist/game/data/scripts/handlers/skill/effects/OpenChest.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::server::SocialAction;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// `broadcastSocialAction(3)`: the player cheers.
const SOCIAL_SUCCESS: i32 = 3;
/// `broadcastSocialAction(13)`: the player is shocked.
const SOCIAL_FAILURE: i32 = 13;

/// Java `OpenChest`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OpenChest;

impl OpenChest {
    /// Java `OpenChest(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(OpenChest))
    }
}

impl EffectHandler for OpenChest {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let chest = cx.effected;
        // `effected instanceof Chest`.
        if !world.npc_template(chest).is_some_and(|t| t.is_type("Chest")) {
            return;
        }
        // `effector.asPlayer()` is null for an NPC, and Java then fails with a
        // NullPointerException: nothing happens.
        let player = cx.effector;
        if !world.is_player(player) {
            return;
        }
        let instance = |w: &World, id| w.objects.get(&id).map_or(0, |e| e.instance_id);
        if world.is_dead(chest) || instance(world, player) != instance(world, chest) {
            return;
        }
        let player_level = world.stats(player).map_or(0, |v| v.level());
        let chest_level = world.stats(chest).map_or(0, |v| v.level());
        let level_gap = (chest_level - player_level).abs();
        if (player_level <= 77 && level_gap <= 6) || (player_level >= 78 && level_gap <= 5) {
            world.broadcast_creature_packet(
                player,
                &SocialAction {
                    object_id: player.0,
                    action_id: SOCIAL_SUCCESS,
                },
            );
            if let Some(npc) = world.npcs.get_mut(&chest) {
                // `chest.setSpecialDrop()`, `chest.setMustRewardExpSp(false)`.
                npc.special_drop = true;
                npc.must_reward_exp_sp = false;
            }
            let max_hp = world.stats(chest).map_or(0, |v| v.max_hp());
            world.reduce_current_hp_by_skill(chest, f64::from(max_hp), Some(player), cx.skill);
        } else {
            world.broadcast_creature_packet(
                player,
                &SocialAction {
                    object_id: player.0,
                    action_id: SOCIAL_FAILURE,
                },
            );
            world.add_damage_hate(chest, player, 0, 1);
            world.set_intention_attack(chest, player);
        }
    }
}
