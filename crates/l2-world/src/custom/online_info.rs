//! `config/Custom/OnlineInfo.ini`: the `.online` voiced command.

use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// `Online.onCommand`: the command only counts when it is exactly `online`.
    pub(crate) fn online_voiced(&mut self, player: ObjectId, command: &str) -> bool {
        if command != "online" || !self.settings.custom_misc.online_command {
            return true;
        }
        // `World.getPlayers().size()`: every player in the world, offline traders included.
        let count = self.players.len();
        if count > 1 {
            self.send_message(player, &format!("There are {count} players online!"));
        } else {
            self.send_message(player, "There is 1 player online!");
        }
        true
    }
}
