//! Server packets of the minigames: the Cube game (the arena lobby and the match), the
//! Underground Coliseum match record, the countdown UI event and the movie start.
//!
//! Values only. The Cube game packets share two ex ids, `EX_BLOCK_UP_SET_LIST` and
//! `EX_BLOCK_UP_SET_STATE`, and tell their meaning apart by a leading sub id int, which Java
//! writes as a constant per packet. The sub ids are written here, and read and ignored when
//! decoding.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Java `0xffffffff` (`-1`): the "no arena" / "all" value of the Cube game lists.
const NO_ARENA: i32 = -1;

/// Writes the id of an `EX_BLOCK_UP_SET_LIST` packet followed by its sub id.
fn write_block_list(w: &mut PacketWriter, sub: i32) {
    write_opcode(w, ServerOpcode::ExBlockUpSetList);
    w.i32(sub);
}

/// Writes the id of an `EX_BLOCK_UP_SET_STATE` packet followed by its sub id.
fn write_block_state(w: &mut PacketWriter, sub: i32) {
    write_opcode(w, ServerOpcode::ExBlockUpSetState);
    w.i32(sub);
}

// Java: gameserver/network/serverpackets/ExCubeGameAddPlayer.java
/// A player joined a team of the Cube game (`EX_BLOCK_UP_SET_LIST`, sub 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExCubeGameAddPlayer {
    /// Java `_isRedTeam`.
    pub red_team: bool,
    pub object_id: i32,
    pub name: String,
}

impl ExCubeGameAddPlayer {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameAddPlayer {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 1);
        w.i32(NO_ARENA)
            .i32(i32::from(self.red_team))
            .i32(self.object_id)
            .string(&self.name);
    }
}

impl Decode for ExCubeGameAddPlayer {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        let _arena = r.i32()?;
        Ok(Self {
            red_team: r.i32()? != 0,
            object_id: r.i32()?,
            name: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameRemovePlayer.java
/// A player left a team of the Cube game (`EX_BLOCK_UP_SET_LIST`, sub 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameRemovePlayer {
    pub red_team: bool,
    pub object_id: i32,
}

impl ExCubeGameRemovePlayer {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameRemovePlayer {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 2);
        w.i32(NO_ARENA).i32(i32::from(self.red_team)).i32(self.object_id);
    }
}

impl Decode for ExCubeGameRemovePlayer {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        let _arena = r.i32()?;
        Ok(Self {
            red_team: r.i32()? != 0,
            object_id: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameChangeTeam.java
/// A player switched team of the Cube game (`EX_BLOCK_UP_SET_LIST`, sub 5). Java writes the
/// "from red" flag and its negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameChangeTeam {
    pub object_id: i32,
    /// Java `_fromRedTeam`.
    pub from_red_team: bool,
}

impl ExCubeGameChangeTeam {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameChangeTeam {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 5);
        w.i32(self.object_id)
            .i32(i32::from(self.from_red_team))
            .i32(i32::from(!self.from_red_team));
    }
}

impl Decode for ExCubeGameChangeTeam {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        let object_id = r.i32()?;
        let from_red_team = r.i32()? != 0;
        let _to_blue = r.i32()?;
        Ok(Self {
            object_id,
            from_red_team,
        })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameChangePoints.java
/// The time and the points of both teams (`EX_BLOCK_UP_SET_STATE`, sub 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameChangePoints {
    pub time_left: i32,
    pub blue_points: i32,
    pub red_points: i32,
}

impl ExCubeGameChangePoints {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetState;
}

impl Encode for ExCubeGameChangePoints {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_state(w, 2);
        w.i32(self.time_left).i32(self.blue_points).i32(self.red_points);
    }
}

impl Decode for ExCubeGameChangePoints {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self {
            time_left: r.i32()?,
            blue_points: r.i32()?,
            red_points: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameExtendedChangePoints.java
/// The time and the points, with the player's own team and points (`EX_BLOCK_UP_SET_STATE`,
/// sub 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameExtendedChangePoints {
    pub time_left: i32,
    pub blue_points: i32,
    pub red_points: i32,
    pub red_team: bool,
    pub object_id: i32,
    pub player_points: i32,
}

impl ExCubeGameExtendedChangePoints {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetState;
}

impl Encode for ExCubeGameExtendedChangePoints {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_state(w, 0);
        w.i32(self.time_left)
            .i32(self.blue_points)
            .i32(self.red_points)
            .i32(i32::from(self.red_team))
            .i32(self.object_id)
            .i32(self.player_points);
    }
}

impl Decode for ExCubeGameExtendedChangePoints {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self {
            time_left: r.i32()?,
            blue_points: r.i32()?,
            red_points: r.i32()?,
            red_team: r.i32()? != 0,
            object_id: r.i32()?,
            player_points: r.i32()?,
        })
    }
}

/// One player of a Cube game team list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubePlayer {
    pub object_id: i32,
    pub name: String,
}

fn write_cube_players(w: &mut PacketWriter, players: &[CubePlayer]) {
    w.i32(players.len() as i32);
    for player in players {
        w.i32(player.object_id).string(&player.name);
    }
}

fn read_cube_players(r: &mut PacketReader) -> Result<Vec<CubePlayer>, PacketError> {
    let count = read_count(r, "cube player count")?;
    let mut players = Vec::new();
    for _ in 0..count {
        players.push(CubePlayer {
            object_id: r.i32()?,
            name: r.string(),
        });
    }
    Ok(players)
}

// Java: gameserver/network/serverpackets/ExCubeGameTeamList.java
/// The players of both teams in a Cube game room (`EX_BLOCK_UP_SET_LIST`, sub 0).
///
/// Java writes `roomNumber - 1` as the room; `room` is that written value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExCubeGameTeamList {
    pub room: i32,
    /// Java writes the blue team first.
    pub blue: Vec<CubePlayer>,
    pub red: Vec<CubePlayer>,
}

impl ExCubeGameTeamList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameTeamList {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 0);
        w.i32(self.room).i32(NO_ARENA);
        write_cube_players(w, &self.blue);
        write_cube_players(w, &self.red);
    }
}

impl Decode for ExCubeGameTeamList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        let room = r.i32()?;
        let _none = r.i32()?;
        let blue = read_cube_players(r)?;
        let red = read_cube_players(r)?;
        Ok(Self { room, blue, red })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameCloseUI.java
/// The Cube game window closes (`EX_BLOCK_UP_SET_LIST`, sub -1, no other field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameCloseUi;

impl ExCubeGameCloseUi {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameCloseUi {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, NO_ARENA);
    }
}

impl Decode for ExCubeGameCloseUi {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameRequestReady.java
/// The "ready?" question of the Cube game (`EX_BLOCK_UP_SET_LIST`, sub 4, no other field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameRequestReady;

impl ExCubeGameRequestReady {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameRequestReady {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 4);
    }
}

impl Decode for ExCubeGameRequestReady {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameChangeTimeToStart.java
/// The seconds left before the Cube game starts (`EX_BLOCK_UP_SET_LIST`, sub 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameChangeTimeToStart {
    pub seconds: i32,
}

impl ExCubeGameChangeTimeToStart {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetList;
}

impl Encode for ExCubeGameChangeTimeToStart {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_list(w, 3);
        w.i32(self.seconds);
    }
}

impl Decode for ExCubeGameChangeTimeToStart {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self { seconds: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/ExCubeGameEnd.java
/// The Cube game is over, with the winning team (`EX_BLOCK_UP_SET_STATE`, sub 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCubeGameEnd {
    pub red_team_win: bool,
}

impl ExCubeGameEnd {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBlockUpSetState;
}

impl Encode for ExCubeGameEnd {
    fn encode(&self, w: &mut PacketWriter) {
        write_block_state(w, 1);
        w.i32(i32::from(self.red_team_win));
    }
}

impl Decode for ExCubeGameEnd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _sub = r.i32()?;
        Ok(Self {
            red_team_win: r.i32()? != 0,
        })
    }
}

/// The record state of an Underground Coliseum match (Java `ExPVPMatchRecord` `START`, `UPDATE`,
/// `FINISH`).
pub const PVP_RECORD_START: i32 = 0;
pub const PVP_RECORD_UPDATE: i32 = 1;
pub const PVP_RECORD_FINISH: i32 = 2;

/// One member of an Underground Coliseum team in the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpMember {
    pub name: String,
    pub kills: i32,
    pub deaths: i32,
}

fn write_pvp_members(w: &mut PacketWriter, members: &[PvpMember]) {
    w.i32(members.len() as i32);
    for member in members {
        w.string(&member.name).i32(member.kills).i32(member.deaths);
    }
}

fn read_pvp_members(r: &mut PacketReader) -> Result<Vec<PvpMember>, PacketError> {
    let count = read_count(r, "member count")?;
    let mut members = Vec::new();
    for _ in 0..count {
        members.push(PvpMember {
            name: r.string(),
            kills: r.i32()?,
            deaths: r.i32()?,
        });
    }
    Ok(members)
}

// Java: gameserver/network/serverpackets/ExPVPMatchRecord.java
/// The Underground Coliseum match record (`EX_PVP_MATCH_RECORD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPvpMatchRecord {
    /// `PVP_RECORD_START`, `PVP_RECORD_UPDATE` or `PVP_RECORD_FINISH`.
    pub record_type: i32,
    /// Java `_winnerTeam`: `0` none, `1` blue, `2` red.
    pub winner_team: i32,
    pub blue_kills: i32,
    pub red_kills: i32,
    pub blue: Vec<PvpMember>,
    pub red: Vec<PvpMember>,
}

impl ExPvpMatchRecord {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPvpMatchRecord;

    /// The winner code Java writes after the winner team: `0` for none, `2` for team 1 and `1`
    /// for any other team.
    pub fn winner_code(winner_team: i32) -> i32 {
        match winner_team {
            0 => 0,
            1 => 2,
            _ => 1,
        }
    }
}

impl Encode for ExPvpMatchRecord {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.record_type)
            .i32(self.winner_team)
            .i32(Self::winner_code(self.winner_team))
            .i32(self.blue_kills)
            .i32(self.red_kills);
        write_pvp_members(w, &self.blue);
        write_pvp_members(w, &self.red);
    }
}

impl Decode for ExPvpMatchRecord {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let record_type = r.i32()?;
        let winner_team = r.i32()?;
        let _winner_code = r.i32()?;
        let blue_kills = r.i32()?;
        let red_kills = r.i32()?;
        let blue = read_pvp_members(r)?;
        let red = read_pvp_members(r)?;
        Ok(Self {
            record_type,
            winner_team,
            blue_kills,
            red_kills,
            blue,
            red,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPVPMatchUserDie.java
server_packet!(
    /// A player of the Underground Coliseum died; the kills of both teams (`EX_PVP_MATCH_USER_DIE`).
    /// Java writes red first.
    ExPvpMatchUserDie,
    ExPvpMatchUserDie {
        red_kills: i32,
        blue_kills: i32,
    }
);

// Java: gameserver/network/serverpackets/ExStartScenePlayer.java
server_packet!(
    /// Starts a movie for the client (`EX_START_SCENE_PLAYER`).
    ExStartScenePlayer,
    ExStartScenePlayer {
        /// Java `_movie.getClientId()`.
        movie_id: i32,
    }
);

// Java: gameserver/network/serverpackets/ExSendUIEvent.java
/// A countdown or hidden timer on the client's screen (`EX_SEND_UI_EVENT`).
///
/// Java sends the start and end times as minute and second strings. `start_time` and `end_time`
/// are the seconds; the packet writes `seconds / 60` and `seconds % 60` (Java truncation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExSendUiEvent {
    pub object_id: i32,
    /// Java `_type`: `true` hides the timer, `false` shows it.
    pub hide: bool,
    /// `true` counts up, `false` counts down.
    pub count_up: bool,
    pub start_time: i32,
    pub end_time: i32,
    /// The client string id of the text, `-1` when the text is in `params`.
    pub npc_string_id: i32,
    pub params: Vec<String>,
}

impl ExSendUiEvent {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExSendUiEvent;
}

impl Encode for ExSendUiEvent {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(i32::from(self.hide))
            .i32(0)
            .i32(0)
            .string(if self.count_up { "1" } else { "0" })
            .string(&(self.start_time / 60).to_string())
            .string(&(self.start_time % 60).to_string())
            .string(&(self.end_time / 60).to_string())
            .string(&(self.end_time % 60).to_string())
            .i32(self.npc_string_id);
        for param in &self.params {
            w.string(param);
        }
    }
}

/// Reads one minute or second string of the timer.
fn read_timer_part(r: &mut PacketReader, field: &'static str) -> Result<i32, PacketError> {
    let text = r.string();
    text.parse::<i32>()
        .map_err(|_| PacketError::invalid(field, format!("not a number: {text:?}")))
}

impl Decode for ExSendUiEvent {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let hide = r.i32()? != 0;
        let _unknown_1 = r.i32()?;
        let _unknown_2 = r.i32()?;
        let count_up = r.string() == "1";
        let start_minutes = read_timer_part(r, "start minutes")?;
        let start_seconds = read_timer_part(r, "start seconds")?;
        let end_minutes = read_timer_part(r, "end minutes")?;
        let end_seconds = read_timer_part(r, "end seconds")?;
        let npc_string_id = r.i32()?;
        let mut params = Vec::new();
        while r.remaining() > 0 {
            let before = r.remaining();
            params.push(r.string());
            if r.remaining() == before {
                break;
            }
        }
        Ok(Self {
            object_id,
            hide,
            count_up,
            start_time: start_minutes.wrapping_mul(60).wrapping_add(start_seconds),
            end_time: end_minutes.wrapping_mul(60).wrapping_add(end_seconds),
            npc_string_id,
            params,
        })
    }
}
