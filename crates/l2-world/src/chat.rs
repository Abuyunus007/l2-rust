//! Chat: `Say2` and the chat handlers of the channels a plain character can use.
// Java: network/clientpackets/Say2.java (runImpl)
// Java: dist/game/data/scripts/handlers/chat/channels/{ChatGeneral,ChatShout,ChatTrade,
// Java: ChatWhisper,ChatHeroVoice,ChatParty}.java
//
// General chat reaches the players within 1250 of the speaker (M3-WREG-01 regions). Shout and
// trade in `ON` mode go to the players in the speaker's map region (M3-TELE-01's map regions),
// `GLOBAL` to everybody. Chat bans and jail come from the punishments (`punishments.rs`). Left
// out because the systems do not exist: cursed weapons, factions, the say filter
// (`UseChatFilter`), voiced commands, fake players, the global-chat flood protector, and the
// clan, alliance, petition and room channels (their messages are dropped with a debug line, like
// Java does for a channel without handler). The party channel is `chat_party` (M6-PARTY-01).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, ChatType, CreatureSay, SayBody, SenderField, SystemMessage};
use l2_util::ObjectId;
use tracing::{debug, info};

use crate::command::LeaveReason;
use crate::data::ChatBroadcast;
use crate::flood::FloodKind;
use crate::world::World;

/// Java: `_text.length() > 105` without an item link.
const MAX_TEXT_LEN: usize = 105;
/// Java: `_text.length() > 500` with an item link (the `\b` character, code 8).
const MAX_LINKED_TEXT_LEN: usize = 500;
/// The character the client puts around an item link.
const ITEM_LINK_MARK: char = '\u{8}';
/// Java `ChatGeneral`: `forEachVisibleObjectInRange(activeChar, Player.class, 1250, ...)`.
const GENERAL_CHAT_RANGE: i32 = 1250;

/// Level needed for trade chat (hard-coded in Java's `ChatTrade`).
const TRADE_CHAT_LEVEL: i32 = 20;

/// Java `ChatType.name()` (the constant name, which the chat log writes).
fn java_chat_name(chat_type: ChatType) -> &'static str {
    match chat_type {
        ChatType::General => "GENERAL",
        ChatType::Shout => "SHOUT",
        ChatType::Whisper => "WHISPER",
        ChatType::Party => "PARTY",
        ChatType::Clan => "CLAN",
        ChatType::Gm => "GM",
        ChatType::PetitionPlayer => "PETITION_PLAYER",
        ChatType::PetitionGm => "PETITION_GM",
        ChatType::Trade => "TRADE",
        ChatType::Alliance => "ALLIANCE",
        ChatType::Announcement => "ANNOUNCEMENT",
        ChatType::Boat => "BOAT",
        ChatType::Friend => "FRIEND",
        ChatType::MsnChat => "MSNCHAT",
        ChatType::PartyMatchRoom => "PARTYMATCH_ROOM",
        ChatType::PartyRoomCommander => "PARTYROOM_COMMANDER",
        ChatType::PartyRoomAll => "PARTYROOM_ALL",
        ChatType::HeroVoice => "HERO_VOICE",
        ChatType::CriticalAnnounce => "CRITICAL_ANNOUNCE",
        ChatType::ScreenAnnounce => "SCREEN_ANNOUNCE",
        ChatType::Battlefield => "BATTLEFIELD",
        ChatType::MpccRoom => "MPCC_ROOM",
        ChatType::NpcGeneral => "NPC_GENERAL",
        ChatType::NpcShout => "NPC_SHOUT",
    }
}

impl World {
    /// Java `Say2.runImpl` followed by the chat handler of the channel.
    pub(crate) fn on_say(&mut self, id: ObjectId, chat_type: i32, text: String, target: Option<String>) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let is_gm = self.data.access_level(player.data.access_level).is_gm;

        let Some(mut chat_type) = ChatType::from_client_id(chat_type) else {
            // Java: warning, ActionFailed, Disconnection.storeAndDeleteWith(LeaveWorld).
            debug!("{}: Say2 with invalid type {chat_type}", player.data.name);
            self.send(id, &ActionFailed);
            self.leave(id, LeaveReason::Logout);
            return;
        };
        if text.is_empty() {
            debug!("{}: sending empty text. Possible packet hack!", player.data.name);
            self.send(id, &ActionFailed);
            self.leave(id, LeaveReason::Logout);
            return;
        }

        // Java: `String.length()` counts UTF-16 units. The client's item links contain code 8.
        let length = text.encode_utf16().count();
        let has_link = text.contains(ITEM_LINK_MARK);
        if !is_gm && ((has_link && length > MAX_LINKED_TEXT_LEN) || (!has_link && length > MAX_TEXT_LEN)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::WhenAUserSKeyboardInputExceedsACertainCumulativeScoreAChatBanWillBeAppliedThisIsDoneToDiscourageSpammingPleaseAvoidPostingTheSameMessageMultipleTimesDuringAShortPeriod),
            );
            return;
        }

        // `player.isChatBanned()`: a chat ban punishment (the bot report debuffs start one). A `.`
        // starts a voiced command and is let through.
        if self.is_chat_banned(id) && !text.starts_with('.') {
            if self.is_affected_by_effect_type(id, crate::effects::EffectType::ChatBlock) {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoChattingIsNotAllowed,
                    ),
                );
            } else if self.punishment_settings().ban_chat_channels.contains(&chat_type) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::ChattingIsCurrentlyProhibited),
                );
            }
            return;
        }

        if self.is_jailed(id)
            && self.punishment_settings().jail_disable_chat
            && matches!(
                chat_type,
                ChatType::Whisper | ChatType::Shout | ChatType::Trade | ChatType::HeroVoice
            )
        {
            self.send_message(id, "You can not chat with players outside of the jail.");
            return;
        }

        if chat_type == ChatType::PetitionPlayer && is_gm {
            chat_type = ChatType::PetitionGm;
        }

        if self.settings.log_chat {
            let Some(player) = self.players.get(&id) else {
                return;
            };
            // Java: `sb.append(chatType.name()).append(" [").append(player)`, where `player` is
            // `WorldObject.toString()` = `Player:<name>[<objectId>]`.
            let name = java_chat_name(chat_type);
            let who = format!("Player:{}[{}]", player.data.name, id.0);
            if chat_type == ChatType::Whisper {
                info!(
                    target: "chat",
                    "{name} [{who} to {}] {text}",
                    target.as_deref().unwrap_or("")
                );
            } else {
                info!(target: "chat", "{name} [{who}] {text}");
            }
        }

        if has_link && !self.publish_items(id, &text) {
            return;
        }

        match chat_type {
            ChatType::General => self.chat_general(id, &text),
            ChatType::Shout => self.chat_shout(id, &text),
            ChatType::Trade => self.chat_trade(id, &text),
            ChatType::Whisper => self.chat_whisper(id, target.as_deref(), &text),
            ChatType::HeroVoice => self.chat_hero_voice(id),
            ChatType::Party => self.chat_party(id, &text),
            ChatType::PartyRoomAll | ChatType::PartyRoomCommander => {
                self.chat_command_channel(id, chat_type, &text)
            }
            ChatType::Battlefield => self.chat_battlefield(id, &text),
            ChatType::PetitionPlayer | ChatType::PetitionGm => self.chat_petition(id, chat_type, &text),
            other => debug!("No handler for chat type {other:?} (not ported yet)"),
        }
    }

    /// Java `Say2.parseAndPublishItem`: every `ID=` link must name an item of the speaker.
    ///
    /// `Item.publish()` (which lets others look the item up by its link) has no effect here: the
    /// link lookup packets are not ported.
    fn publish_items(&self, id: ObjectId, text: &str) -> bool {
        let Some(player) = self.players.get(&id) else {
            return false;
        };
        let units: Vec<u16> = text.encode_utf16().collect();
        let mark = u16::from(ITEM_LINK_MARK as u8);
        let find_mark = |from: usize| {
            units
                .get(from..)
                .and_then(|s| s.iter().position(|&u| u == mark))
                .map(|p| p + from)
        };
        let find_id = |from: usize| {
            units
                .get(from..)
                .and_then(|s| {
                    s.windows(3)
                        .position(|w| w == [b'I'.into(), b'D'.into(), b'='.into()])
                })
                .map(|p| p + from)
        };
        let mut from = 0;
        // `while ((pos1 = _text.indexOf(8, pos1)) > -1)`; Java starts at -1, which acts as 0.
        while let Some(start) = find_mark(from) {
            let Some(mut pos) = find_id(start) else {
                return false;
            };
            pos += 3;
            let mut digits = String::new();
            // `Character.isDigit(_text.charAt(pos))` throws past the end of the text.
            loop {
                let Some(&unit) = units.get(pos) else {
                    return false;
                };
                match char::from_u32(u32::from(unit)).filter(char::is_ascii_digit) {
                    Some(c) => digits.push(c),
                    None => break,
                }
                pos += 1;
            }
            // `Integer.parseInt` throws for no digits and for a number over an int.
            let Ok(object_id) = digits.parse::<i32>() else {
                return false;
            };
            // `World.findObject(id) instanceof Item` and the speaker must own it.
            if !player.data.items.iter().any(|i| i.object_id.0 == object_id) {
                debug!(
                    "{} tried to publish an item that is not theirs: {object_id}",
                    player.data.name
                );
                return false;
            }
            // `pos1 = _text.indexOf(8, pos) + 1; if (pos1 == 0) // Missing ending tag.`
            let Some(end) = find_mark(pos) else {
                return false;
            };
            from = end + 1;
        }
        true
    }

    /// The first lines of the channel handlers: `isChatBanned() && BAN_CHAT_CHANNELS.contains(type)`
    /// ("chatting is prohibited") and, with `jail` (every channel but general), `JAIL_DISABLE_CHAT &&
    /// isJailed() && !isGM()`. `true` when the message must not go out.
    pub(crate) fn channel_refuses(&self, id: ObjectId, chat_type: ChatType, jail: bool) -> bool {
        if self.is_chat_banned(id) && self.punishment_settings().ban_chat_channels.contains(&chat_type) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ChattingIsCurrentlyProhibitedIfYouTryToChatBeforeTheProhibitionIsRemovedTheProhibitionTimeWillIncreaseEvenFurther,
                ),
            );
            return true;
        }
        if jail && self.punishment_settings().jail_disable_chat && self.is_jailed(id) && !self.is_gm(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ChattingIsCurrentlyProhibited),
            );
            return true;
        }
        false
    }

    /// Java `Player.sendMessage(String)`.
    pub(crate) fn send_message(&self, to: ObjectId, text: &str) {
        self.send(to, &SystemMessage::text(text));
    }

    fn say_packet(&self, from: ObjectId, chat_type: ChatType, sender_name: &str, text: &str) -> CreatureSay {
        CreatureSay {
            sender_object_id: from.0,
            chat_type,
            sender: SenderField::Name(sender_name.to_string()),
            message_id: -1,
            body: SayBody::Text(text.to_string()),
        }
    }

    // Java: `World.getPlayers()` with `!BlockList.isBlocked(player, activeChar)`.
    fn broadcast_say(&self, id: ObjectId, packet: &CreatureSay) {
        // In object id order, so the same input gives the same output (D-13).
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for to in ids {
            if !self.is_blocked(to, id) {
                self.send(to, packet);
            }
        }
    }

    // Java: ChatGeneral.onChat (without voiced commands and block list)
    fn chat_general(&mut self, id: ObjectId, text: &str) {
        // Voiced commands first: a text a handler took is not said (`vcdUsed`).
        if self.wedding_voiced_command(id, text) {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if self.channel_refuses(id, ChatType::General, false) {
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if player.core.level < self.settings.minimum_chat_level && !is_gm {
            self.send_message(
                id,
                &format!(
                    "Players can use general chat after Lv. {}.",
                    self.settings.minimum_chat_level
                ),
            );
            return;
        }
        // World.forEachVisibleObjectInRange(activeChar, Player.class, 1250, ...), then the speaker.
        let packet = self.say_packet(id, ChatType::General, &player.data.name, text);
        for to in self.visible_objects_in_range(id, GENERAL_CHAT_RANGE) {
            // Java: `!BlockList.isBlocked(player, activeChar)`.
            if self.players.contains_key(&to) && !self.is_blocked(to, id) {
                self.send(to, &packet);
            }
        }
        self.send(id, &packet);
    }

    // Java: ChatShout.onChat
    fn chat_shout(&mut self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if self.channel_refuses(id, ChatType::Shout, true) {
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if player.core.level < self.settings.minimum_chat_level && !is_gm {
            self.send_message(
                id,
                &format!(
                    "Players can Shout after Lv. {}.",
                    self.settings.minimum_chat_level
                ),
            );
            return;
        }
        // Only the `GLOBAL` mode is flood protected.
        if self.settings.global_chat == ChatBroadcast::Global
            && !is_gm
            && !self.flood_allows(id, FloodKind::GlobalChat)
        {
            self.send_message(id, "Do not spam shout channel.");
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let packet = self.say_packet(id, ChatType::Shout, &player.data.name, text);
        self.broadcast_by_mode(id, self.settings.global_chat, is_gm, &packet, true);
    }

    // Java: ChatTrade.onChat
    fn chat_trade(&mut self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if self.channel_refuses(id, ChatType::Trade, true) {
            return;
        }
        if player.core.level < TRADE_CHAT_LEVEL {
            self.send_message(id, "Players can use Trade chat after Lv. 20.");
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        // Only the `GLOBAL` mode is flood protected.
        if self.settings.trade_chat == ChatBroadcast::Global
            && !is_gm
            && !self.flood_allows(id, FloodKind::GlobalChat)
        {
            self.send_message(id, "Do not spam trade channel.");
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let packet = self.say_packet(id, ChatType::Trade, &player.data.name, text);
        self.broadcast_by_mode(id, self.settings.trade_chat, is_gm, &packet, false);
    }

    /// The `DEFAULT_GLOBAL_CHAT` / `DEFAULT_TRADE_CHAT` switch: `ON` (and `GM` for a GM) goes to
    /// the speaker's region, `GLOBAL` to everybody, `OFF` to nobody (not even the speaker).
    /// There is one region for now, so the first two do the same as the third.
    fn broadcast_by_mode(
        &self,
        id: ObjectId,
        mode: ChatBroadcast,
        speaker_is_gm: bool,
        packet: &CreatureSay,
        // Java `ChatShout` also drops the players the speaker blocks; `ChatTrade` does not.
        mutual_block: bool,
    ) {
        match mode {
            // `World.getPlayers()` with the same `getMapRegionLocId` and instance as the speaker.
            ChatBroadcast::On => self.broadcast_say_in_map_region(id, packet, mutual_block),
            ChatBroadcast::Gm if speaker_is_gm => self.broadcast_say_in_map_region(id, packet, mutual_block),
            // `World.getPlayers()`: everybody.
            ChatBroadcast::Global => self.broadcast_say(id, packet),
            ChatBroadcast::Gm | ChatBroadcast::Off => {}
        }
    }

    /// Java `MapRegionData.getMapRegionLocId(player)` of a player (0 without map region data).
    fn map_region_loc_id(&self, id: ObjectId) -> i32 {
        match (self.zoning.map_regions.as_ref(), self.players.get(&id)) {
            (Some(regions), Some(p)) => regions.map_region_loc_id(p.core.x, p.core.y),
            _ => 0,
        }
    }

    fn broadcast_say_in_map_region(&self, id: ObjectId, packet: &CreatureSay, mutual_block: bool) {
        let region = self.map_region_loc_id(id);
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort();
        for to in ids {
            if self.map_region_loc_id(to) == region
                && self.objects.get(&to).map_or(0, |e| e.instance_id) == instance
                && !self.is_blocked(to, id)
                && !(mutual_block && self.is_blocked(id, to))
            {
                self.send(to, packet);
            }
        }
    }

    // Java: ChatWhisper.onChat
    fn chat_whisper(&mut self, id: ObjectId, target: Option<&str>, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if self.channel_refuses(id, ChatType::Whisper, true) {
            return;
        }
        let sender_silent = player.gm.silence;
        // "Return if no target is set."
        let Some(target) = target else {
            return;
        };
        // `World.getPlayer(name)`: `CharInfoTable.getIdByName` compares ignoring case.
        let wanted = target.to_lowercase();
        let receiver = if target.is_empty() {
            None
        } else {
            self.players
                .values()
                .find(|p| p.data.name.to_lowercase() == wanted)
        };
        // Java `receiver.isSilenceMode(activeChar.getObjectId())`: a receiver in silence mode is
        // answered like an offline one.
        let receiver = receiver.filter(|r| !r.gm.is_silent_for(id, self.settings.silence_mode_exclude));
        let mut excluded_for = None;
        match receiver {
            Some(receiver) => {
                let receiver_id = receiver.core.object_id;
                if self.punishment_settings().jail_disable_chat
                    && self.is_jailed(receiver_id)
                    && !self.is_gm(id)
                {
                    self.send_message(id, "Player is in jail.");
                    return;
                }
                if self.is_chat_banned(receiver_id) {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::ThatPersonIsInMessageRefusalMode),
                    );
                    return;
                }
                // Java: `!BlockList.isBlocked(receiver, activeChar)`: the receiver refuses messages
                // or blocks the sender.
                if self.is_blocked(receiver_id, id) {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::ThatPersonIsInMessageRefusalMode),
                    );
                    return;
                }
                let to_receiver = self.say_packet(id, ChatType::Whisper, &player.data.name, text);
                let to_sender =
                    self.say_packet(id, ChatType::Whisper, &format!("->{}", receiver.data.name), text);
                self.send(receiver.core.object_id, &to_receiver);
                self.send(id, &to_sender);
                // "Allow receiver to send PMs to this char, which is in silence mode."
                if self.settings.silence_mode_exclude && sender_silent {
                    excluded_for = Some(receiver.core.object_id);
                }
            }
            None => {
                self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            }
        }
        if let Some(receiver) = excluded_for
            && let Some(sender) = self.players.get_mut(&id)
        {
            sender.gm.add_silence_exclusion(receiver);
        }
    }

    // Java: ChatBattlefield.onChat (M7-TW-01): while the Territory War channel is open, everybody
    // of the speaker's side hears it.
    fn chat_battlefield(&self, id: ObjectId, text: &str) {
        let side = self.siege_side_of(id);
        if !self.is_tw_channel_open() || side <= 0 {
            return;
        }
        if self.is_chat_banned(id)
            && self
                .punishment_settings()
                .ban_chat_channels
                .contains(&ChatType::Battlefield)
        {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ChattingIsCurrentlyProhibitedIfYouTryToChatBeforeTheProhibitionIsRemovedTheProhibitionTimeWillIncreaseEvenFurther,
                ),
            );
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let packet = self.say_packet(id, ChatType::Battlefield, &player.data.name, text);
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for to in ids {
            if self.siege_side_of(to) == side {
                self.send(to, &packet);
            }
        }
    }

    // Java: ChatParty.onChat (without the jail rule and the block list)
    fn chat_party(&self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let Some(party) = self.party_of(id) else {
            self.send_message(id, "You are not in a party.");
            return;
        };
        // `getParty().broadcastCreatureSay(new CreatureSay(...), activeChar)`: every member,
        // the speaker too.
        let packet = self.say_packet(id, ChatType::Party, &player.data.name, text);
        self.broadcast_party_say(id, party, &packet);
    }

    /// Java `AbstractPlayerGroup.broadcastCreatureSay(msg, broadcaster)`: every member but the
    /// ones who refuse the speaker's messages (the speaker is a member too).
    fn broadcast_party_say(&self, speaker: ObjectId, party: crate::party::PartyId, packet: &CreatureSay) {
        for member in self.party_members(party) {
            if !self.is_blocked(member, speaker) {
                self.send(member, packet);
            }
        }
    }

    // Java: ChatHeroVoice.onChat; the heroes table is not loaded, so only a GM passes.
    fn chat_hero_voice(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if self.channel_refuses(id, ChatType::HeroVoice, true) {
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if !is_gm {
            self.send_message(id, "Only Heroes can enter the Hero channel.");
            return;
        }
        if !self.flood_allows(id, FloodKind::HeroVoice) {
            self.send_message(
                id,
                "Action failed. Heroes are only able to speak in the global channel once every 10 seconds.",
            );
            return;
        }
        // A GM would reach the hero channel after the flood check; heroes do not exist yet.
        debug!("hero voice of a GM is not ported yet");
    }
}
