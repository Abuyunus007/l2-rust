//! Quest 708: Path to Becoming a Lord - Gludio (Chamberlain Sayres, Pinter and Bathis, the
//! Gludio castle lord).
// Java: dist/game/data/scripts/quests/Q00708_PathToBecomingALordGludio/Q00708_PathToBecomingALordGludio.java
//
// Pattern: a lord quest with a confidant. The lord (the online leader of the castle's clan)
// starts it, then asks a clan member to be the confidant: the member's talk with Sayres sets the
// lord's `confidant` variable and moves the lord's cond (`isLordAvailable`: the lord is online,
// is not the talker, the talker is in the lord's clan and the lord's cond is the expected one).
// The lord then gets the Headless Knight's armor (cond 6 to 8, the lord kills) while the
// confidant brings the goods to Pinter (cond 8 to 9).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SAYRES: i32 = 35100;
const PINTER: i32 = 30298;
const BATHIS: i32 = 30332;
const HEADLESS_KNIGHT: i32 = 20280;

// Items
const COKES: i32 = 1879;
const IRON_ORE: i32 = 1869;
const ANIMAL_SKIN: i32 = 1867;
const VARNISH: i32 = 1865;
const HEADLESS_KNIGHT_ARMOR: i32 = 13848;

const MOBS: [i32; 4] = [
    20045, // Skeleton Scout
    20051, // Skeleton Bowman
    20099, // Skeleton
    HEADLESS_KNIGHT,
];

/// Java `GLUDIO_CASTLE`.
const GLUDIO_CASTLE: CastleId = CastleId(1);

const NAME: &str = "Q00708_PathToBecomingALordGludio";

/// Java `Q00708_PathToBecomingALordGludio`.
pub struct Q00708PathToBecomingALordGludio;

/// Java `castle.getOwner()` of Gludio: the owner clan, `None` without a lord.
fn gludio_lord_clan(ctx: &ScriptCtx<'_>) -> Option<i32> {
    ctx.world()
        .castle(GLUDIO_CASTLE)
        .map(|castle| castle.owner_id)
        .filter(|clan| *clan != 0)
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when the clan leader is online.
fn castle_owner(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    ctx.world().clan_online_leader(gludio_lord_clan(ctx)?)
}

/// Java `hasFort()`: a fortress is contracted with Gludio.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| fort.castle_id == GLUDIO_CASTLE.0)
}

/// Java `isLordAvailable(cond, qs)`: the lord's id when it is.
fn is_lord_available(ctx: &mut ScriptCtx<'_>, cond: i32, player: ObjectId) -> Option<ObjectId> {
    let owner = gludio_lord_clan(ctx)?;
    let lord = castle_owner(ctx)?;
    if lord == player || ctx.world().clan_id_of(player) != owner {
        return None;
    }
    let lord_qs = ctx.quest_state_of(NAME, lord, false)?;
    lord_qs.is_cond(ctx, cond).then_some(lord)
}

impl Script for Q00708PathToBecomingALordGludio {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00708_PathToBecomingALordGludio"
    }

    fn id(&self) -> i32 {
        708
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SAYRES]);
        s.add_kill_id(&MOBS);
        s.add_talk_id(&[SAYRES, PINTER, BATHIS]);
        s.register_quest_items(&[HEADLESS_KNIGHT_ARMOR]);
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
        if gludio_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let lord = castle_owner(ctx);
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = event.to_owned();

        match event {
            "35100-03.html" => {
                let qs = qs?;
                qs.start_quest(ctx);
                qs.set(ctx, "FlagJournal", "1");
            }
            "35100-05.html" => qs?.set_cond(ctx, 2, false),
            "35100-08.html" => {
                if let Some(lord) = is_lord_available(ctx, 2, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set(ctx, "confidant", &player.0.to_string());
                        lord_qs.set_cond(ctx, 3, false);
                    }
                    qs?.set_state(ctx, STARTED);
                } else {
                    htmltext = "35100-05a.html".to_owned();
                }
            }
            "30298-03.html" => {
                if let Some(lord) = is_lord_available(ctx, 3, player) {
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 4, false);
                    }
                } else {
                    htmltext = "30298-03a.html".to_owned();
                }
            }
            "30332-02.html" => qs?.set_cond(ctx, 6, false),
            "30332-05.html" => {
                ctx.take_items(player, HEADLESS_KNIGHT_ARMOR, 1);
                qs?.set_cond(ctx, 8, false);
                if let Some(npc) = npc {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcShout,
                        NpcStringId::ListenYouVillagersOurLiegeWhoWillSoonBecomeALordHasDefeatedTheHeadlessKnightYouCanNowRestEasy,
                        &[],
                    );
                }
            }
            "30298-05.html" => {
                if let Some(lord) = is_lord_available(ctx, 8, player) {
                    ctx.take_items(player, ANIMAL_SKIN, 100);
                    ctx.take_items(player, VARNISH, 100);
                    ctx.take_items(player, IRON_ORE, 100);
                    ctx.take_items(player, COKES, 50);
                    if let Some(lord_qs) = ctx.quest_state_of(NAME, lord, false) {
                        lord_qs.set_cond(ctx, 9, false);
                    }
                } else {
                    htmltext = "30298-03a.html".to_owned();
                }
            }
            "35100-12.html" => {
                if lord.is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeLordOfTheTownOfGludioLongMayHeReign,
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
        if qs.is_cond(ctx, 6) {
            if npc.id != HEADLESS_KNIGHT && ctx.get_random(9) == 0 {
                ctx.add_spawn_at(HEADLESS_KNIGHT, npc.object_id, true, 300_000);
            } else if npc.id == HEADLESS_KNIGHT {
                ctx.give_items(killer, HEADLESS_KNIGHT_ARMOR, 1);
                qs.set_cond(ctx, 7, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if gludio_lord_clan(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let lord = castle_owner(ctx);

        match npc.id {
            SAYRES => {
                // Java `isCond(0)`: a state with no row yet has cond 0.
                if qs.get_cond(ctx) == 0 {
                    if lord == Some(talker) {
                        if !has_fort(ctx) {
                            htmltext = "35100-01.html".to_owned();
                        } else {
                            htmltext = "35100-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else if let Some(lord) = is_lord_available(ctx, 2, talker) {
                        if ctx.is_inside_radius_2d(lord, npc.object_id, 200) {
                            htmltext = "35100-07.html".to_owned();
                        } else {
                            htmltext = "35100-05a.html".to_owned();
                        }
                    } else if qs.state(ctx) == STARTED {
                        htmltext = "35100-08a.html".to_owned();
                    } else {
                        htmltext = "35100-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    htmltext = "35100-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "35100-06.html".to_owned();
                } else if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, false);
                    htmltext = "35100-09.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "35100-10.html".to_owned();
                } else if qs.get_cond(ctx) > 5 && qs.get_cond(ctx) < 9 {
                    htmltext = "35100-08.html".to_owned();
                } else if qs.is_cond(ctx, 9) {
                    htmltext = "35100-11.html".to_owned();
                }
            }
            PINTER => {
                if qs.state(ctx) == STARTED && qs.get_cond(ctx) == 0 {
                    if let Some(lord) = is_lord_available(ctx, 3, talker) {
                        if confidant_of(ctx, lord) == talker.0 {
                            htmltext = "30298-01.html".to_owned();
                        }
                    } else if is_lord_available(ctx, 8, talker).is_some() {
                        if ctx.get_quest_items_count(talker, ANIMAL_SKIN) >= 100
                            && ctx.get_quest_items_count(talker, VARNISH) >= 100
                            && ctx.get_quest_items_count(talker, IRON_ORE) >= 100
                            && ctx.get_quest_items_count(talker, COKES) >= 50
                        {
                            htmltext = "30298-04.html".to_owned();
                        } else {
                            htmltext = "30298-04a.html".to_owned();
                        }
                    } else if is_lord_available(ctx, 9, talker).is_some() {
                        htmltext = "30298-06.html".to_owned();
                    }
                }
            }
            BATHIS => {
                if qs.is_cond(ctx, 5) {
                    htmltext = "30332-01.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "30332-03.html".to_owned();
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "30332-04.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// `castleOwner.getQuestState(getName()).getInt("confidant")`.
fn confidant_of(ctx: &mut ScriptCtx<'_>, lord: ObjectId) -> i32 {
    ctx.quest_state_of(NAME, lord, false)
        .map_or(0, |qs| qs.get_int(ctx, "confidant"))
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
        <npc id="35100" type="Folk" level="1" name="Chamberlain Sayres"/>
        <npc id="30298" type="Folk" level="1" name="Pinter"/>
        <npc id="30332" type="Folk" level="1" name="Bathis"/>
        <npc id="20045" type="Monster" level="20" name="Skeleton Scout"/>
        <npc id="20280" type="Monster" level="30" name="Headless Knight"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1865" type="EtcItem" name="Varnish"><set name="is_stackable" val="true"/></item>
        <item id="1867" type="EtcItem" name="Animal Skin"><set name="is_stackable" val="true"/></item>
        <item id="1869" type="EtcItem" name="Iron Ore"><set name="is_stackable" val="true"/></item>
        <item id="1879" type="EtcItem" name="Cokes"><set name="is_stackable" val="true"/></item>
        <item id="13848" type="EtcItem" name="Headless Knight's Armor"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        sayres: Oid,
        pinter: Oid,
        bathis: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00708PathToBecomingALordGludio);
        let pages = Pages::of(&Q00708PathToBecomingALordGludio)
            .plain(&[
                "35100-00.html",
                "35100-00a.html",
                "35100-01.html",
                "35100-03.html",
                "35100-04.html",
                "35100-05.html",
                "35100-05a.html",
                "35100-06.html",
                "35100-07.html",
                "35100-08.html",
                "35100-08a.html",
                "35100-09.html",
                "35100-10.html",
                "35100-11.html",
                "35100-12.html",
                "30298-01.html",
                "30298-03.html",
                "30298-03a.html",
                "30298-04.html",
                "30298-04a.html",
                "30298-05.html",
                "30298-06.html",
                "30332-01.html",
                "30332-02.html",
                "30332-03.html",
                "30332-04.html",
                "30332-05.html",
            ])
            .npcs(&[SAYRES, PINTER, BATHIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let n = Npcs {
            sayres: t.spawn(SAYRES, 20),
            pinter: t.spawn(PINTER, 30),
            bathis: t.spawn(BATHIS, 40),
        };
        (t, n)
    }

    /// Gludio castle owned by the clan 500, led by player 1 (online); player 2 is a member of
    /// the clan, player 3 is outside it.
    fn crown_gludio(t: &mut ScriptTest) {
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: 500,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 1,
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
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
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
            out = Q00708PathToBecomingALordGludio.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, npc: Oid, event: &str, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            out = Q00708PathToBecomingALordGludio.on_event(ctx, event, npc, Some(Oid(player)));
        });
        out
    }

    fn give(t: &mut ScriptTest, player: i32, item: i32, count: i64) {
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(player), item, count));
    }

    #[test]
    fn without_a_lord_every_talk_and_event_says_so() {
        let (mut t, n) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("Castle has no lord."));
        assert_eq!(
            event(&mut t, n.sayres, "35100-03.html", 1).as_deref(),
            Some("Castle has no lord.")
        );
    }

    #[test]
    fn the_whole_path_of_the_lord_and_the_confidant() {
        let (mut t, n) = setup();
        crown_gludio(&mut t);

        // The lord starts the quest and sends for a confidant.
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("35100-01.html"));
        assert_eq!(
            event(&mut t, n.sayres, "35100-03.html", 1).as_deref(),
            Some("35100-03.html")
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "FlagJournal", "1"),
            ]
        );
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("35100-04.html"));
        event(&mut t, n.sayres, "35100-05.html", 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Not the lord's clan: refused. A clan member standing too far away is told so.
        assert_eq!(talk(&mut t, n.sayres, 3).as_deref(), Some("35100-00a.html"));
        assert_eq!(
            event(&mut t, n.sayres, "35100-08.html", 3).as_deref(),
            Some("35100-05a.html")
        );
        assert_eq!(talk(&mut t, n.sayres, 2).as_deref(), Some("35100-07.html"));

        // The member accepts: the lord's cond is 3 with the member as the confidant.
        t.take_writes();
        assert_eq!(
            event(&mut t, n.sayres, "35100-08.html", 2).as_deref(),
            Some("35100-08.html")
        );
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "confidant", "2")),
            "{writes:?}"
        );
        assert!(writes.contains(&rows::update(NAME, "cond", "3")), "{writes:?}");
        assert_eq!(talk(&mut t, n.sayres, 2).as_deref(), Some("35100-08a.html"));

        // Pinter lets the confidant go on: the lord's cond is 4.
        assert_eq!(talk(&mut t, n.pinter, 2).as_deref(), Some("30298-01.html"));
        event(&mut t, n.pinter, "30298-03.html", 2);
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("35100-09.html"));
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("35100-10.html"));

        // Bathis: the lord kills skeletons until a Headless Knight comes out, then the armor.
        assert_eq!(talk(&mut t, n.bathis, 1).as_deref(), Some("30332-01.html"));
        event(&mut t, n.bathis, "30332-02.html", 1);
        assert_eq!(talk(&mut t, n.bathis, 1).as_deref(), Some("30332-03.html"));
        let mut kills = 0;
        while t.npcs_of(HEADLESS_KNIGHT).is_empty() {
            let skeleton = t.spawn(20045, 60);
            t.kill(1, skeleton);
            kills += 1;
            assert!(kills < 200, "no Headless Knight came out");
        }
        let knight = t.npcs_of(HEADLESS_KNIGHT)[0];
        t.kill(1, knight);
        assert_eq!(t.count(1, HEADLESS_KNIGHT_ARMOR), 1);
        assert_eq!(talk(&mut t, n.bathis, 1).as_deref(), Some("30332-04.html"));
        event(&mut t, n.bathis, "30332-05.html", 1);
        assert_eq!(t.count(1, HEADLESS_KNIGHT_ARMOR), 0);

        // The confidant brings the goods.
        assert_eq!(talk(&mut t, n.pinter, 2).as_deref(), Some("30298-04a.html"));
        for (item, count) in [(ANIMAL_SKIN, 100), (VARNISH, 100), (IRON_ORE, 100), (COKES, 50)] {
            give(&mut t, 2, item, count);
        }
        assert_eq!(talk(&mut t, n.pinter, 2).as_deref(), Some("30298-04.html"));
        event(&mut t, n.pinter, "30298-05.html", 2);
        assert_eq!(t.count(2, COKES), 0);
        assert_eq!(talk(&mut t, n.pinter, 2).as_deref(), Some("30298-06.html"));

        // The coronation.
        assert_eq!(talk(&mut t, n.sayres, 1).as_deref(), Some("35100-11.html"));
        t.take();
        t.take_writes();
        assert_eq!(
            event(&mut t, n.sayres, "35100-12.html", 1).as_deref(),
            Some("35100-12.html")
        );
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()), "{names:?}");
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
