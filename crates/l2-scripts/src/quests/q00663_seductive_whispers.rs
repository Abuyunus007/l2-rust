//! Quest 663: Seductive Whispers (Wilbert trades spirit beads for rounds of a card duel; every
//! win in a row raises the prize).
// Java: dist/game/data/scripts/quests/Q00663_SeductiveWhispers/Q00663_SeductiveWhispers.java
//
// The state is `memoState`: 1..4 collecting beads, `n5` a duel is to start (card 1 pending),
// `n6` Wilbert's card is on the table, `n7` the duel is won; `n` = wins so far (tens digit).
// 1005 / 1006 are the same for the one-bead side game.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WILBERT: i32 = 30846;

// Misc
const MIN_LEVEL: i32 = 50;

// Quest items
const SPIRIT_BEAD: i32 = 8766;

/// Java `ItemHolder`: `(item id, count)`.
type Holder = (i32, i64);

// Rewards
const SCROLL_ENCHANT_WEAPON_A_GRADE: Holder = (729, 1);
const SCROLL_ENCHANT_ARMOR_A_GRADE: Holder = (730, 2);
const SCROLL_ENCHANT_WEAPON_B_GRADE: Holder = (947, 2);
const SCROLL_ENCHANT_ARMOR_B_GRADE: Holder = (948, 2);
const SCROLL_ENCHANT_WEAPON_C_GRADE: Holder = (951, 1);
const SCROLL_ENCHANT_WEAPON_D_GRADE: Holder = (955, 1);

/// The 60% weapon recipes, `getRandom(18) + 1` picks the entry (1 = first).
const RECIPES: [Holder; 18] = [
    (4963, 1), // RECIPE_GREAT_SWORD_60
    (4964, 1), // RECIPE_HEAVY_WAR_AXE_60
    (4965, 1), // RECIPE_SPRITES_STAFF_60
    (4966, 1), // RECIPE_KESHANBERK_60
    (4967, 1), // RECIPE_SWORD_OF_VALHALLA_60
    (4968, 1), // RECIPE_KRIS_60
    (4969, 1), // RECIPE_HELL_KNIFE_60
    (4970, 1), // RECIPE_ARTHRO_NAIL_60
    (4971, 1), // RECIPE_DARK_ELVEN_LONG_BOW_60
    (4972, 1), // RECIPE_GREAT_AXE_60
    (5000, 1), // RECIPE_SWORD_OF_DAMASCUS_60
    (5001, 1), // RECIPE_LANCE_60
    (5002, 1), // RECIPE_DEADMANS_GLORY_60
    (5003, 1), // RECIPE_ART_OF_BATTLE_AXE_60
    (5004, 1), // RECIPE_TAFF_OF_EVIL_SPIRITS_60
    (5005, 1), // RECIPE_DEMONS_DAGGER_60
    (5006, 1), // RECIPE_BELLION_CESTUS_60
    (5007, 1), // RECIPE_BOW_OF_PERIL_60
];

/// The weapon parts, `getRandom(18) + 1` picks the entry (1 = first).
const PARTS: [Holder; 18] = [
    (4104, 12), // GREAT_SWORD_BLADE
    (4113, 12), // GREAT_AXE_HEAD
    (4112, 12), // DARK_ELVEN_LONGBOW_SHAFT
    (4108, 12), // SWORD_OF_VALHALLA_BLADE
    (4111, 12), // ARTHRO_NAIL_BLADE
    // NOTE(java-quirk): SPRITES_STAFF_HEAD has the id of the great sword blade (4104).
    (4104, 12), // SPRITES_STAFF_HEAD
    (4109, 12), // KRIS_EDGE
    (4107, 12), // KESHANBERK_BLADE
    (4105, 12), // HEAVY_WAR_AXE_HEAD
    (4110, 12), // HELL_KNIFE_EDGE
    (4114, 13), // SWORD_OF_DAMASCUS_BLADE
    (4115, 13), // LANCE_BLADE
    (4120, 13), // BELLION_CESTUS_EDGE
    (4118, 13), // EVIL_SPIRIT_HEAD
    (4116, 13), // DEADMANS_GLORY_STONE
    (4117, 13), // ART_OF_BATTLE_AXE_BLADE
    (4119, 13), // DEMONS_DAGGER_EDGE
    (4121, 13), // BOW_OF_PERIL_SHAFT
];

// Monsters
const SPITEFUL_SOUL_LEADER: i32 = 20974;
const SPITEFUL_SOUL_LEADER_CHANCE: i32 = 100;
/// Java `MONSTERS`: `(npc id, chance)`.
// NOTE(java-quirk): Java puts 20674 twice (807, then 537); the later value wins.
const MONSTERS: [(i32, i32); 25] = [
    (20674, 537),
    (20678, 372),
    (20954, 460),
    (20956, 540),
    (20957, 565),
    (20958, 425),
    (20959, 682),
    (20960, 372),
    (20961, 547),
    (20962, 522),
    (20963, 498),
    (20975, 975),
    (20976, 825),
    (20996, 385),
    (20997, 342),
    (20998, 377),
    (20999, 450),
    (21000, 395),
    (21001, 535),
    (21002, 472),
    (21006, 502),
    (21007, 540),
    (21008, 692),
    (21009, 740),
    (21010, 595),
];

/// Java `Q00663_SeductiveWhispers`.
pub struct Q00663SeductiveWhispers;

fn give(ctx: &mut ScriptCtx<'_>, player: ObjectId, holder: Holder) {
    ctx.give_items(player, holder.0, holder.1);
}

/// Java `getHtml(player, htmlName, card1pic, card2pic, winCount, card1)`. A missing page is a
/// `NullPointerException` in Java, so `None` here.
fn get_html(
    ctx: &ScriptCtx<'_>,
    player: ObjectId,
    html_name: &str,
    card1pic: i32,
    card2pic: i32,
    win_count: i32,
    card1: i32,
) -> Option<String> {
    let mut html = ctx.get_htm(player, html_name)?;
    html = html.replace("<?card1pic?>", &card1pic.to_string());
    html = html.replace("<?card2pic?>", &card2pic.to_string());
    html = html.replace("<?name?>", &ctx.player_name(player));
    if win_count >= 0 {
        html = html.replace("<?wincount?>", &win_count.to_string());
    }
    if card1 >= 0 {
        html = html.replace("<?card1?>", &card1.to_string());
    }
    Some(html)
}

/// The two-card comparison of the duels: Wilbert's card (`rdn1`, `rdn2`) against the player's
/// card 1 (`i2`, `i1`).
fn wins(rdn1: i32, rdn2: i32, i2: i32, i1: i32) -> bool {
    if rdn1 == i2 {
        let i3 = rdn2 + i1;
        i3 % 5 == 0 && i3 != 10
    } else {
        rdn2 == 5 || i1 == 5
    }
}

impl Script for Q00663SeductiveWhispers {
    fn name(&self) -> &'static str {
        "Q00663_SeductiveWhispers"
    }

    fn path(&self) -> &'static str {
        "quests/Q00663_SeductiveWhispers"
    }

    fn id(&self) -> i32 {
        663
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WILBERT]);
        s.add_talk_id(&[WILBERT]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|m| m.0).collect();
        s.add_kill_id(&monsters);
        s.add_kill_id(&[SPITEFUL_SOUL_LEADER]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = Some(
                if ctx.player_level(player) < MIN_LEVEL {
                    "30846-02.html"
                } else {
                    "30846-01.htm"
                }
                .to_owned(),
            );
        } else if state == STARTED {
            let memo = qs.get_memo_state(ctx);
            if (1..4).contains(&memo) {
                htmltext = Some(
                    if ctx.has_quest_items(player, SPIRIT_BEAD) {
                        "30846-05.html"
                    } else {
                        "30846-04.html"
                    }
                    .to_owned(),
                );
            }

            if memo / 1000 == 0 {
                match memo % 10 {
                    4 => htmltext = Some("30846-05a.html".to_owned()),
                    5 => htmltext = Some("30846-11.html".to_owned()),
                    6 => htmltext = Some("30846-15.html".to_owned()),
                    7 => {
                        if (memo % 100) / 10 >= 7 {
                            qs.set_memo_state(ctx, 1);
                            ctx.give_adena(player, 2_384_000, true);
                            give(ctx, player, SCROLL_ENCHANT_WEAPON_A_GRADE);
                            give(ctx, player, SCROLL_ENCHANT_ARMOR_A_GRADE);
                            htmltext = Some("30846-17.html".to_owned());
                        } else {
                            let win_count = (memo / 10) + 1;
                            // NOTE(java-quirk): the page is `30846-16.htm` in the datapack but Java asks
                            // for `30846-16.html`, which is not there: `getHtm` gives null and the
                            // `replace` throws, so Wilbert shows nothing here.
                            htmltext = get_html(ctx, player, "30846-16.html", 0, 0, win_count, 0);
                        }
                    }
                    _ => {}
                }
            } else if qs.is_memo_state(ctx, 1005) {
                htmltext = Some("30846-23.html".to_owned());
            } else if qs.is_memo_state(ctx, 1006) {
                htmltext = Some("30846-26.html".to_owned());
            }
        }
        htmltext
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        match event {
            "30846-01a.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = Some(event.to_owned());
                }
                return htmltext;
            }
            "30846-06.html" | "30846-07.html" | "30846-08.html" => {
                return Some(event.to_owned());
            }
            _ => {}
        }
        // NOTE(java-quirk): Java does not null-check the quest state here: without one every other
        // event is a NullPointerException (no page).
        let qs = qs?;
        match event {
            "30846-03.htm" => {
                if qs.is_created(ctx) && ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30846-09.html" => {
                if qs.is_started(ctx) && qs.get_memo_state(ctx) % 10 <= 4 {
                    let memo_state = qs.get_memo_state(ctx) / 10;
                    if memo_state < 1 {
                        if ctx.get_quest_items_count(player, SPIRIT_BEAD) >= 50 {
                            ctx.take_items(player, SPIRIT_BEAD, 50);
                            qs.set_memo_state(ctx, 5);
                            qs.set_memo_state_ex(ctx, 1, 0);
                            htmltext = Some(event.to_owned());
                        } else {
                            htmltext = Some("30846-10.html".to_owned());
                        }
                    } else {
                        qs.set_memo_state(ctx, (memo_state * 10) + 5);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = Some("30846-09a.html".to_owned());
                    }
                }
            }
            "30846-14.html" => {
                if qs.is_started(ctx)
                    && qs.get_memo_state(ctx) % 10 == 5
                    && qs.get_memo_state(ctx) / 1000 == 0
                {
                    let card1pic = qs.get_memo_state_ex(ctx, 1).max(0);
                    let i1 = card1pic % 10;
                    let i2 = (card1pic - i1) / 10;
                    let rdn1 = ctx.get_random(2) + 1;
                    let rdn2 = ctx.get_random(5) + 1;
                    let win_count = (qs.get_memo_state(ctx) / 10) + 1;
                    let card2pic = (rdn1 * 10) + rdn2;
                    // NOTE(java-quirk): the first draw ("rdn1 == i2") and the second ("rdn1 != i2") have the
                    // same two outcomes in Java, only the winning test differs.
                    if wins(rdn1, rdn2, i2, i1) {
                        if (qs.get_memo_state(ctx) % 100) / 10 >= 7 {
                            ctx.give_adena(player, 2_384_000, true);
                            give(ctx, player, SCROLL_ENCHANT_WEAPON_A_GRADE);
                            give(ctx, player, SCROLL_ENCHANT_ARMOR_A_GRADE);
                            qs.set_memo_state(ctx, 4);
                            htmltext =
                                get_html(ctx, player, "30846-14.html", card1pic, card2pic, win_count, -1);
                        } else {
                            qs.set_memo_state(ctx, ((qs.get_memo_state(ctx) / 10) * 10) + 7);
                            htmltext =
                                get_html(ctx, player, "30846-13.html", card1pic, card2pic, win_count, -1);
                        }
                    } else {
                        qs.set_memo_state(ctx, ((qs.get_memo_state(ctx) / 10) * 10) + 6);
                        qs.set_memo_state_ex(ctx, 1, card2pic);
                        htmltext = get_html(ctx, player, "30846-12.html", card1pic, card2pic, win_count, -1);
                    }
                }
            }
            "30846-19.html" => {
                if qs.is_started(ctx)
                    && qs.get_memo_state(ctx) % 10 == 6
                    && qs.get_memo_state(ctx) / 1000 == 0
                {
                    let card1pic = qs.get_memo_state_ex(ctx, 1).max(0);
                    let i1 = card1pic % 10;
                    let i2 = (card1pic - i1) / 10;
                    let rnd1 = ctx.get_random(2) + 1;
                    let rnd2 = ctx.get_random(5) + 1;
                    let card2pic = (rnd1 * 10) + rnd2;
                    if wins(rnd1, rnd2, i2, i1) {
                        qs.set_memo_state(ctx, 1);
                        // NOTE(java-quirk): only the "rnd1 == i2" win also clears the card.
                        if rnd1 == i2 {
                            qs.set_memo_state_ex(ctx, 1, 0);
                        }
                        htmltext = get_html(ctx, player, "30846-19.html", card1pic, card2pic, -1, -1);
                    } else {
                        qs.set_memo_state(ctx, ((qs.get_memo_state(ctx) / 10) * 10) + 5);
                        qs.set_memo_state_ex(ctx, 1, card2pic);
                        htmltext = get_html(ctx, player, "30846-18.html", card1pic, card2pic, -1, -1);
                    }
                }
            }
            "30846-20.html" => {
                if qs.is_started(ctx)
                    && qs.get_memo_state(ctx) % 10 == 7
                    && qs.get_memo_state(ctx) / 1000 == 0
                {
                    qs.set_memo_state(ctx, (((qs.get_memo_state(ctx) / 10) + 1) * 10) + 4);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "30846-21.html" => {
                if qs.is_started(ctx)
                    && qs.get_memo_state(ctx) % 10 == 7
                    && qs.get_memo_state(ctx) / 1000 == 0
                {
                    let i0 = qs.get_memo_state(ctx) / 10;
                    if i0 == 0 {
                        ctx.give_adena(player, 40_000, true);
                    } else if i0 == 1 {
                        ctx.give_adena(player, 80_000, true);
                    } else if i0 == 2 {
                        ctx.give_adena(player, 110_000, true);
                        give(ctx, player, SCROLL_ENCHANT_WEAPON_D_GRADE);
                    } else if i0 == 3 {
                        ctx.give_adena(player, 199_000, true);
                        give(ctx, player, SCROLL_ENCHANT_WEAPON_C_GRADE);
                    } else if i0 == 4 {
                        ctx.give_adena(player, 388_000, true);
                        let rdn = ctx.get_random(18);
                        give(ctx, player, RECIPES[usize::try_from(rdn).unwrap_or(0)]);
                    } else if i0 == 5 {
                        ctx.give_adena(player, 675_000, true);
                        let rnd = ctx.get_random(18);
                        give(ctx, player, PARTS[usize::try_from(rnd).unwrap_or(0)]);
                    } else if i0 == 6 {
                        ctx.give_adena(player, 1_284_000, true);
                        give(ctx, player, SCROLL_ENCHANT_WEAPON_B_GRADE);
                        give(ctx, player, SCROLL_ENCHANT_ARMOR_B_GRADE);
                    }

                    qs.set_memo_state(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "30846-21a.html" => {
                if qs.is_started(ctx) && qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30846-22.html" => {
                if qs.is_started(ctx) && qs.get_memo_state(ctx) % 10 == 1 {
                    if ctx.get_quest_items_count(player, SPIRIT_BEAD) >= 1 {
                        ctx.take_items(player, SPIRIT_BEAD, 1);
                        qs.set_memo_state(ctx, 1005);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30846-22a.html".to_owned());
                    }
                }
            }
            "30846-25.html" => {
                if qs.is_started(ctx) && qs.is_memo_state(ctx, 1005) {
                    let card1pic = qs.get_memo_state_ex(ctx, 1).max(0);
                    let card1 = card1pic % 10;
                    let i2 = (card1pic - card1) / 10;
                    let rnd1 = ctx.get_random(2) + 1;
                    let rnd2 = ctx.get_random(5) + 1;
                    let card2pic = (rnd1 * 10) + rnd2;
                    if wins(rnd1, rnd2, i2, card1) {
                        qs.set_memo_state(ctx, 1);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        ctx.give_adena(player, 800, true);
                        // NOTE(java-quirk): only the "rnd1 == i2" win fills in `<?card1?>`.
                        let shown_card1 = if rnd1 == i2 { card1 } else { -1 };
                        htmltext =
                            get_html(ctx, player, "30846-25.html", card1pic, card2pic, -1, shown_card1);
                    } else {
                        qs.set_memo_state(ctx, 1006);
                        qs.set_memo_state_ex(ctx, 1, card2pic);
                        htmltext = get_html(ctx, player, "30846-24.html", card1pic, card2pic, -1, -1);
                    }
                }
            }
            "30846-29.html" => {
                if qs.is_started(ctx) && qs.is_memo_state(ctx, 1006) {
                    let card1pic = qs.get_memo_state_ex(ctx, 1).max(0);
                    let i1 = card1pic % 10;
                    let i2 = (card1pic - i1) / 10;
                    let rnd1 = ctx.get_random(2) + 1;
                    let rnd2 = ctx.get_random(5) + 1;
                    let card2pic = (rnd1 * 10) + rnd2;
                    if wins(rnd1, rnd2, i2, i1) {
                        qs.set_memo_state(ctx, 1);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = get_html(ctx, player, "30846-29.html", card1pic, card2pic, 0, -1);
                    } else {
                        qs.set_memo_state(ctx, 1005);
                        qs.set_memo_state_ex(ctx, 1, card2pic);
                        htmltext = get_html(ctx, player, "30846-28.html", card1pic, card2pic, 0, -1);
                    }
                }
            }
            "30846-30.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30846-31.html" | "30846-32.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let collecting = |ctx: &mut ScriptCtx<'_>, player: ObjectId| {
            ctx.get_quest_state(player, false).is_some_and(|qs| {
                qs.is_started(ctx) && qs.get_memo_state(ctx) >= 1 && qs.get_memo_state(ctx) <= 4
            })
        };
        let mut players = Vec::new();
        if collecting(ctx, killer) {
            players.push(killer);
            players.push(killer);
        }
        if ctx.is_in_party(killer) {
            for member in ctx.world().party_members_of(killer) {
                if collecting(ctx, member) {
                    players.push(member);
                }
            }
        }

        if players.is_empty() {
            return;
        }
        let count = i32::try_from(players.len()).unwrap_or(i32::MAX);
        let index = usize::try_from(ctx.get_random(count)).unwrap_or(0);
        let Some(&rewarded_player) = players.get(index) else {
            return;
        };
        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, rewarded_player, false) {
            let rnd = ctx.get_random(1000);
            if npc.id == SPITEFUL_SOUL_LEADER {
                if rnd <= SPITEFUL_SOUL_LEADER_CHANCE {
                    ctx.give_items(rewarded_player, SPIRIT_BEAD, 2);
                } else {
                    ctx.give_items(rewarded_player, SPIRIT_BEAD, 1);
                }
            } else if MONSTERS.iter().find(|m| m.0 == npc.id).is_some_and(|m| rnd < m.1) {
                ctx.give_items(rewarded_player, SPIRIT_BEAD, 1);
                ctx.play_sound(rewarded_player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00663_SeductiveWhispers";

    const NPCS: &str = r#"
        <npc id="30846" type="Folk" level="70" name="Wilbert"/>
        <npc id="20974" type="Monster" level="60" name="Spiteful Soul Leader"/>
        <npc id="20674" type="Monster" level="60" name="Doom Knight"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8766" type="EtcItem" name="Spirit Bead"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D)"><set name="is_stackable" val="true"/></item>
        <item id="729" type="EtcItem" name="Scroll: Enchant Weapon (A)"><set name="is_stackable" val="true"/></item>
        <item id="730" type="EtcItem" name="Scroll: Enchant Armor (A)"><set name="is_stackable" val="true"/></item>"#;

    const CARDS: &str =
        "<html><body>win <?wincount?> mine <?card1pic?> his <?card2pic?> <?name?></body></html>";

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00663SeductiveWhispers);
        let dir = "data/scripts/quests/Q00663_SeductiveWhispers";
        let mut pages = Pages::of(&Q00663SeductiveWhispers)
            .page("30846-01.htm", &["30846-01a.htm"])
            .page("30846-01a.htm", &["30846-03.htm"])
            .page("30846-03.htm", &["30846-06.html", "30846-07.html"])
            .page("30846-05.html", &["30846-09.html", "30846-22.html"])
            .page("30846-09.html", &["30846-14.html"])
            .page("30846-11.html", &["30846-14.html"])
            .plain(&["30846-02.html", "30846-04.html", "30846-06.html", "30846-07.html"])
            .npcs(&[WILBERT]);
        for page in [
            "30846-12.html",
            "30846-13.html",
            "30846-14.html",
            "30846-18.html",
            "30846-19.html",
        ] {
            pages = pages.file(&format!("{dir}/{page}"), &CARDS.replace("win", page));
        }
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wilbert = t.spawn(WILBERT, 20);
        (t, wilbert)
    }

    /// An event the way the quest window would send it (the page gating of `Pages` skipped).
    fn event(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00663SeductiveWhispers.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn memo(t: &mut ScriptTest) -> i32 {
        let mut out = 0;
        t.world.run_as_script(NAME, |ctx| {
            out = ctx.get_quest_state(Oid(1), false).unwrap().get_memo_state(ctx);
        });
        out
    }

    fn set_memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), false)
                .unwrap()
                .set_memo_state(ctx, value);
        });
    }

    fn start(t: &mut ScriptTest, wilbert: Oid) {
        t.enter(1, 50, vec![item(100, SPIRIT_BEAD, 60)]);
        assert!(t.quest_page(1, wilbert).contains("30846-01.htm"));
        t.event(1, NAME, "30846-01a.htm");
        assert!(t.event(1, NAME, "30846-03.htm").contains("30846-03.htm"));
    }

    #[test]
    fn level_50_is_needed() {
        let (mut t, wilbert) = setup();
        t.enter(1, 49, vec![]);
        assert!(t.quest_page(1, wilbert).contains("30846-02.html"));
        // Not started below the level: the "no quest" message is the answer.
        let answer = event(&mut t, "30846-03.htm").unwrap();
        assert_ne!(answer, "30846-03.htm");
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_starts_and_fifty_beads_open_the_duel() {
        let (mut t, wilbert) = setup();
        start(&mut t, wilbert);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::insert(NAME, "cond", "1"));
        assert_eq!(writes[2], rows::insert(NAME, "memoState", "1"));
        assert!(t.quest_page(1, wilbert).contains("30846-05.html"));
        assert!(t.event(1, NAME, "30846-09.html").contains("30846-09.html"));
        assert_eq!(t.count(1, SPIRIT_BEAD), 10);
        assert_eq!(memo(&mut t), 5);
    }

    #[test]
    fn a_duel_ends_won_or_lost_and_a_won_one_pays_by_the_wins() {
        let (mut t, wilbert) = setup();
        start(&mut t, wilbert);
        t.quest_page(1, wilbert);
        t.event(1, NAME, "30846-09.html");
        // Play rounds until the duel is decided (7: won, 1: lost), with the seeded RNG.
        let mut rounds = 0;
        let mut last = memo(&mut t);
        while last % 10 != 7 && last != 1 {
            rounds += 1;
            assert!(rounds < 60, "the duel never ended");
            let ev = if last % 10 == 6 {
                "30846-19.html"
            } else {
                "30846-14.html"
            };
            assert!(event(&mut t, ev).is_some());
            last = memo(&mut t);
        }
        if last == 1 {
            assert_eq!(t.count(1, 57), 0);
        } else {
            assert_eq!(event(&mut t, "30846-21.html").as_deref(), Some("30846-21.html"));
            assert_eq!(t.count(1, 57), 40_000);
            assert_eq!(memo(&mut t), 1);
        }
    }

    #[test]
    fn prizes_grow_with_the_win_count() {
        let (mut t, wilbert) = setup();
        start(&mut t, wilbert);
        // Two wins: 110000 adena and a D scroll.
        set_memo(&mut t, 27);
        assert_eq!(event(&mut t, "30846-21.html").as_deref(), Some("30846-21.html"));
        assert_eq!((t.count(1, 57), t.count(1, 955)), (110_000, 1));
        assert_eq!(memo(&mut t), 1);
        // Continue the game from a won duel: one more win, back to collecting beads.
        set_memo(&mut t, 17);
        assert_eq!(event(&mut t, "30846-20.html").as_deref(), Some("30846-20.html"));
        assert_eq!(memo(&mut t), 24);
        // Seven wins in a row pay the great prize when the talk shows it.
        set_memo(&mut t, 77);
        t.quest_page(1, wilbert);
        assert_eq!(t.count(1, 729), 1);
        assert_eq!(t.count(1, 730), 2);
        assert_eq!(memo(&mut t), 1);
    }

    #[test]
    fn the_bead_side_game_and_leaving() {
        let (mut t, wilbert) = setup();
        start(&mut t, wilbert);
        assert_eq!(event(&mut t, "30846-22.html").as_deref(), Some("30846-22.html"));
        assert_eq!(memo(&mut t), 1005);
        assert_eq!(t.count(1, SPIRIT_BEAD), 59);
        let mut rounds = 0;
        while memo(&mut t) != 1 {
            rounds += 1;
            assert!(rounds < 60);
            let ev = if memo(&mut t) == 1006 {
                "30846-29.html"
            } else {
                "30846-25.html"
            };
            event(&mut t, ev);
        }
        assert_eq!(event(&mut t, "30846-30.html").as_deref(), Some("30846-30.html"));
        t.take_writes();
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn beads_drop_for_the_killer_collecting() {
        let (mut t, wilbert) = setup();
        start(&mut t, wilbert);
        let before = t.count(1, SPIRIT_BEAD);
        let mut kills = 0;
        while t.count(1, SPIRIT_BEAD) == before {
            let mob = t.spawn(SPITEFUL_SOUL_LEADER, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 5);
        }
        assert!(t.count(1, SPIRIT_BEAD) >= before + 1);
    }
}
