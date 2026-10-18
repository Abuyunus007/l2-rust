//! What the world hands to the persistence task for one player.
// Java: Player.storeCharBase (the `onlinetime` part), PlayerAutoSaveTaskManager
//
// Only data the persistence task is allowed to write travels in a [`PlayerData`]: the columns
// and tables M2 models (D-39: the Java game server shares the database).

use crate::player::{Player, PlayerData};
use crate::world::TICK;

/// The player's data with the live values copied back and the current session added to the
/// total online time (Java: `_onlineTime + (now - _onlineBeginTime) / 1000`).
///
/// `PlayerData::online_time` of the player in the world stays the value loaded from the
/// database, so every snapshot adds the whole session and never counts it twice.
pub fn snapshot_for_store(player: &Player, now_tick: u64) -> PlayerData {
    let mut snapshot = player.snapshot();
    let session_ticks = now_tick.saturating_sub(player.entered_at_tick);
    let session_secs = session_ticks * TICK.as_millis() as u64 / 1000;
    snapshot.online_time += i64::try_from(session_secs).unwrap_or(0);
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerData;
    use l2_util::ObjectId;

    fn data() -> PlayerData {
        PlayerData {
            object_id: ObjectId(1),
            account: "a".into(),
            name: "n".into(),
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
            x: 1,
            y: 2,
            z: 3,
            heading: 4,
            cur_hp: 1.0,
            max_hp: 1.0,
            cur_mp: 1.0,
            max_mp: 1.0,
            cur_cp: 1.0,
            max_cp: 1.0,
            online_time: 100,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        }
    }

    #[test]
    fn adds_the_session_to_the_online_time_without_changing_the_player() {
        let mut player = Player::from_data(data(), 10);
        player.core.x = 99;
        // 10 + 1200 ticks of 50 ms = 60 s
        let snapshot = snapshot_for_store(&player, 1210);
        assert_eq!(snapshot.online_time, 160);
        assert_eq!(snapshot.x, 99);
        assert_eq!(player.data.online_time, 100);
        // A later snapshot adds the whole session again, not the difference.
        assert_eq!(snapshot_for_store(&player, 2410).online_time, 220);
    }
}
