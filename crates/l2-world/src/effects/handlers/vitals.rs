//! What the HP, MP and CP handlers share: reading a creature's vitals and the "restored"
//! messages.
// Java: the repeated code of dist/game/data/scripts/handlers/skill/effects/{Heal,HealPercent,
//       ManaHeal,CpHeal,...}.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::world::World;

/// Which of the three vitals a handler works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Vital {
    Hp,
    Mp,
    Cp,
}

impl Vital {
    /// "S1 HP has been restored." and "S2 HP has been restored by C1." (Java
    /// `S1_HP_HAS_BEEN_RESTORED`, `S2_HP_HAS_BEEN_RESTORED_BY_C1` and the MP and CP twins).
    fn restored_messages(self) -> (SystemMessageId, SystemMessageId) {
        match self {
            Vital::Hp => (
                SystemMessageId::S1HpHasBeenRestored,
                SystemMessageId::S2HpHasBeenRestoredByC1,
            ),
            Vital::Mp => (
                SystemMessageId::S1MpHasBeenRestored,
                SystemMessageId::S2MpHasBeenRestoredByC1,
            ),
            Vital::Cp => (
                SystemMessageId::S1CpHasBeenRestored,
                SystemMessageId::S2CpHasBeenRestoredByC1,
            ),
        }
    }

    /// The stored value (Java `getCurrentHp/Mp/Cp()`).
    pub(super) fn current(self, world: &World, id: ObjectId) -> f64 {
        world.core_of(id).map_or(0.0, |core| match self {
            Vital::Hp => core.cur_hp,
            Vital::Mp => core.cur_mp,
            Vital::Cp => core.cur_cp,
        })
    }

    /// Java `getMaxHp/Mp/Cp()`.
    pub(super) fn max(self, world: &World, id: ObjectId) -> f64 {
        world.stats(id).map_or(0.0, |view| {
            f64::from(match self {
                Vital::Hp => view.max_hp(),
                Vital::Mp => view.max_mp(),
                Vital::Cp => view.max_cp(),
            })
        })
    }

    /// Java `getMaxRecoverableHp/Mp/Cp()`.
    pub(super) fn max_recoverable(self, world: &World, id: ObjectId) -> f64 {
        world.stats(id).map_or(0.0, |view| {
            f64::from(match self {
                Vital::Hp => view.max_recoverable_hp(),
                Vital::Mp => view.max_recoverable_mp(),
                Vital::Cp => view.max_recoverable_cp(),
            })
        })
    }

    /// Java `setCurrentHp/Mp/Cp(value)` (with the broadcast).
    pub(super) fn set(self, world: &mut World, id: ObjectId, value: f64) {
        match self {
            Vital::Hp => world.set_current_hp(id, value, true),
            Vital::Mp => world.set_current_mp(id, value, true),
            Vital::Cp => world.set_current_cp(id, value, true),
        };
    }

    /// The "prevents overheal and negative amount" step every restore handler has:
    /// `Math.max(Math.min(amount, maxRecoverable - current), 0)`.
    pub(super) fn clamp_to_room(self, world: &World, id: ObjectId, amount: f64) -> f64 {
        amount
            .min(self.max_recoverable(world, id) - self.current(world, id))
            .max(0.0)
    }

    /// Adds `amount` (already clamped) when it is not 0: `if (amount != 0) setCurrentX(amount +
    /// getCurrentX())`.
    pub(super) fn add(self, world: &mut World, id: ObjectId, amount: f64) {
        if amount != 0.0 {
            let now = self.current(world, id);
            self.set(world, id, amount + now);
        }
    }

    /// The restore message to the player who got it: the "by C1" form (with the effector's name)
    /// when `by_other`, else the plain one. `(int) amount` is the number shown.
    pub(super) fn send_restored(
        self,
        world: &World,
        effector: ObjectId,
        effected: ObjectId,
        by_other: bool,
        amount: f64,
    ) {
        let (own, other) = self.restored_messages();
        let message = if by_other {
            SystemMessage::new(other).with(SystemMessageParam::Text(world.creature_name(effector)))
        } else {
            SystemMessage::new(own)
        };
        world.send(
            effected,
            &message.with(SystemMessageParam::IntNumber(amount as i32)),
        );
    }
}

/// Java `Creature.isDoor()`.
pub(super) fn is_door(world: &World, id: ObjectId) -> bool {
    world.doors.contains_key(&id)
}
