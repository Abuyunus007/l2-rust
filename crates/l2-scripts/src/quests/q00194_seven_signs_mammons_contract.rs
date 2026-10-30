//! Quest 194: Seven Signs, Mammon's Contract (Sir Gustav, Colin and three transformation NPCs).
// Java: dist/game/data/scripts/quests/Q00194_SevenSignsMammonsContract/Q00194_SevenSignsMammonsContract.java
//
// Pattern: a long chain of cond steps; Colin casts a transformation skill on the player (frog,
// kid, native) and the pages depend on `getTransformationId()` (111, 112, 124).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SIR_GUSTAV_ATHEBALDT: i32 = 30760;
const CLAUDIA_ATHEBALDT: i32 = 31001;
const COLIN: i32 = 32571;
const FROG: i32 = 32572;
const TESS: i32 = 32573;
const KUTA: i32 = 32574;

// Items
const ATHEBALDTS_INTRODUCTION: i32 = 13818;
const NATIVES_GLOVE: i32 = 13819;
const FROG_KINGS_BEAD: i32 = 13820;
const GRANDA_TESS_CANDY_POUCH: i32 = 13821;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Movie.SSQ_CONTRACT_OF_MAMMON`.
const SSQ_CONTRACT_OF_MAMMON: i32 = 10;
/// The ids `getTransformationId()` has under the three transformations.
const FROG_TRANSFORM: i32 = 111;
const KID_TRANSFORM: i32 = 112;
const NATIVE_TRANSFORM: i32 = 124;
/// Java `Q00193_SevenSignsDyingMessage.class.getSimpleName()`: the quest this one follows.
const DYING_MESSAGE: &str = "Q00193_SevenSignsDyingMessage";

// Skills
const TRANSFORMATION_FROG: SkillKey = SkillKey { id: 6201, level: 1 };
const TRANSFORMATION_KID: SkillKey = SkillKey { id: 6202, level: 1 };
const TRANSFORMATION_NATIVE: SkillKey = SkillKey { id: 6203, level: 1 };

/// Java `Q00194_SevenSignsMammonsContract`.
pub struct Q00194SevenSignsMammonsContract;

/// Java `player.getTransformationId()`.
fn transformation_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world().transformation_id(player)
}

/// Java `npc.setTarget(player); npc.doCast(skill)`.
fn transform(ctx: &mut ScriptCtx<'_>, npc: Option<ScriptNpc>, player: ObjectId, skill: SkillKey) {
    // Java dereferences `npc` here: the bypass of an NPC page always has one.
    if let Some(npc) = npc {
        ctx.cast_skill(npc, player, skill);
    }
}

impl Script for Q00194SevenSignsMammonsContract {
    fn name(&self) -> &'static str {
        "Q00194_SevenSignsMammonsContract"
    }

    fn path(&self) -> &'static str {
        "quests/Q00194_SevenSignsMammonsContract"
    }

    fn id(&self) -> i32 {
        194
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIR_GUSTAV_ATHEBALDT]);
        s.add_talk_id(&[SIR_GUSTAV_ATHEBALDT, COLIN, FROG, TESS, KUTA, CLAUDIA_ATHEBALDT]);
        s.register_quest_items(&[
            ATHEBALDTS_INTRODUCTION,
            NATIVES_GLOVE,
            FROG_KINGS_BEAD,
            GRANDA_TESS_CANDY_POUCH,
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
            "30760-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30760-03.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30760-04.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                }
            }
            "showmovie" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    ctx.play_movie(player, SSQ_CONTRACT_OF_MAMMON);
                    return Some(String::new());
                }
            }
            "30760-07.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, ATHEBALDTS_INTRODUCTION, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-03.html" | "32571-04.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, ATHEBALDTS_INTRODUCTION) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-05.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, ATHEBALDTS_INTRODUCTION) {
                    ctx.take_items(player, ATHEBALDTS_INTRODUCTION, -1);
                    transform(ctx, npc, player, TRANSFORMATION_FROG);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-07.html" => {
                if qs.is_cond(ctx, 4)
                    && transformation_id(ctx, player) != FROG_TRANSFORM
                    && !ctx.has_quest_items(player, FROG_KINGS_BEAD)
                {
                    transform(ctx, npc, player, TRANSFORMATION_FROG);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-09.html" => {
                if qs.is_cond(ctx, 4)
                    && transformation_id(ctx, player) == FROG_TRANSFORM
                    && !ctx.has_quest_items(player, FROG_KINGS_BEAD)
                {
                    ctx.world_mut().stop_all_effects(player);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-11.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, FROG_KINGS_BEAD) {
                    ctx.take_items(player, FROG_KINGS_BEAD, -1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                    if transformation_id(ctx, player) == FROG_TRANSFORM {
                        ctx.world_mut().stop_all_effects(player);
                    }
                }
            }
            "32571-13.html" => {
                if qs.is_cond(ctx, 6) {
                    transform(ctx, npc, player, TRANSFORMATION_KID);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-15.html" => {
                if qs.is_cond(ctx, 7)
                    && transformation_id(ctx, player) != KID_TRANSFORM
                    && !ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH)
                {
                    transform(ctx, npc, player, TRANSFORMATION_KID);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-17.html" => {
                if qs.is_cond(ctx, 7)
                    && transformation_id(ctx, player) == KID_TRANSFORM
                    && !ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH)
                {
                    ctx.world_mut().stop_all_effects(player);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-19.html" => {
                if qs.is_cond(ctx, 8) && ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH) {
                    ctx.take_items(player, GRANDA_TESS_CANDY_POUCH, -1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                    if transformation_id(ctx, player) == KID_TRANSFORM {
                        ctx.world_mut().stop_all_effects(player);
                    }
                }
            }
            "32571-21.html" => {
                if qs.is_cond(ctx, 9) {
                    transform(ctx, npc, player, TRANSFORMATION_NATIVE);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-23.html" => {
                if qs.is_cond(ctx, 10)
                    && transformation_id(ctx, player) != NATIVE_TRANSFORM
                    && !ctx.has_quest_items(player, NATIVES_GLOVE)
                {
                    transform(ctx, npc, player, TRANSFORMATION_NATIVE);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-25.html" => {
                if qs.is_cond(ctx, 10)
                    && transformation_id(ctx, player) == NATIVE_TRANSFORM
                    && !ctx.has_quest_items(player, NATIVES_GLOVE)
                {
                    ctx.world_mut().stop_all_effects(player);
                    htmltext = Some(event.to_owned());
                }
            }
            "32571-27.html" => {
                if qs.is_cond(ctx, 11) && ctx.has_quest_items(player, NATIVES_GLOVE) {
                    ctx.take_items(player, NATIVES_GLOVE, -1);
                    qs.set_cond(ctx, 12, true);
                    htmltext = Some(event.to_owned());
                    if transformation_id(ctx, player) == NATIVE_TRANSFORM {
                        ctx.world_mut().stop_all_effects(player);
                    }
                }
            }
            "32572-03.html" | "32572-04.html" => {
                if qs.is_cond(ctx, 4) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32572-05.html" => {
                if qs.is_cond(ctx, 4) {
                    ctx.give_items(player, FROG_KINGS_BEAD, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32573-03.html" => {
                if qs.is_cond(ctx, 7) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32573-04.html" => {
                if qs.is_cond(ctx, 7) {
                    ctx.give_items(player, GRANDA_TESS_CANDY_POUCH, 1);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32574-03.html" | "32574-04.html" => {
                if qs.is_cond(ctx, 10) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32574-05.html" => {
                if qs.is_cond(ctx, 10) {
                    ctx.give_items(player, NATIVES_GLOVE, 1);
                    qs.set_cond(ctx, 11, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31001-02.html" => {
                if qs.is_cond(ctx, 12) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31001-03.html" => {
                if qs.is_cond(ctx, 12) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let transformed = transformation_id(ctx, player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == SIR_GUSTAV_ATHEBALDT {
                    let dying_message_done = ctx
                        .player_quest_state(player, DYING_MESSAGE)
                        .is_some_and(|qs193| qs193.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && dying_message_done {
                        "30760-01.htm"
                    } else {
                        "30760-05.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                SIR_GUSTAV_ATHEBALDT => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30760-02.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30760-06.html".to_owned();
                    } else if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, ATHEBALDTS_INTRODUCTION) {
                        htmltext = "30760-08.html".to_owned();
                    }
                }
                COLIN => match qs.get_cond(ctx) {
                    1 | 2 => htmltext = "32571-01.html".to_owned(),
                    3 => {
                        if ctx.has_quest_items(player, ATHEBALDTS_INTRODUCTION) {
                            htmltext = "32571-02.html".to_owned();
                        }
                    }
                    4 => {
                        if !ctx.has_quest_items(player, FROG_KINGS_BEAD) {
                            htmltext = if transformed != FROG_TRANSFORM {
                                "32571-06.html"
                            } else {
                                "32571-08.html"
                            }
                            .to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_quest_items(player, FROG_KINGS_BEAD) {
                            htmltext = "32571-10.html".to_owned();
                        }
                    }
                    6 => htmltext = "32571-12.html".to_owned(),
                    7 => {
                        if !ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH) {
                            htmltext = if transformed != KID_TRANSFORM {
                                "32571-14.html"
                            } else {
                                "32571-16.html"
                            }
                            .to_owned();
                        }
                    }
                    8 => {
                        if ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH) {
                            htmltext = "32571-18.html".to_owned();
                        }
                    }
                    9 => htmltext = "32571-20.html".to_owned(),
                    10 => {
                        if !ctx.has_quest_items(player, NATIVES_GLOVE) {
                            htmltext = if transformed != NATIVE_TRANSFORM {
                                "32571-22.html"
                            } else {
                                "32571-24.html"
                            }
                            .to_owned();
                        }
                    }
                    11 => {
                        if ctx.has_quest_items(player, NATIVES_GLOVE) {
                            htmltext = "32571-26.html".to_owned();
                        }
                    }
                    12 => htmltext = "32571-28.html".to_owned(),
                    _ => {}
                },
                FROG => match qs.get_cond(ctx) {
                    1..=3 => htmltext = "32572-01.html".to_owned(),
                    4 => {
                        htmltext = if transformed == FROG_TRANSFORM {
                            "32572-02.html"
                        } else {
                            "32572-06.html"
                        }
                        .to_owned();
                    }
                    5 => {
                        if ctx.has_quest_items(player, FROG_KINGS_BEAD) && transformed == FROG_TRANSFORM {
                            htmltext = "32572-07.html".to_owned();
                        }
                    }
                    _ => {}
                },
                TESS => match qs.get_cond(ctx) {
                    1..=6 => htmltext = "32573-01.html".to_owned(),
                    7 => {
                        htmltext = if transformed == KID_TRANSFORM {
                            "32573-02.html"
                        } else {
                            "32573-05.html"
                        }
                        .to_owned();
                    }
                    8 => {
                        if ctx.has_quest_items(player, GRANDA_TESS_CANDY_POUCH)
                            && transformed == KID_TRANSFORM
                        {
                            htmltext = "32573-06.html".to_owned();
                        }
                    }
                    _ => {}
                },
                KUTA => match qs.get_cond(ctx) {
                    1..=9 => htmltext = "32574-01.html".to_owned(),
                    10 => {
                        htmltext = if transformed == NATIVE_TRANSFORM {
                            "32574-02.html"
                        } else {
                            "32574-06.html"
                        }
                        .to_owned();
                    }
                    11 => {
                        if ctx.has_quest_items(player, NATIVES_GLOVE) && transformed == NATIVE_TRANSFORM {
                            htmltext = "32574-07.html".to_owned();
                        }
                    }
                    _ => {}
                },
                CLAUDIA_ATHEBALDT => {
                    if qs.is_cond(ctx, 12) {
                        htmltext = "31001-01.html".to_owned();
                    }
                }
                _ => {}
            },
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

    const NAME: &str = "Q00194_SevenSignsMammonsContract";

    const NPCS: &str = r#"
        <npc id="30760" type="Folk" level="80" name="Sir Gustav Athebaldt"/>
        <npc id="31001" type="Folk" level="80" name="Claudia Athebaldt"/>
        <npc id="32571" type="Folk" level="80" name="Colin"/>
        <npc id="32572" type="Folk" level="80" name="Frog"/>
        <npc id="32573" type="Folk" level="80" name="Tess"/>
        <npc id="32574" type="Folk" level="80" name="Kuta"/>"#;

    const ITEMS: &str = r#"
        <item id="13818" type="EtcItem" name="Athebaldt's Introduction"><set name="is_questitem" val="true"/></item>
        <item id="13819" type="EtcItem" name="Native's Glove"><set name="is_questitem" val="true"/></item>
        <item id="13820" type="EtcItem" name="Frog King's Bead"><set name="is_questitem" val="true"/></item>
        <item id="13821" type="EtcItem" name="Granda Tess' Candy Pouch"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00193_SevenSignsDyingMessage` as far as this quest reads it: a script of that name.
    struct DyingMessage;

    impl Script for DyingMessage {
        fn name(&self) -> &'static str {
            DYING_MESSAGE
        }
        fn path(&self) -> &'static str {
            "quests/Q00193_SevenSignsDyingMessage"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        gustav: Oid,
        colin: Oid,
        claudia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(DyingMessage);
        registry.add(Q00194SevenSignsMammonsContract);
        let pages = Pages::of(&Q00194SevenSignsMammonsContract)
            .page("30760-01.htm", &["30760-02.html"])
            .page("30760-02.html", &["30760-03.html"])
            .page("30760-03.html", &["30760-04.html"])
            .page("30760-04.html", &["showmovie"])
            .page("30760-06.html", &["30760-07.html"])
            .page("32571-02.html", &["32571-03.html"])
            .page("32571-03.html", &["32571-04.html"])
            .page("32571-04.html", &["32571-05.html"])
            .page("32571-06.html", &["32571-07.html"])
            .page("32571-10.html", &["32571-11.html"])
            .page("32571-12.html", &["32571-13.html"])
            .page("32571-18.html", &["32571-19.html"])
            .page("32571-20.html", &["32571-21.html"])
            .page("32571-26.html", &["32571-27.html"])
            .page("31001-01.html", &["31001-02.html"])
            .page("31001-02.html", &["31001-03.html"])
            .plain(&[
                "30760-05.html",
                "30760-07.html",
                "30760-08.html",
                "32571-01.html",
                "32571-05.html",
                "32571-07.html",
                "32571-11.html",
                "32571-13.html",
                "32571-19.html",
                "32571-21.html",
                "32571-27.html",
                "32571-28.html",
                "31001-03.html",
                "level_check.html",
            ])
            .npcs(&[SIR_GUSTAV_ATHEBALDT, CLAUDIA_ATHEBALDT, COLIN, FROG, TESS, KUTA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            gustav: t.spawn(SIR_GUSTAV_ATHEBALDT, 20),
            colin: t.spawn(COLIN, 25),
            claudia: t.spawn(CLAUDIA_ATHEBALDT, 30),
        };
        (t, npcs)
    }

    /// Calls the event hook the way the NPC page of `npc` would: the transformation NPCs of the
    /// later steps only offer their pages to a transformed player, and the test world has no
    /// transformation skills to cast.
    fn call(t: &mut ScriptTest, event: &str, npc: Oid) -> Option<String> {
        let mut answer = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npc);
            answer = Q00194SevenSignsMammonsContract.on_event(ctx, event, npc, Some(Oid(1)));
        });
        answer
    }

    #[test]
    fn the_offer_needs_level_79_and_the_dying_message() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, npcs.gustav).contains("30760-05.html"));
        t.world.run_as_script(DYING_MESSAGE, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.gustav).contains("30760-01.htm"));
        t.enter(2, 78, vec![]);
        t.world.run_as_script(DYING_MESSAGE, |ctx| {
            let qs = ctx.get_quest_state(Oid(2), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        assert!(t.quest_page(2, npcs.gustav).contains("30760-05.html"));
    }

    #[test]
    fn the_whole_chain_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        t.world.run_as_script(DYING_MESSAGE, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.gustav).contains("30760-01.htm"));
        assert!(t.event(1, NAME, "30760-02.html").contains("30760-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.event(1, NAME, "30760-03.html").contains("30760-03.html"));
        assert!(t.event(1, NAME, "30760-04.html").contains("30760-04.html"));
        // The movie answers an empty page and moves to cond 2.
        t.event(1, NAME, "showmovie");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.gustav).contains("30760-06.html"));
        assert!(t.event(1, NAME, "30760-07.html").contains("30760-07.html"));
        assert_eq!(t.count(1, ATHEBALDTS_INTRODUCTION), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.gustav).contains("30760-08.html"));

        // Colin: three pages, then the frog transformation is cast and cond is 4.
        assert!(t.quest_page(1, npcs.colin).contains("32571-02.html"));
        for e in ["32571-03.html", "32571-04.html", "32571-05.html"] {
            assert!(t.event(1, NAME, e).contains(e), "{e}");
        }
        assert_eq!(t.count(1, ATHEBALDTS_INTRODUCTION), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        // Not transformed (the test world has no skill 6201): Colin offers to cast it again.
        assert!(t.quest_page(1, npcs.colin).contains("32571-06.html"));
        assert!(t.event(1, NAME, "32571-07.html").contains("32571-07.html"));

        // The frog gives the bead (cond 5), Colin takes it (cond 6) and casts the kid skill (7).
        assert!(call(&mut t, "32572-03.html", npcs.colin).is_some());
        assert!(call(&mut t, "32572-05.html", npcs.colin).is_some());
        assert_eq!(t.count(1, FROG_KINGS_BEAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-10.html"));
        assert!(t.event(1, NAME, "32571-11.html").contains("32571-11.html"));
        assert_eq!(t.count(1, FROG_KINGS_BEAD), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-12.html"));
        assert!(t.event(1, NAME, "32571-13.html").contains("32571-13.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Tess gives the pouch (cond 8), Colin takes it (cond 9) and casts the native skill (10).
        assert!(call(&mut t, "32573-03.html", npcs.colin).is_some());
        assert!(call(&mut t, "32573-04.html", npcs.colin).is_some());
        assert_eq!(t.count(1, GRANDA_TESS_CANDY_POUCH), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-18.html"));
        assert!(t.event(1, NAME, "32571-19.html").contains("32571-19.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-20.html"));
        assert!(t.event(1, NAME, "32571-21.html").contains("32571-21.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);

        // Kuta gives the glove (cond 11), Colin takes it (cond 12).
        assert!(call(&mut t, "32574-03.html", npcs.colin).is_some());
        assert!(call(&mut t, "32574-05.html", npcs.colin).is_some());
        assert_eq!(t.count(1, NATIVES_GLOVE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "11")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-26.html"));
        assert!(t.event(1, NAME, "32571-27.html").contains("32571-27.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "12")]);
        assert!(t.quest_page(1, npcs.colin).contains("32571-28.html"));

        // Claudia pays exp and sp.
        assert!(t.quest_page(1, npcs.claudia).contains("31001-01.html"));
        assert!(t.event(1, NAME, "31001-02.html").contains("31001-02.html"));
        assert!(t.event(1, NAME, "31001-03.html").contains("31001-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn a_level_below_79_gets_the_level_check_page() {
        let (mut t, _) = setup();
        t.enter(1, 78, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 12, false);
        });
        t.take_writes();
        assert_eq!(
            call(&mut t, "31001-03.html", Oid(0)).as_deref(),
            Some("level_check.html")
        );
        assert!(t.take_writes().is_empty());
    }
}
