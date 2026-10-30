//! Quest 364: Jovial Accordion (Barbado in the Town of Gludin).
// Java: dist/game/data/scripts/quests/Q00364_JovialAccordion/Q00364_JovialAccordion.java
//
// Pattern: a repeatable delivery quest with random chests. The memo state is the stage:
// 1 = Swan to visit, 2 = chest keys given, 3/4 = one/two stolen goods delivered, 5 = done.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SABRIN: i32 = 30060;
const XABER: i32 = 30075;
const SWAN: i32 = 30957;
const BARBADO: i32 = 30959;
const BEER_CHEST: i32 = 30960;
const CLOTH_CHEST: i32 = 30961;

// Items
const STOLEN_BLACK_BEER: i32 = 4321;
const STOLEN_EVENT_CLOTHES: i32 = 4322;
const CLOTHES_CHEST_KEY: i32 = 4323;
const BEER_CHEST_KEY: i32 = 4324;
const THEME_OF_THE_FEAST: i32 = 4421;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00364_JovialAccordion`.
pub struct Q00364JovialAccordion;

impl Script for Q00364JovialAccordion {
    fn name(&self) -> &'static str {
        "Q00364_JovialAccordion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00364_JovialAccordion"
    }

    fn id(&self) -> i32 {
        364
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BARBADO]);
        s.add_talk_id(&[BARBADO, BEER_CHEST, CLOTH_CHEST, SABRIN, XABER, SWAN]);
        s.register_quest_items(&[
            STOLEN_BLACK_BEER,
            STOLEN_EVENT_CLOTHES,
            CLOTHES_CHEST_KEY,
            BEER_CHEST_KEY,
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
            "START" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some("30959-02.htm".to_owned());
                } else {
                    htmltext = Some("30959-03.htm".to_owned());
                }
            }
            "OPEN_CHEST" => {
                if ctx.has_quest_items(player, BEER_CHEST_KEY) {
                    if ctx.get_random_boolean() {
                        ctx.give_items(player, STOLEN_BLACK_BEER, 1);
                        htmltext = Some("30960-02.html".to_owned());
                    } else {
                        htmltext = Some("30960-03.html".to_owned());
                    }
                    ctx.take_items(player, BEER_CHEST_KEY, -1);
                } else {
                    htmltext = Some("30960-04.html".to_owned());
                }
            }
            "OPEN_CLOTH_CHEST" => {
                if ctx.has_quest_items(player, CLOTHES_CHEST_KEY) {
                    if ctx.get_random_boolean() {
                        ctx.give_items(player, STOLEN_EVENT_CLOTHES, 1);
                        htmltext = Some("30961-02.html".to_owned());
                    } else {
                        htmltext = Some("30961-03.html".to_owned());
                    }
                    ctx.take_items(player, CLOTHES_CHEST_KEY, -1);
                } else {
                    htmltext = Some("30961-04.html".to_owned());
                }
            }
            "30957-02.html" => {
                ctx.give_items(player, CLOTHES_CHEST_KEY, 1);
                ctx.give_items(player, BEER_CHEST_KEY, 1);
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        let all_items = [
            BEER_CHEST_KEY,
            CLOTHES_CHEST_KEY,
            STOLEN_BLACK_BEER,
            STOLEN_EVENT_CLOTHES,
        ];
        if state == CREATED {
            if npc.id == BARBADO {
                htmltext = "30959-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                BARBADO => match qs.get_memo_state(ctx) {
                    1..=4 => htmltext = "30959-04.html".to_owned(),
                    5 => {
                        ctx.reward_items(player, THEME_OF_THE_FEAST, 1);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30959-05.html".to_owned();
                    }
                    _ => {}
                },
                BEER_CHEST => htmltext = "30960-01.html".to_owned(),
                CLOTH_CHEST => htmltext = "30961-01.html".to_owned(),
                SABRIN => {
                    if ctx.has_quest_items(player, STOLEN_BLACK_BEER) {
                        ctx.take_items(player, STOLEN_BLACK_BEER, -1);
                        htmltext = "30060-01.html".to_owned();
                        if qs.is_memo_state(ctx, 2) {
                            qs.set_memo_state(ctx, 3);
                        } else if qs.is_memo_state(ctx, 3) {
                            qs.set_memo_state(ctx, 4);
                        }
                    } else {
                        htmltext = "30060-02.html".to_owned();
                    }
                }
                XABER => {
                    if ctx.has_quest_items(player, STOLEN_EVENT_CLOTHES) {
                        ctx.take_items(player, STOLEN_EVENT_CLOTHES, -1);
                        htmltext = "30075-01.html".to_owned();
                        if qs.is_memo_state(ctx, 2) {
                            qs.set_memo_state(ctx, 3);
                        } else if qs.is_memo_state(ctx, 3) {
                            qs.set_memo_state(ctx, 4);
                        }
                    } else {
                        htmltext = "30075-02.html".to_owned();
                    }
                }
                SWAN => match qs.get_memo_state(ctx) {
                    1 => htmltext = "30957-01.html".to_owned(),
                    2 | 3 => {
                        if ctx.has_at_least_one_quest_item(player, &all_items) {
                            htmltext = "30957-03.html".to_owned();
                        // NOTE(java-quirk): Java's `hasQuestItems(player, a, b, c, d)` is "has all
                        // four", so this branch is true unless the player holds all four items.
                        } else if !ctx.has_all_quest_items(player, &all_items) {
                            if qs.is_memo_state(ctx, 2) {
                                ctx.play_sound(player, QuestSound::ItemsoundQuestGiveup);
                                qs.exit_quest(ctx, true, true);
                                htmltext = "30957-06.html".to_owned();
                            } else {
                                qs.set_memo_state(ctx, 5);
                                qs.set_cond(ctx, 3, true);
                                htmltext = "30957-04.html".to_owned();
                            }
                        }
                    }
                    4 => {
                        if !ctx.has_all_quest_items(player, &all_items) {
                            qs.set_memo_state(ctx, 5);
                            qs.set_cond(ctx, 3, true);
                            ctx.give_adena(player, 100, true);
                            htmltext = "30957-05.html".to_owned();
                        }
                    }
                    5 => htmltext = "30957-07.html".to_owned(),
                    _ => {}
                },
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00364_JovialAccordion";

    const NPCS: &str = r#"
        <npc id="30060" type="Folk" level="50" name="Sabrin"/>
        <npc id="30075" type="Folk" level="50" name="Xaber"/>
        <npc id="30957" type="Folk" level="50" name="Swan"/>
        <npc id="30959" type="Folk" level="50" name="Barbado"/>
        <npc id="30960" type="Folk" level="50" name="Beer Chest"/>
        <npc id="30961" type="Folk" level="50" name="Cloth Chest"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4321" type="EtcItem" name="Stolen Black Beer"><set name="is_questitem" val="true"/></item>
        <item id="4322" type="EtcItem" name="Stolen Event Clothes"><set name="is_questitem" val="true"/></item>
        <item id="4323" type="EtcItem" name="Clothes Chest Key"><set name="is_questitem" val="true"/></item>
        <item id="4324" type="EtcItem" name="Beer Chest Key"><set name="is_questitem" val="true"/></item>
        <item id="4421" type="EtcItem" name="Theme of the Feast"/>"#;

    struct Npcs {
        barbado: Oid,
        swan: Oid,
        sabrin: Oid,
        beer_chest: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00364JovialAccordion);
        let pages = Pages::of(&Q00364JovialAccordion)
            .page("30959-01.htm", &["START"])
            .plain(&[
                "30959-02.htm",
                "30959-03.htm",
                "30959-04.html",
                "30959-05.html",
                "30060-01.html",
                "30060-02.html",
                "30075-01.html",
                "30075-02.html",
                "30957-03.html",
                "30957-04.html",
                "30957-05.html",
                "30957-06.html",
                "30957-07.html",
            ])
            .page("30960-01.html", &["OPEN_CHEST"])
            .plain(&["30960-02.html", "30960-03.html", "30960-04.html"])
            .page("30961-01.html", &["OPEN_CLOTH_CHEST"])
            .plain(&["30961-02.html", "30961-03.html", "30961-04.html"])
            .page("30957-01.html", &["30957-02.html"])
            .plain(&["30957-02.html"])
            .npcs(&[SABRIN, XABER, SWAN, BARBADO, BEER_CHEST, CLOTH_CHEST]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let barbado = t.spawn(BARBADO, 20);
        let swan = t.spawn(SWAN, 40);
        let sabrin = t.spawn(SABRIN, 60);
        let beer_chest = t.spawn(BEER_CHEST, 80);
        (
            t,
            Npcs {
                barbado,
                swan,
                sabrin,
                beer_chest,
            },
        )
    }

    fn started_with(memo: &str, items: Vec<l2_world::ItemData>) -> l2_world::PlayerData {
        let mut data = player_data(1, 20, items);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some("2".to_owned())),
                (
                    NAME.to_owned(),
                    "memoState".to_owned(),
                    Some(memo.to_owned()),
                ),
            ],
        };
        data
    }

    #[test]
    fn a_level_14_character_is_sent_away() {
        let (mut t, n) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, n.barbado).contains("30959-01.htm"));
        assert!(t.event(1, NAME, "START").contains("30959-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_swan_gives_the_keys_and_the_chests_use_them() {
        let (mut t, n) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, n.barbado).contains("30959-01.htm"));
        assert!(t.event(1, NAME, "START").contains("30959-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, n.swan).contains("30957-01.html"));
        assert!(t.event(1, NAME, "30957-02.html").contains("30957-02.html"));
        assert_eq!((t.count(1, CLOTHES_CHEST_KEY), t.count(1, BEER_CHEST_KEY)), (1, 1));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "2"), rows::update(NAME, "cond", "2")]
        );

        // The beer chest takes the key, and the beer comes on a coin flip.
        assert!(t.quest_page(1, n.beer_chest).contains("30960-01.html"));
        let page = t.event(1, NAME, "OPEN_CHEST");
        assert!(page.contains("30960-02.html") || page.contains("30960-03.html"), "{page}");
        assert_eq!(t.count(1, BEER_CHEST_KEY), 0);
        assert_eq!(
            t.count(1, STOLEN_BLACK_BEER),
            i64::from(page.contains("30960-02.html"))
        );
        t.quest_page(1, n.beer_chest);
        assert!(t.event(1, NAME, "OPEN_CHEST").contains("30960-04.html"));
    }

    #[test]
    fn sabrin_takes_the_beer_and_swan_pays_100_adena_at_memo_4() {
        let (mut t, n) = setup();
        t.enter_with(started_with("3", vec![item(5, STOLEN_BLACK_BEER, 1)]));
        assert!(t.quest_page(1, n.sabrin).contains("30060-01.html"));
        assert_eq!(t.count(1, STOLEN_BLACK_BEER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "4")]);
        assert!(t.quest_page(1, n.swan).contains("30957-05.html"));
        assert_eq!(t.count(1, 57), 100);
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "5"), rows::update(NAME, "cond", "3")]
        );
    }

    #[test]
    fn barbado_pays_the_theme_at_memo_5() {
        let (mut t, n) = setup();
        t.enter_with(started_with("5", vec![]));
        assert!(t.quest_page(1, n.barbado).contains("30959-05.html"));
        assert_eq!(t.count(1, THEME_OF_THE_FEAST), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn swan_ends_the_quest_when_everything_is_spent_at_memo_2() {
        let (mut t, n) = setup();
        t.enter_with(started_with("2", vec![]));
        assert!(t.quest_page(1, n.swan).contains("30957-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
