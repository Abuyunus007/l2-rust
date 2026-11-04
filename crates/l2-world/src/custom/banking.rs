//! `config/Custom/Banking.ini`: the voiced commands `.bank`, `.deposit` and `.withdraw` that
//! trade Adena for Gold Bars, and the automatic conversion of a big purse.

use l2_util::ObjectId;

use crate::world::World;

/// The Gold Bar item (`3470`).
pub const GOLD_BAR_ID: i32 = 3470;

impl World {
    /// `Banking.onCommand`.
    pub(crate) fn banking_voiced(&mut self, player: ObjectId, command: &str) -> bool {
        let b = self.settings.custom_misc.banking.clone();
        let adena_count = i64::from(b.adena_count);
        let goldbar_count = i64::from(b.goldbar_count);
        let gold_bar_text = if b.goldbar_count > 1 {
            "Gold Bars"
        } else {
            "Gold Bar"
        };

        if command == "bank" {
            self.send_message(
                player,
                &format!("Use .deposit to convert {adena_count} Adena to {goldbar_count} {gold_bar_text}."),
            );
            self.send_message(
                player,
                &format!("Use .withdraw to convert {goldbar_count} {gold_bar_text} to {adena_count} Adena."),
            );
        } else if command == "deposit" {
            let current_adena = self.inventory_item_count(player, l2_data::item::ADENA_ID);
            if current_adena >= adena_count {
                // `reduceAdena(BUY, ..., sendMessage false)`.
                if !self.reduce_adena(player, adena_count, false) {
                    return false;
                }
                self.add_item(player, GOLD_BAR_ID, goldbar_count, false);
                self.send_message(
                    player,
                    &format!(
                        "Successfully converted {goldbar_count} {gold_bar_text} into {adena_count} Adena."
                    ),
                );
            } else {
                let needed = adena_count - current_adena;
                self.send_message(
                    player,
                    &format!("You don't have enough Adena to convert to {gold_bar_text}."),
                );
                self.send_message(
                    player,
                    &format!(
                        "You need {adena_count} Adena but you only have {current_adena}. You need {needed} more."
                    ),
                );
            }
        } else if command == "withdraw" {
            let current_bars = self.inventory_item_count(player, GOLD_BAR_ID);
            if current_bars >= goldbar_count {
                if !self.destroy_item_by_item_id(player, GOLD_BAR_ID, goldbar_count, false) {
                    return false;
                }
                self.add_adena(player, adena_count, false);
                self.send_message(
                    player,
                    &format!(
                        "Successfully converted {adena_count} Adena to {goldbar_count} {gold_bar_text}."
                    ),
                );
            } else {
                let needed = goldbar_count - current_bars;
                self.send_message(player, "You don't have enough Gold Bars to convert to Adena.");
                self.send_message(
                    player,
                    &format!(
                        "You need {goldbar_count} {gold_bar_text} but you only have {current_bars}. You need {needed} more."
                    ),
                );
            }
        }
        true
    }

    /// `BankingAutoConvert.onPlayerItemAdd` for an adena item that was added to the player:
    /// above `BankingAutoConvertAdenaLimit` the player gives `BankingAdenaCount` Adena for the
    /// Gold Bars.
    pub fn banking_auto_convert(&mut self, player: ObjectId) {
        let b = self.settings.custom_misc.banking.clone();
        if !b.auto_convert_enabled {
            return;
        }
        if self.adena_of(player) <= b.auto_convert_adena_limit {
            return;
        }
        // `inventory.reduceAdena(COMPENSATE, count, player, null)`: nothing happens (no message)
        // when the purse has less than the count.
        self.reduce_adena(player, i64::from(b.adena_count), false);
        self.add_item(player, GOLD_BAR_ID, i64::from(b.goldbar_count), false);
        let bars = if b.goldbar_count > 1 { "Bars." } else { "Bar." };
        self.send_message(
            player,
            &format!(
                "Automatically converted {} Adena to {} Gold {bars}",
                b.adena_count, b.goldbar_count
            ),
        );
    }

    /// The Banking switches (for the script).
    pub fn banking_auto_convert_enabled(&self) -> bool {
        self.settings.custom_misc.banking.auto_convert_enabled
    }
}
