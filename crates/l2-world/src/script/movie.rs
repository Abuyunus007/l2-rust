//! The movie holder: `Quest.playMovie` and `EndScenePlayer` (S-API-03).
// Java: entity/actor/holders/player/MovieHolder.java, entity/actor/Player.java (playMovie,
//       stopMovie, getMovieHolder), network/clientpackets/EndScenePlayer.java

use l2_protocol_game::server::ExStartScenePlayer;
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `new MovieHolder(players, movie)`: every player gets `Player.playMovie(holder)`.
    pub fn script_play_movie(&mut self, players: &[ObjectId], movie_id: i32) {
        for &player in players {
            self.player_play_movie(player, movie_id);
        }
    }

    /// Java `Player.playMovie(holder)`: a player who is in a movie already is skipped; else the
    /// attack stops (not the cast: "Confirmed in retail, playing a movie does not abort cast"),
    /// the player stops moving, holds the movie and, unless teleporting, the client is told.
    fn player_play_movie(&mut self, player: ObjectId, movie_id: i32) {
        if !self.players.contains_key(&player) || self.scripts.movies.contains_key(&player) {
            return;
        }
        self.abort_attack(player);
        self.stop_move(player, None);
        self.scripts.movies.insert(player, movie_id);
        if !self.is_teleporting(player) {
            self.send(player, &ExStartScenePlayer { movie_id });
        }
    }

    /// Java `Player.getMovieHolder() != null`.
    pub fn is_playing_movie(&self, player: ObjectId) -> bool {
        self.scripts.movies.contains_key(&player)
    }

    /// Java `EndScenePlayer.runImpl`: the client says the scene ended.
    pub(crate) fn on_end_scene_player(&mut self, player: ObjectId, movie_id: i32) {
        if !self.players.contains_key(&player) || movie_id == 0 {
            return;
        }
        // The holder's movie must be the one the client reports (`getClientId`).
        if self.scripts.movies.get(&player) != Some(&movie_id) {
            return;
        }
        self.scripts.movies.remove(&player);
        // `setTeleporting(true, false); decayMe(); spawnMe(x, y, z); setTeleporting(false, false)`
        // "to avoid to get player removed from World": the table entry stays here anyway.
        if let Some((x, y, z)) = self.position_of(player) {
            self.decay_keep_entry(player);
            self.spawn_at(player, x, y, z);
        }
    }

    /// A player leaving forgets the movie (the holder lives in the `Player`).
    pub(crate) fn script_forget_movie(&mut self, player: ObjectId) {
        self.scripts.movies.remove(&player);
    }
}
