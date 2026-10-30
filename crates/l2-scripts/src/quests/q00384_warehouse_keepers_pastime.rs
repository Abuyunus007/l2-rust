//! Quest 384: Warehouse Keeper's Pastime (Cliff and Baxt play bingo for Irongate medals).
// Java: dist/game/data/scripts/quests/Q00384_WarehouseKeepersPastime/Q00384_WarehouseKeepersPastime.java
//
// Pattern: events are numbers sent by html links; the bingo board is two strings in the quest
// variables (`numbers`: the shuffled 1..9, `selected`: "? ? 5 ? ..."); the pages are read from the
// html files and their `<?CellN?>` / `<?FontColorN?>` placeholders are filled.

use l2_util::ObjectId;
use l2_world::script::{Qs, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CLIFF: i32 = 30182;
const WAREHOUSE_CHIEF_BAXT: i32 = 30685;

// Item
const Q_IRONGATE_MEDAL: i32 = 5964;

// Reward
const MOONSTONE_EARING: i32 = 852;
const DRAKE_LEATHER_BOOTS: i32 = 2437;
const DRAKE_LEATHER_MAIL: i32 = 401;
const MOLD_HARDENER: i32 = 4041;
const NECKLACE_OF_MERMAID: i32 = 917;
const SCRL_OF_ENCH_AM_C: i32 = 952;
const BLACKSMITH_S_FRAME: i32 = 1892;
const SCRL_OF_ENCH_WP_C: i32 = 951;
const ORIHARUKON: i32 = 1893;
const SAMURAI_LONGSWORD: i32 = 135;
const AQUASTONE_RING: i32 = 883;
const SYNTHESIS_COKES: i32 = 1888;
const MITHIRL_ALLOY: i32 = 1890;
const GREAT_HELMET: i32 = 500;
const VARNISH_OF_PURITY: i32 = 1887;
const BLESSED_GLOVES: i32 = 2463;
const CRAFTED_LEATHER: i32 = 1894;

/// Java's `onKill` switch: monster, the roll's range (100 or 1000) and the chance.
const MEDAL_DROPS: [(i32, i32, i32); 26] = [
    (20241, 1000, 328), // HUNTER_GARGOYLE
    (20242, 100, 35),   // DUST_WIND
    (20243, 1000, 312), // THUNDER_WYRM
    (20281, 100, 35),   // DUST_WIND_HOLD
    (20282, 1000, 312), // THUNDER_WYRM_HOLD
    (20286, 1000, 328), // HUNTER_GARGOYLE_HOLD
    (20556, 1000, 176), // GIANT_MONSTEREYE
    (20559, 1000, 226), // ROT_GOLEM
    (20605, 1000, 218), // WEIRD_DRAKE
    (20635, 1000, 216), // CARINKAIN
    (20668, 1000, 312), // GRAVE_GUARD
    (20677, 1000, 522), // TULBEN
    (20758, 100, 38),   // DRAGON_BEARER_CHIEF
    (20759, 100, 39),   // DRAGON_BEARER_WARRIOR
    (20760, 1000, 372), // DRAGON_BEARER_ARCHER
    (20773, 1000, 802), // CONGERER_LORD
    (20774, 1000, 844), // CONGERER
    (20942, 1000, 118), // NIGHTMARE_GUIDE
    (20943, 100, 17),   // NIGHTMARE_KEEPER
    (20944, 1000, 144), // NIGHTMARE_LORD
    (20945, 1000, 162), // CADEINE
    (20946, 100, 25),   // SANHIDRO
    (20947, 1000, 272), // CONNABI
    (20948, 100, 27),   // BARTAL
    (20949, 100, 32),   // LUMINUN
    (20950, 1000, 346), // INNERSEN
];

/// Java `Q00384_WarehouseKeepersPastime`.
pub struct Q00384WarehouseKeepersPastime;

// ---------------------------------------------------------------------- the bingo board

/// Java `createBingoBoard(qs)`: `Collections.shuffle` of 1..9 (Fisher-Yates from the end, with the
/// world RNG instead of a fresh `java.util.Random`).
pub(crate) fn create_bingo_board(ctx: &mut ScriptCtx<'_>, qs: Qs) {
    let mut arr = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    for i in (2..=arr.len()).rev() {
        let j = ctx.get_random(i as i32) as usize;
        arr.swap(i - 1, j);
    }
    let numbers: Vec<String> = arr.iter().map(ToString::to_string).collect();
    qs.set(ctx, "numbers", &numbers.join(" "));
    qs.set(ctx, "selected", "? ? ? ? ? ? ? ? ?");
}

fn selected_of(ctx: &ScriptCtx<'_>, qs: Qs) -> String {
    qs.get(ctx, "selected").unwrap_or_default()
}

fn numbers_of(ctx: &ScriptCtx<'_>, qs: Qs) -> String {
    qs.get(ctx, "numbers").unwrap_or_default()
}

/// Java `getMatchedBingoLineCount(qs)`: the lines (rows, columns, diagonals) fully chosen.
pub(crate) fn matched_bingo_line_count(ctx: &ScriptCtx<'_>, qs: Qs) -> usize {
    let selected = selected_of(ctx, qs);
    let q: Vec<&str> = selected.split(' ').collect();
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    LINES
        .iter()
        .filter(|line| {
            line.iter().all(|&i| {
                q.get(i)
                    .is_some_and(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit()))
            })
        })
        .count()
}

/// Java `selectBingoNumber(qs, num)`: the cell where `num` lies shows it from now on.
pub(crate) fn select_bingo_number(ctx: &mut ScriptCtx<'_>, qs: Qs, num: i32) {
    let numbers = numbers_of(ctx, qs);
    let mut pos = 0;
    for (i, n) in numbers.split(' ').enumerate() {
        if n.parse::<i32>().ok() == Some(num) {
            pos = i;
            break;
        }
    }
    let selected = selected_of(ctx, qs);
    let mut cells: Vec<String> = selected.split(' ').map(str::to_owned).collect();
    if let Some(cell) = cells.get_mut(pos) {
        *cell = num.to_string();
    }
    qs.set(ctx, "selected", &cells.join(" "));
}

/// Java `isSelectedBingoNumber(qs, num)`: `selected.contains(num)`.
pub(crate) fn is_selected_bingo_number(ctx: &ScriptCtx<'_>, qs: Qs, num: i32) -> bool {
    selected_of(ctx, qs).contains(&num.to_string())
}

/// Java `getNumberFromBingoBoard(qs, index)`.
fn number_from_bingo_board(ctx: &ScriptCtx<'_>, qs: Qs, index: usize) -> i32 {
    numbers_of(ctx, qs)
        .split(' ')
        .nth(index)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Java `getBingoSelectCount(qs)`: the digits in `selected`.
pub(crate) fn bingo_select_count(ctx: &ScriptCtx<'_>, qs: Qs) -> usize {
    selected_of(ctx, qs).chars().filter(char::is_ascii_digit).count()
}

/// Java `fillBoard(qs, html)`: chosen cells show their number, the others "?".
pub(crate) fn fill_board(ctx: &ScriptCtx<'_>, qs: Qs, html: Option<String>) -> Option<String> {
    let mut result = html?;
    for i0 in 0..9 {
        let i1 = number_from_bingo_board(ctx, qs, i0);
        let cell = format!("<?Cell{}?>", i0 + 1);
        if is_selected_bingo_number(ctx, qs, i1) {
            result = result.replace(&cell, &i1.to_string());
        } else {
            result = result.replace(&cell, "?");
        }
    }
    Some(result)
}

/// Java `colorBoard(qs, html)`: chosen cells red, the others white, every number shown.
pub(crate) fn color_board(ctx: &ScriptCtx<'_>, qs: Qs, html: Option<String>) -> Option<String> {
    let mut result = html?;
    for i0 in 0..9 {
        let i1 = number_from_bingo_board(ctx, qs, i0);
        let color = if is_selected_bingo_number(ctx, qs, i1) {
            "ff0000"
        } else {
            "ffffff"
        };
        result = result.replace(&format!("<?FontColor{}?>", i0 + 1), color);
        result = result.replace(&format!("<?Cell{}?>", i0 + 1), &i1.to_string());
    }
    Some(result)
}

/// Java `takeHtml(player, qs, num, npcId)`: a number of the second to the fifth pick.
pub(crate) fn take_html(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    qs: Qs,
    num: i32,
    npc_id: i32,
) -> Option<String> {
    let page = |ctx: &ScriptCtx<'_>, n: i32| ctx.get_htm(player, &format!("{npc_id}-{n}.html"));
    let html;
    if !is_selected_bingo_number(ctx, qs, num) {
        select_bingo_number(ctx, qs, num);
        html = match bingo_select_count(ctx, qs) {
            2 => page(ctx, 14),
            3 => page(ctx, 16),
            4 => page(ctx, 18),
            5 => page(ctx, 20),
            _ => None,
        };
    } else {
        html = match bingo_select_count(ctx, qs) {
            1 => page(ctx, 15),
            2 => page(ctx, 17),
            3 => page(ctx, 19),
            4 => page(ctx, 21),
            _ => None,
        };
    }
    fill_board(ctx, qs, html)
}

/// Java `beforeReward(player, qs, num, npcId)`: the sixth pick, the lines decide the reward.
fn before_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, num: i32, npc_id: i32) -> Option<String> {
    if !is_selected_bingo_number(ctx, qs, num) {
        select_bingo_number(ctx, qs, num);
        let i3 = matched_bingo_line_count(ctx, qs);
        let html = if i3 == 3 && bingo_select_count(ctx, qs) == 6 {
            reward(ctx, player, qs, i3);
            ctx.get_htm(player, &format!("{npc_id}-22.html"))
        } else if i3 == 0 && bingo_select_count(ctx, qs) == 6 {
            reward(ctx, player, qs, i3);
            ctx.get_htm(player, &format!("{npc_id}-24.html"))
        } else {
            ctx.get_htm(player, &format!("{npc_id}-23.html"))
        };
        return color_board(ctx, qs, html);
    }
    let html = ctx.get_htm(player, &format!("{npc_id}-25.html"));
    fill_board(ctx, qs, html)
}

/// Java `reward(player, qs, i3)`: three lines or none give a prize, by the price paid (memo 10
/// = 10 medals, 20 = 100 medals).
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, i3: usize) {
    let memo = qs.get_memo_state(ctx);
    if i3 == 3 {
        if memo == 10 {
            let random = ctx.get_random(100);
            let item = if random < 16 {
                SYNTHESIS_COKES
            } else if random < 32 {
                VARNISH_OF_PURITY
            } else if random < 50 {
                CRAFTED_LEATHER
            } else if random < 80 {
                SCRL_OF_ENCH_AM_C
            } else if random < 89 {
                MITHIRL_ALLOY
            } else if random < 98 {
                ORIHARUKON
            } else {
                SCRL_OF_ENCH_WP_C
            };
            ctx.give_items(player, item, 1);
        } else if memo == 20 {
            let random = ctx.get_random(100);
            let item = if random < 50 {
                AQUASTONE_RING
            } else if random < 80 {
                SCRL_OF_ENCH_WP_C
            } else if random < 98 {
                MOONSTONE_EARING
            } else {
                DRAKE_LEATHER_MAIL
            };
            ctx.give_items(player, item, 1);
        }
    } else if i3 == 0 {
        if memo == 10 {
            let random = ctx.get_random(100);
            let item = if random < 50 {
                MOLD_HARDENER
            } else if random < 80 {
                SCRL_OF_ENCH_AM_C
            } else if random < 98 {
                BLACKSMITH_S_FRAME
            } else {
                NECKLACE_OF_MERMAID
            };
            ctx.give_items(player, item, 1);
        } else if memo == 20 {
            let random = ctx.get_random(100);
            if random < 50 {
                ctx.give_items(player, SCRL_OF_ENCH_WP_C, 1);
            } else if random < 80 {
                ctx.give_items(player, GREAT_HELMET, 1);
            } else if random < 98 {
                ctx.give_items(player, DRAKE_LEATHER_BOOTS, 1);
                ctx.give_items(player, BLESSED_GLOVES, 1);
            } else {
                ctx.give_items(player, SAMURAI_LONGSWORD, 1);
            }
        }
    }
}

/// The numbered events both NPCs share; `pages` are their html files (Cliff's and Baxt's names
/// differ in the "no" pages only: 02/03/04/05 are Cliff's alone).
fn bingo_event(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, npc_id: i32, ask: i32) -> Option<String> {
    // (quit page, the 10-medal board, the 100-medal board) differ by NPC prefix only.
    let (quit, ten, hundred, no_medals) = if npc_id == CLIFF {
        ("30182-08.html", "30182-10.html", "30182-12.html", "30182-11.html")
    } else {
        ("30685-08.html", "30685-10.html", "30685-12.html", "30685-11.html")
    };
    match ask {
        3 => Some(
            if npc_id == CLIFF {
                "30182-09a.htm"
            } else {
                "30685-09a.html"
            }
            .to_owned(),
        ),
        6 => {
            qs.exit_quest(ctx, true, false);
            Some(quit.to_owned())
        }
        9 => Some(
            if npc_id == CLIFF {
                "30182-09.html"
            } else {
                "30685-09.html"
            }
            .to_owned(),
        ),
        7 => {
            if ctx.get_quest_items_count(player, Q_IRONGATE_MEDAL) >= 10 {
                ctx.take_items(player, Q_IRONGATE_MEDAL, 10);
                qs.set_memo_state(ctx, 10);
                create_bingo_board(ctx, qs);
                return Some(ten.to_owned());
            }
            Some(no_medals.to_owned())
        }
        8 => {
            if ctx.get_quest_items_count(player, Q_IRONGATE_MEDAL) >= 100 {
                ctx.take_items(player, Q_IRONGATE_MEDAL, 100);
                qs.set_memo_state(ctx, 20);
                create_bingo_board(ctx, qs);
                return Some(hundred.to_owned());
            }
            Some(no_medals.to_owned())
        }
        10..=18 => {
            // Cliff counts (ask - 10) + 1 and Baxt (ask - 9): the same number.
            select_bingo_number(ctx, qs, ask - 9);
            let html = ctx.get_htm(player, &format!("{npc_id}-13.html"));
            fill_board(ctx, qs, html)
        }
        19..=27 => take_html(ctx, player, qs, ask - 18, npc_id),
        55..=63 => before_reward(ctx, player, qs, ask - 54, npc_id),
        _ => None,
    }
}

impl Script for Q00384WarehouseKeepersPastime {
    fn name(&self) -> &'static str {
        "Q00384_WarehouseKeepersPastime"
    }

    fn path(&self) -> &'static str {
        "quests/Q00384_WarehouseKeepersPastime"
    }

    fn id(&self) -> i32 {
        384
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CLIFF]);
        s.add_talk_id(&[CLIFF, WAREHOUSE_CHIEF_BAXT]);
        // Baxt and Cliff are registered too but have no case in Java's switch.
        let mobs: Vec<i32> = [WAREHOUSE_CHIEF_BAXT, CLIFF]
            .into_iter()
            .chain(MEDAL_DROPS.iter().map(|(id, ..)| *id))
            .collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[Q_IRONGATE_MEDAL]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            CLIFF => {
                if qs.is_created(ctx) {
                    if ctx.player_level(player) >= 40 {
                        return Some("30182-01.htm".to_owned());
                    }
                    return Some("30182-04.html".to_owned());
                }
                if ctx.get_quest_items_count(player, Q_IRONGATE_MEDAL) < 10 {
                    return Some("30182-06.html".to_owned());
                }
                Some("30182-07.html".to_owned())
            }
            WAREHOUSE_CHIEF_BAXT => {
                if qs.get_memo_state(ctx) > 0 {
                    if ctx.get_quest_items_count(player, Q_IRONGATE_MEDAL) < 10 {
                        return Some("30685-06.html".to_owned());
                    }
                    return Some("30685-07.html".to_owned());
                }
                Some(htmltext)
            }
            _ => Some(htmltext),
        }
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
        if event.contains(".htm") {
            return Some(event.to_owned());
        }
        let npc = npc?;
        match npc.id {
            CLIFF => {
                if event == "QUEST_ACCEPTED" {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                    qs.set_memo_state(ctx, 384);
                    qs.start_quest(ctx);
                    qs.show_question_mark(ctx, 384);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    return Some("30182-05.htm".to_owned());
                }
                // NOTE(java-quirk): `Integer.parseInt(event)` throws on any other text; here it
                // shows nothing.
                let ask: i32 = event.parse().ok()?;
                match ask {
                    4 => Some("30182-02.htm".to_owned()),
                    5 => Some("30182-03.htm".to_owned()),
                    _ => bingo_event(ctx, player, qs, CLIFF, ask),
                }
            }
            WAREHOUSE_CHIEF_BAXT => {
                let ask: i32 = event.parse().ok()?;
                bingo_event(ctx, player, qs, WAREHOUSE_CHIEF_BAXT, ask)
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = random_player_from_party(ctx, killer, npc) else {
            return;
        };
        let Some(&(_, range, chance)) = MEDAL_DROPS.iter().find(|(id, ..)| *id == npc.id) else {
            return;
        };
        if ctx.get_random(range) < chance {
            ctx.give_item_randomly(member, Some(npc), Q_IRONGATE_MEDAL, 1, 1, 0, 1.0, true);
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

    const NAME: &str = "Q00384_WarehouseKeepersPastime";

    const NPCS: &str = r#"
        <npc id="30182" type="Folk" level="70" name="Cliff"/>
        <npc id="30685" type="Folk" level="70" name="Baxt"/>
        <npc id="20242" type="Monster" level="40" name="Dust Wind"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5964" type="EtcItem" name="Irongate Medal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4041" type="EtcItem" name="Mold Hardener"/>
        <item id="952" type="EtcItem" name="Scroll"/>
        <item id="1892" type="EtcItem" name="Frame"/>
        <item id="917" type="EtcItem" name="Necklace"/>
        <item id="1888" type="EtcItem" name="Cokes"/>
        <item id="1887" type="EtcItem" name="Varnish"/>
        <item id="1894" type="EtcItem" name="Leather"/>
        <item id="1890" type="EtcItem" name="Alloy"/>
        <item id="1893" type="EtcItem" name="Oriharukon"/>
        <item id="951" type="EtcItem" name="Scroll W"/>"#;

    /// A board page with the placeholders and the links of the real pages.
    fn board(file: &str, first: i32) -> String {
        let links: String = (first..first + 9)
            .map(|n| format!(r#"<a action="bypass -h Script {NAME} {n}">{n}</a>"#))
            .collect();
        let cells: String = (1..=9)
            .map(|i| format!("<font color=<?FontColor{i}?>>[<?Cell{i}?>]</font>"))
            .collect();
        format!("<html><body>{file}{links}{cells}</body></html>")
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00384WarehouseKeepersPastime);
        let dir = "data/scripts/quests/Q00384_WarehouseKeepersPastime";
        let mut pages = Pages::of(&Q00384WarehouseKeepersPastime)
            .page("30182-01.htm", &["QUEST_ACCEPTED", "4", "5"])
            .page("30182-05.htm", &[])
            .page("30182-06.html", &[])
            .page("30182-07.html", &["7", "8", "6", "9", "3"])
            .plain(&[
                "30182-04.html",
                "30182-02.htm",
                "30182-03.htm",
                "30182-08.html",
                "30182-11.html",
            ])
            .npcs(&[CLIFF, WAREHOUSE_CHIEF_BAXT]);
        // The 10-medal board page offers the nine first picks; the second-pick pages offer
        // 19..27, the last picks 55..63.
        pages = pages.file(&format!("{dir}/30182-10.html"), &board("30182-10.html", 10));
        pages = pages.file(&format!("{dir}/30182-13.html"), &board("30182-13.html", 19));
        for file in ["14", "15", "16", "17", "18", "19", "21"] {
            pages = pages.file(
                &format!("{dir}/30182-{file}.html"),
                &board(&format!("30182-{file}.html"), 19),
            );
        }
        pages = pages.file(&format!("{dir}/30182-20.html"), &board("30182-20.html", 55));
        for file in ["22", "23", "24", "25"] {
            pages = pages.file(
                &format!("{dir}/30182-{file}.html"),
                &board(&format!("30182-{file}.html"), 55),
            );
        }
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let cliff = t.spawn(CLIFF, 20);
        let baxt = t.spawn(WAREHOUSE_CHIEF_BAXT, 30);
        (t, cliff, baxt)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, cliff, _baxt) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, cliff).contains("30182-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_starts_the_quest_with_memo_384() {
        let (mut t, cliff, _baxt) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, cliff).contains("30182-01.htm"));
        assert!(t.event(1, NAME, "QUEST_ACCEPTED").contains("30182-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "384"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, cliff).contains("30182-06.html"));
    }

    #[test]
    fn medals_buy_a_board_and_six_picks_end_the_game() {
        let (mut t, cliff, _baxt) = setup();
        t.enter(1, 40, vec![item(100, Q_IRONGATE_MEDAL, 10)]);
        t.quest_page(1, cliff);
        t.event(1, NAME, "QUEST_ACCEPTED");
        t.take_writes();
        assert!(t.quest_page(1, cliff).contains("30182-07.html"));
        assert!(t.event(1, NAME, "7").contains("30182-10.html"));
        assert_eq!(t.count(1, Q_IRONGATE_MEDAL), 0);
        let writes = t.take_writes();
        // memoState 384 -> 10, then the two board variables.
        assert_eq!(writes[0], rows::update(NAME, "memoState", "10"));
        assert_eq!(writes.len(), 3);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            let numbers = qs.get(ctx, "numbers").unwrap();
            let mut sorted: Vec<&str> = numbers.split(' ').collect();
            sorted.sort_unstable();
            assert_eq!(sorted, ["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
        });

        // First pick (event 10 = number 1), then picks two to five (19..27), then the sixth.
        let page = t.event(1, NAME, "10");
        assert!(page.contains("30182-13.html"));
        assert!(!page.contains("<?Cell"), "{page}");
        let page = t.event(1, NAME, "20");
        assert!(page.contains("30182-14.html"), "{page}");
        assert!(t.event(1, NAME, "21").contains("30182-16.html"));
        assert!(t.event(1, NAME, "22").contains("30182-18.html"));
        assert!(t.event(1, NAME, "23").contains("30182-20.html"));
        // The sixth pick (60 = number 6) shows the whole board and ends the game.
        let page = t.event(1, NAME, "60");
        assert!(
            ["30182-22.html", "30182-23.html", "30182-24.html"]
                .iter()
                .any(|p| page.contains(p)),
            "{page}"
        );
        assert!(!page.contains("<?"), "{page}");
    }

    #[test]
    fn lines_are_counted_over_rows_columns_and_diagonals() {
        let (mut t, cliff, _baxt) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, cliff);
        t.event(1, NAME, "QUEST_ACCEPTED");
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            for (selected, lines) in [
                ("? ? ? ? ? ? ? ? ?", 0),
                ("1 2 3 ? ? ? ? ? ?", 1),
                ("1 ? ? 4 ? ? 7 ? ?", 1),
                ("1 ? ? ? 5 ? ? ? 9", 1),
                ("? ? 3 ? 5 ? 7 ? ?", 1),
                ("1 2 3 4 5 6 ? ? ?", 2),
                ("1 2 3 4 5 ? 7 ? ?", 3),
                ("1 2 3 4 5 6 7 8 9", 8),
            ] {
                qs.set(ctx, "selected", selected);
                assert_eq!(matched_bingo_line_count(ctx, qs), lines, "{selected}");
            }
        });
    }

    #[test]
    fn a_kill_drops_a_medal_for_a_started_character() {
        let (mut t, cliff, _baxt) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, cliff);
        t.event(1, NAME, "QUEST_ACCEPTED");
        for _ in 0..100 {
            let mob = t.spawn(20242, 60);
            t.kill(1, mob);
            if t.count(1, Q_IRONGATE_MEDAL) > 0 {
                return;
            }
        }
        panic!("no medal after 100 kills");
    }
}
