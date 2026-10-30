//! Quest 455: Wings of Sand (the Separated Souls; the raid bosses of the sand drop the baby dragons).
// Java: dist/game/data/scripts/quests/Q00455_WingsOfSand/Q00455_WingsOfSand.java
//
// Pattern: a party kill where every member in range rolls for the item (`action_for_each_player`),
// the item count moves the quest to cond 2 and 3, and the reward is one roll per step beyond the
// first (`giveItems(qs)`), ending the quest for today (`exitQuest(DAILY, true)`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SEPARATED_SOULS: [i32; 8] = [32864, 32865, 32866, 32867, 32868, 32869, 32870, 32891];

// Monsters
const EMERALD_HORN: i32 = 25718;
const DUST_RIDER: i32 = 25719;
const BLEEDING_FLY: i32 = 25720;
const BLACK_DAGGER_WING: i32 = 25721;
const SHADOW_SUMMONER: i32 = 25722;
const SPIKE_SLASHER: i32 = 25723;
const MUSCLE_BOMBER: i32 = 25724;
const MONSTERS: [i32; 7] = [
    EMERALD_HORN,
    DUST_RIDER,
    BLEEDING_FLY,
    BLACK_DAGGER_WING,
    SHADOW_SUMMONER,
    SPIKE_SLASHER,
    MUSCLE_BOMBER,
];

// Item
const LARGE_BABY_DRAGON: i32 = 17250;
const ARMOR_PARTS: [i32; 17] = [
    15660, 15661, 15662, 15663, 15664, 15665, 15666, 15667, 15668, 15669, 15670, 15671, 15672, 15673, 15674,
    15675, 15691,
];

// Misc
const MIN_LEVEL: i32 = 80;
/// Java `CHANCE`: out of 1000, per party member in range of a kill.
const CHANCE: i32 = 350;

/// Java `Q00455_WingsOfSand`.
pub struct Q00455WingsOfSand;

impl Script for Q00455WingsOfSand {
    fn name(&self) -> &'static str {
        "Q00455_WingsOfSand"
    }

    fn path(&self) -> &'static str {
        "quests/Q00455_WingsOfSand"
    }

    fn id(&self) -> i32 {
        455
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&SEPARATED_SOULS);
        s.add_talk_id(&SEPARATED_SOULS);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[LARGE_BABY_DRAGON]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        // Java: `LocationUtil.checkIfInRange(ALT_PARTY_RANGE, npc, player, false) && getRandom(1000) < CHANCE`.
        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
            && ctx.get_random(1000) < CHANCE
        {
            ctx.give_items(player, LARGE_BABY_DRAGON, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            let count = ctx.get_quest_items_count(player, LARGE_BABY_DRAGON);
            if count == 1 {
                qs.set_cond(ctx, 2, true);
            } else if count == 2 {
                qs.set_cond(ctx, 3, true);
            }
        }
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
        if ctx.player_level(player) >= MIN_LEVEL {
            match event {
                "32864-02.htm" | "32864-03.htm" | "32864-04.htm" => {
                    htmltext = Some(event.to_owned());
                }
                "32864-05.htm" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = "32864-01.htm".to_owned();
                }
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "32864-06.html".to_owned(),
                2 | 3 => {
                    let cond = qs.get_cond(ctx);
                    give_reward(ctx, player, cond);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "32864-07.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "32864-08.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "32864-01.htm".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `Q00455_WingsOfSand.giveItems(QuestState)`: one roll per step beyond the first (`cond - 1`
/// rolls). The caller ends the quest.
fn give_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, cond: i32) {
    for _ in 1..cond {
        let chance = ctx.get_random(1000);
        // Java draws the amount before the branch, even when the branch does not use it.
        let parts = i64::from(ctx.get_random_range(1, 2));
        if chance < 50 {
            // Weapon Recipes
            let item = ctx.get_random_range(15815, 15825);
            ctx.give_items(player, item, 1);
        } else if chance < 100 {
            // Armor Recipes
            let item = ctx.get_random_range(15792, 15808);
            ctx.give_items(player, item, parts);
        } else if chance < 150 {
            // Jewelry Recipes
            let item = ctx.get_random_range(15809, 15811);
            ctx.give_items(player, item, parts);
        } else if chance < 250 {
            // Armor Parts
            if let Some(item) = ctx.get_random_entry(&ARMOR_PARTS) {
                ctx.give_items(player, item, parts);
            }
        } else if chance < 500 {
            // Weapon Parts
            let item = ctx.get_random_range(15634, 15644);
            ctx.give_items(player, item, parts);
        } else if chance < 750 {
            // Jewelry Parts
            let item = ctx.get_random_range(15769, 15771);
            ctx.give_items(player, item, parts);
        } else if chance < 900 {
            // Crystals
            let item = ctx.get_random_range(9552, 9557);
            ctx.give_items(player, item, 1);
        } else if chance < 970 {
            // Blessed Scroll: Enchant Armor (S-Grade)
            ctx.give_items(player, 6578, 1);
        } else {
            // Blessed Scroll: Enchant Weapon (S-Grade)
            ctx.give_items(player, 6577, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00455_WingsOfSand";

    const NPCS: &str = r#"
        <npc id="32864" type="Folk" level="70" name="Separated Soul"/>
        <npc id="25718" type="Monster" level="80" name="Emerald Horn"/>"#;

    /// The ids every roll of the reward can give (the recipes, parts, crystals and scrolls).
    fn reward_ids() -> impl Iterator<Item = i32> {
        (6577..=6578)
            .chain(9552..=9557)
            .chain(15634..=15675)
            .chain(15691..=15691)
            .chain(15769..=15825)
    }

    /// The item templates: the dragon, adena and every reward id (a template that is missing is
    /// refused by the harness).
    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17250" type="EtcItem" name="Large Baby Dragon"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in reward_ids() {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Reward {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        xml
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00455WingsOfSand);
        let pages = Pages::of(&Q00455WingsOfSand)
            .page("32864-01.htm", &["32864-02.htm"])
            .page("32864-02.htm", &["32864-03.htm"])
            .page("32864-03.htm", &["32864-04.htm"])
            .page("32864-04.htm", &["32864-05.htm"])
            .plain(&["32864-05.htm", "32864-06.html", "32864-07.html", "32864-08.html"])
            .npcs(&SEPARATED_SOULS);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let soul = t.spawn(32864, 20);
        (t, soul)
    }

    /// Kills Emerald Horns until the quest is at cond 3 (two dragons); returns the kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..1_000 {
            let horn = t.spawn(EMERALD_HORN, 60);
            t.kill(1, horn);
            if t.count(1, LARGE_BABY_DRAGON) >= 2 {
                return kills;
            }
        }
        panic!("no 2 dragons after 1000 kills");
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, soul) = setup();
        t.enter(1, 79, vec![]);
        assert!(!t.quest_page(1, soul).contains("32864-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn two_dragons_then_the_reward_for_the_day() {
        let (mut t, soul) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-01.htm"));
        assert!(t.event(1, NAME, "32864-02.htm").contains("32864-02.htm"));
        assert!(t.event(1, NAME, "32864-03.htm").contains("32864-03.htm"));
        assert!(t.event(1, NAME, "32864-04.htm").contains("32864-04.htm"));
        assert!(t.event(1, NAME, "32864-05.htm").contains("32864-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, soul).contains("32864-06.html"));

        // Each kill in range rolls 35%: the first dragon is cond 2, the second cond 3.
        hunt(&mut t);
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "cond", "2"), rows::update(NAME, "cond", "3")]
        );

        // The reward: two rolls (each gives at least one item), then `exitQuest(DAILY, true)`.
        let before: i64 = reward_ids().map(|id| t.count(1, id)).sum();
        assert!(t.quest_page(1, soul).contains("32864-07.html"));
        let after: i64 = reward_ids().map(|id| t.count(1, id)).sum();
        assert!(after - before >= 2, "{before} -> {after}");
        assert_eq!(t.count(1, LARGE_BABY_DRAGON), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: NAME.to_owned(),
                    repeatable: false,
                },
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done for today.
        assert!(t.quest_page(1, soul).contains("32864-08.html"));
    }
}
