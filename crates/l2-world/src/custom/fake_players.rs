//! Fake players, `config/Custom/FakePlayers.ini` (M9-C04): NPCs with a `<fakePlayer>` look that
//! the clients see as players.
// The fake players are the NPCs of the datapack that have a `<fakePlayer>` element
// (`NpcTemplate::fake_player_info`); `NpcLoadOptions::fake_players_enabled` keeps them out of the
// templates unless `EnableFakePlayers` is on, so with the switch off nothing here finds one. The
// data (names, talkable names, chat lines) is [`FakePlayerWorld`]. The pvp flag blink of a fake
// player is the script `custom/FakePlayers/PvpFlaggingStopTask` (`l2-scripts`).
//
// # Left out
// - The `PvpRewardItem` and `PvpAnnounce` custom features Java runs when a fake player is killed
//   (`Npc.doDie`) or kills (`Player.doDie`): they are their own custom cards.
// - Nothing of M9-L01's list: `//reload fakeplayerchat` (`ControlRequest::ReloadFakePlayerChat`),
//   the bot report, the follow after a click, the herbs and the aggro of monsters are ported.

use std::collections::{BTreeMap, HashMap};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    CharInfo, ChatType, CreatureSay, ExPrivateStoreSetWholeMsg, PrivateStoreMsgBuy, PrivateStoreMsgSell,
    RecipeShopMsg, SayBody, SenderField, SystemMessage,
};
use l2_util::ObjectId;
use tracing::info;

use crate::admin::{AdminHandler, AdminHandlers, JavaException};
use crate::effects::SkillKey;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `FakePlayerChatManager.MIN_DELAY` and `MAX_DELAY`, ms: how long a fake player takes to answer.
const MIN_DELAY: i32 = 5000;
const MAX_DELAY: i32 = 15000;

/// `FakePlayersConfig`. `Default` is the Java default of each key (everything off).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FakePlayerSettings {
    /// `EnableFakePlayers`.
    pub enabled: bool,
    /// `FakePlayerChat` (`FAKE_PLAYER_CHAT`).
    pub chat: bool,
    /// `FakePlayerUseShots` (`FAKE_PLAYER_USE_SHOTS`).
    pub use_shots: bool,
    /// `FakePlayerKillsRewardPvP` (`FAKE_PLAYER_KILL_PVP`).
    pub kill_pvp: bool,
    /// `FakePlayerUnflaggedKillsKarma` (`FAKE_PLAYER_KILL_KARMA`).
    pub kill_karma: bool,
    /// `FakePlayerAutoAttackable`.
    pub auto_attackable: bool,
    /// `FakePlayerAggroMonsters`.
    pub aggro_monsters: bool,
    /// `FakePlayerAggroPlayers`.
    pub aggro_players: bool,
    /// `FakePlayerAggroFPC`.
    pub aggro_fpc: bool,
    /// `FakePlayerCanDropItems`.
    pub can_drop_items: bool,
    /// `FakePlayerCanPickup`.
    pub can_pickup: bool,
}

/// One `<fakePlayerChat>` of `data/FakePlayerChatData.xml` (Java `FakePlayerChatHolder`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePlayerChatEntry {
    /// `fpcName`: a fake player's name or `ALL`.
    pub fpc_name: String,
    /// `searchMethod`: `EQUALS`, `STARTS_WITH` or `CONTAINS`.
    pub search_method: String,
    /// `searchText` split at `;`.
    pub search_text: Vec<String>,
    /// `answers` split at `;`.
    pub answers: Vec<String>,
}

/// One fake player name: Java's `_fakePlayerNames`, `_fakePlayerIds` and
/// `_talkableFakePlayerNames` for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakePlayerName {
    proper: String,
    npc_id: i32,
    talkable: bool,
}

/// An answer that waits for its random delay (`ThreadPool.schedule` of `manageChat`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingChat {
    player: ObjectId,
    fpc_name: String,
    message: String,
}

/// The fake player data and state the world holds (Java `FakePlayerData`,
/// `FakePlayerChatManager`, `Creature._karma` of the fake players).
#[derive(Debug, Default)]
pub struct FakePlayerWorld {
    /// By the lower case name.
    names: HashMap<String, FakePlayerName>,
    chat: Vec<FakePlayerChatEntry>,
    /// `Creature.getKarma()` of a fake player (only `Player.doDie` raises it).
    karma: HashMap<ObjectId, i32>,
    /// `Npc.getFakePlayerDrops()`: the ground items a fake player will pick up (the last one
    /// first).
    drops: HashMap<ObjectId, Vec<ObjectId>>,
    pending: BTreeMap<u64, PendingChat>,
    next_pending: u64,
}

impl World {
    /// `FakePlayersConfig.FAKE_PLAYERS_ENABLED`.
    pub fn fake_players_enabled(&self) -> bool {
        self.settings.fake_players.enabled
    }

    /// The fake player settings the world runs with.
    pub fn fake_player_settings(&self) -> &FakePlayerSettings {
        &self.settings.fake_players
    }

    /// Java `FakePlayerChatManager.load()` and the template part of `FakePlayerData`: the names of
    /// the fake player templates, and (with `FakePlayerChat`) the chat lines. Does nothing while
    /// the system is off. Call after the NPC data was handed over.
    pub fn load_fake_players(&mut self, chat: Vec<FakePlayerChatEntry>) {
        if !self.settings.fake_players.enabled {
            return;
        }
        let mut names = HashMap::new();
        if let Some(templates) = self.npc_world.templates_arc() {
            for template in templates.iter().filter(|t| t.fake_player) {
                let Some(look) = template.fake_player_info.as_ref() else {
                    continue;
                };
                names.insert(
                    template.name.to_lowercase(),
                    FakePlayerName {
                        proper: template.name.clone(),
                        npc_id: template.id,
                        talkable: look.fake_player_talkable,
                    },
                );
            }
        }
        info!("FakePlayerData: Loaded {} templates.", names.len());
        self.fake_players.names = names;
        if self.settings.fake_players.chat {
            info!("FakePlayerChatManager: Loaded {} chat templates.", chat.len());
            self.fake_players.chat = chat;
        }
    }

    /// The answer to `//reload fakeplayerchat` (Java `FakePlayerChatManager.load()`): with the
    /// system on, the templates are reported, and with the chat on the lines are replaced
    /// (`MESSAGES.clear()`, then the file). The GMs hear it either way.
    pub(crate) fn on_fake_player_chat_reloaded(
        &mut self,
        requester: ObjectId,
        chat: Vec<FakePlayerChatEntry>,
    ) {
        if self.settings.fake_players.enabled {
            info!(
                "FakePlayerData: Loaded {} templates.",
                self.fake_players.names.len()
            );
            if self.settings.fake_players.chat {
                info!("FakePlayerChatManager: Loaded {} chat templates.", chat.len());
                self.fake_players.chat = chat;
            }
        }
        let name = self.object_name(requester).unwrap_or_default();
        self.broadcast_message_to_gms(&format!("{name}: Reloaded Fake Player Chat data."));
        if self.players.contains_key(&requester) {
            crate::admin::reload::reload_warning(self, requester);
        }
    }

    /// Java `NpcTemplate.isFakePlayer()` of an NPC in the world.
    pub fn is_fake_player(&self, id: ObjectId) -> bool {
        self.npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.fake_player)
    }

    /// Java `npc.getTemplate().getFakePlayerInfo().isTalkable()` of a fake player in the world.
    pub(crate) fn fake_player_npc_talkable(&self, id: ObjectId) -> bool {
        self.npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .and_then(|t| t.fake_player_info.as_ref())
            .is_some_and(|look| look.fake_player_talkable)
    }

    /// Java `Npc.getName()` of a fake player (the name its `FakePlayerInfo` sends).
    pub(crate) fn fake_player_npc_name(&self, id: ObjectId) -> Option<String> {
        self.npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.name.clone())
    }

    /// Java `FakePlayerData.getProperName(name)`: `None` for a name no fake player has.
    pub fn fake_player_proper_name(&self, name: &str) -> Option<&str> {
        self.fake_players
            .names
            .get(&name.to_lowercase())
            .map(|n| n.proper.as_str())
    }

    /// Java `FakePlayerData.isTalkable(name)`.
    pub fn fake_player_is_talkable(&self, name: &str) -> bool {
        self.fake_players
            .names
            .get(&name.to_lowercase())
            .is_some_and(|n| n.talkable)
    }

    /// The names of every fake player template, lower case (`FakePlayerData`): the character
    /// creation refuses them (`CharacterCreate`).
    pub fn fake_player_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.fake_players.names.keys().cloned().collect();
        names.sort();
        names
    }

    /// Java `Creature.getKarma()` of a fake player.
    pub fn fake_player_karma(&self, id: ObjectId) -> i32 {
        self.fake_players.karma.get(&id).copied().unwrap_or(0)
    }

    /// Java `AdminFakePlayers` and `FakePlayerChatManager.sendChat`: the live NPC of the fake
    /// player called `fpc_name` (`SpawnTable.getAnySpawn(npcId).getLastSpawn()`).
    fn fake_player_npc(&self, fpc_name: &str) -> Option<ObjectId> {
        let npc_id = self
            .fake_players
            .names
            .values()
            .find(|n| n.proper == fpc_name)?
            .npc_id;
        // The lowest object id, so the same input gives the same output (D-13).
        self.npcs
            .iter()
            .filter(|(_, n)| n.npc_id == npc_id && !n.decayed)
            .map(|(id, _)| *id)
            .min()
    }

    /// Java `FakePlayerChatManager.sendChat(player, fpcName, message)`: the fake player whispers.
    pub fn fake_player_send_chat(&self, player: ObjectId, fpc_name: &str, message: &str) {
        if let Some(npc) = self.fake_player_npc(fpc_name) {
            self.send(
                player,
                &CreatureSay {
                    sender_object_id: npc.0,
                    chat_type: ChatType::Whisper,
                    sender: SenderField::Name(fpc_name.to_owned()),
                    message_id: -1,
                    body: SayBody::Text(message.to_owned()),
                },
            );
        }
    }

    /// Java `FakePlayerChatManager.manageChat(player, fpcName, message)`: the answer comes after
    /// 5 to 15 seconds.
    pub fn fake_player_manage_chat(&mut self, player: ObjectId, fpc_name: &str, message: &str) {
        self.fake_player_manage_chat_delayed(player, fpc_name, message, MIN_DELAY, MAX_DELAY);
    }

    /// Java `manageChat(player, fpcName, message, minDelay, maxDelay)`.
    pub fn fake_player_manage_chat_delayed(
        &mut self,
        player: ObjectId,
        fpc_name: &str,
        message: &str,
        min_delay: i32,
        max_delay: i32,
    ) {
        let delay = self.rng().get(min_delay, max_delay);
        let key = self.fake_players.next_pending;
        self.fake_players.next_pending += 1;
        self.fake_players.pending.insert(
            key,
            PendingChat {
                player,
                fpc_name: fpc_name.to_owned(),
                message: message.to_owned(),
            },
        );
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay).unwrap_or(0));
        self.timers.schedule_at(due, TimerKind::FakePlayerChat(key));
    }

    /// The timer of [`World::fake_player_manage_chat`]: Java `manageResponce`.
    pub(crate) fn on_fake_player_chat_timer(&mut self, key: u64) {
        let Some(pending) = self.fake_players.pending.remove(&key) else {
            return;
        };
        let PendingChat {
            player,
            fpc_name,
            message,
        } = pending;
        if !self.players.contains_key(&player) {
            return;
        }
        let text = message.to_lowercase();

        // Tricky question.
        if text.contains("can you see me")
            && let Some(npc) = self.fake_player_npc(&fpc_name)
        {
            let answer = if self.dist_2d(npc, player) < 3000.0 {
                let visible = self.can_see_target(npc, player)
                    && !self.objects.get(&player).is_some_and(|e| e.invisible);
                if visible {
                    self.pick3("i am not blind", "of course i can", "yes")
                } else {
                    self.pick3(
                        "i know you are around",
                        "not at the moment :P",
                        "no, where are you?",
                    )
                }
            } else {
                self.pick3("nope, can't see you", "nope", "no")
            };
            self.fake_player_send_chat(player, &fpc_name, answer);
            return;
        }

        let lines = self.fake_players.chat.clone();
        for line in &lines {
            if line.fpc_name != fpc_name && line.fpc_name != "ALL" {
                continue;
            }
            let matched = match line.search_method.as_str() {
                "EQUALS" => line.search_text.first().is_some_and(|t| text == *t),
                "STARTS_WITH" => line
                    .search_text
                    .first()
                    .is_some_and(|t| text.starts_with(t.as_str())),
                "CONTAINS" => line.search_text.iter().all(|word| text.contains(word.as_str())),
                _ => false,
            };
            if matched && !line.answers.is_empty() {
                let index = usize::try_from(self.rng().below(line.answers.len() as i32)).unwrap_or(0);
                self.fake_player_send_chat(player, &fpc_name, &line.answers[index]);
            }
        }
    }

    /// `Rnd.nextBoolean() ? a : Rnd.nextBoolean() ? b : c`.
    fn pick3<'a>(&mut self, a: &'a str, b: &'a str, c: &'a str) -> &'a str {
        if self.rng().next_bool() {
            a
        } else if self.rng().next_bool() {
            b
        } else {
            c
        }
    }

    /// `ChatWhisper.onChat`, the fake player part: whispering to the name of a fake player. `true`
    /// when the whisper was one (and is answered here).
    pub(crate) fn fake_player_whisper(&mut self, from: ObjectId, target: &str, text: &str) -> bool {
        if !self.settings.fake_players.enabled {
            return false;
        }
        let Some(name) = self.fake_player_proper_name(target).map(str::to_owned) else {
            return false;
        };
        if self.fake_player_is_talkable(target) {
            if self.settings.fake_players.chat {
                self.send(
                    from,
                    &CreatureSay {
                        sender_object_id: from.0,
                        chat_type: ChatType::Whisper,
                        sender: SenderField::Name(format!("->{name}")),
                        message_id: -1,
                        body: SayBody::Text(text.to_owned()),
                    },
                );
                self.fake_player_manage_chat(from, &name, text);
            } else {
                self.send(
                    from,
                    &SystemMessage::new(SystemMessageId::ThatPersonIsInMessageRefusalMode),
                );
            }
        } else {
            self.send(from, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
        }
        true
    }

    /// Java `Npc.sendInfo(player)` of a fake player: `FakePlayerInfo`, then the private store
    /// message the template shows.
    pub(crate) fn send_fake_player_info(&self, subject: ObjectId, viewer: ObjectId) {
        let Some(info) = self.fake_player_info(subject) else {
            return;
        };
        self.send(viewer, &info);
        let Some(look) = self
            .npcs
            .get(&subject)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .and_then(|t| t.fake_player_info.as_ref())
        else {
            return;
        };
        // Private store message support.
        if look.private_store_type > 0 && !look.private_store_message.is_empty() {
            let message = look.private_store_message.clone();
            let object_id = subject.0;
            match look.private_store_type {
                1 => {
                    self.send(viewer, &PrivateStoreMsgSell { object_id, message });
                }
                3 => {
                    self.send(viewer, &PrivateStoreMsgBuy { object_id, message });
                }
                5 => {
                    self.send(viewer, &RecipeShopMsg { object_id, message });
                }
                8 => {
                    self.send(viewer, &ExPrivateStoreSetWholeMsg { object_id, message });
                }
                _ => {}
            }
        }
    }

    /// Java `new FakePlayerInfo(npc)`. It has the wire layout of `CharInfo` (the same opcode and
    /// fields), so the packet is built as one.
    pub fn fake_player_info(&self, subject: ObjectId) -> Option<CharInfo> {
        use l2_protocol_game::server::charscreen::paperdoll;

        let npc = self.npcs.get(&subject)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let look = template.fake_player_info.as_ref()?;
        let stats = self.stats(subject)?;
        // `getMovementSpeedMultiplier()` and the speeds divided by it, `(int) Math.round(...)`.
        let move_multiplier = stats.movement_speed_multiplier();
        let speed = |value: f64| crate::stats::java_round(value / move_multiplier) as i32;
        let (run_speed, walk_speed) = (speed(stats.run_speed()), speed(stats.walk_speed()));
        let clan = self.clan(look.clan_id);

        let mut paperdoll_item_ids = [0; paperdoll::TOTAL_SLOTS];
        paperdoll_item_ids[paperdoll::HEAD] = look.equip_head;
        paperdoll_item_ids[paperdoll::RHAND] = look.equip_r_hand; // "Or dual hand."
        paperdoll_item_ids[paperdoll::LHAND] = look.equip_l_hand;
        paperdoll_item_ids[paperdoll::GLOVES] = look.equip_gloves;
        paperdoll_item_ids[paperdoll::CHEST] = look.equip_chest;
        paperdoll_item_ids[paperdoll::LEGS] = look.equip_legs;
        paperdoll_item_ids[paperdoll::FEET] = look.equip_feet;
        paperdoll_item_ids[paperdoll::CLOAK] = look.equip_cloak;
        paperdoll_item_ids[paperdoll::HAIR] = look.equip_hair;
        paperdoll_item_ids[paperdoll::HAIR2] = look.equip_hair2;

        Some(CharInfo {
            x: npc.core.x,
            y: npc.core.y,
            z: npc.core.z,
            vehicle_id: 0,
            object_id: subject.0,
            name: template.name.clone(),
            race: template.race.ordinal() as i32,
            is_female: template.sex == l2_data::npc::NpcSex::Female,
            base_class: look.class_id,
            paperdoll_item_ids,
            paperdoll_augmentation_ids: [0; paperdoll::TOTAL_SLOTS],
            talisman_slots: 0,
            can_equip_cloak: true,
            // `getPvpFlag()` is the script value: 1 in combat, 2 blinking.
            pvp_flag: self.npc_script_value(subject),
            karma: self.fake_player_karma(subject),
            m_atk_spd: stats.m_atk_spd(),
            p_atk_spd: stats.p_atk_spd() as i32,
            run_speed,
            walk_speed,
            swim_run_speed: speed(stats.swim_run_speed()),
            swim_walk_speed: speed(stats.swim_walk_speed()),
            fly_run_speed: if template.flying { run_speed } else { 0 },
            fly_walk_speed: if template.flying { walk_speed } else { 0 },
            move_multiplier,
            attack_speed_multiplier: stats.attack_speed_multiplier(),
            collision_radius: template.collision_radius,
            collision_height: template.collision_height,
            hair_style: look.hair,
            hair_color: look.hair_color,
            face: look.face,
            title: template.title.clone(),
            clan_id: clan.map_or(0, |c| c.id),
            clan_crest_id: clan.map_or(0, |c| c.crest_id),
            ally_id: clan.map_or(0, |c| c.ally_id),
            ally_crest_id: clan.map_or(0, |c| c.ally_crest_id),
            sitting: look.sitting,
            running: npc.movement.running,
            in_combat: self.is_in_combat(subject),
            alike_dead: self.is_dead(subject),
            invisible: self.objects.get(&subject).is_some_and(|e| e.invisible),
            mount_type: 0,
            private_store_type: u8::try_from(look.private_store_type).unwrap_or(0),
            cubics: Vec::new(),
            in_party_match_room: false,
            abnormal_visual_effects: self.abnormal_visual_effects(subject).0,
            water_or_flying: u8::from(npc.in_water),
            recom_have: i16::try_from(look.recommends).unwrap_or(0),
            // The wire value is `mount_npc_id + 1000000`; Java writes 0 for no mount.
            mount_npc_id: -1_000_000,
            class_id: look.class_id,
            enchant_effect: u8::try_from(look.weapon_enchant_level).unwrap_or(0),
            team: self.minigame_npc_team(subject),
            clan_crest_large_id: clan.map_or(0, |c| c.crest_large_id),
            noble: look.noble_level != 0,
            hero: look.hero,
            fishing: look.fishing,
            fish_x: look.bait_location_x,
            fish_y: look.bait_location_y,
            fish_z: look.bait_location_z,
            name_color: look.name_color,
            heading: npc.core.heading,
            pledge_class: look.pledge_status,
            pledge_type: 0,
            title_color: look.title_color,
            cursed_weapon_level: 0,
            clan_reputation_score: 0,
            transformation_display_id: 0,
            agathion_id: look.agathion_id,
            abnormal_visual_effect_special: self.abnormal_visual_effects(subject).1,
        })
    }

    /// `Monster.isAutoAttackable(attacker)` of a fake player: `None` for any other NPC.
    pub(crate) fn fake_player_auto_attackable(&self, target: ObjectId, attacker: ObjectId) -> Option<bool> {
        if !self.settings.fake_players.enabled || !self.is_fake_player(target) {
            return None;
        }
        let is_monster = |id: ObjectId| self.npcs.get(&id).is_some_and(|n| n.class.is_monster());
        Some(
            self.settings.fake_players.auto_attackable
                || self.is_in_combat(target)
                || is_monster(attacker)
                || self.npc_script_value(target) > 0,
        )
    }

    /// `Creature.doAttack` / `Creature.callSkill`: a fake player that fights a playable or another
    /// fake player gets its pvp flag (script value 1) and the blink script starts.
    pub(crate) fn fake_player_flag_on_attack(&mut self, attacker: ObjectId, target: ObjectId) {
        if !self.settings.fake_players.enabled
            || self.settings.fake_players.auto_attackable
            || !self.is_fake_player(attacker)
        {
            return;
        }
        if !(self.is_playable(target) || self.is_fake_player(target)) {
            return;
        }
        if self.npc_script_value(attacker) != 1 {
            self.set_npc_script_value(attacker, 1); // in combat
            self.broadcast_info(attacker); // update flag status
            self.fake_player_flag_check(attacker);
        }
    }

    /// `ScriptManager.getScript("PvpFlaggingStopTask").notifyEvent("FLAG_CHECK", npc, null)`.
    fn fake_player_flag_check(&mut self, npc: ObjectId) {
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        self.notify_named_script_event(
            "PvpFlaggingStopTask",
            "FLAG_CHECK",
            Some(crate::script::ScriptNpc {
                object_id: npc,
                id: npc_id,
            }),
            None,
        );
    }

    /// Java `ScriptManager.getScript(name).notifyEvent(event, npc, player)`; nothing when no such
    /// script is installed.
    pub fn notify_named_script_event(
        &mut self,
        script_name: &str,
        event: &str,
        npc: Option<crate::script::ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        if let Some(script) = self.scripts.by_name(script_name) {
            self.script_notify_event(script, event, npc, player);
        }
    }

    /// `Npc.doDie` for a fake player killed by a playable: the killer's pvp kills (the PK branch
    /// needs a negative karma, which a fake player never has).
    pub(crate) fn fake_player_on_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        if !self.settings.fake_players.enabled || !self.is_fake_player(id) {
            return;
        }
        let Some(player) = killer
            .and_then(|k| self.acting_player(k))
            .filter(|_| killer.is_some_and(|k| self.is_playable(k)))
        else {
            return;
        };
        if self.npc_script_value(id) == 0 && self.fake_player_karma(id) < 0 {
            if self.settings.fake_players.kill_karma {
                // Unreachable, kept for the shape of Java's code.
                self.custom_fake_player_killed_rewards(id, player, true);
            }
        } else if self.settings.fake_players.kill_pvp {
            if let Some(p) = self.players.get_mut(&player) {
                p.data.pvp_kills = p.data.pvp_kills.wrapping_add(1);
            }
            self.broadcast_user_info(player);
            // The pvp item reward and the announcement (`PvpRewardItem.ini`, `PvpAnnounce.ini`).
            self.custom_fake_player_killed_rewards(id, player, false);
        }
    }

    /// `Player.doDie` with a fake player as the killer: "FakePlayerUnflaggedKillsKarma" gives the
    /// fake player 150 karma when the dead player had no flag and no karma.
    pub(crate) fn fake_player_killed_player(&mut self, dead: ObjectId, killer: ObjectId) {
        if !self.settings.fake_players.enabled
            || !self.settings.fake_players.kill_karma
            || !self.is_fake_player(killer)
        {
            return;
        }
        if self.pvp_flag_of(dead) == 0 && self.karma_of_player(dead) <= 0 {
            *self.fake_players.karma.entry(killer).or_insert(0) += 150;
            self.broadcast_info(killer);
        }
    }

    /// `AttackableAI.thinkActive` for an aggressive fake player: it picks the nearest target
    /// (`FakePlayerAggro*`) or, with nothing to fight, walks to its loot and picks it up. `true`
    /// when `id` is one (the normal aggression scan is skipped, Java `return`s inside it).
    pub(crate) fn fake_player_aggro_scan(&mut self, id: ObjectId) -> bool {
        if !self.settings.fake_players.enabled || !self.is_fake_player(id) || !self.is_aggressive_npc(id) {
            return false;
        }
        let rounds = self
            .visible_objects(id)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            .count();
        for _ in 0..rounds {
            if self.fake_players.drops.get(&id).is_none_or(Vec::is_empty) {
                let range = self.aggro_range_of(id);
                let settings = self.settings.fake_players.clone();
                let mut candidates: Vec<(f64, ObjectId)> = Vec::new();
                for t in self.visible_objects_in_range(id, range) {
                    if t == id || self.is_dead(t) {
                        continue;
                    }
                    let wanted = (settings.aggro_fpc && self.is_fake_player(t))
                        || (settings.aggro_monsters && self.is_monster(t) && !self.is_fake_player(t))
                        || (settings.aggro_players && self.is_player(t));
                    if wanted && self.aggro_hating(id, t) == 0 {
                        candidates.push((self.dist_3d(id, t), t));
                    }
                }
                // The nearest; the lowest object id on a tie (D-13).
                candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                if let Some((_, nearest)) = candidates.first().copied() {
                    self.add_damage_hate(id, nearest, 0, 1);
                }
            } else if !self.is_in_combat(id) {
                // Must pickup items.
                self.fake_player_pick_up(id);
            }
        }
        true
    }

    /// The pickup step of `AttackableAI.thinkActive`: the last item dropped, taken when the fake
    /// player stands within 50 of it.
    fn fake_player_pick_up(&mut self, id: ObjectId) {
        let Some(item) = self.fake_players.drops.get(&id).and_then(|d| d.last()).copied() else {
            return;
        };
        if let Some(ground) = self.ground_item(item) {
            let item_id = ground.data.item_id;
            let (Some(here), Some(there)) = (self.position_of(id), self.position_of(item)) else {
                return;
            };
            if crate::target::distance_2d(here, there) > 50.0 {
                self.npc_set_intention_move_to(id, there.0, there.1, there.2);
            } else {
                if let Some(drops) = self.fake_players.drops.get_mut(&id) {
                    drops.pop();
                }
                self.broadcast_packet(
                    item,
                    &l2_protocol_game::server::GetItem {
                        player_id: id.0,
                        object_id: item.0,
                        x: there.0,
                        y: there.1,
                        z: there.2,
                    },
                );
                // `pickupMe` and `ItemsOnGroundManager.removeObject`: the item is gone.
                self.decay_ground_item(item);
                // "if (droppedItem.getTemplate().hasExImmediateEffect())": an herb's skills are cast
                // by the fake player.
                self.fake_player_cast_herb(id, item_id);
            }
        } else if let Some(drops) = self.fake_players.drops.get_mut(&id) {
            drops.pop();
        }
        self.set_running(id, true);
    }

    /// The herb part of the pickup: each skill of an item with `hasExImmediateEffect()` is cast
    /// by the fake player (`doSimultaneousCast`), then `broadcastInfo()` ("check if this is
    /// necessary" in Java).
    fn fake_player_cast_herb(&mut self, id: ObjectId, item_id: i32) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            return;
        };
        if !template.ex_immediate_effect {
            return;
        }
        for holder in template.skills.item_skills.iter().flatten() {
            self.do_simultaneous_cast(id, SkillKey::new(holder.id, holder.level));
        }
        self.broadcast_info(id);
    }

    /// `Attackable.doItemDrop` for a fake player as the main damage dealer
    /// (`FakePlayerCanDropItems`): the drops land on the ground and, with `FakePlayerCanPickup`,
    /// the fake player goes to pick them up. Returns `true` when `dealer` is a fake player.
    pub(crate) fn fake_player_drops_loot(&mut self, dying: ObjectId, dealer: ObjectId) -> bool {
        if !self.settings.fake_players.enabled || !self.is_fake_player(dealer) {
            return false;
        }
        if !self.settings.fake_players.can_drop_items {
            return true;
        }
        let Some(templates) = self.item_templates() else {
            return true;
        };
        let Some((npc_id, victim_level, raid)) = self
            .npcs
            .get(&dying)
            .map(|n| (n.npc_id, n.core.level, n.class.is_raid()))
        else {
            return true;
        };
        let level = self.npcs.get(&dealer).map_or(1, |n| n.core.level);
        let killer = crate::rewards::drop::DropKiller::plain(level);
        let victim = crate::rewards::drop::DropVictim {
            level: victim_level,
            raid,
            champion: false,
            seeded: self.is_seeded(dying),
        };
        let settings = self.settings.drops.clone();
        let drop_template_id = self.chest_drop_template_id(dying, npc_id);
        let death_items = self.with_rng(|world, rng| {
            world.npc_world.template(drop_template_id).map(|template| {
                crate::rewards::drop::calculate_drops(
                    &settings,
                    &templates,
                    template,
                    l2_data::npc::DropType::Drop,
                    &victim,
                    &killer,
                    rng,
                )
            })
        });
        let ground = self.settings.ground.clone();
        for drop in death_items.unwrap_or_default() {
            let Some(item) = templates.get(drop.id) else {
                continue;
            };
            let auto_loot = ground.auto_loot_item_ids.contains(&item.id)
                || (!item.ex_immediate_effect
                    && ((!raid && ground.auto_loot) || (raid && ground.auto_loot_raids)));
            if auto_loot {
                // Do nothing.
            } else if ground.auto_loot_herbs && item.ex_immediate_effect {
                // `doSimultaneousCast` of each skill of the herb, then `broadcastInfo()`.
                for holder in item.skills.item_skills.iter().flatten() {
                    self.do_simultaneous_cast(dealer, SkillKey::new(holder.id, holder.level));
                }
                self.broadcast_info(dealer);
            } else {
                let dropped = self.npc_drop_item(dying, Some(dealer), drop.id, drop.count);
                if self.settings.fake_players.can_pickup
                    && let Some(dropped) = dropped
                {
                    self.fake_players.drops.entry(dealer).or_default().push(dropped);
                }
            }
        }
        true
    }

    /// `PvpFlaggingStopTask`, `FLAG_CHECK`: whether the NPC's target is a playable or a fake
    /// player (`target.isPlayable() || target.isFakePlayer()`).
    pub fn fake_player_targets_playable(&self, npc: ObjectId) -> bool {
        self.creature_target(npc)
            .is_some_and(|t| self.is_playable(t) || self.is_fake_player(t))
    }

    /// `PvpFlaggingStopTask`: `npc.broadcastInfo()`, the flag status changed.
    pub fn fake_player_broadcast_info(&mut self, npc: ObjectId) {
        self.broadcast_info(npc);
    }

    /// `PvpConfig.PVP_NORMAL_TIME` (`PvPVsNormalTime`), ms.
    pub fn pvp_normal_time_ms(&self) -> i32 {
        self.settings.pvp.pvp_normal_time_ms
    }

    /// `Attackable.onSpawn` of a fake player: no drops, no karma, no pvp flag, running.
    pub(crate) fn fake_player_on_spawn(&mut self, id: ObjectId) {
        if !self.settings.fake_players.enabled || !self.is_fake_player(id) {
            return;
        }
        self.fake_players.karma.remove(&id);
        // "Clear existing fake player drops."
        self.fake_players.drops.remove(&id);
        self.set_npc_script_value(id, 0);
        self.set_running(id, true);
    }
}

/// Java `AdminFakePlayers` registrations.
pub(crate) fn admin_register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_fakechat", admin_fakechat)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `admin_fakechat <player> <fpcname> <message>`.
fn admin_fakechat(world: &mut World, gm: ObjectId, command: &str) -> Result<(), JavaException> {
    // `command.substring(15).split(" ")`.
    let words: Vec<&str> = command.get(15..).unwrap_or("").split(' ').collect();
    if words.len() < 3 {
        world.send_sys_message(gm, "Usage: //fakechat playername fpcname message");
        return Ok(());
    }
    let wanted = words[0].to_lowercase();
    let player = world
        .players
        .values()
        .find(|p| p.data.name.to_lowercase() == wanted)
        .map(|p| p.core.object_id);
    let Some(player) = player else {
        world.send_sys_message(gm, "Player not found.");
        return Ok(());
    };
    let Some(fpc_name) = world.fake_player_proper_name(words[1]).map(str::to_owned) else {
        world.send_sys_message(gm, "Fake player not found.");
        return Ok(());
    };
    let mut message = String::new();
    for word in &words[2..] {
        message += word;
        message.push(' ');
    }
    world.fake_player_send_chat(player, &fpc_name, &message);
    world.send_sys_message(gm, "Your message has been sent.");
    Ok(())
}
