//! Quest 195: Seven Signs, Secret Ritual of the Priests (Claudia Athebaldt, John, Raymond, Iason Heine).
// Java: dist/game/data/scripts/quests/Q00195_SevenSignsSecretRitualOfThePriests/Q00195_SevenSignsSecretRitualOfThePriests.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const RAYMOND: i32 = 30289;
const IASON_HEINE: i32 = 30969;
const CLAUDIA_ATHEBALDT: i32 = 31001;
const LIGHT_OF_DAWN: i32 = 32575;
const JOHN: i32 = 32576;
const PASSWORD_ENTRY_DEVICE: i32 = 32577;
const IDENTITY_CONFIRM_DEVICE: i32 = 32578;
const DARKNESS_OF_DAWN: i32 = 32579;
const SHELF: i32 = 32580;

// Items
const IDENTITY_CARD: i32 = 13822;
const SHUNAIMANS_CONTRACT: i32 = 13823;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Q00194_SevenSignsMammonsContract.class.getSimpleName()`: the quest this one follows.
const MAMMONS_CONTRACT: &str = "Q00194_SevenSignsMammonsContract";

// Skills
const TRANSFORMATION: (i32, i32) = (6204, 1);

/// Java `Q00195_SevenSignsSecretRitualOfThePriests`.
pub struct Q00195SevenSignsSecretRitualOfThePriests;

impl Script for Q00195SevenSignsSecretRitualOfThePriests {
    fn name(&self) -> &'static str {
        "Q00195_SevenSignsSecretRitualOfThePriests"
    }

    fn path(&self) -> &'static str {
        "quests/Q00195_SevenSignsSecretRitualOfThePriests"
    }

    fn id(&self) -> i32 {
        195
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[
            IDENTITY_CONFIRM_DEVICE,
            PASSWORD_ENTRY_DEVICE,
            DARKNESS_OF_DAWN,
            SHELF,
        ]);
        s.add_start_npc(&[CLAUDIA_ATHEBALDT]);
        s.add_talk_id(&[
            CLAUDIA_ATHEBALDT,
            JOHN,
            RAYMOND,
            IASON_HEINE,
            LIGHT_OF_DAWN,
            DARKNESS_OF_DAWN,
            IDENTITY_CONFIRM_DEVICE,
            PASSWORD_ENTRY_DEVICE,
            SHELF,
        ]);
        s.register_quest_items(&[IDENTITY_CARD, SHUNAIMANS_CONTRACT]);
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
            "31001-03.html" | "31001-04.html" | "31001-05.html" | "32580-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "31001-06.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32576-02.html" => {
                if qs.is_cond(ctx, 1) {
                    ctx.give_items(player, IDENTITY_CARD, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30289-02.html" | "30289-03.html" | "30289-05.html" => {
                if qs.is_cond(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30289-04.html" => {
                if qs.is_cond(ctx, 2) {
                    let npc = npc?;
                    ctx.cast_skill(npc, player, SkillKey::new(TRANSFORMATION.0, TRANSFORMATION.1));
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30289-07.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30289-08.html" => {
                if qs.is_cond(ctx, 3)
                    && ctx.has_quest_items(player, IDENTITY_CARD)
                    && ctx.has_quest_items(player, SHUNAIMANS_CONTRACT)
                {
                    ctx.take_items(player, IDENTITY_CARD, -1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                    if ctx.world().transformation_id(player) == 113 {
                        ctx.world_mut().stop_all_effects(player);
                    }
                }
            }
            "30289-10.html" => {
                if qs.is_cond(ctx, 3) {
                    let npc = npc?;
                    ctx.cast_skill(npc, player, SkillKey::new(TRANSFORMATION.0, TRANSFORMATION.1));
                    htmltext = Some(event.to_owned());
                }
            }
            "30289-11.html" => {
                if qs.is_cond(ctx, 3) {
                    ctx.world_mut().stop_all_effects(player);
                    htmltext = Some(event.to_owned());
                }
            }
            "30969-02.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                    htmltext = Some(event.to_owned());
                }
            }
            "reward" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some("30969-03.html".to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        match npc.id {
            IDENTITY_CONFIRM_DEVICE => Some("32578-01.html".to_owned()),
            PASSWORD_ENTRY_DEVICE => Some("32577-01.html".to_owned()),
            DARKNESS_OF_DAWN => Some("32579-01.html".to_owned()),
            SHELF => Some("32580-01.html".to_owned()),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == CLAUDIA_ATHEBALDT {
                    let done = ctx
                        .player_quest_state(player, MAMMONS_CONTRACT)
                        .is_some_and(|q194| q194.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && done {
                        "31001-01.htm"
                    } else {
                        "31001-02.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                CLAUDIA_ATHEBALDT => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31001-07.html".to_owned();
                    }
                }
                JOHN => match qs.get_cond(ctx) {
                    1 => htmltext = "32576-01.html".to_owned(),
                    2 => htmltext = "32576-03.html".to_owned(),
                    _ => {}
                },
                RAYMOND => match qs.get_cond(ctx) {
                    2 => {
                        if ctx.has_quest_items(player, IDENTITY_CARD)
                            && ctx.world().transformation_id(player) != 113
                        {
                            htmltext = "30289-01.html".to_owned();
                        }
                    }
                    3 => {
                        if ctx.has_quest_items(player, IDENTITY_CARD) {
                            htmltext = if ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                                "30289-06.html"
                            } else {
                                "30289-09.html"
                            }
                            .to_owned();
                        }
                    }
                    4 => htmltext = "30289-12.html".to_owned(),
                    _ => {}
                },
                LIGHT_OF_DAWN => {
                    if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, IDENTITY_CARD) {
                        htmltext = "31001-07.html".to_owned();
                    }
                }
                PASSWORD_ENTRY_DEVICE => {
                    if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, IDENTITY_CARD) {
                        htmltext = "32577-02.html".to_owned();
                        ctx.teleport_player(player, -78_240, 205_858, -7_856, 0, false);
                    }
                }
                SHELF => {
                    if qs.is_cond(ctx, 3) && !ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                        ctx.give_items(player, SHUNAIMANS_CONTRACT, 1);
                        htmltext = "32580-02.html".to_owned();
                    }
                }
                DARKNESS_OF_DAWN => {
                    if qs.is_cond(ctx, 3) && !ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                        htmltext = "32579-02.html".to_owned();
                    }
                }
                IASON_HEINE => {
                    if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, SHUNAIMANS_CONTRACT) {
                        htmltext = "30969-01.html".to_owned();
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00195_SevenSignsSecretRitualOfThePriests";

    const NPCS: &str = r#"
        <npc id="30289" type="Folk" level="80" name="Raymond"/>
        <npc id="30969" type="Folk" level="80" name="Iason Heine"/>
        <npc id="31001" type="Folk" level="80" name="Claudia Athebaldt"/>
        <npc id="32575" type="Folk" level="80" name="Light of Dawn"/>
        <npc id="32576" type="Folk" level="80" name="John"/>
        <npc id="32580" type="Folk" level="80" name="Shelf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13822" type="EtcItem" name="Identity Card"><set name="is_questitem" val="true"/></item>
        <item id="13823" type="EtcItem" name="Shunaiman's Contract"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00194_SevenSignsMammonsContract` as far as this quest reads it: a script of that name.
    struct MammonsContract;

    impl Script for MammonsContract {
        fn name(&self) -> &'static str {
            MAMMONS_CONTRACT
        }
        fn path(&self) -> &'static str {
            "quests/Q00194_SevenSignsMammonsContract"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        claudia: Oid,
        john: Oid,
        raymond: Oid,
        iason: Oid,
        shelf: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(MammonsContract);
        registry.add(Q00195SevenSignsSecretRitualOfThePriests);
        let pages = Pages::of(&Q00195SevenSignsSecretRitualOfThePriests)
            .page("31001-01.htm", &["31001-03.html"])
            .page("31001-03.html", &["31001-04.html"])
            .page("31001-04.html", &["31001-05.html"])
            .page("31001-05.html", &["31001-06.html"])
            .plain(&["31001-02.html", "31001-06.html", "31001-07.html"])
            .page("32576-01.html", &["32576-02.html"])
            .plain(&["32576-02.html", "32576-03.html"])
            .page("30289-01.html", &["30289-02.html"])
            .page("30289-02.html", &["30289-03.html"])
            .page("30289-03.html", &["30289-04.html"])
            .plain(&["30289-04.html"])
            .page("30289-06.html", &["30289-07.html"])
            .page("30289-07.html", &["30289-08.html"])
            .plain(&[
                "30289-08.html",
                "30289-09.html",
                "30289-12.html",
                "level_check.html",
            ])
            .page("30969-01.html", &["30969-02.html"])
            .page("30969-02.html", &["reward"])
            .plain(&["30969-03.html", "32580-02.html"])
            .npcs(&[CLAUDIA_ATHEBALDT, JOHN, RAYMOND, IASON_HEINE, SHELF]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            claudia: t.spawn(CLAUDIA_ATHEBALDT, 20),
            john: t.spawn(JOHN, 40),
            raymond: t.spawn(RAYMOND, 60),
            iason: t.spawn(IASON_HEINE, 80),
            shelf: t.spawn(SHELF, 100),
        };
        (t, npcs)
    }

    fn finish_mammons_contract(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(MAMMONS_CONTRACT, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_level_79_and_mammons_contract() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.claudia).contains("31001-02.html"));
        finish_mammons_contract(&mut t, 1);
        assert!(t.quest_page(1, npcs.claudia).contains("31001-01.htm"));
        t.enter_as(2, 78, race::HUMAN, 0, vec![]);
        finish_mammons_contract(&mut t, 2);
        assert!(t.quest_page(2, npcs.claudia).contains("31001-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn john_gives_the_card_the_shelf_the_contract_and_iason_pays() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        finish_mammons_contract(&mut t, 1);
        assert!(t.quest_page(1, npcs.claudia).contains("31001-01.htm"));
        for e in ["31001-03.html", "31001-04.html", "31001-05.html"] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert!(t.event(1, NAME, "31001-06.html").contains("31001-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // John: the identity card, cond 2.
        assert!(t.quest_page(1, npcs.john).contains("32576-01.html"));
        assert!(t.event(1, NAME, "32576-02.html").contains("32576-02.html"));
        assert_eq!(t.count(1, IDENTITY_CARD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Raymond: cond 3 (the transformation cast needs skill tables the test world lacks).
        assert!(t.quest_page(1, npcs.raymond).contains("30289-01.html"));
        assert!(t.event(1, NAME, "30289-02.html").contains("30289-02.html"));
        assert!(t.event(1, NAME, "30289-03.html").contains("30289-03.html"));
        assert!(t.event(1, NAME, "30289-04.html").contains("30289-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The shelf holds the contract.
        // The shelf has a first-talk page; its "search" link is `onTalk`, called directly here.
        let shelf = npcs.shelf;
        let mut answer = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(shelf).unwrap();
            answer = Q00195SevenSignsSecretRitualOfThePriests.on_talk(ctx, npc, Oid(1));
        });
        assert_eq!(answer.as_deref(), Some("32580-02.html"));
        assert_eq!(t.count(1, SHUNAIMANS_CONTRACT), 1);
        assert!(t.quest_page(1, npcs.raymond).contains("30289-06.html"));
        assert!(t.event(1, NAME, "30289-07.html").contains("30289-07.html"));
        assert!(t.event(1, NAME, "30289-08.html").contains("30289-08.html"));
        assert_eq!(t.count(1, IDENTITY_CARD), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Iason Heine pays.
        assert!(t.quest_page(1, npcs.iason).contains("30969-01.html"));
        assert!(t.event(1, NAME, "30969-02.html").contains("30969-02.html"));
        assert!(t.event(1, NAME, "reward").contains("30969-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.claudia).contains("already been completed"));
    }
}
