//! Quest 712: Path to Becoming a Lord - Oren (Chamberlain Brasseur, Croop, Marty and Valleria,
//! the Oren castle lord).
// Java: dist/game/data/scripts/quests/Q00712_PathToBecomingALordOren/Q00712_PathToBecomingALordOren.java
//
// Pattern: a lord quest with a helper (`isLordAvailable`: the lord is online, is not the
// talker, the talker is in the lord's clan and the lord's cond is the expected one). A clan
// member starts it with Marty (lord cond 3 to 4) and ends its own part with Valleria (lord
// cond 4 to 5); the lord then collects 300 Nebulite Orbs from the Oel Mahums.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BRASSEUR: i32 = 35226;
const CROOP: i32 = 30676;
const MARTY: i32 = 30169;
const VALLERIA: i32 = 30176;

// Item
const NEBULITE_ORB: i32 = 13851;

const OEL_MAHUMS: [i32; 2] = [
    20575, // Oel Mahum Warrior
    20576, // Oel Mahum Witch Doctor
];

/// Java `OREN_CASTLE`.
const OREN_CASTLE: CastleId = CastleId(4);

const NAME: &str = "Q00712_PathToBecomingALordOren";

/// Java `Q00712_PathToBecomingALordOren`.
pub struct Q00712PathToBecomingALordOren;

/// Java `castle.getOwner()` of Oren: the owner clan, `None` without a lord.
fn oren_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(OREN_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when the clan leader is online.
fn castle_owner(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    ctx.world().clan_online_leader(oren_lord_clan(ctx)?)
}

/// Java `hasFort()`: a fortress is contracted with Oren.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == OREN_CASTLE.0)
}

/// Java `isLordAvailable(cond, qs)`: the lord's id when it is.
fn is_lord_available(ctx: &mut ScriptCtx<'_>, cond: i32, player: ObjectId) -> Option<ObjectId> {
    let owner = oren_lord_clan(ctx)?;
    let lord = castle_owner(ctx)?;
    if lord == player || ctx.world().clan_id_of(player) != owner {
        return None;
    }
    let lord_qs = ctx.quest_state_of(NAME, lord, false)?;
    lord_qs.is_cond(ctx, cond).then_some(lord)
}

impl Script for Q00712PathToBecomingALordOren {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00712_PathToBecomingALordOren"
    }

    fn id(&self) -> i32 {
        712
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BRASSEUR, MARTY]);
        s.add_kill_id(&OEL_MAHUMS);
        s.add_talk_id(&[BRASSEUR, CROOP, MARTY, VALLERIA]);
        s.register_quest_items(&[NEBULITE_ORB]);
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
        if oren_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let lord = castle_owner(ctx);
        let qs = ctx.get_quest_state(player, false);

        match event {
            "35226-03.html" => qs?.start_quest(ctx),
            "30676-03.html" => qs?.set_cond(ctx, 3, false),
            "30169-02.html" => {
                if let Some(lord) = is_lord_available(ctx, 3, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 4, false);
                    }
                    qs?.set_state(ctx, STARTED);
                }
            }
            "30176-02.html" => {
                if let Some(lord) = is_lord_available(ctx, 4, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 5, false);
                    }
                    qs?.exit_quest(ctx, true, false);
                }
            }
            "30676-05.html" => qs?.set_cond(ctx, 6, false),
            "30676-07.html" => {
                ctx.take_items(player, NEBULITE_ORB, -1);
                qs?.set_cond(ctx, 8, false);
            }
            "35226-06.html" => {
                if lord.is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfOrenMayThereBeGloryInTheTerritoryOfOren,
                            &[name.as_str()],
                        );
                    }
                    qs?.exit_quest(ctx, true, true);
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
        if qs.is_cond(ctx, 6) {
            if ctx.get_quest_items_count(killer, NEBULITE_ORB) < 300 {
                ctx.give_items(killer, NEBULITE_ORB, 1);
            }
            if ctx.get_quest_items_count(killer, NEBULITE_ORB) >= 300 {
                qs.set_cond(ctx, 7, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if oren_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let lord = castle_owner(ctx);

        match npc.id {
            BRASSEUR => {
                // Java `isCond(0)`: a state with no row yet has cond 0.
                if qs.get_cond(ctx) == 0 {
                    if lord == Some(talker) {
                        if !has_fort(ctx) {
                            htmltext = "35226-01.html".to_owned();
                        } else {
                            htmltext = "35226-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else {
                        htmltext = "35226-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, false);
                    htmltext = "35226-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "35226-04.html".to_owned();
                } else if qs.is_cond(ctx, 8) {
                    htmltext = "35226-05.html".to_owned();
                }
            }
            CROOP => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "30676-01.html".to_owned();
                } else if qs.is_cond(ctx, 3) || qs.is_cond(ctx, 4) {
                    htmltext = "30676-03.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "30676-04.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "30676-05.html".to_owned();
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "30676-06.html".to_owned();
                } else if qs.is_cond(ctx, 8) {
                    htmltext = "30676-08.html".to_owned();
                }
            }
            MARTY => {
                if qs.get_cond(ctx) == 0 {
                    if is_lord_available(ctx, 3, talker).is_some() {
                        htmltext = "30169-01.html".to_owned();
                    } else {
                        htmltext = "30169-00.html".to_owned();
                    }
                }
            }
            VALLERIA => {
                if qs.state(ctx) == STARTED && is_lord_available(ctx, 4, talker).is_some() {
                    htmltext = "30176-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, PlayerClanState};
    use l2_world::residence::CastleRecord;
    use l2_world::{ClanRecord, ResidenceLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, player_data, rows};

    const NPCS: &str = r#"
        <npc id="35226" type="Folk" level="1" name="Chamberlain Brasseur"/>
        <npc id="30676" type="Folk" level="1" name="Croop"/>
        <npc id="30169" type="Folk" level="1" name="Marty"/>
        <npc id="30176" type="Folk" level="1" name="Valleria"/>
        <npc id="20575" type="Monster" level="20" name="Oel Mahum Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13851" type="EtcItem" name="Nebulite Orb"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        brasseur: Oid,
        croop: Oid,
        marty: Oid,
        valleria: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00712PathToBecomingALordOren);
        let pages = Pages::of(&Q00712PathToBecomingALordOren)
            .plain(&[
                "35226-00.html",
                "35226-00a.html",
                "35226-01.html",
                "35226-03.html",
                "35226-04.html",
                "35226-05.html",
                "35226-06.html",
                "30676-01.html",
                "30676-03.html",
                "30676-04.html",
                "30676-05.html",
                "30676-06.html",
                "30676-07.html",
                "30676-08.html",
                "30169-00.html",
                "30169-01.html",
                "30169-02.html",
                "30176-01.html",
                "30176-02.html",
            ])
            .npcs(&[BRASSEUR, CROOP, MARTY, VALLERIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let n = Npcs {
            brasseur: t.spawn(BRASSEUR, 20),
            croop: t.spawn(CROOP, 30),
            marty: t.spawn(MARTY, 40),
            valleria: t.spawn(VALLERIA, 50),
        };
        (t, n)
    }

    /// Oren castle owned by the clan 500, led by player 1 (online); player 2 is a member of
    /// the clan, player 3 is outside it.
    fn crown_oren(t: &mut ScriptTest) {
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: 500,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 4,
                    leader_id: 1,
                    ..ClanDataRecord::default()
                }),
                members: [1, 2]
                    .map(|id| ClanMemberRecord {
                        object_id: id,
                        name: format!("P{id}"),
                        level: 40,
                        ..ClanMemberRecord::default()
                    })
                    .to_vec(),
                ..ClanRecord::default()
            }],
            Vec::new(),
        );
        t.world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 4,
                found: true,
                name: "Oren".to_owned(),
                owner_id: 500,
                ..CastleRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        for id in [1, 2] {
            let mut data = player_data(id, 40, vec![]);
            data.profile.clan = Some(PlayerClanState {
                clan_id: 500,
                privileges: ClanPrivilegesRef::Own(0),
                ..PlayerClanState::default()
            });
            t.enter_with(data);
        }
        t.enter(3, 40, vec![]);
        t.take();
        t.take_writes();
    }

    fn talk(t: &mut ScriptTest, npc: Oid, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            out = Q00712PathToBecomingALordOren.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00712PathToBecomingALordOren.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    #[test]
    fn without_a_lord_every_talk_and_event_says_so() {
        let (mut t, n) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(
            talk(&mut t, n.brasseur, 1).as_deref(),
            Some("Castle has no lord.")
        );
        assert_eq!(
            event(&mut t, n.brasseur, "35226-03.html", 1).as_deref(),
            Some("Castle has no lord.")
        );
    }

    #[test]
    fn someone_else_than_the_lord_is_sent_away() {
        let (mut t, n) = setup();
        crown_oren(&mut t);
        assert_eq!(talk(&mut t, n.brasseur, 3).as_deref(), Some("35226-00a.html"));
        // Marty refuses while the lord's cond is not 3.
        assert_eq!(talk(&mut t, n.marty, 2).as_deref(), Some("30169-00.html"));
    }

    #[test]
    fn the_whole_path_of_the_lord_and_the_helper() {
        let (mut t, n) = setup();
        crown_oren(&mut t);

        // The lord starts the quest; Brasseur moves it to cond 2, Croop to cond 3.
        assert_eq!(talk(&mut t, n.brasseur, 1).as_deref(), Some("35226-01.html"));
        assert_eq!(
            event(&mut t, n.brasseur, "35226-03.html", 1).as_deref(),
            Some("35226-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(talk(&mut t, n.brasseur, 1).as_deref(), Some("35226-04.html"));
        assert_eq!(talk(&mut t, n.croop, 1).as_deref(), Some("30676-01.html"));
        event(&mut t, n.croop, "30676-03.html", 1);
        assert_eq!(talk(&mut t, n.croop, 1).as_deref(), Some("30676-03.html"));
        t.take_writes();

        // The member agrees with Marty (the lord's cond 4), and ends its part with Valleria
        // (the lord's cond 5; the member's quest is closed).
        assert_eq!(talk(&mut t, n.marty, 2).as_deref(), Some("30169-01.html"));
        event(&mut t, n.marty, "30169-02.html", 2);
        assert_eq!(talk(&mut t, n.valleria, 2).as_deref(), Some("30176-01.html"));
        // Outside the clan Valleria has nothing to say (the no-quest page).
        assert_ne!(talk(&mut t, n.valleria, 3).as_deref(), Some("30176-01.html"));
        t.take_writes();
        event(&mut t, n.valleria, "30176-02.html", 2);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "cond", "5")), "{writes:?}");
        assert!(
            writes.contains(&rows::of(2, rows::delete(NAME, true))),
            "{writes:?}"
        );

        // The lord kills Oel Mahums until 300 orbs.
        assert_eq!(talk(&mut t, n.croop, 1).as_deref(), Some("30676-04.html"));
        event(&mut t, n.croop, "30676-05.html", 1);
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), NEBULITE_ORB, 299));
        let mahum = t.spawn(20575, 60);
        t.kill(1, mahum);
        assert_eq!(t.count(1, NEBULITE_ORB), 300);
        assert_eq!(talk(&mut t, n.croop, 1).as_deref(), Some("30676-06.html"));
        event(&mut t, n.croop, "30676-07.html", 1);
        assert_eq!(t.count(1, NEBULITE_ORB), 0);
        assert_eq!(talk(&mut t, n.croop, 1).as_deref(), Some("30676-08.html"));

        // The coronation.
        assert_eq!(talk(&mut t, n.brasseur, 1).as_deref(), Some("35226-05.html"));
        t.take();
        t.take_writes();
        assert_eq!(
            event(&mut t, n.brasseur, "35226-06.html", 1).as_deref(),
            Some("35226-06.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
