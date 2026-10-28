//! Small `Npc` helpers the scripts of Gracia share, built on the `pub` world API.
// Java: entity/actor/Npc.java (setDisplayEffect, broadcastSocialAction)

use l2_protocol_game::server::{ExChangeNpcState, SocialAction};
use l2_util::ObjectId;
use l2_world::ScriptCtx;

/// Java `Npc.setDisplayEffect(value)`: announced to the players around only when it changes.
pub fn set_display_effect(ctx: &mut ScriptCtx<'_>, npc: ObjectId, value: i32) {
    let changed = ctx.world_mut().npc_mut(npc).is_some_and(|n| {
        let changed = n.display_effect != value;
        n.display_effect = value;
        changed
    });
    if changed {
        ctx.world().broadcast_packet(
            npc,
            &ExChangeNpcState {
                object_id: npc.0,
                state: value,
            },
        );
    }
}

/// Java `WorldObject.calculateDistance3D(other)`: `f64::MAX` when either is not in the world.
pub fn distance_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    let (Some(a), Some(b)) = (ctx.world().position_of(a), ctx.world().position_of(b)) else {
        return f64::MAX;
    };
    let (dx, dy, dz) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1), f64::from(a.2 - b.2));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Java `Creature.broadcastSocialAction(id)`.
pub fn broadcast_social_action(ctx: &ScriptCtx<'_>, creature: ObjectId, action_id: i32) {
    ctx.world().broadcast_packet(
        creature,
        &SocialAction {
            object_id: creature.0,
            action_id,
        },
    );
}
