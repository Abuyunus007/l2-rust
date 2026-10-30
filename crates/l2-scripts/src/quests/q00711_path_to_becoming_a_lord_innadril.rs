//! Quest 711: Path to Becoming a Lord - Innadril (Chamberlain Neurath and Iason Heine, the
//! Innadril castle lord).
// Java: dist/game/data/scripts/quests/Q00711_PathToBecomingALordInnadril/Q00711_PathToBecomingALordInnadril.java
//
// Pattern: a lord quest with a confidant (`isLordAvailable`: the lord is online, is not the
// talker, the talker is in the lord's clan and the lord's cond is the expected one). The
// confidant agrees with Neurath (lord cond 3) and Iason Heine (lord cond 4); the lord then kills
// a hundred of the Crokian and Farhite monsters (`mobs`, cond 5 to 6).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NEURATH: i32 = 35316;
const IASON_HEINE: i32 = 30969;

const MOBS: [i32; 10] = [
    20789, // Crokian
    20790, // Dailaon
    20791, // Crokian Warrior
    20792, // Farhite
    20793, // Nos
    20804, // Crokian Lad
    20805, // Dailaon Lad
    20806, // Crokian Lad Warrior
    20807, // Farhite Lad
    20808, // Nos Lad
];

/// Java `INNADRIL_CASTLE`.
const INNADRIL_CASTLE: CastleId = CastleId(6);

const NAME: &str = "Q00711_PathToBecomingALordInnadril";

/// Java `Q00711_PathToBecomingALordInnadril`.
pub struct Q00711PathToBecomingALordInnadril;

/// Java `castle.getOwner()` of Innadril: the owner clan, `None` without a lord.
fn innadril_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(INNADRIL_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when the clan leader is online.
fn castle_owner(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    ctx.world().clan_online_leader(innadril_lord_clan(ctx)?)
}

/// Java `hasFort()`: a fortress is contracted with Innadril.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == INNADRIL_CASTLE.0)
}

/// Java `isLordAvailable(cond, qs)`: the lord's id when it is.
fn is_lord_available(ctx: &mut ScriptCtx<'_>, cond: i32, player: ObjectId) -> Option<ObjectId> {
    let owner = innadril_lord_clan(ctx)?;
    let lord = castle_owner(ctx)?;
    if lord == player || ctx.world().clan_id_of(player) != owner {
        return None;
    }
    let lord_qs = ctx.quest_state_of(NAME, lord, false)?;
    lord_qs.is_cond(ctx, cond).then_some(lord)
}

/// `castleOwner.getQuestState(getName()).getInt("confidant")`.
fn confidant_of(ctx: &mut ScriptCtx<'_>, lord: ObjectId) -> i32 {
    ctx.quest_state_of(NAME, lord, false)
        .map_or(0, |qs| qs.get_int(ctx, "confidant"))
}

impl Script for Q00711PathToBecomingALordInnadril {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00711_PathToBecomingALordInnadril"
    }

    fn id(&self) -> i32 {
        711
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NEURATH]);
        s.add_kill_id(&MOBS);
        s.add_talk_id(&[NEURATH, IASON_HEINE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // NOTE(java-quirk): this onEvent has no "Castle has no lord." answer; Java reads
        // `castle.getOwner().getLeader()` at once and throws without an owner: no page.
        innadril_lord_clan(ctx)?;
        let lord = castle_owner(ctx);
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = event.to_owned();

        match event {
            "35316-03.html" => qs?.start_quest(ctx),
            "35316-05.html" => qs?.set_cond(ctx, 2, false),
            "35316-08.html" => {
                if let Some(lord) = is_lord_available(ctx, 2, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set(ctx, "confidant", &player.0.to_string());
                        lord_qs.set_cond(ctx, 3, false);
                    }
                    qs?.set_state(ctx, STARTED);
                } else {
                    htmltext = "35316-07a.html".to_owned();
                }
            }
            "30969-03.html" => {
                if let Some(lord) = is_lord_available(ctx, 3, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 4, false);
                    }
                } else {
                    htmltext = "30969-00a.html".to_owned();
                }
            }
            "35316-12.html" => {
                if lord.is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfInnadrilMayThereBeGloryInTheTerritoryOfInnadril,
                            &[name.as_str()],
                        );
                    }
                    qs?.exit_quest(ctx, true, true);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 5) {
            let mobs = qs.get_int(ctx, "mobs");
            if mobs < 99 {
                qs.set_int(ctx, "mobs", mobs + 1);
            } else {
                qs.set_cond(ctx, 6, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if innadril_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let lord = castle_owner(ctx);

        match npc.id {
            NEURATH => {
                // Java `isCond(0)`: a state with no row yet has cond 0.
                if qs.get_cond(ctx) == 0 {
                    if lord == Some(talker) {
                        if !has_fort(ctx) {
                            htmltext = "35316-01.html".to_owned();
                        } else {
                            htmltext = "35316-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else if let Some(lord) = is_lord_available(ctx, 2, talker) {
                        if ctx.is_inside_radius_2d(lord, npc.object_id, 200) {
                            htmltext = "35316-07.html".to_owned();
                        } else {
                            htmltext = "35316-07a.html".to_owned();
                        }
                    } else if qs.state(ctx) == STARTED {
                        htmltext = "35316-00b.html".to_owned();
                    } else {
                        htmltext = "35316-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    htmltext = "35316-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "35316-06.html".to_owned();
                } else if qs.is_cond(ctx, 3) {
                    htmltext = "35316-09.html".to_owned();
                } else if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, false);
                    htmltext = "35316-10.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "35316-10.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "35316-11.html".to_owned();
                }
            }
            IASON_HEINE => {
                if qs.state(ctx) == STARTED && qs.get_cond(ctx) == 0 {
                    if let Some(lord) = is_lord_available(ctx, 3, talker) {
                        htmltext = if confidant_of(ctx, lord) == talker.0 {
                            "30969-01.html"
                        } else {
                            "30969-00.html"
                        }
                        .to_owned();
                    } else if let Some(lord) = is_lord_available(ctx, 4, talker) {
                        htmltext = if confidant_of(ctx, lord) == talker.0 {
                            "30969-03.html"
                        } else {
                            "30969-00.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "30969-00a.html".to_owned();
                    }
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
        <npc id="35316" type="Folk" level="1" name="Chamberlain Neurath"/>
        <npc id="30969" type="Folk" level="1" name="Iason Heine"/>
        <npc id="20789" type="Monster" level="20" name="Crokian"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00711PathToBecomingALordInnadril);
        let pages = Pages::of(&Q00711PathToBecomingALordInnadril)
            .plain(&[
                "35316-00.html",
                "35316-00a.html",
                "35316-00b.html",
                "35316-01.html",
                "35316-03.html",
                "35316-04.html",
                "35316-05.html",
                "35316-06.html",
                "35316-07.html",
                "35316-07a.html",
                "35316-08.html",
                "35316-09.html",
                "35316-10.html",
                "35316-11.html",
                "35316-12.html",
                "30969-00.html",
                "30969-00a.html",
                "30969-01.html",
                "30969-03.html",
            ])
            .npcs(&[NEURATH, IASON_HEINE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let neurath = t.spawn(NEURATH, 20);
        let iason = t.spawn(IASON_HEINE, 30);
        (t, neurath, iason)
    }

    /// Innadril castle owned by the clan 500, led by player 1 (online); player 2 is a member of
    /// the clan, player 3 is outside it.
    fn crown_innadril(t: &mut ScriptTest) {
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: 500,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 6,
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
                id: 6,
                found: true,
                name: "Innadril".to_owned(),
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
            out = Q00711PathToBecomingALordInnadril.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00711PathToBecomingALordInnadril.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    #[test]
    fn without_a_lord_the_talk_says_so_and_the_event_gives_no_page() {
        let (mut t, neurath, _) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("Castle has no lord."));
        assert_eq!(event(&mut t, neurath, "35316-03.html", 1), None);
    }

    #[test]
    fn the_whole_path_of_the_lord_and_the_confidant() {
        let (mut t, neurath, iason) = setup();
        crown_innadril(&mut t);

        // The lord starts the quest and moves it to cond 2.
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-01.html"));
        assert_eq!(
            event(&mut t, neurath, "35316-03.html", 1).as_deref(),
            Some("35316-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-04.html"));
        event(&mut t, neurath, "35316-05.html", 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-06.html"));

        // Outside the clan: refused; the member agrees.
        assert_eq!(talk(&mut t, neurath, 3).as_deref(), Some("35316-00a.html"));
        assert_eq!(
            event(&mut t, neurath, "35316-08.html", 3).as_deref(),
            Some("35316-07a.html")
        );
        assert_eq!(talk(&mut t, neurath, 2).as_deref(), Some("35316-07.html"));
        t.take_writes();
        assert_eq!(
            event(&mut t, neurath, "35316-08.html", 2).as_deref(),
            Some("35316-08.html")
        );
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "confidant", "2")),
            "{writes:?}"
        );
        assert!(writes.contains(&rows::update(NAME, "cond", "3")), "{writes:?}");
        assert_eq!(talk(&mut t, neurath, 2).as_deref(), Some("35316-00b.html"));
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-09.html"));

        // Iason Heine: the member goes on (lord cond 4), then Neurath moves the lord to cond 5.
        assert_eq!(talk(&mut t, iason, 2).as_deref(), Some("30969-01.html"));
        event(&mut t, iason, "30969-03.html", 2);
        assert_eq!(talk(&mut t, iason, 2).as_deref(), Some("30969-03.html"));
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-10.html"));

        // A hundred kills: 99 counted, the next one sets cond 6.
        for _ in 0..100 {
            let crokian = t.spawn(20789, 60);
            t.kill(1, crokian);
        }
        assert_eq!(talk(&mut t, neurath, 1).as_deref(), Some("35316-11.html"));

        // The coronation.
        t.take();
        t.take_writes();
        assert_eq!(
            event(&mut t, neurath, "35316-12.html", 1).as_deref(),
            Some("35316-12.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }

    #[test]
    fn iason_heine_refuses_a_stranger() {
        let (mut t, neurath, iason) = setup();
        crown_innadril(&mut t);
        talk(&mut t, neurath, 1);
        event(&mut t, neurath, "35316-03.html", 1);
        // Player 3 has a started quest of its own (cond 0) but is not in the clan.
        talk(&mut t, neurath, 3);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(3), true).unwrap();
            qs.set_state(ctx, STARTED);
        });
        assert_eq!(talk(&mut t, iason, 3).as_deref(), Some("30969-00a.html"));
    }
}
