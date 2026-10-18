//! Server packets of the character selection screen.
//!
//! These packets only carry values. Loading characters from the database, computing the exp
//! percentage and the delete countdown, and checking bans is the caller's job (M2-CHAR-01); the
//! small helpers at the top reproduce the arithmetic Java does *while writing* the packet.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Inventory paperdoll slot numbers (Java `Inventory.PAPERDOLL_*`).
pub mod paperdoll {
    pub const UNDER: usize = 0;
    pub const HEAD: usize = 1;
    pub const HAIR: usize = 2;
    pub const HAIR2: usize = 3;
    pub const NECK: usize = 4;
    pub const RHAND: usize = 5;
    pub const CHEST: usize = 6;
    pub const LHAND: usize = 7;
    pub const REAR: usize = 8;
    pub const LEAR: usize = 9;
    pub const GLOVES: usize = 10;
    pub const LEGS: usize = 11;
    pub const FEET: usize = 12;
    pub const RFINGER: usize = 13;
    pub const LFINGER: usize = 14;
    pub const LBRACELET: usize = 15;
    pub const RBRACELET: usize = 16;
    pub const DECO1: usize = 17;
    pub const DECO2: usize = 18;
    pub const DECO3: usize = 19;
    pub const DECO4: usize = 20;
    pub const DECO5: usize = 21;
    pub const DECO6: usize = 22;
    pub const CLOAK: usize = 23;
    pub const BELT: usize = 24;
    /// Number of slots (Java `PAPERDOLL_TOTALSLOTS`).
    pub const TOTAL_SLOTS: usize = 25;

    /// The order in which the item ids go on the wire (Java `ServerPacket.PAPERDOLL_ORDER`).
    ///
    /// NOTE(java-quirk): `RHAND` appears twice (positions 7 and 14). The client reads the
    /// second one as the two-handed weapon slot; Java simply writes the same item id again.
    pub const WIRE_ORDER: [usize; 26] = [
        UNDER, REAR, LEAR, NECK, RFINGER, LFINGER, HEAD, RHAND, LHAND, GLOVES, CHEST, LEGS, FEET, CLOAK,
        RHAND, HAIR, HAIR2, RBRACELET, LBRACELET, DECO1, DECO2, DECO3, DECO4, DECO5, DECO6, BELT,
    ];
}

/// Java `(float) (exp - expForLevel) / (expForNextLevel - expForLevel)`, as written into
/// [`CharSelectEntry::exp_percent`].
///
/// NOTE(java-quirk): the division is done in 32-bit `float` and widened to `double` afterwards,
/// so the value is only float-precise. Level 80 and above divide by a real exp span, never 0.
pub fn exp_percent(exp: i64, exp_for_level: i64, exp_for_next_level: i64) -> f64 {
    f64::from((exp - exp_for_level) as f32 / (exp_for_next_level - exp_for_level) as f32)
}

/// The "delete timer" int of a character list entry (Java `CharSelectionInfo.writeImpl`).
///
/// `-1` for a banned character (access level below 0), the seconds left when the character is
/// marked for deletion (`delete_time_ms > 0`), else `0`.
pub fn delete_seconds(access_level: i32, delete_time_ms: i64, now_ms: i64) -> i32 {
    if access_level > -1 {
        if delete_time_ms > 0 {
            // Java: `(int) ((deleteTimer - now) / 1000)`, truncating long division.
            ((delete_time_ms - now_ms) / 1000) as i32
        } else {
            0
        }
    } else {
        -1
    }
}

/// Which entry the client highlights when the server does not pick one: the character with the
/// highest `lastAccess`, the first one winning a tie. `None` when no character was ever used.
/// After a delete or restore Java picks slot 0 instead.
pub fn active_slot(last_access: &[i64]) -> Option<usize> {
    let mut best = 0;
    let mut slot = None;
    for (i, &time) in last_access.iter().enumerate() {
        if best < time {
            best = time;
            slot = Some(i);
        }
    }
    slot
}

// Java: gameserver/network/serverpackets/CharSelectionInfo.java (one loop iteration)
/// One character of [`CharSelectionInfo`]: every value Java writes for it, already computed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharSelectEntry {
    pub name: String,
    pub object_id: i32,
    pub clan_id: i32,
    pub sex: i32,
    pub race: i32,
    pub base_class_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub current_hp: f64,
    pub current_mp: f64,
    /// Java writes `(int) sp` of a long: pass the already truncated value.
    pub sp: i32,
    pub exp: i64,
    /// See [`exp_percent`].
    pub exp_percent: f64,
    pub level: i32,
    pub karma: i32,
    pub pk_kills: i32,
    pub pvp_kills: i32,
    /// Visible item id per paperdoll slot, indexed by the [`paperdoll`] constants.
    pub paperdoll: [i32; paperdoll::TOTAL_SLOTS],
    pub hair_style: i32,
    pub hair_color: i32,
    pub face: i32,
    pub max_hp: f64,
    pub max_mp: f64,
    /// See [`delete_seconds`].
    pub delete_seconds: i32,
    pub class_id: i32,
    /// Highlighted in the list (Java: `i == activeId`).
    pub selected: bool,
    /// Enchant of the equipped weapon. Java writes `min(value, 127)` as one byte, so decoding
    /// gives back that byte.
    pub enchant_effect: i32,
    pub augmentation_id: i32,
    pub vitality_points: i32,
}

impl CharSelectEntry {
    /// Bytes of one entry for a given account name (it is written inside every entry).
    fn write(&self, w: &mut PacketWriter, account_name: &str, session_id: i32) {
        w.string(&self.name).i32(self.object_id);
        w.string(account_name).i32(session_id);
        w.i32(self.clan_id).i32(0); // 0: builder level
        w.i32(self.sex).i32(self.race).i32(self.base_class_id);
        w.i32(1); // Java: "GameServerName", always 1
        w.i32(self.x).i32(self.y).i32(self.z);
        w.f64(self.current_hp).f64(self.current_mp);
        w.i32(self.sp).i64(self.exp).f64(self.exp_percent);
        w.i32(self.level)
            .i32(self.karma)
            .i32(self.pk_kills)
            .i32(self.pvp_kills);
        w.bytes(&[0; 7 * 4]); // seven unused ints
        for slot in paperdoll::WIRE_ORDER {
            w.i32(self.paperdoll[slot]);
        }
        w.i32(self.hair_style).i32(self.hair_color).i32(self.face);
        w.f64(self.max_hp).f64(self.max_mp);
        w.i32(self.delete_seconds).i32(self.class_id);
        w.i32(i32::from(self.selected));
        // `min(.., 127)` as in Java, then the low byte like `writeByte(int)`.
        w.u8(self.enchant_effect.min(127) as u8);
        w.i32(self.augmentation_id);
        // Transformation, pet npc id, pet level, pet food, pet food level: retail shows none of
        // them on the character screen.
        w.bytes(&[0; 5 * 4]);
        w.f64(0.0).f64(0.0); // pet hp, pet mp
        w.i32(self.vitality_points);
    }

    /// Reads one entry; also returns the account name and session id written inside it.
    fn read(r: &mut PacketReader) -> Result<(Self, String, i32), PacketError> {
        let name = r.string();
        let object_id = r.i32()?;
        let account_name = r.string();
        let session_id = r.i32()?;
        let clan_id = r.i32()?;
        r.skip(4)?; // builder level
        let sex = r.i32()?;
        let race = r.i32()?;
        let base_class_id = r.i32()?;
        r.skip(4)?; // constant 1
        let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
        let (current_hp, current_mp) = (r.f64()?, r.f64()?);
        let sp = r.i32()?;
        let exp = r.i64()?;
        let exp_percent = r.f64()?;
        let (level, karma, pk_kills, pvp_kills) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
        r.skip(7 * 4)?;
        let mut paperdoll = [0; paperdoll::TOTAL_SLOTS];
        for slot in paperdoll::WIRE_ORDER {
            paperdoll[slot] = r.i32()?;
        }
        let (hair_style, hair_color, face) = (r.i32()?, r.i32()?, r.i32()?);
        let (max_hp, max_mp) = (r.f64()?, r.f64()?);
        let delete_seconds = r.i32()?;
        let class_id = r.i32()?;
        let selected = r.i32()? != 0;
        let enchant_effect = i32::from(r.u8()?);
        let augmentation_id = r.i32()?;
        r.skip(5 * 4 + 2 * 8)?;
        let vitality_points = r.i32()?;
        let entry = Self {
            name,
            object_id,
            clan_id,
            sex,
            race,
            base_class_id,
            x,
            y,
            z,
            current_hp,
            current_mp,
            sp,
            exp,
            exp_percent,
            level,
            karma,
            pk_kills,
            pvp_kills,
            paperdoll,
            hair_style,
            hair_color,
            face,
            max_hp,
            max_mp,
            delete_seconds,
            class_id,
            selected,
            enchant_effect,
            augmentation_id,
            vitality_points,
        };
        Ok((entry, account_name, session_id))
    }
}

// Java: gameserver/network/serverpackets/CharSelectionInfo.java
/// The character list. Sent after `AuthLogin`, `RequestGotoLobby`, `CharacterDelete` and
/// `CharacterRestore`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharSelectionInfo {
    /// Account name, written inside every entry.
    pub account_name: String,
    /// The login server's `playOkID1`, written inside every entry as the "account id".
    pub session_id: i32,
    /// `ServerConfig.MAX_CHARACTERS_NUMBER_PER_ACCOUNT` (`CharMaxNumber`); 0 stops the client
    /// from creating characters.
    pub max_characters: i32,
    pub characters: Vec<CharSelectEntry>,
}

impl CharSelectionInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterSelectionInfo;
}

impl Encode for CharSelectionInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java writes `size` as an int; a character list never gets near i32::MAX.
        w.i32(self.characters.len() as i32).i32(self.max_characters);
        w.u8(0);
        for entry in &self.characters {
            entry.write(w, &self.account_name, self.session_id);
        }
    }
}

impl Decode for CharSelectionInfo {
    /// The account name and session id live inside the entries, so an empty list decodes with an
    /// empty account name and session id 0.
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        let count = usize::try_from(count)
            .map_err(|_| PacketError::invalid("character count", format!("negative: {count}")))?;
        let max_characters = r.i32()?;
        r.skip(1)?;
        let mut packet = Self {
            max_characters,
            ..Self::default()
        };
        for i in 0..count {
            let (entry, account_name, session_id) = CharSelectEntry::read(r)?;
            if i == 0 {
                packet.account_name = account_name;
                packet.session_id = session_id;
            }
            packet.characters.push(entry);
        }
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/NewCharacterSuccess.java (one `addChar`)
/// Race/class template shown on the character creation screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewCharTemplate {
    /// `Race.ordinal()`.
    pub race: i32,
    pub class_id: i32,
    pub base_str: i32,
    pub base_dex: i32,
    pub base_con: i32,
    pub base_int: i32,
    pub base_wit: i32,
    pub base_men: i32,
}

impl NewCharTemplate {
    /// Java hardcodes `0x46` before and `0x0A` after every base stat ("TODO: Unhardcode these").
    const BEFORE_STAT: i32 = 0x46;
    const AFTER_STAT: i32 = 0x0A;
    /// Wire size: race, class id and six stats with their two constants each.
    const WIRE_LEN: usize = 4 * (2 + 6 * 3);

    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.race).i32(self.class_id);
        // Wire order of the stats: STR, DEX, CON, INT, WIT, MEN.
        for stat in [
            self.base_str,
            self.base_dex,
            self.base_con,
            self.base_int,
            self.base_wit,
            self.base_men,
        ] {
            w.i32(Self::BEFORE_STAT).i32(stat).i32(Self::AFTER_STAT);
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let race = r.i32()?;
        let class_id = r.i32()?;
        let mut stats = [0; 6];
        for stat in &mut stats {
            r.skip(4)?;
            *stat = r.i32()?;
            r.skip(4)?;
        }
        let [base_str, base_dex, base_con, base_int, base_wit, base_men] = stats;
        Ok(Self {
            race,
            class_id,
            base_str,
            base_dex,
            base_con,
            base_int,
            base_wit,
            base_men,
        })
    }
}

// Java: gameserver/network/serverpackets/NewCharacterSuccess.java
/// The answer to `NewCharacter`: the templates the creation screen offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCharacterSuccess {
    pub templates: Vec<NewCharTemplate>,
}

impl NewCharacterSuccess {
    pub const OPCODE: ServerOpcode = ServerOpcode::NewCharacterSuccess;
}

impl Encode for NewCharacterSuccess {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.templates.len() as i32);
        for template in &self.templates {
            template.write(w);
        }
    }
}

impl Decode for NewCharacterSuccess {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        let count = usize::try_from(count)
            .map_err(|_| PacketError::invalid("template count", format!("negative: {count}")))?;
        if count > r.remaining() / NewCharTemplate::WIRE_LEN {
            return Err(PacketError::invalid(
                "template count",
                "more templates than bytes",
            ));
        }
        let templates = (0..count)
            .map(|_| NewCharTemplate::read(r))
            .collect::<Result<_, _>>()?;
        Ok(Self { templates })
    }
}

// Java: gameserver/network/serverpackets/CharCreateOk.java
/// Character created (opcode name `CHARACTER_CREATE_SUCCESS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharCreateOk;

impl CharCreateOk {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterCreateSuccess;
}

impl Encode for CharCreateOk {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(1);
    }
}

impl Decode for CharCreateOk {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?; // the constant 1
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/CharCreateFail.java
/// Character creation refused. `reason` is a plain int on the wire (Java passes any int), so
/// unknown values decode fine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharCreateFail {
    pub reason: i32,
}

impl CharCreateFail {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterCreateFail;
    /// "Your character creation has failed."
    pub const REASON_CREATION_FAILED: i32 = 0x00;
    /// "You cannot create another character. Please delete the existing character..."
    pub const REASON_TOO_MANY_CHARACTERS: i32 = 0x01;
    /// "This name already exists."
    pub const REASON_NAME_ALREADY_EXISTS: i32 = 0x02;
    /// The text says "title" but it is the 16-character name limit.
    pub const REASON_16_ENG_CHARS: i32 = 0x03;
    /// "Incorrect name. Please try again."
    pub const REASON_INCORRECT_NAME: i32 = 0x04;
    /// "Characters cannot be created from this server."
    pub const REASON_CREATE_NOT_ALLOWED: i32 = 0x05;
    /// "Unable to create character... Please choose another server."
    pub const REASON_CHOOSE_ANOTHER_SVR: i32 = 0x06;
}

impl Encode for CharCreateFail {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.reason);
    }
}

impl Decode for CharCreateFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { reason: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/CharDeleteSuccess.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharDeleteSuccess;

impl CharDeleteSuccess {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterDeleteSuccess;
}

impl Encode for CharDeleteSuccess {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for CharDeleteSuccess {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/CharDeleteFail.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharDeleteFail {
    pub reason: i32,
}

impl CharDeleteFail {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterDeleteFail;
    pub const REASON_DELETION_FAILED: i32 = 1;
    pub const REASON_YOU_MAY_NOT_DELETE_CLAN_MEMBER: i32 = 2;
    pub const REASON_CLAN_LEADERS_MAY_NOT_BE_DELETED: i32 = 3;
}

impl Encode for CharDeleteFail {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.reason);
    }
}

impl Decode for CharDeleteFail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { reason: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/CharSelected.java
/// The player was loaded and enters the world next (opcode name `CHARACTER_SELECTED`).
#[derive(Debug, Clone, PartialEq)]
pub struct CharSelected {
    pub name: String,
    pub object_id: i32,
    pub title: String,
    /// The login server's `playOkID1`.
    pub session_id: i32,
    pub clan_id: i32,
    pub is_female: bool,
    /// `Race.ordinal()`.
    pub race: i32,
    pub class_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub current_hp: f64,
    pub current_mp: f64,
    /// Java writes `(int) sp` of a long: pass the already truncated value.
    pub sp: i32,
    pub exp: i64,
    pub level: i32,
    pub karma: i32,
    pub pk_kills: i32,
    pub int: i32,
    pub str: i32,
    pub con: i32,
    pub men: i32,
    pub dex: i32,
    pub wit: i32,
    /// Game clock in minutes of the day: `gameTime % (24 * 60)`.
    pub game_time: i32,
}

impl CharSelected {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharacterSelected;
}

impl Encode for CharSelected {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name).i32(self.object_id).string(&self.title);
        w.i32(self.session_id).i32(self.clan_id);
        w.i32(0); // Java: "??"
        w.i32(i32::from(self.is_female)).i32(self.race).i32(self.class_id);
        w.i32(1); // Java: "active ??"
        w.i32(self.x).i32(self.y).i32(self.z);
        w.f64(self.current_hp).f64(self.current_mp);
        w.i32(self.sp).i64(self.exp);
        w.i32(self.level).i32(self.karma).i32(self.pk_kills);
        // Wire order of the stats: INT, STR, CON, MEN, DEX, WIT.
        w.i32(self.int).i32(self.str).i32(self.con);
        w.i32(self.men).i32(self.dex).i32(self.wit);
        w.i32(self.game_time);
        w.i32(0);
        w.i32(self.class_id); // the class id is written a second time
        w.bytes(&[0; 4 * 4]); // four unused ints
        w.bytes(&[0; 64]);
        w.i32(0);
    }
}

impl Decode for CharSelected {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let object_id = r.i32()?;
        let title = r.string();
        let session_id = r.i32()?;
        let clan_id = r.i32()?;
        r.skip(4)?;
        let is_female = r.i32()? != 0;
        let race = r.i32()?;
        let class_id = r.i32()?;
        r.skip(4)?;
        let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
        let (current_hp, current_mp) = (r.f64()?, r.f64()?);
        let sp = r.i32()?;
        let exp = r.i64()?;
        let (level, karma, pk_kills) = (r.i32()?, r.i32()?, r.i32()?);
        let (int, str, con) = (r.i32()?, r.i32()?, r.i32()?);
        let (men, dex, wit) = (r.i32()?, r.i32()?, r.i32()?);
        let game_time = r.i32()?;
        r.skip(4 + 4 + 4 * 4 + 64 + 4)?; // zero, second class id, four zero ints, 64 bytes, zero
        Ok(Self {
            name,
            object_id,
            title,
            session_id,
            clan_id,
            is_female,
            race,
            class_id,
            x,
            y,
            z,
            current_hp,
            current_mp,
            sp,
            exp,
            level,
            karma,
            pk_kills,
            int,
            str,
            con,
            men,
            dex,
            wit,
            game_time,
        })
    }
}

// Java: gameserver/network/serverpackets/ServerClose.java
/// "Server closes the connection" notice, sent right before the socket is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerClose;

impl ServerClose {
    pub const OPCODE: ServerOpcode = ServerOpcode::ServerClose;
}

impl Encode for ServerClose {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ServerClose {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/SSQInfo.java
/// Sky colour of the Seven Signs competition (opcode name `SSQ_INFO`): 0 normal, 1 dusk, 2 dawn.
/// Java sends it right before `CharSelected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsqInfo {
    /// Java `_state`.
    pub state: i32,
}

impl SsqInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::SsqInfo;
}

impl Encode for SsqInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: `writeShort(256 + _state)`.
        w.i16((256 + self.state) as i16);
    }
}

impl Decode for SsqInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            state: i32::from(r.i16()?) - 256,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssq_info_adds_256_to_the_state() {
        let packet = SsqInfo { state: 2 };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, [0x73, 0x02, 0x01]);
        assert_eq!(
            SsqInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn exp_percent_uses_float_precision() {
        assert_eq!(exp_percent(150, 100, 200), 0.5);
        assert_eq!(exp_percent(100, 100, 200), 0.0);
        // A third is not representable: the float result widened to double differs from the
        // double division, which is what the client gets from Java.
        let third = exp_percent(1, 0, 3);
        assert_eq!(third, f64::from(1.0_f32 / 3.0_f32));
        assert_ne!(third, 1.0 / 3.0);
        // Same span as Java at level 1 -> 2 (exp 0 / 68): finite.
        assert!(exp_percent(34, 0, 68).is_finite());
    }

    #[test]
    fn delete_seconds_cases() {
        assert_eq!(delete_seconds(0, 0, 5_000), 0);
        assert_eq!(delete_seconds(0, 15_999, 5_000), 10);
        // Truncating division: 999 ms left is 0 seconds, an elapsed timer goes negative.
        assert_eq!(delete_seconds(0, 5_999, 5_000), 0);
        assert_eq!(delete_seconds(0, 3_000, 5_000), -2);
        // Banned characters show -1, whatever the timer says.
        assert_eq!(delete_seconds(-1, 0, 5_000), -1);
        assert_eq!(delete_seconds(-100, 99_000, 5_000), -1);
    }

    #[test]
    fn active_slot_picks_latest_access_first_wins() {
        assert_eq!(active_slot(&[]), None);
        assert_eq!(active_slot(&[0, 0]), None);
        assert_eq!(active_slot(&[5, 9, 2]), Some(1));
        assert_eq!(active_slot(&[7, 7, 3]), Some(0));
    }

    #[test]
    fn paperdoll_wire_order_covers_every_slot() {
        let mut seen = [0; paperdoll::TOTAL_SLOTS];
        for slot in paperdoll::WIRE_ORDER {
            seen[slot] += 1;
        }
        for (slot, &count) in seen.iter().enumerate() {
            assert_eq!(count, if slot == paperdoll::RHAND { 2 } else { 1 }, "slot {slot}");
        }
    }

    fn entry() -> CharSelectEntry {
        let mut paperdoll = [0; paperdoll::TOTAL_SLOTS];
        paperdoll[paperdoll::RHAND] = 6_592;
        paperdoll[paperdoll::CHEST] = 1_146;
        paperdoll[paperdoll::BELT] = 9_999;
        CharSelectEntry {
            name: "Tester".into(),
            object_id: 268_435_456,
            clan_id: 7,
            sex: 1,
            race: 2,
            base_class_id: 18,
            x: -71_301,
            y: 258_259,
            z: -3_132,
            current_hp: 100.5,
            current_mp: 50.25,
            sp: 1_000,
            exp: 123_456_789_012,
            exp_percent: 0.5,
            level: 20,
            karma: 3,
            pk_kills: 4,
            pvp_kills: 5,
            paperdoll,
            hair_style: 2,
            hair_color: 1,
            face: 0,
            max_hp: 200.0,
            max_mp: 100.0,
            delete_seconds: 600,
            class_id: 19,
            selected: true,
            enchant_effect: 12,
            augmentation_id: 77,
            vitality_points: 20_000,
        }
    }

    fn info() -> CharSelectionInfo {
        CharSelectionInfo {
            account_name: "acc".into(),
            session_id: 0x1234,
            max_characters: 7,
            characters: vec![entry(), CharSelectEntry::default()],
        }
    }

    #[test]
    fn char_selection_info_round_trip() {
        let packet = info();
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x09);
        assert_eq!(
            CharSelectionInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn char_selection_info_empty_list() {
        let packet = CharSelectionInfo {
            max_characters: 7,
            ..CharSelectionInfo::default()
        };
        // opcode, count 0, max 7, byte 0
        assert_eq!(packet.to_bytes(), [0x09, 0, 0, 0, 0, 7, 0, 0, 0, 0]);
        let decoded = CharSelectionInfo::decode(&mut PacketReader::new(&packet.to_bytes()[1..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn char_selection_info_entry_size() {
        // Field sizes in Java write order, for name "Tester" and account "acc" (UTF-16 + terminator).
        let sizes = [
            14, 4, 8, 4, // name, object id, account, session id
            4, 4, 4, 4, 4, 4, // clan, builder level, sex, race, base class, constant 1
            12, 16, 4, 8, 8, // x y z, hp mp, sp, exp, exp percent
            16, 28, 104, // level..pvp kills, seven ints, 26 paperdoll ints
            12, 16, // hair style, colour, face; max hp, max mp
            4, 4, 4, 1, 4, // delete timer, class id, selected, enchant byte, augmentation
            20, 16, 4, // transformation and pet ints, pet hp and mp, vitality
        ];
        let one = CharSelectionInfo {
            characters: vec![entry()],
            account_name: "acc".into(),
            ..CharSelectionInfo::default()
        };
        assert_eq!(one.to_bytes().len(), 1 + 4 + 4 + 1 + sizes.iter().sum::<usize>());
    }

    #[test]
    fn char_selection_info_clamps_enchant_to_127() {
        let mut e = entry();
        e.enchant_effect = 300;
        let packet = CharSelectionInfo {
            characters: vec![e],
            ..CharSelectionInfo::default()
        };
        let decoded = CharSelectionInfo::decode(&mut PacketReader::new(&packet.to_bytes()[1..])).unwrap();
        assert_eq!(decoded.characters[0].enchant_effect, 127);
    }

    #[test]
    fn char_selection_info_rejects_bad_input() {
        let bytes = info().to_bytes();
        assert!(CharSelectionInfo::decode(&mut PacketReader::new(&bytes[1..bytes.len() - 1])).is_err());
        let negative = [0xFF, 0xFF, 0xFF, 0xFF, 7, 0, 0, 0, 0];
        assert!(CharSelectionInfo::decode(&mut PacketReader::new(&negative)).is_err());
    }

    fn template(class_id: i32) -> NewCharTemplate {
        NewCharTemplate {
            race: 0,
            class_id,
            base_str: 40,
            base_dex: 30,
            base_con: 43,
            base_int: 21,
            base_wit: 11,
            base_men: 25,
        }
    }

    #[test]
    fn new_character_success_layout() {
        let packet = NewCharacterSuccess {
            templates: vec![template(0)],
        };
        let mut expected = vec![0x0D];
        let mut ints = |values: &[i32]| {
            for v in values {
                expected.extend(v.to_le_bytes());
            }
        };
        // count, race, class, then (0x46, stat, 0x0A) for STR, DEX, CON, INT, WIT, MEN.
        ints(&[1, 0, 0]);
        for stat in [40, 30, 43, 21, 11, 25] {
            ints(&[0x46, stat, 0x0A]);
        }
        let bytes = packet.to_bytes();
        assert_eq!(bytes, expected);
        assert_eq!(
            NewCharacterSuccess::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn new_character_success_count_is_checked() {
        // Claims 1000 templates but has none: must fail cleanly, not allocate.
        assert!(NewCharacterSuccess::decode(&mut PacketReader::new(&1000_i32.to_le_bytes())).is_err());
        assert!(NewCharacterSuccess::decode(&mut PacketReader::new(&(-1_i32).to_le_bytes())).is_err());
        let empty = NewCharacterSuccess { templates: vec![] };
        assert_eq!(empty.to_bytes(), [0x0D, 0, 0, 0, 0]);
    }

    #[test]
    fn small_packets() {
        assert_eq!(CharCreateOk.to_bytes(), [0x0F, 1, 0, 0, 0]);
        assert_eq!(
            CharCreateFail {
                reason: CharCreateFail::REASON_INCORRECT_NAME
            }
            .to_bytes(),
            [0x10, 4, 0, 0, 0]
        );
        assert_eq!(CharDeleteSuccess.to_bytes(), [0x1D]);
        assert_eq!(
            CharDeleteFail {
                reason: CharDeleteFail::REASON_CLAN_LEADERS_MAY_NOT_BE_DELETED
            }
            .to_bytes(),
            [0x1E, 3, 0, 0, 0]
        );
        assert_eq!(ServerClose.to_bytes(), [0x20]);
        // Round trips of the packets above.
        let fail = CharCreateFail { reason: 6 };
        assert_eq!(
            CharCreateFail::decode(&mut PacketReader::new(&fail.to_bytes()[1..])).unwrap(),
            fail
        );
        let fail = CharDeleteFail { reason: 2 };
        assert_eq!(
            CharDeleteFail::decode(&mut PacketReader::new(&fail.to_bytes()[1..])).unwrap(),
            fail
        );
        assert!(CharCreateOk::decode(&mut PacketReader::new(&CharCreateOk.to_bytes()[1..])).is_ok());
        assert!(CharCreateOk::decode(&mut PacketReader::new(&[1, 0])).is_err());
    }

    fn selected() -> CharSelected {
        CharSelected {
            name: "Tester".into(),
            object_id: 268_435_456,
            title: "".into(),
            session_id: 0x1234,
            clan_id: 0,
            is_female: false,
            race: 1,
            class_id: 18,
            x: 1,
            y: 2,
            z: 3,
            current_hp: 10.0,
            current_mp: 5.0,
            sp: 99,
            exp: 1 << 40,
            level: 5,
            karma: 0,
            pk_kills: 1,
            int: 21,
            str: 41,
            con: 43,
            men: 25,
            dex: 30,
            wit: 11,
            game_time: 600,
        }
    }

    #[test]
    fn char_selected_round_trip_and_size() {
        let packet = selected();
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x0B);
        assert_eq!(
            CharSelected::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        // Tail: game time, zero, class id again, four zero ints, 64 zero bytes, zero int.
        let tail = &bytes[bytes.len() - 96..];
        assert_eq!(tail[..4], 600_i32.to_le_bytes());
        assert_eq!(tail[8..12], 18_i32.to_le_bytes());
        assert!(tail[4..8].iter().chain(&tail[12..]).all(|&b| b == 0));
    }
}
