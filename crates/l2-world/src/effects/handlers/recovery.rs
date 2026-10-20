//! The `Recovery` effect: a player's death penalty goes down one level (the penalty buff of skill
//! 5076 changes with it, and is gone at level 0).
// Java: dist/game/data/scripts/handlers/skill/effects/Recovery.java
// Java: entity/actor/Player.java (reduceDeathPenaltyBuffLevel)

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Player.DEATH_PENALTY_SKILL_ID`: the penalty buff, one skill per level.
const DEATH_PENALTY_SKILL_ID: i32 = 5076;

/// Java `Recovery`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Recovery;

impl Recovery {
    /// Java `Recovery(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Recovery))
    }
}

impl EffectHandler for Recovery {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effected) {
            reduce_death_penalty_buff_level(world, cx.effected);
        }
    }
}

/// Java `Player.reduceDeathPenaltyBuffLevel()`.
fn reduce_death_penalty_buff_level(world: &mut World, player: l2_util::ObjectId) {
    let Some(level) = world
        .players
        .get(&player)
        .map(|p| p.data.profile.death_penalty_level)
    else {
        return;
    };
    if level <= 0 {
        return;
    }
    let table = world.skill_table();
    if table
        .as_ref()
        .and_then(|t| t.get(DEATH_PENALTY_SKILL_ID, level))
        .is_some()
    {
        world.remove_skill(player, DEATH_PENALTY_SKILL_ID, true);
    }
    let level = level - 1;
    if let Some(p) = world.players.get_mut(&player) {
        p.data.profile.death_penalty_level = level;
    }
    if level > 0 {
        if table
            .as_ref()
            .and_then(|t| t.get(DEATH_PENALTY_SKILL_ID, level))
            .is_some()
        {
            world.add_skill(player, DEATH_PENALTY_SKILL_ID, level);
        }
        world.send_etc_status_update(player);
        let sm = SystemMessage::new(SystemMessageId::YourDeathPenaltyIsNowLevelS1)
            .with(SystemMessageParam::IntNumber(level));
        world.send(player, &sm);
    } else {
        world.send_etc_status_update(player);
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YourDeathPenaltyHasBeenLifted),
        );
    }
}
