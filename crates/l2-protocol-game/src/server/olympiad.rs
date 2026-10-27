//! Server packets of the Olympiad (the match list and results, the observer and the spelled
//! buffs of a fighter) and of the hero list.
//!
//! Values only, like the other server packets: the Olympiad games, the participants and the
//! heroes are the world's and arrive here as plain fields. The Java constructors that only
//! rearrange values (the winning team of a result) are written as constructors below.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExOlympiadMatchEnd.java
server_unit_packet!(
    /// The Olympiad match is over (`EX_OLYMPIAD_MATCH_END`). Java sends the static packet.
    ExOlympiadMatchEnd,
    ExOlympiadMatchEnd
);

// Java: gameserver/network/serverpackets/ExOlympiadMode.java
/// The Olympiad observer mode (`EX_OLYMPIAD_MODE`): `0` returns the player, `3` starts spectating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExOlympiadMode {
    /// Java `_mode`, written as a byte.
    pub mode: u8,
}

impl ExOlympiadMode {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExOlympiadMode;
}

impl Encode for ExOlympiadMode {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(self.mode);
    }
}

impl Decode for ExOlympiadMode {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { mode: r.u8()? })
    }
}

/// One running match of the match list (Java `AbstractOlympiadGame` as the list shows it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadMatchEntry {
    /// Java `game.getStadiumId()` (arena 1 is `0`).
    pub stadium_id: i32,
    /// `1` non-classed, `2` classed, `-1` teams, `0` any other game type.
    pub game_kind: i32,
    /// `1` standby, `2` playing (`task.isRunning()`).
    pub state: i32,
    /// Java `game.getPlayerNames()[0]`.
    pub player_1: String,
    /// Java `game.getPlayerNames()[1]`.
    pub player_2: String,
}

// Java: gameserver/network/serverpackets/ExOlympiadMatchList.java
/// The running matches, shown in the observer list (`EX_RECEIVE_OLYMPIAD`, type `0`).
///
/// Java counts the tasks with a game and writes that count; the world passes only the games that
/// have one, so the count is `games.len()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExOlympiadMatchList {
    pub games: Vec<OlympiadMatchEntry>,
}

impl ExOlympiadMatchList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReceiveOlympiad;
    /// The type int that selects the match list.
    const TYPE: i32 = 0;
}

impl Encode for ExOlympiadMatchList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(Self::TYPE);
        w.i32(self.games.len() as i32);
        // Java writes an unused zero after the count.
        w.i32(0);
        for game in &self.games {
            w.i32(game.stadium_id)
                .i32(game.game_kind)
                .i32(game.state)
                .string(&game.player_1)
                .string(&game.player_2);
        }
    }
}

impl Decode for ExOlympiadMatchList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _type = r.i32()?;
        let count = read_count(r, "match count")?;
        let _zero = r.i32()?;
        let mut games = Vec::new();
        for _ in 0..count {
            games.push(OlympiadMatchEntry {
                stadium_id: r.i32()?,
                game_kind: r.i32()?,
                state: r.i32()?,
                player_1: r.string(),
                player_2: r.string(),
            });
        }
        Ok(Self { games })
    }
}

/// One fighter in the result of a match (Java `OlympiadInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadResultEntry {
    pub name: String,
    pub clan_name: String,
    pub clan_id: i32,
    pub class_id: i32,
    /// Damage dealt in the match.
    pub damage: i32,
    pub current_points: i32,
    pub diff_points: i32,
}

fn write_result_entry(w: &mut PacketWriter, entry: &OlympiadResultEntry) {
    w.string(&entry.name)
        .string(&entry.clan_name)
        .i32(entry.clan_id)
        .i32(entry.class_id)
        .i32(entry.damage)
        .i32(entry.current_points)
        .i32(entry.diff_points);
}

fn read_result_entry(r: &mut PacketReader) -> Result<OlympiadResultEntry, PacketError> {
    Ok(OlympiadResultEntry {
        name: r.string(),
        clan_name: r.string(),
        clan_id: r.i32()?,
        class_id: r.i32()?,
        damage: r.i32()?,
        current_points: r.i32()?,
        diff_points: r.i32()?,
    })
}

// Java: gameserver/network/serverpackets/ExOlympiadMatchResult.java
/// The result of a match (`EX_RECEIVE_OLYMPIAD`, type `1`). Teams are `1` and `2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExOlympiadMatchResult {
    /// `true` for a tie: Java writes the boolean as `1` (tie) or `0` (win).
    pub tie: bool,
    /// Java `_winnerList.get(0).getName()`: the first winner's name (empty when there is none).
    pub winner_name: String,
    /// Java `_winTeam`.
    pub win_team: i32,
    pub winners: Vec<OlympiadResultEntry>,
    /// Java `_loseTeam`.
    pub lose_team: i32,
    pub losers: Vec<OlympiadResultEntry>,
}

impl ExOlympiadMatchResult {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReceiveOlympiad;
    /// The type int that selects the match result.
    const TYPE: i32 = 1;

    /// Java's constructor: `_loseTeam` is `2`, or `1` when the winning team is `2`; a winning team
    /// `0` (no winner) is shown as team `1`.
    pub fn new(
        tie: bool,
        win_team: i32,
        winners: Vec<OlympiadResultEntry>,
        losers: Vec<OlympiadResultEntry>,
    ) -> Self {
        let mut lose_team = 2;
        let mut win_team = win_team;
        if win_team == 2 {
            lose_team = 1;
        } else if win_team == 0 {
            win_team = 1;
        }
        let winner_name = winners.first().map(|w| w.name.clone()).unwrap_or_default();
        Self {
            tie,
            winner_name,
            win_team,
            winners,
            lose_team,
            losers,
        }
    }
}

impl Encode for ExOlympiadMatchResult {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(Self::TYPE);
        w.i32(i32::from(self.tie));
        w.string(&self.winner_name);
        w.i32(self.win_team);
        w.i32(self.winners.len() as i32);
        for entry in &self.winners {
            write_result_entry(w, entry);
        }
        w.i32(self.lose_team);
        w.i32(self.losers.len() as i32);
        for entry in &self.losers {
            write_result_entry(w, entry);
        }
    }
}

impl Decode for ExOlympiadMatchResult {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _type = r.i32()?;
        let tie = r.i32()? != 0;
        let winner_name = r.string();
        let win_team = r.i32()?;
        let count = read_count(r, "winner count")?;
        let mut winners = Vec::new();
        for _ in 0..count {
            winners.push(read_result_entry(r)?);
        }
        let lose_team = r.i32()?;
        let count = read_count(r, "loser count")?;
        let mut losers = Vec::new();
        for _ in 0..count {
            losers.push(read_result_entry(r)?);
        }
        Ok(Self {
            tie,
            winner_name,
            win_team,
            winners,
            lose_team,
            losers,
        })
    }
}

// Java: gameserver/network/serverpackets/ExOlympiadUserInfo.java
/// A fighter's side, id, name, class and HP/CP (`EX_OLYMPIAD_USER_INFO`).
///
/// Java's constructors take the player, or the participant of a match, and fall back to `0/100`
/// HP and CP when the player is gone; the world passes those values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExOlympiadUserInfo {
    /// Java `getOlympiadSide()`, written as a byte.
    pub side: u8,
    pub object_id: i32,
    pub name: String,
    /// The base class id (Java `getPlayerClass().getId()` or `getBaseClass()`).
    pub class_id: i32,
    pub cur_hp: i32,
    pub max_hp: i32,
    pub cur_cp: i32,
    pub max_cp: i32,
}

impl ExOlympiadUserInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExOlympiadUserInfo;
}

impl Encode for ExOlympiadUserInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(self.side)
            .i32(self.object_id)
            .string(&self.name)
            .i32(self.class_id)
            .i32(self.cur_hp)
            .i32(self.max_hp)
            .i32(self.cur_cp)
            .i32(self.max_cp);
    }
}

impl Decode for ExOlympiadUserInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            side: r.u8()?,
            object_id: r.i32()?,
            name: r.string(),
            class_id: r.i32()?,
            cur_hp: r.i32()?,
            max_hp: r.i32()?,
            cur_cp: r.i32()?,
            max_cp: r.i32()?,
        })
    }
}

/// One buff shown on a fighter (Java `BuffInfo`, the skill's display id and level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlympiadSpelledEffect {
    /// Java `getSkill().getDisplayId()`.
    pub skill_id: i32,
    /// Java `getSkill().getDisplayLevel()`, written as a short.
    pub level: i16,
    /// Java `getTime()`.
    pub time: i32,
}

// Java: gameserver/network/serverpackets/ExOlympiadSpelledInfo.java
/// The buffs of a fighter in the Olympiad (`EX_OLYMPIAD_SPELLED_INFO`).
///
/// Java skips buffs that are not in use; the world passes only the ones that are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExOlympiadSpelledInfo {
    pub player_id: i32,
    pub effects: Vec<OlympiadSpelledEffect>,
}

impl ExOlympiadSpelledInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExOlympiadSpelledInfo;
}

impl Encode for ExOlympiadSpelledInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.player_id).i32(self.effects.len() as i32);
        for effect in &self.effects {
            w.i32(effect.skill_id).i16(effect.level).i32(effect.time);
        }
    }
}

impl Decode for ExOlympiadSpelledInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_id = r.i32()?;
        let count = read_count(r, "effect count")?;
        let mut effects = Vec::new();
        for _ in 0..count {
            effects.push(OlympiadSpelledEffect {
                skill_id: r.i32()?,
                level: r.i16()?,
                time: r.i32()?,
            });
        }
        Ok(Self { player_id, effects })
    }
}

/// One hero of the hero list (Java `StatSet` of `Hero.getHeroes()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroEntry {
    pub name: String,
    pub class_id: i32,
    /// Java `Hero.CLAN_NAME`, `""` when the hero has no clan.
    pub clan_name: String,
    pub clan_crest: i32,
    /// Java `Hero.ALLY_NAME`, `""` when there is no alliance.
    pub ally_name: String,
    pub ally_crest: i32,
    /// Java `Hero.COUNT`: how many times the hero won.
    pub count: i32,
}

// Java: gameserver/network/serverpackets/ExHeroList.java
/// The hero list (`EX_HERO_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExHeroList {
    pub heroes: Vec<HeroEntry>,
}

impl ExHeroList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExHeroList;
}

impl Encode for ExHeroList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.heroes.len() as i32);
        for hero in &self.heroes {
            w.string(&hero.name)
                .i32(hero.class_id)
                .string(&hero.clan_name)
                .i32(hero.clan_crest)
                .string(&hero.ally_name)
                .i32(hero.ally_crest)
                .i32(hero.count);
        }
    }
}

impl Decode for ExHeroList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "hero count")?;
        let mut heroes = Vec::new();
        for _ in 0..count {
            heroes.push(HeroEntry {
                name: r.string(),
                class_id: r.i32()?,
                clan_name: r.string(),
                clan_crest: r.i32()?,
                ally_name: r.string(),
                ally_crest: r.i32()?,
                count: r.i32()?,
            });
        }
        Ok(Self { heroes })
    }
}

// Java: gameserver/network/serverpackets/ObservationMode.java
/// The observer mode starts at a place (`OBSERVER_START`, M8-OLY-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationMode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ObservationMode {
    pub const OPCODE: ServerOpcode = ServerOpcode::ObserverStart;
}

impl Encode for ObservationMode {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z);
        // Java writes three unknown bytes: 0, 0xC0, 0.
        w.u8(0).u8(0xC0).u8(0);
    }
}

impl Decode for ObservationMode {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        };
        let _ = (r.u8()?, r.u8()?, r.u8()?);
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/ObservationReturn.java
/// The observer mode ends; the player is back at a place (`OBSERVER_END`, M8-OLY-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationReturn {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ObservationReturn {
    pub const OPCODE: ServerOpcode = ServerOpcode::ObserverEnd;
}

impl Encode for ObservationReturn {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z);
    }
}

impl Decode for ObservationReturn {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}
