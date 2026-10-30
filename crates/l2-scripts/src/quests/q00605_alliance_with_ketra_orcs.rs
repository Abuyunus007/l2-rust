//! Quest 605: Alliance with Ketra Orcs (Wahkan in the Ketra Orc Village, Varka badges).
// Java: dist/game/data/scripts/quests/Q00605_AllianceWithKetraOrcs/Q00605_AllianceWithKetraOrcs.java
//
// Pattern: a five-step alliance. Each step trades badges (soldier, officer, captain) of the
// Varka monsters for a Ketra mark; the badges are dropped by a random party member in the
// started state, only as many as the current step asks for (`canGetItem`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const WAHKAN: i32 = 31371;

// Monsters: (id, drop chance out of 1000, minimum cond, badge item). Java's `MOBS`.
const MOBS: [(i32, i32, i32); 21] = [
    (21350, 500, 1),
    (21351, 500, 1),
    (21353, 509, 1),
    (21354, 521, 1),
    (21355, 519, 1),
    (21357, 500, 2),
    (21358, 500, 2),
    (21360, 509, 2),
    (21361, 518, 2),
    (21362, 518, 2),
    (21364, 527, 2),
    (21365, 500, 3),
    (21366, 500, 3),
    (21368, 508, 3),
    (21369, 628, 2),
    (21370, 604, 2),
    (21371, 627, 3),
    (21372, 604, 3),
    (21373, 649, 3),
    (21374, 626, 3),
    (21375, 626, 3), // Disciple of Prophet
];
// NOTE: the last two of Java's `MOBS` (21375, Disciple of Prophet, 626 at cond 3) are added below.

// Items
const VARKA_BADGE_SOLDIER: i32 = 7216;
const VARKA_BADGE_OFFICER: i32 = 7217;
const VARKA_BADGE_CAPTAIN: i32 = 7218;
const VALOR_TOTEM: i32 = 7219;
const WISDOM_TOTEM: i32 = 7220;
const KETRA_MARKS: [i32; 5] = [7211, 7212, 7213, 7214, 7215];
const VARKA_MARKS: [i32; 5] = [7221, 7222, 7223, 7224, 7225];

// Misc
const MIN_LEVEL: i32 = 74;
/// Java `SOLDIER_BADGE_COUNT`, `OFFICER_BADGE_COUNT`, `CAPTAIN_BADGE_COUNT`, indexed by cond - 1.
const SOLDIER_BADGE_COUNT: [i64; 5] = [100, 200, 300, 300, 400];
const OFFICER_BADGE_COUNT: [i64; 5] = [0, 100, 200, 300, 400];
const CAPTAIN_BADGE_COUNT: [i64; 5] = [0, 0, 100, 200, 200];

/// Java `DropInfo.getItemId()`: the badge of the monster's minimum cond.
fn badge_of(min_cond: i32) -> i32 {
    match min_cond {
        1 => VARKA_BADGE_SOLDIER,
        2 => VARKA_BADGE_OFFICER,
        _ => VARKA_BADGE_CAPTAIN,
    }
}

/// Java `canGetItem(qs, itemId)`: the player holds fewer badges of that kind than the cond asks.
fn can_get_item(ctx: &ScriptCtx<'_>, player: ObjectId, cond: i32, item_id: i32) -> bool {
    let index = (cond - 1) as usize;
    let count = match item_id {
        VARKA_BADGE_SOLDIER => SOLDIER_BADGE_COUNT[index],
        VARKA_BADGE_OFFICER => OFFICER_BADGE_COUNT[index],
        VARKA_BADGE_CAPTAIN => CAPTAIN_BADGE_COUNT[index],
        _ => 0,
    };
    ctx.get_quest_items_count(player, item_id) < count
}

/// Java `Q00605_AllianceWithKetraOrcs`.
pub struct Q00605AllianceWithKetraOrcs;

impl Script for Q00605AllianceWithKetraOrcs {
    fn name(&self) -> &'static str {
        "Q00605_AllianceWithKetraOrcs"
    }

    fn path(&self) -> &'static str {
        "quests/Q00605_AllianceWithKetraOrcs"
    }

    fn id(&self) -> i32 {
        605
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAHKAN]);
        s.add_talk_id(&[WAHKAN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[VARKA_BADGE_SOLDIER, VARKA_BADGE_OFFICER, VARKA_BADGE_CAPTAIN]);
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
        let mut htmltext = Some(event.to_owned());
        match event {
            "31371-12a.html" | "31371-12b.html" | "31371-25.html" => {}
            "31371-04.htm" => {
                if ctx.has_at_least_one_quest_item(player, &VARKA_MARKS) {
                    return Some("31371-03.htm".to_owned());
                }
                qs.set_state(ctx, STARTED);
                ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                for (i, mark) in KETRA_MARKS.iter().enumerate() {
                    if ctx.has_quest_items(player, *mark) {
                        qs.set_cond(ctx, i as i32 + 2, false);
                        return Some(format!("31371-0{}.htm", i + 5));
                    }
                }
                qs.set_cond(ctx, 1, false);
            }
            "31371-12.html" => {
                if ctx.get_quest_items_count(player, VARKA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[0] {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, VARKA_BADGE_SOLDIER, -1);
                ctx.give_items(player, KETRA_MARKS[0], 1);
                qs.set_cond(ctx, 2, true);
            }
            "31371-15.html" => {
                if ctx.get_quest_items_count(player, VARKA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[1]
                    || ctx.get_quest_items_count(player, VARKA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[1]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[VARKA_BADGE_SOLDIER, VARKA_BADGE_OFFICER, KETRA_MARKS[0]],
                );
                ctx.give_items(player, KETRA_MARKS[1], 1);
                qs.set_cond(ctx, 3, true);
            }
            "31371-18.html" => {
                if ctx.get_quest_items_count(player, VARKA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[2]
                    || ctx.get_quest_items_count(player, VARKA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[2]
                    || ctx.get_quest_items_count(player, VARKA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[2]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[
                        VARKA_BADGE_SOLDIER,
                        VARKA_BADGE_OFFICER,
                        VARKA_BADGE_CAPTAIN,
                        KETRA_MARKS[1],
                    ],
                );
                ctx.give_items(player, KETRA_MARKS[2], 1);
                qs.set_cond(ctx, 4, true);
            }
            "31371-21.html" => {
                if !ctx.has_quest_items(player, VALOR_TOTEM)
                    || ctx.get_quest_items_count(player, VARKA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[3]
                    || ctx.get_quest_items_count(player, VARKA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[3]
                    || ctx.get_quest_items_count(player, VARKA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[3]
                {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items_many(
                    player,
                    -1,
                    &[
                        VARKA_BADGE_SOLDIER,
                        VARKA_BADGE_OFFICER,
                        VARKA_BADGE_CAPTAIN,
                        VALOR_TOTEM,
                        KETRA_MARKS[2],
                    ],
                );
                ctx.give_items(player, KETRA_MARKS[3], 1);
                qs.set_cond(ctx, 5, true);
            }
            "31371-26.html" => {
                ctx.take_items_many(player, -1, &KETRA_MARKS);
                ctx.take_items_many(player, -1, &[VALOR_TOTEM, WISDOM_TOTEM]);
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_state(killer, STARTED) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        let Some(&(_, chance, min_cond)) = MOBS.iter().find(|(id, _, _)| *id == npc.id) else {
            return;
        };
        let item = badge_of(min_cond);
        let cond = qs.get_cond(ctx);
        if cond >= min_cond
            && cond < 6
            && can_get_item(ctx, member, cond, item)
            && ctx.get_random(1000) < chance
        {
            ctx.give_items(member, item, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let count = |ctx: &ScriptCtx<'_>, id: i32| ctx.get_quest_items_count(player, id);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31371-01.htm"
                } else {
                    "31371-02.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if count(ctx, VARKA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[0] {
                        "31371-11.html"
                    } else {
                        "31371-10.html"
                    }
                    .to_owned();
                }
                2 => {
                    htmltext = if ctx.has_quest_items(player, KETRA_MARKS[0])
                        && count(ctx, VARKA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[1]
                        && count(ctx, VARKA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[1]
                    {
                        "31371-14.html"
                    } else {
                        "31371-13.html"
                    }
                    .to_owned();
                }
                3 => {
                    htmltext = if ctx.has_quest_items(player, KETRA_MARKS[1])
                        && count(ctx, VARKA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[2]
                        && count(ctx, VARKA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[2]
                        && count(ctx, VARKA_BADGE_CAPTAIN) >= CAPTAIN_BADGE_COUNT[2]
                    {
                        "31371-17.html"
                    } else {
                        "31371-16.html"
                    }
                    .to_owned();
                }
                4 => {
                    htmltext = if ctx.has_all_quest_items(player, &[KETRA_MARKS[2], VALOR_TOTEM])
                        && count(ctx, VARKA_BADGE_SOLDIER) >= SOLDIER_BADGE_COUNT[3]
                        && count(ctx, VARKA_BADGE_OFFICER) >= OFFICER_BADGE_COUNT[3]
                        && count(ctx, VARKA_BADGE_CAPTAIN) >= CAPTAIN_BADGE_COUNT[3]
                    {
                        "31371-20.html"
                    } else {
                        "31371-19.html"
                    }
                    .to_owned();
                }
                5 => {
                    if !ctx.has_quest_items(player, KETRA_MARKS[3])
                        || !ctx.has_quest_items(player, WISDOM_TOTEM)
                        || count(ctx, VARKA_BADGE_SOLDIER) < SOLDIER_BADGE_COUNT[4]
                        || count(ctx, VARKA_BADGE_OFFICER) < OFFICER_BADGE_COUNT[4]
                        || count(ctx, VARKA_BADGE_CAPTAIN) < CAPTAIN_BADGE_COUNT[4]
                    {
                        return Some("31371-22.html".to_owned());
                    }
                    qs.set_cond(ctx, 6, true);
                    ctx.take_items_many(
                        player,
                        -1,
                        &[
                            VARKA_BADGE_SOLDIER,
                            VARKA_BADGE_OFFICER,
                            VARKA_BADGE_CAPTAIN,
                            WISDOM_TOTEM,
                            KETRA_MARKS[3],
                        ],
                    );
                    ctx.give_items(player, KETRA_MARKS[4], 1);
                    htmltext = "31371-23.html".to_owned();
                }
                6 => {
                    if ctx.has_quest_items(player, KETRA_MARKS[4]) {
                        htmltext = "31371-24.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00605_AllianceWithKetraOrcs";

    const NPCS: &str = r#"
        <npc id="31371" type="Folk" level="70" name="Wahkan"/>
        <npc id="21350" type="Monster" level="20" name="Varka Silenos Recruit"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7216" type="EtcItem" name="Varka Badge - Soldier"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7217" type="EtcItem" name="Varka Badge - Officer"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7211" type="EtcItem" name="Mark of Ketra's Alliance - Level 1"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00605AllianceWithKetraOrcs);
        let pages = Pages::of(&Q00605AllianceWithKetraOrcs)
            .page("31371-01.htm", &["31371-04.htm"])
            .plain(&[
                "31371-02.htm",
                "31371-03.htm",
                "31371-04.htm",
                "31371-10.html",
                "31371-13.html",
            ])
            .page("31371-11.html", &["31371-12.html"])
            .plain(&["31371-12.html", "31371-14.html", "31371-05.htm"])
            .npcs(&[WAHKAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wahkan = t.spawn(WAHKAN, 20);
        (t, wahkan)
    }

    #[test]
    fn a_character_below_level_74_is_sent_away() {
        let (mut t, wahkan) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, wahkan).contains("31371-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_alliance_starts_at_cond_one() {
        let (mut t, wahkan) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, wahkan).contains("31371-01.htm"));
        assert!(t.event(1, NAME, "31371-04.htm").contains("31371-04.htm"));
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
        assert!(t.quest_page(1, wahkan).contains("31371-10.html"));
    }

    #[test]
    fn a_hundred_soldier_badges_make_the_first_trade() {
        let (mut t, wahkan) = setup();
        t.enter(1, 74, vec![item(100, VARKA_BADGE_SOLDIER, 99)]);
        t.quest_page(1, wahkan);
        t.event(1, NAME, "31371-04.htm");
        t.take_writes();
        t.take();
        // 99 badges: the NPC still asks for more.
        assert!(t.quest_page(1, wahkan).contains("31371-10.html"));
        // The Varka Silenos Recruit drops a soldier badge at cond 1 (50%).
        for _ in 0..400 {
            if t.count(1, VARKA_BADGE_SOLDIER) >= 100 {
                break;
            }
            let mob = t.spawn(21350, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, VARKA_BADGE_SOLDIER), 100);
        assert!(t.quest_page(1, wahkan).contains("31371-11.html"));
        assert!(t.event(1, NAME, "31371-12.html").contains("31371-12.html"));
        assert_eq!(
            (t.count(1, VARKA_BADGE_SOLDIER), t.count(1, KETRA_MARKS[0])),
            (0, 1)
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
    }
}
