//! Titles: a clan member's title, the title colour items and the title reset.
// Java: network/clientpackets/{RequestGiveNickName,RequestChangeNicknameColor,RequestResetNickname}.java
// Java: entity/actor/Creature.java (setTitle), entity/actor/Player.java (broadcastTitleInfo)
//
// Nothing is written to the database here: the title and its colour are stored with the character
// when it is saved, like in Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExRequestChangeNicknameColor, SystemMessage};
use l2_util::ObjectId;

use super::model::ClanAccess;
use crate::world::World;

/// Java `RequestChangeNicknameColor.COLORS`, indexed by the `colorNum` of the packet. The values
/// are the ints Java stores (the client reads them as BGR).
const COLORS: [i32; 10] = [
    0x9393FF, // Pink
    0x7C49FC, // Rose Pink
    0x97F8FC, // Lemon Yellow
    0xFA9AEE, // Lilac
    0xFF5D93, // Cobalt Violet
    0x00FCA0, // Mint Green
    0xA0A601, // Peacock Green
    0x7898AF, // Yellow Ochre
    0x486295, // Chocolate
    0x999999, // Silver
];

/// The title colour `RequestResetNickname` sets.
// NOTE(java-quirk): this is not the default title colour of a character (`0xECF9A2`).
const RESET_TITLE_COLOR: i32 = 0x00FF_FF77;

/// Java `Creature.setTitle(value)` for a player: a title of more than 21 characters is cut to its
/// first 20 (Java counts UTF-16 units).
// NOTE(java-quirk): `substring(0, 20)` when `length() > 21`, so a title of 21 characters stays
// whole and one of 22 loses two.
fn java_player_title(value: &str) -> String {
    let units: Vec<u16> = value.encode_utf16().collect();
    if units.len() > 21 {
        String::from_utf16_lossy(&units[..20])
    } else {
        value.to_owned()
    }
}

impl World {
    /// Java `Creature.setTitle(value)` of a player.
    fn set_player_title(&mut self, id: ObjectId, value: &str) {
        if let Some(player) = self.players.get_mut(&id) {
            player.data.title = java_player_title(value);
        }
    }

    /// Java `RequestGiveNickName.runImpl`.
    pub(crate) fn on_request_give_nick_name(&mut self, id: ObjectId, target: &str, title: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        // Noblesse can bestow a title to themselves.
        if player.data.profile.noble && target.to_lowercase() == player.data.name.to_lowercase() {
            self.set_player_title(id, title);
            self.send(id, &SystemMessage::new(SystemMessageId::YourTitleHasBeenChanged));
            self.broadcast_title_info(id);
            return;
        }
        // Can the player change/give a title?
        if !self.has_clan_access(id, ClanAccess::AssignTitle) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let clan_id = self.clan_id_of(id);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.level < 3 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::APlayerCanOnlyBeGrantedATitleIfTheClanIsLevel3OrAbove),
            );
            return;
        }
        match self.clan_member_by_name(clan_id, target) {
            Some(member) if member.online && self.players.contains_key(&member.object_id) => {
                // Is the target from the same clan? It is a member of this clan.
                self.set_player_title(member.object_id, title);
                self.send(
                    member.object_id,
                    &SystemMessage::new(SystemMessageId::YourTitleHasBeenChanged),
                );
                self.broadcast_title_info(member.object_id);
            }
            Some(_) => {
                self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            }
            None => {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::TheTargetMustBeAClanMember),
                );
            }
        }
    }

    /// Java `RequestChangeNicknameColor.runImpl`: a title colour item is used up for a new title
    /// and colour.
    pub(crate) fn on_request_change_nickname_color(
        &mut self,
        id: ObjectId,
        color_num: i32,
        title: &str,
        item_object_id: i32,
    ) {
        let Some(color) = usize::try_from(color_num)
            .ok()
            .and_then(|n| COLORS.get(n))
            .copied()
        else {
            return;
        };
        let Some(item_id) = self
            .inventory_item(id, ObjectId(item_object_id))
            .map(|item| item.item_id)
        else {
            return;
        };
        let is_nickname_color_item = self
            .item_templates()
            .and_then(|templates| {
                templates
                    .get(item_id)
                    .and_then(|t| t.etc().and_then(|etc| etc.handler.clone()))
            })
            .is_some_and(|handler| handler.eq_ignore_ascii_case("NicknameColor"));
        if !is_nickname_color_item {
            return;
        }
        if self.destroy_item_by_object_id(id, ObjectId(item_object_id), 1, true) {
            self.set_player_title(id, title);
            if let Some(player) = self.players.get_mut(&id) {
                player.data.profile.title_color = color;
            }
            self.broadcast_user_info(id);
        }
    }

    /// Java `NicknameColor.onItemUse` (a player's item handler): the window asks which colour.
    pub(crate) fn nickname_color_on_use(&mut self, id: ObjectId, item: &crate::player::ItemData) -> bool {
        self.send(
            id,
            &ExRequestChangeNicknameColor {
                item_object_id: item.object_id.0,
            },
        );
        true
    }

    /// Java `RequestResetNickname.runImpl`.
    pub(crate) fn on_request_reset_nickname(&mut self, id: ObjectId) {
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.title_color = RESET_TITLE_COLOR;
        }
        self.set_player_title(id, "");
        self.broadcast_title_info(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_title_is_cut_like_java() {
        assert_eq!(java_player_title("short"), "short");
        let twenty_one = "a".repeat(21);
        assert_eq!(java_player_title(&twenty_one), twenty_one);
        // 22 characters: the first 20 stay.
        assert_eq!(java_player_title(&"b".repeat(22)), "b".repeat(20));
    }
}
