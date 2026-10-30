//! Quest 344: 1000 Years, the End of Lamentation (Gilmore, Kaien, Rodemai, Garvarentz and Orven
//! in the Antharas lair area; the cave servants drop the articles).
// Java: dist/game/data/scripts/quests/Q00344_1000YearsTheEndOfLamentation/Q00344_1000YearsTheEndOfLamentation.java
//
// Pattern: a party kill drop (articles, a chance per monster), the articles are sold at 60 adena
// each or, with a chance of count / 1000, they give one of four relics. The relic decides the NPC
// that rewards it (the memo state is the relic: 1 hilt, 2 key, 3 necklace, 4 crucifix).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KAIEN: i32 = 30623;
const GARVARENTZ: i32 = 30704;
const GILMORE: i32 = 30754;
const RODEMAI: i32 = 30756;
const ORVEN: i32 = 30857;

// Items
const ARTICLES: i32 = 4269;
const OLD_KEY: ItemHolder = ItemHolder::new(4270, 1);
const OLD_HILT: ItemHolder = ItemHolder::new(4271, 1);
const TOTEM_NECKLACE: ItemHolder = ItemHolder::new(4272, 1);
const CRUCIFIX: ItemHolder = ItemHolder::new(4273, 1);

// Monsters: (id, chance of an article), Java's `MONSTER_CHANCES`.
const MONSTER_CHANCES: [(i32, f64); 10] = [
    (20236, 0.58), // Cave Servant
    (20238, 0.75), // Cave Servant Warrior
    (20237, 0.78), // Cave Servant Archer
    (20239, 0.79), // Cave Servant Captain
    (20240, 0.85), // Royal Cave Servant
    (20272, 0.58), // Cave Servant
    (20273, 0.78), // Cave Servant Archer
    (20274, 0.75), // Cave Servant Warrior
    (20275, 0.79), // Cave Servant Captain
    (20276, 0.85), // Royal Cave Servant
];

// Rewards
const ORIHARUKON_ORE: ItemHolder = ItemHolder::new(1874, 25);
const VARNISH_OF_PURITY: ItemHolder = ItemHolder::new(1887, 10);
const SCROLL_EWC: ItemHolder = ItemHolder::new(951, 1);
const RAID_SWORD: ItemHolder = ItemHolder::new(133, 1);
const COKES: ItemHolder = ItemHolder::new(1879, 55);
const RING_OF_AGES: ItemHolder = ItemHolder::new(885, 1);
const LEATHER: ItemHolder = ItemHolder::new(1882, 70);
const COARSE_BONE_POWDER: ItemHolder = ItemHolder::new(1881, 50);
const HEAVY_DOOM_HAMMER: ItemHolder = ItemHolder::new(191, 1);
const STONE_OF_PURITY: ItemHolder = ItemHolder::new(1875, 19);
const SCROLL_EAC: ItemHolder = ItemHolder::new(952, 5);
const DRAKE_LEATHER_BOOTS: ItemHolder = ItemHolder::new(2437, 1);

// Misc
const MIN_LEVEL: i32 = 48;

/// Java `ItemHolder`: an item id and a count.
#[derive(Clone, Copy)]
struct ItemHolder {
    id: i32,
    count: i64,
}

impl ItemHolder {
    const fn new(id: i32, count: i64) -> Self {
        Self { id, count }
    }
}

/// Java `Q00344_1000YearsTheEndOfLamentation`.
pub struct Q00344ThousandYearsTheEndOfLamentation;

impl Script for Q00344ThousandYearsTheEndOfLamentation {
    fn name(&self) -> &'static str {
        "Q00344_1000YearsTheEndOfLamentation"
    }

    fn path(&self) -> &'static str {
        "quests/Q00344_1000YearsTheEndOfLamentation"
    }

    fn id(&self) -> i32 {
        344
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GILMORE]);
        s.add_talk_id(&[KAIEN, GARVARENTZ, GILMORE, RODEMAI, ORVEN]);
        let ids: Vec<i32> = MONSTER_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[ARTICLES, OLD_KEY.id, OLD_HILT.id, TOTEM_NECKLACE.id, CRUCIFIX.id]);
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
            "30754-03.htm" | "30754-16.html" => {
                htmltext = Some(event.to_owned());
            }
            "30754-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30754-08.html" => {
                if qs.is_cond(ctx, 1) {
                    let count = ctx.get_quest_items_count(player, ARTICLES);
                    if count < 1 {
                        htmltext = Some("30754-07.html".to_owned());
                    } else {
                        ctx.take_items(player, ARTICLES, -1);
                        if i64::from(ctx.get_random(1000)) >= count {
                            ctx.give_adena(player, count * 60, true);
                            htmltext = Some(event.to_owned());
                        } else {
                            qs.set_cond(ctx, 2, true);
                            match ctx.get_random(4) {
                                0 => {
                                    qs.set_memo_state(ctx, 1);
                                    ctx.give_items(player, OLD_HILT.id, OLD_HILT.count);
                                }
                                1 => {
                                    qs.set_memo_state(ctx, 2);
                                    ctx.give_items(player, OLD_KEY.id, OLD_KEY.count);
                                }
                                2 => {
                                    qs.set_memo_state(ctx, 3);
                                    ctx.give_items(player, TOTEM_NECKLACE.id, TOTEM_NECKLACE.count);
                                }
                                3 => {
                                    qs.set_memo_state(ctx, 4);
                                    ctx.give_items(player, CRUCIFIX.id, CRUCIFIX.count);
                                }
                                _ => {}
                            }

                            htmltext = Some("30754-09.html".to_owned());
                        }
                    }
                }
            }
            "30754-17.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                    qs.exit_quest(ctx, true, true);
                }
            }
            "relic_info" => match qs.get_memo_state(ctx) {
                1 => htmltext = Some("30754-10.html".to_owned()),
                2 => htmltext = Some("30754-11.html".to_owned()),
                3 => htmltext = Some("30754-12.html".to_owned()),
                4 => htmltext = Some("30754-13.html".to_owned()),
                _ => {}
            },
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            GILMORE => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30754-02.htm"
                    } else {
                        "30754-01.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = if ctx.has_quest_items(talker, ARTICLES) {
                            "30754-06.html"
                        } else {
                            "30754-05.html"
                        }
                        .to_owned();
                    } else if ctx.has_quest_items(talker, OLD_KEY.id)
                        || ctx.has_quest_items(talker, OLD_HILT.id)
                        || ctx.has_quest_items(talker, TOTEM_NECKLACE.id)
                        || ctx.has_quest_items(talker, CRUCIFIX.id)
                    {
                        htmltext = "30754-14.html".to_owned();
                    } else {
                        qs.set_cond(ctx, 1, false);
                        htmltext = "30754-15.html".to_owned();
                    }
                } else {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            KAIEN => {
                if qs.get_memo_state(ctx) == 1 {
                    if ctx.has_quest_items(talker, OLD_HILT.id) {
                        ctx.take_items(talker, OLD_HILT.id, -1);
                        let rand = ctx.get_random(100);
                        if rand <= 52 {
                            ctx.reward_items(talker, ORIHARUKON_ORE.id, ORIHARUKON_ORE.count);
                        } else if rand <= 76 {
                            ctx.reward_items(talker, VARNISH_OF_PURITY.id, VARNISH_OF_PURITY.count);
                        } else if rand <= 98 {
                            ctx.reward_items(talker, SCROLL_EWC.id, SCROLL_EWC.count);
                        } else {
                            ctx.reward_items(talker, RAID_SWORD.id, RAID_SWORD.count);
                        }

                        qs.set_cond(ctx, 1, false);
                        htmltext = "30623-01.html".to_owned();
                    } else {
                        htmltext = "30623-02.html".to_owned();
                    }
                }
            }
            RODEMAI => {
                if qs.get_memo_state(ctx) == 2 {
                    if ctx.has_quest_items(talker, OLD_KEY.id) {
                        ctx.take_items(talker, OLD_KEY.id, -1);
                        let rand = ctx.get_random(100);
                        if rand <= 39 {
                            ctx.reward_items(talker, COKES.id, COKES.count);
                        } else if rand <= 89 {
                            ctx.reward_items(talker, SCROLL_EWC.id, SCROLL_EWC.count);
                        } else {
                            ctx.reward_items(talker, RING_OF_AGES.id, RING_OF_AGES.count);
                        }

                        qs.set_cond(ctx, 1, false);
                        htmltext = "30756-01.html".to_owned();
                    } else {
                        htmltext = "30756-02.html".to_owned();
                    }
                }
            }
            GARVARENTZ => {
                if qs.get_memo_state(ctx) == 3 {
                    if ctx.has_quest_items(talker, TOTEM_NECKLACE.id) {
                        ctx.take_items(talker, TOTEM_NECKLACE.id, -1);
                        let rand = ctx.get_random(100);
                        if rand <= 47 {
                            ctx.reward_items(talker, LEATHER.id, LEATHER.count);
                        } else if rand <= 97 {
                            ctx.reward_items(talker, COARSE_BONE_POWDER.id, COARSE_BONE_POWDER.count);
                        } else {
                            ctx.reward_items(talker, HEAVY_DOOM_HAMMER.id, HEAVY_DOOM_HAMMER.count);
                        }

                        qs.set_cond(ctx, 1, false);
                        htmltext = "30704-01.html".to_owned();
                    } else {
                        htmltext = "30704-02.html".to_owned();
                    }
                }
            }
            ORVEN => {
                if qs.get_memo_state(ctx) == 4 {
                    if ctx.has_quest_items(talker, CRUCIFIX.id) {
                        ctx.take_items(talker, CRUCIFIX.id, -1);
                        let rand = ctx.get_random(100);
                        if rand <= 49 {
                            ctx.reward_items(talker, STONE_OF_PURITY.id, STONE_OF_PURITY.count);
                        } else if rand <= 69 {
                            ctx.reward_items(talker, SCROLL_EAC.id, SCROLL_EAC.count);
                        } else {
                            ctx.reward_items(talker, DRAKE_LEATHER_BOOTS.id, DRAKE_LEATHER_BOOTS.count);
                        }

                        qs.set_cond(ctx, 1, false);
                        htmltext = "30857-01.html".to_owned();
                    } else {
                        htmltext = "30857-02.html".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) {
            let chance = MONSTER_CHANCES
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0.0, |(_, chance)| *chance);
            ctx.give_item_randomly(qs.player(), Some(npc), ARTICLES, 1, 1, 0, chance, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use l2_world::QuestWrite;

    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00344_1000YearsTheEndOfLamentation";

    const NPCS: &str = r#"
        <npc id="30623" type="Folk" level="60" name="Kaien"/>
        <npc id="30704" type="Folk" level="60" name="Garvarentz"/>
        <npc id="30754" type="Folk" level="60" name="Gilmore"/>
        <npc id="30756" type="Folk" level="60" name="Rodemai"/>
        <npc id="30857" type="Folk" level="60" name="Orven"/>
        <npc id="20236" type="Monster" level="50" name="Cave Servant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4269" type="EtcItem" name="Articles"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4270" type="EtcItem" name="Old Key"><set name="is_questitem" val="true"/></item>
        <item id="4271" type="EtcItem" name="Old Hilt"><set name="is_questitem" val="true"/></item>
        <item id="4272" type="EtcItem" name="Totem Necklace"><set name="is_questitem" val="true"/></item>
        <item id="4273" type="EtcItem" name="Crucifix"><set name="is_questitem" val="true"/></item>
        <item id="1874" type="EtcItem" name="Orihaurukon Ore"><set name="is_stackable" val="true"/></item>
        <item id="1887" type="EtcItem" name="Varnish of Purity"><set name="is_stackable" val="true"/></item>
        <item id="951" type="EtcItem" name="Scroll"/>
        <item id="133" type="EtcItem" name="Raid Sword"/>
        <item id="1879" type="EtcItem" name="Cokes"><set name="is_stackable" val="true"/></item>
        <item id="885" type="EtcItem" name="Ring of Ages"/>
        <item id="1882" type="EtcItem" name="Leather"><set name="is_stackable" val="true"/></item>
        <item id="1881" type="EtcItem" name="Coarse Bone Powder"><set name="is_stackable" val="true"/></item>
        <item id="191" type="EtcItem" name="Heavy Doom Hammer"/>
        <item id="1875" type="EtcItem" name="Stone of Purity"><set name="is_stackable" val="true"/></item>
        <item id="952" type="EtcItem" name="Scroll EAC"/>
        <item id="2437" type="EtcItem" name="Drake Leather Boots"/>"#;

    const GILMORE_PAGE: &str = "30754-09.html";

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00344ThousandYearsTheEndOfLamentation);
        let mut pages = Pages::of(&Q00344ThousandYearsTheEndOfLamentation)
            .page("30754-01.htm", &[])
            .page("30754-02.htm", &["30754-03.htm"])
            .page("30754-03.htm", &["30754-04.htm"])
            .plain(&["30754-04.htm", "30754-05.html", "30754-07.html"])
            .page("30754-06.html", &["30754-08.html"])
            .page("30754-08.html", &["30754-16.html", "30754-17.html"])
            .plain(&["30754-10.html", "30754-11.html", "30754-12.html", "30754-13.html"])
            .plain(&["30754-14.html", "30754-16.html", "30754-17.html"])
            .page("30754-15.html", &["30754-16.html", "30754-17.html"])
            .plain(&["30623-01.html", "30623-02.html", "30704-01.html", "30704-02.html"])
            .plain(&["30756-01.html", "30756-02.html", "30857-01.html", "30857-02.html"])
            .npcs(&[GILMORE, KAIEN, GARVARENTZ, RODEMAI, ORVEN]);
        // The relic page: its `-h` bypass (the one Java writes) is the link of `relic_info`.
        pages = pages.file(
            "data/scripts/quests/Q00344_1000YearsTheEndOfLamentation/30754-09.html",
            "<html><body>30754-09.html<a action=\"bypass -h Script Q00344_1000YearsTheEndOfLamentation relic_info\">Tell me more</a></body></html>",
        );
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gilmore = t.spawn(GILMORE, 20);
        let kaien = t.spawn(KAIEN, 40);
        let garvarentz = t.spawn(GARVARENTZ, 60);
        let rodemai = t.spawn(RODEMAI, 80);
        let orven = t.spawn(ORVEN, 100);
        (t, gilmore, kaien, garvarentz, rodemai, orven)
    }

    /// Gilmore's accept: the quest is started (cond 1).
    fn start(t: &mut ScriptTest, gilmore: Oid) {
        assert!(t.quest_page(1, gilmore).contains("30754-02.htm"));
        assert!(t.event(1, NAME, "30754-03.htm").contains("30754-03.htm"));
        assert!(t.event(1, NAME, "30754-04.htm").contains("30754-04.htm"));
    }

    fn relic_of(memo: i32) -> ItemHolder {
        match memo {
            1 => OLD_HILT,
            2 => OLD_KEY,
            3 => TOTEM_NECKLACE,
            _ => CRUCIFIX,
        }
    }

    /// Puts the player at cond 2 with the relic of `memo` (what the articles lottery gives).
    fn prepare_relic(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
            qs.set_memo_state(ctx, memo);
            let relic = relic_of(memo);
            ctx.give_items(Oid(1), relic.id, relic.count);
        });
        t.take_writes();
    }

    #[test]
    fn a_character_below_level_forty_eight_is_sent_away() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, gilmore).contains("30754-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn gilmore_starts_the_quest_at_level_forty_eight() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 48, vec![]);
        assert!(t.quest_page(1, gilmore).contains("30754-02.htm"));
        assert!(t.event(1, NAME, "30754-03.htm").contains("30754-03.htm"));
        t.take_writes();
        assert!(t.event(1, NAME, "30754-04.htm").contains("30754-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, gilmore).contains("30754-05.html"));
    }

    #[test]
    fn the_articles_are_sold_for_sixty_adena_each() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 48, vec![item(100, ARTICLES, 5)]);
        start(&mut t, gilmore);
        t.take_writes();
        assert!(t.quest_page(1, gilmore).contains("30754-06.html"));
        assert!(t.event(1, NAME, "30754-08.html").contains("30754-08.html"));
        assert_eq!(t.count(1, ARTICLES), 0);
        assert_eq!(t.count(1, 57), 300);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn articles_from_the_cave_servants_come_one_by_one() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 48, vec![]);
        start(&mut t, gilmore);
        t.take_writes();
        // Cave servants (58 % each) until the player has an article.
        for _ in 0..200 {
            if t.count(1, ARTICLES) >= 1 {
                break;
            }
            let servant = t.spawn(20236, 60);
            t.kill(1, servant);
        }
        assert!(t.count(1, ARTICLES) >= 1);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, gilmore).contains("30754-06.html"));
    }

    #[test]
    fn a_thousand_articles_always_give_a_relic_and_the_next_cond() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 48, vec![item(100, ARTICLES, 1000)]);
        start(&mut t, gilmore);
        t.take_writes();
        assert!(t.quest_page(1, gilmore).contains("30754-06.html"));
        assert!(t.event(1, NAME, "30754-08.html").contains(GILMORE_PAGE));
        assert_eq!(t.count(1, ARTICLES), 0);
        // `setCond(2, true)`, then the relic of the lottery: its memo state.
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::update(NAME, "cond", "2"));
        let memo = match &writes[1] {
            QuestWrite::InsertVar { var, value, .. } if var == "memoState" => value.parse::<i32>().unwrap(),
            other => panic!("{other:?}"),
        };
        assert!((1..=4).contains(&memo));
        assert_eq!(writes.len(), 2);
        let relic = relic_of(memo);
        assert_eq!(t.count(1, relic.id), 1);

        // Gilmore: the relic information (memo state 1 to 4 has its own page).
        assert!(
            t.event(1, NAME, "relic_info")
                .contains(&format!("30754-{}.html", 9 + memo))
        );
        // Gilmore talks about the relic in the bag: the "14" page.
        assert!(t.quest_page(1, gilmore).contains("30754-14.html"));
    }

    #[test]
    fn each_relic_is_rewarded_by_its_own_npc() {
        // (memo, npc index in setup order: Kaien, Garvarentz, Rodemai, Orven, page of the reward)
        let cases = [
            (1, "30623-01.html"),
            (2, "30756-01.html"),
            (3, "30704-01.html"),
            (4, "30857-01.html"),
        ];
        for (memo, page) in cases {
            let (mut t, _gilmore, kaien, garvarentz, rodemai, orven) = setup();
            t.enter(1, 48, vec![]);
            // A started quest with the relic of `memo` (the articles step is skipped here).
            t.world.run_as_script(NAME, |ctx| {
                let qs = ctx.get_quest_state(Oid(1), true).unwrap();
                qs.start_quest(ctx);
            });
            prepare_relic(&mut t, memo);
            let npc = match memo {
                1 => kaien,
                2 => rodemai,
                3 => garvarentz,
                _ => orven,
            };
            assert!(t.quest_page(1, npc).contains(page), "memo {memo}");
            // The relic goes, a reward comes, the quest goes on at cond 1 (no sound).
            assert_eq!(t.count(1, relic_of(memo).id), 0);
            assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "1")]);
        }
    }

    #[test]
    fn gilmore_sends_the_player_on_when_the_relic_is_gone() {
        let (mut t, gilmore, ..) = setup();
        t.enter(1, 48, vec![]);
        start(&mut t, gilmore);
        // Cond 2 without any relic in the bag: Gilmore sends the player to the relic NPCs.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, gilmore).contains("30754-15.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "1")]);
        // The quit ends the repeatable quest.
        t.take();
        assert!(t.event(1, NAME, "30754-17.html").contains("30754-17.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
