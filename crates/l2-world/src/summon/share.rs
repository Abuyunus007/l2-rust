//! Servitor Share: part of the owner's stats added to its servitor's.
// Java: entity/actor/instance/Servitor.java (getPAtk, getPDef, getMAtk, getMDef, getMAtkSpd,
//       getMaxHp, getMaxMp, getCriticalHit, getPAtkSpd)
// Java: entity/actor/Player.java (setServitorShare, getServitorShareBonus)
//
// `ServitorShare` (the effect, `effects/handlers/servitor_share.rs`) gives the owner a map of
// stat -> multiplier; every getter above of a servitor adds `owner value * (multiplier - 1)`.
// NOTE(java-quirk): Java overrides the `Creature` getters, so a caller that asks the stat object
// (`getStat().getMaxHp()`) gets the value without the share. Here the share is part of the stat
// view's getter, which every caller uses.

use l2_data::Stat;
use l2_util::ObjectId;

use crate::stats::StatView;
use crate::world::World;

impl World {
    /// Java `Player.setServitorShare(map)` (`None` = `null`).
    pub(crate) fn set_servitor_share(&mut self, owner: ObjectId, share: Option<Vec<(Stat, f64)>>) {
        match share {
            Some(share) => {
                self.summons.servitor_share.insert(owner, share);
            }
            None => {
                self.summons.servitor_share.remove(&owner);
            }
        }
    }

    /// Java `Player.getServitorShareBonus(stat)`: 1 without a share or for a stat it does not
    /// name.
    pub fn servitor_share_bonus(&self, owner: ObjectId, stat: Stat) -> f64 {
        self.summons
            .servitor_share
            .get(&owner)
            .and_then(|share| share.iter().find(|(s, _)| *s == stat).map(|(_, v)| *v))
            .unwrap_or(1.0)
    }
}

impl<'w> StatView<'w> {
    /// The owner's view of a servitor (`getOwner() != null` in `Servitor`'s element getters);
    /// `None` for anybody else.
    pub(crate) fn servitor_owner_view(&self) -> Option<StatView<'w>> {
        let npc = self.npc()?;
        let world = self.world();
        let id = npc.core.object_id;
        if !world.is_servitor(id) {
            return None;
        }
        let owner = world.summon_owner(id)?;
        self.other(owner)
    }

    /// What a servitor's getter of `stat` adds: `owner value * (getServitorShareBonus(stat) - 1)`,
    /// with `owner_value` the owner's getter (`player.getPAtk(target)` ...). 0 for anybody else
    /// (`asPlayer() == null` takes the plain getter).
    pub(crate) fn servitor_share(&self, stat: Stat, owner_value: impl FnOnce(&StatView<'w>) -> f64) -> f64 {
        let Some(npc) = self.npc() else {
            return 0.0;
        };
        let world = self.world();
        let id = npc.core.object_id;
        if !world.is_servitor(id) {
            return 0.0;
        }
        let Some(owner) = world.acting_player(id) else {
            return 0.0;
        };
        let bonus = world.servitor_share_bonus(owner, stat);
        if bonus == 1.0 {
            // `value * 0.0` adds nothing (the owner's stats are finite).
            return 0.0;
        }
        let Some(view) = self.other(owner) else {
            return 0.0;
        };
        owner_value(&view) * (bonus - 1.0)
    }
}
