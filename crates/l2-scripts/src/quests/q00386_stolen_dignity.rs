//! Quest 386: Stolen Dignity (Romp plays the same bingo as Q384, for weapon parts).
// Java: dist/game/data/scripts/quests/Q00386_StolenDignity/Q00386_StolenDignity.java
//
// Pattern: as Q00384 (numbered events, bingo board in the quest variables, pages filled from the
// html files); the board helpers are shared with it (`q00384_warehouse_keepers_pastime`).

use l2_util::ObjectId;
use l2_world::script::{Qs, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::q00384_warehouse_keepers_pastime::{
    bingo_select_count, color_board, create_bingo_board, fill_board, is_selected_bingo_number,
    matched_bingo_line_count, select_bingo_number, take_html,
};

// NPC
const WAREHOUSE_KEEPER_ROMP: i32 = 30843;

// Item
const Q_STOLEN_INF_ORE: i32 = 6363;

/// Java's `onKill` switch: monster, the roll's range (100 or 1000) and the chance.
const ORE_DROPS: [(i32, i32, i32); 26] = [
    // NOTE(java-quirk): Java compares `getRandom(1000) < 20.200001`, which is a roll of 0..=20.
    (20670, 1000, 21),  // CRIMSON_DRAKE
    (20671, 1000, 211), // KADIOS
    (20954, 1000, 184), // HUNGRY_CORPSE
    (20956, 1000, 216), // PAST_KNIGHT
    (20958, 100, 17),   // BLADE_DEATH
    (20959, 1000, 273), // DARK_GUARD
    (20960, 1000, 149), // BLOODY_GHOST
    (20963, 1000, 199), // BLOODY_LORD
    (20967, 1000, 257), // PAST_CREATURE
    (20969, 1000, 205), // GIANT_SHADOW
    (20970, 1000, 208), // ANCIENTS_SOLDIER
    (20971, 1000, 299), // ANCIENTS_WARRIOR
    (20974, 100, 44),   // SPITE_SOUL_LEADER
    (20975, 100, 39),   // SPITE_SOUL_WIZARD
    (21001, 1000, 214), // WRECKED_ARCHER
    (21003, 1000, 173), // FLOAT_OF_GRAVE
    (21005, 1000, 211), // GRAVE_PREDATOR
    (21020, 1000, 478), // FALLEN_ORC_SHAMAN
    (21021, 1000, 234), // SHARP_TALON_TIGER
    (21108, 1000, 245), // GLOW_WISP
    (21110, 100, 26),   // MARSH_PREDATOR
    (21113, 100, 37),   // HAMES_ORC_SNIPER
    (21114, 1000, 352), // CURSED_GUARDIAN
    (21116, 1000, 487), // HAMES_ORC_CHIEFTAIN
    (21258, 1000, 487), // FALLEN_ORC_SHAMAN_TRANS
    (21259, 1000, 487), // SHARP_TALON_TIGER_TRANS
];

/// Java `reward(player, qs, count)`: the weapon parts of `getRandom(33)`, in the order of the
/// `case` labels.
const REWARDS: [i32; 33] = [
    5529, // DRAGON_SLAYER_EDGE
    5532, // METEOR_SHOWER_HEAD
    5533, // ELYSIAN_HEAD
    5534, // SOUL_BOW_SHAFT
    5535, // CARNIUM_BOW_SHAFT
    5536, // BLOODY_ORCHID_HEAD
    5537, // SOUL_SEPARATOR_HEAD
    5538, // DRAGON_GRINDER_EDGE
    5539, // BLOOD_TORNADO_EDGE
    5541, // TALLUM_GLAIVE_EDGE
    5542, // HALBARD_EDGE
    5543, // DASPARIONS_STAFF_HEAD
    5544, // WORLDTREES_BRANCH_HEAD
    5545, // DARK_LEGIONS_EDGE_EDGE
    5546, // SWORD_OF_MIRACLE_EDGE
    5547, // ELEMENTAL_SWORD_EDGE
    5548, // TALLUM_BLADE_EDGE
    8331, // INFERNO_MASTER_BLADE
    8341, // EYE_OF_SOUL_PIECE
    8342, // DRAGON_FLAME_HEAD_PIECE
    8349, // DOOM_CRUSHER_HEAD
    8346, // HAMMER_OF_DESTROYER_PIECE
    8712, // SIRR_BLADE_BLADE
    8713, // SWORD_OF_IPOS_BLADE
    8714, // BARAKIEL_AXE_PIECE
    8715, // TUNING_FORK_OF_BEHEMOTH_PIECE
    8716, // NAGA_STORM_PIECE
    8717, // TIPHON_SPEAR_EDGE
    8718, // SHYID_BOW_SHAFT
    8719, // SOBEKK_HURRICANE_EDGE
    8720, // TONGUE_OF_THEMIS_PIECE
    8721, // HAND_OF_CABRIO_HEAD
    8722, // CRYSTAL_OF_DEAMON_PIECE
];

/// Java `reward(player, qs, count)`.
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, count: i64) {
    let index = ctx.get_random(REWARDS.len() as i32) as usize;
    if let Some(&item) = REWARDS.get(index) {
        ctx.give_items(player, item, count);
    }
}

/// Java `beforeReward(player, qs, num)`: the sixth pick, three lines give 4 parts, none give 10.
fn before_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, num: i32) -> Option<String> {
    if !is_selected_bingo_number(ctx, qs, num) {
        select_bingo_number(ctx, qs, num);
        let i3 = matched_bingo_line_count(ctx, qs);
        let html = if i3 == 3 && bingo_select_count(ctx, qs) == 6 {
            reward(ctx, player, 4);
            ctx.get_htm(player, "30843-22.html")
        } else if i3 == 0 && bingo_select_count(ctx, qs) == 6 {
            reward(ctx, player, 10);
            ctx.get_htm(player, "30843-24.html")
        } else {
            ctx.get_htm(player, "30843-23.html")
        };
        return color_board(ctx, qs, html);
    }
    let html = ctx.get_htm(player, "30843-25.html");
    fill_board(ctx, qs, html)
}

/// Java `Q00386_StolenDignity`.
pub struct Q00386StolenDignity;

impl Script for Q00386StolenDignity {
    fn name(&self) -> &'static str {
        "Q00386_StolenDignity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00386_StolenDignity"
    }

    fn id(&self) -> i32 {
        386
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAREHOUSE_KEEPER_ROMP]);
        s.add_talk_id(&[WAREHOUSE_KEEPER_ROMP]);
        let mobs: Vec<i32> = ORE_DROPS.iter().map(|(id, ..)| *id).collect();
        s.add_kill_id(&mobs);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext = ctx.get_no_quest_msg(player);
        if npc.id == WAREHOUSE_KEEPER_ROMP {
            if qs.is_created(ctx) {
                if ctx.player_level(player) >= 58 {
                    return Some("30843-01.htm".to_owned());
                }
                return Some("30843-04.html".to_owned());
            }
            if ctx.get_quest_items_count(player, Q_STOLEN_INF_ORE) < 100 {
                return Some("30843-06.html".to_owned());
            }
            return Some("30843-07.html".to_owned());
        }
        Some(htmltext)
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
        if npc?.id != WAREHOUSE_KEEPER_ROMP {
            return None;
        }
        if event == "QUEST_ACCEPTED" {
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            qs.set_memo_state(ctx, 336);
            qs.start_quest(ctx);
            qs.show_question_mark(ctx, 336);
            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
            return Some("30843-05.htm".to_owned());
        }
        if event.contains(".html") {
            return Some(event.to_owned());
        }
        // NOTE(java-quirk): `Integer.parseInt(event)` throws on any other text; here it shows nothing.
        let ask: i32 = event.parse().ok()?;
        match ask {
            // NOTE(java-quirk): the page files are 30843-09a.htm and 30843-09.html; Java names
            // them the other way round.
            3 => Some("30843-09a.html".to_owned()),
            5 => Some("30843-03.html".to_owned()),
            6 => {
                qs.exit_quest(ctx, true, false);
                Some("30843-08.html".to_owned())
            }
            9 => Some("30843-09.htm".to_owned()),
            8 => {
                if ctx.get_quest_items_count(player, Q_STOLEN_INF_ORE) >= 100 {
                    ctx.take_items(player, Q_STOLEN_INF_ORE, 100);
                    create_bingo_board(ctx, qs);
                    return Some("30843-12.html".to_owned());
                }
                Some("30843-11.html".to_owned())
            }
            10..=18 => {
                select_bingo_number(ctx, qs, (ask - 10) + 1);
                let html = ctx.get_htm(player, "30843-13.html");
                fill_board(ctx, qs, html)
            }
            19..=27 => take_html(ctx, player, qs, ask - 18, WAREHOUSE_KEEPER_ROMP),
            55..=63 => before_reward(ctx, player, qs, ask - 54),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = random_player_from_party(ctx, killer, npc) else {
            return;
        };
        let Some(&(_, range, chance)) = ORE_DROPS.iter().find(|(id, ..)| *id == npc.id) else {
            return;
        };
        if ctx.get_random(range) < chance {
            ctx.give_item_randomly(member, Some(npc), Q_STOLEN_INF_ORE, 1, 1, 0, 1.0, true);
        }
    }
}

/// Java `getRandomPlayerFromParty(player, npc)`: the killer counts twice, every started party
/// member in range once.
fn random_player_from_party(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) -> Option<ObjectId> {
    let mut candidates = Vec::new();
    let qs = ctx.get_quest_state(player, false);
    if qs.is_some_and(|qs| qs.is_started(ctx)) {
        candidates.push(player);
        candidates.push(player);
    }
    if ctx.is_in_party(player) {
        for member in ctx.world().party_members_of(player) {
            let member_qs = ctx.get_quest_state(member, false);
            if member_qs.is_some_and(|qs| qs.is_started(ctx))
                && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, member, true)
            {
                candidates.push(member);
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let index = ctx.get_random(candidates.len() as i32) as usize;
    candidates.get(index).copied()
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00386_StolenDignity";

    const NPCS: &str = r#"
        <npc id="30843" type="Folk" level="70" name="Romp"/>
        <npc id="21116" type="Monster" level="60" name="Hames Orc Chieftain"/>"#;

    fn items() -> String {
        let mut out = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
            <item id="6363" type="EtcItem" name="Stolen Infernium Ore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#,
        );
        for id in REWARDS {
            out.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Part {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        out
    }

    fn board(file: &str, first: i32) -> String {
        let links: String = (first..first + 9)
            .map(|n| format!(r#"<a action="bypass -h Script {NAME} {n}">{n}</a>"#))
            .collect();
        let cells: String = (1..=9)
            .map(|i| format!("<font color=<?FontColor{i}?>>[<?Cell{i}?>]</font>"))
            .collect();
        format!("<html><body>{file}{links}{cells}</body></html>")
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00386StolenDignity);
        let dir = "data/scripts/quests/Q00386_StolenDignity";
        let mut pages = Pages::of(&Q00386StolenDignity)
            .page("30843-01.htm", &["5"])
            .page("30843-03.html", &["QUEST_ACCEPTED"])
            .page("30843-06.html", &[])
            .page("30843-07.html", &["8", "9"])
            .plain(&["30843-04.html", "30843-05.htm", "30843-08.html", "30843-11.html"])
            .npcs(&[WAREHOUSE_KEEPER_ROMP]);
        pages = pages.file(&format!("{dir}/30843-12.html"), &board("30843-12.html", 10));
        pages = pages.file(&format!("{dir}/30843-13.html"), &board("30843-13.html", 19));
        for file in ["14", "15", "16", "17", "18", "19", "21"] {
            pages = pages.file(
                &format!("{dir}/30843-{file}.html"),
                &board(&format!("30843-{file}.html"), 19),
            );
        }
        pages = pages.file(&format!("{dir}/30843-20.html"), &board("30843-20.html", 55));
        for file in ["22", "23", "24", "25"] {
            pages = pages.file(
                &format!("{dir}/30843-{file}.html"),
                &board(&format!("30843-{file}.html"), 55),
            );
        }
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let romp = t.spawn(WAREHOUSE_KEEPER_ROMP, 20);
        (t, romp)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, romp) = setup();
        t.enter(1, 57, vec![]);
        assert!(t.quest_page(1, romp).contains("30843-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_starts_the_quest_with_memo_336() {
        let (mut t, romp) = setup();
        t.enter(1, 58, vec![]);
        assert!(t.quest_page(1, romp).contains("30843-01.htm"));
        assert!(t.event(1, NAME, "5").contains("30843-03.html"));
        assert!(t.event(1, NAME, "QUEST_ACCEPTED").contains("30843-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "336"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, romp).contains("30843-06.html"));
    }

    #[test]
    fn a_hundred_ores_buy_a_board_and_six_picks_give_parts() {
        let (mut t, romp) = setup();
        t.enter(1, 58, vec![item(100, Q_STOLEN_INF_ORE, 100)]);
        t.quest_page(1, romp);
        t.event(1, NAME, "5");
        t.event(1, NAME, "QUEST_ACCEPTED");
        t.take_writes();
        assert!(t.quest_page(1, romp).contains("30843-07.html"));
        assert!(t.event(1, NAME, "8").contains("30843-12.html"));
        assert_eq!(t.count(1, Q_STOLEN_INF_ORE), 0);
        assert_eq!(t.take_writes().len(), 2);
        let page = t.event(1, NAME, "10");
        assert!(page.contains("30843-13.html"));
        assert!(!page.contains("<?Cell"), "{page}");
        assert!(t.event(1, NAME, "20").contains("30843-14.html"));
        assert!(t.event(1, NAME, "21").contains("30843-16.html"));
        assert!(t.event(1, NAME, "22").contains("30843-18.html"));
        assert!(t.event(1, NAME, "23").contains("30843-20.html"));
        let page = t.event(1, NAME, "60");
        assert!(
            ["30843-22.html", "30843-23.html", "30843-24.html"]
                .iter()
                .any(|p| page.contains(p)),
            "{page}"
        );
        // Three lines gave 4 parts, none gave 10, else nothing.
        let parts: i64 = REWARDS.iter().map(|&id| t.count(1, id)).sum();
        assert!(matches!(parts, 0 | 4 | 10), "{parts}");
    }

    #[test]
    fn a_hames_chieftain_drops_ore() {
        let (mut t, romp) = setup();
        t.enter(1, 58, vec![]);
        t.quest_page(1, romp);
        t.event(1, NAME, "5");
        t.event(1, NAME, "QUEST_ACCEPTED");
        for _ in 0..100 {
            let mob = t.spawn(21116, 60);
            t.kill(1, mob);
            if t.count(1, Q_STOLEN_INF_ORE) > 0 {
                return;
            }
        }
        panic!("no ore after 100 kills");
    }
}
