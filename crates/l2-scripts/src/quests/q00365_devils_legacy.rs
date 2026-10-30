//! Quest 365: Devil's Legacy (Randolf and Collob in the Town of Gludin).
// Java: dist/game/data/scripts/quests/Q00365_DevilsLegacy/Q00365_DevilsLegacy.java
//
// Pattern: a repeatable kill-and-sell quest. Pirate zombies drop treasure chests, Randolf buys
// them, Collob opens one for 600 adena: most of the time a small prize, sometimes a big prize
// and a poison cast (memo state 2 then locks Collob's chest service).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const COLLOB: i32 = 30092;
const RANDOLF: i32 = 30095;

// Item
const PIRATES_TREASURE_CHEST: i32 = 5873;

// Rewards
const ENCHANT_WEAPON_C: i32 = 951;
const ENCHANT_ARMOR_C: i32 = 952;
const ENCHANT_WEAPON_D: i32 = 955;
const ENCHANT_ARMOR_D: i32 = 956;
const THREAD: i32 = 1868;
const ANIMAL_BONE: i32 = 1872;
const COKES: i32 = 1879;
const STEEL: i32 = 1880;
const COARSE_BONE_POWDER: i32 = 1881;
const LEATHER: i32 = 1882;
const CORD: i32 = 1884;

// Misc
const MIN_LEVEL: i32 = 39;
const ADENA_ID: i32 = 57;

// Skill: `SkillHolder(4035, 2)`
const POISON: (i32, i32) = (4035, 2);

// Mobs: `(npc id, drop chance)`
const MOBS: [(i32, f64); 4] = [
    (20836, 0.47), // pirates_zombie
    (20845, 0.40), // pirates_zombie_captain
    (21629, 0.40), // pirates_zombie_captain_1
    (21630, 0.40), // pirates_zombie_captain_2
];

/// Java `Q00365_DevilsLegacy`.
pub struct Q00365DevilsLegacy;

impl Script for Q00365DevilsLegacy {
    fn name(&self) -> &'static str {
        "Q00365_DevilsLegacy"
    }

    fn path(&self) -> &'static str {
        "quests/Q00365_DevilsLegacy"
    }

    fn id(&self) -> i32 {
        365
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RANDOLF]);
        s.add_talk_id(&[RANDOLF, COLLOB]);
        let ids: Vec<i32> = MOBS.iter().map(|m| m.0).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[PIRATES_TREASURE_CHEST]);
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
            "30095-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30095-05.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30095-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "REWARD" => {
                if !qs.is_memo_state(ctx, 1) {
                    htmltext = Some("30092-04.html".to_owned());
                } else if !ctx.has_quest_items(player, PIRATES_TREASURE_CHEST) {
                    htmltext = Some("30092-02.html".to_owned());
                } else if ctx.adena_of(player) < 600 {
                    htmltext = Some("30092-03.html".to_owned());
                } else {
                    let item_id;
                    if ctx.get_random(100) < 80 {
                        let chance = ctx.get_random(100);
                        item_id = if chance < 1 {
                            ENCHANT_WEAPON_D
                        } else if chance < 4 {
                            ENCHANT_ARMOR_D
                        } else if chance < 36 {
                            THREAD
                        } else if chance < 68 {
                            CORD
                        } else {
                            ANIMAL_BONE
                        };
                        htmltext = Some("30092-05.html".to_owned());
                    } else {
                        let chance = ctx.get_random(1000);
                        item_id = if chance < 10 {
                            ENCHANT_WEAPON_C
                        } else if chance < 40 {
                            ENCHANT_ARMOR_C
                        } else if chance < 60 {
                            ENCHANT_WEAPON_D
                        } else if chance < 260 {
                            ENCHANT_ARMOR_D
                        } else if chance < 445 {
                            COKES
                        } else if chance < 630 {
                            STEEL
                        } else if chance < 815 {
                            LEATHER
                        } else {
                            COARSE_BONE_POWDER
                        };
                        // Java dereferences `npc` here: the event comes from Collob's page.
                        if let Some(npc) = npc {
                            ctx.cast_skill(npc, player, SkillKey::new(POISON.0, POISON.1));
                            let max_mp = ctx
                                .world()
                                .stats(npc.object_id)
                                .map_or(0.0, |s| f64::from(s.max_mp()));
                            ctx.world_mut().set_current_mp(npc.object_id, max_mp, true);
                        }
                        qs.set_memo_state(ctx, 2);
                        htmltext = Some("30092-06.html".to_owned());
                    }
                    ctx.take_items(player, PIRATES_TREASURE_CHEST, 1);
                    ctx.take_items(player, ADENA_ID, 600);
                    ctx.reward_items(player, item_id, 1);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if let Some(qs) = qs {
            let chance = MOBS.iter().find(|m| m.0 == npc.id).map_or(0.0, |m| m.1);
            ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                PIRATES_TREASURE_CHEST,
                1,
                1,
                0,
                chance,
                true,
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            RANDOLF => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30095-01.htm"
                    } else {
                        "30095-03.html"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if ctx.has_quest_items(player, PIRATES_TREASURE_CHEST) {
                        let chest_count = ctx.get_quest_items_count(player, PIRATES_TREASURE_CHEST);
                        ctx.give_adena(player, (chest_count * 400) + 19800, true);
                        ctx.take_items(player, PIRATES_TREASURE_CHEST, -1);
                        htmltext = "30095-04.html".to_owned();
                    } else {
                        htmltext = "30095-07.html".to_owned();
                    }
                }
            }
            COLLOB => {
                if qs.is_started(ctx) {
                    htmltext = if qs.is_memo_state(ctx, 1) {
                        "30092-01.html"
                    } else {
                        "30092-07.html"
                    }
                    .to_owned();
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

    const NAME: &str = "Q00365_DevilsLegacy";

    const NPCS: &str = r#"
        <npc id="30092" type="Folk" level="50" name="Collob"/>
        <npc id="30095" type="Folk" level="50" name="Randolf"/>
        <npc id="20836" type="Monster" level="40" name="Pirate Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5873" type="EtcItem" name="Pirate's Treasure Chest"><set name="is_stackable" val="true"/><set name="is_questitem" val="true"/></item>
        <item id="951" type="EtcItem" name="Scroll: Enchant Weapon (C)"/>
        <item id="952" type="EtcItem" name="Scroll: Enchant Armor (C)"/>
        <item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D)"/>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"/>
        <item id="1868" type="EtcItem" name="Thread"><set name="is_stackable" val="true"/></item>
        <item id="1872" type="EtcItem" name="Animal Bone"><set name="is_stackable" val="true"/></item>
        <item id="1879" type="EtcItem" name="Cokes"><set name="is_stackable" val="true"/></item>
        <item id="1880" type="EtcItem" name="Steel"><set name="is_stackable" val="true"/></item>
        <item id="1881" type="EtcItem" name="Coarse Bone Powder"><set name="is_stackable" val="true"/></item>
        <item id="1882" type="EtcItem" name="Leather"><set name="is_stackable" val="true"/></item>
        <item id="1884" type="EtcItem" name="Cord"><set name="is_stackable" val="true"/></item>"#;

    const REWARDS: [i32; 11] = [951, 952, 955, 956, 1868, 1872, 1879, 1880, 1881, 1882, 1884];

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00365DevilsLegacy);
        let pages = Pages::of(&Q00365DevilsLegacy)
            .page("30095-01.htm", &["30095-02.htm"])
            .page("30095-02.htm", &[])
            .plain(&["30095-03.html", "30095-04.html", "30095-07.html"])
            .page("30095-05.html", &[])
            .page("30095-06.html", &[])
            .page("30092-01.html", &["REWARD"])
            .plain(&[
                "30092-02.html",
                "30092-03.html",
                "30092-04.html",
                "30092-05.html",
                "30092-06.html",
                "30092-07.html",
            ])
            .npcs(&[COLLOB, RANDOLF]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let randolf = t.spawn(RANDOLF, 20);
        let collob = t.spawn(COLLOB, 40);
        (t, randolf, collob)
    }

    #[test]
    fn a_level_38_character_is_sent_away() {
        let (mut t, randolf, _) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, randolf).contains("30095-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_sell_chests_and_quit() {
        let (mut t, randolf, _) = setup();
        t.enter(1, 39, vec![item(5, PIRATES_TREASURE_CHEST, 3)]);
        assert!(t.quest_page(1, randolf).contains("30095-01.htm"));
        assert!(t.event(1, NAME, "30095-02.htm").contains("30095-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Randolf buys the 3 chests: 3 * 400 + 19800 adena.
        assert!(t.quest_page(1, randolf).contains("30095-04.html"));
        assert_eq!(t.count(1, 57), 21_000);
        assert_eq!(t.count(1, PIRATES_TREASURE_CHEST), 0);
        assert!(t.quest_page(1, randolf).contains("30095-07.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn collob_opens_a_chest_for_600_adena() {
        let (mut t, randolf, collob) = setup();
        t.enter(
            1,
            39,
            vec![item(5, PIRATES_TREASURE_CHEST, 1), item(6, 57, 1_000)],
        );
        t.quest_page(1, randolf);
        t.event(1, NAME, "30095-02.htm");
        t.take_writes();

        assert!(t.quest_page(1, collob).contains("30092-01.html"));
        let page = t.event(1, NAME, "REWARD");
        let big = page.contains("30092-06.html");
        assert!(big || page.contains("30092-05.html"), "{page}");
        assert_eq!(t.count(1, PIRATES_TREASURE_CHEST), 0);
        assert_eq!(t.count(1, 57), 400);
        let prizes: i64 = REWARDS.iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(prizes, 1);
        // The big prize locks Collob (memo state 2).
        let writes = t.take_writes();
        if big {
            assert_eq!(writes, [rows::update(NAME, "memoState", "2")]);
            assert!(t.quest_page(1, collob).contains("30092-07.html"));
        } else {
            assert!(writes.is_empty());
        }
    }

    #[test]
    fn the_big_prize_makes_collob_cast_the_poison_and_fills_his_mana() {
        // One chest in five is the big prize (the poison, memo state 2). The world's random
        // numbers are seeded, so the first world that rolls one is the same on every run.
        let skills = r#"<skill id="4035" levels="2" name="Poison"><operateType>A1</operateType><targetType>ONE</targetType></skill>"#;
        let npcs = r#"
            <npc id="30092" type="Folk" level="50" name="Collob"><stats><vitals hp="1000" mp="300"/></stats></npc>
            <npc id="30095" type="Folk" level="50" name="Randolf"/>"#;
        for attempt in 0..100 {
            let mut registry = ScriptRegistry::new();
            registry.add(Q00365DevilsLegacy);
            let pages = Pages::of(&Q00365DevilsLegacy)
                .page("30095-01.htm", &["30095-02.htm"])
                .page("30092-01.html", &["REWARD"])
                .plain(&["30092-05.html", "30092-06.html"])
                .npcs(&[COLLOB, RANDOLF]);
            let mut t = ScriptTest::with_pages(registry, npcs, ITEMS, pages);
            t.set_skills(ITEMS, skills);
            let randolf = t.spawn(RANDOLF, 20);
            let collob = t.spawn(COLLOB, 40);
            t.enter(
                1,
                39,
                vec![item(5, PIRATES_TREASURE_CHEST, 1), item(6, 57, 1_000)],
            );
            for _ in 0..attempt {
                // Burn random numbers so each attempt rolls differently.
                t.world.run_as_script(NAME, |ctx| {
                    ctx.get_random(100);
                });
            }
            t.quest_page(1, randolf);
            t.event(1, NAME, "30095-02.htm");
            t.quest_page(1, collob);
            t.world.set_current_mp(collob, 0.0, true);
            let page = t.event(1, NAME, "REWARD");
            if !page.contains("30092-06.html") {
                continue;
            }
            assert_eq!(
                t.world.script_last_skill_cast(collob),
                Some(SkillKey::new(POISON.0, POISON.1))
            );
            // His mana is full again, so he can cast on the next player too.
            assert_eq!(t.world.script_npc_current_mp(collob), 300.0);
            return;
        }
        panic!("no big prize in 100 tries");
    }

    #[test]
    fn collob_wants_the_chest_and_the_adena() {
        let (mut t, randolf, collob) = setup();
        t.enter(1, 39, vec![]);
        t.quest_page(1, randolf);
        t.event(1, NAME, "30095-02.htm");
        t.quest_page(1, collob);
        assert!(t.event(1, NAME, "REWARD").contains("30092-02.html"));

        let (mut t, randolf, collob) = setup();
        t.enter(1, 39, vec![item(5, PIRATES_TREASURE_CHEST, 1)]);
        t.quest_page(1, randolf);
        t.event(1, NAME, "30095-02.htm");
        t.quest_page(1, collob);
        assert!(t.event(1, NAME, "REWARD").contains("30092-03.html"));
    }

    #[test]
    fn pirate_zombies_drop_chests() {
        let (mut t, randolf, _) = setup();
        t.enter(1, 39, vec![]);
        t.quest_page(1, randolf);
        t.event(1, NAME, "30095-02.htm");
        for _ in 0..30 {
            let mob = t.spawn(20836, 60);
            t.kill(1, mob);
        }
        assert!(t.count(1, PIRATES_TREASURE_CHEST) >= 5);
    }
}
