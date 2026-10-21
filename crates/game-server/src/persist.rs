//! The persistence task: stores player snapshots in order and tells the login server link when
//! a player's data is safe.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (store, storeCharBase)
// Java: org/l2jmobius/gameserver/taskmanagers/PlayerAutoSaveTaskManager.java (autosave interval)
// Java: org/l2jmobius/gameserver/Shutdown.java (everybody is stored before the DB is closed)
// Design: docs/design/world-loop.md ("Persistence")
//
// The world never waits for the database (D-15): it pushes `PersistCommand`s into the outbox
// channel and this task applies them one after the other. One task, one queue, so the writes of
// a character are applied in the order the world produced them, and `PlayerLeft` (which the
// world sends after the final `StorePlayer`) is only handled once that store finished. That is
// requirement 1: the login server hears `PlayerLogout` strictly after the data is written, so
// a quick re-login on the Java game server can never read stale rows.
//
// Side by side with the Java GS (D-39, requirement 2): the Java game server runs on the same
// database. This task only ever writes
// - `characters` through Java's own `UPDATE_CHARACTER` and `UPDATE_ONLINE_STATUS`,
// - `items`, `character_skills`, `character_shortcuts` of the character it stores,
// - `character_skills_save` and `character_item_reuse_save` (the buffs and reuse timers),
// - `character_reco_bonus` (the Nevit hourglass time) and the `character_variables` rows the
//   world changed (Nevit's points; M4-VIT-01),
// - `character_reco_bonus` again for a vote or `RecoGiveTask` (`storeRecommendationValues`) and
//   for Java's daily reset (`resetRecommends`: the two `UPDATE`s over every row, M5-FU-03; only
//   with `DailyResetDatabaseWrites = True` in `SideBySide.ini`),
// - the clan tables (`clan_data`, `clan_privs`, `clan_subpledges`, `clan_notices`) and the clan
//   columns of offline members' `characters` rows, with Java's clan statements
//   (`PersistCommand::Clan`, M6-CLAN-01),
// with the statements the repositories copied from Java. It never runs bulk updates or cleanup
// on its own; the daily reset `UPDATE`s are the one bulk write, because the Java game server
// runs the same ones every day.
//
// Item rows reach the database two ways, like in Java: `PersistCommand::ItemChanges` carries the
// rows of one `Item.updateDatabase()` (an equip, an unequip, a destroyed item) and is written at
// once; the full store (`StorePlayer`) writes what differs from the last write, which covers
// everything Java only writes later (a lower count, new items). The baseline of a character
// follows both, so neither writes a row twice or inserts a row that exists.
// `UPDATE_CHARACTER` writes every column, including ones M2 does not model (nobless...). The clan
// columns come from the snapshot when it carries them (`PlayerProfile::clan`, M6-CLAN-01).
// (`vitality_points` is modelled since M4-VIT-01 and comes from the snapshot.) Those are read from the row first and written back unchanged, so a store never
// changes what the Java side owns. The account may be on one game server at a time (the login
// server enforces it), so nobody else changes the row between that read and the write.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_config::IniFile;
use l2_config::game::GeneralConfig;
use l2_db::{
    CharacterRepository, CharacterRow, CharacterUpdate, ClanRepository, ClanUpdate, GroundItemRecord,
    GroundItemRepository, ItemLocation, ItemRepository, ItemReuseRepository, ItemUpdate, NewItem,
    NewItemReuse, NewPunishment, NewShortcut, NewSkillSave, Pool, PunishmentRepository,
    RecommendationRepository, ShortcutRepository, SkillLevel, SkillRepository, VariableRepository,
};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanRow, ClanUpdateRow};
use l2_world::{
    ClanRecord, ClanWrite, GroundItemRow, ItemData, LsEvent, ObjectId, PersistCommand, PlayerData,
    PunishmentAffect, PunishmentRecord, PunishmentType, SavedEffect, SavedItemReuse, ShortcutData, SkillData,
    StoredPunishment,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, error, info, warn};

use crate::runtime::OutboxMessage;

/// What the login server link has to send, in the order the world produced it.
///
/// The persistence task is the only consumer of the world's outbox, so it also forwards the
/// login server events. Keeping both in one queue preserves their order (a player who enters
/// and leaves quickly is announced before it is logged out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsNotice {
    /// Java `LoginServerThread.sendLogin` -> `PlayerInGame`.
    PlayerInGame { account: String },
    /// Java `LoginServerThread.sendLogout` -> `PlayerLogout`. Only sent after the character's
    /// final store finished (or failed for good, which is logged).
    PlayerLogout { account: String },
    /// Java `LoginServerThread.sendAccessLevel` -> `ChangeAccessLevel` (a ban is -1, its end 0).
    AccessLevel { account: String, level: i32 },
}

/// Why a store failed.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("database error: {0}")]
    Db(#[from] l2_db::DbError),
    /// There is no row to update. The Rust game server never creates characters here.
    #[error("character {0} does not exist in the database")]
    NoSuchCharacter(i32),
    /// The row belongs to another account than the snapshot says: something is badly wrong, so
    /// nothing is written.
    #[error("character {0} belongs to another account than {1}")]
    WrongAccount(i32, String),
}

/// Where snapshots go. The real one is [`DbStore`]; tests record the calls.
pub trait PlayerStore: Send + 'static {
    /// Writes the character's current state.
    fn store_player(&mut self, player: &PlayerData) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// The character left the world: marks it offline (Java `setOnlineStatus(false, true)`).
    fn player_offline(
        &mut self,
        object_id: ObjectId,
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Item rows that changed (Java `Item.updateDatabase()`): written at once, not with the next
    /// full store. `upserts` become an `UPDATE` when the row is known, else an `INSERT`.
    fn store_item_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[ItemData],
        removed: &[ObjectId],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `ItemsOnGroundManager.run()`: the table `itemsonground` is emptied and `rows` are
    /// inserted. Only sent with `SaveDroppedItem` on.
    fn store_ground_items(
        &mut self,
        rows: &[GroundItemRow],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// `character_skills` rows that changed (Java `Player.storeSkill` / `removeSkill`): written at
    /// once, not with the next full store. An upsert becomes an `UPDATE` of the level when the row
    /// is known, else a `REPLACE`; `removed` skill ids are deleted.
    fn store_skill_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[SkillData],
        removed: &[i32],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `Player.storeRecommendationValues`: `REPLACE INTO character_reco_bonus
    /// (charId,rec_have,rec_left)`; `time_left` falls back to the column default (0).
    fn store_reco_values(
        &mut self,
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `DailyResetManager.resetRecommends`, the database half: the two `UPDATE`s of every
    /// `character_reco_bonus` row.
    fn reset_recommends(&mut self) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `PlayerVariables.storeMe`: the `character_variables` rows one character changed
    /// (deletes first, then upserts).
    fn store_player_variables(
        &mut self,
        owner: ObjectId,
        upserts: &[(String, String)],
        removed: &[String],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;
    /// Java `PunishmentTask.onStart`: `INSERT INTO punishments`. The store keeps the id the
    /// database gave the row to find it again in [`end_punishment`](Self::end_punishment).
    fn store_punishment(
        &mut self,
        _punishment: &StoredPunishment,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `PunishmentTask.onEnd`: the row's `expiration` becomes `ended_at`. `id` is 0 for a
    /// punishment this store inserted itself (found again by key, affect and type).
    fn end_punishment(
        &mut self,
        _id: i32,
        _key: &str,
        _affect: PunishmentAffect,
        _kind: PunishmentType,
        _ended_at: i64,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `PunishmentManager.changeCharAccessLevel`.
    fn change_char_access_level(
        &mut self,
        _char_id: i32,
        _level: i32,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A clan row write with Java's statement (`Clan.store`, `updateClanInDB`, ...).
    fn store_clan_write(
        &mut self,
        _write: &ClanWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }
}

/// How often a failed store is tried again before it is given up (and logged as lost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total tries, at least 1.
    pub attempts: u32,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 2,
            delay: Duration::from_millis(500),
        }
    }
}

/// Who waits for a character's last store. The session of a client that goes back to the
/// character screen (`Restart`) waits here before it reads the character list, because Java
/// stores synchronously before it answers `RestartResponse` (found by M2-E2E-01).
///
/// Network-side bookkeeping, not game state (D-10): the lock is held only to add or take a
/// waiter.
#[derive(Clone, Default)]
pub struct LeftAcks(Arc<Mutex<HashMap<ObjectId, Vec<oneshot::Sender<()>>>>>);

impl LeftAcks {
    /// A receiver that completes once `PlayerLeft` of `id` was handled: the final store is done
    /// (or failed for good, which is logged) and the character is marked offline.
    pub fn wait_for(&self, id: ObjectId) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id)
            .or_default()
            .push(tx);
        rx
    }

    /// Wakes everyone waiting for `id`. The persistence task calls it after `PlayerLeft`.
    pub fn ack(&self, id: ObjectId) {
        let waiters = self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
        for tx in waiters.into_iter().flatten() {
            let _ = tx.send(());
        }
    }
}

/// Starts the persistence task on the receiving end of the world's outbox.
///
/// The task ends when the channel is closed (the world thread is gone), after handling
/// everything that was queued: that is the shutdown drain (D-15).
pub fn spawn<S: PlayerStore>(
    store: S,
    messages: UnboundedReceiver<OutboxMessage>,
    notices: UnboundedSender<LsNotice>,
    retry: RetryPolicy,
    acks: LeftAcks,
) -> task::JoinHandle<()> {
    task::spawn(run(store, messages, notices, retry, acks))
}

async fn run<S: PlayerStore>(
    mut store: S,
    mut messages: UnboundedReceiver<OutboxMessage>,
    notices: UnboundedSender<LsNotice>,
    retry: RetryPolicy,
    acks: LeftAcks,
) {
    // Characters whose last store failed for good.
    let mut unsaved: HashSet<ObjectId> = HashSet::new();
    let attempts = retry.attempts.max(1);
    while let Some(message) = messages.recv().await {
        match message {
            OutboxMessage::Persist(PersistCommand::StorePlayer(player)) => {
                let mut result = store.store_player(&player).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store {} (try {} of {attempts}): {e}",
                        player.name,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_player(&player).await;
                }
                match result {
                    Ok(()) => {
                        unsaved.remove(&player.object_id);
                        debug!("Stored {}.", player.name);
                    }
                    Err(e) => {
                        error!("The state of {} could NOT be stored: {e}", player.name);
                        unsaved.insert(player.object_id);
                    }
                }
            }
            OutboxMessage::Persist(PersistCommand::PlayerLeft {
                account,
                object_id,
                logout,
            }) => {
                if unsaved.remove(&object_id) {
                    // Java logs a failed store and carries on with the logout, so does this.
                    error!("Character {} left with data that is NOT stored.", object_id.0);
                }
                if let Err(e) = store.player_offline(object_id).await {
                    error!("Could not mark character {} offline: {e}", object_id.0);
                }
                // A restart keeps the connection: the account is still on this server.
                if logout {
                    let _ = notices.send(LsNotice::PlayerLogout { account });
                }
                acks.ack(object_id);
            }
            OutboxMessage::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_item_changes(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store item changes of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_item_changes(owner, &upserts, &removed).await;
                }
                // A lost item write is caught by the next full store, which compares the whole
                // item list against what was last written.
                if let Err(e) = result {
                    error!("Item changes of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::SaveGroundItems(rows)) => {
                let mut result = store.store_ground_items(&rows).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the items on the ground (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_ground_items(&rows).await;
                }
                // The next periodic save (or the one at shutdown) writes the whole table again.
                if let Err(e) = result {
                    error!("The items on the ground could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::SkillChanges {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_skill_changes(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store skill changes of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_skill_changes(owner, &upserts, &removed).await;
                }
                // A lost skill write is caught by the next full store, which compares the whole
                // skill list against what was last written.
                if let Err(e) = result {
                    error!("Skill changes of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::RecoValues {
                owner,
                rec_have,
                rec_left,
            }) => {
                let mut result = store.store_reco_values(owner, rec_have, rec_left).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the recommendations of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_reco_values(owner, rec_have, rec_left).await;
                }
                // Java logs it and goes on; the next full store writes the counters again.
                if let Err(e) = result {
                    error!("The recommendations of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::ResetRecommends) => {
                // Java: "Could not reset Recommendations System", logged, no retry.
                if let Err(e) = store.reset_recommends().await {
                    error!("Could not reset Recommendations System: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::PlayerVariables {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_player_variables(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the variables of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_player_variables(owner, &upserts, &removed).await;
                }
                // The variables stay marked as changed in the world: the next full store writes
                // them again.
                if let Err(e) = result {
                    error!("The variables of {} could NOT be stored: {e}", owner.0);
                }
            }

            OutboxMessage::Persist(PersistCommand::StorePunishment(punishment)) => {
                let mut result = store.store_punishment(&punishment).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the punishment of {} (try {} of {attempts}): {e}",
                        punishment.key,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_punishment(&punishment).await;
                }
                // Java logs it and goes on: the punishment is active, it is only not stored.
                if let Err(e) = result {
                    error!("The punishment of {} could NOT be stored: {e}", punishment.key);
                }
            }
            OutboxMessage::Persist(PersistCommand::EndPunishment {
                id,
                key,
                affect,
                kind,
                ended_at,
            }) => {
                let mut result = store.end_punishment(id, &key, affect, kind, ended_at).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not end the punishment of {key} (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.end_punishment(id, &key, affect, kind, ended_at).await;
                }
                if let Err(e) = result {
                    error!("The end of the punishment of {key} could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::CharAccessLevel { char_id, level }) => {
                let mut result = store.change_char_access_level(char_id, level).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not set the access level of character {char_id} (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.change_char_access_level(char_id, level).await;
                }
                if let Err(e) = result {
                    error!("The access level of character {char_id} could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Clan(write)) => {
                // Java logs a failed clan statement and goes on (no retry).
                if let Err(e) = store.store_clan_write(&write).await {
                    error!("Clan data could NOT be stored ({}): {e}", clan_write_name(&write));
                }
            }
            OutboxMessage::Ls(LsEvent::PlayerInGame { account }) => {
                let _ = notices.send(LsNotice::PlayerInGame { account });
            }
            OutboxMessage::Ls(LsEvent::AccessLevel { account, level }) => {
                let _ = notices.send(LsNotice::AccessLevel { account, level });
            }
        }
    }
    debug!("Outbox closed, persistence task done.");
}

/// Java `PunishmentManager.load()`, the database half: the rows of `punishments`. A database
/// error is logged and the server goes on without punishments, like Java. The world sorts out
/// the unknown and the expired ones (`World::load_punishments`).
pub async fn load_punishments(pool: &Pool) -> Vec<PunishmentRecord> {
    match PunishmentRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| PunishmentRecord {
                id: i32::try_from(row.id).unwrap_or(0),
                key: row.key,
                affect: row.affect,
                kind: row.kind,
                expiration: row.expiration,
                reason: row.reason,
                punished_by: row.punished_by,
            })
            .collect(),
        Err(e) => {
            warn!("PunishmentManager: Error while loading punishments: {e}");
            Vec::new()
        }
    }
}

/// Java `ClanTable`'s constructor, the database half: every clan (`new Clan(clanId)`'s queries)
/// and the `clan_wars` rows. Only reads (D-39). A clan whose rows cannot be read is logged and
/// left out, like Java's caught `restore` (which keeps a half-restored clan).
pub async fn load_clans(pool: &Pool) -> (Vec<ClanRecord>, Vec<(String, String)>) {
    let repo = ClanRepository::new(pool.clone());
    let ids = match repo.clan_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            error!("ClanTable: Error restoring ClanTable. {e}");
            return (Vec::new(), Vec::new());
        }
    };
    let mut clans = Vec::with_capacity(ids.len());
    for clan_id in ids {
        match load_clan(&repo, clan_id).await {
            Ok(record) => clans.push(record),
            Err(e) => error!("Clan: Error restoring clan data of clan {clan_id}: {e}"),
        }
    }
    let wars = match repo.clan_wars().await {
        Ok(rows) => rows.into_iter().map(|r| (r.clan1, r.clan2)).collect(),
        Err(e) => {
            error!("ClanTable: Error restoring clan wars data. {e}");
            Vec::new()
        }
    };
    (clans, wars)
}

/// The rows of one clan (`Clan.restore`, `restoreSubPledges`, `restoreRankPrivs`,
/// `restoreSkills`, `restoreNotice`).
pub async fn load_clan(repo: &ClanRepository, clan_id: i32) -> Result<ClanRecord, l2_db::DbError> {
    let mut record = ClanRecord {
        clan_id,
        ..ClanRecord::default()
    };
    if let Some(row) = repo.clan_data(clan_id).await? {
        let int = |v: Option<i32>| v.unwrap_or(0);
        record.data = Some(ClanDataRecord {
            name: row.clan_name,
            level: int(row.clan_level),
            reputation_score: row.reputation_score,
            castle_id: int(row.has_castle),
            blood_alliance_count: i32::from(row.blood_alliance_count),
            blood_oath_count: i32::from(row.blood_oath_count),
            ally_id: int(row.ally_id),
            ally_name: row.ally_name,
            leader_id: int(row.leader_id),
            crest_id: int(row.crest_id),
            crest_large_id: int(row.crest_large_id),
            ally_crest_id: int(row.ally_crest_id),
            auction_bid_at: row.auction_bid_at,
            ally_penalty_expiry_time: clamp_i64(row.ally_penalty_expiry_time),
            ally_penalty_type: i32::from(row.ally_penalty_type),
            char_penalty_expiry_time: clamp_i64(row.char_penalty_expiry_time),
            dissolving_expiry_time: clamp_i64(row.dissolving_expiry_time),
            // `getInt` of an unsigned `INT(10)`: the low 32 bits.
            new_leader_id: row.new_leader_id as i32,
        });
        record.members = repo
            .clan_members(clan_id)
            .await?
            .into_iter()
            .map(|m| ClanMemberRecord {
                object_id: clamp_i32(u64::from(m.char_id)),
                name: m.char_name,
                level: m.level.map_or(0, i32::from),
                class_id: m.classid.map_or(0, i32::from),
                // `getString("title")` may be null; it is written as an empty string.
                title: m.title.unwrap_or_default(),
                power_grade: m.power_grade.map_or(0, i32::from),
                pledge_type: i32::from(m.subpledge),
                apprentice: clamp_i32(u64::from(m.apprentice)),
                sponsor: clamp_i32(u64::from(m.sponsor)),
                female: m.sex.is_some_and(|s| s != 0),
                race: m.race.map_or(0, i32::from),
            })
            .collect();
    }
    record.sub_pledges = repo
        .sub_pledges(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.sub_pledge_id, r.name.unwrap_or_default(), r.leader_id))
        .collect();
    record.rank_privs = repo
        .rank_privs(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.rank, r.party, r.privs))
        .collect();
    record.skills = repo
        .clan_skills(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.skill_id, r.skill_level, r.sub_pledge_id))
        .collect();
    record.notices = repo
        .clan_notices(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.enabled, r.notice))
        .collect();
    Ok(record)
}

/// The Java method a clan write comes from, for the log.
fn clan_write_name(write: &ClanWrite) -> &'static str {
    match write {
        ClanWrite::Insert(_) => "Clan.store",
        ClanWrite::Update(_) => "Clan.updateClanInDB",
        ClanWrite::Level { .. } => "Clan.changeLevel",
        ClanWrite::AllyCrestOfClan { .. } => "Clan.changeAllyCrest",
        ClanWrite::RemoveMember { .. } => "Clan.removeMemberInDatabase",
        ClanWrite::CharClanPrivs { .. } => "Clan.setNewLeader",
        ClanWrite::CharSubPledge { .. } => "ClanMember.updatePledgeType",
        ClanWrite::CharPowerGrade { .. } => "ClanMember.updatePowerGrade",
        ClanWrite::CharApprenticeSponsor { .. } => "ClanMember.saveApprenticeAndSponsor",
        ClanWrite::InsertSubPledge { .. } => "Clan.createSubPledge",
        ClanWrite::UpdateSubPledge { .. } => "Clan.updateSubPledgeInDB",
        ClanWrite::RankPrivs { .. } => "Clan.setRankPrivs",
        ClanWrite::Notice { .. } => "Clan.storeNotice",
        ClanWrite::Delete { .. } => "ClanTable.destroyClan",
        ClanWrite::UpdateSkill { .. } | ClanWrite::InsertSkill { .. } => "Clan.addNewSkill",
    }
}

/// `INSERT_CLAN_DATA`'s values.
fn new_clan_of(row: &ClanRow) -> l2_db::NewClan {
    l2_db::NewClan {
        clan_id: row.clan_id,
        clan_name: row.name.clone(),
        clan_level: row.level,
        castle_id: row.castle_id,
        blood_alliance_count: row.blood_alliance_count,
        blood_oath_count: row.blood_oath_count,
        ally_id: row.ally_id,
        ally_name: row.ally_name.clone(),
        leader_id: row.leader_id,
        crest_id: row.crest_id,
        crest_large_id: row.crest_large_id,
        ally_crest_id: row.ally_crest_id,
        new_leader_id: row.new_leader_id,
    }
}

/// `updateClanInDB`'s values.
fn clan_update_of(row: &ClanUpdateRow) -> ClanUpdate {
    ClanUpdate {
        clan_id: row.clan_id,
        leader_id: row.leader_id,
        ally_id: row.ally_id,
        ally_name: row.ally_name.clone(),
        reputation_score: row.reputation_score,
        ally_penalty_expiry_time: row.ally_penalty_expiry_time,
        ally_penalty_type: row.ally_penalty_type,
        char_penalty_expiry_time: row.char_penalty_expiry_time,
        dissolving_expiry_time: row.dissolving_expiry_time,
        new_leader_id: row.new_leader_id,
    }
}

/// Java `CharInfoTable`'s constructor, the database half: the id and name of every character.
pub async fn load_char_names(pool: &Pool) -> Vec<(i32, String)> {
    match CharacterRepository::new(pool.clone()).all_names().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| (i32::try_from(row.char_id).unwrap_or(0), row.char_name))
            .collect(),
        Err(e) => {
            warn!("CharInfoTable: Failed to load character names: {e}");
            Vec::new()
        }
    }
}

/// Java `ItemsOnGroundManager`'s constructor, the database half (`load()`): what is in
/// `itemsonground` when the server starts. The world puts the rows back
/// (`World::load_ground_items`).
///
/// D-39: exactly as Java, nothing touches the table unless the owner turned `SaveDroppedItem`
/// (read, and rewritten later by the world's periodic save) or `ClearDroppedItemTable` (emptied
/// when `SaveDroppedItem` is off) on. A database error is logged and the server goes on without
/// the items, like Java.
pub async fn restore_ground_items(pool: &Pool, general: &GeneralConfig) -> Vec<GroundItemRow> {
    let repo = GroundItemRepository::new(pool.clone());
    // If SaveDroppedItem is false, may want to delete all items previously stored to avoid add old
    // items on reactivate.
    if !general.save_dropped_item && general.clear_dropped_item_table {
        if let Err(e) = repo.empty_table().await {
            error!("ItemsOnGroundManager: Error while cleaning table ItemsOnGround {e}");
        }
    }
    if !general.save_dropped_item {
        return Vec::new();
    }

    // If DestroyPlayerDroppedItem was previously false, items currently protected will be added to
    // ItemsAutoDestroy.
    if general.destroy_dropped_player_item {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        if let Err(e) = repo
            .recycle_protected(now_ms, general.destroy_equipable_player_item)
            .await
        {
            error!("ItemsOnGroundManager: Error while updating table ItemsOnGround {e}");
        }
    }

    // Add items to world.
    let rows = match repo.load_all().await {
        Ok(records) => records
            .into_iter()
            .map(|r| GroundItemRow {
                object_id: ObjectId(r.object_id),
                item_id: r.item_id,
                count: r.count,
                enchant_level: r.enchant_level,
                x: r.x,
                y: r.y,
                z: r.z,
                drop_time: r.drop_time,
                equipable: r.equipable,
            })
            .collect(),
        Err(e) => {
            error!("ItemsOnGroundManager: Error while loading ItemsOnGround {e}");
            Vec::new()
        }
    };

    if general.empty_dropped_item_table_after_load {
        if let Err(e) = repo.empty_table().await {
            error!("ItemsOnGroundManager: Error while cleaning table ItemsOnGround {e}");
        }
    }
    rows
}

/// Java `GeneralConfig.CHAR_DATA_STORE_INTERVAL`: `CharacterDataStoreInterval` (minutes,
/// default 15) in `General.ini`. The generated config keeps this key as a placeholder, so it
/// is read here, with Java's default when the file or key is missing.
///
/// Java checks the players once a second, so anything below a second behaves like a second.
pub fn autosave_interval(config_dir: &std::path::Path) -> Duration {
    let minutes = match IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        Ok(ini) => ini.get_int("CharacterDataStoreInterval", 15),
        Err(e) => {
            warn!(
                "Cannot read {}: {e}; autosave uses the default of 15 minutes.",
                GeneralConfig::FILE
            );
            15
        }
    };
    let millis = i64::from(minutes).saturating_mul(60_000).max(1000);
    Duration::from_millis(u64::try_from(millis).unwrap_or(1000))
}

/// Java `Item.restoreAttributes` for the items of one owner: the augmentation and the element
/// attributes the `item_attributes` and `item_elementals` rows give them. Java reads the two
/// tables one item at a time; this reads them once for the owner (same rows, same order).
// Java: Item.restoreAttributes
pub async fn restore_item_attributes<'a>(
    repo: &ItemRepository,
    owner_id: i32,
    items: impl IntoIterator<Item = &'a mut ItemData>,
) -> Result<(), l2_db::DbError> {
    let mut items: Vec<&mut ItemData> = items.into_iter().collect();
    if items.is_empty() {
        return Ok(());
    }
    let augmentations = repo.augmentations_of_owner(owner_id).await?;
    let elementals = repo.elementals_of_owner(owner_id).await?;
    apply_item_attribute_rows(&mut items, &augmentations, &elementals);
    Ok(())
}

/// The rule part of [`restore_item_attributes`]: the rows of the augmentation `-1` and of an
/// element with type or value `-1` are skipped (the table's "nothing" default).
fn apply_item_attribute_rows(
    items: &mut [&mut ItemData],
    augmentations: &[l2_db::OwnerAugmentationRow],
    elementals: &[l2_db::OwnerElementalRow],
) {
    for item in items.iter_mut() {
        let id = item.object_id.0;
        for row in augmentations.iter().filter(|r| r.item_id == id) {
            if row.aug_attributes != -1 {
                item.augmentation = Some(row.aug_attributes);
            }
        }
        for row in elementals.iter().filter(|r| r.item_id == id) {
            if row.elem_type != -1 && row.elem_value != -1 {
                item.apply_attribute(i32::from(row.elem_type), row.elem_value);
            }
        }
    }
}

/// The element rows an item writes (`Item.updateItemElements`), ordered by element so two lists
/// with the same elements compare equal.
fn element_rows(item: &ItemData) -> Vec<l2_db::ItemElementalRow> {
    let mut rows: Vec<l2_db::ItemElementalRow> = item
        .elementals
        .iter()
        .map(|e| l2_db::ItemElementalRow {
            // The column is a `tinyint`; `Elementals.getElement()` is a Java `byte`.
            elem_type: e.element as i8,
            elem_value: e.value,
        })
        .collect();
    rows.sort_by_key(|r| r.elem_type);
    rows
}

// ---------------------------------------------------------------------------------------
// The database store
// ---------------------------------------------------------------------------------------

/// What is known to be in the database for one character, so a store only writes the
/// difference (Java writes items, skills and shortcuts when they change, not on every save).
#[derive(Debug, Default)]
struct Baseline {
    items: HashMap<i32, ItemData>,
    skills: HashMap<i32, i32>,
    shortcuts: HashMap<(i32, i32), ShortcutData>,
    /// True while the maps come from the database instead of an earlier snapshot. Rows that are
    /// only in the database are not ours to delete: the snapshot may simply not carry them.
    from_database: bool,
}

impl Baseline {
    /// The state after a successful store: the snapshot itself.
    fn of_snapshot(player: &PlayerData) -> Self {
        Self {
            items: player.items.iter().map(|i| (i.object_id.0, i.clone())).collect(),
            skills: player
                .skills
                .iter()
                .map(|s| (s.skill_id, s.skill_level))
                .collect(),
            shortcuts: player.shortcuts.iter().map(|s| ((s.page, s.slot), *s)).collect(),
            from_database: false,
        }
    }
}

/// Stores into the real database through the repositories (Java's SQL, D-39).
pub struct DbStore {
    characters: CharacterRepository,
    items: ItemRepository,
    ground_items: GroundItemRepository,
    skills: SkillRepository,
    item_reuse: ItemReuseRepository,
    shortcuts: ShortcutRepository,
    reco: RecommendationRepository,
    variables: VariableRepository,
    punishments: PunishmentRepository,
    clans: ClanRepository,
    /// The ids the database gave the punishments this store inserted, by (key, affect, type).
    punishment_ids: HashMap<(String, &'static str, &'static str), u32>,
    baselines: HashMap<ObjectId, Baseline>,
}

impl DbStore {
    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            ground_items: GroundItemRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            item_reuse: ItemReuseRepository::new(pool.clone()),
            shortcuts: ShortcutRepository::new(pool.clone()),
            reco: RecommendationRepository::new(pool.clone()),
            variables: VariableRepository::new(pool.clone()),
            punishments: PunishmentRepository::new(pool.clone()),
            clans: ClanRepository::new(pool),
            punishment_ids: HashMap::new(),
            baselines: HashMap::new(),
        }
    }

    /// Java `Player.storeRecommendations` and `PlayerVariables.saveNowSync`: the Nevit rows the
    /// world changed (M4-VIT-01). Only what the snapshot says it changed is written.
    async fn store_nevit_rows(&self, player: &PlayerData) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        let profile = &player.profile;
        // `REPLACE INTO character_reco_bonus (charId,rec_have,rec_left,time_left)`.
        if profile.reco_store {
            self.reco
                .replace_reco_bonus(
                    char_id,
                    profile.recom_have,
                    profile.recom_left,
                    profile.recom_time_left_ms,
                )
                .await?;
        }
        // The deletes first, then the upserts, both by name (Java sorts them).
        for name in &profile.variables_removed {
            self.variables.delete_variable(char_id, name).await?;
        }
        for name in &profile.variables_changed {
            // Java: `if (value != null)`.
            if let Some(value) = profile.variables.get(name) {
                self.variables.upsert_variable(char_id, name, value).await?;
            }
        }
        Ok(())
    }

    /// Java `Player.storeEffect`: the character's `character_skills_save` rows are deleted, then
    /// one row per saved buff or reuse timer, `buff_index` counting from 1 in the order the world
    /// gave them (`restoreEffects` reads them back in that order). Java runs the batch on one
    /// connection without a transaction, so does this.
    // Java: Player.storeEffect (DELETE_SKILL_SAVE, ADD_SKILL_SAVE)
    async fn store_effects(
        &self,
        char_id: i32,
        class_index: i32,
        effects: &[SavedEffect],
    ) -> Result<(), PersistError> {
        let rows: Vec<NewSkillSave> = effects
            .iter()
            .enumerate()
            .map(|(index, effect)| NewSkillSave {
                skill_id: effect.skill_id,
                skill_level: effect.skill_level,
                remaining_time: effect.remaining_time,
                reuse_delay: effect.reuse_delay,
                systime: effect.systime,
                restore_type: effect.restore_type,
                buff_index: i32::try_from(index + 1).unwrap_or(i32::MAX),
            })
            .collect();
        self.skills.delete_skill_effects(char_id, class_index).await?;
        self.skills.add_skill_effects(char_id, class_index, &rows).await?;
        Ok(())
    }

    /// M5-FIX-03: Java `Player.storeItemReuseDelay` replaces the character's
    /// `character_item_reuse_save` rows with the timers that still run. Java catches a failure and
    /// logs it ("Could not store char item reuse data"), so a failure here does not fail the store.
    // Java: Player.storeItemReuseDelay (DELETE_ITEM_REUSE_SAVE, ADD_ITEM_REUSE_SAVE)
    async fn store_item_reuse(&self, char_id: i32, rows: &[SavedItemReuse]) {
        let rows: Vec<NewItemReuse> = rows
            .iter()
            .map(|r| NewItemReuse {
                item_id: r.item_id,
                item_obj_id: r.item_obj_id,
                reuse_delay: r.reuse_delay,
                systime: r.systime,
            })
            .collect();
        let result = async {
            self.item_reuse.delete_item_reuse(char_id).await?;
            self.item_reuse.add_item_reuse(char_id, &rows).await
        }
        .await;
        if let Err(e) = result {
            warn!("Could not store char item reuse data: {e}");
        }
    }

    /// The baseline of a character that was not stored since the server started: its rows as
    /// they are in the database now.
    async fn load_baseline(&self, char_id: i32, class_index: Option<i32>) -> Result<Baseline, PersistError> {
        let mut baseline = Baseline {
            from_database: true,
            ..Baseline::default()
        };
        for location in ALL_LOCATIONS {
            for row in self.items.items_of_location(char_id, location).await? {
                let item = ItemData {
                    object_id: ObjectId(row.object_id),
                    item_id: row.item_id.unwrap_or(0),
                    count: i64::try_from(row.count).unwrap_or(i64::MAX),
                    enchant_level: row.enchant_level.unwrap_or(0),
                    loc: row.loc.clone().unwrap_or_default(),
                    loc_data: row.loc_data.unwrap_or(0),
                    custom_type1: row.custom_type1.unwrap_or(0),
                    custom_type2: row.custom_type2.unwrap_or(0),
                    mana_left: row.mana_left,
                    time: row.time,
                    ..Default::default()
                };
                baseline.items.insert(row.object_id, item);
            }
        }
        // Java `Item.restoreAttributes`: the augmentation and elements the rows hold now.
        restore_item_attributes(&self.items, char_id, baseline.items.values_mut()).await?;
        if let Some(class_index) = class_index {
            for skill in self.skills.skills_of(char_id, class_index).await? {
                baseline.skills.insert(skill.skill_id, skill.skill_level);
            }
            for row in self.shortcuts.shortcuts_of(char_id, class_index).await? {
                let shortcut = ShortcutData {
                    slot: row.slot,
                    page: row.page,
                    kind: row.kind.unwrap_or(0),
                    shortcut_id: row.shortcut_id.and_then(|v| i32::try_from(v).ok()).unwrap_or(0),
                    level: row.level.unwrap_or(0),
                };
                baseline
                    .shortcuts
                    .insert((shortcut.page, shortcut.slot), shortcut);
            }
        }
        Ok(baseline)
    }

    /// Java `Item.updateInDb` (the row exists, `old` is what was written before) or
    /// `Item.insertIntoDb` (`old` is `None`), then the augmentation and element rows if they
    /// differ from `old` (Java writes them when they are set, and with an insert). `Ok(false)`
    /// when the item is in a location the schema does not know (nothing is written).
    async fn write_item(
        &self,
        char_id: i32,
        item: &ItemData,
        old: Option<&ItemData>,
    ) -> Result<bool, PersistError> {
        let id = item.object_id.0;
        let exists = old.is_some();
        let Some(loc) = ItemLocation::from_db_str(&item.loc) else {
            warn!(
                "Item {id} of character {char_id} has the unknown location {:?}; not stored.",
                item.loc
            );
            return Ok(false);
        };
        if exists {
            self.items
                .update_item(
                    id,
                    &ItemUpdate {
                        owner_id: char_id,
                        count: item.count,
                        loc,
                        loc_data: item.loc_data,
                        enchant_level: item.enchant_level,
                        custom_type1: item.custom_type1,
                        custom_type2: item.custom_type2,
                        mana_left: item.mana_left,
                        time: item.time,
                    },
                )
                .await?;
        } else {
            self.items
                .insert_item(&NewItem {
                    owner_id: char_id,
                    item_id: item.item_id,
                    count: item.count,
                    loc,
                    loc_data: item.loc_data,
                    enchant_level: item.enchant_level,
                    object_id: id,
                    custom_type1: item.custom_type1,
                    custom_type2: item.custom_type2,
                    mana_left: item.mana_left,
                    time: item.time,
                })
                .await?;
        }
        // Java: updateItemAttributes (REPLACE) / removeAugmentation (DELETE), updateItemElements
        // (DELETE all, INSERT each).
        let old_augmentation = old.and_then(|o| o.augmentation);
        if item.augmentation != old_augmentation {
            match item.augmentation {
                Some(augmentation) => self.items.set_augmentation(id, augmentation).await?,
                None => self.items.delete_augmentation(id).await?,
            }
        }
        let rows = element_rows(item);
        if rows != old.map(element_rows).unwrap_or_default() {
            self.items.replace_elementals(id, &rows).await?;
        }
        Ok(true)
    }

    /// The writes of `store_item_changes`; the baseline follows each successful statement.
    async fn apply_item_changes(
        &self,
        char_id: i32,
        upserts: &[ItemData],
        removed: &[ObjectId],
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        for item in upserts {
            let id = item.object_id.0;
            if !self.write_item(char_id, item, baseline.items.get(&id)).await? {
                continue;
            }
            baseline.items.insert(id, item.clone());
        }
        for object_id in removed {
            // Java `removeFromDb` only runs for a row that exists.
            if baseline.items.contains_key(&object_id.0) {
                self.items.delete_item(object_id.0).await?;
                baseline.items.remove(&object_id.0);
            }
        }
        Ok(())
    }

    /// The writes of `store_skill_changes`; the baseline follows each successful statement.
    // Java: Player.storeSkill (UPDATE_CHARACTER_SKILL_LEVEL when the skill was known, else
    // ADD_NEW_SKILLS) and Player.removeSkill (DELETE_SKILL_FROM_CHAR).
    async fn apply_skill_changes(
        &self,
        char_id: i32,
        class_index: i32,
        upserts: &[SkillData],
        removed: &[i32],
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        for SkillData {
            skill_id,
            skill_level,
        } in upserts
        {
            if baseline.skills.contains_key(skill_id) {
                self.skills
                    .update_skill_level(char_id, class_index, *skill_id, *skill_level)
                    .await?;
            } else {
                self.skills
                    .replace_skills(
                        char_id,
                        class_index,
                        &[SkillLevel {
                            skill_id: *skill_id,
                            skill_level: *skill_level,
                        }],
                    )
                    .await?;
            }
            baseline.skills.insert(*skill_id, *skill_level);
        }
        for skill_id in removed {
            self.skills.delete_skill(char_id, class_index, *skill_id).await?;
            baseline.skills.remove(skill_id);
        }
        Ok(())
    }

    /// Writes what differs between `player` and the baseline, and updates the baseline after
    /// every successful statement, so a failure half way leaves it accurate for the next try.
    async fn sync_children(
        &self,
        player: &PlayerData,
        class_index: Option<i32>,
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        // Rows we saw in an earlier snapshot and that are gone now were destroyed in the world.
        let may_delete = !baseline.from_database;

        // Items. Java: Item.insertIntoDb / updateInDb / removeFromDb.
        let now_items: HashSet<i32> = player.items.iter().map(|i| i.object_id.0).collect();
        for item in &player.items {
            let id = item.object_id.0;
            if baseline.items.get(&id) == Some(item) {
                continue;
            }
            if !self.write_item(char_id, item, baseline.items.get(&id)).await? {
                continue;
            }
            baseline.items.insert(id, item.clone());
        }
        if may_delete {
            let gone: Vec<i32> = baseline
                .items
                .keys()
                .copied()
                .filter(|id| !now_items.contains(id))
                .collect();
            for id in gone {
                self.items.delete_item(id).await?;
                baseline.items.remove(&id);
            }
        }

        // Skills and shortcuts belong to the active class index; without one (a subclass is
        // active and M2 does not model subclasses) they are left alone.
        let Some(class_index) = class_index else {
            return Ok(());
        };

        // Java: Player.storeSkill
        let now_skills: HashSet<i32> = player.skills.iter().map(|s| s.skill_id).collect();
        for SkillData {
            skill_id,
            skill_level,
        } in &player.skills
        {
            match baseline.skills.get(skill_id) {
                Some(level) if level == skill_level => continue,
                Some(_) => {
                    self.skills
                        .update_skill_level(char_id, class_index, *skill_id, *skill_level)
                        .await?;
                }
                None => {
                    self.skills
                        .replace_skills(
                            char_id,
                            class_index,
                            &[SkillLevel {
                                skill_id: *skill_id,
                                skill_level: *skill_level,
                            }],
                        )
                        .await?;
                }
            }
            baseline.skills.insert(*skill_id, *skill_level);
        }
        if may_delete {
            let gone: Vec<i32> = baseline
                .skills
                .keys()
                .copied()
                .filter(|id| !now_skills.contains(id))
                .collect();
            for id in gone {
                self.skills.delete_skill(char_id, class_index, id).await?;
                baseline.skills.remove(&id);
            }
        }

        // Java: Shortcuts.registerShortcutInDb / deleteShortcutFromDb
        let now_shortcuts: HashSet<(i32, i32)> = player.shortcuts.iter().map(|s| (s.page, s.slot)).collect();
        for shortcut in &player.shortcuts {
            let key = (shortcut.page, shortcut.slot);
            if baseline.shortcuts.get(&key) == Some(shortcut) {
                continue;
            }
            self.shortcuts
                .replace_shortcut(
                    char_id,
                    class_index,
                    &NewShortcut {
                        slot: shortcut.slot,
                        page: shortcut.page,
                        kind: shortcut.kind,
                        shortcut_id: shortcut.shortcut_id,
                        level: shortcut.level,
                    },
                )
                .await?;
            baseline.shortcuts.insert(key, *shortcut);
        }
        if may_delete {
            let gone: Vec<(i32, i32)> = baseline
                .shortcuts
                .keys()
                .copied()
                .filter(|key| !now_shortcuts.contains(key))
                .collect();
            for (page, slot) in gone {
                self.shortcuts
                    .delete_shortcut(char_id, class_index, slot, page)
                    .await?;
                baseline.shortcuts.remove(&(page, slot));
            }
        }
        Ok(())
    }
}

/// Every place an item of a character can be (the baseline reads them all, so an item that
/// moved between inventory and warehouse is updated, not inserted twice).
const ALL_LOCATIONS: [ItemLocation; 11] = [
    ItemLocation::Void,
    ItemLocation::Inventory,
    ItemLocation::Paperdoll,
    ItemLocation::Warehouse,
    ItemLocation::ClanWh,
    ItemLocation::Pet,
    ItemLocation::PetEquip,
    ItemLocation::Lease,
    ItemLocation::Refund,
    ItemLocation::Mail,
    ItemLocation::Freight,
];

/// The class index of the data being stored: 0 for the base class. A subclass (class id
/// differs from the base class) is not modelled in M2, so its skills and shortcuts are skipped.
fn class_index_of(player: &PlayerData) -> Option<i32> {
    (player.class_id == player.base_class).then_some(0)
}

fn clamp_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// The values of Java's `Player.storeCharBase`: what the snapshot models comes from the
/// snapshot, everything else is the row's current value, written back unchanged.
// Java: Player.storeCharBase (UPDATE_CHARACTER)
pub fn build_character_update(player: &PlayerData, row: &CharacterRow) -> CharacterUpdate {
    // Java stores the base class level, exp and sp in `characters`; the active subclass has its
    // own row in `character_subclasses`. M2 does not model that, so the row keeps its values.
    let base_active = player.class_id == player.base_class;
    // M6-CLAN-01: the clan columns the world keeps (Java `storeCharBase`); a snapshot without
    // them keeps the row's values.
    let clan = player.profile.clan.filter(|c| !c.keep_stored);
    let title = if player.title.is_empty() && row.title.is_none() {
        None
    } else {
        Some(player.title.clone())
    };
    CharacterUpdate {
        level: if base_active {
            player.level
        } else {
            i32::from(row.level.unwrap_or(1))
        },
        // Java getMaxHp() etc. are ints; HP/MP/CP current values are bound as doubles.
        max_hp: player.max_hp as i32,
        cur_hp: player.cur_hp,
        max_cp: player.max_cp as i32,
        cur_cp: player.cur_cp,
        max_mp: player.max_mp as i32,
        cur_mp: player.cur_mp,
        face: player.face,
        hair_style: player.hair_style,
        hair_color: player.hair_color,
        sex: i32::from(player.sex == 1),
        heading: player.heading,
        x: player.x,
        y: player.y,
        z: player.z,
        exp: if base_active {
            player.exp
        } else {
            row.exp.map_or(0, clamp_i64)
        },
        // Java writes `_expBeforeDeath`, which it never reads back (0 after a login).
        exp_before_death: player.profile.exp_before_death,
        sp: if base_active { player.sp } else { clamp_i64(row.sp) },
        karma: player.karma,
        fame: clamp_i32(u64::from(row.fame)),
        pvp_kills: player.pvp_kills,
        pk_kills: player.pk_kills,
        clan_id: clan.map_or_else(
            || row.clanid.map_or(0, |v| clamp_i32(u64::from(v))),
            |c| c.clan_id,
        ),
        race: player.race,
        class_id: player.class_id,
        delete_time: clamp_i64(row.deletetime),
        title,
        // Java writes `getAppearance().getTitleColor()`: the PvP title colours change it.
        title_color: player.profile.title_color,
        access_level: player.access_level,
        // The snapshot is of a player in the world; `PlayerLeft` marks it offline afterwards.
        online: 1,
        is_in_7s_dungeon: row.isin7sdungeon != 0,
        clan_privs: clan.map_or_else(
            || row.clan_privs.map_or(0, |v| clamp_i32(u64::from(v))),
            |c| c.stored_privileges(),
        ),
        wants_peace: clan.map_or_else(|| row.wantspeace.map_or(0, i32::from), |c| c.wants_peace),
        base_class: player.base_class,
        online_time: player.online_time,
        newbie: row.newbie.map_or(0, |v| clamp_i32(u64::from(v))),
        noble: row.nobless != 0,
        power_grade: clan.map_or_else(|| row.power_grade.map_or(0, i32::from), |c| c.power_grade),
        subpledge: clan.map_or_else(|| i32::from(row.subpledge), |c| c.pledge_type),
        lvl_joined_academy: clan.map_or_else(|| i32::from(row.lvl_joined_academy), |c| c.lvl_joined_academy),
        apprentice: clan.map_or_else(|| i64::from(row.apprentice), |c| i64::from(c.apprentice)),
        sponsor: clan.map_or_else(|| i64::from(row.sponsor), |c| i64::from(c.sponsor)),
        clan_join_expiry_time: clan.map_or_else(
            || clamp_i64(row.clan_join_expiry_time),
            |c| c.clan_join_expiry_time,
        ),
        clan_create_expiry_time: clan.map_or_else(
            || clamp_i64(row.clan_create_expiry_time),
            |c| c.clan_create_expiry_time,
        ),
        char_name: player.name.clone(),
        death_penalty_level: i64::from(row.death_penalty_level),
        bookmark_slot: i32::from(row.bookmarkslot),
        // Java stores `(int) _vitalityPoints`. The world keeps the number in the profile; 0 (never
        // a real value, the minimum is 1) means a snapshot that does not carry it.
        vitality_points: if player.profile.vitality_points >= 1 {
            player.profile.vitality_points
        } else {
            i32::from(row.vitality_points)
        },
        language: row.language.clone(),
        faction: i32::from(row.faction),
        pccafe_points: row.pccafe_points,
    }
}

impl PlayerStore for DbStore {
    async fn store_player(&mut self, player: &PlayerData) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        let row = self
            .characters
            .restore_character(char_id)
            .await?
            .ok_or(PersistError::NoSuchCharacter(char_id))?;
        if row.account_name.as_deref() != Some(player.account.as_str()) {
            return Err(PersistError::WrongAccount(char_id, player.account.clone()));
        }
        // Java: Player.store -> storeCharBase
        self.characters
            .update_character(char_id, &build_character_update(player, &row))
            .await?;
        // Java: Player.store -> getVariables().storeMe(); autoSave / deleteMe -> storeRecommendations.
        self.store_nevit_rows(player).await?;
        // Java: Player.store -> storeEffect (the buffs of the character, see `store_effects`).
        if let Some(effects) = &player.profile.effects_to_store {
            self.store_effects(char_id, player.profile.class_index, effects)
                .await?;
        }

        // Java: Player.store -> storeItemReuseDelay.
        if let Some(rows) = &player.profile.item_reuse_to_store {
            self.store_item_reuse(char_id, rows).await;
        }

        // Java: Inventory.updateDatabase, Shortcuts / storeSkill (by difference).
        let class_index = class_index_of(player);
        let mut baseline = match self.baselines.remove(&player.object_id) {
            Some(baseline) => baseline,
            None => self.load_baseline(char_id, class_index).await?,
        };
        let result = self.sync_children(player, class_index, &mut baseline).await;
        if result.is_ok() {
            // From now on the baseline is exactly what we manage: rows that were only in the
            // database (not in the snapshot) drop out and are never deleted.
            baseline = Baseline::of_snapshot(player);
        }
        self.baselines.insert(player.object_id, baseline);
        result
    }

    async fn store_item_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[ItemData],
        removed: &[ObjectId],
    ) -> Result<(), PersistError> {
        let char_id = owner.0;
        // The rows this character is known to have: from its last store, else from the database
        // (once; the baseline then follows every write).
        let mut baseline = match self.baselines.remove(&owner) {
            Some(baseline) => baseline,
            None => {
                let row = self
                    .characters
                    .restore_character(char_id)
                    .await?
                    .ok_or(PersistError::NoSuchCharacter(char_id))?;
                let class_index = (row.classid == Some(row.base_class)).then_some(0);
                self.load_baseline(char_id, class_index).await?
            }
        };
        let result = self
            .apply_item_changes(char_id, upserts, removed, &mut baseline)
            .await;
        self.baselines.insert(owner, baseline);
        result
    }

    async fn store_ground_items(&mut self, rows: &[GroundItemRow]) -> Result<(), PersistError> {
        let records: Vec<GroundItemRecord> = rows
            .iter()
            .map(|row| GroundItemRecord {
                object_id: row.object_id.0,
                item_id: row.item_id,
                count: row.count,
                enchant_level: row.enchant_level,
                x: row.x,
                y: row.y,
                z: row.z,
                drop_time: row.drop_time,
                equipable: row.equipable,
            })
            .collect();
        self.ground_items.replace_all(&records).await?;
        Ok(())
    }

    async fn store_skill_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[SkillData],
        removed: &[i32],
    ) -> Result<(), PersistError> {
        let char_id = owner.0;
        let row = self
            .characters
            .restore_character(char_id)
            .await?
            .ok_or(PersistError::NoSuchCharacter(char_id))?;
        // Skills belong to the active class index; a subclass is not modelled (see `sync_children`).
        let class_index = (row.classid == Some(row.base_class)).then_some(0);
        let Some(class_index) = class_index else {
            return Ok(());
        };
        // The skills this character is known to have: from its last store, else from the
        // database (once; the baseline then follows every write).
        let mut baseline = match self.baselines.remove(&owner) {
            Some(baseline) => baseline,
            None => self.load_baseline(char_id, Some(class_index)).await?,
        };
        let result = self
            .apply_skill_changes(char_id, class_index, upserts, removed, &mut baseline)
            .await;
        self.baselines.insert(owner, baseline);
        result
    }

    async fn store_reco_values(
        &mut self,
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    ) -> Result<(), PersistError> {
        self.reco.replace_reco_values(owner.0, rec_have, rec_left).await?;
        Ok(())
    }

    async fn reset_recommends(&mut self) -> Result<(), PersistError> {
        self.reco.reset_recommends().await?;
        Ok(())
    }

    async fn store_player_variables(
        &mut self,
        owner: ObjectId,
        upserts: &[(String, String)],
        removed: &[String],
    ) -> Result<(), PersistError> {
        // The deletes first, then the upserts (`PlayerVariables.saveNowSync`).
        for name in removed {
            self.variables.delete_variable(owner.0, name).await?;
        }
        for (name, value) in upserts {
            self.variables.upsert_variable(owner.0, name, value).await?;
        }
        Ok(())
    }

    async fn store_punishment(&mut self, punishment: &StoredPunishment) -> Result<(), PersistError> {
        let id = self
            .punishments
            .insert(&NewPunishment {
                key: punishment.key.clone(),
                affect: punishment.affect.name().to_owned(),
                kind: punishment.kind.name().to_owned(),
                expiration: punishment.expiration,
                reason: punishment.reason.clone(),
                punished_by: punishment.punished_by.clone(),
            })
            .await?;
        if let Ok(id) = u32::try_from(id) {
            self.punishment_ids.insert(
                (
                    punishment.key.clone(),
                    punishment.affect.name(),
                    punishment.kind.name(),
                ),
                id,
            );
        }
        Ok(())
    }

    async fn end_punishment(
        &mut self,
        id: i32,
        key: &str,
        affect: PunishmentAffect,
        kind: PunishmentType,
        ended_at: i64,
    ) -> Result<(), PersistError> {
        let known = self
            .punishment_ids
            .remove(&(key.to_owned(), affect.name(), kind.name()));
        let Some(id) = u32::try_from(id).ok().filter(|id| *id > 0).or(known) else {
            // The insert failed (it was logged): there is no row to update.
            warn!(
                "No stored row for the punishment {} of {key}; nothing to end.",
                kind.name()
            );
            return Ok(());
        };
        self.punishments.update_expiration(id, ended_at).await?;
        Ok(())
    }

    async fn change_char_access_level(&mut self, char_id: i32, level: i32) -> Result<(), PersistError> {
        self.characters.update_access_level(char_id, level).await?;
        Ok(())
    }

    async fn store_clan_write(&mut self, write: &ClanWrite) -> Result<(), PersistError> {
        let clans = &self.clans;
        match write {
            ClanWrite::Insert(row) => clans.insert_clan(&new_clan_of(row)).await?,
            ClanWrite::Update(row) => clans.update_clan(&clan_update_of(row)).await?,
            ClanWrite::Level { clan_id, level } => clans.update_clan_level(*clan_id, *level).await?,
            ClanWrite::AllyCrestOfClan { clan_id, crest_id } => {
                clans.update_ally_crest_of_clan(*clan_id, *crest_id).await?
            }
            ClanWrite::RemoveMember {
                char_id,
                clan_join_expiry_time,
                clan_create_expiry_time,
            } => {
                clans
                    .remove_member(*char_id, *clan_join_expiry_time, *clan_create_expiry_time)
                    .await?
            }
            ClanWrite::CharClanPrivs { char_id, privs } => {
                clans.update_char_clan_privs(*char_id, *privs).await?
            }
            ClanWrite::CharSubPledge { char_id, pledge_type } => {
                clans.update_char_subpledge(*char_id, *pledge_type).await?
            }
            ClanWrite::CharPowerGrade { char_id, power_grade } => {
                clans.update_char_power_grade(*char_id, *power_grade).await?
            }
            ClanWrite::CharApprenticeSponsor {
                char_id,
                apprentice,
                sponsor,
            } => {
                clans
                    .update_char_apprentice_sponsor(*char_id, *apprentice, *sponsor)
                    .await?
            }
            ClanWrite::InsertSubPledge {
                clan_id,
                sub_pledge_id,
                name,
                leader_id,
            } => {
                clans
                    .insert_sub_pledge(*clan_id, *sub_pledge_id, name, *leader_id)
                    .await?
            }
            ClanWrite::UpdateSubPledge {
                clan_id,
                sub_pledge_id,
                name,
                leader_id,
            } => {
                clans
                    .update_sub_pledge(*clan_id, *sub_pledge_id, name, *leader_id)
                    .await?
            }
            ClanWrite::RankPrivs { clan_id, rank, privs } => {
                clans.replace_rank_privs(*clan_id, *rank, *privs).await?
            }
            ClanWrite::Notice {
                clan_id,
                notice,
                enabled,
            } => clans.store_notice(*clan_id, notice, *enabled).await?,
            ClanWrite::Delete { clan_id, castle_id } => clans.delete_clan(*clan_id, *castle_id).await?,
            ClanWrite::UpdateSkill {
                clan_id,
                skill_id,
                skill_level,
            } => clans.update_clan_skill(*clan_id, *skill_id, *skill_level).await?,
            ClanWrite::InsertSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
                sub_pledge_id,
            } => {
                clans
                    .insert_clan_skill(*clan_id, *skill_id, *skill_level, skill_name, *sub_pledge_id)
                    .await?
            }
        }
        Ok(())
    }

    async fn player_offline(&mut self, object_id: ObjectId) -> Result<(), PersistError> {
        self.baselines.remove(&object_id);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        // Java: Player.updateOnlineStatus (UPDATE_ONLINE_STATUS: online and lastAccess)
        self.characters
            .update_online_status(object_id.0, 0, now_ms)
            .await?;
        info!("Character {} is offline.", object_id.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Store(String),
        Offline(i32),
        /// Item changes: owner, object ids written, object ids deleted.
        Items(i32, Vec<i32>, Vec<i32>),
        /// The rows of one `itemsonground` save (object ids).
        GroundItems(Vec<i32>),
        /// Skill changes: owner, (skill id, level) written, skill ids deleted.
        Skills(i32, Vec<(i32, i32)>, Vec<i32>),
        /// Recommendation counters: owner, rec_have, rec_left.
        RecoValues(i32, i32, i32),
        /// The daily reset of `character_reco_bonus`.
        ResetRecommends,
        /// Variables: owner, (name, value) written, names deleted.
        Variables(i32, Vec<(String, String)>, Vec<String>),
        /// A stored punishment: key, affect and type names.
        Punishment(String, &'static str, &'static str),
        /// An ended punishment: row id, key, affect and type names, time.
        EndPunishment(i32, String, &'static str, &'static str, i64),
        /// `changeCharAccessLevel`: character, level.
        AccessLevel(i32, i32),
    }

    /// Records the calls; stores can be made to fail, and are slow so ordering bugs show.
    #[derive(Clone, Default)]
    struct Recording {
        calls: Arc<Mutex<Vec<Call>>>,
        /// Names whose stores fail this many more times.
        failures: Arc<Mutex<HashMap<String, u32>>>,
    }

    impl PlayerStore for Recording {
        async fn store_player(&mut self, player: &PlayerData) -> Result<(), PersistError> {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let mut failures = self.failures.lock().unwrap();
            if let Some(left) = failures.get_mut(&player.name).filter(|left| **left > 0) {
                *left -= 1;
                return Err(PersistError::NoSuchCharacter(player.object_id.0));
            }
            self.calls.lock().unwrap().push(Call::Store(player.name.clone()));
            Ok(())
        }

        async fn player_offline(&mut self, object_id: ObjectId) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Offline(object_id.0));
            Ok(())
        }

        async fn store_ground_items(&mut self, rows: &[GroundItemRow]) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::GroundItems(rows.iter().map(|r| r.object_id.0).collect()));
            Ok(())
        }

        async fn store_item_changes(
            &mut self,
            owner: ObjectId,
            upserts: &[ItemData],
            removed: &[ObjectId],
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Items(
                owner.0,
                upserts.iter().map(|i| i.object_id.0).collect(),
                removed.iter().map(|i| i.0).collect(),
            ));
            Ok(())
        }

        async fn store_skill_changes(
            &mut self,
            owner: ObjectId,
            upserts: &[SkillData],
            removed: &[i32],
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Skills(
                owner.0,
                upserts.iter().map(|s| (s.skill_id, s.skill_level)).collect(),
                removed.to_vec(),
            ));
            Ok(())
        }

        async fn store_reco_values(
            &mut self,
            owner: ObjectId,
            rec_have: i32,
            rec_left: i32,
        ) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::RecoValues(owner.0, rec_have, rec_left));
            Ok(())
        }

        async fn reset_recommends(&mut self) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::ResetRecommends);
            Ok(())
        }

        async fn store_player_variables(
            &mut self,
            owner: ObjectId,
            upserts: &[(String, String)],
            removed: &[String],
        ) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Variables(owner.0, upserts.to_vec(), removed.to_vec()));
            Ok(())
        }

        async fn store_punishment(&mut self, punishment: &StoredPunishment) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Punishment(
                punishment.key.clone(),
                punishment.affect.name(),
                punishment.kind.name(),
            ));
            Ok(())
        }

        async fn end_punishment(
            &mut self,
            id: i32,
            key: &str,
            affect: PunishmentAffect,
            kind: PunishmentType,
            ended_at: i64,
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::EndPunishment(
                id,
                key.to_owned(),
                affect.name(),
                kind.name(),
                ended_at,
            ));
            Ok(())
        }

        async fn change_char_access_level(&mut self, char_id: i32, level: i32) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::AccessLevel(char_id, level));
            Ok(())
        }
    }

    fn player(id: i32, name: &str) -> PlayerData {
        PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 1,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 10.0,
            max_hp: 10.0,
            cur_mp: 5.0,
            max_mp: 5.0,
            cur_cp: 0.0,
            max_cp: 5.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        }
    }

    fn store_msg(id: i32, name: &str) -> OutboxMessage {
        OutboxMessage::Persist(PersistCommand::StorePlayer(Box::new(player(id, name))))
    }

    fn left_msg(id: i32) -> OutboxMessage {
        OutboxMessage::Persist(PersistCommand::PlayerLeft {
            account: format!("acc{id}"),
            object_id: ObjectId(id),
            logout: true,
        })
    }

    const NO_WAIT: RetryPolicy = RetryPolicy {
        attempts: 3,
        delay: Duration::ZERO,
    };

    /// Runs `messages` through the task and returns the store calls and the notices.
    async fn run_all(store: Recording, messages: Vec<OutboxMessage>) -> (Vec<Call>, Vec<LsNotice>) {
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, mut notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, LeftAcks::default());
        for message in messages {
            tx.send(message).unwrap();
        }
        drop(tx);
        // The drain: the task ends only after every queued message was handled.
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        let mut notices = Vec::new();
        while let Ok(n) = notice_rx.try_recv() {
            notices.push(n);
        }
        let calls = calls.lock().unwrap().clone();
        (calls, notices)
    }

    #[tokio::test]
    async fn logout_is_announced_only_after_the_store_finished() {
        let (calls, notices) = run_all(Recording::default(), vec![store_msg(1, "Alpha"), left_msg(1)]).await;
        assert_eq!(calls, vec![Call::Store("Alpha".into()), Call::Offline(1)]);
        assert_eq!(
            notices,
            vec![LsNotice::PlayerLogout {
                account: "acc1".into()
            }]
        );
    }

    #[tokio::test]
    async fn the_notice_is_not_sent_while_the_store_is_still_running() {
        let store = Recording::default();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, mut notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let calls = Arc::clone(&store.calls);
        let task = spawn(store, rx, notice_tx, NO_WAIT, LeftAcks::default());
        tx.send(store_msg(1, "Alpha")).unwrap();
        tx.send(left_msg(1)).unwrap();
        // The store sleeps 20 ms: right now nothing may have been announced.
        assert!(notice_rx.try_recv().is_err());
        let notice = tokio::time::timeout(Duration::from_secs(10), notice_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(notice, LsNotice::PlayerLogout { .. }));
        // When the notice arrives the store is already in the log.
        assert_eq!(calls.lock().unwrap().first(), Some(&Call::Store("Alpha".into())));
        drop(tx);
        task.await.unwrap();
    }

    fn item_msg(owner: i32, upserts: &[i32], removed: &[i32]) -> OutboxMessage {
        let item = |object_id: i32| ItemData {
            object_id: ObjectId(object_id),
            item_id: 57,
            count: 1,
            enchant_level: 0,
            loc: "INVENTORY".into(),
            loc_data: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
            ..Default::default()
        };
        OutboxMessage::Persist(PersistCommand::ItemChanges {
            owner: ObjectId(owner),
            upserts: upserts.iter().map(|o| item(*o)).collect(),
            removed: removed.iter().map(|o| ObjectId(*o)).collect(),
        })
    }

    /// `Item.restoreAttributes`: the rows with -1 are the table's "nothing"; elements keep the
    /// order of the rows; an item without rows stays plain.
    #[test]
    fn restored_attribute_rows_follow_javas_rules() {
        use l2_db::{OwnerAugmentationRow, OwnerElementalRow};
        use l2_world::effects::item_extras::ItemElemental;

        let plain = |id: i32| ItemData {
            object_id: ObjectId(id),
            item_id: 57,
            ..ItemData::default()
        };
        let (mut a, mut b, mut c) = (plain(1), plain(2), plain(3));
        let augmentations = [
            OwnerAugmentationRow {
                item_id: 1,
                aug_attributes: 0x0002_0001,
            },
            OwnerAugmentationRow {
                item_id: 2,
                aug_attributes: -1,
            },
        ];
        let elementals = [
            OwnerElementalRow {
                item_id: 1,
                elem_type: 2,
                elem_value: 30,
            },
            OwnerElementalRow {
                item_id: 1,
                elem_type: 4,
                elem_value: 10,
            },
            OwnerElementalRow {
                item_id: 2,
                elem_type: -1,
                elem_value: 5,
            },
            OwnerElementalRow {
                item_id: 2,
                elem_type: 3,
                elem_value: -1,
            },
        ];
        apply_item_attribute_rows(&mut [&mut a, &mut b, &mut c], &augmentations, &elementals);
        assert_eq!(a.augmentation, Some(0x0002_0001));
        assert_eq!(
            a.elementals,
            [
                ItemElemental {
                    element: 2,
                    value: 30
                },
                ItemElemental {
                    element: 4,
                    value: 10
                }
            ]
        );
        assert_eq!((b.augmentation, b.elementals.len()), (None, 0));
        assert_eq!((c.augmentation, c.elementals.len()), (None, 0));
    }

    /// The element rows a store writes do not depend on the order the item holds them in, so a
    /// reordered list alone writes nothing.
    #[test]
    fn element_rows_are_ordered_by_element() {
        let mut item = ItemData::default();
        item.apply_attribute(4, 10);
        item.apply_attribute(1, 20);
        let rows: Vec<_> = element_rows(&item)
            .iter()
            .map(|r| (r.elem_type, r.elem_value))
            .collect();
        assert_eq!(rows, [(1, 20), (4, 10)]);
    }

    #[tokio::test]
    async fn item_changes_are_applied_in_order_between_the_stores() {
        let messages = vec![
            item_msg(1, &[10], &[]),
            store_msg(1, "A1"),
            item_msg(1, &[11, 12], &[10]),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Items(1, vec![10], vec![]),
                Call::Store("A1".into()),
                Call::Items(1, vec![11, 12], vec![10]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn punishment_writes_keep_their_order_and_the_login_server_hears_the_ban() {
        let messages = vec![
            OutboxMessage::Persist(PersistCommand::StorePunishment(StoredPunishment {
                key: "7".into(),
                affect: PunishmentAffect::Character,
                kind: PunishmentType::Ban,
                expiration: 0,
                reason: "r".into(),
                punished_by: "GM".into(),
            })),
            store_msg(7, "Seven"),
            left_msg(7),
            OutboxMessage::Persist(PersistCommand::CharAccessLevel {
                char_id: 7,
                level: -1,
            }),
            OutboxMessage::Ls(LsEvent::AccessLevel {
                account: "acc7".into(),
                level: -1,
            }),
            OutboxMessage::Persist(PersistCommand::EndPunishment {
                id: 0,
                key: "7".into(),
                affect: PunishmentAffect::Character,
                kind: PunishmentType::Ban,
                ended_at: 123,
            }),
        ];
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Punishment("7".into(), "CHARACTER", "BAN"),
                Call::Store("Seven".into()),
                Call::Offline(7),
                Call::AccessLevel(7, -1),
                Call::EndPunishment(0, "7".into(), "CHARACTER", "BAN", 123),
            ]
        );
        assert_eq!(
            notices,
            vec![
                LsNotice::PlayerLogout {
                    account: "acc7".into()
                },
                LsNotice::AccessLevel {
                    account: "acc7".into(),
                    level: -1
                },
            ]
        );
    }

    #[tokio::test]
    async fn skill_changes_are_applied_in_order_between_the_stores() {
        let skills = |owner: i32, upserts: &[(i32, i32)], removed: &[i32]| {
            OutboxMessage::Persist(PersistCommand::SkillChanges {
                owner: ObjectId(owner),
                upserts: upserts
                    .iter()
                    .map(|&(skill_id, skill_level)| SkillData {
                        skill_id,
                        skill_level,
                    })
                    .collect(),
                removed: removed.to_vec(),
            })
        };
        let messages = vec![
            skills(1, &[(239, 1)], &[]),
            store_msg(1, "A1"),
            skills(1, &[(239, 2), (1216, 1)], &[3]),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Skills(1, vec![(239, 1)], vec![]),
                Call::Store("A1".into()),
                Call::Skills(1, vec![(239, 2), (1216, 1)], vec![3]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn recommendation_writes_are_applied_in_order_with_the_stores() {
        let messages = vec![
            OutboxMessage::Persist(PersistCommand::RecoValues {
                owner: ObjectId(1),
                rec_have: 4,
                rec_left: 19,
            }),
            store_msg(1, "A1"),
            OutboxMessage::Persist(PersistCommand::ResetRecommends),
            OutboxMessage::Persist(PersistCommand::PlayerVariables {
                owner: ObjectId(1),
                upserts: vec![("hunting_time".into(), "0".into())],
                removed: vec!["nevit_b".into()],
            }),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::RecoValues(1, 4, 19),
                Call::Store("A1".into()),
                Call::ResetRecommends,
                Call::Variables(
                    1,
                    vec![("hunting_time".into(), "0".into())],
                    vec!["nevit_b".into()]
                ),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn ground_item_saves_are_applied_in_order_with_the_other_writes() {
        let ground = |ids: &[i32]| {
            OutboxMessage::Persist(PersistCommand::SaveGroundItems(
                ids.iter()
                    .map(|id| GroundItemRow {
                        object_id: ObjectId(*id),
                        item_id: 57,
                        count: 1,
                        enchant_level: 0,
                        x: 0,
                        y: 0,
                        z: 0,
                        drop_time: -1,
                        equipable: false,
                    })
                    .collect(),
            ))
        };
        let messages = vec![item_msg(1, &[10], &[]), ground(&[7, 8]), ground(&[]), left_msg(1)];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Items(1, vec![10], vec![]),
                Call::GroundItems(vec![7, 8]),
                Call::GroundItems(vec![]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn stores_of_one_character_keep_their_order() {
        let messages = vec![
            store_msg(1, "A1"),
            store_msg(2, "B1"),
            store_msg(1, "A2"),
            left_msg(2),
            store_msg(1, "A3"),
            left_msg(1),
        ];
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Store("A1".into()),
                Call::Store("B1".into()),
                Call::Store("A2".into()),
                Call::Offline(2),
                Call::Store("A3".into()),
                Call::Offline(1),
            ]
        );
        assert_eq!(notices.len(), 2);
    }

    #[tokio::test]
    async fn login_server_events_keep_their_order_with_the_logouts() {
        let (_, notices) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Ls(LsEvent::PlayerInGame {
                    account: "acc1".into(),
                }),
                store_msg(1, "Alpha"),
                left_msg(1),
                OutboxMessage::Ls(LsEvent::PlayerInGame {
                    account: "acc1".into(),
                }),
            ],
        )
        .await;
        assert_eq!(
            notices,
            vec![
                LsNotice::PlayerInGame {
                    account: "acc1".into()
                },
                LsNotice::PlayerLogout {
                    account: "acc1".into()
                },
                LsNotice::PlayerInGame {
                    account: "acc1".into()
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_failed_store_is_retried_and_the_logout_waits_for_it() {
        let store = Recording::default();
        store.failures.lock().unwrap().insert("Alpha".into(), 2);
        let (calls, notices) = run_all(store, vec![store_msg(1, "Alpha"), left_msg(1)]).await;
        // Two failures, the third try works (3 attempts).
        assert_eq!(calls, vec![Call::Store("Alpha".into()), Call::Offline(1)]);
        assert_eq!(notices.len(), 1);
    }

    #[tokio::test]
    async fn a_store_that_never_works_does_not_block_the_logout_forever() {
        let store = Recording::default();
        store.failures.lock().unwrap().insert("Alpha".into(), 100);
        let (calls, notices) = run_all(
            store,
            vec![
                store_msg(1, "Alpha"),
                left_msg(1),
                store_msg(2, "Beta"),
                left_msg(2),
            ],
        )
        .await;
        // Alpha is lost (logged as an error); Beta is unaffected and the LS hears both logouts.
        assert_eq!(
            calls,
            vec![Call::Offline(1), Call::Store("Beta".into()), Call::Offline(2)]
        );
        assert_eq!(notices.len(), 2);
    }

    #[tokio::test]
    async fn the_task_drains_everything_before_it_ends() {
        let mut messages = Vec::new();
        for id in 1..=20 {
            messages.push(store_msg(id, &format!("P{id}")));
            messages.push(left_msg(id));
        }
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(calls.len(), 40);
        assert_eq!(notices.len(), 20);
    }

    struct NullSink;

    impl l2_world::PacketSink for NullSink {
        fn send(&self, _bytes: Vec<u8>) {}
        fn left_world(&self, _reason: l2_world::LeaveReason) {}
    }

    fn enter(handle: &crate::state::WorldHandle, session: u64, id: i32, name: &str) {
        assert!(handle.send(l2_world::WorldCommand::EnterWorld {
            session: l2_world::SessionId(session),
            player: Box::new(player(id, name)),
            sink: Box::new(NullSink),
            ip: None,
        }));
    }

    fn runtime_with(store: Recording) -> (crate::runtime::WorldRuntime, UnboundedReceiver<LsNotice>) {
        let (notice_tx, notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let runtime = crate::runtime::WorldRuntime::start_with_autosave(
            l2_util::WorldRng::from_seed(5),
            Duration::from_millis(200),
            move |outbox| spawn(store, outbox, notice_tx, NO_WAIT, LeftAcks::default()),
        )
        .unwrap();
        (runtime, notice_rx)
    }

    #[tokio::test]
    async fn a_logout_in_the_world_is_announced_after_its_store() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, mut notices) = runtime_with(store);
        let handle = runtime.handle();
        enter(&handle, 1, 0x6000_0001, "Alpha");
        assert!(handle.send(l2_world::WorldCommand::Input {
            session: l2_world::SessionId(1),
            input: l2_world::ClientInput::Logout,
        }));
        let first = tokio::time::timeout(Duration::from_secs(10), notices.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            first,
            LsNotice::PlayerInGame {
                account: "acc1610612737".into()
            }
        );
        let second = tokio::time::timeout(Duration::from_secs(10), notices.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            second,
            LsNotice::PlayerLogout {
                account: "acc1610612737".into()
            }
        );
        // The store is in the log by the time the logout is announced.
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::Store("Alpha".into()), Call::Offline(0x6000_0001)]
        );
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn autosave_goes_through_the_world_timer_to_the_store() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, _notices) = runtime_with(store);
        let handle = runtime.handle();
        enter(&handle, 1, 0x6000_0001, "Alpha");
        // The interval is 200 ms; wait for at least one autosave without a logout.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while calls.lock().unwrap().is_empty() {
            assert!(tokio::time::Instant::now() < deadline, "no autosave happened");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(calls.lock().unwrap().first(), Some(&Call::Store("Alpha".into())));
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| matches!(c, Call::Offline(_)))
        );
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_returns_only_after_every_player_is_stored_and_announced() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, mut notices) = runtime_with(store);
        let handle = runtime.handle();
        for (session, id, name) in [
            (1, 0x6000_0001, "Alpha"),
            (2, 0x6000_0002, "Beta"),
            (3, 0x6000_0003, "Gamma"),
        ] {
            enter(&handle, session, id, name);
        }
        // Not waiting for anything: the shutdown itself must drain the slow stores.
        tokio::time::timeout(Duration::from_secs(10), runtime.shutdown())
            .await
            .unwrap();
        let calls = calls.lock().unwrap().clone();
        let stores = calls.iter().filter(|c| matches!(c, Call::Store(_))).count();
        let offline = calls.iter().filter(|c| matches!(c, Call::Offline(_))).count();
        assert_eq!((stores, offline), (3, 3));
        let mut logouts = 0;
        while let Ok(notice) = notices.try_recv() {
            if matches!(notice, LsNotice::PlayerLogout { .. }) {
                logouts += 1;
            }
        }
        assert_eq!(logouts, 3);
    }

    #[test]
    fn autosave_interval_follows_general_ini() {
        let dir = tempfile::tempdir().unwrap();
        // Java default without the file or the key: 15 minutes.
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(900));
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 5\n").unwrap();
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(300));
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 0\n").unwrap();
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(1));
    }
}
