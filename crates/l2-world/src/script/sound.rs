//! Java `QuestSound`: the named sounds of the quest scripts.
// Java: mechanics/script/QuestSound.java

use l2_protocol_game::server::PlaySound;

/// Java `QuestSound`: a `PlaySound` with a fixed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSound {
    /// `ITEMSOUND_QUEST_ACCEPT`: `ItemSound.quest_accept`.
    ItemsoundQuestAccept,
    /// `ITEMSOUND_QUEST_MIDDLE`: `ItemSound.quest_middle`.
    ItemsoundQuestMiddle,
    /// `ITEMSOUND_QUEST_FINISH`: `ItemSound.quest_finish`.
    ItemsoundQuestFinish,
    /// `ITEMSOUND_QUEST_ITEMGET`: `ItemSound.quest_itemget`.
    ItemsoundQuestItemget,
    /// `ITEMSOUND_QUEST_TUTORIAL`: `ItemSound.quest_tutorial`.
    ItemsoundQuestTutorial,
    /// `ITEMSOUND_QUEST_GIVEUP`: `ItemSound.quest_giveup`.
    ItemsoundQuestGiveup,
    /// `ITEMSOUND_QUEST_BEFORE_BATTLE`: `ItemSound.quest_before_battle`.
    ItemsoundQuestBeforeBattle,
    /// `ITEMSOUND_QUEST_JACKPOT`: `ItemSound.quest_jackpot`.
    ItemsoundQuestJackpot,
    /// `ITEMSOUND_QUEST_FANFARE_1`: `ItemSound.quest_fanfare_1`.
    ItemsoundQuestFanfare1,
    /// `ITEMSOUND_QUEST_FANFARE_2`: `ItemSound.quest_fanfare_2`.
    ItemsoundQuestFanfare2,
    /// `ITEMSOUND_QUEST_FANFARE_MIDDLE`: `ItemSound.quest_fanfare_middle`.
    ItemsoundQuestFanfareMiddle,
    /// `ITEMSOUND_ARMOR_WOOD`: `ItemSound.armor_wood_3`.
    ItemsoundArmorWood,
    /// `ITEMSOUND_ARMOR_CLOTH`: `ItemSound.item_drop_equip_armor_cloth`.
    ItemsoundArmorCloth,
    /// `AMDSOUND_ED_CHIMES`: `AmdSound.ed_chimes_05`.
    AmdsoundEdChimes,
    /// `HORROR_01`: `horror_01`.
    Horror01,
    /// `AMBSOUND_HORROR_01`: `AmbSound.dd_horror_01`.
    AmbsoundHorror01,
    /// `AMBSOUND_HORROR_03`: `AmbSound.d_horror_03`.
    AmbsoundHorror03,
    /// `AMBSOUND_HORROR_15`: `AmbSound.d_horror_15`.
    AmbsoundHorror15,
    /// `ITEMSOUND_ARMOR_LEATHER`: `ItemSound.itemdrop_armor_leather`.
    ItemsoundArmorLeather,
    /// `ITEMSOUND_WEAPON_SPEAR`: `ItemSound.itemdrop_weapon_spear`.
    ItemsoundWeaponSpear,
    /// `AMBSOUND_MT_CREAK`: `AmbSound.mt_creak01`.
    AmbsoundMtCreak,
    /// `AMBSOUND_EG_DRON`: `AmbSound.eg_dron_02`.
    AmbsoundEgDron,
    /// `SKILLSOUND_HORROR_02`: `SkillSound5.horror_02`.
    SkillsoundHorror02,
    /// `CHRSOUND_MHFIGHTER_CRY`: `ChrSound.MHFighter_cry`.
    ChrsoundMhfighterCry,
    /// `AMDSOUND_WIND_LOOT`: `AmdSound.d_wind_loot_02`.
    AmdsoundWindLoot,
    /// `INTERFACESOUND_CHARSTAT_OPEN`: `InterfaceSound.charstat_open_01`.
    InterfacesoundCharstatOpen,
    /// `AMDSOUND_HORROR_02`: `AmdSound.dd_horror_02`.
    AmdsoundHorror02,
    /// `CHRSOUND_FDELF_CRY`: `ChrSound.FDElf_Cry`.
    ChrsoundFdelfCry,
    /// `AMBSOUND_WINGFLAP`: `AmbSound.t_wingflap_04`.
    AmbsoundWingflap,
    /// `AMBSOUND_THUNDER`: `AmbSound.thunder_02`.
    AmbsoundThunder,
    /// `AMBSOUND_DRONE`: `AmbSound.ed_drone_02`.
    AmbsoundDrone,
    /// `AMBSOUND_CRYSTAL_LOOP`: `AmbSound.cd_crystal_loop`.
    AmbsoundCrystalLoop,
    /// `AMBSOUND_PERCUSSION_01`: `AmbSound.dt_percussion_01`.
    AmbsoundPercussion01,
    /// `AMBSOUND_PERCUSSION_02`: `AmbSound.ac_percussion_02`.
    AmbsoundPercussion02,
    /// `ITEMSOUND_BROKEN_KEY`: `ItemSound2.broken_key`.
    ItemsoundBrokenKey,
    /// `ITEMSOUND_SIREN`: `ItemSound3.sys_siren`.
    ItemsoundSiren,
    /// `ITEMSOUND_ENCHANT_SUCCESS`: `ItemSound3.sys_enchant_success`.
    ItemsoundEnchantSuccess,
    /// `ITEMSOUND_ENCHANT_FAILED`: `ItemSound3.sys_enchant_failed`.
    ItemsoundEnchantFailed,
    /// `ITEMSOUND_SOW_SUCCESS`: `ItemSound3.sys_sow_success`.
    ItemsoundSowSuccess,
    /// `SKILLSOUND_HORROR_1`: `SkillSound5.horror_01`.
    SkillsoundHorror1,
    /// `SKILLSOUND_HORROR_2`: `SkillSound5.horror_02`.
    SkillsoundHorror2,
    /// `SKILLSOUND_ANTARAS_FEAR`: `SkillSound3.antaras_fear`.
    SkillsoundAntarasFear,
    /// `SKILLSOUND_JEWEL_CELEBRATE`: `SkillSound2.jewel.celebrate`.
    SkillsoundJewelCelebrate,
    /// `SKILLSOUND_LIQUID_MIX`: `SkillSound5.liquid_mix_01`.
    SkillsoundLiquidMix,
    /// `SKILLSOUND_LIQUID_SUCCESS`: `SkillSound5.liquid_success_01`.
    SkillsoundLiquidSuccess,
    /// `SKILLSOUND_LIQUID_FAIL`: `SkillSound5.liquid_fail_01`.
    SkillsoundLiquidFail,
    /// `ETCSOUND_ELROKI_SONG_FULL`: `EtcSound.elcroki_song_full`.
    EtcsoundElrokiSongFull,
    /// `ETCSOUND_ELROKI_SONG_1ST`: `EtcSound.elcroki_song_1st`.
    EtcsoundElrokiSong1st,
    /// `ETCSOUND_ELROKI_SONG_2ND`: `EtcSound.elcroki_song_2nd`.
    EtcsoundElrokiSong2nd,
    /// `ETCSOUND_ELROKI_SONG_3RD`: `EtcSound.elcroki_song_3rd`.
    EtcsoundElrokiSong3rd,
    /// `BS01_A`: `BS01_A`.
    Bs01A,
    /// `BS02_A`: `BS02_A`.
    Bs02A,
    /// `BS03_A`: `BS03_A`.
    Bs03A,
    /// `BS04_A`: `BS04_A`.
    Bs04A,
    /// `BS06_A`: `BS06_A`.
    Bs06A,
    /// `BS07_A`: `BS07_A`.
    Bs07A,
    /// `BS08_A`: `BS08_A`.
    Bs08A,
    /// `BS01_D`: `BS01_D`.
    Bs01D,
    /// `BS02_D`: `BS02_D`.
    Bs02D,
    /// `BS05_D`: `BS05_D`.
    Bs05D,
    /// `BS07_D`: `BS07_D`.
    Bs07D,
}

impl QuestSound {
    /// Every sound, in Java's declaration order.
    pub const ALL: [Self; 61] = [
        Self::ItemsoundQuestAccept,
        Self::ItemsoundQuestMiddle,
        Self::ItemsoundQuestFinish,
        Self::ItemsoundQuestItemget,
        Self::ItemsoundQuestTutorial,
        Self::ItemsoundQuestGiveup,
        Self::ItemsoundQuestBeforeBattle,
        Self::ItemsoundQuestJackpot,
        Self::ItemsoundQuestFanfare1,
        Self::ItemsoundQuestFanfare2,
        Self::ItemsoundQuestFanfareMiddle,
        Self::ItemsoundArmorWood,
        Self::ItemsoundArmorCloth,
        Self::AmdsoundEdChimes,
        Self::Horror01,
        Self::AmbsoundHorror01,
        Self::AmbsoundHorror03,
        Self::AmbsoundHorror15,
        Self::ItemsoundArmorLeather,
        Self::ItemsoundWeaponSpear,
        Self::AmbsoundMtCreak,
        Self::AmbsoundEgDron,
        Self::SkillsoundHorror02,
        Self::ChrsoundMhfighterCry,
        Self::AmdsoundWindLoot,
        Self::InterfacesoundCharstatOpen,
        Self::AmdsoundHorror02,
        Self::ChrsoundFdelfCry,
        Self::AmbsoundWingflap,
        Self::AmbsoundThunder,
        Self::AmbsoundDrone,
        Self::AmbsoundCrystalLoop,
        Self::AmbsoundPercussion01,
        Self::AmbsoundPercussion02,
        Self::ItemsoundBrokenKey,
        Self::ItemsoundSiren,
        Self::ItemsoundEnchantSuccess,
        Self::ItemsoundEnchantFailed,
        Self::ItemsoundSowSuccess,
        Self::SkillsoundHorror1,
        Self::SkillsoundHorror2,
        Self::SkillsoundAntarasFear,
        Self::SkillsoundJewelCelebrate,
        Self::SkillsoundLiquidMix,
        Self::SkillsoundLiquidSuccess,
        Self::SkillsoundLiquidFail,
        Self::EtcsoundElrokiSongFull,
        Self::EtcsoundElrokiSong1st,
        Self::EtcsoundElrokiSong2nd,
        Self::EtcsoundElrokiSong3rd,
        Self::Bs01A,
        Self::Bs02A,
        Self::Bs03A,
        Self::Bs04A,
        Self::Bs06A,
        Self::Bs07A,
        Self::Bs08A,
        Self::Bs01D,
        Self::Bs02D,
        Self::Bs05D,
        Self::Bs07D,
    ];

    /// Java `getSoundName()`.
    pub fn sound_name(self) -> &'static str {
        match self {
            Self::ItemsoundQuestAccept => "ItemSound.quest_accept",
            Self::ItemsoundQuestMiddle => "ItemSound.quest_middle",
            Self::ItemsoundQuestFinish => "ItemSound.quest_finish",
            Self::ItemsoundQuestItemget => "ItemSound.quest_itemget",
            Self::ItemsoundQuestTutorial => "ItemSound.quest_tutorial",
            Self::ItemsoundQuestGiveup => "ItemSound.quest_giveup",
            Self::ItemsoundQuestBeforeBattle => "ItemSound.quest_before_battle",
            Self::ItemsoundQuestJackpot => "ItemSound.quest_jackpot",
            Self::ItemsoundQuestFanfare1 => "ItemSound.quest_fanfare_1",
            Self::ItemsoundQuestFanfare2 => "ItemSound.quest_fanfare_2",
            Self::ItemsoundQuestFanfareMiddle => "ItemSound.quest_fanfare_middle",
            Self::ItemsoundArmorWood => "ItemSound.armor_wood_3",
            Self::ItemsoundArmorCloth => "ItemSound.item_drop_equip_armor_cloth",
            Self::AmdsoundEdChimes => "AmdSound.ed_chimes_05",
            Self::Horror01 => "horror_01",
            Self::AmbsoundHorror01 => "AmbSound.dd_horror_01",
            Self::AmbsoundHorror03 => "AmbSound.d_horror_03",
            Self::AmbsoundHorror15 => "AmbSound.d_horror_15",
            Self::ItemsoundArmorLeather => "ItemSound.itemdrop_armor_leather",
            Self::ItemsoundWeaponSpear => "ItemSound.itemdrop_weapon_spear",
            Self::AmbsoundMtCreak => "AmbSound.mt_creak01",
            Self::AmbsoundEgDron => "AmbSound.eg_dron_02",
            Self::SkillsoundHorror02 => "SkillSound5.horror_02",
            Self::ChrsoundMhfighterCry => "ChrSound.MHFighter_cry",
            Self::AmdsoundWindLoot => "AmdSound.d_wind_loot_02",
            Self::InterfacesoundCharstatOpen => "InterfaceSound.charstat_open_01",
            Self::AmdsoundHorror02 => "AmdSound.dd_horror_02",
            Self::ChrsoundFdelfCry => "ChrSound.FDElf_Cry",
            Self::AmbsoundWingflap => "AmbSound.t_wingflap_04",
            Self::AmbsoundThunder => "AmbSound.thunder_02",
            Self::AmbsoundDrone => "AmbSound.ed_drone_02",
            Self::AmbsoundCrystalLoop => "AmbSound.cd_crystal_loop",
            Self::AmbsoundPercussion01 => "AmbSound.dt_percussion_01",
            Self::AmbsoundPercussion02 => "AmbSound.ac_percussion_02",
            Self::ItemsoundBrokenKey => "ItemSound2.broken_key",
            Self::ItemsoundSiren => "ItemSound3.sys_siren",
            Self::ItemsoundEnchantSuccess => "ItemSound3.sys_enchant_success",
            Self::ItemsoundEnchantFailed => "ItemSound3.sys_enchant_failed",
            Self::ItemsoundSowSuccess => "ItemSound3.sys_sow_success",
            Self::SkillsoundHorror1 => "SkillSound5.horror_01",
            Self::SkillsoundHorror2 => "SkillSound5.horror_02",
            Self::SkillsoundAntarasFear => "SkillSound3.antaras_fear",
            Self::SkillsoundJewelCelebrate => "SkillSound2.jewel.celebrate",
            Self::SkillsoundLiquidMix => "SkillSound5.liquid_mix_01",
            Self::SkillsoundLiquidSuccess => "SkillSound5.liquid_success_01",
            Self::SkillsoundLiquidFail => "SkillSound5.liquid_fail_01",
            Self::EtcsoundElrokiSongFull => "EtcSound.elcroki_song_full",
            Self::EtcsoundElrokiSong1st => "EtcSound.elcroki_song_1st",
            Self::EtcsoundElrokiSong2nd => "EtcSound.elcroki_song_2nd",
            Self::EtcsoundElrokiSong3rd => "EtcSound.elcroki_song_3rd",
            Self::Bs01A => "BS01_A",
            Self::Bs02A => "BS02_A",
            Self::Bs03A => "BS03_A",
            Self::Bs04A => "BS04_A",
            Self::Bs06A => "BS06_A",
            Self::Bs07A => "BS07_A",
            Self::Bs08A => "BS08_A",
            Self::Bs01D => "BS01_D",
            Self::Bs02D => "BS02_D",
            Self::Bs05D => "BS05_D",
            Self::Bs07D => "BS07_D",
        }
    }

    /// Java `getPacket()`: `new PlaySound(soundName)`.
    pub fn packet(self) -> PlaySound {
        PlaySound::file(self.sound_name())
    }
}

/// Java `QuestSound.getSound(soundName)`: the packet of a known sound, or a new
/// `PlaySound(soundName)`; both are the same packet.
pub fn sound_packet(sound_name: &str) -> PlaySound {
    PlaySound::file(sound_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quest_sounds_are_javas_files() {
        assert_eq!(
            QuestSound::ItemsoundQuestAccept.sound_name(),
            "ItemSound.quest_accept"
        );
        assert_eq!(
            QuestSound::ItemsoundQuestMiddle.sound_name(),
            "ItemSound.quest_middle"
        );
        assert_eq!(
            QuestSound::ItemsoundQuestFinish.sound_name(),
            "ItemSound.quest_finish"
        );
        assert_eq!(
            QuestSound::ItemsoundQuestItemget.sound_name(),
            "ItemSound.quest_itemget"
        );
        assert_eq!(QuestSound::ALL.len(), 61);
        assert_eq!(
            QuestSound::ItemsoundQuestAccept.packet(),
            sound_packet("ItemSound.quest_accept")
        );
    }
}
