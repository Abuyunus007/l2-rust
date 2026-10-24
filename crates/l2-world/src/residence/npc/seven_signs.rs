//! What the castle NPCs ask of Seven Signs.
// Java: mechanics/sevensigns/SevenSigns.java (getSealOwner, getCurrentPeriod, getPlayerCabal,
//       isCompResultsPeriod, isCompetitionPeriod, isSealValidationPeriod)
//
// M8: Seven Signs is not ported. The answers are the ones of a server where nobody joined a cabal
// and the seals are not owned, in the seal validation period: the same state `siege/mod.rs`
// (`seven_signs_strife_owner`, `seven_signs_is_seal_validation`) and the mercenary tickets assume.

use l2_util::ObjectId;

use crate::world::World;

/// `SevenSigns.CABAL_NULL`.
pub const CABAL_NULL: i32 = 0;
/// `SevenSigns.CABAL_DUSK`.
pub const CABAL_DUSK: i32 = 1;
/// `SevenSigns.CABAL_DAWN`.
pub const CABAL_DAWN: i32 = 2;

/// `SevenSigns.SEAL_AVARICE`.
pub(super) const SEAL_AVARICE: i32 = 1;
/// `SevenSigns.SEAL_GNOSIS`.
pub(super) const SEAL_GNOSIS: i32 = 2;
/// `SevenSigns.SEAL_STRIFE`.
pub(super) const SEAL_STRIFE: i32 = 3;

/// `SevenSigns.PERIOD_COMP_RECRUITING`.
pub(super) const PERIOD_COMP_RECRUITING: i32 = 0;
/// `SevenSigns.PERIOD_COMPETITION`.
pub(super) const PERIOD_COMPETITION: i32 = 1;
/// `SevenSigns.PERIOD_COMP_RESULTS`.
pub(super) const PERIOD_COMP_RESULTS: i32 = 2;
/// `SevenSigns.PERIOD_SEAL_VALIDATION`.
pub(super) const PERIOD_SEAL_VALIDATION: i32 = 3;

impl World {
    /// Java `SevenSigns.getSealOwner(seal)`.
    // M8: the seals' owners.
    pub(super) fn seven_signs_seal_owner(&self, seal: i32) -> i32 {
        if seal == SEAL_STRIFE {
            return self.seven_signs_strife_owner();
        }
        CABAL_NULL
    }

    /// Java `SevenSigns.getCurrentPeriod()`.
    // M8: the period of the cycle.
    pub(super) fn seven_signs_current_period(&self) -> i32 {
        if self.seven_signs_is_seal_validation() {
            PERIOD_SEAL_VALIDATION
        } else {
            PERIOD_COMPETITION
        }
    }

    /// Java `SevenSigns.getPlayerCabal(objectId)`.
    // M8: the cabal a character joined.
    pub(super) fn seven_signs_player_cabal(&self, _player: ObjectId) -> i32 {
        CABAL_NULL
    }

    /// Java `SevenSigns.isCompResultsPeriod()`.
    pub(super) fn seven_signs_is_comp_results_period(&self) -> bool {
        self.seven_signs_current_period() == PERIOD_COMP_RESULTS
    }

    /// Java `SevenSigns.isCompetitionPeriod()`.
    pub(super) fn seven_signs_is_competition_period(&self) -> bool {
        self.seven_signs_current_period() == PERIOD_COMPETITION
    }
}
