//! `config/Custom/ScreenWelcomeMessage.ini`: a text in the middle of the screen at login.
// Java: network/clientpackets/EnterWorld.java (`ScreenWelcomeMessageConfig.WELCOME_MESSAGE_ENABLED`)

use l2_protocol_game::server::ExShowScreenMessage;
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// `player.sendPacket(new ExShowScreenMessage(WELCOME_MESSAGE_TEXT, WELCOME_MESSAGE_TIME))`.
    pub(crate) fn screen_welcome_on_enter(&self, id: ObjectId) {
        let s = &self.settings.custom_misc.screen_welcome;
        if !s.enabled {
            return;
        }
        let packet = ExShowScreenMessage::text(&s.text, s.time_ms);
        self.send(id, &packet);
    }
}
