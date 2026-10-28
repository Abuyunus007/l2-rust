//! The Gatekeeper Spirits of the Lilith and Anakim chambers (Seven Signs, Seal of Avarice).
// Java: dist/game/data/scripts/ai/others/GatekeeperSpirit/GatekeeperSpirit.java

use l2_util::ObjectId;
use l2_world::sevensigns::{CABAL_DAWN, CABAL_DUSK, SEAL_AVARICE};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GATEKEEPER_SPIRIT_ENTER: i32 = 31111;
const GATEKEEPER_SPIRIT_EXIT: i32 = 31112;
const LILITH: i32 = 25283;
const ANAKIM: i32 = 25286;

// Exit gatekeeper spawn locations
const SPAWN_LILITH_GATEKEEPER: (i32, i32, i32) = (184410, -10111, -5488);
const SPAWN_ANAKIM_GATEKEEPER: (i32, i32, i32) = (184410, -13102, -5488);

// Teleport
const TELEPORT_DUSK: (i32, i32, i32) = (184464, -13104, -5504);
const TELEPORT_DAWN: (i32, i32, i32) = (184448, -10112, -5504);
const EXIT: (i32, i32, i32) = (182960, -11904, -4897);

/// Java `ai.others.GatekeeperSpirit.GatekeeperSpirit`.
pub struct GatekeeperSpirit;

impl Script for GatekeeperSpirit {
    fn name(&self) -> &'static str {
        "GatekeeperSpirit"
    }

    fn path(&self) -> &'static str {
        "ai/others/GatekeeperSpirit"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GATEKEEPER_SPIRIT_ENTER, GATEKEEPER_SPIRIT_EXIT]);
        s.add_first_talk_id(&[GATEKEEPER_SPIRIT_ENTER, GATEKEEPER_SPIRIT_EXIT]);
        s.add_talk_id(&[GATEKEEPER_SPIRIT_ENTER, GATEKEEPER_SPIRIT_EXIT]);
        s.add_kill_id(&[LILITH, ANAKIM]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "ANAKIM" => {
                let (x, y, z) = SPAWN_ANAKIM_GATEKEEPER;
                ctx.add_spawn(GATEKEEPER_SPIRIT_EXIT, x, y, z, 0, false, 900_000, 0);
            }
            "LILITH" => {
                let (x, y, z) = SPAWN_LILITH_GATEKEEPER;
                ctx.add_spawn(GATEKEEPER_SPIRIT_EXIT, x, y, z, 0, false, 900_000, 0);
            }
            "ENTER" => {
                let player = player?;
                let player_cabal = ctx.ss_player_cabal(player);
                let seal_of_avarice_owner = ctx.ss_seal_owner(SEAL_AVARICE);
                let comp_winner = ctx.ss_cabal_highest_score();

                if !ctx.ss_is_seal_validation_period() {
                    htmltext = Some("31111-no.html".to_owned());
                } else if comp_winner == CABAL_DUSK
                    && player_cabal == CABAL_DUSK
                    && seal_of_avarice_owner == CABAL_DUSK
                {
                    ctx.teleport_player(player, TELEPORT_DUSK.0, TELEPORT_DUSK.1, TELEPORT_DUSK.2, 0, false);
                } else if comp_winner == CABAL_DAWN
                    && player_cabal == CABAL_DAWN
                    && seal_of_avarice_owner == CABAL_DAWN
                {
                    ctx.teleport_player(player, TELEPORT_DAWN.0, TELEPORT_DAWN.1, TELEPORT_DAWN.2, 0, false);
                } else {
                    htmltext = Some("31111-no.html".to_owned());
                }
            }
            "EXIT" => {
                let player = player?;
                ctx.teleport_player(player, EXIT.0, EXIT.1, EXIT.2, 0, true);
            }
            _ => {}
        }

        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            ANAKIM => ctx.start_quest_timer("ANAKIM", 10000, Some(npc), Some(killer), false),
            LILITH => ctx.start_quest_timer("LILITH", 10000, Some(npc), Some(killer), false),
            _ => {}
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::sevensigns::{
        PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SevenSignsLoad, SevenSignsStatus, SignsPlayer,
    };

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="31111" type="Folk" level="70" name="Gatekeeper Spirit"/>
        <npc id="31112" type="Folk" level="70" name="Gatekeeper Spirit"/>
        <npc id="25283" type="Monster" level="80" name="Lilith"/>"#;
    const DIR: &str = "data/scripts/ai/others/GatekeeperSpirit";
    const WEDNESDAY: i64 = 1_700_000_000_000;

    /// A dusk member in the validation period, dusk winning the Seal of Avarice, or not.
    fn setup(period: i32, winner_score: bool, owner: i32, cabal: &str) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(GatekeeperSpirit);
        let main = r#"<html><body>main<a action="bypass -h Script GatekeeperSpirit ENTER">e</a>
<a action="bypass -h Script GatekeeperSpirit EXIT">x</a></body></html>"#;
        let pages = [
            (format!("{DIR}/31111.html"), main.to_owned()),
            (format!("{DIR}/31111-no.html"), "<html><body>no</body></html>".to_owned()),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let gatekeeper = t.spawn(31111, 20);
        let mut status = SevenSignsStatus {
            current_cycle: 4,
            active_period: period,
            last_save: WEDNESDAY - 3_600_000,
            ..SevenSignsStatus::default()
        };
        status.seal_owners[SEAL_AVARICE as usize] = owner;
        if winner_score {
            status.dusk_stone_score = 100.0;
        }
        let players = vec![SignsPlayer {
            char_id: 1,
            cabal: cabal.into(),
            seal: SEAL_AVARICE,
            ..SignsPlayer::default()
        }];
        t.world.load_seven_signs_at(
            SevenSignsLoad {
                players,
                status: Some(status),
                ..SevenSignsLoad::default()
            },
            WEDNESDAY,
            |_| 0,
        );
        t.enter(1, 80, vec![]);
        t.talk(1, gatekeeper);
        t.take();
        (t, gatekeeper)
    }

    #[test]
    fn the_first_page_is_the_npc_id() {
        let (mut t, gatekeeper) = setup(PERIOD_COMPETITION, false, 0, "dusk");
        t.talk(1, gatekeeper);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("main"));
    }

    #[test]
    fn the_winner_who_owns_avarice_may_enter() {
        let (mut t, _) = setup(PERIOD_SEAL_VALIDATION, true, CABAL_DUSK, "dusk");
        t.bypass(1, "Script GatekeeperSpirit ENTER");
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!(at, (crate::testing::X, crate::testing::Y, crate::testing::Z));
        assert!(pages_to(&t.take(), 1).is_empty());
    }

    #[test]
    fn everybody_else_is_sent_away() {
        // Not the validation period.
        let (mut t, _) = setup(PERIOD_COMPETITION, true, CABAL_DUSK, "dusk");
        t.bypass(1, "Script GatekeeperSpirit ENTER");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("no"));
        // The other cabal.
        let (mut t, _) = setup(PERIOD_SEAL_VALIDATION, true, CABAL_DUSK, "dawn");
        t.bypass(1, "Script GatekeeperSpirit ENTER");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("no"));
        // Avarice owned by the other cabal.
        let (mut t, _) = setup(PERIOD_SEAL_VALIDATION, true, CABAL_DAWN, "dusk");
        t.bypass(1, "Script GatekeeperSpirit ENTER");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("no"));
    }

    #[test]
    fn exit_teleports_out() {
        let (mut t, _) = setup(PERIOD_COMPETITION, false, 0, "dusk");
        t.bypass(1, "Script GatekeeperSpirit EXIT");
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!(at, (crate::testing::X, crate::testing::Y, crate::testing::Z));
    }

    #[test]
    fn the_kill_of_lilith_opens_the_exit_ten_seconds_later() {
        let (mut t, _) = setup(PERIOD_COMPETITION, false, 0, "dusk");
        let lilith = t.world.spawn_npc_at(LILITH, crate::testing::X + 30, crate::testing::Y, crate::testing::Z, 0);
        let lilith = lilith.unwrap();
        let before = t.world.npc_count();
        t.world.script_on_attackable_kill(lilith, Some(ObjectId(1)));
        // 2.5 s of `onKillDelay`, then the 10 s timer: 12.5 s = 250 ticks.
        t.ticks(260);
        assert_eq!(t.world.npc_count(), before + 1);
        let spawned = t.world.npc_ids().into_iter().find(|id| t.world.npc(*id).unwrap().npc_id == GATEKEEPER_SPIRIT_EXIT);
        assert!(spawned.is_some());
    }
}
