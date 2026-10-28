//! `data/scripts/ai/areas/Gracia/AI/NPC`: the NPCs of Gracia.

pub mod abyss_gaze;
pub mod ekimus_mouth;
pub mod fortune_telling;
pub mod general_dilios;
pub mod klemis;
pub mod lekon;
pub mod nemo;
pub mod nottingale;
pub mod seyo;
pub mod zealot_of_shilen;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(abyss_gaze::AbyssGaze);
    r.add(ekimus_mouth::EkimusMouth);
    r.add(fortune_telling::FortuneTelling);
    r.add(general_dilios::GeneralDilios);
    r.add(klemis::Klemis);
    r.add(lekon::Lekon);
    r.add(nemo::Nemo);
    r.add(nottingale::Nottingale);
    r.add(seyo::Seyo);
    r.add(zealot_of_shilen::ZealotOfShilen);
}
