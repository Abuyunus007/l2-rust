//! The Dawn and Dusk priests' hunting grounds teleports (Seven Signs): the page depends on the
//! Seal of Gnosis.
// Java: dist/game/data/scripts/ai/others/HuntingGroundsTeleport/HuntingGroundsTeleport.java

use l2_util::ObjectId;
use l2_world::sevensigns::{CABAL_NULL, SEAL_GNOSIS};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PRIESTS: [i32; 22] = [
    31078, 31079, 31080, 31081, 31082, 31083, 31084, 31085, 31086, 31087, 31088, 31089, 31090, 31091, 31168,
    31169, 31692, 31693, 31694, 31695, 31997, 31998,
];

const DAWN_NPCS: [i32; 11] = [
    31078, 31079, 31080, 31081, 31082, 31083, 31084, 31168, 31692, 31694, 31997,
];

/// Java `ai.others.HuntingGroundsTeleport.HuntingGroundsTeleport`.
pub struct HuntingGroundsTeleport;

impl Script for HuntingGroundsTeleport {
    fn name(&self) -> &'static str {
        "HuntingGroundsTeleport"
    }

    fn path(&self) -> &'static str {
        "ai/others/HuntingGroundsTeleport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&PRIESTS);
        s.add_talk_id(&PRIESTS);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let player_cabal = ctx.ss_player_cabal(player);
        if player_cabal == CABAL_NULL {
            return Some(if DAWN_NPCS.contains(&npc.id) { "dawn_tele-no.htm" } else { "dusk_tele-no.htm" }.to_owned());
        }

        let mut htmltext = "";
        let check = ctx.ss_is_seal_validation_period()
            && player_cabal == ctx.ss_seal_owner(SEAL_GNOSIS)
            && ctx.ss_player_seal(player) == SEAL_GNOSIS;
        // (low page when the player holds the Seal of Gnosis, else the hunting grounds page)
        let pick = |low: &'static str, hg: &'static str| if check { low } else { hg };
        match npc.id {
            31078 | 31085 => htmltext = pick("low_gludin.htm", "hg_gludin.htm"),
            31079 | 31086 => htmltext = pick("low_gludio.htm", "hg_gludio.htm"),
            31080 | 31087 => htmltext = pick("low_dion.htm", "hg_dion.htm"),
            31081 | 31088 => htmltext = pick("low_giran.htm", "hg_giran.htm"),
            31082 | 31089 => htmltext = pick("low_heine.htm", "hg_heine.htm"),
            31083 | 31090 => htmltext = pick("low_oren.htm", "hg_oren.htm"),
            31084 | 31091 => htmltext = pick("low_aden.htm", "hg_aden.htm"),
            31168 | 31169 => htmltext = pick("low_hw.htm", "hg_hw.htm"),
            31692 | 31693 => htmltext = pick("low_goddard.htm", "hg_goddard.htm"),
            31694 | 31695 => htmltext = pick("low_rune.htm", "hg_rune.htm"),
            31997 | 31998 => htmltext = pick("low_schuttgart.htm", "hg_schuttgart.htm"),
            _ => {}
        }

        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::sevensigns::{
        CABAL_DAWN, CABAL_DUSK, PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SevenSignsLoad, SevenSignsStatus,
        SignsPlayer,
    };

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="31078" type="Folk" level="70" name="Priest of Dawn"/>
        <npc id="31085" type="Folk" level="70" name="Priest of Dusk"/>"#;
    const DIR: &str = "data/scripts/ai/others/HuntingGroundsTeleport";
    const WEDNESDAY: i64 = 1_700_000_000_000;

    fn setup(period: i32, gnosis_owner: i32, cabal: &str, seal: i32) -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HuntingGroundsTeleport);
        let mut pages = Vec::new();
        for n in ["dawn_tele-no", "dusk_tele-no", "hg_gludin", "low_gludin"] {
            pages.push((format!("{DIR}/{n}.htm"), format!("<html><body>{n}</body></html>")));
        }
        for npc in [31078, 31085] {
            pages.push((
                format!("data/html/default/{npc}.htm"),
                r#"<html><body>priest <a action="bypass Script HuntingGroundsTeleport">Talk</a></body></html>"#
                    .to_owned(),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let dawn = t.spawn(31078, 20);
        let dusk = t.spawn(31085, 40);
        let mut status = SevenSignsStatus {
            current_cycle: 4,
            active_period: period,
            last_save: WEDNESDAY - 3_600_000,
            ..SevenSignsStatus::default()
        };
        status.seal_owners[SEAL_GNOSIS as usize] = gnosis_owner;
        let players = if cabal.is_empty() {
            vec![]
        } else {
            vec![SignsPlayer {
                char_id: 1,
                cabal: cabal.into(),
                seal,
                ..SignsPlayer::default()
            }]
        };
        t.world.load_seven_signs_at(
            SevenSignsLoad {
                players,
                status: Some(status),
                ..SevenSignsLoad::default()
            },
            WEDNESDAY,
            |_| 0,
        );
        t.enter(1, 60, vec![]);
        t.take();
        (t, dawn, dusk)
    }

    /// Clicks the priest, then his `Script` link; the page it ends on.
    fn talk(t: &mut ScriptTest, npc: ObjectId) -> String {
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script HuntingGroundsTeleport");
        pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    #[test]
    fn a_player_without_a_cabal_gets_the_no_page_of_the_priest_side() {
        let (mut t, dawn, dusk) = setup(PERIOD_COMPETITION, 0, "", 0);
        assert!(talk(&mut t, dawn).contains("dawn_tele-no"));
        assert!(talk(&mut t, dusk).contains("dusk_tele-no"));
    }

    #[test]
    fn a_member_gets_the_hunting_grounds_page() {
        let (mut t, dawn, dusk) = setup(PERIOD_COMPETITION, 0, "dawn", SEAL_GNOSIS);
        assert!(talk(&mut t, dawn).contains("hg_gludin"));
        assert!(talk(&mut t, dusk).contains("hg_gludin"));
    }

    #[test]
    fn the_gnosis_owner_gets_the_low_price_page_in_the_validation_period() {
        let (mut t, dawn, _) = setup(PERIOD_SEAL_VALIDATION, CABAL_DAWN, "dawn", SEAL_GNOSIS);
        assert!(talk(&mut t, dawn).contains("low_gludin"));
        // The other cabal owns the seal: the plain page.
        let (mut t, dawn, _) = setup(PERIOD_SEAL_VALIDATION, CABAL_DUSK, "dawn", SEAL_GNOSIS);
        assert!(talk(&mut t, dawn).contains("hg_gludin"));
        // Not the member's seal.
        let (mut t, dawn, _) = setup(PERIOD_SEAL_VALIDATION, CABAL_DAWN, "dawn", 1);
        assert!(talk(&mut t, dawn).contains("hg_gludin"));
    }
}
