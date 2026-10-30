//! Quest 10272: Light Fragment (Orbyu sends the adventurer to Artius, who wants 100 Fragment Powder).
// Java: dist/game/data/scripts/quests/Q10272_LightFragment/Q10272_LightFragment.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ORBYU: i32 = 32560;
const ARTIUS: i32 = 32559;
const GINBY: i32 = 32566;
const LELRIKIA: i32 = 32567;
const LEKON: i32 = 32557;

// Monsters
const MOBS: [i32; 14] = [
    22536, 22537, 22538, 22539, 22540, 22541, 22542, 22543, 22544, 22547, 22550, 22551, 22552, 22596,
];

// Items
const ADENA_ID: i32 = 57;
const FRAGMENT_POWDER: i32 = 13853;
const LIGHT_FRAGMENT_POWDER: i32 = 13854;
const LIGHT_FRAGMENT: i32 = 13855;
const DROP_CHANCE: f64 = 60.0;

/// Java `Q10271_TheEnvelopingDarkness.class.getSimpleName()`.
const Q10271: &str = "Q10271_TheEnvelopingDarkness";

/// Java `Q10272_LightFragment`.
pub struct Q10272LightFragment;

impl Script for Q10272LightFragment {
    fn name(&self) -> &'static str {
        "Q10272_LightFragment"
    }

    fn path(&self) -> &'static str {
        "quests/Q10272_LightFragment"
    }

    fn id(&self) -> i32 {
        10272
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORBYU]);
        s.add_talk_id(&[ORBYU, ARTIUS, GINBY, LELRIKIA, LEKON]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[FRAGMENT_POWDER, LIGHT_FRAGMENT_POWDER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "32560-06.html" => qs.start_quest(ctx),
            "32559-03.html" => qs.set_cond(ctx, 2, true),
            "32559-07.html" => qs.set_cond(ctx, 3, true),
            "pay" => {
                if ctx.get_quest_items_count(player, ADENA_ID) >= 10000 {
                    ctx.take_items(player, ADENA_ID, 10000);
                    return Some("32566-05.html".to_owned());
                }
                return Some("32566-04a.html".to_owned());
            }
            "32567-04.html" => qs.set_cond(ctx, 4, true),
            "32559-12.html" => qs.set_cond(ctx, 5, true),
            "32557-03.html" => {
                if ctx.get_quest_items_count(player, LIGHT_FRAGMENT_POWDER) >= 100 {
                    ctx.take_items(player, LIGHT_FRAGMENT_POWDER, 100);
                    qs.set(ctx, "wait", "1");
                } else {
                    return Some("32557-04.html".to_owned());
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 5)
        {
            let count = ctx.get_quest_items_count(killer, FRAGMENT_POWDER);
            if count < 100 {
                let mut chance = DROP_CHANCE as i32;
                let mut num_items = chance / 100;
                chance %= 100;
                if ctx.get_random(100) < chance {
                    num_items += 1;
                }

                if num_items > 0 {
                    if count + i64::from(num_items) > 100 {
                        num_items = 100 - count as i32;
                    }

                    if num_items > 0 {
                        ctx.give_items(killer, FRAGMENT_POWDER, i64::from(num_items));
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;

        match npc.id {
            ORBYU => match qs.state(ctx) {
                CREATED => {
                    if ctx.player_level(player) < 75 {
                        htmltext = "32560-03.html".to_owned();
                    } else {
                        let done = ctx
                            .player_quest_state(player, Q10271)
                            .is_some_and(|q| q.is_completed());
                        htmltext = if done { "32560-01.htm" } else { "32560-02.html" }.to_owned();
                    }
                }
                STARTED => htmltext = "32560-06.html".to_owned(),
                COMPLETED => htmltext = "32560-04.html".to_owned(),
                _ => {}
            },
            ARTIUS => {
                if qs.is_completed(ctx) {
                    htmltext = "32559-19.html".to_owned();
                } else {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32559-01.html".to_owned(),
                        2 => htmltext = "32559-04.html".to_owned(),
                        3 => htmltext = "32559-08.html".to_owned(),
                        4 => htmltext = "32559-10.html".to_owned(),
                        5 => {
                            if ctx.get_quest_items_count(player, FRAGMENT_POWDER) >= 100 {
                                htmltext = "32559-15.html".to_owned();
                                qs.set_cond(ctx, 6, true);
                            } else {
                                htmltext = if ctx.has_quest_items(player, FRAGMENT_POWDER) {
                                    "32559-14.html"
                                } else {
                                    "32559-13.html"
                                }
                                .to_owned();
                            }
                        }
                        6 => {
                            if ctx.get_quest_items_count(player, LIGHT_FRAGMENT_POWDER) < 100 {
                                htmltext = "32559-16.html".to_owned();
                            } else {
                                htmltext = "32559-17.html".to_owned();
                                qs.set_cond(ctx, 7, true);
                            }
                        }
                        7 => {
                            // NOTE(java-quirk): Java has "TODO Nothing here?": the no-quest page stays.
                        }
                        8 => {
                            htmltext = "32559-18.html".to_owned();
                            ctx.give_adena(player, 556_980, true);
                            ctx.add_exp_and_sp(player, 1_009_016, 91_363);
                            qs.exit_quest(ctx, false, true);
                        }
                        _ => {}
                    }
                }
            }
            GINBY => match qs.get_cond(ctx) {
                1 | 2 => htmltext = "32566-02.html".to_owned(),
                3 => htmltext = "32566-01.html".to_owned(),
                4 => htmltext = "32566-09.html".to_owned(),
                5 | 6 => htmltext = "32566-10.html".to_owned(),
                _ => {}
            },
            LELRIKIA => match qs.get_cond(ctx) {
                3 => htmltext = "32567-01.html".to_owned(),
                4 => htmltext = "32567-05.html".to_owned(),
                _ => {}
            },
            LEKON => match qs.get_cond(ctx) {
                7 => {
                    if qs.get_int(ctx, "wait") == 1 {
                        htmltext = "32557-05.html".to_owned();
                        qs.unset(ctx, "wait");
                        qs.set_cond(ctx, 8, true);
                        ctx.give_items(player, LIGHT_FRAGMENT, 1);
                    } else {
                        htmltext = "32557-01.html".to_owned();
                    }
                }
                8 => htmltext = "32557-06.html".to_owned(),
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{ItemData, QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q10272_LightFragment";

    const NPCS: &str = r#"
        <npc id="32560" type="Folk" level="80" name="Orbyu"/>
        <npc id="32559" type="Folk" level="80" name="Artius"/>
        <npc id="32566" type="Folk" level="80" name="Ginby"/>
        <npc id="32567" type="Folk" level="80" name="Lelrikia"/>
        <npc id="32557" type="Folk" level="80" name="Lekon"/>
        <npc id="22536" type="Monster" level="80" name="Royal Guard Captain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13853" type="EtcItem" name="Fragment Powder"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13854" type="EtcItem" name="Light Fragment Powder"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13855" type="EtcItem" name="Light Fragment"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10271 (read through `player_quest_state` only).
    struct EnvelopingDarknessStandIn;

    impl Script for EnvelopingDarknessStandIn {
        fn name(&self) -> &'static str {
            Q10271
        }

        fn path(&self) -> &'static str {
            "quests/Q10271_TheEnvelopingDarkness"
        }

        fn id(&self) -> i32 {
            10271
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10272LightFragment);
        registry.add(EnvelopingDarknessStandIn);
        let pages = Pages::of(&Q10272LightFragment)
            .page("32560-01.htm", &["32560-05.htm"])
            .page("32560-05.htm", &["32560-06.html"])
            .plain(&["32560-02.html", "32560-03.html", "32560-04.html", "32560-06.html"])
            .page("32559-01.html", &["32559-02.html"])
            .page("32559-02.html", &["32559-03.html"])
            .plain(&["32559-03.html", "32559-04.html", "32559-18.html", "32559-19.html"])
            .page("32566-01.html", &["32566-04.html"])
            .page("32566-04.html", &["pay"])
            .plain(&["32566-04a.html", "32566-05.html"])
            .page("32557-01.html", &["32557-02.html"])
            .page("32557-02.html", &["32557-03.html"])
            .plain(&["32557-03.html", "32557-04.html", "32557-05.html", "32557-06.html"])
            .npcs(&[ORBYU, ARTIUS, GINBY, LELRIKIA, LEKON]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    /// A character that has finished Q10271.
    fn darkness_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(Q10271.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    /// Character 1 in the middle of the quest.
    fn at_cond(t: &mut ScriptTest, id: i32, level: i32, cond: &str, items: Vec<ItemData>) {
        let mut data = player_data(id, level, items);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some(cond.to_owned()))],
        };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_75_and_the_enveloping_darkness() {
        let mut t = setup();
        let orbyu = t.spawn(ORBYU, 20);
        darkness_done(&mut t, 1, 74);
        assert!(t.quest_page(1, orbyu).contains("32560-03.html"));
        darkness_done(&mut t, 2, 75);
        t.enter(3, 75, vec![]);
        assert!(t.quest_page(3, orbyu).contains("32560-02.html"));
        assert!(t.quest_page(2, orbyu).contains("32560-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn orbyu_starts_and_artius_moves_the_quest_on() {
        let mut t = setup();
        let orbyu = t.spawn(ORBYU, 20);
        let artius = t.spawn(ARTIUS, 30);
        darkness_done(&mut t, 1, 75);
        t.quest_page(1, orbyu);
        t.event(1, NAME, "32560-05.htm");
        assert!(t.event(1, NAME, "32560-06.html").contains("32560-06.html"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, artius).contains("32559-01.html"));
        t.event(1, NAME, "32559-02.html");
        t.event(1, NAME, "32559-03.html");
        assert!(t.quest_page(1, artius).contains("32559-04.html"));
        assert!(!t.take_writes().is_empty());
    }

    #[test]
    fn ginby_takes_ten_thousand_adena() {
        let mut t = setup();
        let ginby = t.spawn(GINBY, 20);
        at_cond(&mut t, 1, 80, "3", vec![item(1000, 57, 10_000)]);
        t.quest_page(1, ginby);
        t.event(1, NAME, "32566-04.html");
        assert!(t.event(1, NAME, "pay").contains("32566-05.html"));
        assert_eq!(t.count(1, 57), 0);
        // Without the adena the page says so and nothing is taken.
        at_cond(&mut t, 2, 80, "3", vec![]);
        t.quest_page(2, ginby);
        t.event(2, NAME, "32566-04.html");
        assert!(t.event(2, NAME, "pay").contains("32566-04a.html"));
    }

    #[test]
    fn the_hunt_collects_fragment_powder_up_to_100() {
        let mut t = setup();
        at_cond(&mut t, 1, 80, "5", vec![]);
        for _ in 0..200 {
            let mob = t.spawn(22536, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, FRAGMENT_POWDER), 100);
    }

    #[test]
    fn artius_pays_at_the_end() {
        let mut t = setup();
        let artius = t.spawn(ARTIUS, 20);
        at_cond(&mut t, 1, 80, "8", vec![]);
        assert!(t.quest_page(1, artius).contains("32559-18.html"));
        assert_eq!(t.count(1, 57), 556_980);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, artius).contains("32559-19.html"));
    }

    #[test]
    fn lekon_trades_100_light_powder_for_the_fragment() {
        let mut t = setup();
        let lekon = t.spawn(LEKON, 20);
        at_cond(&mut t, 1, 80, "7", vec![item(1000, LIGHT_FRAGMENT_POWDER, 100)]);
        assert!(t.quest_page(1, lekon).contains("32557-01.html"));
        t.event(1, NAME, "32557-02.html");
        assert!(t.event(1, NAME, "32557-03.html").contains("32557-03.html"));
        assert_eq!(t.count(1, LIGHT_FRAGMENT_POWDER), 0);
        assert!(t.quest_page(1, lekon).contains("32557-05.html"));
        assert_eq!(t.count(1, LIGHT_FRAGMENT), 1);
        assert!(t.quest_page(1, lekon).contains("32557-06.html"));
    }
}
