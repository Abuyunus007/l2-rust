//! Quest 118: To Lead And Be Led (Blacksmith Pinter, sponsor and academy member).
// Java: dist/game/data/scripts/quests/Q00118_ToLeadAndBeLed/Q00118_ToLeadAndBeLed.java
//
// Pattern: two players in one script. The apprentice (a clan academy member) owns the quest; the
// sponsor talks to the same NPC and `onEvent` answers for the sponsor's apprentice. The sponsor
// and apprentice come from the clan state (`player_sponsor` / `player_apprentice`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BLACKSMITH_PINTER: i32 = 30298;

// Items
const CRYSTAL_D: i32 = 1458;
const BLOOD_OF_MAILLE_LIZARDMAN: i32 = 8062;
const LEG_OF_KING_ARANEID: i32 = 8063;

// Reward
const CLAN_OATH_HELM: i32 = 7850;
const CLAN_OATH_ARMOR: i32 = 7851;
const CLAN_OATH_GAUNTLETS_HEAVY_ARMOR: i32 = 7852;
const CLAN_OATH_SABATON_HEAVY_ARMOR: i32 = 7853;
const CLAN_OATH_BRIGANDINE: i32 = 7854;
const CLAN_OATH_LEATHER_GLOVES_LIGHT_ARMOR: i32 = 7855;
const CLAN_OATH_BOOTS_LIGHT_ARMOR: i32 = 7856;
const CLAN_OATH_AKETON: i32 = 7857;
const CLAN_OATH_PADDED_GLOVES_ROBE: i32 = 7858;
const CLAN_OATH_SANDALS_ROBE: i32 = 7859;

// Monster
const MAILLE_LIZARDMAN: i32 = 20919;
const MAILLE_LIZARDMAN_SCOUT: i32 = 20920;
const MAILLE_LIZARDMAN_GUARD: i32 = 20921;
const KING_OF_THE_ARANEID: i32 = 20927;

// Misc
const MIN_LEVEL: i32 = 19;
const CRYSTAL_COUNT_1: i64 = 922;
const CRYSTAL_COUNT_2: i64 = 771;

const THE_LEADER_AND_THE_FOLLOWER: &str = "Q00123_TheLeaderAndTheFollower";

/// Java `Q00118_ToLeadAndBeLed`.
pub struct Q00118ToLeadAndBeLed;

impl Script for Q00118ToLeadAndBeLed {
    fn name(&self) -> &'static str {
        "Q00118_ToLeadAndBeLed"
    }

    fn path(&self) -> &'static str {
        "quests/Q00118_ToLeadAndBeLed"
    }

    fn id(&self) -> i32 {
        118
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACKSMITH_PINTER]);
        s.add_talk_id(&[BLACKSMITH_PINTER]);
        s.add_kill_id(&[
            MAILLE_LIZARDMAN,
            MAILLE_LIZARDMAN_SCOUT,
            MAILLE_LIZARDMAN_GUARD,
            KING_OF_THE_ARANEID,
        ]);
        s.register_quest_items(&[LEG_OF_KING_ARANEID, BLOOD_OF_MAILLE_LIZARDMAN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;

        // Manage Sponsor's quest events.
        if ctx.player_apprentice(player) > 0 {
            let apprentice = ObjectId(ctx.player_apprentice(player));
            // `World.getPlayer(..) == null`: the apprentice is not online.
            ctx.world().player(apprentice)?;

            let q118 = ctx.get_quest_state(apprentice, false);
            // NOTE(java-quirk): Java's `checkIfInRange(.., npc, ..)` throws on a null NPC; here it is false.
            let in_range = npc
                .is_some_and(|n| ctx.check_if_in_range(ctx.alt_party_range(), n.object_id, apprentice, true));
            match event {
                "sponsor" => {
                    if !in_range {
                        htmltext = Some("30298-09.html".to_owned());
                    } else if q118.is_none_or(|q| !q.is_memo_state(ctx, 2) && !q.is_memo_state(ctx, 3)) {
                        htmltext = Some("30298-14.html".to_owned());
                    } else if q118.is_some_and(|q| q.is_memo_state(ctx, 2)) {
                        htmltext = Some("30298-08.html".to_owned());
                    } else if q118.is_some_and(|q| q.is_memo_state(ctx, 3)) {
                        htmltext = Some("30298-12.html".to_owned());
                    }
                }
                "30298-10.html" => {
                    if in_range
                        && let Some(q118) = q118
                        && q118.is_memo_state(ctx, 2)
                    {
                        match q118.get_memo_state_ex(ctx, 1) {
                            1 => {
                                if ctx.get_quest_items_count(player, CRYSTAL_D) >= CRYSTAL_COUNT_1 {
                                    ctx.take_items(player, CRYSTAL_D, CRYSTAL_COUNT_1);
                                    q118.set_memo_state(ctx, 3);
                                    q118.set_cond(ctx, 6, true);
                                    htmltext = Some(event.to_owned());
                                } else {
                                    htmltext = Some("30298-11.html".to_owned());
                                }
                            }
                            2 | 3 => {
                                if ctx.get_quest_items_count(player, CRYSTAL_D) >= CRYSTAL_COUNT_2 {
                                    ctx.take_items(player, CRYSTAL_D, CRYSTAL_COUNT_2);
                                    q118.set_memo_state(ctx, 3);
                                    q118.set_cond(ctx, 6, true);
                                    htmltext = Some(event.to_owned());
                                } else {
                                    htmltext = Some("30298-11a.html".to_owned());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            return htmltext;
        }

        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30298-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30298-05a.html" | "30298-05b.html" | "30298-05c.html" | "30298-05g.html" => {
                htmltext = Some(event.to_owned());
            }
            "30298-05d.html" | "30298-05e.html" | "30298-05f.html" => {
                if qs.is_memo_state(ctx, 1)
                    && ctx.get_quest_items_count(player, BLOOD_OF_MAILLE_LIZARDMAN) >= 10
                {
                    let (ex, cond) = match event {
                        "30298-05d.html" => (1, 3),
                        "30298-05e.html" => (2, 4),
                        _ => (3, 5),
                    };
                    ctx.take_items(player, BLOOD_OF_MAILLE_LIZARDMAN, -1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_memo_state_ex(ctx, 1, ex);
                    qs.set_cond(ctx, cond, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match npc.id {
            MAILLE_LIZARDMAN | MAILLE_LIZARDMAN_SCOUT | MAILLE_LIZARDMAN_GUARD => {
                // Java's `giveItemRandomly(killer, npc, ITEM, 1, 10, 7, true)`: amount 1, limit 10 and a
                // drop chance of 7 (700%, so every kill drops).
                if qs.is_memo_state(ctx, 1)
                    && ctx.give_item_randomly(
                        killer,
                        Some(npc),
                        BLOOD_OF_MAILLE_LIZARDMAN,
                        1,
                        1,
                        10,
                        7.0,
                        true,
                    )
                {
                    qs.set_cond(ctx, 2, false);
                }
            }
            KING_OF_THE_ARANEID => {
                if qs.is_memo_state(ctx, 4) && ctx.player_sponsor(killer) > 0 {
                    let c0 = ObjectId(ctx.player_sponsor(killer));
                    if ctx.world().player(c0).is_some()
                        && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, c0, true)
                        && ctx.give_item_randomly(killer, Some(npc), LEG_OF_KING_ARANEID, 1, 1, 8, 7.0, true)
                    {
                        qs.set_cond(ctx, 8, false);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let (q123_started, q123_completed) = match ctx.player_quest_state(player, THE_LEADER_AND_THE_FOLLOWER)
        {
            Some(q) => (q.is_started(), q.is_completed()),
            None => (false, false),
        };
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if q123_started {
                    htmltext = "30298-02b.html".to_owned();
                } else if q123_completed {
                    htmltext = "30298-02a.htm".to_owned();
                } else if ctx.player_level(player) >= MIN_LEVEL
                    && ctx.player_pledge_type(player) == -1
                    && ctx.player_sponsor(player) > 0
                {
                    htmltext = "30298-01.htm".to_owned();
                } else {
                    htmltext = "30298-02.htm".to_owned();
                }
            }
            STARTED => {
                if qs.is_memo_state(ctx, 1) {
                    if ctx.get_quest_items_count(player, BLOOD_OF_MAILLE_LIZARDMAN) < 10 {
                        htmltext = "30298-04.html".to_owned();
                    } else {
                        htmltext = "30298-05.html".to_owned();
                    }
                } else if qs.is_memo_state(ctx, 2) {
                    if ctx.player_sponsor(player) == 0 {
                        match qs.get_memo_state_ex(ctx, 1) {
                            1 => htmltext = "30298-06a.html".to_owned(),
                            2 => htmltext = "30298-06b.html".to_owned(),
                            3 => htmltext = "30298-06c.html".to_owned(),
                            _ => {}
                        }
                    } else {
                        let c0 = ObjectId(ctx.player_sponsor(player));
                        if ctx.world().player(c0).is_some()
                            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, c0, true)
                        {
                            htmltext = "30298-07.html".to_owned();
                        } else {
                            match qs.get_memo_state_ex(ctx, 1) {
                                1 => htmltext = "30298-06.html".to_owned(),
                                2 => htmltext = "30298-06d.html".to_owned(),
                                3 => htmltext = "30298-06e.html".to_owned(),
                                _ => {}
                            }
                        }
                    }
                } else if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 7, true);
                    htmltext = "30298-15.html".to_owned();
                } else if qs.is_memo_state(ctx, 4) {
                    if ctx.get_quest_items_count(player, LEG_OF_KING_ARANEID) < 8 {
                        htmltext = "30298-16.html".to_owned();
                    } else {
                        let set: Option<[i32; 4]> = match qs.get_memo_state_ex(ctx, 1) {
                            1 => Some([
                                CLAN_OATH_HELM,
                                CLAN_OATH_ARMOR,
                                CLAN_OATH_GAUNTLETS_HEAVY_ARMOR,
                                CLAN_OATH_SABATON_HEAVY_ARMOR,
                            ]),
                            2 => Some([
                                CLAN_OATH_HELM,
                                CLAN_OATH_BRIGANDINE,
                                CLAN_OATH_LEATHER_GLOVES_LIGHT_ARMOR,
                                CLAN_OATH_BOOTS_LIGHT_ARMOR,
                            ]),
                            3 => Some([
                                CLAN_OATH_HELM,
                                CLAN_OATH_AKETON,
                                CLAN_OATH_PADDED_GLOVES_ROBE,
                                CLAN_OATH_SANDALS_ROBE,
                            ]),
                            _ => None,
                        };
                        if let Some(set) = set {
                            for id in set {
                                ctx.give_items(player, id, 1);
                            }
                            ctx.take_items(player, LEG_OF_KING_ARANEID, -1);
                        }
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30298-17.html".to_owned();
                    }
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{PlayerClanState, ScriptRegistry};

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest, item, player_data};

    const NPCS: &str = r#"
        <npc id="30298" type="Folk" level="30" name="Pinter"/>
        <npc id="20919" type="Monster" level="20" name="Maille Lizardman"/>
        <npc id="20927" type="Monster" level="20" name="King of the Araneid"/>"#;

    const ITEMS: &str = r#"
        <item id="1458" type="EtcItem" name="Crystal: D-Grade"><set name="is_stackable" val="true"/></item>
        <item id="8062" type="EtcItem" name="Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8063" type="EtcItem" name="Leg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7850" type="Armor" name="Helm"/>
        <item id="7851" type="Armor" name="Armor"/>
        <item id="7852" type="Armor" name="Gauntlets"/>
        <item id="7853" type="Armor" name="Sabaton"/>"#;

    const NAME: &str = "Q00118_ToLeadAndBeLed";

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00118ToLeadAndBeLed);
        let pages = Pages::of(&Q00118ToLeadAndBeLed)
            .page("30298-01.htm", &["30298-03.htm"])
            .plain(&["30298-02.htm", "30298-03.htm", "30298-04.html"])
            .page(
                "30298-05.html",
                &["30298-05a.html", "30298-05b.html", "30298-05c.html"],
            )
            .page("30298-05a.html", &["30298-05d.html", "30298-05g.html"])
            .page("30298-05b.html", &["30298-05e.html", "30298-05g.html"])
            .page("30298-05c.html", &["30298-05f.html", "30298-05g.html"])
            .plain(&[
                "30298-05d.html",
                "30298-05e.html",
                "30298-05f.html",
                "30298-05g.html",
            ])
            .plain(&[
                "30298-06.html",
                "30298-06a.html",
                "30298-07.html",
                "30298-15.html",
            ])
            .plain(&["30298-16.html", "30298-17.html"])
            .npcs(&[BLACKSMITH_PINTER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let pinter = t.spawn(BLACKSMITH_PINTER, 20);
        (t, pinter)
    }

    /// Player 1 is the academy member (sponsor 2), player 2 its sponsor (apprentice 1).
    fn enter_pair(t: &mut ScriptTest, crystals: i64) {
        let mut academy = player_data(1, 19, vec![]);
        academy.profile.clan = Some(PlayerClanState {
            clan_id: 0,
            pledge_type: -1,
            sponsor: 2,
            ..Default::default()
        });
        t.enter_with(academy);
        let mut sponsor = player_data(2, 40, vec![item(300, CRYSTAL_D, crystals)]);
        sponsor.profile.clan = Some(PlayerClanState {
            clan_id: 0,
            apprentice: 1,
            ..Default::default()
        });
        t.enter_with(sponsor);
    }

    /// Java `onEvent(event, npc, sponsor)` for the sponsor (the page link `sponsor` is in the NPC's
    /// default page, which the stand-ins do not have).
    fn sponsor_event(t: &mut ScriptTest, pinter: Oid, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(pinter);
            out = Q00118ToLeadAndBeLed.on_event(ctx, event, npc, Some(Oid(2)));
        });
        out
    }

    #[test]
    fn only_an_academy_member_with_a_sponsor_at_level_19_starts() {
        let (mut t, pinter) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, pinter).contains("30298-02.htm"));
        t.enter(3, 40, vec![]);
        assert!(t.quest_page(3, pinter).contains("30298-02.htm"));
        assert!(t.take_writes().is_empty());
        let (mut t, pinter) = setup();
        enter_pair(&mut t, 0);
        assert!(t.quest_page(1, pinter).contains("30298-01.htm"));
    }

    #[test]
    fn the_whole_quest_to_the_reward_of_the_heavy_armor_line() {
        let (mut t, pinter) = setup();
        enter_pair(&mut t, 1000);
        t.quest_page(1, pinter);
        assert!(t.event(1, NAME, "30298-03.htm").contains("30298-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                insert(NAME, "cond", "1"),
                insert(NAME, "<state>", "Started"),
                insert(NAME, "memoState", "1")
            ]
        );
        assert!(t.quest_page(1, pinter).contains("30298-04.html"));

        // Ten drops of blood: the limit sets cond 2.
        for _ in 0..10 {
            let mob = t.spawn(MAILLE_LIZARDMAN, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, BLOOD_OF_MAILLE_LIZARDMAN), 10);
        assert!(t.world.player_quests(Oid(1)).is_some());
        t.take_writes();
        assert!(t.quest_page(1, pinter).contains("30298-05.html"));
        assert!(t.event(1, NAME, "30298-05a.html").contains("30298-05a.html"));
        assert!(t.event(1, NAME, "30298-05d.html").contains("30298-05d.html"));
        assert_eq!(t.count(1, BLOOD_OF_MAILLE_LIZARDMAN), 0);
        assert_eq!(
            t.take_writes(),
            [
                update(NAME, "memoState", "2"),
                insert(NAME, "memoStateEx1", "1"),
                update(NAME, "cond", "3")
            ]
        );

        // The sponsor is next to Pinter: page 07, then the sponsor pays 922 crystals.
        assert!(t.quest_page(1, pinter).contains("30298-07.html"));
        assert_eq!(
            sponsor_event(&mut t, pinter, "sponsor").as_deref(),
            Some("30298-08.html")
        );
        assert_eq!(
            sponsor_event(&mut t, pinter, "30298-10.html").as_deref(),
            Some("30298-10.html")
        );
        assert_eq!(t.count(2, CRYSTAL_D), 78);
        assert_eq!(
            t.take_writes(),
            [
                update(NAME, "memoState", "3"),
                update(NAME, "cond", "6"),
                // cond 3 to 6 skips steps: `setCond` records them in `__compltdStateFlags`.
                insert(NAME, "__compltdStateFlags", "-2147483609")
            ]
        );
        assert_eq!(
            sponsor_event(&mut t, pinter, "sponsor").as_deref(),
            Some("30298-12.html")
        );

        // memo 3 -> 4 at the next talk, then the King of the Araneid drops eight legs.
        assert!(t.quest_page(1, pinter).contains("30298-15.html"));
        assert_eq!(
            t.take_writes(),
            [
                update(NAME, "memoState", "4"),
                update(NAME, "cond", "7"),
                update(NAME, "__compltdStateFlags", "-2147483545")
            ]
        );
        assert!(t.quest_page(1, pinter).contains("30298-16.html"));
        for _ in 0..8 {
            let king = t.spawn(KING_OF_THE_ARANEID, 60);
            t.kill(1, king);
        }
        assert_eq!(t.count(1, LEG_OF_KING_ARANEID), 8);
        t.take_writes();
        assert!(t.quest_page(1, pinter).contains("30298-17.html"));
        for id in [7850, 7851, 7852, 7853] {
            assert_eq!(t.count(1, id), 1);
        }
        assert_eq!(t.count(1, LEG_OF_KING_ARANEID), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }

    #[test]
    fn the_sponsor_without_enough_crystals_is_refused() {
        let (mut t, pinter) = setup();
        enter_pair(&mut t, 100);
        t.quest_page(1, pinter);
        t.event(1, NAME, "30298-03.htm");
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 2);
            qs.set_memo_state_ex(ctx, 1, 2);
        });
        t.take_writes();
        assert_eq!(
            sponsor_event(&mut t, pinter, "30298-10.html").as_deref(),
            Some("30298-11a.html")
        );
        assert_eq!(t.count(2, CRYSTAL_D), 100);
        assert!(t.take_writes().is_empty());
    }
}
