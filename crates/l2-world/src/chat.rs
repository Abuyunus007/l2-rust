//! Chat: `Say2` and the chat handlers of the channels a plain character can use.
// Java: network/clientpackets/Say2.java (runImpl)
// Java: dist/game/data/scripts/handlers/chat/channels/{ChatGeneral,ChatShout,ChatTrade,
// Java: ChatWhisper,ChatHeroVoice}.java
//
// There are no regions yet (M3), so "players in range" and "players in the same region" both
// mean everybody in the world. Left out because the systems do not exist: chat bans, jail, cursed
// weapons, block lists, factions, the say filter (`UseChatFilter`), voiced commands, fake
// players, the global-chat flood protector, and the party, clan, alliance, petition and room
// channels (their messages are dropped with a debug line, like Java does for a channel without
// handler).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, ChatType, CreatureSay, SayBody, SenderField, SystemMessage};
use l2_util::ObjectId;
use tracing::{debug, info};

use crate::command::LeaveReason;
use crate::data::ChatBroadcast;
use crate::world::World;

/// Java: `_text.length() > 105` without an item link.
const MAX_TEXT_LEN: usize = 105;
/// Java: `_text.length() > 500` with an item link (the `\b` character, code 8).
const MAX_LINKED_TEXT_LEN: usize = 500;
/// The character the client puts around an item link.
const ITEM_LINK_MARK: char = '\u{8}';
/// Level needed for trade chat (hard-coded in Java's `ChatTrade`).
const TRADE_CHAT_LEVEL: i32 = 20;

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

        if chat_type == ChatType::PetitionPlayer && is_gm {
            chat_type = ChatType::PetitionGm;
        }

        if self.settings.log_chat {
            let Some(player) = self.players.get(&id) else {
                return;
            };
            if chat_type == ChatType::Whisper {
                info!(
                    target: "chat",
                    "{chat_type:?} [{} to {}] {text}",
                    player.data.name,
                    target.as_deref().unwrap_or("")
                );
            } else {
                info!(target: "chat", "{chat_type:?} [{}] {text}", player.data.name);
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
        let find_mark = |from: usize| units.get(from..).and_then(|s| s.iter().position(|&u| u == mark)).map(|p| p + from);
        let find_id = |from: usize| {
            units
                .get(from..)
                .and_then(|s| s.windows(3).position(|w| w == [b'I'.into(), b'D'.into(), b'='.into()]))
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

    /// Java `Player.sendMessage(String)`.
    fn send_message(&self, to: ObjectId, text: &str) {
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

    fn broadcast_say(&self, packet: &CreatureSay) {
        let ids: Vec<ObjectId> = self.players.keys().copied().collect();
        for to in ids {
            self.send(to, packet);
        }
    }

    // Java: ChatGeneral.onChat (without voiced commands, chat ban and block list)
    fn chat_general(&self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
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
        // World.forEachVisibleObjectInRange(activeChar, Player.class, 1250, ...) and then the
        // speaker: with no regions yet that is everybody in the world, the speaker once.
        let packet = self.say_packet(id, ChatType::General, &player.data.name, text);
        self.broadcast_say(&packet);
    }

    // Java: ChatShout.onChat
    fn chat_shout(&self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if player.core.level < self.settings.minimum_chat_level && !is_gm {
            self.send_message(
                id,
                &format!("Players can Shout after Lv. {}.", self.settings.minimum_chat_level),
            );
            return;
        }
        let packet = self.say_packet(id, ChatType::Shout, &player.data.name, text);
        self.broadcast_by_mode(self.settings.global_chat, is_gm, &packet);
    }

    // Java: ChatTrade.onChat
    fn chat_trade(&self, id: ObjectId, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.core.level < TRADE_CHAT_LEVEL {
            self.send_message(id, "Players can use Trade chat after Lv. 20.");
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        let packet = self.say_packet(id, ChatType::Trade, &player.data.name, text);
        self.broadcast_by_mode(self.settings.trade_chat, is_gm, &packet);
    }

    /// The `DEFAULT_GLOBAL_CHAT` / `DEFAULT_TRADE_CHAT` switch: `ON` (and `GM` for a GM) goes to
    /// the speaker's region, `GLOBAL` to everybody, `OFF` to nobody (not even the speaker).
    /// There is one region for now, so the first two do the same as the third.
    fn broadcast_by_mode(&self, mode: ChatBroadcast, speaker_is_gm: bool, packet: &CreatureSay) {
        match mode {
            ChatBroadcast::On | ChatBroadcast::Global => self.broadcast_say(packet),
            ChatBroadcast::Gm if speaker_is_gm => self.broadcast_say(packet),
            ChatBroadcast::Gm | ChatBroadcast::Off => {}
        }
    }

    // Java: ChatWhisper.onChat
    fn chat_whisper(&self, id: ObjectId, target: Option<&str>, text: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
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
        match receiver {
            Some(receiver) => {
                let to_receiver = self.say_packet(id, ChatType::Whisper, &player.data.name, text);
                let to_sender = self.say_packet(id, ChatType::Whisper, &format!("->{}", receiver.data.name), text);
                self.send(receiver.core.object_id, &to_receiver);
                self.send(id, &to_sender);
            }
            None => {
                self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            }
        }
    }

    // Java: ChatHeroVoice.onChat; the heroes table is not loaded, so only a GM passes.
    fn chat_hero_voice(&self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if !is_gm {
            self.send_message(id, "Only Heroes can enter the Hero channel.");
            return;
        }
        // A GM would reach the hero channel after the flood check; heroes do not exist yet.
        debug!("hero voice of a GM is not ported yet");
    }
}
