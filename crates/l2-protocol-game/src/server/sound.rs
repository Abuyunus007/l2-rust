//! `PlaySound`: the client plays a sound file. The first user is the duel start (`B04_S01`).

// Java: gameserver/network/serverpackets/PlaySound.java
server_packet!(
    /// Plays `sound_file` (`PLAY_SOUND`). Java names the ints `unknown1`, `unknown3` ... `unknown8`
    /// and only fills them for ship and quest sounds.
    PlaySound,
    PlaySound {
        /// Java `_unknown1`: "0 for quest and ship"; the duel start sends 1.
        unknown1: i32,
        sound_file: String,
        /// Java: "0 for quest; 1 for ship".
        unknown3: i32,
        /// Java: "0 for quest; objectId of ship".
        unknown4: i32,
        /// Java: x.
        unknown5: i32,
        /// Java: y.
        unknown6: i32,
        /// Java: z.
        unknown7: i32,
        /// Java: always 0.
        unknown8: i32,
    }
);

impl PlaySound {
    /// Java `new PlaySound(soundFile)`.
    pub fn file(sound_file: &str) -> Self {
        Self::with_unknown1(0, sound_file)
    }

    /// Java `new PlaySound(unknown1, soundFile, 0, 0, 0, 0, 0)`.
    pub fn with_unknown1(unknown1: i32, sound_file: &str) -> Self {
        Self {
            unknown1,
            sound_file: sound_file.to_owned(),
            unknown3: 0,
            unknown4: 0,
            unknown5: 0,
            unknown6: 0,
            unknown7: 0,
            unknown8: 0,
        }
    }
}
