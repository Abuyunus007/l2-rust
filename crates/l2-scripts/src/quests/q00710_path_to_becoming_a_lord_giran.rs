//! Quest 710: Path to Becoming a Lord - Giran (Chamberlain Saul, Gesto, Felton and the Zaken's freight, the Giran castle lord).
// Java: dist/game/data/scripts/quests/Q00710_PathToBecomingALordGiran/Q00710_PathToBecomingALordGiran.java
//
// Pattern: a lord quest for the owner of Giran castle (`castle.owner_id`, the clan whose leader
// is online): a cargo seal from the freight box, 300 Gesto's boxes from Zaken's soldiers (cond 7
// to 8), and a coronation shout of the lord's name when the clan leader is online.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SAUL: i32 = 35184;
const GESTO: i32 = 30511;
const FELTON: i32 = 30879;
const CARGO_BOX: i32 = 32243;

// Items
const FREIGHT_CHESTS_SEAL: i32 = 13014;
const GESTOS_BOX: i32 = 13013;

/// Java `MOBS`: Zaken's soldiers (the Gesto's box drop).
const MOBS: [i32; 11] = [
    20832, 20833, 20835, 21602, 21603, 21604, 21605, 21606, 21607, 21608, 21609,
];

/// Java `GIRAN_CASTLE`.
const GIRAN_CASTLE: CastleId = CastleId(3);

/// Java `Q00710_PathToBecomingALordGiran`.
pub struct Q00710PathToBecomingALordGiran;

/// Java `castle.getOwner()` of Giran: the owner clan, `None` without a lord. (The Java code
/// returns "Castle has no lord." when the castle is missing too; this server has no castle to
/// read in that case.)
fn giran_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(GIRAN_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `Q00710_PathToBecomingALordGiran.hasFort()`: a fortress is contracted with Giran.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == GIRAN_CASTLE.0)
}

impl Script for Q00710PathToBecomingALordGiran {
    fn name(&self) -> &'static str {
        "Q00710_PathToBecomingALordGiran"
    }

    fn path(&self) -> &'static str {
        "quests/Q00710_PathToBecomingALordGiran"
    }

    fn id(&self) -> i32 {
        710
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SAUL]);
        s.add_kill_id(&MOBS);
        s.add_talk_id(&[SAUL, GESTO, FELTON, CARGO_BOX]);
        s.register_quest_items(&[FREIGHT_CHESTS_SEAL, GESTOS_BOX]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java: "Castle has no lord." is returned as a chat line, before anything else.
        if giran_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(player, false);

        match event {
            "35184-03.html" => {
                if let Some(qs) = qs {
                    qs.start_quest(ctx);
                }
            }
            "30511-03.html" => {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 3, false);
                }
            }
            "30879-02.html" => {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 4, false);
                }
            }
            "35184-07.html" => {
                let lord_clan = giran_lord_clan(ctx).unwrap_or_default();
                // Java `castle.getOwner().getLeader().getPlayer() != null`: the lord's leader is online.
                if ctx.world().clan_online_leader(lord_clan).is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfGiranMayThereBeGloryInTheTerritoryOfGiran,
                            &[name.as_str()],
                        );
                    }
                    if let Some(qs) = qs {
                        qs.exit_quest(ctx, true, true);
                    }
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 7) {
            if ctx.get_quest_items_count(killer, GESTOS_BOX) < 300 {
                ctx.give_items(killer, GESTOS_BOX, 1);
            }
            if ctx.get_quest_items_count(killer, GESTOS_BOX) >= 300 {
                qs.set_cond(ctx, 8, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let Some(lord_clan) = giran_lord_clan(ctx) else {
            return Some("Castle has no lord.".to_owned());
        };
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        let castle_owner = ctx.world().clan_online_leader(lord_clan);

        match npc.id {
            SAUL => {
                // Java `isCond(0)`: a state with no row yet has cond 0 (see Q00713).
                if qs.get_cond(ctx) == 0 {
                    if castle_owner == Some(player) {
                        if has_fort(ctx) {
                            htmltext = Some("35184-00.html".to_owned());
                            qs.exit_quest(ctx, true, false);
                        } else {
                            htmltext = Some("35184-01.html".to_owned());
                        }
                    } else {
                        htmltext = Some("35184-00a.html".to_owned());
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some("35184-04.html".to_owned());
                } else if qs.is_cond(ctx, 2) {
                    htmltext = Some("35184-05.html".to_owned());
                } else if qs.is_cond(ctx, 9) {
                    htmltext = Some("35184-06.html".to_owned());
                }
            }
            GESTO => {
                if qs.is_cond(ctx, 2) {
                    htmltext = Some("30511-01.html".to_owned());
                } else if qs.is_cond(ctx, 3) || qs.is_cond(ctx, 4) {
                    htmltext = Some("30511-04.html".to_owned());
                } else if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, FREIGHT_CHESTS_SEAL, -1);
                    qs.set_cond(ctx, 7, false);
                    htmltext = Some("30511-05.html".to_owned());
                } else if qs.is_cond(ctx, 7) {
                    htmltext = Some("30511-06.html".to_owned());
                } else if qs.is_cond(ctx, 8) {
                    ctx.take_items(player, GESTOS_BOX, -1);
                    qs.set_cond(ctx, 9, false);
                    htmltext = Some("30511-07.html".to_owned());
                } else if qs.is_cond(ctx, 9) {
                    htmltext = Some("30511-07.html".to_owned());
                }
            }
            FELTON => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some("30879-01.html".to_owned());
                } else if qs.is_cond(ctx, 4) {
                    htmltext = Some("30879-03.html".to_owned());
                }
            }
            CARGO_BOX => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, false);
                    ctx.give_items(player, FREIGHT_CHESTS_SEAL, 1);
                    htmltext = Some("32243-01.html".to_owned());
                } else if qs.is_cond(ctx, 5) {
                    htmltext = Some("32243-02.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::residence::CastleRecord;
    use l2_world::{ResidenceLoad, Script, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00710_PathToBecomingALordGiran";

    const NPCS: &str = r#"
        <npc id="35184" type="Folk" level="1" name="Chamberlain Saul"/>
        <npc id="30511" type="Folk" level="1" name="Gesto"/>
        <npc id="30879" type="Folk" level="1" name="Felton"/>
        <npc id="32243" type="Folk" level="1" name="Freight Box"/>
        <npc id="20832" type="Monster" level="60" name="Zaken's Pikeman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13014" type="EtcItem" name="Freight Chest's Seal"><set name="is_questitem" val="true"/></item>
        <item id="13013" type="EtcItem" name="Gesto's Box"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00710PathToBecomingALordGiran);
        let pages = Pages::of(&Q00710PathToBecomingALordGiran)
            .plain(&[
                "35184-00.html",
                "35184-00a.html",
                "35184-01.html",
                "35184-04.html",
                "35184-05.html",
                "35184-06.html",
                "35184-07.html",
                "30511-01.html",
                "30511-03.html",
                "30511-04.html",
                "30511-05.html",
                "30511-06.html",
                "30511-07.html",
                "30879-01.html",
                "30879-03.html",
                "32243-01.html",
                "32243-02.html",
            ])
            .npcs(&[SAUL, GESTO, FELTON, CARGO_BOX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let saul = t.spawn(SAUL, 20);
        let gesto = t.spawn(GESTO, 30);
        (t, saul, gesto)
    }

    /// Giran castle with a lord: the clan of player 1, whose leader (player 1) is in the world.
    fn crown_giran(t: &mut ScriptTest) {
        t.world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 3,
                found: true,
                name: "Giran".to_owned(),
                ..CastleRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        let clan = t.world.create_clan(Oid(1), "GiranLords").unwrap();
        t.world.castle_set_owner(GIRAN_CASTLE, Some(clan));
        t.take();
        t.take_writes();
    }

    /// The talk hook of `npc` for `player`, called directly.
    fn talk(t: &mut ScriptTest, npc: Oid, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            out = Q00710PathToBecomingALordGiran.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    /// A quest link of the script for `player`, from `npc`.
    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00710PathToBecomingALordGiran.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    /// The cond of the player's state, set by the test (a step a Zaken's kill or a freight box
    /// would set).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(NAME, Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, cond, false);
        });
        t.take_writes();
    }

    #[test]
    fn the_lord_of_giran_starts_the_quest_and_gets_the_seal() {
        let (mut t, saul, gesto) = setup();
        t.enter(1, 40, vec![]);
        crown_giran(&mut t);

        assert_eq!(talk(&mut t, saul, 1).as_deref(), Some("35184-01.html"));
        assert_eq!(
            event(&mut t, saul, "35184-03.html", 1).as_deref(),
            Some("35184-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Saul sends the lord to Gesto (cond 2): Gesto's talk sets nothing, Saul's sets cond 2.
        assert_eq!(talk(&mut t, saul, 1).as_deref(), Some("35184-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(talk(&mut t, gesto, 1).as_deref(), Some("30511-01.html"));
        assert_eq!(
            event(&mut t, gesto, "30511-03.html", 1).as_deref(),
            Some("30511-03.html")
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
    }

    #[test]
    fn someone_else_than_the_lord_is_sent_away() {
        let (mut t, saul, _) = setup();
        t.enter(1, 40, vec![]);
        t.enter(2, 40, vec![]);
        crown_giran(&mut t);

        assert_eq!(talk(&mut t, saul, 2).as_deref(), Some("35184-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_gesto_boxes_and_the_coronation() {
        let (mut t, saul, gesto) = setup();
        t.enter(1, 40, vec![]);
        crown_giran(&mut t);
        set_cond(&mut t, 5);

        // The freight seal goes to Gesto (cond 7).
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), FREIGHT_CHESTS_SEAL, 1);
        });
        assert_eq!(talk(&mut t, gesto, 1).as_deref(), Some("30511-05.html"));
        assert_eq!(t.count(1, FREIGHT_CHESTS_SEAL), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "7")));

        // Zaken's soldiers drop the boxes: 300 of them make cond 8.
        let mut kills = 0;
        while t.count(1, GESTOS_BOX) < 300 {
            let soldier = t.spawn(20832, 60);
            t.kill(1, soldier);
            kills += 1;
            assert!(kills < 400, "too many kills");
        }
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
        assert_eq!(talk(&mut t, gesto, 1).as_deref(), Some("30511-07.html"));
        assert_eq!(t.count(1, GESTOS_BOX), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "9")));

        // The lord's coronation: the shout and the end.
        assert_eq!(talk(&mut t, saul, 1).as_deref(), Some("35184-06.html"));
        assert_eq!(
            event(&mut t, saul, "35184-07.html", 1).as_deref(),
            Some("35184-07.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
