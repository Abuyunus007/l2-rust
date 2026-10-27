//! The wedding system (`AllowWedding`): engagement, marriage, divorce and `.gotolove`
//! (M8-MISC-01).
// Java: managers/CoupleManager.java
// Java: entity/actor/holders/player/Couple.java
// Java: entity/actor/Player.java (`_married`, `_partnerId`, `_coupleId`, `_engagerequest`,
//       `_marryrequest`, `_marryaccepted`, `engageAnswer`)
// Java: network/clientpackets/{EnterWorld,DlgAnswer}.java (the wedding branches)
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/Wedding.java (`.engage`, `.divorce`,
//       `.gotolove`)
// Java: dist/game/data/scripts/custom/events/Wedding/Wedding.java (the priest, NPC 50007, and its
//       pages)
// Java: dist/game/data/scripts/handlers/chat/channels/ChatGeneral.java (the voiced command
//       dispatch)
//
// # How it fits in
// - [`World::load_couples`] is `CoupleManager.load` (only with `AllowWedding`, like Java's
//   `GameServer`). Every change of a couple is a [`WeddingWrite`] for the persistence task
//   (`mods_wedding`, D-15); the couple list itself lives in memory, as in Java.
// - The per player flags Java keeps in `Player` live in [`WeddingWorld::players`], so the shared
//   `Player` struct stays as it is. A player gets an entry when it enters the world and loses it
//   when it leaves.
// - The voiced commands are the `ChatGeneral` dispatch for text that starts with `.`
//   ([`World::wedding_voiced_command`]); the general voiced command registry is a script
//   concern (D-05), only the three wedding commands exist.
// - The priest is built in like `ClanMaster` and `ManorManager`: `Script Wedding <event>` goes to
//   [`World::wedding_script_event`], the first click to [`World::wedding_first_talk`].
// - `.gotolove` waits for its timer on the wheel (the Java `EscapeFinalizer`), and takes the
//   cast slot like `forceIsCasting` does, so a hit or a movement order aborts it.
//
// # Left out, with the system that brings it (every one is a check or a state of another M8 card)
// - Checks `.gotolove` makes on states that do not exist yet: a boss zone, a combat flag,
//   the Olympiad, a registered event, the observer mode, the Seven Signs festival,
//   a dimensional rift.
// - The combat/zone checks for `.engage` need nothing else.

use l2_data::ClassId;
use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ConfirmDlg, MagicSkillUse, NpcHtmlMessage, SetupGauge, SkillPosition, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use crate::dialog::INTERACTION_DISTANCE;
use crate::effects::SkillKey;
use crate::punishments::format_duration;
use crate::target::distance_2d;
use crate::teleport::TeleportTarget;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;
use l2_protocol_game::server::charscreen::paperdoll;

/// `Wedding.MANAGER_ID`: the wedding priest.
pub const WEDDING_MANAGER_ID: i32 = 50007;
/// `Wedding.FORMAL_WEAR`: the item the chest slot must hold.
const FORMAL_WEAR: i32 = 6408;
/// The script folder below `data/scripts` (`Quest.getPath()`).
const WEDDING_PATH: &str = "custom/events/Wedding";
/// The wedding march, cast on both partners (`MagicSkillUse(player, player, 2230, 1, 1, 0)`).
const WEDDING_MARCH: i32 = 2230;
/// `CommonSkill.LARGE_FIREWORK`.
const LARGE_FIREWORK: SkillKey = SkillKey { id: 2025, level: 1 };
/// The Scroll of Escape animation of `.gotolove`.
const ESCAPE_SKILL: i32 = 1050;
/// `dlg.addTime(15 * 1000)`.
const ENGAGE_DIALOG_MS: i32 = 15 * 1000;

/// The values of `config/Custom/Wedding.ini` (Java `WeddingConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeddingSettings {
    /// `AllowWedding`.
    pub allow_wedding: bool,
    /// `WeddingPrice`.
    pub wedding_price: i32,
    /// `WeddingPunishInfidelity`.
    pub punish_infidelity: bool,
    /// `WeddingTeleport`.
    pub teleport: bool,
    /// `WeddingTeleportPrice`.
    pub teleport_price: i32,
    /// `WeddingTeleportDuration`, in seconds.
    pub teleport_duration: i32,
    /// `WeddingAllowSameSex`.
    pub same_sex: bool,
    /// `WeddingFormalWear`.
    pub formal_wear: bool,
    /// `WeddingDivorceCosts`, percent.
    pub divorce_costs: i32,
}

impl Default for WeddingSettings {
    /// The Java defaults.
    fn default() -> Self {
        Self {
            allow_wedding: false,
            wedding_price: 250_000_000,
            punish_infidelity: true,
            teleport: true,
            teleport_price: 50_000,
            teleport_duration: 60,
            same_sex: false,
            formal_wear: true,
            divorce_costs: 20,
        }
    }
}

/// Java `Couple`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Couple {
    pub id: i32,
    pub player1_id: i32,
    pub player2_id: i32,
    /// `getMaried()`.
    pub married: bool,
    /// `getAffiancedDate()`, Unix milliseconds.
    pub affiance_date: i64,
    /// `getWeddingDate()`, Unix milliseconds.
    pub wedding_date: i64,
}

/// A change of `mods_wedding` for the persistence task, with Java's statement
/// (`Couple` constructor, `marry`, `divorce`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeddingWrite {
    /// `INSERT INTO mods_wedding (id, player1Id, player2Id, married, affianceDate, weddingDate)`.
    Insert(Couple),
    /// `UPDATE mods_wedding set married = ?, weddingDate = ? where id = ?`.
    Marry { id: i32, wedding_date: i64 },
    /// `DELETE FROM mods_wedding WHERE id=?`.
    Divorce { id: i32 },
}

/// What `Player` keeps for the wedding system.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerWedding {
    /// `_partnerId`.
    pub partner_id: i32,
    /// `_coupleId`.
    pub couple_id: i32,
    /// `_married`.
    pub married: bool,
    /// `_marryrequest`.
    pub marry_request: bool,
    /// `_marryaccepted`.
    pub marry_accepted: bool,
    /// `_engagerequest`.
    pub engage_request: bool,
    /// `_engageid`.
    pub engage_id: i32,
    /// `PlayerAction.USER_ENGAGE` is in the player's action set (`addAction` / `removeAction`).
    pub engage_action: bool,
}

/// The state of the wedding system (one field of the [`World`]).
#[derive(Debug, Default)]
pub struct WeddingWorld {
    pub(crate) settings: WeddingSettings,
    /// `CoupleManager._couples`.
    pub(crate) couples: Vec<Couple>,
    pub(crate) players: std::collections::HashMap<ObjectId, PlayerWedding>,
}

/// The timers of the wedding system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeddingTimer {
    /// Java `Wedding.EscapeFinalizer.run()`: the `.gotolove` wait is over.
    Escape {
        player: ObjectId,
        x: i32,
        y: i32,
        z: i32,
        /// `partner.isIn7sDungeon()` when the wait began (M8-SS-01).
        to_7s_dungeon: bool,
    },
}

impl World {
    // ------------------------------------------------------------------ CoupleManager

    /// Installs the values of `Wedding.ini`.
    pub fn set_wedding_settings(&mut self, settings: WeddingSettings) {
        self.wedding.settings = settings;
    }

    /// Java `CoupleManager.load()`: the couples of `mods_wedding`, by id.
    pub fn load_couples(&mut self, couples: Vec<Couple>) {
        info!("CoupleManager: Loaded {} couples(s)", couples.len());
        self.wedding.couples = couples;
    }

    /// Java `CoupleManager.getCouples()`.
    pub fn couples(&self) -> &[Couple] {
        &self.wedding.couples
    }

    /// Java `CoupleManager.getCouple(coupleId)`.
    pub fn couple(&self, couple_id: i32) -> Option<&Couple> {
        self.wedding.couples.iter().find(|c| c.id == couple_id)
    }

    /// What the wedding system knows about an online player (`None` before the player entered).
    pub fn player_wedding(&self, player: ObjectId) -> Option<&PlayerWedding> {
        self.wedding.players.get(&player)
    }

    fn wedding_of(&self, player: ObjectId) -> PlayerWedding {
        self.wedding.players.get(&player).cloned().unwrap_or_default()
    }

    fn wedding_mut(&mut self, player: ObjectId) -> &mut PlayerWedding {
        self.wedding.players.entry(player).or_default()
    }

    fn wedding_name(&self, player: ObjectId) -> String {
        self.players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `CoupleManager.createCouple(player1, player2)`.
    pub(crate) fn create_couple(&mut self, player1: ObjectId, player2: ObjectId) {
        if !self.players.contains_key(&player1)
            || !self.players.contains_key(&player2)
            || self.wedding_of(player1).partner_id != 0
            || self.wedding_of(player2).partner_id != 0
        {
            return;
        }
        // `IdManager.getNextId()` (D-39: the Rust range).
        let id = match self.npc_world.ids.as_ref().map(|ids| ids.allocate()) {
            Some(Ok(id)) => id.0,
            Some(Err(e)) => {
                warn!("Could not create couple: no object id: {e}");
                return;
            }
            None => {
                warn!("Could not create couple: no id factory installed.");
                return;
            }
        };
        let now = crate::world::unix_ms_now();
        let couple = Couple {
            id,
            player1_id: player1.0,
            player2_id: player2.0,
            married: false,
            affiance_date: now,
            wedding_date: now,
        };
        self.outbox
            .persist(crate::outbox::PersistCommand::Wedding(WeddingWrite::Insert(
                couple.clone(),
            )));
        self.wedding.couples.push(couple);
        let first = self.wedding_mut(player1);
        first.partner_id = player2.0;
        first.couple_id = id;
        let second = self.wedding_mut(player2);
        second.partner_id = player1.0;
        second.couple_id = id;
    }

    /// Java `CoupleManager.deleteCouple(coupleId)`.
    pub(crate) fn delete_couple(&mut self, couple_id: i32) {
        let Some(index) = self.wedding.couples.iter().position(|c| c.id == couple_id) else {
            // NOTE(java-quirk): Java reads `_couples.get(-1)` and throws.
            warn!("CoupleManager: no couple {couple_id} to delete.");
            return;
        };
        let couple = self.wedding.couples.remove(index);
        for id in [couple.player1_id, couple.player2_id] {
            let id = ObjectId(id);
            if self.players.contains_key(&id) {
                let state = self.wedding_mut(id);
                state.partner_id = 0;
                state.married = false;
                state.couple_id = 0;
            }
        }
        self.outbox
            .persist(crate::outbox::PersistCommand::Wedding(WeddingWrite::Divorce {
                id: couple.id,
            }));
    }

    /// Java `Couple.marry()`.
    fn marry_couple(&mut self, couple_id: i32) {
        let now = crate::world::unix_ms_now();
        let Some(couple) = self.wedding.couples.iter_mut().find(|c| c.id == couple_id) else {
            return;
        };
        couple.married = true;
        couple.wedding_date = now;
        self.outbox
            .persist(crate::outbox::PersistCommand::Wedding(WeddingWrite::Marry {
                id: couple_id,
                wedding_date: now,
            }));
    }

    // ------------------------------------------------------------------ EnterWorld, leaving

    /// Java `EnterWorld`'s "Wedding checks" (`AllowWedding`): the player finds its couple, and the
    /// partner learns it logged in.
    pub(crate) fn wedding_on_enter(&mut self, player: ObjectId) {
        if !self.wedding.settings.allow_wedding {
            return;
        }
        let me = player.0;
        let mut state = self.wedding_of(player);
        for couple in &self.wedding.couples {
            if couple.player1_id == me || couple.player2_id == me {
                if couple.married {
                    state.married = true;
                }
                state.couple_id = couple.id;
                state.partner_id = if couple.player1_id == me {
                    couple.player2_id
                } else {
                    couple.player1_id
                };
            }
        }
        let partner = ObjectId(state.partner_id);
        self.wedding.players.insert(player, state.clone());
        if state.partner_id != 0 && self.players.contains_key(&partner) {
            self.send_message(partner, "Your partner has logged in.");
        }
    }

    /// The player left the world: its flags go (they live on the `Player` object in Java).
    pub(crate) fn wedding_on_player_leave(&mut self, player: ObjectId) {
        self.wedding.players.remove(&player);
    }

    // ------------------------------------------------------------------ voiced commands

    /// Java `ChatGeneral.onChat`'s voiced command branch. `true` when a handler took the text
    /// (`vcdUsed`), which is then not said.
    pub(crate) fn wedding_voiced_command(&mut self, player: ObjectId, text: &str) -> bool {
        if !text.starts_with('.') || !self.wedding.settings.allow_wedding {
            return false;
        }
        // `new StringTokenizer(text)`: split at whitespace.
        let tokens: Vec<&str> = text
            .split([' ', '\t', '\n', '\r', '\u{c}'])
            .filter(|t| !t.is_empty())
            .collect();
        let command = if tokens.len() > 1 {
            tokens[0].get(1..).unwrap_or("")
        } else {
            text.get(1..).unwrap_or("")
        };
        // `VoicedCommandHandler.getHandler`: the text up to the first space.
        let word = command.split(' ').next().unwrap_or(command);
        match word {
            "engage" => {
                self.engage(player);
            }
            "divorce" => {
                self.divorce(player);
            }
            "gotolove" => {
                self.go_to_love(player);
            }
            _ => return false,
        }
        true
    }

    /// Java `Wedding.divorce(activeChar)`.
    fn divorce(&mut self, player: ObjectId) -> bool {
        let state = self.wedding_of(player);
        if state.partner_id == 0 {
            return false;
        }
        let partner_id = ObjectId(state.partner_id);
        let couple_id = state.couple_id;
        let mut adena_amount: i64 = 0;
        if state.married {
            self.send_message(player, "You are now divorced.");
            adena_amount = (self.adena_of(player) / 100) * i64::from(self.wedding.settings.divorce_costs);
            // `getInventory().reduceAdena(FEE, ..)` sends no inventory update in Java.
            self.reduce_adena(player, adena_amount, false);
        } else {
            self.send_message(player, "You have broken up as a couple.");
        }

        if self.players.contains_key(&partner_id) {
            self.wedding_mut(partner_id).partner_id = 0;
            if self.wedding_of(partner_id).married {
                self.send_message(partner_id, "Your spouse has decided to divorce you.");
            } else {
                self.send_message(
                    partner_id,
                    "Your fiance has decided to break the engagement with you.",
                );
            }
            // Give adena.
            if adena_amount > 0 {
                self.add_adena(partner_id, adena_amount, false);
            }
        }

        self.delete_couple(couple_id);
        true
    }

    /// Java `Wedding.engage(activeChar)`.
    fn engage(&mut self, player: ObjectId) -> bool {
        let Some(target) = self.target_of(player) else {
            self.send_message(player, "You have no one targeted.");
            return false;
        };
        if !self.players.contains_key(&target) {
            self.send_message(player, "You can only ask another player to engage you.");
            return false;
        }
        if self.wedding_of(player).partner_id != 0 {
            self.send_message(player, "You are already engaged.");
            if self.wedding.settings.punish_infidelity {
                self.punish_infidelity(player);
            }
            return false;
        }

        // Check if player target himself.
        if target == player {
            self.send_message(
                player,
                "Is there something wrong with you, are you trying to go out with youself?",
            );
            return false;
        }
        let target_state = self.wedding_of(target);
        if target_state.married {
            self.send_message(player, "Player already married.");
            return false;
        }
        if target_state.engage_request {
            self.send_message(player, "Player already asked by someone else.");
            return false;
        }
        if target_state.partner_id != 0 {
            self.send_message(player, "Player already engaged with someone else.");
            return false;
        }
        let female = |world: &Self, id: ObjectId| world.players.get(&id).is_some_and(|p| p.data.sex != 0);
        if female(self, target) == female(self, player) && !self.wedding.settings.same_sex {
            self.send_message(player, "Gay marriage is not allowed on this server!");
            return false;
        }

        // Check if target has player on friendlist. (Java reads `character_friends` of the target;
        // an online character's list is the same data, kept up to date by `friends.rs`.)
        if !self.is_friend_of(target, player) {
            self.send_message(
                player,
                "The player you want to ask is not on your friends list, you must first be on each others friends list before you choose to engage.",
            );
            return false;
        }

        let state = self.wedding_mut(target);
        state.engage_request = true;
        state.engage_id = player.0;
        state.engage_action = true;

        let name = self.wedding_name(player);
        let dialog = ConfirmDlg::new(SystemMessageId::S13)
            .with(SystemMessageParam::Text(format!(
                "{name} is asking to engage you. Do you want to start a new relationship?"
            )))
            .with_time(ENGAGE_DIALOG_MS);
        self.send(target, &dialog);
        true
    }

    /// The infidelity punishment of `.engage`: a big head and a Seven Signs debuff.
    fn punish_infidelity(&mut self, player: ObjectId) {
        // Give player a Big Head.
        self.start_abnormal_visual_effect(player, &[AbnormalVisualEffect::BigHead]);

        // Lets recycle the sevensigns debuffs.
        let (level, class_id) = self
            .players
            .get(&player)
            .map_or((1, 0), |p| (p.core.level, p.data.class_id));
        let skill_level = if level > 40 { 2 } else { 1 };
        let mage = u16::try_from(class_id).is_ok_and(|id| ClassId(id).is_mage());
        let skill_id = if mage { 4362 } else { 4361 };
        if !self.is_affected_by_skill(player, skill_id) {
            self.apply_effects(player, player, SkillKey::new(skill_id, skill_level));
        }
    }

    /// Java `DlgAnswer`'s `USER_ENGAGE` branch (the `S1_3` question): `true` when the answer was
    /// for the engagement, whatever the answer.
    pub(crate) fn wedding_dlg_answer(&mut self, player: ObjectId, answer: i32) -> bool {
        // `player.removeAction(PlayerAction.USER_ENGAGE)`.
        let Some(state) = self.wedding.players.get_mut(&player) else {
            return false;
        };
        if !std::mem::take(&mut state.engage_action) {
            return false;
        }
        if self.wedding.settings.allow_wedding {
            self.engage_answer(player, answer);
        }
        true
    }

    /// Java `Player.engageAnswer(answer)`.
    fn engage_answer(&mut self, player: ObjectId, answer: i32) {
        let state = self.wedding_of(player);
        if !state.engage_request || state.engage_id == 0 {
            return;
        }
        let asker = ObjectId(state.engage_id);
        let me = self.wedding_mut(player);
        me.engage_request = false;
        me.engage_id = 0;
        if self.players.contains_key(&asker) {
            if answer == 1 {
                self.create_couple(asker, player);
                self.send_message(asker, "Request to Engage has been >ACCEPTED<");
            } else {
                self.send_message(asker, "Request to Engage has been >DENIED<!");
            }
        }
    }

    // ------------------------------------------------------------------ .gotolove

    /// `player.getChestArmorInstance().getId() == FORMAL_WEAR` (or no rule).
    fn is_wearing_formal_wear(&self, player: ObjectId) -> bool {
        if !self.wedding.settings.formal_wear {
            return true;
        }
        self.players.get(&player).is_some_and(|p| {
            crate::enter::paperdoll_items(&p.data)[paperdoll::CHEST].is_some_and(|i| i.item_id == FORMAL_WEAR)
        })
    }

    fn instance_of_player(&self, player: ObjectId) -> i32 {
        self.objects.get(&player).map_or(0, |e| e.instance_id)
    }

    /// Java `Wedding.goToLove(activeChar)`.
    fn go_to_love(&mut self, player: ObjectId) -> bool {
        let settings = self.wedding.settings.clone();
        let teleport_timer = settings.teleport_duration.wrapping_mul(1000);

        if !settings.teleport {
            self.send_message(player, "The 'Go to Love' teleport feature has been disabled.");
            return false;
        }
        let state = self.wedding_of(player);
        if !state.married {
            self.send_message(player, "You're not married.");
            return false;
        }
        if state.partner_id == 0 {
            self.send_message(
                player,
                "Couldn't find your fiance in the Database - Inform a Gamemaster.",
            );
            warn!(
                "Married but couldn't find parter for {}",
                self.wedding_name(player)
            );
            return false;
        }
        // (Boss zone and combat flag: not ported yet.)
        if self.is_cursed_weapon_equipped(player) {
            self.send_message(
                player,
                "While you are holding a Cursed Weapon you can't go to your love!",
            );
            return false;
        }
        if self.is_jailed(player) {
            self.send_message(player, "You are in Jail!");
            return false;
        }
        // Olympiad (M8-OLY-01; the registered events are scripts).
        if self.is_in_olympiad_mode(player) {
            self.send_message(player, "You are in the Olympiad now.");
            return false;
        }
        if self.is_in_duel(player) {
            self.send_message(player, "You are in a duel!");
            return false;
        }
        // Observer mode (M8-OLY-01).
        if self.in_observer_mode(player) {
            self.send_message(player, "You are in the observation.");
            return false;
        }
        if self.siege_of(player).is_some() {
            self.send_message(player, "You are in a siege, you cannot go to your partner.");
            return false;
        }
        if self.is_festival_participant(player) {
            self.send_message(player, "You are in a festival.");
            return false;
        }
        if self.is_in_dimensional_rift(player) {
            self.send_message(player, "You are in the dimensional rift.");
            // NOTE(java-quirk): the second line is the duel message of the partner.
            self.send_message(player, "Your partner is in a duel.");
            return false;
        }
        if self.is_inside_zone(player, ZoneId::NoSummonFriend) {
            self.send_message(player, "You are in area which blocks summoning.");
            return false;
        }

        let partner = ObjectId(state.partner_id);
        if !self.players.contains_key(&partner) {
            self.send_message(player, "Your partner is not online.");
            return false;
        }
        if self.instance_of_player(player) != self.instance_of_player(partner) {
            self.send_message(player, "Your partner is in another World!");
            return false;
        }
        if self.is_jailed(partner) {
            self.send_message(player, "Your partner is in Jail.");
            return false;
        }
        if self.is_cursed_weapon_equipped(partner) {
            self.send_message(
                player,
                "Your partner is holding a Cursed Weapon and you can't go to your love!",
            );
            return false;
        }
        // (Partner: boss zone, event: not ported yet.) Olympiad: M8-OLY-01.
        if self.is_in_olympiad_mode(partner) {
            self.send_message(player, "Your partner is in the Olympiad now.");
            return false;
        }
        if self.is_in_duel(partner) {
            self.send_message(player, "Your partner is in a duel.");
            return false;
        }
        if self.is_festival_participant(partner) {
            self.send_message(player, "Your partner is in a festival.");
            return false;
        }
        if self.is_in_dimensional_rift(partner) {
            self.send_message(player, "Your partner is in dimensional rift.");
            return false;
        }
        // Observer mode (M8-OLY-01).
        if self.in_observer_mode(partner) {
            self.send_message(player, "Your partner is in the observation.");
            return false;
        }
        if self.siege_of(partner).is_some() {
            self.send_message(
                player,
                "Your partner is in a siege, you cannot go to your partner.",
            );
            return false;
        }
        // The partner is in a Seven Signs dungeon (M8-SS-01).
        if self.is_in_7s_dungeon(partner) && !self.is_in_7s_dungeon(player) {
            let player_cabal = self.ss_player_cabal(player.0);
            if self.ss_is_seal_validation_period() {
                if player_cabal != self.ss_cabal_highest_score() {
                    self.send_message(
                        player,
                        "Your Partner is in a Seven Signs Dungeon and you are not in the winner Cabal!",
                    );
                    return false;
                }
            } else if player_cabal == crate::sevensigns::CABAL_NULL {
                self.send_message(
                    player,
                    "Your Partner is in a Seven Signs Dungeon and you are not registered!",
                );
                return false;
            }
        }
        if self.is_inside_zone(partner, ZoneId::NoSummonFriend) {
            self.send_message(player, "Your partner is in area which blocks summoning.");
            return false;
        }

        if !self.reduce_adena(player, i64::from(settings.teleport_price), true) {
            return false; // Player already informed by system message inside reduceAdena.
        }

        self.send_message(
            player,
            &format!(
                "You will be teleported to your partner in {}.",
                format_duration(i64::from(teleport_timer))
            ),
        );

        self.set_intention_idle(player);

        // SoE Animation section.
        self.set_target(player, Some(player));
        self.disable_all_skills(player);
        if let Some(packet) = self.wedding_skill_use(player, ESCAPE_SKILL, 1, teleport_timer) {
            // `broadcastSkillPacket(packet, player)` is `broadcastPacket(packet)`.
            self.broadcast_creature_packet(player, &packet);
        }
        self.send(
            player,
            &SetupGauge::new(player.0, SetupGauge::BLUE, teleport_timer),
        );
        // End SoE Animation section.

        let (x, y, z) = self.position_of(partner).unwrap_or((0, 0, 0));
        let to_7s_dungeon = self.is_in_7s_dungeon(partner);
        // Continue execution later.
        let delay = u64::try_from(teleport_timer).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let timer = self.timers.schedule_at(
            due,
            TimerKind::Wedding(WeddingTimer::Escape {
                player,
                x,
                y,
                z,
                to_7s_dungeon,
            }),
        );
        // `setSkillCast(task)` and `forceIsCasting(ticks + teleportTimer / MILLIS_IN_TICK)`.
        let interrupt = self.game_ticks().wrapping_add(teleport_timer / 100);
        self.set_casting_now(player, true);
        let cast = self.cast_state_mut(player);
        cast.skill_cast = Some(timer);
        cast.cast_interrupt_time = interrupt;
        true
    }

    /// `new MagicSkillUse(creature, skillId, skillLevel, hitTime, 0)` at the creature itself.
    fn wedding_skill_use(
        &self,
        id: ObjectId,
        skill_id: i32,
        level: i32,
        hit_time: i32,
    ) -> Option<MagicSkillUse> {
        let (x, y, z) = self.position_of(id)?;
        let at = SkillPosition { x, y, z };
        Some(MagicSkillUse {
            creature_id: id.0,
            target_id: id.0,
            skill_id,
            skill_level: level,
            hit_time,
            reuse_delay: 0,
            creature: at,
            ground_location: None,
            target: at,
        })
    }

    /// The timers of the wedding system.
    pub(crate) fn on_wedding_timer(&mut self, timer: WeddingTimer) {
        match timer {
            WeddingTimer::Escape {
                player,
                x,
                y,
                z,
                to_7s_dungeon,
            } => self.escape_finalizer(player, x, y, z, to_7s_dungeon),
        }
    }

    /// Java `Wedding.EscapeFinalizer.run()`.
    fn escape_finalizer(&mut self, player: ObjectId, x: i32, y: i32, z: i32, to_7s_dungeon: bool) {
        if !self.players.contains_key(&player) {
            return;
        }
        // The task is over; `setSkillCast` keeps the finished future in Java.
        if let Some(state) = self.cast.creatures.get_mut(&player) {
            state.skill_cast = None;
        }
        if self.is_dead(player) {
            return;
        }
        if self.siege_at(x, y, z).is_some() {
            self.send_message(player, "Your partner is in siege, you can't go to your partner.");
            return;
        }
        self.set_in_7s_dungeon(player, to_7s_dungeon);
        self.enable_all_skills(player);
        self.set_casting_now(player, false);
        self.tele_to_location(player, TeleportTarget::at(x, y, z));
    }

    // ------------------------------------------------------------------ the priest

    /// `Quest.getHtm(player, fileName)` of the `Wedding` script.
    fn wedding_html(&self, player: ObjectId, file: &str) -> String {
        self.get_htm(player, &format!("data/scripts/{WEDDING_PATH}/{file}"))
            .unwrap_or_default()
    }

    /// `Wedding.sendHtml(player, fileName, regex, replacement)`: the page goes to `player` and is
    /// returned.
    fn wedding_send_html(&mut self, player: ObjectId, file: &str, replace: Option<(&str, &str)>) -> String {
        let mut html = self.wedding_html(player, file);
        if let Some((from, to)) = replace {
            html = html.replace(from, to);
        }
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
        html
    }

    /// Whether the NPC is the wedding priest.
    pub(crate) fn is_wedding_manager(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .is_some_and(|n| n.npc_id == WEDDING_MANAGER_ID)
    }

    /// Java `Wedding.onFirstTalk`, then `Quest.showResult`. `true` when the NPC is the priest.
    pub(crate) fn wedding_first_talk(&mut self, npc: ObjectId, player: ObjectId, start: bool) -> bool {
        if !self.is_wedding_manager(npc) {
            return false;
        }
        if start {
            // `addStartNpc`: `ON_NPC_QUEST_START` makes the NPC the quest NPC of the player.
            if let Some(p) = self.players.get_mut(&player) {
                p.talk.last_quest_npc_object = npc.0;
            }
        }
        let html = self
            .wedding_html(player, "Start.html")
            .replace("%fee%", &self.wedding.settings.wedding_price.to_string());
        self.wedding_show_result(player, npc, &html);
        true
    }

    /// Java `Player.processScriptEvent("Wedding", event)`. `true` when the script name is ours.
    pub(crate) fn wedding_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        if script_name != "Wedding" {
            return false;
        }
        if event.is_empty() {
            return true;
        }
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return true;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let npc = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => target,
            _ if quest_npc > 0 => match self
                .find_object(ObjectId(quest_npc))
                .filter(|o| self.npcs.contains_key(o))
            {
                Some(npc) if in_range(self, npc) => npc,
                _ => return true,
            },
            _ => return true,
        };
        if let Some(result) = self.wedding_on_event(player, event) {
            self.wedding_show_result(player, npc, &result);
        }
        true
    }

    /// Java `Quest.showResult(player, res, npc)` of the `Wedding` script.
    fn wedding_show_result(&mut self, player: ObjectId, npc: ObjectId, res: &str) {
        if res.is_empty() {
            return;
        }
        if res.ends_with(".htm") || res.ends_with(".html") {
            let content = self.wedding_html(player, res);
            if content.is_empty() {
                debug!("{player}: page {res} of Wedding not found.");
            } else {
                self.wedding_send_npc_html(player, npc, &content);
            }
        } else if res.starts_with("<html") {
            self.wedding_send_npc_html(player, npc, res);
        } else {
            self.send_message(player, res);
        }
    }

    /// `showHtmlFile`'s send: `%objectId%` and `%playername%` replaced, then `ActionFailed`.
    fn wedding_send_npc_html(&mut self, player: ObjectId, npc: ObjectId, content: &str) {
        let name = self.wedding_name(player);
        let html = content
            .replace("%objectId%", &npc.0.to_string())
            .replace("%playername%", &name);
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }

    /// Java `Wedding.onEvent(event, npc, player)`.
    fn wedding_on_event(&mut self, player: ObjectId, event: &str) -> Option<String> {
        let state = self.wedding_of(player);
        if state.partner_id == 0 {
            return Some("NoPartner.html".to_owned());
        }
        let partner = ObjectId(state.partner_id);
        if !self.players.contains_key(&partner) {
            return Some("NotFound.html".to_owned());
        }
        if state.married {
            return Some("Already.html".to_owned());
        }
        if state.marry_accepted {
            return Some("WaitForPartner.html".to_owned());
        }

        // NOTE(java-quirk): whatever the click was (accept or decline), the first one of the
        // player who was asked only clears the requests and shows the question again.
        if state.marry_request {
            if !self.is_wearing_formal_wear(player) || !self.is_wearing_formal_wear(partner) {
                return Some(self.wedding_send_html(partner, "NoFormal.html", None));
            }
            self.wedding_mut(player).marry_request = false;
            self.wedding_mut(partner).marry_request = false;
            let name = self.wedding_name(partner);
            return Some(self.wedding_html(player, "Ask.html").replace("%player%", &name));
        }

        match event {
            "ask" => {
                if !self.is_wearing_formal_wear(player) || !self.is_wearing_formal_wear(partner) {
                    Some(self.wedding_send_html(partner, "NoFormal.html", None))
                } else {
                    self.wedding_mut(player).marry_accepted = true;
                    self.wedding_mut(partner).marry_request = true;

                    let me = self.wedding_name(player);
                    self.wedding_send_html(partner, "Ask.html", Some(("%player%", &me)));
                    let other = self.wedding_name(partner);
                    Some(
                        self.wedding_html(player, "Requested.html")
                            .replace("%player%", &other),
                    )
                }
            }
            "accept" => {
                let price = i64::from(self.wedding.settings.wedding_price);
                if !self.is_wearing_formal_wear(player) || !self.is_wearing_formal_wear(partner) {
                    Some(self.wedding_send_html(partner, "NoFormal.html", None))
                } else if self.adena_of(player) < price || self.adena_of(partner) < price {
                    let fee = price.to_string();
                    Some(self.wedding_send_html(partner, "Adena.html", Some(("%fee%", &fee))))
                } else {
                    self.reduce_adena(player, price, true);
                    self.reduce_adena(partner, price, true);

                    // Accept the wedding request.
                    self.wedding_mut(player).marry_accepted = true;
                    self.marry_couple(state.couple_id);

                    // Messages to the couple.
                    self.send_message(player, "Congratulations you are married!");
                    let me = self.wedding_mut(player);
                    me.married = true;
                    me.marry_request = false;
                    self.send_message(partner, "Congratulations you are married!");
                    let other = self.wedding_mut(partner);
                    other.married = true;
                    other.marry_request = false;

                    // Wedding march.
                    for id in [player, partner] {
                        if let Some(packet) = self.wedding_skill_use(id, WEDDING_MARCH, 1, 1) {
                            self.broadcast_creature_packet(id, &packet);
                        }
                    }

                    // Fireworks.
                    self.do_cast(player, LARGE_FIREWORK);
                    self.do_cast(partner, LARGE_FIREWORK);

                    let text = format!(
                        "Congratulations to {} and {}! They have been married.",
                        self.wedding_name(player),
                        self.wedding_name(partner)
                    );
                    self.broadcast_announcement(&text, false);
                    Some(self.wedding_send_html(partner, "Accepted.html", None))
                }
            }
            "decline" => {
                for id in [player, partner] {
                    let s = self.wedding_mut(id);
                    s.marry_request = false;
                    s.marry_accepted = false;
                }
                self.send_message(player, "You declined your partner's marriage request.");
                self.send_message(partner, "Your partner declined your marriage request.");
                Some(self.wedding_send_html(partner, "Declined.html", None))
            }
            _ => None,
        }
    }
}
