//! Quest 713: Path to Becoming a Lord - Aden (Chamberlain Logan and High Priest Orven, the Aden castle lord).
// Java: dist/game/data/scripts/quests/Q00713_PathToBecomingALordAden/Q00713_PathToBecomingALordAden.java
//
// Pattern: a lord quest for the owner of Aden castle (`castle.owner_id`, the clan whose leader
// is online). Logan opens it to the lord only, Orven grants the support, and the coronation is
// a shout of the lord's name when the clan leader is online.
//
// FOLLOW-UP(engine): nothing in the Java server sets cond 4 (the Taik Orc kills count is never
// started), so the path stops at cond 2 until a GM changes it, as in Java.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LOGAN: i32 = 35274;
const ORVEN: i32 = 30857;

// Monsters: the Taik Orc supply leader and supply.
const MOBS: [i32; 2] = [20669, 20665];

/// Java `ADEN_CASTLE`.
const ADEN_CASTLE: CastleId = CastleId(5);

/// Java `Q00713_PathToBecomingALordAden`.
pub struct Q00713PathToBecomingALordAden;

/// Java `castle.getOwner()` of Aden: the owner clan, `None` without a lord. (The Java code
/// returns "Castle has no lord." when the castle is missing too; this server has no castle to
/// read in that case.)
fn aden_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(ADEN_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `Q00713_PathToBecomingALordAden.hasFort()`: a fortress is contracted with Aden.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == ADEN_CASTLE.0)
}

impl Script for Q00713PathToBecomingALordAden {
    fn name(&self) -> &'static str {
        "Q00713_PathToBecomingALordAden"
    }

    fn path(&self) -> &'static str {
        "quests/Q00713_PathToBecomingALordAden"
    }

    fn id(&self) -> i32 {
        713
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LOGAN]);
        s.add_kill_id(&MOBS);
        s.add_talk_id(&[LOGAN, ORVEN]);
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
        if aden_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(player, false);

        match event {
            "35274-02.html" => {
                if let Some(qs) = qs {
                    qs.start_quest(ctx);
                }
            }
            "30857-03.html" => {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 2, false);
                }
            }
            "35274-05.html" => {
                let lord_clan = aden_lord_clan(ctx).unwrap_or_default();
                // Java `castle.getOwner().getLeader().getPlayer() != null`: the lord's leader is online.
                if ctx.world().clan_online_leader(lord_clan).is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfAdenMayThereBeGloryInTheTerritoryOfAden,
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
        if qs.is_cond(ctx, 4) {
            let mobs = qs.get_int(ctx, "mobs");
            if mobs < 100 {
                qs.set_int(ctx, "mobs", mobs + 1);
            } else {
                qs.set_cond(ctx, 5, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let Some(lord_clan) = aden_lord_clan(ctx) else {
            return Some("Castle has no lord.".to_owned());
        };
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        let castle_owner = ctx.world().clan_online_leader(lord_clan);

        match npc.id {
            LOGAN => {
                if qs.is_cond(ctx, 0) {
                    if castle_owner == Some(player) {
                        if has_fort(ctx) {
                            htmltext = Some("35274-00.html".to_owned());
                            qs.exit_quest(ctx, true, false);
                        } else {
                            htmltext = Some("35274-01.html".to_owned());
                        }
                    } else {
                        htmltext = Some("35274-00a.html".to_owned());
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    htmltext = Some("35274-03.html".to_owned());
                } else if qs.is_cond(ctx, 7) {
                    htmltext = Some("35274-04.html".to_owned());
                }
            }
            ORVEN => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some("30857-01.html".to_owned());
                } else if qs.is_cond(ctx, 2) {
                    htmltext = Some("30857-04.html".to_owned());
                } else if qs.is_cond(ctx, 4) {
                    htmltext = Some("30857-05.html".to_owned());
                } else if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 7, false);
                    htmltext = Some("30857-06.html".to_owned());
                } else if qs.is_cond(ctx, 7) {
                    htmltext = Some("30857-06.html".to_owned());
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

    const NAME: &str = "Q00713_PathToBecomingALordAden";

    const NPCS: &str = r#"
        <npc id="35274" type="Folk" level="1" name="Chamberlain Logan"/>
        <npc id="30857" type="Folk" level="1" name="High Priest Orven"/>
        <npc id="20669" type="Monster" level="30" name="Taik Orc Supply Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00713PathToBecomingALordAden);
        let pages = Pages::of(&Q00713PathToBecomingALordAden)
            .plain(&[
                "35274-00.html",
                "35274-00a.html",
                "35274-01.html",
                "35274-02.html",
                "35274-03.html",
                "35274-04.html",
                "35274-05.html",
                "30857-01.html",
                "30857-02.html",
                "30857-03.html",
                "30857-04.html",
                "30857-05.html",
                "30857-06.html",
            ])
            .npcs(&[LOGAN, ORVEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let logan = t.spawn(LOGAN, 20);
        let orven = t.spawn(ORVEN, 30);
        (t, logan, orven)
    }

    /// Aden castle with a lord: the clan of player 1, whose leader (player 1) is in the world.
    fn crown_aden(t: &mut ScriptTest) {
        t.world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 5,
                found: true,
                name: "Aden".to_owned(),
                ..CastleRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        let clan = t.world.create_clan(Oid(1), "AdenLords").unwrap();
        t.world.castle_set_owner(CastleId(5), Some(clan));
        t.take();
        t.take_writes();
    }

    /// The NPC's talk hook for `player` (called directly: the click path of a castle NPC is not
    /// what these tests check).
    fn talk(t: &mut ScriptTest, npc: Oid, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            out = Q00713PathToBecomingALordAden.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    /// A quest link of the script for `player`, from `npc`.
    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00713PathToBecomingALordAden.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    #[test]
    fn the_lord_of_aden_starts_the_quest_and_orven_gives_cond_2() {
        let (mut t, logan, orven) = setup();
        t.enter(1, 40, vec![]);
        crown_aden(&mut t);

        assert_eq!(talk(&mut t, logan, 1).as_deref(), Some("35274-01.html"));
        assert_eq!(
            event(&mut t, logan, "35274-02.html", 1).as_deref(),
            Some("35274-02.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        assert_eq!(talk(&mut t, orven, 1).as_deref(), Some("30857-01.html"));
        assert_eq!(
            event(&mut t, orven, "30857-03.html", 1).as_deref(),
            Some("30857-03.html")
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(talk(&mut t, orven, 1).as_deref(), Some("30857-04.html"));
    }

    #[test]
    fn someone_else_than_the_lord_is_sent_away_and_the_quest_is_closed() {
        let (mut t, logan, _) = setup();
        t.enter(1, 40, vec![]);
        t.enter(2, 40, vec![]);
        crown_aden(&mut t);

        assert_eq!(talk(&mut t, logan, 2).as_deref(), Some("35274-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_coronation_shouts_the_lords_name_and_ends_the_quest() {
        let (mut t, logan, _) = setup();
        t.enter(1, 40, vec![]);
        crown_aden(&mut t);
        // Cond 7 is what Orven's kill chain leads to (cond 4 is never set by the Java server).
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(NAME, Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 7, false);
        });
        t.take_writes();

        assert_eq!(talk(&mut t, logan, 1).as_deref(), Some("35274-04.html"));
        assert_eq!(
            event(&mut t, logan, "35274-05.html", 1).as_deref(),
            Some("35274-05.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_hundred_and_one_taik_orcs_make_the_cond_5() {
        let (mut t, _, orven) = setup();
        t.enter(1, 40, vec![]);
        crown_aden(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(NAME, Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 4, false);
        });
        t.take_writes();

        // The first hundred kills count (`mobs` 1 to 100), the 101st sets cond 5.
        for _ in 0..101 {
            let orc = t.spawn(20669, 60);
            t.kill(1, orc);
        }
        assert_eq!(talk(&mut t, orven, 1).as_deref(), Some("30857-06.html"));
    }
}
