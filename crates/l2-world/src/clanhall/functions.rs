//! What the hall functions do besides being paid for: the regeneration factors of the clan's own
//! hall and the exp a resurrection in the hall gives back.
// Java: mechanics/stats/Formulas.java (calcHpRegen, calcMpRegen: `ClanHall.FUNC_RESTORE_HP`,
//       `FUNC_RESTORE_MP`)
// Java: network/clientpackets/RequestRestartPoint.java (case 1: `FUNC_RESTORE_EXP`)
//
// The teleport, support and item creation functions are used at the hall manager (`manager.rs`),
// the decoration functions show in `AgitDecoInfo` (`residence/clan_hall.rs`).

use l2_util::ObjectId;

use crate::residence::{HALL_FUNC_RESTORE_EXP, ClanHallId, ResidenceRef};
use crate::world::World;
use crate::zones::ZoneId;

/// The fees of one hall function: the time between two fees and the fee of each level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanHallFunctionSettings<const N: usize> {
    /// `CH_*_FEE_RATIO`: milliseconds between two fees.
    pub ratio: i64,
    /// `CH_*1_FEE` to `CH_*N_FEE`.
    pub fees: [i32; N],
}

impl<const N: usize> ClanHallFunctionSettings<N> {
    pub const fn new(ratio: i64, fees: [i32; N]) -> Self {
        Self { ratio, fees }
    }

    /// `FEE_RATIO / 1000 / 60 / 60 / 24`: the days the pages print.
    pub fn days(&self) -> i64 {
        self.ratio / 1000 / 60 / 60 / 24
    }

    /// The fee of the 1-based fee slot (`CH_*<slot>_FEE`); the last slot for one past the end
    /// (Java's `default:` is the highest level).
    pub fn fee(&self, slot: usize) -> i32 {
        self.fees[slot.clamp(1, N) - 1]
    }
}

impl World {
    /// The factor `1 + level / 100` that `Formulas.calcHpRegen` / `calcMpRegen` multiply in when
    /// the player stands in the zone of the hall his clan owns and the hall has the function.
    pub(crate) fn clan_hall_regen_factor(&self, player: ObjectId, function_type: i32) -> Option<f64> {
        if !self.is_inside_zone(player, ZoneId::ClanHall) {
            return None;
        }
        let hideout = self.clan_of(player).map(|c| c.hideout_id).filter(|id| *id > 0)?;
        let (x, y, z) = self.position_of(player)?;
        // `ZoneManager.getZone(player, ClanHallZone.class).getResidenceId() == hideoutId`.
        if !self.clan_hall_contains(ClanHallId(hideout), x, y, z) {
            return None;
        }
        let function = self.residence_function(ResidenceRef::ClanHall(ClanHallId(hideout)), function_type)?;
        Some(1.0 + f64::from(function.lvl) / 100.0)
    }

    /// The level of the exp-recovery function of the auctionable hall a clan owns
    /// (`ClanHallTable.getClanHallByOwner(clan).getFunction(FUNC_RESTORE_EXP).getLevel()`): the
    /// percent of the lost exp `RequestRestartPoint` gives back to a player who returns to the
    /// hall. The restart point itself is M7-SIEGE-01 (`death.rs`).
    pub fn clan_hall_restore_exp_level(&self, clan_id: i32) -> Option<i32> {
        let hall = self.auctionable_hall_by_owner(clan_id)?;
        self.residence_function(ResidenceRef::ClanHall(hall), HALL_FUNC_RESTORE_EXP)
            .map(|f| f.lvl)
    }
}
