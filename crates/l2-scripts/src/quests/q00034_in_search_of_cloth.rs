//! Quest 34: In Search of Cloth (Radia in the Talking Island, Ralford and Varan in the Gludio
//! and Talking Island spiders' hunt).
// Java: dist/game/data/scripts/quests/Q00034_InSearchOfCloth/Q00034_InSearchOfCloth.java
//
// Pattern: a kill-count chain with a party-member check (`docs/design/scripts.md`, "Quest
// patterns"): the spiders give spinnerets to a player at cond 4 with a 50% roll, then the
// delivery chain moves on through Ralford and Radia.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RADIA: i32 = 30088;
const RALFORD: i32 = 30165;
const VARAN: i32 = 30294;

// Monsters
const MOBS: [i32; 2] = [
    20560, // Trisalim Spider
    20561, // Trisalim Tarantula
];

// Items
const SUEDE: i32 = 1866;
const THREAD: i32 = 1868;
const MYSTERIOUS_CLOTH: i32 = 7076;
const SKEIN_OF_YARN: i32 = 7161;
const SPINNERET: i32 = 7528;

// Misc
const MIN_LEVEL: i32 = 60;
const SPINNERET_COUNT: i64 = 10;
const SUEDE_COUNT: i64 = 3000;
const THREAD_COUNT: i64 = 5000;

/// Java `Q00034_InSearchOfCloth`.
pub struct Q00034InSearchOfCloth;

impl Script for Q00034InSearchOfCloth {
    fn name(&self) -> &'static str {
        "Q00034_InSearchOfCloth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00034_InSearchOfCloth"
    }

    fn id(&self) -> i32 {
        34
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RADIA]);
        s.add_talk_id(&[RADIA, RALFORD, VARAN]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[SKEIN_OF_YARN, SPINNERET]);
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30088-03.htm" => {
                qs.start_quest(ctx);
            }
            "30294-02.html" => {
                qs.set_cond(ctx, 2, true);
            }
            "30088-06.html" => {
                qs.set_cond(ctx, 3, true);
            }
            "30165-02.html" => {
                qs.set_cond(ctx, 4, true);
            }
            "30165-05.html" => {
                if ctx.get_quest_items_count(player, SPINNERET) < SPINNERET_COUNT {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, SPINNERET, SPINNERET_COUNT);
                ctx.give_items(player, SKEIN_OF_YARN, 1);
                qs.set_cond(ctx, 6, true);
            }
            "30088-10.html" => {
                if ctx.get_quest_items_count(player, SUEDE) >= SUEDE_COUNT
                    && ctx.get_quest_items_count(player, THREAD) >= THREAD_COUNT
                    && ctx.has_quest_items(player, SKEIN_OF_YARN)
                {
                    ctx.take_items(player, SKEIN_OF_YARN, 1);
                    ctx.take_items(player, SUEDE, SUEDE_COUNT);
                    ctx.take_items(player, THREAD, THREAD_COUNT);
                    ctx.give_items(player, MYSTERIOUS_CLOTH, 1);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = Some("30088-11.html".to_owned());
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        // Java: `getRandomPartyMember(player, 4)` is a member at cond 4; the roll is taken only
        // when there is one.
        if let Some(member) = ctx.get_random_party_member_cond(player, 4)
            && ctx.get_random_boolean()
        {
            let qs = ctx.get_quest_state(member, false);
            // NOTE(java-quirk): the spinneret goes to the killer, not to the member drawn, and
            // the cond 5 check reads the member's state.
            ctx.give_items(player, SPINNERET, 1);
            if ctx.get_quest_items_count(player, SPINNERET) >= SPINNERET_COUNT {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 5, true);
                }
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            RADIA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30088-01.htm"
                    } else {
                        "30088-02.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30088-04.html".to_owned(),
                    2 => htmltext = "30088-05.html".to_owned(),
                    3 => htmltext = "30088-07.html".to_owned(),
                    6 => {
                        htmltext = if ctx.get_quest_items_count(player, SUEDE) >= SUEDE_COUNT
                            && ctx.get_quest_items_count(player, THREAD) >= THREAD_COUNT
                        {
                            "30088-08.html"
                        } else {
                            "30088-09.html"
                        }
                        .to_owned();
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            VARAN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30294-01.html".to_owned(),
                        2 => htmltext = "30294-03.html".to_owned(),
                        _ => {}
                    }
                }
            }
            RALFORD => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        3 => htmltext = "30165-01.html".to_owned(),
                        4 => htmltext = "30165-03.html".to_owned(),
                        5 => htmltext = "30165-04.html".to_owned(),
                        6 => htmltext = "30165-06.html".to_owned(),
                        _ => {}
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00034_InSearchOfCloth";

    const NPCS: &str = r#"
        <npc id="30088" type="Folk" level="70" name="Radia"/>
        <npc id="30165" type="Folk" level="70" name="Ralford"/>
        <npc id="30294" type="Folk" level="70" name="Varan"/>
        <npc id="20560" type="Monster" level="30" name="Trisalim Spider"/>
        <npc id="20561" type="Monster" level="40" name="Trisalim Tarantula"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1866" type="EtcItem" name="Suede"><set name="is_stackable" val="true"/></item>
        <item id="1868" type="EtcItem" name="Thread"><set name="is_stackable" val="true"/></item>
        <item id="7076" type="EtcItem" name="Mysterious Cloth"/>
        <item id="7161" type="EtcItem" name="Skein of Yarn"><set name="is_questitem" val="true"/></item>
        <item id="7528" type="EtcItem" name="Spinneret"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        radia: Oid,
        ralford: Oid,
        varan: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00034InSearchOfCloth);
        let pages = Pages::of(&Q00034InSearchOfCloth)
            .page("30088-01.htm", &["30088-03.htm"])
            .page("30088-05.html", &["30088-06.html"])
            .page("30088-08.html", &["30088-10.html"])
            .page("30165-01.html", &["30165-02.html"])
            .page("30165-04.html", &["30165-05.html"])
            .page("30294-01.html", &["30294-02.html"])
            .plain(&[
                "30088-02.html",
                "30088-03.htm",
                "30088-04.html",
                "30088-06.html",
                "30088-07.html",
                "30088-09.html",
                "30088-10.html",
                "30088-11.html",
                "30165-02.html",
                "30165-03.html",
                "30165-05.html",
                "30165-06.html",
                "30294-02.html",
                "30294-03.html",
            ])
            .npcs(&[RADIA, RALFORD, VARAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            radia: t.spawn(RADIA, 20),
            ralford: t.spawn(RALFORD, 40),
            varan: t.spawn(VARAN, 60),
        };
        (t, npcs)
    }

    /// Kills spiders until the player has 10 spinnerets; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let spider = t.spawn(MOBS[0], 80);
            t.kill(1, spider);
            if t.count(1, SPINNERET) >= SPINNERET_COUNT {
                return kills;
            }
        }
        panic!("no 10 spinnerets after 400 kills");
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, npcs.radia).contains("30088-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_radia_to_the_cloth() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![item(1001, SUEDE, 3000), item(1002, THREAD, 5000)]);
        assert!(t.quest_page(1, npcs.radia).contains("30088-01.htm"));
        assert!(t.event(1, NAME, "30088-03.htm").contains("30088-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.radia).contains("30088-04.html"));

        // Varan sends the player on to Radia, Radia to Ralford.
        assert!(t.quest_page(1, npcs.varan).contains("30294-01.html"));
        assert!(t.event(1, NAME, "30294-02.html").contains("30294-02.html"));
        assert!(t.quest_page(1, npcs.radia).contains("30088-05.html"));
        assert!(t.event(1, NAME, "30088-06.html").contains("30088-06.html"));
        assert!(t.quest_page(1, npcs.ralford).contains("30165-01.html"));
        assert!(t.event(1, NAME, "30165-02.html").contains("30165-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "cond", "3"),
                rows::update(NAME, "cond", "4"),
            ]
        );

        // The spiders give spinnerets to the killer (a roll of 50%); the 10th sets cond 5.
        let kills = hunt(&mut t);
        assert!(kills >= 10, "{kills}");
        assert_eq!(t.count(1, SPINNERET), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.ralford).contains("30165-04.html"));

        // Ralford takes the spinnerets and gives the skein of yarn: cond 6.
        assert!(t.event(1, NAME, "30165-05.html").contains("30165-05.html"));
        assert_eq!((t.count(1, SPINNERET), t.count(1, SKEIN_OF_YARN)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Radia has the suede and the thread: the cloth, the end.
        assert!(t.quest_page(1, npcs.radia).contains("30088-08.html"));
        assert!(t.event(1, NAME, "30088-10.html").contains("30088-10.html"));
        assert_eq!(t.count(1, MYSTERIOUS_CLOTH), 1);
        assert_eq!(
            (t.count(1, SUEDE), t.count(1, THREAD), t.count(1, SKEIN_OF_YARN)),
            (0, 0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.radia)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn too_few_spinnerets_and_missing_cloth_materials_are_refused() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![item(1001, SUEDE, 3000)]);
        t.quest_page(1, npcs.radia);
        t.event(1, NAME, "30088-03.htm");
        t.quest_page(1, npcs.radia);
        t.event(1, NAME, "30294-02.html");
        t.event(1, NAME, "30088-06.html");
        t.event(1, NAME, "30165-02.html");
        t.take_writes();

        // Ralford's `30165-05.html` with no spinnerets: the no-quest page, nothing taken. The
        // real page is only reached at cond 5, so the event is run directly (`run_as_script`).
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00034InSearchOfCloth.on_event(ctx, "30165-05.html", None, Some(Oid(1)));
        });
        assert!(out.unwrap().contains("not on a quest"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, SKEIN_OF_YARN), 0);

        // Radia's cloth without the thread: the page for the missing materials, nothing given.
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00034InSearchOfCloth.on_event(ctx, "30088-10.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30088-11.html"));
        assert_eq!(t.count(1, MYSTERIOUS_CLOTH), 0);
        assert_eq!(t.count(1, SUEDE), 3000);
        assert!(t.take_writes().is_empty());
    }
}
