//! Quest 10295: Seven Signs, Solina's Tomb (Eris Evil Thoughts in the Monastery of Silence, Solina's Tomb).
// Java: dist/game/data/scripts/quests/Q10295_SevenSignsSolinasTomb/Q10295_SevenSignsSolinasTomb.java
//
// Pattern: a memo-state quest (`docs/design/scripts.md`): the memo state is the step of the tomb,
// moved by Eris and Solinas here and by the tomb's instance script (not ported: memo 2 comes
// from there). The four holy items (scroll, shield, sword, staff) are taken from the altars and
// used at the devices; the reward is Eris's experience (one time, not for a sub-class).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ELCADIA: i32 = 32787;
const ERISS_EVIL_THOUGHTS: i32 = 32792;
const SOLINAS_EVIL_THOUGHTS: i32 = 32793;
const SOLINA: i32 = 32794;
const ERIS: i32 = 32795;
const ANAIS: i32 = 32796;
const JUDE_VAN_ETINA: i32 = 32797;
const TELEPORT_CONTROL_DEVICE_1: i32 = 32837;
const POWERFUL_DEVICE_1: i32 = 32838;
const POWERFUL_DEVICE_2: i32 = 32839;
const POWERFUL_DEVICE_3: i32 = 32840;
const POWERFUL_DEVICE_4: i32 = 32841;
const TELEPORT_CONTROL_DEVICE_2: i32 = 32842;
const TOMB_OF_THE_SAINTESS: i32 = 32843;
const TELEPORT_CONTROL_DEVICE_3: i32 = 32844;
const ALTAR_OF_HALLOWS_1: i32 = 32857;
const ALTAR_OF_HALLOWS_2: i32 = 32858;
const ALTAR_OF_HALLOWS_3: i32 = 32859;
const ALTAR_OF_HALLOWS_4: i32 = 32860;

// Items
const SCROLL_OF_ABSTINENCE: i32 = 17228;
const SHIELD_OF_SACRIFICE: i32 = 17229;
const SWORD_OF_HOLY_SPIRIT: i32 = 17230;
const STAFF_OF_BLESSING: i32 = 17231;

// Misc
const MIN_LEVEL: i32 = 81;

/// Java `Q10294_SevenSignsToTheMonasteryOfSilence.class.getSimpleName()`.
const MONASTERY_OF_SILENCE: &str = "Q10294_SevenSignsToTheMonasteryOfSilence";

/// Java `takeItems(player, -1, SCROLL_OF_ABSTINENCE, SHIELD_OF_SACRIFICE, SWORD_OF_HOLY_SPIRIT, STAFF_OF_BLESSING)`.
const HOLY_ITEMS: [i32; 4] = [
    SCROLL_OF_ABSTINENCE,
    SHIELD_OF_SACRIFICE,
    SWORD_OF_HOLY_SPIRIT,
    STAFF_OF_BLESSING,
];

/// Java `Q10295_SevenSignsSolinasTomb`.
pub struct Q10295SevenSignsSolinasTomb;

impl Script for Q10295SevenSignsSolinasTomb {
    fn name(&self) -> &'static str {
        "Q10295_SevenSignsSolinasTomb"
    }

    fn path(&self) -> &'static str {
        "quests/Q10295_SevenSignsSolinasTomb"
    }

    fn id(&self) -> i32 {
        10295
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ERISS_EVIL_THOUGHTS]);
        s.add_talk_id(&[
            ERISS_EVIL_THOUGHTS,
            SOLINAS_EVIL_THOUGHTS,
            SOLINA,
            ERIS,
            ANAIS,
            JUDE_VAN_ETINA,
            TELEPORT_CONTROL_DEVICE_1,
            POWERFUL_DEVICE_1,
            POWERFUL_DEVICE_2,
            POWERFUL_DEVICE_3,
            POWERFUL_DEVICE_4,
            TELEPORT_CONTROL_DEVICE_2,
            TOMB_OF_THE_SAINTESS,
            TELEPORT_CONTROL_DEVICE_3,
            ALTAR_OF_HALLOWS_1,
            ALTAR_OF_HALLOWS_2,
            ALTAR_OF_HALLOWS_3,
            ALTAR_OF_HALLOWS_4,
            ELCADIA,
        ]);
        s.register_quest_items(&[
            SCROLL_OF_ABSTINENCE,
            SHIELD_OF_SACRIFICE,
            SWORD_OF_HOLY_SPIRIT,
            STAFF_OF_BLESSING,
        ]);
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

        let mut htmltext = None;
        match event {
            "32792-02.htm" | "32792-04.htm" | "32792-05.htm" | "32792-06.htm" | "32793-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "32792-03.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32793-02.html" | "32793-03.html" => {
                if qs.is_memo_state(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32793-04.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32793-05.html" | "32794-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "32793-07.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32793-08.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32837-02.html" => {
                if qs.get_memo_state(ctx) > 1 {
                    ctx.take_items_many(player, -1, &HOLY_ITEMS);
                    htmltext = Some(event.to_owned());
                }
            }
            "32838-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(if ctx.has_quest_items(player, SCROLL_OF_ABSTINENCE) {
                        event.to_owned()
                    } else {
                        "32838-03.html".to_owned()
                    });
                }
            }
            "32839-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(if ctx.has_quest_items(player, SHIELD_OF_SACRIFICE) {
                        event.to_owned()
                    } else {
                        "32839-03.html".to_owned()
                    });
                }
            }
            "32840-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(if ctx.has_quest_items(player, SWORD_OF_HOLY_SPIRIT) {
                        event.to_owned()
                    } else {
                        "32840-03.html".to_owned()
                    });
                }
            }
            "32841-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(if ctx.has_quest_items(player, STAFF_OF_BLESSING) {
                        event.to_owned()
                    } else {
                        "32841-03.html".to_owned()
                    });
                }
            }
            "32857-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    if ctx.has_quest_items(player, STAFF_OF_BLESSING) {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_items(player, STAFF_OF_BLESSING, 1);
                        htmltext = Some("32857-03.html".to_owned());
                    }
                }
            }
            "32858-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    if ctx.has_quest_items(player, SWORD_OF_HOLY_SPIRIT) {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_items(player, SWORD_OF_HOLY_SPIRIT, 1);
                        htmltext = Some("32858-03.html".to_owned());
                    }
                }
            }
            "32859-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    if ctx.has_quest_items(player, SCROLL_OF_ABSTINENCE) {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_items(player, SCROLL_OF_ABSTINENCE, 1);
                        htmltext = Some("32859-03.html".to_owned());
                    }
                }
            }
            "32860-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    if ctx.has_quest_items(player, SHIELD_OF_SACRIFICE) {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_items(player, SHIELD_OF_SACRIFICE, 1);
                        htmltext = Some("32860-03.html".to_owned());
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
        if qs.is_completed(ctx) {
            if npc.id == ERISS_EVIL_THOUGHTS {
                htmltext = "32792-07.html".to_owned();
            }
        } else if qs.is_created(ctx) {
            let monastery_done = ctx
                .player_quest_state(player, MONASTERY_OF_SILENCE)
                .is_some_and(|q| q.is_completed());
            if ctx.player_level(player) >= MIN_LEVEL && monastery_done {
                htmltext = "32792-01.htm".to_owned();
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                ERISS_EVIL_THOUGHTS => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 1 {
                        htmltext = "32792-12.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "32792-08.html".to_owned();
                    } else if memo_state > 2 && memo_state < 6 {
                        htmltext = "32792-09.html".to_owned();
                    } else if memo_state == 6 {
                        if ctx.is_sub_class_active(player) {
                            htmltext = "32792-10.html".to_owned();
                        } else {
                            ctx.add_exp_and_sp(player, 125_000_000, 12_500_000);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "32792-11.html".to_owned();
                        }
                    }
                }
                SOLINAS_EVIL_THOUGHTS => match qs.get_memo_state(ctx) {
                    3 => htmltext = "32793-01.html".to_owned(),
                    4 => htmltext = "32793-09.html".to_owned(),
                    5 => htmltext = "32793-10.html".to_owned(),
                    6 => htmltext = "32793-11.html".to_owned(),
                    _ => {}
                },
                SOLINA => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "32794-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 5) {
                        htmltext = "32794-03.html".to_owned();
                    }
                }
                ERIS => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "32795-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 5) {
                        htmltext = "32795-02.html".to_owned();
                    }
                }
                ANAIS => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "32796-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 5) {
                        htmltext = "32796-02.html".to_owned();
                    }
                }
                JUDE_VAN_ETINA => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "32797-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 5) {
                        htmltext = "32797-02.html".to_owned();
                    }
                }
                TELEPORT_CONTROL_DEVICE_1 => {
                    if qs.get_memo_state(ctx) > 1 {
                        ctx.take_items_many(player, -1, &HOLY_ITEMS);
                        htmltext = "32837-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 1) {
                        htmltext = "32837-03.html".to_owned();
                    }
                }
                POWERFUL_DEVICE_1 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32838-01.html".to_owned();
                    }
                }
                POWERFUL_DEVICE_2 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32839-01.html".to_owned();
                    }
                }
                POWERFUL_DEVICE_3 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32840-01.html".to_owned();
                    }
                }
                POWERFUL_DEVICE_4 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32841-01.html".to_owned();
                    }
                }
                TELEPORT_CONTROL_DEVICE_2 => {
                    if qs.get_memo_state(ctx) > 2 {
                        htmltext = "32842-01.html".to_owned();
                    }
                }
                TOMB_OF_THE_SAINTESS => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "32843-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) > 2 {
                        htmltext = "32843-02.html".to_owned();
                    }
                }
                TELEPORT_CONTROL_DEVICE_3 => {
                    if qs.get_memo_state(ctx) > 2 {
                        htmltext = "32844-01.html".to_owned();
                    }
                }
                ALTAR_OF_HALLOWS_1 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32857-01.html".to_owned();
                    }
                }
                ALTAR_OF_HALLOWS_2 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32858-01.html".to_owned();
                    }
                }
                ALTAR_OF_HALLOWS_3 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32859-01.html".to_owned();
                    }
                }
                ALTAR_OF_HALLOWS_4 => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32860-01.html".to_owned();
                    }
                }
                ELCADIA => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state < 1 {
                        htmltext = "32787-01.html".to_owned();
                    } else {
                        match memo_state {
                            1 => htmltext = "32787-02.html".to_owned(),
                            2 => htmltext = "32787-03.html".to_owned(),
                            3 => htmltext = "32787-04.html".to_owned(),
                            4 => htmltext = "32787-05.html".to_owned(),
                            5 => htmltext = "32787-06.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10295_SevenSignsSolinasTomb";

    const NPCS: &str = r#"
        <npc id="32792" type="Folk" level="80" name="Eris Evil Thoughts"/>
        <npc id="32793" type="Folk" level="80" name="Solinas Evil Thoughts"/>
        <npc id="32794" type="Folk" level="80" name="Solina"/>
        <npc id="32787" type="Folk" level="80" name="Elcadia"/>
        <npc id="32838" type="Folk" level="80" name="Powerful Device"/>
        <npc id="32837" type="Folk" level="80" name="Teleport Control Device"/>
        <npc id="32857" type="Folk" level="80" name="Altar of Hallows"/>
        <npc id="32841" type="Folk" level="80" name="Powerful Device"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17228" type="EtcItem" name="Scroll of Abstinence"><set name="is_questitem" val="true"/></item>
        <item id="17229" type="EtcItem" name="Shield of Sacrifice"><set name="is_questitem" val="true"/></item>
        <item id="17230" type="EtcItem" name="Sword of Holy Spirit"><set name="is_questitem" val="true"/></item>
        <item id="17231" type="EtcItem" name="Staff of Blessing"><set name="is_questitem" val="true"/></item>"#;

    /// Stands in for Q10294 (its completion is read through `player_quest_state` only).
    struct MonasteryStandIn;

    impl Script for MonasteryStandIn {
        fn name(&self) -> &'static str {
            MONASTERY_OF_SILENCE
        }

        fn path(&self) -> &'static str {
            "quests/Q10294_SevenSignsToTheMonasteryOfSilence"
        }

        fn id(&self) -> i32 {
            10294
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        eris: Oid,
        solinas: Oid,
        solina: Oid,
        elcadia: Oid,
        device1: Oid,
        device4: Oid,
        teleport1: Oid,
        altar1: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10295SevenSignsSolinasTomb);
        registry.add(MonasteryStandIn);
        let pages = Pages::of(&Q10295SevenSignsSolinasTomb)
            .page("32792-01.htm", &["32792-02.htm"])
            .page("32792-02.htm", &["32792-03.htm", "32792-04.htm", "32792-06.htm"])
            .plain(&[
                "32792-03.htm",
                "32792-06.htm",
                "32792-07.html",
                "32792-08.html",
                "32792-09.html",
                "32792-10.html",
                "32792-11.html",
                "32792-12.html",
            ])
            .page("32792-04.htm", &["32792-05.htm"])
            .plain(&["32792-05.htm"])
            .page("32793-01.html", &["32793-02.html"])
            .page("32793-02.html", &["32793-03.html"])
            .page("32793-03.html", &["32793-04.html"])
            .page("32793-04.html", &["32793-05.html"])
            .page("32793-05.html", &["32793-06.html"])
            .page("32793-06.html", &["32793-07.html"])
            .page("32793-07.html", &["32793-08.html"])
            .plain(&["32793-08.html", "32793-09.html", "32793-10.html", "32793-11.html"])
            .page("32794-01.html", &["32794-02.html"])
            .plain(&["32794-02.html", "32794-03.html"])
            .plain(&[
                "32795-01.html",
                "32795-02.html",
                "32796-01.html",
                "32796-02.html",
                "32797-01.html",
                "32797-02.html",
            ])
            // The "-01" pages of the devices and altars link to their "-02" page (plain, see above).
            .page("32837-01.html", &["32837-02.html"])
            .plain(&["32837-02.html", "32837-03.html"])
            .page("32838-01.html", &["32838-02.html"])
            .plain(&["32838-02.html", "32838-03.html"])
            .page("32839-01.html", &["32839-02.html"])
            .plain(&["32839-02.html", "32839-03.html"])
            .page("32840-01.html", &["32840-02.html"])
            .plain(&["32840-02.html", "32840-03.html"])
            .page("32841-01.html", &["32841-02.html"])
            .plain(&["32841-02.html", "32841-03.html"])
            .plain(&["32842-01.html", "32843-01.html", "32843-02.html", "32844-01.html"])
            .page("32857-01.html", &["32857-02.html"])
            .plain(&["32857-02.html", "32857-03.html"])
            .page("32858-01.html", &["32858-02.html"])
            .plain(&["32858-02.html", "32858-03.html"])
            .page("32859-01.html", &["32859-02.html"])
            .plain(&["32859-02.html", "32859-03.html"])
            .page("32860-01.html", &["32860-02.html"])
            .plain(&["32860-02.html", "32860-03.html"])
            .plain(&[
                "32787-01.html",
                "32787-02.html",
                "32787-03.html",
                "32787-04.html",
                "32787-05.html",
                "32787-06.html",
            ])
            .npcs(&[
                ERISS_EVIL_THOUGHTS,
                SOLINAS_EVIL_THOUGHTS,
                SOLINA,
                ELCADIA,
                POWERFUL_DEVICE_1,
                POWERFUL_DEVICE_4,
                TELEPORT_CONTROL_DEVICE_1,
                ALTAR_OF_HALLOWS_1,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            eris: t.spawn(ERISS_EVIL_THOUGHTS, 20),
            solinas: t.spawn(SOLINAS_EVIL_THOUGHTS, 30),
            solina: t.spawn(SOLINA, 40),
            elcadia: t.spawn(ELCADIA, 50),
            device1: t.spawn(POWERFUL_DEVICE_1, 60),
            device4: t.spawn(POWERFUL_DEVICE_4, 70),
            teleport1: t.spawn(TELEPORT_CONTROL_DEVICE_1, 80),
            altar1: t.spawn(ALTAR_OF_HALLOWS_1, 90),
        };
        (t, npcs)
    }

    /// A character started at `memo` (and `cond`), with `items`; Q10294 done when `level` allows.
    fn started_at(
        t: &mut ScriptTest,
        id: i32,
        level: i32,
        cond: &str,
        memo: &str,
        items: Vec<l2_world::ItemData>,
    ) {
        let mut data = player_data(id, level, items);
        data.profile.quests = QuestLoad {
            states: vec![
                (MONASTERY_OF_SILENCE.to_owned(), Some("Completed".to_owned())),
                (NAME.to_owned(), Some("Started".to_owned())),
            ],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some(cond.to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())),
            ],
        };
        t.enter_with(data);
    }

    #[test]
    fn eris_starts_the_tomb_at_81_after_the_monastery() {
        let (mut t, npcs) = setup();
        let mut low = player_data(1, 80, vec![]);
        low.profile.quests = QuestLoad {
            states: vec![(MONASTERY_OF_SILENCE.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(low);
        assert!(!t.quest_page(1, npcs.eris).contains("32792-01.htm"));

        let mut data = player_data(2, 81, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(MONASTERY_OF_SILENCE.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        assert!(t.quest_page(2, npcs.eris).contains("32792-01.htm"));
        assert!(t.event(2, NAME, "32792-02.htm").contains("32792-02.htm"));
        assert!(t.event(2, NAME, "32792-03.htm").contains("32792-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started")),
                rows::of(2, rows::insert(NAME, "memoState", "1")),
            ]
        );
    }

    #[test]
    fn the_altars_and_the_devices_at_step_one() {
        let (mut t, npcs) = setup();
        started_at(&mut t, 1, 81, "1", "1", vec![]);
        // Without the item the device says so; with it the page goes on.
        assert!(t.quest_page(1, npcs.device1).contains("32838-01.html"));
        assert!(t.event(1, NAME, "32838-02.html").contains("32838-03.html"));
        // The altar gives the staff once; the next time it says so.
        assert!(t.quest_page(1, npcs.altar1).contains("32857-01.html"));
        assert!(t.event(1, NAME, "32857-02.html").contains("32857-03.html"));
        assert_eq!(t.count(1, STAFF_OF_BLESSING), 1);
        assert!(t.quest_page(1, npcs.altar1).contains("32857-01.html"));
        assert!(t.event(1, NAME, "32857-02.html").contains("32857-02.html"));
        assert_eq!(t.count(1, STAFF_OF_BLESSING), 1);
        assert!(t.quest_page(1, npcs.device4).contains("32841-01.html"));
        assert!(t.event(1, NAME, "32841-02.html").contains("32841-02.html"));
        // The teleport control device does nothing before step two; Elcadia answers at step one.
        assert!(t.quest_page(1, npcs.teleport1).contains("32837-03.html"));
        assert!(t.quest_page(1, npcs.elcadia).contains("32787-02.html"));
        assert_eq!(t.count(1, STAFF_OF_BLESSING), 1);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_solinas_steps_and_the_experience_for_eris() {
        let (mut t, npcs) = setup();
        started_at(&mut t, 1, 81, "1", "3", vec![]);
        assert!(t.quest_page(1, npcs.solinas).contains("32793-01.html"));
        assert!(t.event(1, NAME, "32793-02.html").contains("32793-02.html"));
        assert!(t.event(1, NAME, "32793-03.html").contains("32793-03.html"));
        assert!(t.event(1, NAME, "32793-04.html").contains("32793-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Solina (memo 4) and the Solinas' page: memo 5.
        assert!(t.quest_page(1, npcs.solina).contains("32794-01.html"));
        assert!(t.event(1, NAME, "32794-02.html").contains("32794-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "5")]);

        // Memo 6 (set by Solinas' page 08 in the tomb): Eris pays the experience and the quest ends.
        started_at(&mut t, 2, 81, "3", "6", vec![]);
        assert!(t.quest_page(2, npcs.eris).contains("32792-11.html"));
        let writes = t.take_writes();
        assert!(writes.contains(&rows::of(2, rows::delete(NAME, false))));
        assert!(writes.contains(&rows::of(2, rows::update(NAME, "<state>", "Completed"))));
    }

    #[test]
    fn the_teleport_device_takes_the_holy_items_from_step_two() {
        let (mut t, npcs) = setup();
        started_at(
            &mut t,
            1,
            81,
            "1",
            "2",
            vec![
                crate::testing::item(100, SCROLL_OF_ABSTINENCE, 1),
                crate::testing::item(101, STAFF_OF_BLESSING, 1),
            ],
        );
        assert!(t.quest_page(1, npcs.teleport1).contains("32837-01.html"));
        assert_eq!(t.count(1, SCROLL_OF_ABSTINENCE), 0);
        assert_eq!(t.count(1, STAFF_OF_BLESSING), 0);
    }
}
