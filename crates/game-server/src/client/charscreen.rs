//! The character screen: `NewCharacter`, `CharacterCreate`, `CharacterDelete`, `CharacterRestore`,
//! `CharacterSelect` and `RequestGotoLobby`.
// Java: org/l2jmobius/gameserver/network/clientpackets/{NewCharacter,CharacterCreate,CharacterDelete,
// Java: CharacterRestore,CharacterSelect,RequestGotoLobby}.java
// Java: org/l2jmobius/gameserver/network/GameClient.java (markToDeleteChar, restore, getCharSelection)
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (create)
//
// What a new character is made of is worked out in [`super::newchar`]; this module checks the
// request like Java, stores the rows through a [`CharScreenStore`] (the database in the server,
// a fake in tests) and answers the client.
//
// `ReplyCharacters` (the character count the login server asks for with `RequestCharacters`) is
// answered by the login server link from the same database, so nothing is sent from here: Java
// does not push it after a create or delete either.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use l2_config::GameConfig;
use l2_data::{ClassId, Datapack};
use l2_db::{
    CharacterRepository, ClanRepository, ItemRepository, MacroRepository, Pool, RecommendationRepository,
    ShortcutRepository, SkillRepository,
};
use l2_net::{Control, Encode};
use l2_protocol_game::ConnectionState;
use l2_protocol_game::client::{CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect};
use l2_protocol_game::server::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelected, LeaveWorld,
    NewCharTemplate, NewCharacterSuccess, ServerClose, SsqInfo,
};
use l2_util::time::now_millis;
use l2_util::{ObjectId, WorldRng};
use l2_world::flood::MILLIS_IN_TICK;
use l2_world::{FloodConfig, FloodKind, FloodPunishment, IdFactory};
use tracing::{debug, error, info, warn};

use super::BoxFuture;
use super::newchar::{
    CreateRequest, IdSource, ItemMaxMp, NewCharacterPlan, PlanContext, PlanError, StatBonus, build_plan,
    race_allowed,
};
use super::session::ClientSession;

/// The base classes whose templates the creation screen shows, in Java's order.
const NEW_CHARACTER_CLASSES: [u16; 11] = [0, 10, 18, 25, 31, 38, 44, 49, 53, 123, 124];

/// Milliseconds of a day, the unit of `DeleteCharAfterDays`.
const MILLIS_PER_DAY: i32 = 86_400_000;

/// Why a store call failed (the text is logged, never sent to the client).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StoreError(pub String);

impl From<l2_db::DbError> for StoreError {
    fn from(e: l2_db::DbError) -> Self {
        Self(e.to_string())
    }
}

/// The answer of `GameClient.markToDeleteChar`, apart from its `-1` (an error, [`StoreError`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// 0: not in a clan: the timer was set, or the character was deleted at once.
    Done,
    /// 1: member of a clan.
    ClanMember,
    /// 2: leader of a clan.
    ClanLeader,
}

/// What the character screen reads and writes. [`DbCharScreen`] in the server; a fake in tests.
pub trait CharScreenStore: Send + Sync + 'static {
    /// Java `CharInfoTable.getAccountCharacterCount`: a database error gives 0 (Java logs it).
    fn account_character_count<'a>(&'a self, account: &'a str) -> BoxFuture<'a, i64>;
    /// Java `CharInfoTable.doesCharNameExist`: a database error gives `false` (Java logs it).
    fn name_exists<'a>(&'a self, name: &'a str) -> BoxFuture<'a, bool>;
    /// Writes every row of a new character, in the order of [`NewCharacterPlan`].
    fn insert_character<'a>(&'a self, plan: &'a NewCharacterPlan) -> BoxFuture<'a, Result<(), StoreError>>;
    /// Removes what a failed [`insert_character`](Self::insert_character) left behind.
    fn discard_character<'a>(&'a self, char_id: i32) -> BoxFuture<'a, ()>;
    /// Java `GameClient.markToDeleteChar` after the slot lookup.
    fn mark_to_delete<'a>(
        &'a self,
        char_id: i32,
        delete_days: i32,
        delete_wedding: bool,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<DeleteOutcome, StoreError>>;
    /// Java `GameClient.restore` after the slot lookup.
    fn restore<'a>(&'a self, char_id: i32) -> BoxFuture<'a, Result<(), StoreError>>;
    /// Java `Player.updateOnlineStatus`: `online` and `lastAccess` of the character.
    fn set_online<'a>(
        &'a self,
        char_id: i32,
        online: i32,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<(), StoreError>>;
}

/// [`CharScreenStore`] on the game database.
pub struct DbCharScreen {
    characters: CharacterRepository,
    items: ItemRepository,
    skills: SkillRepository,
    shortcuts: ShortcutRepository,
    macros: MacroRepository,
    clans: ClanRepository,
    reco: RecommendationRepository,
}

impl DbCharScreen {
    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            shortcuts: ShortcutRepository::new(pool.clone()),
            macros: MacroRepository::new(pool.clone()),
            clans: ClanRepository::new(pool.clone()),
            reco: RecommendationRepository::new(pool),
        }
    }

    async fn insert(&self, plan: &NewCharacterPlan) -> Result<(), StoreError> {
        let id = plan.char_id;
        // Player.createDb
        self.characters.insert_character(&plan.insert).await?;
        // Inventory: Item.insertIntoDb
        for item in &plan.items {
            self.items.insert_item(item).await?;
        }
        // Player.addSkill(skill, true) -> storeSkill (class index 0)
        self.skills.replace_skills(id, 0, &plan.skills).await?;
        // registerAllShortcuts: macros first (registerMacro), then each shortcut.
        for m in &plan.macros {
            self.macros.insert_macro(id, m).await?;
        }
        for shortcut in &plan.shortcuts {
            self.shortcuts.replace_shortcut(id, 0, shortcut).await?;
        }
        // Disconnection.storeAndDelete: storeMe (UPDATE_CHARACTER), then deleteMe.
        self.characters.update_character(id, &plan.update).await?;
        self.characters
            .update_online_status(id, 0, plan.last_access)
            .await?;
        let (have, left, time_left) = plan.reco;
        self.reco.replace_reco_bonus(id, have, left, time_left).await?;
        Ok(())
    }

    async fn mark(
        &self,
        char_id: i32,
        delete_days: i32,
        delete_wedding: bool,
        now_ms: i64,
    ) -> Result<DeleteOutcome, StoreError> {
        let clan_id = self.characters.clan_id_of(char_id).await?;
        let mut outcome = DeleteOutcome::Done;
        if clan_id != 0 {
            // ClanTable.getClan(clanId): a clan that does not exist counts as no clan.
            outcome = match self
                .clans
                .leader_id(i32::try_from(clan_id).unwrap_or(i32::MAX))
                .await?
            {
                None => DeleteOutcome::Done,
                Some(leader) if leader == char_id => DeleteOutcome::ClanLeader,
                Some(_) => DeleteOutcome::ClanMember,
            };
        }
        if outcome == DeleteOutcome::Done {
            if delete_days == 0 {
                self.characters.delete_character(char_id, delete_wedding).await?;
            } else {
                // NOTE(java-quirk): `DELETE_DAYS * 86400000` is an int product in Java and
                // overflows from 25 days up.
                let span = i64::from(delete_days.wrapping_mul(MILLIS_PER_DAY));
                self.characters.set_delete_time(char_id, now_ms + span).await?;
            }
        }
        Ok(outcome)
    }
}

impl CharScreenStore for DbCharScreen {
    fn account_character_count<'a>(&'a self, account: &'a str) -> BoxFuture<'a, i64> {
        Box::pin(async move {
            self.characters
                .account_character_count(account)
                .await
                .unwrap_or_else(|e| {
                    warn!("Couldn't retrieve account for id: {e}");
                    0
                })
        })
    }

    fn name_exists<'a>(&'a self, name: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            self.characters.name_exists(name).await.unwrap_or_else(|e| {
                warn!("Could not check existing charname: {e}");
                false
            })
        })
    }

    fn insert_character<'a>(&'a self, plan: &'a NewCharacterPlan) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(self.insert(plan))
    }

    fn discard_character<'a>(&'a self, char_id: i32) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            // Our own half-written character: delete_character removes every table it can be in.
            if let Err(e) = self.characters.delete_character(char_id, false).await {
                error!("Could not clean up the half created character {char_id}: {e}");
            }
        })
    }

    fn mark_to_delete<'a>(
        &'a self,
        char_id: i32,
        delete_days: i32,
        delete_wedding: bool,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<DeleteOutcome, StoreError>> {
        Box::pin(self.mark(char_id, delete_days, delete_wedding, now_ms))
    }

    fn restore<'a>(&'a self, char_id: i32) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move { Ok(self.characters.clear_delete_time(char_id).await?) })
    }

    fn set_online<'a>(
        &'a self,
        char_id: i32,
        online: i32,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            Ok(self
                .characters
                .update_online_status(char_id, online, now_ms)
                .await?)
        })
    }
}

/// The name checks of `CharacterCreate` that need only the config: letters and digits (Java
/// `StringUtil.isAlphaNumeric`), then the `CnameTemplate` pattern (Java
/// `CHARNAME_TEMPLATE_PATTERN.matcher(name).matches()`).
// Java: CharacterCreate.runImpl (the name checks)
pub fn is_valid_char_name(config: &GameConfig, name: &str) -> bool {
    is_alpha_numeric(name) && config.manual.server.charname_template.is_match(name)
}

/// Java `StringUtil.isAlphaNumeric`: not empty and every UTF-16 unit is a letter or a digit.
///
/// NOTE: Java asks `Character.isLetterOrDigit(char)`; Rust's `is_alphabetic`/`is_numeric` agree for
/// the characters players type, and differ only for rare marks and number symbols.
pub fn is_alpha_numeric(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.encode_utf16().all(|unit| {
        // A surrogate half is not a letter on its own in Java.
        char::from_u32(u32::from(unit)).is_some_and(|c| c.is_alphabetic() || c.is_numeric())
    })
}

/// Everything the character screen needs besides the connection.
pub struct CharScreen {
    pub store: Arc<dyn CharScreenStore>,
    pub datapack: Arc<Datapack>,
    pub config: Arc<GameConfig>,
    pub ids: Arc<IdFactory>,
    pub stat_bonus: StatBonus,
    pub item_max_mp: ItemMaxMp,
    /// Names of fake players, lower case (Java `FakePlayerData.getProperName`). NPCs are not
    /// loaded yet, so the set is empty in the server.
    pub reserved_names: HashSet<String>,
    /// The `CharacterSelect` flood protector of `FloodProtector.ini` (a client at the character
    /// screen has no player in the world, so this one is checked here, with the shared
    /// [`FloodAction`] state machine of the world crate).
    pub select_flood: FloodConfig,
    /// The game ticks of 100 ms count from here (`GameTimeTaskManager`).
    pub started: Instant,
    /// Java `synchronized (CharInfoTable.getInstance())`: count, name check and insert are one step.
    pub create_lock: tokio::sync::Mutex<()>,
}

impl CharScreen {
    /// The Java default of `FloodProtectorCharacterSelectInterval`, in ticks of 100 ms.
    pub const DEFAULT_SELECT_INTERVAL_TICKS: i32 = 30;

    pub fn new(
        store: Arc<dyn CharScreenStore>,
        datapack: Arc<Datapack>,
        config: Arc<GameConfig>,
        ids: Arc<IdFactory>,
        select_interval_ticks: i32,
    ) -> Self {
        if config.manual.player.forbidden_names.iter().any(String::is_empty) {
            warn!(
                "ForbiddenNames has an empty entry (the key is missing or ends in a comma): like Java, \
                 every character name is refused."
            );
        }
        let stat_bonus = StatBonus::load(&datapack.root);
        let wanted = ItemMaxMp::wanted_ids(&datapack);
        // Java's code default for `CustomItemsLoad` is false, like `Datapack::load`.
        let item_max_mp = ItemMaxMp::load(&datapack.root, &wanted, false);
        Self {
            store,
            datapack,
            config,
            ids,
            stat_bonus,
            item_max_mp,
            reserved_names: HashSet::new(),
            select_flood: FloodConfig::with_interval(select_interval_ticks),
            started: Instant::now(),
            create_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Replaces the `CharacterSelect` flood protector with the full settings of
    /// `FloodProtector.ini` (the interval alone is what [`new`](Self::new) takes).
    pub fn with_select_flood(mut self, config: FloodConfig) -> Self {
        self.select_flood = config;
        self
    }

    /// `GameTimeTaskManager.getGameTicks()`: ticks of 100 ms.
    fn game_ticks(&self) -> i32 {
        i32::try_from(self.started.elapsed().as_millis() / u128::from(MILLIS_IN_TICK.unsigned_abs()))
            .unwrap_or(i32::MAX)
    }

    /// The templates of `NewCharacter`.
    // Java: NewCharacter.runImpl + NewCharacterSuccess.addChar
    pub fn new_character_templates(&self) -> NewCharacterSuccess {
        new_character_templates(&self.datapack)
    }

    fn plan_context(&self) -> PlanContext<'_> {
        PlanContext {
            datapack: &self.datapack,
            config: &self.config,
            stat_bonus: &self.stat_bonus,
            item_max_mp: &self.item_max_mp,
        }
    }
}

/// The eleven starting templates in Java's order. A missing template is left out (Java keeps the
/// `null` in its list and skips it while writing, which breaks the count the client reads).
// Java: NewCharacter.runImpl
pub fn new_character_templates(datapack: &Datapack) -> NewCharacterSuccess {
    let templates = NEW_CHARACTER_CLASSES
        .iter()
        .filter_map(|&class| {
            let template = datapack.player_templates.get(ClassId(class));
            if template.is_none() {
                warn!("NewCharacter: no template for class {class}.");
            }
            template
        })
        .map(|t| NewCharTemplate {
            race: t.race as i32,
            class_id: i32::from(t.class_id.0),
            base_str: t.base_str,
            base_dex: t.base_dex,
            base_con: t.base_con,
            base_int: t.base_int,
            base_wit: t.base_wit,
            base_men: t.base_men,
        })
        .collect();
    NewCharacterSuccess { templates }
}

/// The checks of `CharacterCreate.runImpl` that need no database. `Err` is the `CharCreateFail`
/// reason.
// Java: CharacterCreate.runImpl (before the synchronized block)
pub fn check_request(screen: &CharScreen, packet: &CharacterCreate, client: &str) -> Result<(), i32> {
    let name = packet.name.as_str();
    let length = name.encode_utf16().count();
    if length == 0 || length > 16 {
        return Err(CharCreateFail::REASON_16_ENG_CHARS);
    }
    // NOTE(java-quirk): with `ForbiddenNames` empty the list is `[""]` and every name contains
    // the empty string, so no name is accepted. The shipped Player.ini lists real names.
    let lower = name.to_lowercase();
    if screen
        .config
        .manual
        .player
        .forbidden_names
        .iter()
        .any(|forbidden| lower.contains(&forbidden.to_lowercase()))
    {
        return Err(CharCreateFail::REASON_INCORRECT_NAME);
    }
    if screen.reserved_names.contains(&lower) {
        return Err(CharCreateFail::REASON_INCORRECT_NAME);
    }
    if !is_valid_char_name(&screen.config, name) {
        return Err(CharCreateFail::REASON_INCORRECT_NAME);
    }
    if !(0..=2).contains(&packet.face) {
        warn!(
            "Character Creation Failure: Character face {} is invalid. Possible client hack. {client}",
            packet.face
        );
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    }
    let max_style = if packet.is_female { 6 } else { 4 };
    if packet.hair_style < 0 || packet.hair_style > max_style {
        warn!(
            "Character Creation Failure: Character hair style {} is invalid. Possible client hack. {client}",
            packet.hair_style
        );
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    }
    if !(0..=3).contains(&packet.hair_color) {
        warn!(
            "Character Creation Failure: Character hair color {} is invalid. Possible client hack. {client}",
            packet.hair_color
        );
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    }
    Ok(())
}

/// Java `CharacterCreate.runImpl` from the `synchronized` block on, until the character exists in
/// the database. Returns the plan that was stored.
// Java: CharacterCreate.runImpl, Player.create, initNewChar
pub async fn create_character(
    screen: &CharScreen,
    account: &str,
    packet: &CharacterCreate,
    rng: &mut WorldRng,
) -> Result<NewCharacterPlan, i32> {
    let _guard = screen.create_lock.lock().await;
    let max = i64::from(screen.config.server.max_characters_number_per_account);
    if screen.store.account_character_count(account).await >= max && max != 0 {
        return Err(CharCreateFail::REASON_TOO_MANY_CHARACTERS);
    }
    if screen.store.name_exists(&packet.name).await {
        return Err(CharCreateFail::REASON_NAME_ALREADY_EXISTS);
    }
    let datapack = &screen.datapack;
    let template = datapack.player_templates.get_by_id(packet.class_id);
    // `PlayerClass.getPlayerClass(classId).level() > 0`: only the classes without a parent can be
    // created. (The class list file cannot tell: its parent 0 is both "none" and the human fighter.)
    let is_base_class = u16::try_from(packet.class_id).is_ok_and(|id| NEW_CHARACTER_CLASSES.contains(&id));
    let Some(template) = template.filter(|_| is_base_class) else {
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    };
    if !race_allowed(&screen.config, template.race) {
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    }

    let allocate = || screen.ids.allocate().ok().map(|id| id.0);
    let mut ids = IdSource::new(&allocate);
    let Some(char_id) = allocate().inspect(|id| ids.taken.push(*id)) else {
        error!("Could not create a character: no object id left.");
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    };
    let request = CreateRequest {
        account,
        name: &packet.name,
        is_female: packet.is_female,
        hair_style: packet.hair_style,
        hair_color: packet.hair_color,
        face: packet.face,
    };
    let release = |taken: &[i32]| {
        for &id in taken {
            screen.ids.release(ObjectId(id));
        }
    };
    let plan = match build_plan(
        &screen.plan_context(),
        &request,
        template,
        char_id,
        &mut ids,
        rng,
        now_millis(),
    ) {
        Ok(plan) => plan,
        Err(e) => {
            match e {
                PlanError::OutOfIds => error!("Could not create a character: no object id left."),
                PlanError::NoStatRow(_) => error!("Could not create a character: {e}"),
            }
            release(&ids.taken);
            return Err(CharCreateFail::REASON_CREATION_FAILED);
        }
    };
    // NOTE(deviation): Java ignores a failed insert after `createDb` and answers CharCreateOk
    // (or, when `createDb` itself fails, dies with a NullPointerException and never answers).
    // A half written character is cleaned up and the client is told the creation failed.
    if let Err(e) = screen.store.insert_character(&plan).await {
        error!("Could not insert char data: {e}");
        screen.store.discard_character(plan.char_id).await;
        release(&plan.object_ids);
        return Err(CharCreateFail::REASON_CREATION_FAILED);
    }
    Ok(plan)
}

/// `GameTimeTaskManager.getGameTime() % (24 * 60)`: the in-game minute of the day.
///
/// NOTE: Java counts from local midnight; this counts from UTC midnight (std has no time zone).
pub fn game_time_minutes(now_ms: i64) -> i32 {
    const TICKS_PER_IG_DAY: i64 = 144_000;
    let ticks = now_ms.rem_euclid(86_400_000) / 100;
    // Java: `(ticks % TICKS_PER_IG_DAY) / MILLIS_IN_TICK`, then `% (24 * 60)` when written.
    i32::try_from(((ticks % TICKS_PER_IG_DAY) / 100) % (24 * 60)).unwrap_or(0)
}

// ---- the packet handlers ---------------------------------------------------------------------

impl ClientSession {
    /// Java `NewCharacter.runImpl`.
    pub(super) fn on_new_character(&mut self, ctl: &mut Control<'_>) {
        ctl.send(self.ctx.charscreen.new_character_templates().to_bytes());
    }

    /// Java `CharacterCreate.runImpl`.
    pub(super) async fn on_character_create(&mut self, packet: CharacterCreate, ctl: &mut Control<'_>) {
        let Some(account) = self.account.clone() else {
            return;
        };
        let screen = Arc::clone(&self.ctx);
        let screen = &screen.charscreen;
        let client = format!("{} ({})", account, self.ip);
        if let Err(reason) = check_request(screen, &packet, &client) {
            ctl.send(CharCreateFail { reason }.to_bytes());
            return;
        }
        match create_character(screen, &account, &packet, &mut self.rng).await {
            Err(reason) => ctl.send(CharCreateFail { reason }.to_bytes()),
            Ok(plan) => {
                info!("Created new character {} for account '{account}'.", plan.char_id);
                ctl.send(CharCreateOk.to_bytes());
                // Java: `client.setCharSelection(new CharSelectionInfo(...).getCharInfo())` loads the
                // list again but does not send it; the client asks with RequestGotoLobby.
                if let Some((characters, _)) = self.load_character_list(None).await {
                    self.characters = characters;
                }
            }
        }
    }

    /// Java `GameClient.getObjectIdForSlot`.
    fn object_id_for_slot(&self, slot: i32) -> Option<i32> {
        let found = usize::try_from(slot)
            .ok()
            .and_then(|s| self.characters.get(s))
            .map(|c| c.entry.object_id);
        if found.is_none() {
            warn!(
                "{} tried to delete Character in slot {slot} but no characters exits at that slot.",
                self.ip
            );
        }
        found
    }

    /// Java `getFloodProtectors().canSelectCharacter()` (`FloodProtectorAction.canPerformAction`
    /// for `CharacterSelect`): false when the request came too soon. The kick punishment closes
    /// the connection.
    // NOTE: the `ban` punishment is the world's `PunishmentManager` (the account ban rows and the
    // login server access level), which a client at the character screen cannot reach: it is
    // logged and skipped. `jail` needs a player, which Java does not have here either.
    async fn can_select_character(&mut self, ctl: &mut Control<'_>) -> bool {
        let screen = Arc::clone(&self.ctx);
        let screen = &screen.charscreen;
        let kind = FloodKind::CharacterSelect;
        let who = self.account.clone().unwrap_or_else(|| self.ip.to_string());
        let check = self.select_flood.check(&screen.select_flood, screen.game_ticks());
        if let Some(ms) = check.log_flooding_ms {
            warn!(
                "{}: {who} called command {} ~{ms} ms after previous command.",
                kind.type_name(),
                kind.type_name()
            );
        }
        if let Some(extra) = check.log_extra {
            warn!(
                "{}: {who} issued {extra} extra requests within ~{} ms.",
                kind.type_name(),
                screen
                    .select_flood
                    .protection_interval
                    .wrapping_mul(MILLIS_IN_TICK)
            );
        }
        if check.punish {
            match &screen.select_flood.punishment_type {
                FloodPunishment::Kick => {
                    warn!("{}: {who} kicked for flooding", kind.type_name());
                    ctl.send_and_close(LeaveWorld.to_bytes());
                }
                FloodPunishment::Ban | FloodPunishment::Jail => warn!(
                    "{}: {who} would be punished for flooding; only kick is supported at the character screen",
                    kind.type_name()
                ),
                FloodPunishment::Unknown(text) => {
                    warn!("FloodProtector: Unknown punishment type configured: {text}");
                }
            }
        }
        check.allowed
    }

    /// Java `CharacterDelete.runImpl`.
    pub(super) async fn on_character_delete(&mut self, packet: CharacterDelete, ctl: &mut Control<'_>) {
        let screen = Arc::clone(&self.ctx);
        let screen = &screen.charscreen;
        if let Some(char_id) = self.object_id_for_slot(packet.char_slot) {
            let delete_days = screen.config.player.delete_days;
            let wedding = screen.config.wedding.allow_wedding;
            match screen
                .store
                .mark_to_delete(char_id, delete_days, wedding, now_millis())
                .await
            {
                Ok(DeleteOutcome::Done) => {
                    // NOTE: the clan member removal of a deleted clan member is not ported (no clans yet).
                    info!("Delete, {char_id}, {}", self.ip);
                    ctl.send(CharDeleteSuccess.to_bytes());
                }
                Ok(DeleteOutcome::ClanMember) => ctl.send(
                    CharDeleteFail {
                        reason: CharDeleteFail::REASON_YOU_MAY_NOT_DELETE_CLAN_MEMBER,
                    }
                    .to_bytes(),
                ),
                Ok(DeleteOutcome::ClanLeader) => ctl.send(
                    CharDeleteFail {
                        reason: CharDeleteFail::REASON_CLAN_LEADERS_MAY_NOT_BE_DELETED,
                    }
                    .to_bytes(),
                ),
                // -1: Java answers nothing but the list.
                Err(e) => error!("Error updating delete time of character: {e}"),
            }
        }
        // `new CharSelectionInfo(account, playOkID1, 0)`: slot 0 is highlighted.
        self.reload_and_send_list(Some(0), ctl).await;
    }

    /// Java `CharacterRestore.runImpl`.
    pub(super) async fn on_character_restore(&mut self, packet: CharacterRestore, ctl: &mut Control<'_>) {
        if !self.can_select_character(ctl).await {
            return;
        }
        let screen = Arc::clone(&self.ctx);
        if let Some(char_id) = self.object_id_for_slot(packet.char_slot) {
            if let Err(e) = screen.charscreen.store.restore(char_id).await {
                error!("Error restoring character: {e}");
            }
            info!("Restore, {char_id}, {}", self.ip);
        }
        self.reload_and_send_list(Some(0), ctl).await;
    }

    /// Java `RequestGotoLobby.runImpl`: the list is sent again, but the slot table of the client
    /// is not replaced.
    pub(super) async fn on_request_goto_lobby(&mut self, ctl: &mut Control<'_>) {
        if let Some((_, list)) = self.load_character_list(None).await {
            ctl.send(list.to_bytes());
        }
    }

    /// Loads the list, sends it and keeps it as the slot table (`setCharSelection`).
    async fn reload_and_send_list(&mut self, active: Option<usize>, ctl: &mut Control<'_>) {
        if let Some((characters, list)) = self.load_character_list(active).await {
            ctl.send(list.to_bytes());
            self.characters = characters;
        }
    }

    /// Java `CharacterSelect.runImpl`: the player is loaded (see [`super::enter`]) and the client
    /// is told; the world takes the character over at `EnterWorld`.
    pub(super) async fn on_character_select(&mut self, packet: CharacterSelect, ctl: &mut Control<'_>) {
        if !self.can_select_character(ctl).await {
            return;
        }
        let Some(key) = self.session_key else {
            return;
        };
        let Some(info) = usize::try_from(packet.char_slot)
            .ok()
            .and_then(|s| self.characters.get(s))
            .cloned()
        else {
            return;
        };
        // NOTE: not ported yet because their systems do not exist: SecondaryAuth, a character that
        // is already in the world (offline traders), dual-box limits and the faction balance. The
        // RESTORE_LOCATION variable is applied by the loader. The "Banned?" check of the
        // punishment manager (character, account, IP) is the world's, at enter world
        // (`World::enter`): the punishments live there.
        if info.access_level < 0 {
            // "Selected character is banned (compatibility with previous versions)."
            ctl.send_and_close(ServerClose.to_bytes());
            return;
        }
        let ctx = Arc::clone(&self.ctx);
        let screen = &ctx.charscreen;
        let Some(account) = self.account.clone() else {
            return;
        };
        // Java: "Disconnect offline trader." The trader of the character is stored and removed
        // before the character is read, so the read sees what the shop sold (M6-FU-14).
        self.release_offline_trader(info.entry.object_id).await;
        // Java: `client.load(slot)`. A character that cannot be loaded is not selected; Java
        // logs it and sends nothing.
        let player = match ctx.loader.load_player(&account, info.entry.object_id).await {
            Ok(player) => player,
            Err(e) => {
                super::enter::log_load_failure(info.entry.object_id, &e);
                return;
            }
        };
        let now = now_millis();
        // Player.load ... cha.setOnlineStatus(true, true)
        if let Err(e) = screen.store.set_online(info.entry.object_id, 1, now).await {
            error!("Failed updating character online status: {e}");
        }
        self.selected = Some(info.entry.object_id);
        let entry = &info.entry;
        let (str, dex, con, int, wit, men) = screen
            .datapack
            .player_templates
            .get_by_id(entry.class_id)
            .map_or((0, 0, 0, 0, 0, 0), |t| {
                (
                    t.base_str, t.base_dex, t.base_con, t.base_int, t.base_wit, t.base_men,
                )
            });
        ctl.send(
            SsqInfo {
                // NOTE: the Seven Signs state is not modelled; 0 is the sky outside the validation period.
                state: 0,
            }
            .to_bytes(),
        );
        self.state = ConnectionState::Entering;
        ctl.send(
            CharSelected {
                name: entry.name.clone(),
                object_id: entry.object_id,
                title: info.title.clone(),
                session_id: key.play_ok1,
                clan_id: entry.clan_id,
                is_female: entry.sex != 0,
                race: entry.race,
                class_id: entry.class_id,
                x: entry.x,
                y: entry.y,
                z: entry.z,
                current_hp: entry.current_hp,
                current_mp: entry.current_mp,
                sp: entry.sp,
                exp: entry.exp,
                level: entry.level,
                karma: entry.karma,
                pk_kills: entry.pk_kills,
                int,
                str,
                con,
                men,
                dex,
                wit,
                game_time: game_time_minutes(now),
            }
            .to_bytes(),
        );
        // Java: `Player.load` queued `sendSkillList()`, which reaches the client right after
        // `CharSelected` (the Java capture shows it there).
        ctl.send(l2_world::enter::skill_list(&player, &*ctx.data).to_bytes());
        // The world takes it over at `EnterWorld`.
        self.pending = Some(Box::new(player));
        debug!("Character {} selected by {}.", entry.object_id, self.ip);
    }

    /// Java `CharacterSelect`: `World.getPlayer(objectId)` is the character's offline trader, if
    /// there is one; `Disconnection.of(player).storeAndDelete()`. The world answers whether it
    /// was one, and the final store is awaited (a bounded wait, like the restart's).
    async fn release_offline_trader(&self, char_id: i32) {
        const WAIT: std::time::Duration = std::time::Duration::from_secs(10);
        let id = ObjectId(char_id);
        // Registered before the command, so the acknowledgement cannot be missed.
        let stored = self.ctx.left_acks.wait_for(id);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = self.ctx.world.send(l2_world::WorldCommand::ReleaseOfflineTrader {
            object_id: id,
            done: Box::new(move |was_trader| {
                let _ = tx.send(was_trader);
            }),
        });
        if !sent {
            self.ctx.left_acks.cancel_wait(id);
            return;
        }
        match tokio::time::timeout(WAIT, rx).await {
            Ok(Ok(true)) => {
                if tokio::time::timeout(WAIT, stored).await.is_err() {
                    warn!("The store of the offline trader {char_id} took too long; continuing.");
                }
            }
            _ => self.ctx.left_acks.cancel_wait(id),
        }
    }

    /// The client left after `CharacterSelect` but before the world took the character over:
    /// Java stores and removes the player, which clears the online flag.
    pub(super) async fn release_selected_character(&mut self) {
        self.pending = None;
        let Some(char_id) = self.selected.take() else {
            return;
        };
        if let Err(e) = self
            .ctx
            .charscreen
            .store
            .set_online(char_id, 0, now_millis())
            .await
        {
            error!("Failed updating character online status: {e}");
        }
    }
}
