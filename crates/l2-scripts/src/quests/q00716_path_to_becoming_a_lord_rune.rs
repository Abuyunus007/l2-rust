//! Quest 716: Path to Becoming a Lord - Rune (Chamberlain Frederick, Agripel and Innocentin, the
//! Rune castle lord).
// Java: dist/game/data/scripts/quests/Q00716_PathToBecomingALordRune/Q00716_PathToBecomingALordRune.java
//
// Pattern: a lord quest with a confidant, unlike the other castles' quests without a check of
// the clan: the lord (the online leader of the castle's clan) needs Hidden Truth (Q00021) and
// Hiding Behind the Truth (Q00025) done, a confidant agrees with Frederick (lord cond 5) and
// Innocentin (lord cond 6) and then kills pagans in place of the lord (`paganCount` of the
// lord's quest, 100 for cond 8).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FREDERICK: i32 = 35509;
const AGRIPEL: i32 = 31348;
const INNOCENTIN: i32 = 31328;

/// Java `RUNE_CASTLE`.
const RUNE_CASTLE: CastleId = CastleId(8);

const NAME: &str = "Q00716_PathToBecomingALordRune";

/// Java `Q00716_PathToBecomingALordRune`.
pub struct Q00716PathToBecomingALordRune;

/// Java `castle.getOwner()` of Rune: the owner clan, `None` without a lord.
fn rune_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(RUNE_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when the clan leader is online.
fn castle_owner(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    ctx.world().clan_online_leader(rune_lord_clan(ctx)?)
}

/// Java `hasFort()`: a fortress is contracted with Rune.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == RUNE_CASTLE.0)
}

/// `castleOwner != null && castleOwner != player && castleOwner.getQuestState(name) != null &&
/// castleOwner.getQuestState(name).isCond(cond)`: the lord's id when it holds. (There is no
/// clan check in this quest.)
fn lord_at_cond(ctx: &mut ScriptCtx<'_>, cond: i32, player: ObjectId) -> Option<ObjectId> {
    let lord = castle_owner(ctx)?;
    if lord == player {
        return None;
    }
    let lord_qs = ctx.quest_state_of(NAME, lord, false)?;
    lord_qs.is_cond(ctx, cond).then_some(lord)
}

impl Script for Q00716PathToBecomingALordRune {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00716_PathToBecomingALordRune"
    }

    fn id(&self) -> i32 {
        716
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FREDERICK]);
        // Java `PAGANS`: 22138..=22176 and 22188..=22195.
        let pagans: Vec<i32> = (22138..=22176).chain(22188..=22195).collect();
        s.add_kill_id(&pagans);
        s.add_talk_id(&[FREDERICK, AGRIPEL, INNOCENTIN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        if rune_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let lord = castle_owner(ctx);

        match event {
            "35509-03.html" => qs.start_quest(ctx),
            "31348-03.html" => qs.set_cond(ctx, 3, false),
            "35509-08.html" => {
                // NOTE(java-quirk): no check at all: with the lord offline or without a quest
                // state Java throws here and no page is shown.
                let lord_qs = ctx.quest_state_of(NAME, lord?, false)?;
                lord_qs.set(ctx, "confidant", &player.0.to_string());
                lord_qs.set_cond(ctx, 5, false);
                qs.set_state(ctx, STARTED);
            }
            "31328-03.html" => {
                if let Some(lord) = lord_at_cond(ctx, 5, player)
                    && let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false)
                {
                    lord_qs.set_cond(ctx, 6, false);
                }
            }
            "31348-08.html" => qs.set_cond(ctx, 8, false),
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if rune_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let lord = castle_owner(ctx);

        match npc.id {
            FREDERICK => match qs.get_cond(ctx) {
                0 => {
                    if lord == Some(player) {
                        if !has_fort(ctx) {
                            htmltext = "35509-01.html".to_owned();
                        } else {
                            htmltext = "35509-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else if let Some(lord) = lord_at_cond(ctx, 4, player) {
                        if ctx.is_inside_radius_2d(lord, npc.object_id, 200) {
                            htmltext = "35509-07.html".to_owned();
                        } else {
                            htmltext = "35509-07a.html".to_owned();
                        }
                    } else if qs.state(ctx) == STARTED {
                        htmltext = "35509-00b.html".to_owned();
                    } else {
                        htmltext = "35509-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                }
                1 => {
                    let completed = |ctx: &ScriptCtx<'_>, name: &str| {
                        ctx.player_quest_state(player, name)
                            .is_some_and(|q| q.is_completed())
                    };
                    if completed(ctx, "Q00025_HidingBehindTheTruth") && completed(ctx, "Q00021_HiddenTruth") {
                        qs.set_cond(ctx, 2, false);
                        htmltext = "35509-04.html".to_owned();
                    } else {
                        htmltext = "35509-03.html".to_owned();
                    }
                }
                2 => htmltext = "35509-04a.html".to_owned(),
                3 => {
                    qs.set_cond(ctx, 4, false);
                    htmltext = "35509-05.html".to_owned();
                }
                4 => htmltext = "35509-06.html".to_owned(),
                5 => htmltext = "35509-09.html".to_owned(),
                6 => {
                    qs.set_cond(ctx, 7, false);
                    htmltext = "35509-10.html".to_owned();
                }
                7 => htmltext = "35509-11.html".to_owned(),
                8 => {
                    if lord.is_some() {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfRuneMayThereBeGloryInTheTerritoryOfRune,
                            &[name.as_str()],
                        );
                        htmltext = "35509-12.html".to_owned();
                        qs.exit_quest(ctx, true, true);
                    }
                }
                _ => {}
            },
            AGRIPEL => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "31348-01.html".to_owned();
                } else if qs.is_cond(ctx, 7) {
                    if qs.get(ctx, "paganCount").is_some() && qs.get_int(ctx, "paganCount") >= 100 {
                        htmltext = "31348-07.html".to_owned();
                    } else {
                        htmltext = "31348-04.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 8) {
                    htmltext = "31348-09.html".to_owned();
                }
            }
            INNOCENTIN => {
                if qs.state(ctx) == STARTED && qs.get_cond(ctx) == 0 {
                    if let Some(lord) = lord_at_cond(ctx, 5, player) {
                        let confidant = ctx
                            .quest_state_of(NAME, lord, false)
                            .map_or(0, |q| q.get_int(ctx, "confidant"));
                        htmltext = if confidant == player.0 {
                            "31328-01.html"
                        } else {
                            "31328-00.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "31328-00a.html".to_owned();
                    }
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
        if qs.state(ctx) != STARTED {
            return;
        }
        // NOTE(java-quirk): `castle.getOwner().getLeader()` throws without an owner.
        if rune_lord_clan(ctx).is_none() {
            return;
        }
        if qs.get_cond(ctx) == 0
            && let Some(lord) = lord_at_cond(ctx, 7, killer)
            && let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false)
        {
            if lord_qs.get(ctx, "paganCount").is_some() {
                let count = lord_qs.get_int(ctx, "paganCount");
                lord_qs.set(ctx, "paganCount", &(count + 1).to_string());
            } else {
                lord_qs.set(ctx, "paganCount", "1");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, PlayerClanState};
    use l2_world::residence::CastleRecord;
    use l2_world::script::COMPLETED;
    use l2_world::{ClanRecord, ResidenceLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, player_data, rows};

    /// Stand-ins for Hidden Truth and Hiding Behind the Truth (not needed ported here: the world
    /// only keeps the rows of a known script).
    struct StandIn(&'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }

        fn path(&self) -> &'static str {
            "quests/StandIn"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    const NPCS: &str = r#"
        <npc id="35509" type="Folk" level="1" name="Chamberlain Frederick"/>
        <npc id="31348" type="Folk" level="1" name="Agripel"/>
        <npc id="31328" type="Folk" level="1" name="Innocentin"/>
        <npc id="22138" type="Monster" level="60" name="Pagan"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        frederick: Oid,
        agripel: Oid,
        innocentin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00716PathToBecomingALordRune);
        registry.add(StandIn("Q00021_HiddenTruth"));
        registry.add(StandIn("Q00025_HidingBehindTheTruth"));
        let pages = Pages::of(&Q00716PathToBecomingALordRune)
            .plain(&[
                "35509-00.html",
                "35509-00a.html",
                "35509-00b.html",
                "35509-01.html",
                "35509-03.html",
                "35509-04.html",
                "35509-04a.html",
                "35509-05.html",
                "35509-06.html",
                "35509-07.html",
                "35509-07a.html",
                "35509-08.html",
                "35509-09.html",
                "35509-10.html",
                "35509-11.html",
                "35509-12.html",
                "31348-01.html",
                "31348-03.html",
                "31348-04.html",
                "31348-07.html",
                "31348-08.html",
                "31348-09.html",
                "31328-00.html",
                "31328-00a.html",
                "31328-01.html",
                "31328-03.html",
            ])
            .npcs(&[FREDERICK, AGRIPEL, INNOCENTIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let n = Npcs {
            frederick: t.spawn(FREDERICK, 20),
            agripel: t.spawn(AGRIPEL, 30),
            innocentin: t.spawn(INNOCENTIN, 40),
        };
        (t, n)
    }

    /// Rune castle owned by the clan 500, led by player 1 (online); player 2 is a member of
    /// the clan, player 3 is outside it.
    fn crown_rune(t: &mut ScriptTest) {
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: 500,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 8,
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
                id: 8,
                found: true,
                name: "Rune".to_owned(),
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
            out = Q00716PathToBecomingALordRune.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00716PathToBecomingALordRune.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    fn complete(t: &mut ScriptTest, quest: &str) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(quest, Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn without_a_lord_every_talk_and_event_says_so() {
        let (mut t, n) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(
            talk(&mut t, n.frederick, 1).as_deref(),
            Some("Castle has no lord.")
        );
        // An event without a quest state of the player gives no page.
        assert_eq!(event(&mut t, n.frederick, "35509-03.html", 1), None);
    }

    #[test]
    fn the_whole_path_of_the_lord_and_the_confidant() {
        let (mut t, n) = setup();
        crown_rune(&mut t);

        // The lord starts the quest; it waits for the two earlier quests.
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-01.html"));
        assert_eq!(
            event(&mut t, n.frederick, "35509-03.html", 1).as_deref(),
            Some("35509-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-03.html"));
        complete(&mut t, "Q00021_HiddenTruth");
        complete(&mut t, "Q00025_HidingBehindTheTruth");
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-04.html"));
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-04a.html"));
        assert_eq!(talk(&mut t, n.agripel, 1).as_deref(), Some("31348-01.html"));
        event(&mut t, n.agripel, "31348-03.html", 1);
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-05.html"));
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-06.html"));
        t.take_writes();

        // NOTE(java-quirk): no clan check here: a stranger is offered the same page as a member.
        assert_eq!(talk(&mut t, n.frederick, 3).as_deref(), Some("35509-07.html"));
        assert_eq!(talk(&mut t, n.frederick, 2).as_deref(), Some("35509-07.html"));
        event(&mut t, n.frederick, "35509-08.html", 2);
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "confidant", "2")),
            "{writes:?}"
        );
        assert!(writes.contains(&rows::update(NAME, "cond", "5")), "{writes:?}");
        assert_eq!(talk(&mut t, n.frederick, 2).as_deref(), Some("35509-00b.html"));
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-09.html"));

        // Innocentin: the confidant goes on (lord cond 6), Frederick moves the lord to cond 7.
        assert_eq!(talk(&mut t, n.innocentin, 2).as_deref(), Some("31328-01.html"));
        event(&mut t, n.innocentin, "31328-03.html", 2);
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-10.html"));
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-11.html"));

        // The confidant kills pagans for the lord: 99 are not enough, 100 are.
        assert_eq!(talk(&mut t, n.agripel, 1).as_deref(), Some("31348-04.html"));
        for _ in 0..99 {
            let pagan = t.spawn(22138, 60);
            t.kill(2, pagan);
        }
        assert_eq!(talk(&mut t, n.agripel, 1).as_deref(), Some("31348-04.html"));
        let pagan = t.spawn(22138, 60);
        t.kill(2, pagan);
        assert_eq!(talk(&mut t, n.agripel, 1).as_deref(), Some("31348-07.html"));
        event(&mut t, n.agripel, "31348-08.html", 1);
        assert_eq!(talk(&mut t, n.agripel, 1).as_deref(), Some("31348-09.html"));

        // The coronation (the talk itself shouts and ends the quest).
        t.take();
        t.take_writes();
        assert_eq!(talk(&mut t, n.frederick, 1).as_deref(), Some("35509-12.html"));
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
