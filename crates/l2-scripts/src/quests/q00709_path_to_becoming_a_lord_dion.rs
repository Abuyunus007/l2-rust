//! Quest 709: Path to Becoming a Lord - Dion (Chamberlain Crosby, Rouke and Sophya, the Dion
//! castle lord).
// Java: dist/game/data/scripts/quests/Q00709_PathToBecomingALordDion/Q00709_PathToBecomingALordDion.java
//
// Pattern: a lord quest with a confidant, like Q00708 (`isLordAvailable`: the lord is online,
// is not the talker, the talker is in the lord's clan and the lord's cond is the expected one).
// The lord gets the Bloody Axe Black Epaulette (cond 6 to 8) while the confidant collects 100
// Mandragora Roots for Rouke (cond 8 to 9).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CROSBY: i32 = 35142;
const ROUKE: i32 = 31418;
const SOPHYA: i32 = 30735;
const BLOODY_AXE_AIDE: i32 = 27392;

// Items
const MANDRAGORA_ROOT: i32 = 13849;
const BLOODY_AXE_BLACK_EPAULETTE: i32 = 13850;

const OL_MAHUMS: [i32; 5] = [
    20208, // Ol Mahum Raider
    20209, // Ol Mahum Marksman
    20210, // Ol Mahum Sergeant
    20211, // Ol Mahum Captain
    BLOODY_AXE_AIDE,
];

const MANDRAGORAS: [i32; 3] = [
    20154, // Mandragora Sprout
    20155, // Mandragora Sapling
    20156, // Mandragora Blossom
];

/// Java `DION_CASTLE`.
const DION_CASTLE: CastleId = CastleId(2);

const NAME: &str = "Q00709_PathToBecomingALordDion";

/// Java `Q00709_PathToBecomingALordDion`.
pub struct Q00709PathToBecomingALordDion;

/// Java `castle.getOwner()` of Dion: the owner clan, `None` without a lord.
fn dion_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(DION_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when the clan leader is online.
fn castle_owner(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    ctx.world().clan_online_leader(dion_lord_clan(ctx)?)
}

/// Java `hasFort()`: a fortress is contracted with Dion.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == DION_CASTLE.0)
}

/// Java `isLordAvailable(cond, qs)`: the lord's id when it is.
fn is_lord_available(ctx: &mut ScriptCtx<'_>, cond: i32, player: ObjectId) -> Option<ObjectId> {
    let owner = dion_lord_clan(ctx)?;
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

impl Script for Q00709PathToBecomingALordDion {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00709_PathToBecomingALordDion"
    }

    fn id(&self) -> i32 {
        709
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CROSBY]);
        s.add_kill_id(&OL_MAHUMS);
        s.add_kill_id(&MANDRAGORAS);
        s.add_talk_id(&[CROSBY, SOPHYA, ROUKE]);
        s.register_quest_items(&[BLOODY_AXE_BLACK_EPAULETTE, MANDRAGORA_ROOT]);
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
        if dion_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let lord = castle_owner(ctx);
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = event.to_owned();

        match event {
            "35142-03.html" => qs?.start_quest(ctx),
            "35142-06.html" => {
                if let Some(lord) = is_lord_available(ctx, 2, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set(ctx, "confidant", &player.0.to_string());
                        lord_qs.set_cond(ctx, 3, false);
                    }
                    qs?.set_state(ctx, STARTED);
                } else {
                    htmltext = "35142-05a.html".to_owned();
                }
            }
            "31418-03.html" => {
                if let Some(lord) = is_lord_available(ctx, 3, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 4, false);
                    }
                } else {
                    htmltext = "35142-05a.html".to_owned();
                }
            }
            "30735-02.html" => qs?.set_cond(ctx, 6, false),
            "30735-05.html" => {
                ctx.take_items(player, BLOODY_AXE_BLACK_EPAULETTE, 1);
                qs?.set_cond(ctx, 8, false);
            }
            "31418-05.html" => {
                if let Some(lord) = is_lord_available(ctx, 8, player) {
                    ctx.take_items(player, MANDRAGORA_ROOT, -1);
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 9, false);
                    }
                }
            }
            "35142-10.html" => {
                if lord.is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeLordOfTheTownOfDionLongMayHeReign,
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

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 6) && OL_MAHUMS.contains(&npc.id) {
            if npc.id != BLOODY_AXE_AIDE && ctx.get_random(9) == 0 {
                ctx.add_spawn_at(BLOODY_AXE_AIDE, npc.object_id, true, 300_000);
            } else if npc.id == BLOODY_AXE_AIDE {
                ctx.give_items(killer, BLOODY_AXE_BLACK_EPAULETTE, 1);
                qs.set_cond(ctx, 7, false);
            }
        }

        if qs.state(ctx) == STARTED
            && qs.get_cond(ctx) == 0
            && is_lord_available(ctx, 8, killer).is_some()
            && MANDRAGORAS.contains(&npc.id)
            && ctx.get_quest_items_count(killer, MANDRAGORA_ROOT) < 100
        {
            ctx.give_items(killer, MANDRAGORA_ROOT, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if dion_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let lord = castle_owner(ctx);

        match npc.id {
            CROSBY => {
                // Java `isCond(0)`: a state with no row yet has cond 0.
                if qs.get_cond(ctx) == 0 {
                    if lord == Some(talker) {
                        if !has_fort(ctx) {
                            htmltext = "35142-01.html".to_owned();
                        } else {
                            htmltext = "35142-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else if let Some(lord) = is_lord_available(ctx, 2, talker) {
                        if ctx.is_inside_radius_2d(lord, npc.object_id, 200) {
                            htmltext = "35142-05.html".to_owned();
                        } else {
                            htmltext = "35142-05a.html".to_owned();
                        }
                    } else {
                        htmltext = "35142-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, false);
                    htmltext = "35142-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) || qs.is_cond(ctx, 3) {
                    htmltext = "35142-04a.html".to_owned();
                } else if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, false);
                    htmltext = "35142-07.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "35142-07.html".to_owned();
                } else if qs.get_cond(ctx) > 5 && qs.get_cond(ctx) < 9 {
                    htmltext = "35142-08.html".to_owned();
                } else if qs.is_cond(ctx, 9) {
                    htmltext = "35142-09.html".to_owned();
                }
            }
            ROUKE => {
                if qs.state(ctx) == STARTED && qs.get_cond(ctx) == 0 {
                    if let Some(lord) = is_lord_available(ctx, 3, talker) {
                        if confidant_of(ctx, lord) == talker.0 {
                            htmltext = "31418-01.html".to_owned();
                        }
                    } else if is_lord_available(ctx, 8, talker).is_some() {
                        if ctx.get_quest_items_count(talker, MANDRAGORA_ROOT) >= 100 {
                            htmltext = "31418-04.html".to_owned();
                        } else {
                            htmltext = "31418-04a.html".to_owned();
                        }
                    } else if is_lord_available(ctx, 9, talker).is_some() {
                        htmltext = "31418-06.html".to_owned();
                    }
                }
            }
            SOPHYA => {
                if qs.is_cond(ctx, 5) {
                    htmltext = "30735-01.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "30735-03.html".to_owned();
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "30735-04.html".to_owned();
                } else if qs.is_cond(ctx, 8) {
                    htmltext = "30735-06.html".to_owned();
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
        <npc id="35142" type="Folk" level="1" name="Chamberlain Crosby"/>
        <npc id="31418" type="Folk" level="1" name="Rouke"/>
        <npc id="30735" type="Folk" level="1" name="Sophya"/>
        <npc id="20208" type="Monster" level="20" name="Ol Mahum Raider"/>
        <npc id="27392" type="Monster" level="30" name="Bloody Axe Aide"/>
        <npc id="20154" type="Monster" level="20" name="Mandragora Sprout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13849" type="EtcItem" name="Mandragora Root"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13850" type="EtcItem" name="Bloody Axe Black Epaulette"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        crosby: Oid,
        rouke: Oid,
        sophya: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00709PathToBecomingALordDion);
        let pages = Pages::of(&Q00709PathToBecomingALordDion)
            .plain(&[
                "35142-00.html",
                "35142-00a.html",
                "35142-01.html",
                "35142-03.html",
                "35142-04.html",
                "35142-04a.html",
                "35142-05.html",
                "35142-05a.html",
                "35142-06.html",
                "35142-07.html",
                "35142-08.html",
                "35142-09.html",
                "35142-10.html",
                "31418-01.html",
                "31418-03.html",
                "31418-04.html",
                "31418-04a.html",
                "31418-05.html",
                "31418-06.html",
                "30735-01.html",
                "30735-02.html",
                "30735-03.html",
                "30735-04.html",
                "30735-05.html",
                "30735-06.html",
            ])
            .npcs(&[CROSBY, ROUKE, SOPHYA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let n = Npcs {
            crosby: t.spawn(CROSBY, 20),
            rouke: t.spawn(ROUKE, 30),
            sophya: t.spawn(SOPHYA, 40),
        };
        (t, n)
    }

    /// Dion castle owned by the clan 500, led by player 1 (online); player 2 is a member of
    /// the clan, player 3 is outside it.
    fn crown_dion(t: &mut ScriptTest) {
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: 500,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 2,
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
                id: 2,
                found: true,
                name: "Dion".to_owned(),
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
            out = Q00709PathToBecomingALordDion.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00709PathToBecomingALordDion.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    #[test]
    fn without_a_lord_every_talk_and_event_says_so() {
        let (mut t, n) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("Castle has no lord."));
        assert_eq!(
            event(&mut t, n.crosby, "35142-03.html", 1).as_deref(),
            Some("Castle has no lord.")
        );
    }

    #[test]
    fn the_whole_path_of_the_lord_and_the_confidant() {
        let (mut t, n) = setup();
        crown_dion(&mut t);

        // The lord starts the quest; Crosby moves it to cond 2 on the next talk.
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-01.html"));
        assert_eq!(
            event(&mut t, n.crosby, "35142-03.html", 1).as_deref(),
            Some("35142-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-04a.html"));

        // Not the lord's clan: refused.
        assert_eq!(talk(&mut t, n.crosby, 3).as_deref(), Some("35142-00a.html"));
        assert_eq!(
            event(&mut t, n.crosby, "35142-06.html", 3).as_deref(),
            Some("35142-05a.html")
        );
        // The member is in range of the lord and accepts.
        assert_eq!(talk(&mut t, n.crosby, 2).as_deref(), Some("35142-05.html"));
        t.take_writes();
        assert_eq!(
            event(&mut t, n.crosby, "35142-06.html", 2).as_deref(),
            Some("35142-06.html")
        );
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "confidant", "2")),
            "{writes:?}"
        );
        assert!(writes.contains(&rows::update(NAME, "cond", "3")), "{writes:?}");

        // Rouke lets the confidant go on: the lord's cond is 4, then 5.
        assert_eq!(talk(&mut t, n.rouke, 2).as_deref(), Some("31418-01.html"));
        event(&mut t, n.rouke, "31418-03.html", 2);
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-07.html"));
        assert_eq!(talk(&mut t, n.sophya, 1).as_deref(), Some("30735-01.html"));

        // Sophya: the lord kills Ol Mahums until a Bloody Axe Aide comes out.
        event(&mut t, n.sophya, "30735-02.html", 1);
        assert_eq!(talk(&mut t, n.sophya, 1).as_deref(), Some("30735-03.html"));
        let mut kills = 0;
        while t.npcs_of(BLOODY_AXE_AIDE).is_empty() {
            let mahum = t.spawn(20208, 60);
            t.kill(1, mahum);
            kills += 1;
            assert!(kills < 200, "no Bloody Axe Aide came out");
        }
        let aide = t.npcs_of(BLOODY_AXE_AIDE)[0];
        t.kill(1, aide);
        assert_eq!(t.count(1, BLOODY_AXE_BLACK_EPAULETTE), 1);
        assert_eq!(talk(&mut t, n.sophya, 1).as_deref(), Some("30735-04.html"));
        event(&mut t, n.sophya, "30735-05.html", 1);
        assert_eq!(t.count(1, BLOODY_AXE_BLACK_EPAULETTE), 0);
        assert_eq!(talk(&mut t, n.sophya, 1).as_deref(), Some("30735-06.html"));
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-08.html"));

        // The confidant collects a hundred roots (never more) and hands them in.
        assert_eq!(talk(&mut t, n.rouke, 2).as_deref(), Some("31418-04a.html"));
        for _ in 0..105 {
            let sprout = t.spawn(20154, 60);
            t.kill(2, sprout);
        }
        assert_eq!(t.count(2, MANDRAGORA_ROOT), 100);
        assert_eq!(talk(&mut t, n.rouke, 2).as_deref(), Some("31418-04.html"));
        event(&mut t, n.rouke, "31418-05.html", 2);
        assert_eq!(t.count(2, MANDRAGORA_ROOT), 0);
        assert_eq!(talk(&mut t, n.rouke, 2).as_deref(), Some("31418-06.html"));

        // The coronation.
        assert_eq!(talk(&mut t, n.crosby, 1).as_deref(), Some("35142-09.html"));
        t.take();
        t.take_writes();
        assert_eq!(
            event(&mut t, n.crosby, "35142-10.html", 1).as_deref(),
            Some("35142-10.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
