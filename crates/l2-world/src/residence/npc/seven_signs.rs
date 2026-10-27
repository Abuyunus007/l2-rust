//! What the castle NPCs ask of Seven Signs.
// Java: mechanics/sevensigns/SevenSigns.java (getSealOwner, getCurrentPeriod, getPlayerCabal,
//       isCompResultsPeriod, isCompetitionPeriod, isSealValidationPeriod)
//
// M8-SS-01: the answers come from the Seven Signs state (`sevensigns/`).

use l2_util::ObjectId;

use crate::world::World;

pub use crate::sevensigns::{CABAL_DAWN, CABAL_DUSK, CABAL_NULL};
pub(super) use crate::sevensigns::{
    PERIOD_COMP_RECRUITING, PERIOD_COMP_RESULTS, PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SEAL_AVARICE,
    SEAL_GNOSIS, SEAL_STRIFE,
};

impl World {
    /// Java `SevenSigns.getSealOwner(seal)`.
    pub(super) fn seven_signs_seal_owner(&self, seal: i32) -> i32 {
        self.ss_seal_owner(seal)
    }

    /// Java `SevenSigns.getCurrentPeriod()`.
    pub(super) fn seven_signs_current_period(&self) -> i32 {
        self.ss_current_period()
    }

    /// Java `SevenSigns.getPlayerCabal(objectId)`.
    pub(super) fn seven_signs_player_cabal(&self, player: ObjectId) -> i32 {
        self.ss_player_cabal(player.0)
    }

    /// Java `SevenSigns.isCompResultsPeriod()`.
    pub(super) fn seven_signs_is_comp_results_period(&self) -> bool {
        self.ss_is_comp_results_period()
    }

    /// Java `SevenSigns.isCompetitionPeriod()`.
    pub(super) fn seven_signs_is_competition_period(&self) -> bool {
        self.ss_is_competition_period()
    }
}
