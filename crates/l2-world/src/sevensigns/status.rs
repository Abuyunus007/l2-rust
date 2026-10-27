//! `RequestSSQStatus`: the record of the Seven Signs (the four `SSQStatus` pages).
// Java: network/clientpackets/RequestSSQStatus.java
// Java: network/serverpackets/SSQStatus.java (the values it reads from SevenSigns)
//
// With `CycleDatabaseWrites = False` the rows are read again before the page is shown (D-50,
// D-53: the Java game server runs the cycle); a failed read shows nothing.

use l2_protocol_game::server::{SevenSignsView, SsqStatus};
use l2_util::ObjectId;

use super::{CABAL_DAWN, CABAL_DUSK, SevenSignsThen};
use crate::world::World;

impl World {
    /// Java `RequestSSQStatus.runImpl`.
    pub(crate) fn on_request_ssq_status(&mut self, player: ObjectId, page: i8) {
        if !self.players.contains_key(&player) {
            return;
        }
        if (self.ss_is_seal_validation_period() || self.ss_is_comp_results_period()) && page == 4 {
            return;
        }
        if self.seven_signs.settings.cycle_database_writes {
            self.ss_send_status(player, page);
        } else {
            self.ss_request_read(SevenSignsThen::Status { player, page });
        }
    }

    /// `player.sendPacket(new SSQStatus(objectId, page))` on the state as it is now.
    pub(crate) fn ss_send_status(&mut self, player: ObjectId, page: i8) {
        if !self.players.contains_key(&player) {
            return;
        }
        // The state may have changed with the read (D-50): the check of `runImpl` again.
        if (self.ss_is_seal_validation_period() || self.ss_is_comp_results_period()) && page == 4 {
            return;
        }
        let view = self.ss_view(player.0);
        self.send(player, &SsqStatus::new(page, &view));
    }

    /// The values `SSQStatus.writeImpl` reads from `SevenSigns` for one character.
    pub fn ss_view(&self, char_id: i32) -> SevenSignsView {
        let s = &self.seven_signs.status;
        SevenSignsView {
            winning_cabal: s.cabal_highest_score(),
            current_period: s.active_period,
            current_cycle: s.current_cycle,
            player_cabal: self.ss_player_cabal(char_id),
            player_seal: self.ss_player_seal(char_id),
            player_stone_contrib: i64::from(self.ss_player_stone_contrib(char_id)),
            player_adena_collect: i64::from(self.ss_player_adena_collect(char_id)),
            dawn_stone_score: s.dawn_stone_score,
            dusk_stone_score: s.dusk_stone_score,
            dawn_festival_score: s.dawn_festival_score,
            dusk_festival_score: s.dusk_festival_score,
            dawn_total_score: s.current_score(CABAL_DAWN),
            dusk_total_score: s.current_score(CABAL_DUSK),
            total_dawn_members: self.ss_total_members(CABAL_DAWN),
            total_dusk_members: self.ss_total_members(CABAL_DUSK),
            seal_owner: s.seal_owners,
            dawn_proportion: s.dawn_seal_totals,
            dusk_proportion: s.dusk_seal_totals,
            // `SevenSignsFestival.getHighestScoreData`: Java's blank set (score 0, members "",
            // which splits into one empty name) where the cycle has no data.
            festivals: self.festival_high_scores(),
        }
    }
}
