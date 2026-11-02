//! The premium system (`config/Custom/PremiumSystem.ini`, `EnablePremiumSystem`, off by default):
//! the status of a player, the `account_premium` rows, the rates of exp, sp, drops and spoil, the
//! voiced command `.premium` and the admin commands `//premium_*`.
// Java: managers/PremiumManager.java
// Java: config/custom/PremiumSystemConfig.java
// Java: entity/actor/Player.java (setPremiumStatus, hasPremiumStatus)
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/Premium.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPremium.java
//
// # Model
// `World::premium` keeps the cache of `PremiumManager._premiumData` (key to the end date in Unix
// milliseconds), the set of players with `_premiumStatus` and the expire timer of each. The key
// is the account name (`AccountWidePremium = True`) or the character name, lower case.
//
// Java reads the row at `OnPlayerLogin` and blocks. The world never waits for the database
// (D-15), so `EnterWorld` sends `PersistCommand::PremiumRequest` and the answer
// (`WorldCommand::PremiumLoaded`) does the rest of the listener; the player is not premium for
// the few milliseconds between (the PC cafe timer is started again by the answer).
//
// # Side by side (D-39, D-53)
// - The row of an account is read at every login (D-50), so a purchase on the Java server shows.
// - `addPremiumTime` (an admin command or the community board of this server) writes its
//   `REPLACE` as Java does: the action is explicit and for one account.
// - The `DELETE` of an expired row (`removePremiumStatus(account, false)` at login) is a global
//   clean-up both servers do, so it is only written with `CycleDatabaseWrites`.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::warn;

use crate::admin::commands::java_split_space;
use crate::admin::{AdminHandler, AdminHandlers, JavaException};
use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

use super::CustomPlayTimer;

/// Java `PremiumSystemConfig` without the PC cafe keys (those are `PcCafeSettings`).
#[derive(Debug, Clone, PartialEq)]
pub struct PremiumSettings {
    /// `EnablePremiumSystem`.
    pub enabled: bool,
    /// `AccountWidePremium`.
    pub account_wide: bool,
    /// `PcCafeRetailLike` (`//premium_add*` starts the PC cafe timer of the account's player).
    pub pc_cafe_retail_like: bool,
    /// `CycleDatabaseWrites` of `SideBySide.ini` (D-53).
    pub cycle_database_writes: bool,
    /// `Rates.ini` `RateXp` and `RateSp`, `DeathDropChanceMultiplier`, ... for the `.premium` page.
    pub shown_rates: ShownRates,
    /// The multipliers.
    pub rates: PremiumRates,
}

/// The server rates the `.premium` page prints next to the premium ones (`RatesConfig`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShownRates {
    pub rate_xp: f32,
    pub rate_sp: f32,
    pub death_drop_chance: f32,
    pub death_drop_amount: f32,
    pub spoil_drop_chance: f32,
    pub spoil_drop_amount: f32,
}

impl Default for ShownRates {
    fn default() -> Self {
        Self {
            rate_xp: 1.0,
            rate_sp: 1.0,
            death_drop_chance: 1.0,
            death_drop_amount: 1.0,
            spoil_drop_chance: 1.0,
            spoil_drop_amount: 1.0,
        }
    }
}

/// The multipliers of a premium player (`PREMIUM_RATE_*`), Java `float`s.
#[derive(Debug, Clone, PartialEq)]
pub struct PremiumRates {
    pub xp: f32,
    pub sp: f32,
    pub drop_chance: f32,
    pub drop_amount: f32,
    pub spoil_chance: f32,
    pub spoil_amount: f32,
    pub quest_xp: f32,
    pub quest_sp: f32,
    /// `PremiumRateDropChanceByItemId`.
    pub drop_chance_by_id: HashMap<i32, f32>,
    /// `PremiumRateDropAmountByItemId`.
    pub drop_amount_by_id: HashMap<i32, f32>,
}

impl Default for PremiumRates {
    /// The defaults of `PremiumSystemConfig.load()` (not of the shipped file).
    fn default() -> Self {
        Self {
            xp: 2.0,
            sp: 2.0,
            drop_chance: 2.0,
            drop_amount: 1.0,
            spoil_chance: 2.0,
            spoil_amount: 1.0,
            quest_xp: 1.0,
            quest_sp: 1.0,
            drop_chance_by_id: HashMap::new(),
            drop_amount_by_id: HashMap::new(),
        }
    }
}

impl Default for PremiumSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            account_wide: true,
            pc_cafe_retail_like: true,
            cycle_database_writes: false,
            shown_rates: ShownRates::default(),
            rates: PremiumRates::default(),
        }
    }
}

/// A write of `account_premium` with Java's statement (`PremiumManager`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PremiumWrite {
    /// `REPLACE INTO account_premium (account_name,enddate) VALUE (?,?)`.
    Replace { account: String, end_date: i64 },
    /// `DELETE FROM account_premium WHERE account_name = ?`.
    Delete { account: String },
}

/// What the world keeps for the premium system.
#[derive(Debug, Default)]
pub(crate) struct PremiumWorld {
    pub(crate) settings: PremiumSettings,
    /// `PremiumManager._premiumData`: lower case key to the end date.
    data: HashMap<String, i64>,
    /// The multipliers shared with the drop rolls of a premium killer.
    rates: std::sync::Arc<PremiumRates>,
    /// The players with `_premiumStatus`.
    status: HashSet<ObjectId>,
    /// `PremiumManager._expiretasks`, by player (see the NOTE in `start_expire_task`).
    timers: HashMap<ObjectId, TimerId>,
}

/// Java `Float.toString` for the rates the `.premium` page prints: the shortest text that reads
/// back as the same float, with a fraction (`2.0`, `0.5`) and the `1.0E7` form outside `1e-3..1e7`.
pub(crate) fn java_float(value: f32) -> String {
    let abs = value.abs();
    if abs == 0.0 || (1e-3..1e7).contains(&abs) {
        return format!("{value:?}");
    }
    let text = format!("{value:e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let mantissa = if mantissa.contains('.') {
        mantissa.to_owned()
    } else {
        format!("{mantissa}.0")
    };
    format!("{mantissa}E{exponent}")
}

/// `DateTimeFormatter.ofPattern("dd.MM.yyyy HH:mm")` in the machine's zone.
pub(crate) fn premium_date(ms: i64) -> String {
    let local = ms.saturating_add(l2_util::time::local_offset_millis(ms));
    let days = local.div_euclid(86_400_000);
    let minutes = local.rem_euclid(86_400_000) / 60_000;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day:02}.{month:02}.{year:04} {:02}:{:02}",
        minutes / 60,
        minutes % 60
    )
}

impl PremiumRates {
    /// `rateChance *= PREMIUM_RATE_DROP_CHANCE_BY_ID.get(itemId)` or `PREMIUM_RATE_DROP_CHANCE`
    /// for a `DROP` item of the death drop list. Herb and raid drops get nothing ("TODO: Premium
    /// herb chance? :)" in Java).
    pub(crate) fn drop_chance_factor(&self, item_id: i32, herb: bool, raid: bool) -> f64 {
        if let Some(by_id) = self.drop_chance_by_id.get(&item_id) {
            f64::from(*by_id)
        } else if herb || raid {
            1.0
        } else {
            f64::from(self.drop_chance)
        }
    }

    /// The same for the amount.
    pub(crate) fn drop_amount_factor(&self, item_id: i32, herb: bool, raid: bool) -> f64 {
        if let Some(by_id) = self.drop_amount_by_id.get(&item_id) {
            f64::from(*by_id)
        } else if herb || raid {
            1.0
        } else {
            f64::from(self.drop_amount)
        }
    }
}

impl World {
    /// Installs the premium system. Call before players enter. The drop rolls read the same
    /// multipliers through `DropSettings`.
    pub fn set_premium_settings(&mut self, settings: PremiumSettings) {
        self.premium.rates = std::sync::Arc::new(settings.rates.clone());
        self.premium.settings = settings;
    }

    /// Java `PremiumSystemConfig.PREMIUM_SYSTEM_ENABLED`.
    pub fn premium_system_enabled(&self) -> bool {
        self.premium.settings.enabled
    }

    /// Java `Player.hasPremiumStatus()`: `PREMIUM_SYSTEM_ENABLED && _premiumStatus`.
    pub fn has_premium_status(&self, id: ObjectId) -> bool {
        self.premium.settings.enabled && self.premium.status.contains(&id)
    }

    /// The multipliers a drop roll of this killer applies (`PREMIUM_SYSTEM_ENABLED &&
    /// player.hasPremiumStatus()`), else `None`.
    pub(crate) fn premium_drop_rates(&self, id: ObjectId) -> Option<std::sync::Arc<PremiumRates>> {
        self.has_premium_status(id)
            .then(|| std::sync::Arc::clone(&self.premium.rates))
    }

    /// Java `Player.setPremiumStatus(value)`.
    fn set_premium_status(&mut self, id: ObjectId, value: bool) {
        if value {
            self.premium.status.insert(id);
        } else {
            self.premium.status.remove(&id);
        }
    }

    /// `PremiumSystemConfig.ACCOUNT_WIDE_PREMIUM ? player.getAccountName() : player.getName()`.
    fn premium_key_of(&self, id: ObjectId) -> Option<String> {
        let player = self.players.get(&id)?;
        Some(if self.premium.settings.account_wide {
            player.data.account.clone()
        } else {
            player.data.name.clone()
        })
    }

    /// Java `PremiumManager.getPremiumExpiration(accountName)`: 0 when the cache has no row.
    pub fn premium_expiration(&self, key: &str) -> i64 {
        self.premium.data.get(&key.to_lowercase()).copied().unwrap_or(0)
    }

    /// Premium rates for the exp of a kill: Java `addExp *= PREMIUM_RATE_XP` with a `long`
    /// and a `float` (the product is a float, cut back to a long), and the same for the sp.
    pub(crate) fn premium_exp_sp(&self, id: ObjectId, exp: i64, sp: i32) -> (i64, i32) {
        if !self.has_premium_status(id) {
            return (exp, sp);
        }
        let rates = &self.premium.settings.rates;
        ((exp as f32 * rates.xp) as i64, (sp as f32 * rates.sp) as i32)
    }

    /// Premium rates of `Party.calculateExpSpPartyCutoff`: a `double` times a `float`.
    pub(crate) fn premium_party_exp_sp(&self, id: ObjectId, exp: f64, sp: f64) -> (f64, f64) {
        if !self.has_premium_status(id) {
            return (exp, sp);
        }
        let rates = &self.premium.settings.rates;
        (exp * f64::from(rates.xp), sp * f64::from(rates.sp))
    }

    /// Premium rates of `Quest.addExpAndSp`.
    pub(crate) fn premium_quest_exp_sp(&self, id: ObjectId, exp: i64, sp: i32) -> (i64, i32) {
        if !self.has_premium_status(id) {
            return (exp, sp);
        }
        let rates = &self.premium.settings.rates;
        (
            (exp as f32 * rates.quest_xp) as i64,
            (sp as f32 * rates.quest_sp) as i32,
        )
    }

    // ------------------------------------------------------------------ the listeners

    /// `OnPlayerLogin` of `PremiumManager`: ask for the row.
    pub(crate) fn premium_on_enter(&mut self, id: ObjectId) {
        if !self.premium.settings.enabled {
            return;
        }
        let Some(key) = self.premium_key_of(id) else {
            return;
        };
        self.outbox.persist(PersistCommand::PremiumRequest {
            player: id,
            key: key.to_lowercase(),
        });
    }

    /// The answer to `PersistCommand::PremiumRequest`: the rest of `_playerLoginEvent`. `row` is
    /// the `enddate` of the row, `None` when there is none.
    pub(crate) fn on_premium_loaded(&mut self, id: ObjectId, key: &str, result: Result<Option<i64>, ()>) {
        if !self.players.contains_key(&id) || !self.premium.settings.enabled {
            return;
        }
        let row = match result {
            Ok(row) => row,
            Err(()) => {
                // Java: "Problem with PremiumManager" and goes on with the cache it has.
                warn!("Problem with PremiumManager: the row of {key} could not be read.");
                None
            }
        };
        // NOTE(java-quirk): `loadPremiumData` only puts a row it found; a cached end date stays
        // when the row is gone.
        if let Some(end) = row {
            self.premium.data.insert(key.to_lowercase(), end);
        }
        let now = unix_ms_now();
        let expiration = self.premium_expiration(key);
        self.set_premium_status(id, expiration > now);
        if self.has_premium_status(id) {
            self.start_premium_expire_task(id, expiration - now);
            // `EnterWorld` ran `PcCafePointsManager.run(player)` before the status was known.
            if self.premium.settings.pc_cafe_retail_like {
                self.pc_cafe_run(id);
            }
        } else if expiration > 0 {
            self.remove_premium_status(key, false);
        }
    }

    /// `_playerLogoutEvent`: the expire task goes. The status goes with the player.
    pub(crate) fn premium_on_leave(&mut self, id: ObjectId) {
        self.stop_premium_expire_task(id);
        self.premium.status.remove(&id);
    }

    /// `PremiumManager.startExpireTask(player, delay)`.
    // NOTE(java-quirk): Java keys the tasks by account, so a second character of the account
    // replaces the first one's entry (the first task still runs); here every player has its own
    // task, which ends the same status at the same time.
    fn start_premium_expire_task(&mut self, id: ObjectId, delay_ms: i64) {
        let delay = u64::try_from(delay_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::CustomPlay(CustomPlayTimer::PremiumExpire(id)));
        if let Some(old) = self.premium.timers.insert(id, timer) {
            self.timers.cancel(old);
        }
    }

    /// `PremiumManager.stopExpireTask(player)`.
    fn stop_premium_expire_task(&mut self, id: ObjectId) {
        if let Some(timer) = self.premium.timers.remove(&id) {
            self.timers.cancel(timer);
        }
    }

    /// `PremiumExpireTask.run`: `player.setPremiumStatus(false)`. The row stays until the next
    /// login finds it expired.
    pub(crate) fn on_premium_expire_timer(&mut self, id: ObjectId) {
        self.premium.timers.remove(&id);
        self.set_premium_status(id, false);
    }

    // ------------------------------------------------------------------ the manager

    /// Java `PremiumManager.addPremiumTime(accountName, days, DAYS)`.
    pub fn add_premium_time(&mut self, account_name: &str, days: i64) {
        let key = account_name.to_lowercase();
        let add_time = days.saturating_mul(86_400_000);
        let now = unix_ms_now();
        // "New premium task at least from now."
        let old = now.max(self.premium_expiration(&key));
        let new_expiration = old.saturating_add(add_time);
        self.outbox
            .persist(PersistCommand::Premium(PremiumWrite::Replace {
                account: key.clone(),
                end_date: new_expiration,
            }));
        self.premium.data.insert(key, new_expiration);
        // "UPDATE PlAYER PREMIUMSTATUS": the first player of the account only.
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        let found = ids.into_iter().find(|id| {
            self.players
                .get(id)
                .is_some_and(|p| p.data.account.eq_ignore_ascii_case(account_name))
        });
        if let Some(id) = found {
            self.stop_premium_expire_task(id);
            self.start_premium_expire_task(id, new_expiration - now);
            if !self.has_premium_status(id) {
                self.set_premium_status(id, true);
            }
        }
    }

    /// Java `PremiumManager.removePremiumStatus(accountName, checkOnline)`.
    pub fn remove_premium_status(&mut self, account_name: &str, check_online: bool) {
        let key = account_name.to_lowercase();
        if check_online {
            let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
            ids.sort_unstable();
            let found = ids.into_iter().find(|id| {
                self.players
                    .get(id)
                    .is_some_and(|p| p.data.account.eq_ignore_ascii_case(account_name))
                    && self.has_premium_status(*id)
            });
            if let Some(id) = found {
                self.set_premium_status(id, false);
                self.stop_premium_expire_task(id);
            }
        }
        self.premium.data.remove(&key);
        // The clean-up of an expired row is a global write (D-53); an admin's removal is the
        // explicit action of one account and always written.
        if check_online || self.premium.settings.cycle_database_writes {
            self.outbox
                .persist(PersistCommand::Premium(PremiumWrite::Delete { account: key }));
        }
    }

    // ------------------------------------------------------------------ `.premium`

    /// `Premium.onCommand` through `VoicedCommandHandler.getHandler(word)`. `None`: the premium
    /// system is off, or the word is not one of the commands (`MasterHandler` registers the
    /// handler only with `PREMIUM_SYSTEM_ENABLED`).
    pub(crate) fn premium_voiced_handler(&mut self, player: ObjectId, word: &str) -> Option<bool> {
        if !self.premium.settings.enabled || word != "premium" {
            return None;
        }
        let key = self.premium_key_of(player)?;
        let end_date = self.premium_expiration(&key);
        let settings = self.premium.settings.clone();
        let entity_type = if settings.account_wide {
            "Account"
        } else {
            "Player"
        };
        let shown = settings.shown_rates;
        let rates = &settings.rates;
        // `RATE * PREMIUM_RATE` is a float product.
        let times = |rate: f32, premium: f32| java_float(rate * premium);
        let mut html = String::new();
        if end_date == 0 {
            html.push_str(&format!(
                "<html><body><title>{entity_type}</title><center><table><tr><td><center>{entity_type} Status: <font color=\"LEVEL\">Normal<br></font></td></tr>"
            ));
            html.push_str(&format!(
                "<tr><td>Rate XP: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                java_float(shown.rate_xp)
            ));
            html.push_str(&format!(
                "<tr><td>Rate SP: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                java_float(shown.rate_sp)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Chance: <font color=\"LEVEL\"> x{}<br1></font></td></tr><br>",
                java_float(shown.death_drop_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Amount: <font color=\"LEVEL\"> x{}<br1></font></td></tr><br>",
                java_float(shown.death_drop_amount)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Chance: <font color=\"LEVEL\"> x{}<br1></font></td></tr><br>",
                java_float(shown.spoil_drop_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Amount: <font color=\"LEVEL\"> x{}<br><br></font></td></tr><br>",
                java_float(shown.spoil_drop_amount)
            ));
            html.push_str("<tr><td><center>Premium Info & Rules<br></td></tr>");
            html.push_str(&format!(
                "<tr><td>Rate XP: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.rate_xp, rates.xp)
            ));
            html.push_str(&format!(
                "<tr><td>Rate SP: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.rate_sp, rates.sp)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Chance: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.death_drop_chance, rates.drop_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Amount: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.death_drop_amount, rates.drop_amount)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Chance: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.spoil_drop_chance, rates.spoil_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Amount: <font color=\"LEVEL\"> x{}<br1></font></td></tr>",
                times(shown.spoil_drop_amount, rates.spoil_amount)
            ));
            html.push_str("<tr><td> <font color=\"70FFCA\">1. Premium benefits CAN NOT BE TRANSFERED.<br1></font></td></tr>");
            html.push_str("<tr><td> <font color=\"70FFCA\">2. Premium does not effect party members.<br1></font></td></tr>");
            html.push_str(if settings.account_wide {
                "<tr><td> <font color=\"70FFCA\">3. Premium benefits effect ALL characters in same account.</font></td></tr>"
            } else {
                "<tr><td> <font color=\"70FFCA\">3. Premium benefits effect one character in your account.</font></td></tr>"
            });
        } else {
            html.push_str(&format!(
                "<html><body><title>Premium {entity_type}Details</title><center><table><tr><td><center>{entity_type} Status: <font color=\"LEVEL\">Premium<br></font></td></tr>"
            ));
            html.push_str(&format!(
                "<tr><td>Rate XP: <font color=\"LEVEL\">x{} <br1></font></td></tr>",
                times(shown.rate_xp, rates.xp)
            ));
            html.push_str(&format!(
                "<tr><td>Rate SP: <font color=\"LEVEL\">x{}  <br1></font></td></tr>",
                times(shown.rate_sp, rates.sp)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Chance: <font color=\"LEVEL\">x{} <br1></font></td></tr>",
                times(shown.death_drop_chance, rates.drop_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Drop Amount: <font color=\"LEVEL\">x{} <br1></font></td></tr>",
                times(shown.death_drop_amount, rates.drop_amount)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Chance: <font color=\"LEVEL\">x{} <br1></font></td></tr>",
                times(shown.spoil_drop_chance, rates.spoil_chance)
            ));
            html.push_str(&format!(
                "<tr><td>Spoil Amount: <font color=\"LEVEL\">x{} <br1></font></td></tr>",
                times(shown.spoil_drop_amount, rates.spoil_amount)
            ));
            html.push_str(&format!(
                "<tr><td>Expires: <font color=\"00A5FF\">{}</font></td></tr>",
                premium_date(end_date)
            ));
            html.push_str(&format!(
                "<tr><td>Current Date: <font color=\"70FFCA\">{}<br><br></font></td></tr>",
                premium_date(unix_ms_now())
            ));
            html.push_str("<tr><td><center>Premium Info & Rules<br></center></td></tr>");
            html.push_str("<tr><td><font color=\"70FFCA\">1. Premium status CAN NOT BE TRANSFERED.<br1></font></td></tr>");
            html.push_str("<tr><td><font color=\"70FFCA\">2. Premium does not effect party members.<br1></font></td></tr>");
            html.push_str(if settings.account_wide {
                "<tr><td><font color=\"70FFCA\">3. Premium account effects ALL characters in same account.<br><br><br></font></td></tr>"
            } else {
                "<tr><td><font color=\"70FFCA\">3. Premium account effect one character in your account.<br><br><br></font></td></tr>"
            });
            html.push_str("<tr><td><center>Thank you for supporting our server.</td></tr>");
        }
        html.push_str("</table></center></body></html>");
        // `new NpcHtmlMessage(5)`: the item id is 5.
        self.send_html(
            player,
            &NpcHtmlMessage {
                npc_obj_id: 0,
                html,
                item_id: 5,
            },
        );
        Some(true)
    }

    // ------------------------------------------------------------------ `//premium_*`

    /// The `premium_menu.htm` page of `AdminPremium.onCommand`, sent at the end of every command.
    fn show_premium_menu(&mut self, gm: ObjectId) {
        let html = self
            .get_htm(gm, "data/html/admin/premium_menu.htm")
            .unwrap_or_default();
        self.send_html(
            gm,
            &NpcHtmlMessage {
                npc_obj_id: 0,
                html,
                item_id: 0,
            },
        );
    }

    /// `AdminPremium.addPremiumStatus`.
    fn admin_add_premium(&mut self, gm: ObjectId, months: i64, account_name: &str) {
        if !self.premium.settings.enabled {
            self.send_message(gm, "Premium system is disabled.");
            return;
        }
        let entity_type = if self.premium.settings.account_wide {
            "Account "
        } else {
            "Player "
        };
        // "TODO: Add check if account exists XD"
        self.add_premium_time(account_name, months * 30);
        let until = premium_date(self.premium_expiration(account_name));
        self.send_message(
            gm,
            &format!("{entity_type}{account_name} will now have premium status until {until}."),
        );
        if self.premium.settings.pc_cafe_retail_like {
            let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
            ids.sort_unstable();
            // `player.getAccountName().matches(accountName)`: a regex match of the whole name;
            // an account name is plain text, so it is the equality.
            let found = ids.into_iter().find(|id| {
                self.players
                    .get(id)
                    .is_some_and(|p| p.data.account == account_name)
            });
            if let Some(id) = found {
                self.pc_cafe_run(id);
            }
        }
    }

    /// `AdminPremium.viewPremiumInfo`.
    fn admin_premium_info(&mut self, gm: ObjectId, account_name: &str) {
        if !self.premium.settings.enabled {
            self.send_message(gm, "Premium system is disabled.");
            return;
        }
        let entity_type = if self.premium.settings.account_wide {
            "Account "
        } else {
            "Player "
        };
        let end = self.premium_expiration(account_name);
        if end > 0 {
            self.send_message(
                gm,
                &format!(
                    "{entity_type}{account_name} has premium status until {}.",
                    premium_date(end)
                ),
            );
        } else {
            self.send_message(gm, &format!("{entity_type}{account_name} has no premium status."));
        }
    }

    /// `AdminPremium.removePremium`.
    fn admin_remove_premium(&mut self, gm: ObjectId, account_name: &str) {
        if !self.premium.settings.enabled {
            self.send_message(gm, "Premium system is disabled.");
            return;
        }
        let entity_type = if self.premium.settings.account_wide {
            "Account "
        } else {
            "Player "
        };
        if self.premium_expiration(account_name) > 0 {
            self.remove_premium_status(account_name, true);
            self.send_message(
                gm,
                &format!("{entity_type}{account_name} has no longer premium status."),
            );
        } else {
            self.send_message(gm, &format!("{entity_type}{account_name} has no premium status."));
        }
    }
}

impl World {
    /// Runs `AdminPremium.onCommand` for a GM, as `//premium_menu`, `//premium_add1`, ...
    /// do once the command handler has found the access right. For tools and tests.
    pub fn premium_admin_command(&mut self, gm: ObjectId, command: &str) {
        // The handler never fails: Java's exceptions of this command are answered inside.
        let _ = admin_premium(self, gm, command);
    }
}

/// Registers the admin commands (called by `AdminWorld::default`).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    for command in [
        "admin_premium_menu",
        "admin_premium_add1",
        "admin_premium_add2",
        "admin_premium_add3",
        "admin_premium_info",
        "admin_premium_remove",
    ] {
        handlers.register(command, admin_premium as AdminHandler);
    }
}

/// Java `AdminPremium.onCommand`. `command.substring(19)` is the text after the command and one
/// character; a command without it is a `StringIndexOutOfBoundsException` that answers "Please
/// enter a valid account name.".
fn admin_premium(world: &mut World, gm: ObjectId, command: &str) -> Result<(), JavaException> {
    let word = java_split_space(command).first().copied().unwrap_or("");
    let tail = |skip: usize| command.get(skip..);
    let invalid = |world: &World| world.send_sys_message(gm, "Please enter a valid account name.");
    match word {
        "admin_premium_menu" => {}
        "admin_premium_add1" | "admin_premium_add2" | "admin_premium_add3" => {
            let months = i64::from(word.as_bytes()[word.len() - 1] - b'0');
            match tail(19) {
                Some(account) => world.admin_add_premium(gm, months, account),
                None => invalid(world),
            }
        }
        "admin_premium_info" => match tail(19) {
            Some(account) => world.admin_premium_info(gm, account),
            None => invalid(world),
        },
        "admin_premium_remove" => match tail(21) {
            Some(account) => world.admin_remove_premium(gm, account),
            None => invalid(world),
        },
        _ => {}
    }
    world.show_premium_menu(gm);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_float_text() {
        assert_eq!(java_float(2.0), "2.0");
        assert_eq!(java_float(0.5), "0.5");
        assert_eq!(java_float(1.5 * 2.0), "3.0");
        assert_eq!(java_float(1.0e7), "1.0E7");
        assert_eq!(java_float(0.0), "0.0");
    }

    #[test]
    fn dates_are_day_month_year_hour_minute() {
        // 2001-09-09 01:46:40 UTC; the zone offset of the machine moves it, the shape does not.
        let text = premium_date(1_000_000_000_000);
        assert_eq!(text.len(), 16);
        assert_eq!(&text[2..3], ".");
        assert_eq!(&text[5..6], ".");
        assert_eq!(&text[10..11], " ");
        assert_eq!(&text[13..14], ":");
        assert!(text.contains(".2001 ") || text.contains(".2001"));
    }

    #[test]
    fn premium_factors_skip_herbs_and_raids() {
        let mut rates = PremiumRates::default();
        rates.drop_chance_by_id.insert(57, 3.0);
        assert_eq!(rates.drop_chance_factor(57, false, true), 3.0);
        assert_eq!(rates.drop_chance_factor(1, true, false), 1.0);
        assert_eq!(rates.drop_chance_factor(1, false, true), 1.0);
        assert_eq!(rates.drop_chance_factor(1, false, false), 2.0);
        assert_eq!(rates.drop_amount_factor(1, false, false), 1.0);
    }
}
