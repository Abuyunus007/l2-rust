//! Quest 662: A Game of Cards (Klump trades 50 Red Gems for a hand of five cards that he
//! turns face up; the pairs of the hand decide the reward).
// Java: dist/game/data/scripts/quests/Q00662_AGameOfCards/Q00662_AGameOfCards.java
//
// The hand lives in two quest variables: `v1` (cards 1 to 4, two decimal digits each) and
// `ExMemoState` (card 5 in the low two digits, the bit field of the cards turned face up above
// them). The card pages are templates (`FontColorN`, `CellN`) that the script fills in.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLUMP: i32 = 30845;

// Items
const RED_GEM: i32 = 8765;
const ZIGGOS_GEMSTONE: i32 = 8868;

// Misc
const MIN_LEVEL: i32 = 61;
const REQUIRED_CHIP_COUNT: i64 = 50;

/// Java `MONSTERS`: `(npc id, chance)`.
const MONSTERS: [(i32, i32); 38] = [
    (20672, 357), // Trives
    (20673, 357), // Falibati
    (20674, 583), // Doom Knight
    (20677, 435), // Tulben
    (20955, 358), // Ghostly Warrior
    (20958, 283), // Death Agent
    (20959, 455), // Dark Guard
    (20961, 365), // Bloody Knight
    (20962, 348), // Bloody Priest
    (20965, 457), // Chimera Piece
    (20966, 493), // Changed Creation
    (20968, 418), // Nonexistent Man
    (20972, 350), // Shaman of Ancient Times
    (20973, 453), // Forgotten Ancient People
    (21002, 315), // Doom Scout
    (21004, 320), // Dismal Pole
    (21006, 335), // Doom Servant
    (21008, 462), // Doom Archer
    (21010, 397), // Doom Warrior
    (21109, 507), // Hames Orc Scout
    (21112, 552), // Hames Orc Footman
    (21114, 587), // Cursed Guardian
    (21116, 812), // Hames Orc Overlord
    (21278, 483), // Antelope
    (21279, 483), // Antelope
    (21280, 483), // Antelope
    (21286, 515), // Buffalo
    (21287, 515), // Buffalo
    (21288, 515), // Buffalo
    (21508, 493), // Splinter Stakato
    (21510, 527), // Splinter Stakato Soldier
    (21513, 562), // Needle Stakato
    (21515, 598), // Needle Stakato Soldier
    (21520, 458), // Eye of Splendor
    (21526, 552), // Wisdom of Splendor
    (21530, 488), // Victory of Splendor
    (21535, 573), // Signet of Splendor
    (18001, 232), // Blood Queen
];

/// Java `Q00662_AGameOfCards`.
pub struct Q00662AGameOfCards;

/// The fold of a card drawn from 1..70 to the 14 faces (Java repeats it for each card).
fn fold(card: i32) -> i32 {
    if card >= 57 {
        card - 56
    } else if card >= 43 {
        card - 42
    } else if card >= 29 {
        card - 28
    } else if card >= 15 {
        card - 14
    } else {
        card
    }
}

/// Java `setHtml(htmltext, var, regex)`: the face of a card.
fn face(card: i32) -> &'static str {
    match card {
        1 => "!",
        2 => "=",
        3 => "T",
        4 => "V",
        5 => "O",
        6 => "P",
        7 => "S",
        8 => "E",
        9 => "H",
        10 => "A",
        11 => "R",
        12 => "D",
        13 => "I",
        14 => "N",
        _ => "ERROR",
    }
}

/// The five `FontColorN`/`CellN` replacements of the card pages: a card that is not turned (bit
/// `n` of `i9` clear) is a yellow "?", a turned one red with its face.
fn fill_cards(mut htmltext: String, i9: i32, cards: [i32; 5]) -> String {
    for (n, card) in cards.iter().enumerate() {
        let number = n + 1;
        let divisor = 1 << number;
        let turned = i9 % divisor >= divisor / 2;
        let (color, cell) = if turned {
            ("FF6F6F", face(*card))
        } else {
            ("FFFF00", "?")
        };
        htmltext = htmltext.replace(&format!("FontColor{number}"), color);
        htmltext = htmltext.replace(&format!("Cell{number}"), cell);
    }
    htmltext
}

/// Java's `i6` of the finished hand: tens count the pairs (`i1` against the rest, then the
/// rest among themselves), as the decompiled script computes them.
// The last `i8 += 1` assignments are dead, kept as Java has them.
#[allow(unused_assignments)]
fn score(i1: i32, i2: i32, i3: i32, i4: i32, i5: i32) -> i32 {
    let mut i6 = 0;
    let mut i8 = 0;
    if (1..=14).contains(&i1)
        && (1..=14).contains(&i2)
        && (1..=14).contains(&i3)
        && (1..=14).contains(&i4)
        && (1..=14).contains(&i5)
    {
        if i1 == i2 {
            i6 += 10;
            i8 += 8;
        }
        if i1 == i3 {
            i6 += 10;
            i8 += 4;
        }
        if i1 == i4 {
            i6 += 10;
            i8 += 2;
        }
        if i1 == i5 {
            i6 += 10;
            i8 += 1;
        }

        if i6 % 100 < 10 {
            if i8 % 16 < 8 {
                if i8 % 8 < 4 && i2 == i3 {
                    i6 += 10;
                    i8 += 4;
                }
                if i8 % 4 < 2 && i2 == i4 {
                    i6 += 10;
                    i8 += 2;
                }
                if i8 % 2 < 1 && i2 == i5 {
                    i6 += 10;
                    i8 += 1;
                }
            }
        } else if i6 % 10 == 0 && i8 % 16 < 8 {
            if i8 % 8 < 4 && i2 == i3 {
                i6 += 1;
                i8 += 4;
            }
            if i8 % 4 < 2 && i2 == i4 {
                i6 += 1;
                i8 += 2;
            }
            if i8 % 2 < 1 && i2 == i5 {
                i6 += 1;
                i8 += 1;
            }
        }

        if i6 % 100 < 10 {
            if i8 % 8 < 4 {
                if i8 % 4 < 2 && i3 == i4 {
                    i6 += 10;
                    i8 += 2;
                }
                if i8 % 2 < 1 && i3 == i5 {
                    i6 += 10;
                    i8 += 1;
                }
            }
        } else if i6 % 10 == 0 && i8 % 8 < 4 {
            if i8 % 4 < 2 && i3 == i4 {
                i6 += 1;
                i8 += 2;
            }
            if i8 % 2 < 1 && i3 == i5 {
                i6 += 1;
                i8 += 1;
            }
        }

        if i6 % 100 < 10 {
            if i8 % 4 < 2 && i8 % 2 < 1 && i4 == i5 {
                i6 += 10;
                i8 += 1;
            }
        } else if i6 % 10 == 0 && i8 % 4 < 2 && i8 % 2 < 1 && i4 == i5 {
            i6 += 1;
            i8 += 1;
        }
    }
    i6
}

impl Script for Q00662AGameOfCards {
    fn name(&self) -> &'static str {
        "Q00662_AGameOfCards"
    }

    fn path(&self) -> &'static str {
        "quests/Q00662_AGameOfCards"
    }

    fn id(&self) -> i32 {
        662
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLUMP]);
        s.add_talk_id(&[KLUMP]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|m| m.0).collect();
        s.add_kill_id(&monsters);
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
            "30845-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    if qs.is_created(ctx) {
                        qs.start_quest(ctx);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30845-06.html" | "30845-08.html" | "30845-09.html" | "30845-09a.html" | "30845-09b.html"
            | "30845-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "30845-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "return" => {
                htmltext = Some(
                    if ctx.get_quest_items_count(player, RED_GEM) < REQUIRED_CHIP_COUNT {
                        "30845-04.html"
                    } else {
                        "30845-05.html"
                    }
                    .to_owned(),
                );
            }
            "30845-11.html" => {
                if ctx.get_quest_items_count(player, RED_GEM) >= REQUIRED_CHIP_COUNT {
                    let (mut i1, mut i2, mut i3, mut i4, mut i5) = (0, 0, 0, 0, 0);
                    while i1 == i2
                        || i1 == i3
                        || i1 == i4
                        || i1 == i5
                        || i2 == i3
                        || i2 == i4
                        || i2 == i5
                        || i3 == i4
                        || i3 == i5
                        || i4 == i5
                    {
                        i1 = ctx.get_random(70) + 1;
                        i2 = ctx.get_random(70) + 1;
                        i3 = ctx.get_random(70) + 1;
                        i4 = ctx.get_random(70) + 1;
                        i5 = ctx.get_random(70) + 1;
                    }
                    let (i1, i2, i3, i4, i5) = (fold(i1), fold(i2), fold(i3), fold(i4), fold(i5));

                    qs.set_int(ctx, "v1", (i4 * 1_000_000) + (i3 * 10_000) + (i2 * 100) + i1);
                    qs.set_int(ctx, "ExMemoState", i5);
                    ctx.take_items(player, RED_GEM, REQUIRED_CHIP_COUNT);
                    htmltext = Some(event.to_owned());
                }
            }
            "turncard1" | "turncard2" | "turncard3" | "turncard4" | "turncard5" => {
                let cond = qs.get_int(ctx, "v1");
                let i1 = qs.get_int(ctx, "ExMemoState");
                let i5 = i1 % 100;
                let mut i9 = i1 / 100;
                let i1 = cond % 100;
                let i2 = (cond % 10_000) / 100;
                let i3 = (cond % 1_000_000) / 10_000;
                let i4 = (cond % 100_000_000) / 1_000_000;
                // The card number of the event and its bit.
                let n = i32::from(event.as_bytes()[8] - b'0');
                let bit = 1 << (n - 1);
                let divisor = bit * 2;
                if i9 % divisor < bit {
                    i9 += bit;
                }
                if i9 % 32 < 31 {
                    qs.set_int(ctx, "ExMemoState", (i9 * 100) + i5);
                }

                if i9 % 32 < 31 {
                    htmltext = ctx.get_htm(player, "30845-12.html");
                } else if i9 % 32 == 31 {
                    let i6 = score(i1, i2, i3, i4, i5);
                    let finish = |ctx: &mut ScriptCtx<'_>, page: &str| {
                        qs.set_int(ctx, "ExMemoState", 0);
                        qs.set_int(ctx, "v1", 0);
                        ctx.get_htm(player, page)
                    };
                    if i6 == 40 {
                        ctx.reward_items(player, ZIGGOS_GEMSTONE, 43);
                        ctx.reward_items(player, 959, 3);
                        ctx.reward_items(player, 729, 1);
                        htmltext = finish(ctx, "30845-13.html");
                    } else if i6 == 30 {
                        ctx.reward_items(player, 959, 2);
                        ctx.reward_items(player, 951, 2);
                        htmltext = finish(ctx, "30845-14.html");
                    } else if i6 == 21 || i6 == 12 {
                        ctx.reward_items(player, 729, 1);
                        ctx.reward_items(player, 947, 2);
                        ctx.reward_items(player, 955, 1);
                        htmltext = finish(ctx, "30845-15.html");
                    } else if i6 == 20 {
                        ctx.reward_items(player, 951, 2);
                        htmltext = finish(ctx, "30845-16.html");
                    } else if i6 == 11 {
                        ctx.reward_items(player, 951, 1);
                        htmltext = finish(ctx, "30845-17.html");
                    } else if i6 == 10 {
                        ctx.reward_items(player, 956, 2);
                        htmltext = finish(ctx, "30845-18.html");
                    } else if i6 == 0 {
                        htmltext = finish(ctx, "30845-19.html");
                    }
                }

                if let Some(page) = htmltext.take() {
                    htmltext = Some(fill_cards(page, i9, [i1, i2, i3, i4, i5]));
                }
            }
            "playagain" => {
                htmltext = Some(
                    if ctx.get_quest_items_count(player, RED_GEM) < REQUIRED_CHIP_COUNT {
                        "30845-21.html"
                    } else {
                        "30845-20.html"
                    }
                    .to_owned(),
                );
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        match qs.state(ctx) {
            CREATED => {
                htmltext = Some(
                    if ctx.player_level(player) < MIN_LEVEL {
                        "30845-02.html"
                    } else {
                        "30845-01.htm"
                    }
                    .to_owned(),
                );
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(
                        if ctx.get_quest_items_count(player, RED_GEM) < REQUIRED_CHIP_COUNT {
                            "30845-04.html"
                        } else {
                            "30845-05.html"
                        }
                        .to_owned(),
                    );
                } else if qs.get_int(ctx, "ExMemoState") != 0 {
                    let i0 = qs.get_int(ctx, "v1");
                    let i1 = qs.get_int(ctx, "ExMemoState");
                    let i5 = i1 % 100;
                    let i9 = i1 / 100;
                    let i1 = i0 % 100;
                    let i2 = (i0 % 10_000) / 100;
                    let i3 = (i0 % 1_000_000) / 10_000;
                    let i4 = (i0 % 100_000_000) / 1_000_000;
                    // Java: `getHtm(..)` of a missing page would throw on the `replace`.
                    htmltext = ctx
                        .get_htm(player, "30845-11a.html")
                        .map(|page| fill_cards(page, i9, [i1, i2, i3, i4, i5]));
                }
            }
            COMPLETED => htmltext = Some(ctx.get_already_completed_msg(player)),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): the killer is in the list twice (so alone he is always drawn), and a
        // party member is added once more per quest state, the killer included.
        let mut players = vec![killer, killer];
        if ctx.is_in_party(killer) {
            for member in ctx.world().party_members_of(killer) {
                if ctx.get_quest_state(member, false).is_some() {
                    players.push(member);
                }
            }
        }

        let count = i32::try_from(players.len()).unwrap_or(i32::MAX);
        let index = usize::try_from(ctx.get_random(count)).unwrap_or(0);
        let Some(&player) = players.get(index) else {
            return;
        };
        let Some(&(_, chance)) = MONSTERS.iter().find(|m| m.0 == npc.id) else {
            return;
        };
        // NOTE(java-quirk): `chance` (0..1000) is compared to a roll of 1000 and then passed as
        // the 0..1 drop chance of `giveItemRandomly`, where it always succeeds.
        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
            && chance < ctx.get_random(1000)
            && ctx.get_quest_state(player, false).is_some()
        {
            ctx.give_item_randomly(player, Some(npc), RED_GEM, 1, 1, 0, f64::from(chance), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00662_AGameOfCards";

    const NPCS: &str = r#"
        <npc id="30845" type="Folk" level="70" name="Klump"/>
        <npc id="20672" type="Monster" level="60" name="Trives"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8765" type="EtcItem" name="Red Gem"><set name="is_stackable" val="true"/></item>
        <item id="8868" type="EtcItem" name="Ziggo's Gemstone"><set name="is_stackable" val="true"/></item>
        <item id="959" type="EtcItem" name="Scroll: Enchant Weapon (B)"><set name="is_stackable" val="true"/></item>
        <item id="729" type="EtcItem" name="Scroll: Enchant Weapon (A)"><set name="is_stackable" val="true"/></item>
        <item id="951" type="EtcItem" name="Scroll: Enchant Weapon (C)"><set name="is_stackable" val="true"/></item>
        <item id="947" type="EtcItem" name="Scroll: Enchant Weapon (B)"><set name="is_stackable" val="true"/></item>
        <item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D)"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"><set name="is_stackable" val="true"/></item>"#;

    /// The card page of the datapack, cut down to its placeholders and links.
    const CARD_PAGE: &str = r#"<html><body>cards
        <font color="FontColor1">Cell1</font><font color="FontColor2">Cell2</font>
        <font color="FontColor3">Cell3</font><font color="FontColor4">Cell4</font>
        <font color="FontColor5">Cell5</font>
        <a action="bypass -h Script Q00662_AGameOfCards turncard1">1</a>
        <a action="bypass -h Script Q00662_AGameOfCards turncard2">2</a>
        <a action="bypass -h Script Q00662_AGameOfCards turncard3">3</a>
        <a action="bypass -h Script Q00662_AGameOfCards turncard4">4</a>
        <a action="bypass -h Script Q00662_AGameOfCards turncard5">5</a></body></html>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00662AGameOfCards);
        let dir = "data/scripts/quests/Q00662_AGameOfCards";
        let mut pages = Pages::of(&Q00662AGameOfCards)
            .page("30845-01.htm", &["30845-03.htm"])
            .plain(&[
                "30845-02.html",
                "30845-03.htm",
                "30845-04.html",
                "30845-06.html",
                "30845-07.html",
            ])
            .page("30845-05.html", &["30845-10.html", "30845-08.html"])
            .page("30845-10.html", &["30845-11.html"])
            .page("30845-20.html", &["30845-11.html"])
            .page(
                "30845-11.html",
                &["turncard1", "turncard2", "turncard3", "turncard4", "turncard5"],
            )
            .plain(&["30845-08.html", "30845-21.html"])
            .page("30845-04.html", &["30845-07.html"])
            .npcs(&[KLUMP]);
        for page in [
            "30845-11a.html",
            "30845-12.html",
            "30845-13.html",
            "30845-14.html",
            "30845-15.html",
            "30845-16.html",
            "30845-17.html",
            "30845-18.html",
            "30845-19.html",
        ] {
            pages = pages.file(&format!("{dir}/{page}"), &CARD_PAGE.replace("cards", page));
        }
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klump = t.spawn(KLUMP, 20);
        (t, klump)
    }

    fn set_hand(t: &mut ScriptTest, hand: [i32; 5], turned: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(
                ctx,
                "v1",
                hand[3] * 1_000_000 + hand[2] * 10_000 + hand[1] * 100 + hand[0],
            );
            qs.set_int(ctx, "ExMemoState", turned * 100 + hand[4]);
        });
    }

    #[test]
    fn level_61_is_needed() {
        let (mut t, klump) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, klump).contains("30845-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_gems_deal_a_hand_and_four_of_a_kind_pays() {
        let (mut t, klump) = setup();
        t.enter(1, 61, vec![item(100, RED_GEM, 50)]);
        assert!(t.quest_page(1, klump).contains("30845-01.htm"));
        assert!(t.event(1, NAME, "30845-03.htm").contains("30845-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, klump).contains("30845-05.html"));
        t.event(1, NAME, "30845-10.html");
        assert!(t.event(1, NAME, "30845-11.html").contains("30845-11.html"));
        assert_eq!(t.count(1, RED_GEM), 0);
        let writes = t.take_writes();
        assert_eq!(writes.len(), 2, "{writes:?}");

        // A hand set up by the test: all five cards equal pay the jackpot.
        set_hand(&mut t, [5, 5, 5, 5, 5], 0);
        t.take_writes();
        let first = t.event(1, NAME, "turncard1");
        assert!(first.contains("30845-12.html"), "{first}");
        assert!(first.contains("FF6F6F") && first.contains(">O<"), "{first}");
        t.event(1, NAME, "turncard2");
        t.event(1, NAME, "turncard3");
        t.event(1, NAME, "turncard4");
        let last = t.event(1, NAME, "turncard5");
        assert!(last.contains("30845-13.html"), "{last}");
        assert_eq!(
            (t.count(1, ZIGGOS_GEMSTONE), t.count(1, 959), t.count(1, 729)),
            (43, 3, 1)
        );
        let writes = t.take_writes();
        assert_eq!(
            writes[writes.len() - 2..],
            [
                rows::update(NAME, "ExMemoState", "0"),
                rows::update(NAME, "v1", "0")
            ]
        );
    }

    #[test]
    fn a_hand_without_a_pair_pays_nothing_and_a_pair_pays_armor_scrolls() {
        let (mut t, klump) = setup();
        t.enter(1, 61, vec![item(100, RED_GEM, 50)]);
        t.quest_page(1, klump);
        t.event(1, NAME, "30845-03.htm");
        t.quest_page(1, klump);
        t.event(1, NAME, "30845-10.html");
        t.event(1, NAME, "30845-11.html");
        // Nothing equal: page 19, no reward.
        set_hand(&mut t, [1, 2, 3, 4, 5], 0b01111);
        t.take_writes();
        assert!(t.event(1, NAME, "turncard5").contains("30845-19.html"));
        // One pair (cards 1 and 2): the two armor scrolls.
        set_hand(&mut t, [7, 7, 3, 4, 5], 0b01111);
        assert!(t.event(1, NAME, "turncard5").contains("30845-18.html"));
        assert_eq!(t.count(1, 956), 2);
    }

    #[test]
    fn the_interrupted_hand_is_shown_when_klump_is_talked_to() {
        // NOTE: the cond never leaves 1 in Java either; the test moves it to reach the branch.
        let (mut t, klump) = setup();
        t.enter(1, 61, vec![]);
        t.quest_page(1, klump);
        t.event(1, NAME, "30845-03.htm");
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        set_hand(&mut t, [9, 9, 9, 9, 9], 0b00001);
        let page = t.quest_page(1, klump);
        assert!(page.contains("30845-11a.html"), "{page}");
        // Card 1 is turned (red, its face "H"), the others are still "?".
        assert!(page.contains(r#"<font color="FF6F6F">H</font>"#), "{page}");
        assert!(page.contains(r#"<font color="FFFF00">?</font>"#), "{page}");
    }

    #[test]
    fn the_monsters_drop_red_gems() {
        let (mut t, klump) = setup();
        t.enter(1, 61, vec![]);
        t.quest_page(1, klump);
        t.event(1, NAME, "30845-03.htm");
        let mut kills = 0;
        while t.count(1, RED_GEM) < 2 {
            let mob = t.spawn(20_672, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 50, "no red gems");
        }
        assert!(t.count(1, RED_GEM) >= 2);
    }
}
