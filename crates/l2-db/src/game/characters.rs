//! The `characters` table: everything the character screen, enter world and leave world use.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (INSERT_CHARACTER, UPDATE_CHARACTER,
// Java: RESTORE_CHARACTER, updateOnlineStatus, restore), network/serverpackets/CharSelectionInfo.java,
// Java: network/GameClient.java (markToDeleteChar, restore, deleteCharByObjId),
// Java: data/sql/CharInfoTable.java, LoginServerThread.getCharsOnServer,
// Java: managers/PunishmentManager.changeCharAccessLevel, managers/DatabaseIdManager (online reset)
//
// REFERENCE for the game tables (copy this layout):
// * the SQL text is copied verbatim into a constant, with a `// Java:` comment naming the
//   statement. The Java GS keeps running on the same database (D-39), so what we write must
//   look exactly like what Java writes: same statements, same value types (Java binds HP/CP/MP
//   as `double` into integer columns and lets MySQL round, so do we).
// * one repository method per statement. Methods only talk to the database; decisions
//   (is the delete time over, may this name be used) belong to the callers.
// * row structs mirror the column types and nullability (`Option` for nullable columns); write
//   structs mirror the Java parameter lists, in the same order as the `?` placeholders.
// * `SELECT *` rows ignore columns the struct does not list (`createDate`, see below).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.INSERT_CHARACTER
const INSERT_CHARACTER: &str = "INSERT INTO characters (account_name,charId,char_name,level,maxHp,curHp,maxCp,curCp,maxMp,curMp,face,hairStyle,hairColor,sex,exp,sp,karma,fame,pvpkills,pkkills,clanid,race,classid,deletetime,cancraft,title,title_color,accesslevel,online,isin7sdungeon,clan_privs,wantspeace,base_class,newbie,nobless,power_grade,createDate,lastAccess) values (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)";
// Java: Player.UPDATE_CHARACTER
const UPDATE_CHARACTER: &str = "UPDATE characters SET level=?,maxHp=?,curHp=?,maxCp=?,curCp=?,maxMp=?,curMp=?,face=?,hairStyle=?,hairColor=?,sex=?,heading=?,x=?,y=?,z=?,exp=?,expBeforeDeath=?,sp=?,karma=?,fame=?,pvpkills=?,pkkills=?,clanid=?,race=?,classid=?,deletetime=?,title=?,title_color=?,accesslevel=?,online=?,isin7sdungeon=?,clan_privs=?,wantspeace=?,base_class=?,onlinetime=?,newbie=?,nobless=?,power_grade=?,subpledge=?,lvl_joined_academy=?,apprentice=?,sponsor=?,clan_join_expiry_time=?,clan_create_expiry_time=?,char_name=?,death_penalty_level=?,bookmarkslot=?,vitality_points=?,language=?,faction=?,pccafe_points=? WHERE charId=?";
// Java: Player.RESTORE_CHARACTER
const RESTORE_CHARACTER: &str = "SELECT * FROM characters WHERE charId=?";
// Java: Player.RESTORE_CHARACTER's `createDate` column (S-HAND-02: read as text)
// Java: CharacterBirthday.SELECT_PENDING_BIRTHDAY_GIFTS. The `createDate` column Java also selects is
// not read, so it is left out.
// NOTE(java-quirk): Java binds `System.currentTimeMillis() - 24h` (milliseconds) to `FROM_UNIXTIME(?)`,
// which takes seconds; a value that big gives NULL, so `BETWEEN` is never true and no character is
// found. The statement and the bind are kept as they are.
const SELECT_PENDING_BIRTHDAY_GIFTS: &str = "SELECT charId, char_name, (YEAR(NOW()) - YEAR(createDate)) AS age FROM characters WHERE (YEAR(NOW()) - YEAR(createDate) > 0) AND ((DATE_ADD(createDate, INTERVAL (YEAR(NOW()) - YEAR(createDate)) YEAR)) BETWEEN FROM_UNIXTIME(?) AND NOW())";

const SELECT_CREATE_DATE: &str = "SELECT CAST(createDate AS CHAR) FROM characters WHERE charId=?";
// Java: Player.restore (other characters of the account)
const SELECT_OTHER_CHARACTERS: &str =
    "SELECT charId, char_name FROM characters WHERE account_name=? AND charId<>?";
// Java: Player.updateOnlineStatus
const UPDATE_ONLINE_STATUS: &str = "UPDATE characters SET online=?, lastAccess=? WHERE charId=?";
// Java: DatabaseIdManager.UPDATE_CHARACTER_STATUS_QUERY
const RESET_ALL_ONLINE: &str = "UPDATE characters SET online = 0";
// Java: CharSelectionInfo.loadCharacterSelectInfo
const SELECT_ACCOUNT_CHARACTERS: &str = "SELECT * FROM characters WHERE account_name=? ORDER BY createDate";
// Java: LoginServerThread.getCharsOnServer
const SELECT_ACCOUNT_DELETE_TIMES: &str = "SELECT deletetime FROM characters WHERE account_name=?";
// Java: GameClient.markToDeleteChar
const SELECT_CLAN_ID: &str = "SELECT clanId FROM characters WHERE charId=?";
// Java: GameClient.markToDeleteChar
const UPDATE_DELETE_TIME: &str = "UPDATE characters SET deletetime=? WHERE charId=?";
// Java: GameClient.restore
const CLEAR_DELETE_TIME: &str = "UPDATE characters SET deletetime=0 WHERE charId=?";
// Java: CharInfoTable constructor
const SELECT_ALL_NAMES: &str = "SELECT charId, char_name, accesslevel FROM characters";
// Java: CharInfoTable.getIdByName
const SELECT_ID_BY_NAME: &str = "SELECT charId,accesslevel FROM characters WHERE char_name=?";
// Java: CharInfoTable.getNameById
const SELECT_NAME_BY_ID: &str = "SELECT char_name,accesslevel FROM characters WHERE charId=?";
// Java: CharInfoTable.doesCharNameExist
const COUNT_BY_NAME: &str = "SELECT COUNT(*) as count FROM characters WHERE char_name=?";
// Java: CharInfoTable.getAccountCharacterCount
const COUNT_BY_ACCOUNT: &str = "SELECT COUNT(char_name) as count FROM characters WHERE account_name=?";
// Java: AdminRepairChar.handleRepair
const REPAIR_MOVE: &str = "UPDATE characters SET x=-84318, y=244579, z=-3730 WHERE char_name=?";
const REPAIR_SELECT_ID: &str = "SELECT charId FROM characters where char_name=?";
const REPAIR_SHORTCUTS: &str = "DELETE FROM character_shortcuts WHERE charId=?";
const REPAIR_ITEMS: &str = "UPDATE items SET loc='INVENTORY' WHERE owner_id=?";
// Java: PunishmentManager.changeCharAccessLevel
const UPDATE_ACCESS_LEVEL: &str = "UPDATE characters SET accessLevel=? WHERE charId=?";

/// How many times [`CharacterRepository::delete_character`] binds the character id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Binds {
    /// One `?`.
    One,
    /// Two `?`, both the character id.
    Two,
}

/// The statements of `GameClient.deleteCharByObjId`, in the order Java runs them.
///
/// The order matters for the shared database: the Java GS and ours can delete the same
/// character, and a half finished delete must leave the same rows behind. `characters` goes
/// after every table that references it; `mods_wedding` is separate because Java only runs it
/// when `AllowWedding` is on.
// Java: GameClient.deleteCharByObjId
const DELETE_STATEMENTS: [(&str, Binds); 22] = [
    (
        "DELETE FROM character_contacts WHERE charId=? OR contactId=?",
        Binds::Two,
    ),
    (
        "DELETE FROM character_friends WHERE charId=? OR friendId=?",
        Binds::Two,
    ),
    ("DELETE FROM character_hennas WHERE charId=?", Binds::One),
    ("DELETE FROM character_macroses WHERE charId=?", Binds::One),
    ("DELETE FROM character_quests WHERE charId=?", Binds::One),
    ("DELETE FROM character_recipebook WHERE charId=?", Binds::One),
    ("DELETE FROM character_shortcuts WHERE charId=?", Binds::One),
    ("DELETE FROM character_skills WHERE charId=?", Binds::One),
    ("DELETE FROM character_skills_save WHERE charId=?", Binds::One),
    ("DELETE FROM character_subclasses WHERE charId=?", Binds::One),
    ("DELETE FROM heroes WHERE charId=?", Binds::One),
    ("DELETE FROM olympiad_nobles WHERE charId=?", Binds::One),
    ("DELETE FROM seven_signs WHERE charId=?", Binds::One),
    (
        "DELETE FROM pets WHERE item_obj_id IN (SELECT object_id FROM items WHERE items.owner_id=?)",
        Binds::One,
    ),
    (
        "DELETE FROM item_attributes WHERE itemId IN (SELECT object_id FROM items WHERE items.owner_id=?)",
        Binds::One,
    ),
    ("DELETE FROM items WHERE owner_id=?", Binds::One),
    ("DELETE FROM merchant_lease WHERE player_id=?", Binds::One),
    ("DELETE FROM character_raid_points WHERE charId=?", Binds::One),
    ("DELETE FROM character_reco_bonus WHERE charId=?", Binds::One),
    ("DELETE FROM character_instance_time WHERE charId=?", Binds::One),
    ("DELETE FROM character_variables WHERE charId=?", Binds::One),
    ("DELETE FROM characters WHERE charId=?", Binds::One),
];
/// Only when the wedding mod is enabled (`WeddingConfig.ALLOW_WEDDING`), after the list above.
// Java: GameClient.deleteCharByObjId
const DELETE_WEDDING: &str = "DELETE FROM mods_wedding WHERE player1Id = ? OR player2Id = ?";

/// A calendar date for `characters.createDate` (a `DATE` column).
///
/// Java binds a `Timestamp` of the creation time and MySQL keeps the date part, in the JVM's
/// time zone. The caller decides which zone; [`CalendarDate::from_unix_millis_utc`] is the UTC
/// shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate {
    /// Proleptic Gregorian year.
    pub year: i32,
    /// 1..=12.
    pub month: u8,
    /// 1..=31.
    pub day: u8,
}

impl CalendarDate {
    /// The UTC date of a Unix time in milliseconds.
    pub fn from_unix_millis_utc(millis: i64) -> Self {
        // Days since 1970-01-01 -> civil date (H. Hinnant's days_from_civil, inverted).
        let z = millis.div_euclid(86_400_000) + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let mp = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        // All three are small by construction (month 1..=12, day 1..=31, year within i32 for
        // any i64 millisecond time).
        Self {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    }
}

impl CalendarDate {
    /// Parses the `YYYY-MM-DD` text MySQL gives for a `DATE` (a time part after a space is
    /// ignored). `None` for anything else, including the zero date.
    pub fn parse(text: &str) -> Option<Self> {
        let date = text.split([' ', 'T']).next()?;
        let mut parts = date.split('-');
        let year: i32 = parts.next()?.parse().ok()?;
        let month: u8 = parts.next()?.parse().ok()?;
        let day: u8 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some(Self { year, month, day })
    }
}

impl std::fmt::Display for CalendarDate {
    /// `YYYY-MM-DD`, the text MySQL parses into a `DATE`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A row of `characters` (`SELECT *`), one field per column.
///
/// Types follow the column types (unsigned columns are unsigned). Nullable columns are
/// `Option`; where Java calls `getInt`/`getLong` a NULL reads as 0, so use `unwrap_or(0)`.
///
/// `createDate` (`DATE`) is not listed: decoding a `DATE` needs the `chrono` or `time` feature
/// of sqlx, which is not an approved dependency yet (D-28). Java only reads it for the
/// birthday event. Add the field when that dependency is approved.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CharacterRow {
    /// `account_name`.
    pub account_name: Option<String>,
    /// `charId`: the object id (Java `int`, see [`CharacterRow::object_id`]).
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `char_name`.
    pub char_name: String,
    /// `level`.
    pub level: Option<u8>,
    /// `maxHp`.
    #[sqlx(rename = "maxHp")]
    pub max_hp: Option<u32>,
    /// `curHp` (Java reads it as `double`).
    #[sqlx(rename = "curHp")]
    pub cur_hp: Option<u32>,
    /// `maxCp`.
    #[sqlx(rename = "maxCp")]
    pub max_cp: Option<u32>,
    /// `curCp`.
    #[sqlx(rename = "curCp")]
    pub cur_cp: Option<u32>,
    /// `maxMp`.
    #[sqlx(rename = "maxMp")]
    pub max_mp: Option<u32>,
    /// `curMp`.
    #[sqlx(rename = "curMp")]
    pub cur_mp: Option<u32>,
    /// `face`.
    pub face: Option<u8>,
    /// `hairStyle`.
    #[sqlx(rename = "hairStyle")]
    pub hair_style: Option<u8>,
    /// `hairColor`.
    #[sqlx(rename = "hairColor")]
    pub hair_color: Option<u8>,
    /// `sex`: 0 male, 1 female.
    pub sex: Option<u8>,
    /// `heading`.
    pub heading: Option<i32>,
    /// `x`.
    pub x: Option<i32>,
    /// `y`.
    pub y: Option<i32>,
    /// `z`.
    pub z: Option<i32>,
    /// `exp`.
    pub exp: Option<u64>,
    /// `expBeforeDeath`.
    #[sqlx(rename = "expBeforeDeath")]
    pub exp_before_death: Option<u64>,
    /// `sp`.
    pub sp: u64,
    /// `karma`.
    pub karma: Option<u32>,
    /// `fame`.
    pub fame: u32,
    /// `pvpkills`.
    pub pvpkills: Option<u16>,
    /// `pkkills`.
    pub pkkills: Option<u16>,
    /// `clanid`.
    pub clanid: Option<u32>,
    /// `race`: the `Race` ordinal.
    pub race: Option<u8>,
    /// `classid`: the active class.
    pub classid: Option<u8>,
    /// `base_class`.
    pub base_class: u8,
    /// `transform_id`.
    pub transform_id: u16,
    /// `deletetime`: millisecond time the delete becomes final, 0 when not marked.
    pub deletetime: u64,
    /// `cancraft`.
    pub cancraft: Option<u8>,
    /// `title`.
    pub title: Option<String>,
    /// `title_color`.
    pub title_color: u32,
    /// `accesslevel`.
    pub accesslevel: Option<i32>,
    /// `online`.
    pub online: Option<u8>,
    /// `onlinetime`: seconds.
    pub onlinetime: Option<i32>,
    /// `char_slot`.
    pub char_slot: Option<u8>,
    /// `newbie`.
    pub newbie: Option<u32>,
    /// `lastAccess`: millisecond time of the last login/logout.
    #[sqlx(rename = "lastAccess")]
    pub last_access: u64,
    /// `clan_privs`.
    pub clan_privs: Option<u32>,
    /// `wantspeace`.
    pub wantspeace: Option<u8>,
    /// `isin7sdungeon`.
    pub isin7sdungeon: u8,
    /// `power_grade`.
    pub power_grade: Option<u8>,
    /// `nobless`.
    pub nobless: u8,
    /// `subpledge`.
    pub subpledge: i16,
    /// `lvl_joined_academy`.
    pub lvl_joined_academy: u8,
    /// `apprentice`.
    pub apprentice: u32,
    /// `sponsor`.
    pub sponsor: u32,
    /// `clan_join_expiry_time`.
    pub clan_join_expiry_time: u64,
    /// `clan_create_expiry_time`.
    pub clan_create_expiry_time: u64,
    /// `death_penalty_level`.
    pub death_penalty_level: u16,
    /// `bookmarkslot`.
    pub bookmarkslot: u16,
    /// `vitality_points`.
    pub vitality_points: u16,
    /// `language`: two letters, NULL until the first store.
    pub language: Option<String>,
    /// `faction`: 0 none, 1 good, 2 evil.
    pub faction: u8,
    /// `pccafe_points`.
    pub pccafe_points: i32,
}

impl CharacterRow {
    /// The object id as Java's `int`. Ids stay below 2^31 (the Rust id range is part of D-39).
    pub fn object_id(&self) -> i32 {
        self.char_id as i32
    }
}

/// `charId` and `char_name`, e.g. the other characters of an account.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CharacterIdName {
    /// `charId`.
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `char_name`.
    pub char_name: String,
}

/// A row of the startup name table (`charId`, `char_name`, `accesslevel`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CharacterNameAccess {
    /// `charId`.
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    /// `char_name`.
    pub char_name: String,
    /// `accesslevel`; Java's `getInt` turns NULL into 0.
    pub accesslevel: Option<i32>,
}

/// The values of a new character, in the order of `INSERT_CHARACTER`'s columns.
///
/// Types are Java's parameter types. HP/CP/MP are `f64` because Java binds `getCurrentHp()` etc.
/// as `double` into integer columns.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCharacter {
    /// `account_name`.
    pub account_name: String,
    /// `charId`.
    pub char_id: i32,
    /// `char_name`.
    pub char_name: String,
    /// `level`.
    pub level: i32,
    /// `maxHp`.
    pub max_hp: i32,
    /// `curHp`.
    pub cur_hp: f64,
    /// `maxCp`.
    pub max_cp: i32,
    /// `curCp`.
    pub cur_cp: f64,
    /// `maxMp`.
    pub max_mp: i32,
    /// `curMp`.
    pub cur_mp: f64,
    /// `face`.
    pub face: i32,
    /// `hairStyle`.
    pub hair_style: i32,
    /// `hairColor`.
    pub hair_color: i32,
    /// `sex`: 1 for female, else 0.
    pub sex: i32,
    /// `exp`.
    pub exp: i64,
    /// `sp`.
    pub sp: i64,
    /// `karma`.
    pub karma: i32,
    /// `fame`.
    pub fame: i32,
    /// `pvpkills`.
    pub pvp_kills: i32,
    /// `pkkills`.
    pub pk_kills: i32,
    /// `clanid`.
    pub clan_id: i32,
    /// `race`: the `Race` ordinal.
    pub race: i32,
    /// `classid`.
    pub class_id: i32,
    /// `deletetime`.
    pub delete_time: i64,
    /// `cancraft`: Java stores 1 when the class has dwarven craft.
    pub can_craft: bool,
    /// `title`.
    pub title: Option<String>,
    /// `title_color`.
    pub title_color: i32,
    /// `accesslevel`.
    pub access_level: i32,
    /// `online`.
    pub online: i32,
    /// `isin7sdungeon`.
    pub is_in_7s_dungeon: bool,
    /// `clan_privs`.
    pub clan_privs: i32,
    /// `wantspeace`.
    pub wants_peace: i32,
    /// `base_class`.
    pub base_class: i32,
    /// `newbie`.
    pub newbie: i32,
    /// `nobless`.
    pub noble: bool,
    /// `createDate`.
    pub create_date: CalendarDate,
    /// `lastAccess`: millisecond time.
    pub last_access: i64,
}

/// The values `Player.storeCharBase` writes, in the order of `UPDATE_CHARACTER`'s `SET` list.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterUpdate {
    /// `level` (the base class level).
    pub level: i32,
    /// `maxHp`.
    pub max_hp: i32,
    /// `curHp`.
    pub cur_hp: f64,
    /// `maxCp`.
    pub max_cp: i32,
    /// `curCp`.
    pub cur_cp: f64,
    /// `maxMp`.
    pub max_mp: i32,
    /// `curMp`.
    pub cur_mp: f64,
    /// `face`.
    pub face: i32,
    /// `hairStyle`.
    pub hair_style: i32,
    /// `hairColor`.
    pub hair_color: i32,
    /// `sex`: 1 for female, else 0.
    pub sex: i32,
    /// `heading`.
    pub heading: i32,
    /// `x` (the last location while observing).
    pub x: i32,
    /// `y`.
    pub y: i32,
    /// `z`.
    pub z: i32,
    /// `exp` (the base class exp).
    pub exp: i64,
    /// `expBeforeDeath`.
    pub exp_before_death: i64,
    /// `sp` (the base class sp).
    pub sp: i64,
    /// `karma`.
    pub karma: i32,
    /// `fame`.
    pub fame: i32,
    /// `pvpkills`.
    pub pvp_kills: i32,
    /// `pkkills`.
    pub pk_kills: i32,
    /// `clanid`.
    pub clan_id: i32,
    /// `race`.
    pub race: i32,
    /// `classid` (the active class).
    pub class_id: i32,
    /// `deletetime`.
    pub delete_time: i64,
    /// `title`.
    pub title: Option<String>,
    /// `title_color`.
    pub title_color: i32,
    /// `accesslevel`.
    pub access_level: i32,
    /// `online`.
    pub online: i32,
    /// `isin7sdungeon`.
    pub is_in_7s_dungeon: bool,
    /// `clan_privs`.
    pub clan_privs: i32,
    /// `wantspeace`.
    pub wants_peace: i32,
    /// `base_class`.
    pub base_class: i32,
    /// `onlinetime` in seconds, already including the current session.
    pub online_time: i64,
    /// `newbie`.
    pub newbie: i32,
    /// `nobless`.
    pub noble: bool,
    /// `power_grade`.
    pub power_grade: i32,
    /// `subpledge`.
    pub subpledge: i32,
    /// `lvl_joined_academy`.
    pub lvl_joined_academy: i32,
    /// `apprentice`.
    pub apprentice: i64,
    /// `sponsor`.
    pub sponsor: i64,
    /// `clan_join_expiry_time`.
    pub clan_join_expiry_time: i64,
    /// `clan_create_expiry_time`.
    pub clan_create_expiry_time: i64,
    /// `char_name`.
    pub char_name: String,
    /// `death_penalty_level`.
    pub death_penalty_level: i64,
    /// `bookmarkslot`.
    pub bookmark_slot: i32,
    /// `vitality_points`.
    pub vitality_points: i32,
    /// `language`.
    pub language: Option<String>,
    /// `faction`: 0 none, 1 good, 2 evil.
    pub faction: i32,
    /// `pccafe_points`.
    pub pccafe_points: i32,
}

/// Every query on `characters` that M2 needs.
#[derive(Debug, Clone)]
pub struct CharacterRepository {
    pool: Pool,
}

impl CharacterRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Inserts a freshly created character. Fails on a duplicate `charId` (primary key).
    // Java: Player.createDb (INSERT_CHARACTER)
    pub async fn insert_character(&self, c: &NewCharacter) -> Result<()> {
        sqlx::query(INSERT_CHARACTER)
            .bind(&c.account_name)
            .bind(c.char_id)
            .bind(&c.char_name)
            .bind(c.level)
            .bind(c.max_hp)
            .bind(c.cur_hp)
            .bind(c.max_cp)
            .bind(c.cur_cp)
            .bind(c.max_mp)
            .bind(c.cur_mp)
            .bind(c.face)
            .bind(c.hair_style)
            .bind(c.hair_color)
            .bind(c.sex)
            .bind(c.exp)
            .bind(c.sp)
            .bind(c.karma)
            .bind(c.fame)
            .bind(c.pvp_kills)
            .bind(c.pk_kills)
            .bind(c.clan_id)
            .bind(c.race)
            .bind(c.class_id)
            .bind(c.delete_time)
            .bind(i32::from(c.can_craft))
            .bind(c.title.as_deref())
            .bind(c.title_color)
            .bind(c.access_level)
            .bind(c.online)
            .bind(i32::from(c.is_in_7s_dungeon))
            .bind(c.clan_privs)
            .bind(c.wants_peace)
            .bind(c.base_class)
            .bind(c.newbie)
            .bind(i32::from(c.noble))
            // NOTE(java-quirk): power_grade is the literal `setLong(36, 0)` in createDb.
            .bind(0_i64)
            .bind(c.create_date.to_string())
            .bind(c.last_access)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Writes the character's base data (leave world, periodic store).
    // Java: Player.storeCharBase (UPDATE_CHARACTER)
    pub async fn update_character(&self, char_id: i32, c: &CharacterUpdate) -> Result<()> {
        sqlx::query(UPDATE_CHARACTER)
            .bind(c.level)
            .bind(c.max_hp)
            .bind(c.cur_hp)
            .bind(c.max_cp)
            .bind(c.cur_cp)
            .bind(c.max_mp)
            .bind(c.cur_mp)
            .bind(c.face)
            .bind(c.hair_style)
            .bind(c.hair_color)
            .bind(c.sex)
            .bind(c.heading)
            .bind(c.x)
            .bind(c.y)
            .bind(c.z)
            .bind(c.exp)
            .bind(c.exp_before_death)
            .bind(c.sp)
            .bind(c.karma)
            .bind(c.fame)
            .bind(c.pvp_kills)
            .bind(c.pk_kills)
            .bind(c.clan_id)
            .bind(c.race)
            .bind(c.class_id)
            .bind(c.delete_time)
            .bind(c.title.as_deref())
            .bind(c.title_color)
            .bind(c.access_level)
            .bind(c.online)
            .bind(i32::from(c.is_in_7s_dungeon))
            .bind(c.clan_privs)
            .bind(c.wants_peace)
            .bind(c.base_class)
            .bind(c.online_time)
            .bind(c.newbie)
            .bind(i32::from(c.noble))
            .bind(c.power_grade)
            .bind(c.subpledge)
            .bind(c.lvl_joined_academy)
            .bind(c.apprentice)
            .bind(c.sponsor)
            .bind(c.clan_join_expiry_time)
            .bind(c.clan_create_expiry_time)
            .bind(&c.char_name)
            .bind(c.death_penalty_level)
            .bind(c.bookmark_slot)
            .bind(c.vitality_points)
            .bind(c.language.as_deref())
            .bind(c.faction)
            .bind(c.pccafe_points)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The character with this id, or `None`.
    // Java: Player.restore (RESTORE_CHARACTER)
    pub async fn restore_character(&self, char_id: i32) -> Result<Option<CharacterRow>> {
        let row = sqlx::query_as::<_, CharacterRow>(RESTORE_CHARACTER)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Id and name of the account's other characters (Java keeps them in `Player._chars`).
    // Java: Player.restore
    pub async fn other_characters(&self, account_name: &str, char_id: i32) -> Result<Vec<CharacterIdName>> {
        let rows = sqlx::query_as::<_, CharacterIdName>(SELECT_OTHER_CHARACTERS)
            .bind(account_name)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Sets `online` and `lastAccess` (enter and leave world). `now_ms` is the current time.
    // Java: Player.updateOnlineStatus
    pub async fn update_online_status(&self, char_id: i32, online: i32, now_ms: i64) -> Result<()> {
        sqlx::query(UPDATE_ONLINE_STATUS)
            .bind(online)
            .bind(now_ms)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Marks every character offline (game server startup).
    // Java: DatabaseIdManager.UPDATE_CHARACTER_STATUS_QUERY
    pub async fn reset_all_online(&self) -> Result<()> {
        sqlx::query(RESET_ALL_ONLINE).execute(&self.pool).await?;
        Ok(())
    }

    /// The account's characters for the character selection screen, oldest first.
    ///
    /// Rows past their delete time are returned as is; deleting them (and leaving the clan) is
    /// the caller's job, like `CharSelectionInfo.restoreChar` does.
    // Java: CharSelectionInfo.loadCharacterSelectInfo
    pub async fn characters_of_account(&self, account_name: &str) -> Result<Vec<CharacterRow>> {
        let rows = sqlx::query_as::<_, CharacterRow>(SELECT_ACCOUNT_CHARACTERS)
            .bind(account_name)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// `deletetime` of each character of the account (0 = not marked); the login server wants
    /// the character count and the non-zero times.
    // Java: LoginServerThread.getCharsOnServer
    pub async fn delete_times_of_account(&self, account_name: &str) -> Result<Vec<u64>> {
        let times = sqlx::query_scalar::<_, u64>(SELECT_ACCOUNT_DELETE_TIMES)
            .bind(account_name)
            .fetch_all(&self.pool)
            .await?;
        Ok(times)
    }

    /// `characters.createDate` of a character (a `DATE`, read as text because decoding a `DATE`
    /// needs a dependency D-28 does not allow yet). `None` when the row is missing or the
    /// column is NULL or not a date.
    // Java: Player.restore (`getCreateDate().setTimeInMillis(rset.getTimestamp("createDate")...)`)
    pub async fn create_date_of(&self, char_id: i32) -> Result<Option<CalendarDate>> {
        let text = sqlx::query_scalar::<_, Option<String>>(SELECT_CREATE_DATE)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(text.flatten().and_then(|t| CalendarDate::parse(&t)))
    }

    /// The character's clan id; 0 when the row is missing or the column is NULL, like
    /// `rs.next() ? rs.getInt(1) : 0`.
    // Java: GameClient.markToDeleteChar
    pub async fn clan_id_of(&self, char_id: i32) -> Result<u32> {
        let clan_id = sqlx::query_scalar::<_, Option<u32>>(SELECT_CLAN_ID)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(clan_id.flatten().unwrap_or(0))
    }

    /// Marks the character for deletion at `delete_time_ms` (now + `DeleteCharAfterDays`).
    // Java: GameClient.markToDeleteChar
    pub async fn set_delete_time(&self, char_id: i32, delete_time_ms: i64) -> Result<()> {
        sqlx::query(UPDATE_DELETE_TIME)
            .bind(delete_time_ms)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Takes the deletion mark back.
    // Java: GameClient.restore
    pub async fn clear_delete_time(&self, char_id: i32) -> Result<()> {
        sqlx::query(CLEAR_DELETE_TIME)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The characters whose birthday came since `since`, as `(charId, char_name, age)`. `since` is
    /// bound as Java binds it (see [`SELECT_PENDING_BIRTHDAY_GIFTS`]).
    // Java: CharacterBirthday.onDailyReset
    pub async fn pending_birthday_gifts(&self, since: i64) -> Result<Vec<(i32, String, i32)>> {
        let rows: Vec<(i32, String, i64)> = sqlx::query_as(SELECT_PENDING_BIRTHDAY_GIFTS)
            .bind(since)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(char_id, name, age)| (char_id, name, i32::try_from(age).unwrap_or(i32::MAX)))
            .collect())
    }

    /// Deletes the character and every row that belongs to it, statement by statement in
    /// Java's order. `delete_wedding` is `WeddingConfig.ALLOW_WEDDING`.
    ///
    /// Not a transaction, like Java: each statement commits on its own and the first error
    /// stops the rest, so a failure half way leaves the same rows Java would leave. A negative
    /// id does nothing (Java returns early). Leaving the in-memory name table is the caller's job.
    // Java: GameClient.deleteCharByObjId
    pub async fn delete_character(&self, char_id: i32, delete_wedding: bool) -> Result<()> {
        if char_id < 0 {
            return Ok(());
        }
        // Java runs everything on one connection.
        let mut conn = self.pool.acquire().await?;
        for (sql, binds) in DELETE_STATEMENTS {
            let query = sqlx::query(sql).bind(char_id);
            let query = if binds == Binds::Two {
                query.bind(char_id)
            } else {
                query
            };
            query.execute(&mut *conn).await?;
        }
        if delete_wedding {
            sqlx::query(DELETE_WEDDING)
                .bind(char_id)
                .bind(char_id)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }

    /// Id, name and access level of every character (loaded once at startup).
    // Java: CharInfoTable constructor
    pub async fn all_names(&self) -> Result<Vec<CharacterNameAccess>> {
        let rows = sqlx::query_as::<_, CharacterNameAccess>(SELECT_ALL_NAMES)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Id and access level of the character with this name (case-insensitive: the column
    /// collation is `utf8mb4_unicode_ci`), or `None`.
    // Java: CharInfoTable.getIdByName
    pub async fn id_by_name(&self, name: &str) -> Result<Option<(u32, i32)>> {
        let rows = sqlx::query_as::<_, (u32, Option<i32>)>(SELECT_ID_BY_NAME)
            .bind(name)
            .fetch_all(&self.pool)
            .await?;
        // NOTE(java-quirk): Java loops `while (rs.next())` and keeps the last row. Names are
        // not unique in the schema (plain KEY), so the last matching row wins.
        Ok(rows.last().map(|&(id, access)| (id, access.unwrap_or(0))))
    }

    /// Name and access level of the character with this id, or `None`.
    // Java: CharInfoTable.getNameById
    pub async fn name_by_id(&self, char_id: i32) -> Result<Option<(String, i32)>> {
        let row = sqlx::query_as::<_, (String, Option<i32>)>(SELECT_NAME_BY_ID)
            .bind(char_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(name, access)| (name, access.unwrap_or(0))))
    }

    /// Whether a character with this name exists (case-insensitive, see [`Self::id_by_name`]).
    // Java: CharInfoTable.doesCharNameExist
    pub async fn name_exists(&self, name: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(COUNT_BY_NAME)
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    /// How many characters the account has.
    // Java: CharInfoTable.getAccountCharacterCount
    pub async fn account_character_count(&self, account_name: &str) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(COUNT_BY_ACCOUNT)
            .bind(account_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// `//repair` and `//restore`: the character stands in Talking Island again, loses its
    /// shortcuts and every item goes back to the inventory. Nothing happens past the first
    /// statement for a name that has no character.
    // Java: AdminRepairChar.handleRepair
    pub async fn repair_character(&self, name: &str) -> Result<()> {
        sqlx::query(REPAIR_MOVE).bind(name).execute(&self.pool).await?;
        let char_id = sqlx::query_scalar::<_, i32>(REPAIR_SELECT_ID)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        // `objId == 0` in Java: no row.
        let Some(char_id) = char_id.filter(|id| *id != 0) else {
            return Ok(());
        };
        sqlx::query(REPAIR_SHORTCUTS)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(REPAIR_ITEMS)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets the character's `accesslevel` (ban/unban through the punishment manager).
    // Java: PunishmentManager.changeCharAccessLevel
    pub async fn update_access_level(&self, char_id: i32, access_level: i32) -> Result<()> {
        // NOTE(java-quirk): Java binds the id with `setString`; MySQL converts it back to the
        // number, so binding the integer has the same effect.
        sqlx::query(UPDATE_ACCESS_LEVEL)
            .bind(access_level)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn a_date_text_is_parsed() {
        let date = |y, m, d| {
            Some(CalendarDate {
                year: y,
                month: m,
                day: d,
            })
        };
        assert_eq!(CalendarDate::parse("2024-03-07"), date(2024, 3, 7));
        assert_eq!(CalendarDate::parse("2024-03-07 00:00:00"), date(2024, 3, 7));
        assert_eq!(CalendarDate::parse("0000-00-00"), None);
        assert_eq!(CalendarDate::parse("2024-13-01"), None);
        assert_eq!(CalendarDate::parse("nonsense"), None);
        assert_eq!(placeholders(SELECT_CREATE_DATE), 1);
    }

    // The statements are copied from Java; these checks catch a slip while copying.
    #[test]
    fn insert_has_one_placeholder_per_column() {
        let columns = INSERT_CHARACTER
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(cols, _)| cols.split(',').count())
            .unwrap();
        assert_eq!(columns, 38);
        assert_eq!(placeholders(INSERT_CHARACTER), 38);
    }

    #[test]
    fn update_has_51_set_columns_and_the_id() {
        let set = UPDATE_CHARACTER
            .split_once(" SET ")
            .and_then(|(_, rest)| rest.split_once(" WHERE "))
            .map(|(set, _)| set.split(',').count())
            .unwrap();
        assert_eq!(set, 51);
        assert_eq!(placeholders(UPDATE_CHARACTER), 52);
        assert!(UPDATE_CHARACTER.ends_with("WHERE charId=?"));
    }

    #[test]
    fn delete_statements_keep_javas_order_and_binds() {
        let tables: Vec<&str> = DELETE_STATEMENTS
            .iter()
            .map(|(sql, _)| {
                sql.strip_prefix("DELETE FROM ")
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            tables,
            [
                "character_contacts",
                "character_friends",
                "character_hennas",
                "character_macroses",
                "character_quests",
                "character_recipebook",
                "character_shortcuts",
                "character_skills",
                "character_skills_save",
                "character_subclasses",
                "heroes",
                "olympiad_nobles",
                "seven_signs",
                "pets",
                "item_attributes",
                "items",
                "merchant_lease",
                "character_raid_points",
                "character_reco_bonus",
                "character_instance_time",
                "character_variables",
                "characters",
            ]
        );
        for (sql, binds) in DELETE_STATEMENTS {
            let expected = if binds == Binds::Two { 2 } else { 1 };
            assert_eq!(placeholders(sql), expected, "{sql}");
        }
        assert_eq!(placeholders(DELETE_WEDDING), 2);
    }

    #[test]
    fn dates_are_formatted_for_mysql() {
        assert_eq!(CalendarDate::from_unix_millis_utc(0).to_string(), "1970-01-01");
        // 2023-11-14 22:13:20 UTC.
        assert_eq!(
            CalendarDate::from_unix_millis_utc(1_700_000_000_000).to_string(),
            "2023-11-14"
        );
        // Leap day 2000-02-29 00:00:00 UTC and the millisecond before it.
        assert_eq!(
            CalendarDate::from_unix_millis_utc(951_782_400_000).to_string(),
            "2000-02-29"
        );
        assert_eq!(
            CalendarDate::from_unix_millis_utc(951_782_399_999).to_string(),
            "2000-02-28"
        );
        // Before 1970 rounds toward the earlier day.
        assert_eq!(CalendarDate::from_unix_millis_utc(-1).to_string(), "1969-12-31");
        assert_eq!(
            CalendarDate::from_unix_millis_utc(-86_400_000 * 365).to_string(),
            "1969-01-01"
        );
    }
}
