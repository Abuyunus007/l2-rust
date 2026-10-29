//! Quest 21: Hidden Truth (the Mysterious Wizard, a tombstone, the ghost of von Hellmann and his page, a broken bookshelf).
// Java: dist/game/data/scripts/quests/Q00021_HiddenTruth/Q00021_HiddenTruth.java
//
// Pattern: a ghost story with a moving NPC (`docs/design/scripts.md`, "Quest patterns"): the
// tombstone wakes the ghost (cond 2), the ghost sends its page along a route (cond 3), the page
// walks back and the bookshelf gives the cross of Einhasad (cond 6). The three priests Agripel,
// Benedict and Dominic each check off one of their flags (cond 7), and Innocentin pays the reward.
// One-time: `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): the route-finished DESPAWN timer is started without a player, and Java's
// `getQuestState(null, ..)` throws there, which `notifyEvent` catches: the page is never
// despawned by that timer (only the talk path despawns it).
// The page walks the datapack route `rune_ghost1b` through `World::walking_start_moving`; the
// unit tests have no walker data, so they call the route-finished hook themselves.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const INNOCENTIN: i32 = 31328;
const AGRIPEL: i32 = 31348;
const BENEDICT: i32 = 31349;
const DOMINIC: i32 = 31350;
const MYSTERIOUS_WIZARD: i32 = 31522;
const TOMBSTONE: i32 = 31523;
const GHOST_OF_VON_HELLMAN: i32 = 31524;
const GHOST_OF_VON_HELLMANS_PAGE: i32 = 31525;
const BROKEN_BOOKSHELF: i32 = 31526;

/// Java `GHOST_LOC` and `PAGE_LOC` (heading 0).
const GHOST_LOC: (i32, i32, i32) = (51_432, -54_570, -3_136);
const PAGE_LOC: (i32, i32, i32) = (51_446, -54_514, -3_136);

// Items
const CROSS_OF_EINHASAD: i32 = 7140;
const CROSS_OF_EINHASAD2: i32 = 7141;

// Misc
const MIN_LEVEL: i32 = 63;
const PAGE_ROUTE_NAME: &str = "rune_ghost1b";
/// Java `Q00022_TragedyInVonHellmannForest.class.getSimpleName()`.
const TRAGEDY_IN_VON_HELLMANN_FOREST: &str = "Q00022_TragedyInVonHellmannForest";
/// Java `getName()`.
const NAME: &str = "Q00021_HiddenTruth";

/// The Java fields of the script: the static `PAGE_COUNT` and `GHOST_SPAWNED`, and the instance
/// `PAGE_SPAWNED` and `MOVE_ENDED` (one instance of the script, so all four are one state).
#[derive(Default)]
struct HiddenTruthState {
    page_count: i32,
    ghost_spawned: bool,
    page_spawned: bool,
    move_ended: bool,
}

/// Java `Q00021_HiddenTruth`.
pub struct Q00021HiddenTruth;

impl Script for Q00021HiddenTruth {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00021_HiddenTruth"
    }

    fn id(&self) -> i32 {
        21
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MYSTERIOUS_WIZARD]);
        s.add_talk_id(&[
            MYSTERIOUS_WIZARD,
            TOMBSTONE,
            GHOST_OF_VON_HELLMAN,
            GHOST_OF_VON_HELLMANS_PAGE,
            BROKEN_BOOKSHELF,
            AGRIPEL,
            BENEDICT,
            DOMINIC,
            INNOCENTIN,
        ]);
        s.add_route_finished_id(&[GHOST_OF_VON_HELLMANS_PAGE]);
        s.add_creature_see_id(&[GHOST_OF_VON_HELLMANS_PAGE]);
        s.register_quest_items(&[CROSS_OF_EINHASAD]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): a timer without a player (the route's DESPAWN) makes Java's
        // `getQuestState(null, false)` throw, and `notifyEvent` swallows it: nothing runs.
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "31328-02.html" | "31328-03.html" | "31328-04.html" | "31522-01.htm" | "31522-04.html"
            | "31523-02.html" | "31524-02.html" | "31524-03.html" | "31524-04.html" | "31524-05.html"
            | "31526-01.html" | "31526-02.html" | "31526-04.html" | "31526-05.html" | "31526-06.html"
            | "31526-12.html" | "31526-13.html" => {
                htmltext = Some(event.to_owned());
            }
            "31328-05.html" => {
                if qs.is_cond(ctx, 7) {
                    ctx.give_items(player, CROSS_OF_EINHASAD2, 1);
                    ctx.add_exp_and_sp(player, 131_228, 11_978);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-02.htm" => {
                if ctx.player_level(player) < MIN_LEVEL {
                    htmltext = Some("31522-03.htm".to_owned());
                } else {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31523-03.html" => {
                if ctx.state::<HiddenTruthState>().ghost_spawned {
                    htmltext = Some("31523-04.html".to_owned());
                    ctx.play_sound(player, QuestSound::SkillsoundHorror02);
                } else {
                    // Java: `addSpawn(GHOST, GHOST_LOC, false, 0)` and its "Who awoke me?".
                    if let Some(ghost) = ctx.add_spawn(
                        GHOST_OF_VON_HELLMAN,
                        GHOST_LOC.0,
                        GHOST_LOC.1,
                        GHOST_LOC.2,
                        0,
                        false,
                        0,
                        0,
                    ) {
                        ctx.broadcast_npc_say(ghost, ChatType::General, NpcStringId::WhoAwokeMe, &[]);
                        ctx.state::<HiddenTruthState>().ghost_spawned = true;
                        ctx.start_quest_timer("DESPAWN_GHOST", 300_000, Some(ghost), Some(player), false);
                    }
                    qs.set_cond(ctx, 2, false);
                    ctx.play_sound(player, QuestSound::SkillsoundHorror02);
                    htmltext = Some(event.to_owned());
                }
            }
            "31524-06.html" => {
                if ctx.state::<HiddenTruthState>().page_count < 5 {
                    // Java: the page, its script value (the player), its NPC say and its route.
                    if let Some(page) = ctx.add_spawn(
                        GHOST_OF_VON_HELLMANS_PAGE,
                        PAGE_LOC.0,
                        PAGE_LOC.1,
                        PAGE_LOC.2,
                        0,
                        false,
                        0,
                        0,
                    ) {
                        ctx.set_script_value(page, player.0);
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            page,
                            ChatType::NpcGeneral,
                            NpcStringId::MyMasterHasInstructedMeToBeYourGuideS1,
                            &[&name],
                        );
                        ctx.world_mut()
                            .walking_start_moving(page.object_id, PAGE_ROUTE_NAME);
                    }
                    ctx.state::<HiddenTruthState>().page_count += 1;
                    qs.set_cond(ctx, 3, false);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31524-06a.html".to_owned());
                }
            }
            "31526-03.html" => {
                ctx.play_sound(player, QuestSound::ItemsoundArmorCloth);
                htmltext = Some(event.to_owned());
            }
            "31526-07.html" => {
                qs.set_cond(ctx, 4, false);
                htmltext = Some(event.to_owned());
            }
            "31526-08.html" => {
                if !qs.is_cond(ctx, 5) {
                    ctx.play_sound(player, QuestSound::AmdsoundEdChimes);
                    qs.set_cond(ctx, 5, false);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31526-09.html".to_owned());
                }
            }
            "31526-14.html" => {
                ctx.give_items(player, CROSS_OF_EINHASAD, 1);
                qs.set_cond(ctx, 6, false);
                htmltext = Some(event.to_owned());
            }
            "DESPAWN_GHOST" => {
                ctx.state::<HiddenTruthState>().ghost_spawned = false;
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
            }
            "DESPAWN" => {
                ctx.state::<HiddenTruthState>().page_count -= 1;
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MYSTERIOUS_WIZARD => match qs.state(ctx) {
                CREATED => htmltext = "31522-01.htm".to_owned(),
                STARTED => htmltext = "31522-05.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            TOMBSTONE => htmltext = "31523-01.html".to_owned(),
            GHOST_OF_VON_HELLMAN => match qs.get_cond(ctx) {
                2 => htmltext = "31524-01.html".to_owned(),
                3 => {
                    if ctx.state::<HiddenTruthState>().page_spawned {
                        htmltext = "31524-07b.html".to_owned();
                    } else if ctx.state::<HiddenTruthState>().page_count < 5 {
                        // Java: `addSpawn(PAGE, PAGE_LOC, true, 0)`: the page walks for this player.
                        if let Some(page) = ctx.add_spawn(
                            GHOST_OF_VON_HELLMANS_PAGE,
                            PAGE_LOC.0,
                            PAGE_LOC.1,
                            PAGE_LOC.2,
                            0,
                            true,
                            0,
                            0,
                        ) {
                            let st = ctx.state::<HiddenTruthState>();
                            st.page_count += 1;
                            st.page_spawned = true;
                            ctx.set_script_value(page, player.0);
                            ctx.world_mut()
                                .walking_start_moving(page.object_id, PAGE_ROUTE_NAME);
                        }
                        htmltext = "31524-07.html".to_owned();
                    } else {
                        htmltext = "31524-07a.html".to_owned();
                    }
                }
                4 => htmltext = "31524-07c.html".to_owned(),
                _ => {}
            },
            GHOST_OF_VON_HELLMANS_PAGE => {
                if qs.is_cond(ctx, 3) {
                    if ctx.state::<HiddenTruthState>().move_ended {
                        htmltext = "31525-02.html".to_owned();
                        ctx.start_quest_timer("DESPAWN", 3000, Some(npc), Some(player), false);
                    } else {
                        htmltext = "31525-01.html".to_owned();
                    }
                }
            }
            BROKEN_BOOKSHELF => match qs.get_cond(ctx) {
                3 => htmltext = "31526-01.html".to_owned(),
                4 => {
                    qs.set_cond(ctx, 5, false);
                    ctx.play_sound(player, QuestSound::AmdsoundEdChimes);
                    htmltext = "31526-10.html".to_owned();
                }
                5 => htmltext = "31526-11.html".to_owned(),
                6 => htmltext = "31526-15.html".to_owned(),
                _ => {}
            },
            AGRIPEL => {
                if ctx.has_quest_items(player, CROSS_OF_EINHASAD) && qs.is_cond(ctx, 6) {
                    qs.set(ctx, "AGRIPEL", "1");
                    if qs.get_int(ctx, "AGRIPEL") == 1
                        && qs.get_int(ctx, "DOMINIC") == 1
                        && qs.get_int(ctx, "BENEDICT") == 1
                    {
                        htmltext = "31348-03.html".to_owned();
                        qs.set_cond(ctx, 7, false);
                    } else if qs.get_int(ctx, "DOMINIC") == 1 || qs.get_int(ctx, "BENEDICT") == 1 {
                        htmltext = "31348-02.html".to_owned();
                    } else {
                        htmltext = "31348-01.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "31348-03.html".to_owned();
                }
            }
            BENEDICT => {
                if ctx.has_quest_items(player, CROSS_OF_EINHASAD) && qs.is_cond(ctx, 6) {
                    qs.set(ctx, "BENEDICT", "1");
                    if qs.get_int(ctx, "AGRIPEL") == 1
                        && qs.get_int(ctx, "DOMINIC") == 1
                        && qs.get_int(ctx, "BENEDICT") == 1
                    {
                        htmltext = "31349-03.html".to_owned();
                        qs.set_cond(ctx, 7, false);
                    } else if qs.get_int(ctx, "AGRIPEL") == 1 || qs.get_int(ctx, "DOMINIC") == 1 {
                        htmltext = "31349-02.html".to_owned();
                    } else {
                        htmltext = "31349-01.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "31349-03.html".to_owned();
                }
            }
            DOMINIC => {
                if ctx.has_quest_items(player, CROSS_OF_EINHASAD) && qs.is_cond(ctx, 6) {
                    qs.set(ctx, "DOMINIC", "1");
                    if qs.get_int(ctx, "AGRIPEL") == 1
                        && qs.get_int(ctx, "DOMINIC") == 1
                        && qs.get_int(ctx, "BENEDICT") == 1
                    {
                        htmltext = "31350-03.html".to_owned();
                        qs.set_cond(ctx, 7, false);
                    } else if qs.get_int(ctx, "AGRIPEL") == 1 || qs.get_int(ctx, "BENEDICT") == 1 {
                        htmltext = "31350-02.html".to_owned();
                    } else {
                        htmltext = "31350-01.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "31350-03.html".to_owned();
                }
            }
            INNOCENTIN => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, CROSS_OF_EINHASAD) {
                    htmltext = "31328-01.html".to_owned();
                } else if qs.is_completed(ctx)
                    && ctx
                        .player_quest_state(player, TRAGEDY_IN_VON_HELLMANN_FOREST)
                        .is_none()
                {
                    htmltext = "31328-06.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, creature: ObjectId) {
        // Java: `creature.isPlayer()`.
        if ctx.world().player(creature).is_some() {
            ctx.play_sound(creature, QuestSound::Horror01);
        }
    }

    fn on_route_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        // Java: `World.getPlayer(npc.getScriptValue()).getQuestState(getName())`.
        let player = ObjectId(ctx.get_script_value(npc));
        if ctx.player_quest_state(player, NAME).is_some() {
            ctx.start_quest_timer("DESPAWN", 15_000, Some(npc), None, false);
            ctx.state::<HiddenTruthState>().move_ended = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NPCS: &str = r#"
        <npc id="31522" type="Folk" level="70" name="Mysterious Wizard"/>
        <npc id="31523" type="Folk" level="70" name="Tombstone"/>
        <npc id="31524" type="Folk" level="70" name="Ghost of von Hellmann"/>
        <npc id="31525" type="Folk" level="70" name="Ghost of von Hellmann's Page"/>
        <npc id="31526" type="Folk" level="70" name="Broken Bookshelf"/>
        <npc id="31348" type="Folk" level="70" name="Agripel"/>
        <npc id="31349" type="Folk" level="70" name="Benedict"/>
        <npc id="31350" type="Folk" level="70" name="Dominic"/>
        <npc id="31328" type="Folk" level="70" name="Innocentin"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7140" type="EtcItem" name="Cross of Einhasad"><set name="is_questitem" val="true"/></item>
        <item id="7141" type="EtcItem" name="Cross of Einhasad"/>"#;

    struct Npcs {
        wizard: Oid,
        tombstone: Oid,
        ghost: Oid,
        bookshelf: Oid,
        agripel: Oid,
        benedict: Oid,
        dominic: Oid,
        innocentin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00021HiddenTruth);
        let pages = Pages::of(&Q00021HiddenTruth)
            .page("31522-01.htm", &["31522-02.htm", "31522-04.html"])
            .page("31523-01.html", &["31523-02.html"])
            .page("31523-02.html", &["31523-03.html"])
            .page("31524-01.html", &["31524-02.html"])
            .page("31524-02.html", &["31524-03.html"])
            .page("31524-03.html", &["31524-04.html"])
            .page("31524-04.html", &["31524-05.html"])
            .page("31524-05.html", &["31524-06.html"])
            .page("31526-01.html", &["31526-02.html", "31526-03.html"])
            .page("31526-03.html", &["31526-04.html"])
            .page("31526-04.html", &["31526-05.html"])
            .page("31526-05.html", &["31526-06.html"])
            .page("31526-06.html", &["31526-07.html"])
            .page("31526-07.html", &["31526-08.html"])
            .page("31526-08.html", &["31526-12.html"])
            .page("31526-12.html", &["31526-14.html", "31526-13.html"])
            .page("31328-01.html", &["31328-02.html"])
            .page("31328-02.html", &["31328-03.html"])
            .page("31328-03.html", &["31328-04.html"])
            .page("31328-04.html", &["31328-05.html"])
            .plain(&[
                "31522-02.htm",
                "31522-03.htm",
                "31522-04.html",
                "31522-05.html",
                "31523-03.html",
                "31523-04.html",
                "31524-06.html",
                "31524-06a.html",
                "31524-07.html",
                "31524-07a.html",
                "31524-07b.html",
                "31524-07c.html",
                "31525-01.html",
                "31525-02.html",
                "31526-09.html",
                "31526-10.html",
                "31526-11.html",
                "31526-14.html",
                "31526-15.html",
                "31348-01.html",
                "31348-02.html",
                "31348-03.html",
                "31349-01.html",
                "31349-02.html",
                "31349-03.html",
                "31350-01.html",
                "31350-02.html",
                "31350-03.html",
                "31328-05.html",
                "31328-06.html",
            ])
            .npcs(&[
                MYSTERIOUS_WIZARD,
                TOMBSTONE,
                GHOST_OF_VON_HELLMAN,
                GHOST_OF_VON_HELLMANS_PAGE,
                BROKEN_BOOKSHELF,
                AGRIPEL,
                BENEDICT,
                DOMINIC,
                INNOCENTIN,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            wizard: t.spawn(MYSTERIOUS_WIZARD, 20),
            tombstone: t.spawn(TOMBSTONE, 40),
            ghost: t.spawn(GHOST_OF_VON_HELLMAN, 60),
            bookshelf: t.spawn(BROKEN_BOOKSHELF, 80),
            agripel: t.spawn(AGRIPEL, 100),
            benedict: t.spawn(BENEDICT, 120),
            dominic: t.spawn(DOMINIC, 140),
            innocentin: t.spawn(INNOCENTIN, 160),
        };
        (t, npcs)
    }

    #[test]
    fn the_wizard_wants_level_63() {
        let (mut t, npcs) = setup();
        t.enter(1, 62, vec![]);
        assert!(t.quest_page(1, npcs.wizard).contains("31522-01.htm"));
        assert!(t.event(1, NAME, "31522-02.htm").contains("31522-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_tombstone_ghost_page_bookshelf_crosses_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 63, vec![]);
        assert!(t.quest_page(1, npcs.wizard).contains("31522-01.htm"));
        assert!(t.event(1, NAME, "31522-02.htm").contains("31522-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.wizard).contains("31522-05.html"));

        // The tombstone wakes the ghost (cond 2).
        assert!(t.quest_page(1, npcs.tombstone).contains("31523-01.html"));
        assert!(t.event(1, NAME, "31523-02.html").contains("31523-02.html"));
        // The ghost of the setup stands by; the tombstone's event adds one more.
        let ghosts = t.npcs_of(GHOST_OF_VON_HELLMAN).len();
        assert!(t.event(1, NAME, "31523-03.html").contains("31523-03.html"));
        assert_eq!(t.npcs_of(GHOST_OF_VON_HELLMAN).len(), ghosts + 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The ghost sends its page (cond 3).
        assert!(t.quest_page(1, npcs.ghost).contains("31524-01.html"));
        for event in ["31524-02.html", "31524-03.html", "31524-04.html", "31524-05.html"] {
            assert!(t.event(1, NAME, event).contains(event));
        }
        assert!(t.event(1, NAME, "31524-06.html").contains("31524-06.html"));
        assert_eq!(t.npcs_of(GHOST_OF_VON_HELLMANS_PAGE).len(), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The page walks back: the route's end is the page's ("walking" is the datapack's).
        // The page stands far from the players, so its talk runs through the script directly.
        let page = t.npcs_of(GHOST_OF_VON_HELLMANS_PAGE)[0];
        t.world.run_as_script(NAME, |ctx| {
            let Some(npc) = ctx.script_npc(page) else {
                panic!("the page is in the world");
            };
            assert_eq!(
                Q00021HiddenTruth.on_talk(ctx, npc, Oid(1)).as_deref(),
                Some("31525-01.html")
            );
            Q00021HiddenTruth.on_route_finished(ctx, npc);
            assert_eq!(
                Q00021HiddenTruth.on_talk(ctx, npc, Oid(1)).as_deref(),
                Some("31525-02.html")
            );
        });
        t.wait_ms(3_000);
        assert!(!t.in_world(page));

        // The bookshelf gives the cross (cond 6).
        assert!(t.quest_page(1, npcs.bookshelf).contains("31526-01.html"));
        assert!(t.event(1, NAME, "31526-03.html").contains("31526-03.html"));
        for event in [
            "31526-04.html",
            "31526-05.html",
            "31526-06.html",
            "31526-07.html",
            "31526-08.html",
        ] {
            assert!(t.event(1, NAME, event).contains(event), "{event}");
        }
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "cond", "4"), rows::update(NAME, "cond", "5")]
        );
        assert!(t.event(1, NAME, "31526-12.html").contains("31526-12.html"));
        assert!(t.event(1, NAME, "31526-14.html").contains("31526-14.html"));
        assert_eq!(t.count(1, CROSS_OF_EINHASAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // The three priests, each checks a flag; Dominic, the last, sets cond 7.
        assert!(t.quest_page(1, npcs.agripel).contains("31348-01.html"));
        assert!(t.quest_page(1, npcs.benedict).contains("31349-02.html"));
        assert!(t.quest_page(1, npcs.dominic).contains("31350-03.html"));
        assert_eq!(t.take_writes().len(), 4);

        // Innocentin: the second cross and the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.innocentin).contains("31328-01.html"));
        for event in ["31328-02.html", "31328-03.html", "31328-04.html"] {
            assert!(t.event(1, NAME, event).contains(event));
        }
        assert!(t.event(1, NAME, "31328-05.html").contains("31328-05.html"));
        assert_eq!(t.count(1, CROSS_OF_EINHASAD2), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.innocentin).contains("31328-06.html"));
    }
}
