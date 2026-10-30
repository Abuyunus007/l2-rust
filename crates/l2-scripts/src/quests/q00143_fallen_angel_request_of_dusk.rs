//! Quest 143: Fallen Angel - Request of Dusk (started by Q00998_FallenAngelSelect).
// Java: dist/game/data/scripts/quests/Q00143_FallenAngelRequestOfDusk/Q00143_FallenAngelRequestOfDusk.java
//
// Pattern: no start NPC (the select quest notifies "30894-01.html"), a `talk` memo, a rock that
// spawns an angel to talk to (`addSpawn`, `deleteMe`) and the `isAngelSpawned` field.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TOBIAS: i32 = 30297;
const CASIAN: i32 = 30612;
const NATOOLS: i32 = 30894;
const ROCK: i32 = 32368;
const ANGEL: i32 = 32369;

// Items
const SEALED_PROPHECY_PATH_OF_THE_GOD: i32 = 10354;
const PROPHECY_PATH_OF_THE_GOD: i32 = 10355;
const EMPTY_SOUND_CRYSTAL: i32 = 10356;
const ANGEL_MEDICINE: i32 = 10357;
const ANGELS_MESSAGE: i32 = 10358;

// Misc
const MAX_REWARD_LEVEL: i32 = 43;

/// Java's field `isAngelSpawned`, kept by the world (D-10).
#[derive(Default)]
struct Fields {
    is_angel_spawned: bool,
}

/// Java `Q00143_FallenAngelRequestOfDusk`.
pub struct Q00143FallenAngelRequestOfDusk;

impl Script for Q00143FallenAngelRequestOfDusk {
    fn name(&self) -> &'static str {
        "Q00143_FallenAngelRequestOfDusk"
    }

    fn path(&self) -> &'static str {
        "quests/Q00143_FallenAngelRequestOfDusk"
    }

    fn id(&self) -> i32 {
        143
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[NATOOLS, TOBIAS, CASIAN, ROCK, ANGEL]);
        s.register_quest_items(&[
            SEALED_PROPHECY_PATH_OF_THE_GOD,
            PROPHECY_PATH_OF_THE_GOD,
            EMPTY_SOUND_CRYSTAL,
            ANGEL_MEDICINE,
            ANGELS_MESSAGE,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = Some(event.to_owned());
        match event {
            "30894-02.html" | "30297-04.html" | "30612-05.html" | "30612-06.html" | "30612-07.html"
            | "30612-08.html" | "32369-04.html" | "32369-05.html" | "32369-07.html" | "32369-08.html"
            | "32369-09.html" | "32369-10.html" => {}
            "30894-01.html" => qs.start_quest(ctx),
            "30894-03.html" => {
                qs.set_cond(ctx, 2, true);
                ctx.give_items(player, SEALED_PROPHECY_PATH_OF_THE_GOD, 1);
            }
            "30297-03.html" => {
                ctx.take_items(player, SEALED_PROPHECY_PATH_OF_THE_GOD, -1);
                qs.set(ctx, "talk", "1");
            }
            "30297-05.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 3, true);
                ctx.give_items(player, PROPHECY_PATH_OF_THE_GOD, 1);
                ctx.give_items(player, EMPTY_SOUND_CRYSTAL, 1);
            }
            "30612-03.html" => {
                ctx.take_items(player, PROPHECY_PATH_OF_THE_GOD, -1);
                qs.set(ctx, "talk", "1");
            }
            "30612-09.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 4, true);
                ctx.give_items(player, ANGEL_MEDICINE, 1);
            }
            "32368-04.html" => {
                if ctx.state::<Fields>().is_angel_spawned {
                    return Some("32368-03.html".to_owned());
                }

                let npc = npc?;
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.add_spawn(ANGEL, x + 100, y + 100, z, 0, false, 120_000, 0);
                }
                ctx.start_quest_timer("despawn", 120_000, None, Some(player), false);
                ctx.state::<Fields>().is_angel_spawned = true;
            }
            "32369-03.html" => {
                ctx.take_items(player, ANGEL_MEDICINE, -1);
                qs.set(ctx, "talk", "1");
            }
            "32369-06.html" => qs.set(ctx, "talk", "2"),
            "32369-11.html" => {
                qs.unset(ctx, "talk");
                ctx.take_items(player, EMPTY_SOUND_CRYSTAL, -1);
                ctx.give_items(player, ANGELS_MESSAGE, 1);
                qs.set_cond(ctx, 5, true);
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
                ctx.state::<Fields>().is_angel_spawned = false;
            }
            "despawn" => {
                if ctx.state::<Fields>().is_angel_spawned {
                    ctx.state::<Fields>().is_angel_spawned = false;
                }
                htmltext = None;
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            NATOOLS => match qs.state(ctx) {
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30894-01.html".to_owned(),
                    _ => htmltext = "30894-04.html".to_owned(),
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            TOBIAS => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30297-01.html".to_owned(),
                        2 => {
                            htmltext = if qs.is_set(ctx, "talk") {
                                "30297-04.html"
                            } else {
                                "30297-02.html"
                            }
                            .to_owned();
                        }
                        3 | 4 => htmltext = "30297-06.html".to_owned(),
                        5 => {
                            ctx.give_adena(player, 89_046, true);
                            if ctx.player_level(player) <= MAX_REWARD_LEVEL {
                                ctx.add_exp_and_sp(player, 223_036, 13_901);
                            }
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30297-07.html".to_owned();
                        }
                        _ => {}
                    }
                }
            }
            CASIAN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 | 2 => htmltext = "30612-01.html".to_owned(),
                        3 => {
                            htmltext = if qs.is_set(ctx, "talk") {
                                "30612-04.html"
                            } else {
                                "30612-02.html"
                            }
                            .to_owned();
                        }
                        _ => htmltext = "30612-10.html".to_owned(),
                    }
                }
            }
            ROCK => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=3 => htmltext = "32368-01.html".to_owned(),
                        4 => htmltext = "32368-02.html".to_owned(),
                        5 => htmltext = "32368-05.html".to_owned(),
                        _ => {}
                    }
                }
            }
            ANGEL => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=3 => htmltext = "32369-01.html".to_owned(),
                        4 => {
                            if qs.get_int(ctx, "talk") == 1 {
                                htmltext = "32369-04.html".to_owned();
                            } else if qs.get_int(ctx, "talk") == 2 {
                                htmltext = "32369-07.html".to_owned();
                            } else {
                                htmltext = "32369-02.html".to_owned();
                            }
                        }
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00143_FallenAngelRequestOfDusk";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30297" type="Folk" level="1" name="Tobias"/>
        <npc id="30612" type="Folk" level="1" name="Casian"/>
        <npc id="30894" type="Folk" level="1" name="Natools"/>
        <npc id="32368" type="Folk" level="1" name="Rock"/>
        <npc id="32369" type="Folk" level="1" name="Angel"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10354" type="EtcItem" name="Sealed Prophecy"><set name="is_questitem" val="true"/></item>
        <item id="10355" type="EtcItem" name="Prophecy"><set name="is_questitem" val="true"/></item>
        <item id="10356" type="EtcItem" name="Empty Sound Crystal"><set name="is_questitem" val="true"/></item>
        <item id="10357" type="EtcItem" name="Angel Medicine"><set name="is_questitem" val="true"/></item>
        <item id="10358" type="EtcItem" name="Angel's Message"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        tobias: Oid,
        casian: Oid,
        natools: Oid,
        rock: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00143FallenAngelRequestOfDusk);
        let pages = Pages::of(&Q00143FallenAngelRequestOfDusk)
            .page("30297-02.html", &["30297-03.html"])
            .page("30297-03.html", &["30297-04.html"])
            .page("30297-04.html", &["30297-05.html"])
            .page("30612-02.html", &["30612-03.html"])
            .page("30612-03.html", &["30612-05.html"])
            .page("30612-04.html", &["30612-06.html"])
            .page("30612-05.html", &["30612-06.html"])
            .page("30612-06.html", &["30612-07.html"])
            .page("30612-07.html", &["30612-08.html"])
            .page("30612-08.html", &["30612-09.html"])
            .page("30894-01.html", &["30894-02.html"])
            .page("30894-02.html", &["30894-03.html"])
            .page("32368-02.html", &["32368-04.html"])
            .page("32369-02.html", &["32369-03.html"])
            .page("32369-03.html", &["32369-04.html"])
            .page("32369-04.html", &["32369-05.html"])
            .page("32369-05.html", &["32369-06.html"])
            .page("32369-06.html", &["32369-07.html"])
            .page("32369-07.html", &["32369-08.html"])
            .page("32369-08.html", &["32369-09.html"])
            .page("32369-09.html", &["32369-10.html"])
            .page("32369-10.html", &["32369-11.html"])
            .plain(&[
                "30297-01.html",
                "30297-05.html",
                "30297-06.html",
                "30297-07.html",
                "30612-01.html",
                "30612-09.html",
                "30612-10.html",
                "30894-03.html",
                "30894-04.html",
                "32368-01.html",
                "32368-03.html",
                "32368-04.html",
                "32368-05.html",
                "32369-01.html",
                "32369-11.html",
            ])
            .npcs(&[NATOOLS, TOBIAS, CASIAN, ROCK, ANGEL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            tobias: t.spawn(TOBIAS, 20),
            casian: t.spawn(CASIAN, 30),
            natools: t.spawn(NATOOLS, 40),
            rock: t.spawn(ROCK, 50),
        };
        (t, npcs)
    }

    /// Runs a chain of page links as the player clicks them.
    fn click(t: &mut ScriptTest, events: &[&str]) {
        for e in events {
            assert!(t.event(1, NAME, e).contains(e), "{e}");
        }
    }

    #[test]
    fn the_whole_chain_the_angel_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        // Q00998 makes the state (`startQuest(name, player)`) and notifies this event.
        t.world.run_as_script(NAME, |ctx| {
            let _ = ctx.get_quest_state(Oid(1), true);
            let _ = Q00143FallenAngelRequestOfDusk.on_event(ctx, "30894-01.html", None, Some(Oid(1)));
        });
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.natools).contains("30894-01.html"));
        click(&mut t, &["30894-02.html", "30894-03.html"]);
        assert_eq!(t.count(1, SEALED_PROPHECY_PATH_OF_THE_GOD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.natools).contains("30894-04.html"));

        // Tobias: the seal is taken at "30297-03.html" (`talk` = 1), cond 3 at "30297-05.html".
        assert!(t.quest_page(1, npcs.tobias).contains("30297-02.html"));
        click(&mut t, &["30297-03.html"]);
        assert_eq!(t.count(1, SEALED_PROPHECY_PATH_OF_THE_GOD), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        assert!(t.quest_page(1, npcs.tobias).contains("30297-04.html"));
        click(&mut t, &["30297-05.html"]);
        assert_eq!(t.count(1, PROPHECY_PATH_OF_THE_GOD), 1);
        assert_eq!(t.count(1, EMPTY_SOUND_CRYSTAL), 1);
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "3")]
        );
        assert!(t.quest_page(1, npcs.tobias).contains("30297-06.html"));

        // Casian takes the prophecy and gives the medicine.
        assert!(t.quest_page(1, npcs.casian).contains("30612-02.html"));
        click(&mut t, &["30612-03.html"]);
        assert_eq!(t.count(1, PROPHECY_PATH_OF_THE_GOD), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        assert!(t.quest_page(1, npcs.casian).contains("30612-04.html"));
        click(
            &mut t,
            &["30612-06.html", "30612-07.html", "30612-08.html", "30612-09.html"],
        );
        assert_eq!(t.count(1, ANGEL_MEDICINE), 1);
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "4")]
        );

        // The rock spawns the angel once; a second try says so.
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        click(&mut t, &["32368-04.html"]);
        let angels = t.npcs_of(ANGEL);
        assert_eq!(angels.len(), 1);
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        assert!(t.event(1, NAME, "32368-04.html").contains("32368-03.html"));

        // The angel: medicine at "32369-03.html", `talk` 2 at "32369-06.html", cond 5 at the end.
        assert!(t.quest_page(1, angels[0]).contains("32369-02.html"));
        click(&mut t, &["32369-03.html"]);
        assert_eq!(t.count(1, ANGEL_MEDICINE), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        click(&mut t, &["32369-04.html", "32369-05.html", "32369-06.html"]);
        assert_eq!(t.take_writes(), [rows::update(NAME, "talk", "2")]);
        assert!(t.quest_page(1, angels[0]).contains("32369-07.html"));
        click(
            &mut t,
            &["32369-08.html", "32369-09.html", "32369-10.html", "32369-11.html"],
        );
        assert_eq!(t.count(1, ANGELS_MESSAGE), 1);
        assert_eq!(t.count(1, EMPTY_SOUND_CRYSTAL), 0);
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "5")]
        );
        assert!(!t.in_world(angels[0]));
        assert!(t.quest_page(1, npcs.rock).contains("32368-05.html"));

        // Tobias pays.
        assert!(t.quest_page(1, npcs.tobias).contains("30297-07.html"));
        assert_eq!(t.count(1, ADENA), 89_046);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn the_despawn_timer_lets_the_rock_spawn_again() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let _ = ctx.get_quest_state(Oid(1), true);
            let _ = Q00143FallenAngelRequestOfDusk.on_event(ctx, "30894-01.html", None, Some(Oid(1)));
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, 4, false);
            }
        });
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        click(&mut t, &["32368-04.html"]);
        assert_eq!(t.npcs_of(ANGEL).len(), 1);
        t.wait_ms(121_000);
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        click(&mut t, &["32368-04.html"]);
        assert_eq!(t.npcs_of(ANGEL).len(), 1);
    }
}
