//! Quest 905: Refined Dragon Blood (the Separated Souls send the adventurer to the dragon knights and drakes).
// Java: dist/game/data/scripts/quests/Q00905_RefinedDragonBlood/Q00905_RefinedDragonBlood.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SEPARATED_SOULS: [i32; 8] = [32864, 32865, 32866, 32867, 32868, 32869, 32870, 32891];

// Items
const UNREFINED_RED_DRAGON_BLOOD: i32 = 21913;
const UNREFINED_BLUE_DRAGON_BLOOD: i32 = 21914;
const REFINED_RED_DRAGON_BLOOD: i32 = 21903;
const REFINED_BLUE_DRAGON_BLOOD: i32 = 21904;

/// Java `MONSTERS`: the monster and the unrefined blood it drops.
const MONSTERS: [(i32, i32); 10] = [
    (22844, UNREFINED_BLUE_DRAGON_BLOOD), // Dragon Knight
    (22845, UNREFINED_BLUE_DRAGON_BLOOD), // Dragon Knight
    (22846, UNREFINED_BLUE_DRAGON_BLOOD), // Elite Dragon Knight
    (22847, UNREFINED_RED_DRAGON_BLOOD),  // Dragon Knight Warrior
    (22848, UNREFINED_RED_DRAGON_BLOOD),  // Drake Leader
    (22849, UNREFINED_RED_DRAGON_BLOOD),  // Drake Warrior
    (22850, UNREFINED_RED_DRAGON_BLOOD),  // Drake Scout
    (22851, UNREFINED_RED_DRAGON_BLOOD),  // Drake Mage
    (22852, UNREFINED_BLUE_DRAGON_BLOOD), // Dragon Guard
    (22853, UNREFINED_BLUE_DRAGON_BLOOD), // Dragon Mage
];

// Misc
const MIN_LEVEL: i32 = 83;
const DRAGON_BLOOD_COUNT: i64 = 10;

/// Java `Q00905_RefinedDragonBlood`.
pub struct Q00905RefinedDragonBlood;

impl Script for Q00905RefinedDragonBlood {
    fn name(&self) -> &'static str {
        "Q00905_RefinedDragonBlood"
    }

    fn path(&self) -> &'static str {
        "quests/Q00905_RefinedDragonBlood"
    }

    fn id(&self) -> i32 {
        905
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&SEPARATED_SOULS);
        s.add_talk_id(&SEPARATED_SOULS);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[UNREFINED_RED_DRAGON_BLOOD, UNREFINED_BLUE_DRAGON_BLOOD]);
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
        if qs.is_cond(ctx, 1) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
            let Some(item_id) = MONSTERS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map(|(_, item)| *item)
            else {
                return;
            };
            if ctx.get_quest_items_count(player, item_id) < DRAGON_BLOOD_COUNT {
                ctx.give_items(player, item_id, 1);
                if ctx.get_quest_items_count(player, UNREFINED_RED_DRAGON_BLOOD) >= DRAGON_BLOOD_COUNT
                    && ctx.get_quest_items_count(player, UNREFINED_BLUE_DRAGON_BLOOD) >= DRAGON_BLOOD_COUNT
                {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
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
                "32864-04.htm" | "32864-09.html" | "32864-10.html" => htmltext = Some(event.to_owned()),
                "32864-05.htm" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                "32864-11.html" => {
                    ctx.give_items(player, REFINED_RED_DRAGON_BLOOD, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = Some(event.to_owned());
                }
                "32864-12.html" => {
                    ctx.give_items(player, REFINED_BLUE_DRAGON_BLOOD, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
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
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "32864-02.html"
                } else {
                    "32864-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "32864-06.html".to_owned(),
                2 => {
                    if !qs.is_set(ctx, "wait") {
                        htmltext = "32864-07.html".to_owned();
                        qs.set_int(ctx, "wait", 1);
                    } else {
                        htmltext = "32864-08.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "32864-03.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "32864-02.html"
                    } else {
                        "32864-01.htm"
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00905_RefinedDragonBlood";

    const NPCS: &str = r#"
        <npc id="32864" type="Folk" level="85" name="Separated Soul of Dragon"/>
        <npc id="22844" type="Monster" level="80" name="Dragon Knight"/>
        <npc id="22847" type="Monster" level="80" name="Dragon Knight Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="21913" type="EtcItem" name="Unrefined Red Dragon Blood"><set name="is_questitem" val="true"/></item>
        <item id="21914" type="EtcItem" name="Unrefined Blue Dragon Blood"><set name="is_questitem" val="true"/></item>
        <item id="21903" type="EtcItem" name="Refined Red Dragon Blood"/>
        <item id="21904" type="EtcItem" name="Refined Blue Dragon Blood"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00905RefinedDragonBlood);
        let pages = Pages::of(&Q00905RefinedDragonBlood)
            .page("32864-01.htm", &["32864-04.htm"])
            .page("32864-04.htm", &["32864-05.htm"])
            .plain(&["32864-02.html", "32864-03.html", "32864-05.htm", "32864-06.html"])
            .plain(&["32864-07.html"])
            .page("32864-08.html", &["32864-09.html", "32864-10.html"])
            .page("32864-09.html", &["32864-10.html", "32864-11.html"])
            .page("32864-10.html", &["32864-09.html", "32864-12.html"])
            .plain(&["32864-11.html", "32864-12.html"])
            .npcs(&[32864]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let soul = t.spawn(32864, 20);
        (t, soul)
    }

    #[test]
    fn a_character_below_level_83_is_sent_away() {
        let (mut t, soul) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_red_and_ten_blue_bloods_then_the_refined_one_once_a_day() {
        let (mut t, soul) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-01.htm"));
        assert!(t.event(1, NAME, "32864-04.htm").contains("32864-04.htm"));
        assert!(t.event(1, NAME, "32864-05.htm").contains("32864-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, soul).contains("32864-06.html"));

        // Red (22847) and blue (22844) kills, until both bloods reach ten: cond 2 on the last one.
        let mut kills = 0;
        while t.count(1, UNREFINED_RED_DRAGON_BLOOD) < DRAGON_BLOOD_COUNT
            || t.count(1, UNREFINED_BLUE_DRAGON_BLOOD) < DRAGON_BLOOD_COUNT
        {
            kills += 1;
            assert!(kills < 300, "no ten bloods after 300 kills");
            let id = if kills % 2 == 0 { 22844 } else { 22847 };
            let mob = t.spawn(id, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The first talk after cond 2 says `wait`; the next one the other page.
        assert!(t.quest_page(1, soul).contains("32864-07.html"));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "wait", "1")]);
        assert!(t.quest_page(1, soul).contains("32864-08.html"));

        // The refined red blood, `exitQuest(DAILY, true)`.
        assert!(t.event(1, NAME, "32864-09.html").contains("32864-09.html"));
        assert!(t.event(1, NAME, "32864-11.html").contains("32864-11.html"));
        assert_eq!(t.count(1, REFINED_RED_DRAGON_BLOOD), 1);
        assert_eq!(t.count(1, UNREFINED_RED_DRAGON_BLOOD), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, soul).contains("32864-03.html"));
    }
}
