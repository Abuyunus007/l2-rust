//! Quest 10294: Seven Signs, To the Monastery of Silence (Elcadia, Eris, the reading desks of the Monastery of Silence).
// Java: dist/game/data/scripts/quests/Q10294_SevenSignsToTheMonasteryOfSilence/Q10294_SevenSignsToTheMonasteryOfSilence.java
//
// Pattern: a delivery chain by condition, with the four right books of the reading desks
// (`good1`..`good4`, one per desk and set by the desk's own page). Each right book shows an
// effect on its desk (`setDisplayEffect(1)`), a spawn or a buff, and the fourth one
// plays the movie of the burial ground. The monsters come 22 s after the first book (a quest
// timer). Eris gives the reward at cond 3 (one time).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const ELCADIA: i32 = 32784;
const ELCADIA_INSTANCE: i32 = 32787;
const ERIS_EVIL_THOUGHTS: i32 = 32792;
const SOLINAS_EVIL_THOUGHTS: i32 = 32793;
const JUDE_VAN_ETINA: i32 = 32797;
const RELIC_GUARDIAN: i32 = 32803;
const RELIC_WATCHER1: i32 = 32804;
const RELIC_WATCHER2: i32 = 32805;
const RELIC_WATCHER3: i32 = 32806;
const RELIC_WATCHER4: i32 = 32807;
const ODD_GLOBE: i32 = 32815;
const READING_DESK1: i32 = 32821;
const READING_DESK2: i32 = 32822;
const READING_DESK3: i32 = 32823;
const READING_DESK4: i32 = 32824;
const READING_DESK5: i32 = 32825;
const READING_DESK6: i32 = 32826;
const READING_DESK7: i32 = 32827;
const READING_DESK8: i32 = 32828;
const READING_DESK9: i32 = 32829;
const READING_DESK10: i32 = 32830;
const READING_DESK11: i32 = 32831;
const READING_DESK12: i32 = 32832;
const READING_DESK13: i32 = 32833;
const READING_DESK14: i32 = 32834;
const READING_DESK15: i32 = 32835;
const READING_DESK16: i32 = 32836;
const JUDE_EVIL_THOUGHTS: i32 = 32888;

// Monsters
const SOLINA_LAY_BROTHER: i32 = 22125;
const GUIDE_SOLINA: i32 = 27415;

// Misc
const MIN_LEVEL: i32 = 81;

/// Java `Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom.class.getSimpleName()`.
const FORBIDDEN_BOOK: &str = "Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom";

// Buffs (Java `SkillHolder(id, level)`)
const VAMPIRIC_RAGE: (i32, i32) = (6727, 1);
const RESIST_HOLY: (i32, i32) = (6729, 1);
const MAGE_BUFFS: [(i32, i32); 4] = [
    (6714, 1), // Wind Walk of Elcadia
    (6721, 1), // Empower of Elcadia
    (6722, 1), // Acumen of Elcadia
    (6717, 1), // Berserker Spirit of Elcadia
];
const WARRIOR_BUFFS: [(i32, i32); 4] = [
    (6714, 1), // Wind Walk of Elcadia
    (6715, 1), // Haste of Elcadia
    (6716, 1), // Might of Elcadia
    (6717, 1), // Berserker Spirit of Elcadia
];

/// Java `Movie.SSQ2_HOLY_BURIAL_GROUND_CLOSING`.
const SSQ2_HOLY_BURIAL_GROUND_CLOSING: i32 = 25;

/// Java `hasCheckedAllRightBooks(qs)`.
fn has_checked_all_right_books(ctx: &ScriptCtx<'_>, qs: l2_world::Qs) -> bool {
    qs.get_int(ctx, "good1") == 1
        && qs.get_int(ctx, "good2") == 1
        && qs.get_int(ctx, "good3") == 1
        && qs.get_int(ctx, "good4") == 1
}

/// Java `Q10294_SevenSignsToTheMonasteryOfSilence`.
pub struct Q10294SevenSignsToTheMonasteryOfSilence;

impl Script for Q10294SevenSignsToTheMonasteryOfSilence {
    fn name(&self) -> &'static str {
        "Q10294_SevenSignsToTheMonasteryOfSilence"
    }

    fn path(&self) -> &'static str {
        "quests/Q10294_SevenSignsToTheMonasteryOfSilence"
    }

    fn id(&self) -> i32 {
        10294
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ELCADIA_INSTANCE]);
        s.add_start_npc(&[ELCADIA, ODD_GLOBE, ELCADIA_INSTANCE, RELIC_GUARDIAN]);
        s.add_talk_id(&[
            ELCADIA,
            ELCADIA_INSTANCE,
            ERIS_EVIL_THOUGHTS,
            RELIC_GUARDIAN,
            ODD_GLOBE,
            READING_DESK1,
            READING_DESK2,
            READING_DESK3,
            READING_DESK4,
            READING_DESK5,
            READING_DESK6,
            READING_DESK7,
            READING_DESK8,
            READING_DESK9,
            READING_DESK10,
            READING_DESK11,
            READING_DESK12,
            READING_DESK13,
            READING_DESK14,
            READING_DESK15,
            READING_DESK16,
            JUDE_VAN_ETINA,
            SOLINAS_EVIL_THOUGHTS,
            RELIC_WATCHER1,
            RELIC_WATCHER2,
            RELIC_WATCHER3,
            RELIC_WATCHER4,
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

        let mut htmltext = None;
        match event {
            "32784-03.htm" | "32784-04.htm" => htmltext = Some(event.to_owned()),
            "32784-05.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32792-02.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32792-03.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32792-04.html" | "32792-05.html" | "32792-06.html" | "32803-02.html" | "32803-03.html"
            | "32822-02.html" | "32804-02.html" | "32804-04.html" | "32804-06.html" | "32804-07.html"
            | "32804-08.html" | "32804-09.html" | "32804-10.html" | "32805-02.html" | "32805-04.html"
            | "32805-06.html" | "32805-07.html" | "32805-08.html" | "32805-09.html" | "32805-10.html"
            | "32806-02.html" | "32806-04.html" | "32806-06.html" | "32806-07.html" | "32806-08.html"
            | "32806-09.html" | "32806-10.html" | "32807-02.html" | "32807-04.html" | "32807-06.html"
            | "32807-07.html" | "32807-08.html" | "32807-09.html" | "32807-10.html" => {
                if qs.is_cond(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32792-08.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.unset(ctx, "good1");
                    qs.unset(ctx, "good2");
                    qs.unset(ctx, "good3");
                    qs.unset(ctx, "good4");
                    ctx.add_exp_and_sp(player, 25_000_000, 2_500_000);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "BUFF" => {
                if let Some(npc) = npc {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                    let buffs = if ctx.is_mage_class(player) {
                        MAGE_BUFFS
                    } else {
                        WARRIOR_BUFFS
                    };
                    for (id, level) in buffs {
                        ctx.world_mut()
                            .do_simultaneous_cast(npc.object_id, SkillKey::new(id, level));
                    }
                }
            }
            "RIGHT_BOOK1" => {
                if let Some(npc) = npc {
                    qs.set(ctx, "good1", "1");
                    ctx.world_mut().set_npc_display_effect(npc.object_id, 1);
                    ctx.start_quest_timer("SPAWN_MOBS", 22_000, Some(npc), Some(player), false);
                    htmltext = Some("32821-02.html".to_owned());
                    if has_checked_all_right_books(ctx, qs) {
                        ctx.play_movie(player, SSQ2_HOLY_BURIAL_GROUND_CLOSING);
                    }
                }
            }
            "RIGHT_BOOK2" => {
                if let Some(npc) = npc {
                    qs.set(ctx, "good2", "1");
                    ctx.world_mut().set_npc_display_effect(npc.object_id, 1);
                    ctx.cast_skill(npc, player, SkillKey::new(VAMPIRIC_RAGE.0, VAMPIRIC_RAGE.1));
                    htmltext = Some("32821-02.html".to_owned());
                    if has_checked_all_right_books(ctx, qs) {
                        ctx.play_movie(player, SSQ2_HOLY_BURIAL_GROUND_CLOSING);
                    }
                }
            }
            "RIGHT_BOOK3" => {
                if let Some(npc) = npc {
                    qs.set(ctx, "good3", "1");
                    ctx.world_mut().set_npc_display_effect(npc.object_id, 1);
                    let instance = ctx.world().instance_id_of(player);
                    if let Some(jude) =
                        ctx.add_spawn(JUDE_VAN_ETINA, 85_783, -253_471, -8_320, 65, false, 0, instance)
                    {
                        ctx.cast_skill(jude, player, SkillKey::new(RESIST_HOLY.0, RESIST_HOLY.1));
                    }
                    htmltext = Some("32821-02.html".to_owned());
                    if has_checked_all_right_books(ctx, qs) {
                        ctx.play_movie(player, SSQ2_HOLY_BURIAL_GROUND_CLOSING);
                    }
                }
            }
            "RIGHT_BOOK4" => {
                if let Some(npc) = npc {
                    qs.set(ctx, "good4", "1");
                    ctx.world_mut().set_npc_display_effect(npc.object_id, 1);
                    let instance = ctx.world().instance_id_of(player);
                    if let Some(solina) = ctx.add_spawn(
                        SOLINAS_EVIL_THOUGHTS,
                        85_793,
                        -247_581,
                        -8_320,
                        0,
                        false,
                        0,
                        instance,
                    ) {
                        ctx.cast_skill(solina, player, SkillKey::new(RESIST_HOLY.0, RESIST_HOLY.1));
                    }
                    htmltext = Some("32821-02.html".to_owned());
                    if has_checked_all_right_books(ctx, qs) {
                        ctx.play_movie(player, SSQ2_HOLY_BURIAL_GROUND_CLOSING);
                    }
                }
            }
            "DONE1" => {
                htmltext = Some(
                    if qs.get_int(ctx, "good1") == 1 {
                        "32804-05.html"
                    } else {
                        "32804-03.html"
                    }
                    .to_owned(),
                );
            }
            "DONE2" => {
                htmltext = Some(
                    if qs.get_int(ctx, "good2") == 1 {
                        "32805-05.html"
                    } else {
                        "32805-03.html"
                    }
                    .to_owned(),
                );
            }
            "DONE3" => {
                htmltext = Some(
                    if qs.get_int(ctx, "good3") == 1 {
                        "32806-05.html"
                    } else {
                        "32806-03.html"
                    }
                    .to_owned(),
                );
            }
            "DONE4" => {
                htmltext = Some(
                    if qs.get_int(ctx, "good4") == 1 {
                        "32807-05.html"
                    } else {
                        "32807-03.html"
                    }
                    .to_owned(),
                );
            }
            "SPAWN_MOBS" => {
                // Java `player.getInstanceId()`: the monsters are made in the player's instance.
                let instance = ctx.world().instance_id_of(player);
                let (x, y, z, heading) = (88_655, -250_591, -8_320, 144);
                ctx.add_spawn(JUDE_EVIL_THOUGHTS, x, y, z, heading, false, 0, instance);
                ctx.add_spawn(GUIDE_SOLINA, x, y, z, heading, false, 0, instance);
                ctx.add_spawn(SOLINA_LAY_BROTHER, x, y, z, heading, false, 0, instance);
                ctx.add_spawn(SOLINA_LAY_BROTHER, x, y, z, heading, false, 0, instance);
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32787.html".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ELCADIA => {
                if qs.is_completed(ctx) {
                    htmltext = "32784-02.html".to_owned();
                } else if qs.is_created(ctx) {
                    let forbidden_done = ctx
                        .player_quest_state(player, FORBIDDEN_BOOK)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && forbidden_done {
                        "32784-01.htm"
                    } else {
                        "32784-07.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) && qs.is_cond(ctx, 1) {
                    htmltext = "32784-06.html".to_owned();
                }
            }
            ERIS_EVIL_THOUGHTS => match qs.get_cond(ctx) {
                1 => htmltext = "32792-01.html".to_owned(),
                2 => htmltext = "32792-04.html".to_owned(),
                3 => {
                    htmltext = if ctx.is_sub_class_active(player) {
                        "32792-09.html"
                    } else {
                        "32792-07.html"
                    }
                    .to_owned();
                }
                _ => {}
            },
            RELIC_GUARDIAN => {
                if qs.is_cond(ctx, 2) {
                    if has_checked_all_right_books(ctx, qs) {
                        qs.set_cond(ctx, 3, true);
                        htmltext = "32803-04.html".to_owned();
                    } else {
                        htmltext = "32803-01.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 3) {
                    htmltext = "32803-05.html".to_owned();
                }
            }
            ODD_GLOBE => {
                if qs.get_cond(ctx) < 3 {
                    htmltext = "32815-01.html".to_owned();
                }
            }
            ELCADIA_INSTANCE => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "32787-01.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "32787-02.html".to_owned();
                }
            }
            READING_DESK2 | READING_DESK3 | READING_DESK4 | READING_DESK6 | READING_DESK7 | READING_DESK8
            | READING_DESK10 | READING_DESK11 | READING_DESK12 | READING_DESK14 | READING_DESK15
            | READING_DESK16 => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "32822-01.html".to_owned();
                }
            }
            READING_DESK1 => {
                htmltext = if qs.get_int(ctx, "good1") == 1 {
                    "32821-03.html"
                } else {
                    "32821-01.html"
                }
                .to_owned();
            }
            READING_DESK5 => {
                htmltext = if qs.get_int(ctx, "good2") == 1 {
                    "32821-03.html"
                } else {
                    "32825-01.html"
                }
                .to_owned();
            }
            READING_DESK9 => {
                htmltext = if qs.get_int(ctx, "good3") == 1 {
                    "32821-03.html"
                } else {
                    "32829-01.html"
                }
                .to_owned();
            }
            READING_DESK13 => {
                htmltext = if qs.get_int(ctx, "good4") == 1 {
                    "32821-03.html"
                } else {
                    "32833-01.html"
                }
                .to_owned();
            }
            SOLINAS_EVIL_THOUGHTS
            | JUDE_VAN_ETINA
            | RELIC_WATCHER1
            | RELIC_WATCHER2
            | RELIC_WATCHER3
            | RELIC_WATCHER4 => {
                if qs.is_cond(ctx, 2) {
                    htmltext = format!("{}-01.html", npc.id);
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
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, player_data, rows};

    const NAME: &str = "Q10294_SevenSignsToTheMonasteryOfSilence";

    const NPCS: &str = r#"
        <npc id="32784" type="Folk" level="80" name="Elcadia"/>
        <npc id="32787" type="Folk" level="80" name="Elcadia"/>
        <npc id="32792" type="Folk" level="80" name="Eris Evil Thoughts"/>
        <npc id="32803" type="Folk" level="80" name="Relic Guardian"/>
        <npc id="32821" type="Folk" level="80" name="Reading Desk"/>
        <npc id="32825" type="Folk" level="80" name="Reading Desk"/>
        <npc id="32829" type="Folk" level="80" name="Reading Desk"/>
        <npc id="32833" type="Folk" level="80" name="Reading Desk"/>
        <npc id="32888" type="Monster" level="80" name="Jude Evil Thoughts"/>
        <npc id="27415" type="Monster" level="80" name="Guide Solina"/>
        <npc id="22125" type="Monster" level="80" name="Solina Lay Brother"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10293 (its completion is read through `player_quest_state` only).
    struct ForbiddenBookStandIn;

    impl Script for ForbiddenBookStandIn {
        fn name(&self) -> &'static str {
            FORBIDDEN_BOOK
        }

        fn path(&self) -> &'static str {
            "quests/Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom"
        }

        fn id(&self) -> i32 {
            10293
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        elcadia: Oid,
        eris: Oid,
        guardian: Oid,
        desk1: Oid,
        desk5: Oid,
        desk9: Oid,
        desk13: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10294SevenSignsToTheMonasteryOfSilence);
        registry.add(ForbiddenBookStandIn);
        let pages = Pages::of(&Q10294SevenSignsToTheMonasteryOfSilence)
            .page("32784-01.htm", &["32784-03.htm"])
            .page("32784-03.htm", &["32784-05.html", "32784-04.htm"])
            .plain(&[
                "32784-04.htm",
                "32784-05.html",
                "32784-06.html",
                "32784-07.htm",
                "32784-02.html",
            ])
            .page("32792-01.html", &["32792-02.html"])
            .page("32792-02.html", &["32792-03.html"])
            .plain(&["32792-03.html", "32792-04.html", "32792-09.html"])
            .page("32792-07.html", &["32792-08.html"])
            .plain(&["32792-08.html"])
            .page("32803-01.html", &["32803-03.html", "32803-02.html"])
            .plain(&["32803-02.html", "32803-03.html", "32803-04.html", "32803-05.html"])
            .page("32821-01.html", &["RIGHT_BOOK1"])
            .plain(&["32821-02.html", "32821-03.html"])
            .page("32825-01.html", &["RIGHT_BOOK2"])
            .page("32829-01.html", &["RIGHT_BOOK3"])
            .page("32833-01.html", &["RIGHT_BOOK4"])
            .npcs(&[
                ELCADIA,
                ERIS_EVIL_THOUGHTS,
                RELIC_GUARDIAN,
                READING_DESK1,
                READING_DESK5,
                READING_DESK9,
                READING_DESK13,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            elcadia: t.spawn(ELCADIA, 20),
            eris: t.spawn(ERIS_EVIL_THOUGHTS, 30),
            guardian: t.spawn(RELIC_GUARDIAN, 40),
            desk1: t.spawn(READING_DESK1, 50),
            desk5: t.spawn(READING_DESK5, 60),
            desk9: t.spawn(READING_DESK9, 70),
            desk13: t.spawn(READING_DESK13, 80),
        };
        (t, npcs)
    }

    /// A character that has finished Forbidden Book, with `level`.
    fn forbidden_book_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(FORBIDDEN_BOOK.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    #[test]
    fn elcadia_starts_the_quest_at_81() {
        let (mut t, npcs) = setup();
        forbidden_book_done(&mut t, 1, 80);
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-07.htm"));
        forbidden_book_done(&mut t, 2, 81);
        assert!(t.quest_page(2, npcs.elcadia).contains("32784-01.htm"));
        assert!(t.event(2, NAME, "32784-03.htm").contains("32784-03.htm"));
        assert!(t.event(2, NAME, "32784-05.html").contains("32784-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started")),
            ]
        );
    }

    #[test]
    fn the_books_the_monsters_and_the_movie() {
        let (mut t, npcs) = setup();
        forbidden_book_done(&mut t, 1, 81);
        t.quest_page(1, npcs.elcadia);
        t.event(1, NAME, "32784-03.htm");
        t.event(1, NAME, "32784-05.html");
        t.take_writes();

        // Eris: cond 1 to 2.
        assert!(t.quest_page(1, npcs.eris).contains("32792-01.html"));
        assert!(t.event(1, NAME, "32792-02.html").contains("32792-02.html"));
        assert!(t.event(1, NAME, "32792-03.html").contains("32792-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The guardian is not ready without all the books.
        assert!(t.quest_page(1, npcs.guardian).contains("32803-01.html"));

        // Book 1: the desk's page, the monsters after 22 s.
        assert!(t.quest_page(1, npcs.desk1).contains("32821-01.html"));
        t.bypass(1, &format!("Script {NAME} RIGHT_BOOK1"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32821-02.html"));
        // The desk's display effect (`setDisplayEffect(1)`, `ExChangeNpcState`).
        assert!(
            names_to(&out, 1).iter().any(|n| n == "EX_CHANGE_NPC_STATE"),
            "{:?}",
            names_to(&out, 1)
        );
        assert_eq!(t.world.npc(npcs.desk1).map(|n| n.display_effect), Some(1));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "good1", "1")]);
        assert!(t.npcs_of(GUIDE_SOLINA).is_empty());
        t.wait_ms(22_000);
        assert_eq!(t.npcs_of(GUIDE_SOLINA).len(), 1);
        assert_eq!(t.npcs_of(SOLINA_LAY_BROTHER).len(), 2);
        assert_eq!(t.npcs_of(JUDE_EVIL_THOUGHTS).len(), 1);

        // Books 2 to 4; the fourth one plays the movie.
        assert!(t.quest_page(1, npcs.desk5).contains("32825-01.html"));
        assert!(t.event(1, NAME, "RIGHT_BOOK2").contains("32821-02.html"));
        assert!(t.quest_page(1, npcs.desk9).contains("32829-01.html"));
        assert!(t.event(1, NAME, "RIGHT_BOOK3").contains("32821-02.html"));
        t.take();
        assert!(t.quest_page(1, npcs.desk13).contains("32833-01.html"));
        // `event` drains the output, so the bypass is sent here to read the movie packet.
        t.bypass(1, &format!("Script {NAME} RIGHT_BOOK4"));
        let out = t.take();
        assert!(names_to(&out, 1).iter().any(|n| n == "EX_START_SCENE_PLAYER"));
        assert!(
            pages_to(&out, 1)
                .last()
                .is_some_and(|p| p.contains("32821-02.html"))
        );

        // The guardian, with all four: cond 3.
        assert!(t.quest_page(1, npcs.guardian).contains("32803-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "good2", "1"),
                rows::insert(NAME, "good3", "1"),
                rows::insert(NAME, "good4", "1"),
                rows::update(NAME, "cond", "3"),
            ]
        );

        // Eris: the books are unset, the experience is paid and the quest ends.
        assert!(t.quest_page(1, npcs.eris).contains("32792-07.html"));
        assert!(t.event(1, NAME, "32792-08.html").contains("32792-08.html"));
        let writes = t.take_writes();
        for var in ["good1", "good2", "good3", "good4"] {
            assert!(
                writes.contains(&rows::of(1, rows::delete_var(NAME, var))),
                "{var}"
            );
        }
        assert!(writes.contains(&rows::of(1, rows::delete(NAME, false))));
        assert!(writes.contains(&rows::of(1, rows::update(NAME, "<state>", "Completed"))));
    }
}
