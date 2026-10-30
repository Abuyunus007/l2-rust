//! Quest 660: Aiding the Floran Village (Maria or Alex hand it out; the monsters of the Floran
//! area drop three kinds of items that Alex pays for or trades for scrolls and adena).
// Java: dist/game/data/scripts/quests/Q00660_AidingTheFloranVillage/Q00660_AidingTheFloranVillage.java

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALEX: i32 = 30291;
const MARIA: i32 = 30608;

// Items
const SCROLL_ENCHANT_WEAPON_D_GRADE: i32 = 955;
const SCROLL_ENCHANT_ARMOR_D_GRADE: i32 = 956;
const WATCHING_EYES: i32 = 8074;
const ROUGHLY_HEWN_ROCK_GOLEM_SHARD: i32 = 8075;
const DELU_LIZARDMANS_SCALE: i32 = 8076;

// Misc
const MIN_LEVEL: i32 = 30;
const ADENA_REWARD_1: i64 = 13_000;
const ADENA_REWARD_2: i64 = 1_000;
const ADENA_REWARD_3: i64 = 20_000;
const ADENA_REWARD_4: i64 = 2_000;
const ADENA_REWARD_5: i64 = 45_000;
const ADENA_REWARD_6: i64 = 5_000;
const DELU_LIZARDMAN_COMMANDER_DOUBLE_ITEM_CHANCE: i32 = 33;

// Monsters
const DELU_LIZARDMAN_COMMANDER: i32 = 21107; // Delu Lizardman Commander

/// Java `MONSTERS`: `(npc id, item, chance)`.
const MONSTERS: [(i32, i32, f64); 6] = [
    (21102, WATCHING_EYES, 0.500),                 // Watchman of the Plains
    (21106, WATCHING_EYES, 0.630),                 // Cursed Seer
    (21103, ROUGHLY_HEWN_ROCK_GOLEM_SHARD, 0.520), // Roughly Hewn Rock Golem
    (20781, DELU_LIZARDMANS_SCALE, 0.650),         // Delu Lizardman Shaman
    (21104, DELU_LIZARDMANS_SCALE, 0.650),         // Delu Lizardman Supplier
    (21105, DELU_LIZARDMANS_SCALE, 0.750),         // Delu Lizardman Special Agent
];

const QUEST_ITEMS: [i32; 3] = [
    WATCHING_EYES,
    ROUGHLY_HEWN_ROCK_GOLEM_SHARD,
    DELU_LIZARDMANS_SCALE,
];

/// Java `Q00660_AidingTheFloranVillage`.
pub struct Q00660AidingTheFloranVillage;

/// The three counts: eyes, shards, scales.
fn counts(ctx: &ScriptCtx<'_>, player: ObjectId) -> (i64, i64, i64) {
    (
        ctx.get_quest_items_count(player, WATCHING_EYES),
        ctx.get_quest_items_count(player, ROUGHLY_HEWN_ROCK_GOLEM_SHARD),
        ctx.get_quest_items_count(player, DELU_LIZARDMANS_SCALE),
    )
}

/// Java `tradeItems(player, requiredValue, itemCount1, itemCount2, itemCount3)`.
fn trade_items(ctx: &mut ScriptCtx<'_>, player: ObjectId, required_value: i64, counts: (i64, i64, i64)) {
    let (item_count1, item_count2, item_count3) = counts;
    let mut required = required_value;
    if item_count1 < required {
        ctx.take_items(player, WATCHING_EYES, item_count1);
        required -= item_count1;
    } else {
        ctx.take_items(player, WATCHING_EYES, required);
        required = 0;
    }

    if item_count2 < required {
        ctx.take_items(player, ROUGHLY_HEWN_ROCK_GOLEM_SHARD, item_count2);
        required -= item_count2;
    } else {
        ctx.take_items(player, ROUGHLY_HEWN_ROCK_GOLEM_SHARD, required);
        required = 0;
    }

    if item_count3 < required {
        ctx.take_items(player, DELU_LIZARDMANS_SCALE, item_count3);
    } else {
        ctx.take_items(player, DELU_LIZARDMANS_SCALE, required);
    }
}

impl Script for Q00660AidingTheFloranVillage {
    fn name(&self) -> &'static str {
        "Q00660_AidingTheFloranVillage"
    }

    fn path(&self) -> &'static str {
        "quests/Q00660_AidingTheFloranVillage"
    }

    fn id(&self) -> i32 {
        660
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MARIA, ALEX]);
        s.add_talk_id(&[MARIA, ALEX]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|m| m.0).collect();
        s.add_kill_id(&monsters);
        s.add_kill_id(&[DELU_LIZARDMAN_COMMANDER]);
        s.register_quest_items(&QUEST_ITEMS);
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
            "30608-06.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30608-06a.htm".to_owned());
                }
            }
            "30608-02.htm" | "30608-03.html" | "30291-07.html" | "30291-09.html" | "30291-10.html"
            | "30291-14.html" | "30291-18.html" => {
                htmltext = Some(event.to_owned());
            }
            "30291-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    if qs.is_created(ctx) {
                        qs.set_state(ctx, STARTED);
                        qs.set_cond(ctx, 2, false);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                    }
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30291-02.htm".to_owned());
                }
            }
            "30291-06.html" => {
                let (a, b, c) = counts(ctx, player);
                let item_count = a + b + c;
                if item_count > 0 {
                    ctx.give_adena(player, item_count * 100, true);
                    ctx.take_items_many(player, -1, &QUEST_ITEMS);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30291-08.html".to_owned());
                }
            }
            "30291-08a.html" => {
                qs.exit_quest(ctx, true, true);
                ctx.take_items_many(player, -1, &QUEST_ITEMS);
                htmltext = Some(event.to_owned());
            }
            "30291-12.html" => {
                let counts = counts(ctx, player);
                let item_count = counts.0 + counts.1 + counts.2;
                if item_count < 100 {
                    htmltext = Some("30291-11.html".to_owned());
                } else {
                    trade_items(ctx, player, 100, counts);
                    // NOTE(java-quirk): getRandom(99) > 50, one chance less than the others.
                    if ctx.get_random(99) > 50 {
                        ctx.give_items(player, SCROLL_ENCHANT_ARMOR_D_GRADE, 1);
                        ctx.give_adena(player, ADENA_REWARD_1, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_adena(player, ADENA_REWARD_2, true);
                        htmltext = Some("30291-13.html".to_owned());
                    }
                }
            }
            "30291-16.html" => {
                let counts = counts(ctx, player);
                let item_count = counts.0 + counts.1 + counts.2;
                if item_count < 200 {
                    htmltext = Some("30291-15.html".to_owned());
                } else {
                    trade_items(ctx, player, 200, counts);
                    if ctx.get_random(100) >= 50 {
                        if ctx.get_random(2) == 0 {
                            ctx.give_items(player, SCROLL_ENCHANT_ARMOR_D_GRADE, 1);
                            ctx.give_adena(player, ADENA_REWARD_3, true);
                        } else {
                            ctx.give_items(player, SCROLL_ENCHANT_WEAPON_D_GRADE, 1);
                        }
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_adena(player, ADENA_REWARD_4, true);
                        htmltext = Some("30291-17.html".to_owned());
                    }
                }
            }
            "30291-20.html" => {
                let counts = counts(ctx, player);
                let item_count = counts.0 + counts.1 + counts.2;
                if item_count < 500 {
                    htmltext = Some("30291-19.html".to_owned());
                } else {
                    trade_items(ctx, player, 500, counts);
                    if ctx.get_random(100) >= 50 {
                        ctx.give_items(player, SCROLL_ENCHANT_WEAPON_D_GRADE, 1);
                        ctx.give_adena(player, ADENA_REWARD_5, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.give_adena(player, ADENA_REWARD_6, true);
                        htmltext = Some("30291-21.html".to_owned());
                    }
                }
            }
            "30291-22.html" => {
                let (a, b, c) = counts(ctx, player);
                let item_count = a + b + c;
                if item_count <= 0 {
                    htmltext = Some("30291-23.html".to_owned());
                } else {
                    ctx.give_adena(player, item_count * 100, true);
                    htmltext = Some(event.to_owned());
                }
                ctx.take_items_many(player, -1, &QUEST_ITEMS);
                qs.exit_quest(ctx, true, true);
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(player, 2, 2, Some(npc));
        if qs.is_some() {
            // NOTE(java-quirk): the item goes to the killer, not to the party member drawn.
            if let Some((_, item, chance)) = MONSTERS.iter().find(|m| m.0 == npc.id) {
                ctx.give_item_randomly(player, Some(npc), *item, 1, 1, 0, *chance, true);
            } else {
                if ctx.get_random(100) < DELU_LIZARDMAN_COMMANDER_DOUBLE_ITEM_CHANCE {
                    ctx.give_items(player, DELU_LIZARDMANS_SCALE, 2);
                } else {
                    ctx.give_items(player, DELU_LIZARDMANS_SCALE, 1);
                }
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        let level = ctx.player_level(player);
        if state == CREATED {
            match npc.id {
                MARIA => {
                    htmltext = if level >= MIN_LEVEL {
                        "30608-01.htm"
                    } else {
                        "30608-04.html"
                    }
                    .to_owned();
                }
                ALEX => {
                    htmltext = if level >= MIN_LEVEL {
                        "30291-01.htm"
                    } else {
                        "30291-02.htm"
                    }
                    .to_owned();
                }
                _ => {}
            }
        } else if state == STARTED {
            match npc.id {
                MARIA => htmltext = "30608-05.html".to_owned(),
                ALEX => match qs.get_cond(ctx) {
                    1 => {
                        // Quest started with Maria.
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30291-04.html".to_owned();
                    }
                    2 => htmltext = "30291-05.html".to_owned(),
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00660_AidingTheFloranVillage";

    const NPCS: &str = r#"
        <npc id="30291" type="Folk" level="70" name="Alex"/>
        <npc id="30608" type="Folk" level="70" name="Maria"/>
        <npc id="21102" type="Monster" level="40" name="Watchman of the Plains"/>
        <npc id="21107" type="Monster" level="40" name="Delu Lizardman Commander"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D)"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"><set name="is_stackable" val="true"/></item>
        <item id="8074" type="EtcItem" name="Watching Eyes"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8075" type="EtcItem" name="Rock Golem Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8076" type="EtcItem" name="Delu Lizardman Scale"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00660AidingTheFloranVillage);
        let pages = Pages::of(&Q00660AidingTheFloranVillage)
            .page("30608-01.htm", &["30608-02.htm", "30608-03.html"])
            .page("30608-02.htm", &["30608-06.htm", "30608-03.html"])
            .plain(&[
                "30608-03.html",
                "30608-04.html",
                "30608-05.html",
                "30608-06.htm",
                "30608-06a.htm",
            ])
            .page("30291-01.htm", &["30291-03.htm"])
            .plain(&[
                "30291-02.htm",
                "30291-03.htm",
                "30291-04.html",
                "30291-08.html",
                "30291-08a.html",
            ])
            .page("30291-05.html", &["30291-06.html", "30291-09.html"])
            .page("30291-06.html", &["30291-07.html", "30291-08a.html"])
            .page("30291-07.html", &[])
            .page(
                "30291-09.html",
                &["30291-10.html", "30291-14.html", "30291-18.html", "30291-07.html"],
            )
            .page("30291-10.html", &["30291-12.html", "30291-07.html"])
            .page("30291-14.html", &["30291-16.html", "30291-07.html"])
            .page("30291-18.html", &["30291-20.html", "30291-07.html"])
            .page("30291-12.html", &["30291-07.html", "30291-22.html"])
            .page("30291-13.html", &["30291-07.html", "30291-22.html"])
            .page("30291-16.html", &["30291-07.html", "30291-22.html"])
            .page("30291-17.html", &["30291-07.html", "30291-22.html"])
            .page("30291-20.html", &["30291-07.html", "30291-22.html"])
            .page("30291-21.html", &["30291-07.html", "30291-22.html"])
            .plain(&["30291-11.html", "30291-15.html", "30291-19.html", "30291-23.html"])
            .npcs(&[MARIA, ALEX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let maria = t.spawn(MARIA, 20);
        let alex = t.spawn(ALEX, 30);
        (t, maria, alex)
    }

    #[test]
    fn level_30_is_needed() {
        let (mut t, maria, alex) = setup();
        t.enter(1, 29, vec![]);
        assert!(t.quest_page(1, maria).contains("30608-04.html"));
        assert!(t.quest_page(1, alex).contains("30291-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn maria_starts_and_alex_pays_for_the_items() {
        let (mut t, maria, alex) = setup();
        t.enter(
            1,
            30,
            vec![item(100, WATCHING_EYES, 7), item(101, DELU_LIZARDMANS_SCALE, 3)],
        );
        assert!(t.quest_page(1, maria).contains("30608-01.htm"));
        t.event(1, NAME, "30608-02.htm");
        assert!(t.event(1, NAME, "30608-06.htm").contains("30608-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, maria).contains("30608-05.html"));
        // Alex moves the quest to cond 2.
        assert!(t.quest_page(1, alex).contains("30291-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, alex).contains("30291-05.html"));
        assert!(t.event(1, NAME, "30291-06.html").contains("30291-06.html"));
        assert_eq!(t.count(1, 57), 1_000);
        assert_eq!(
            (t.count(1, WATCHING_EYES), t.count(1, DELU_LIZARDMANS_SCALE)),
            (0, 0)
        );
        // The quest ends on request.
        assert!(t.event(1, NAME, "30291-08a.html").contains("30291-08a.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn alex_starts_the_quest_at_cond_2() {
        let (mut t, _, alex) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, alex).contains("30291-01.htm"));
        assert!(t.event(1, NAME, "30291-03.htm").contains("30291-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "cond", "2"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483645")
            ]
        );
    }

    #[test]
    fn the_kills_drop_and_the_hundred_item_trade_pays() {
        let (mut t, _, alex) = setup();
        t.enter(1, 30, vec![]);
        t.quest_page(1, alex);
        t.event(1, NAME, "30291-03.htm");
        t.take_writes();
        // Watchman drops eyes (50%), the commander one or two scales, both at cond 2.
        let mut kills = 0;
        while t.count(1, WATCHING_EYES) < 3 {
            let mob = t.spawn(21102, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100);
        }
        let scales_before = t.count(1, DELU_LIZARDMANS_SCALE);
        let commander = t.spawn(DELU_LIZARDMAN_COMMANDER, 40);
        t.kill(1, commander);
        let scales = t.count(1, DELU_LIZARDMANS_SCALE) - scales_before;
        assert!((1..=2).contains(&scales));

        // Trade 100: a character with 100 items gets the armor scroll and 13000 adena or 1000.
        t.enter(
            2,
            30,
            vec![
                item(200, WATCHING_EYES, 60),
                item(201, ROUGHLY_HEWN_ROCK_GOLEM_SHARD, 50),
            ],
        );
        t.quest_page(2, alex);
        t.event(2, NAME, "30291-03.htm");
        t.quest_page(2, alex);
        t.event(2, NAME, "30291-09.html");
        t.event(2, NAME, "30291-10.html");
        let page = t.event(2, NAME, "30291-12.html");
        assert!(
            page.contains("30291-12.html") || page.contains("30291-13.html"),
            "{page}"
        );
        assert_eq!(
            t.count(2, WATCHING_EYES) + t.count(2, ROUGHLY_HEWN_ROCK_GOLEM_SHARD),
            10
        );
        let adena = t.count(2, 57);
        assert!(adena == 13_000 || adena == 1_000, "{adena}");
        assert_eq!(t.count(2, 956), i64::from(adena == 13_000));
    }

    #[test]
    fn too_few_items_for_the_trades() {
        let (mut t, _, alex) = setup();
        t.enter(1, 30, vec![item(100, WATCHING_EYES, 99)]);
        t.quest_page(1, alex);
        t.event(1, NAME, "30291-03.htm");
        t.quest_page(1, alex);
        t.event(1, NAME, "30291-09.html");
        t.event(1, NAME, "30291-10.html");
        assert!(t.event(1, NAME, "30291-12.html").contains("30291-11.html"));
        assert_eq!(t.count(1, WATCHING_EYES), 99);
    }
}
