//! Quest 422: Repent Your Sins (Black Judge; the penitent's manacles and the PK kills to repent).
// Java: dist/game/data/scripts/quests/Q00422_RepentYourSins/Q00422_RepentYourSins.java
//
// Pattern: the quest walks through memo states (the step of the quest, `memoState`) and a memo
// value of its own (the level of the manacles' owner, `memoStateEx` 1). Each step is a talk with
// one NPC, and the kills of the step's monster give the quest items. The end is a roll of the
// PK kills against the manacles' enchant level.

use l2_util::ObjectId;
use l2_world::script::{Qs, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BLACKSMITH_PUSHKIN: i32 = 30300;
const PIOTUR: i32 = 30597;
const ELDER_CASIAN: i32 = 30612;
const KATARI: i32 = 30668;
const MAGISTER_JOAN: i32 = 30718;
const BLACK_JUDGE: i32 = 30981;

// Items
const RATMAN_SCAVENGERS_SKULL: i32 = 4326;
const TUREK_WAR_HOUNDS_TAIL: i32 = 4327;
const TYRANT_KINGPINS_HEART: i32 = 4328;
const TRISALIM_TARANTULAS_VENOM_SAC: i32 = 4329;
const PENITENTS_MANACLES1: i32 = 4330;
const MANUAL_OF_MANACLES: i32 = 4331;
const PENITENTS_MANACLES: i32 = 4425;

// Reward
const MANACLES_OF_PENITENT: i32 = 4426;

// Materials
const SILVER_NUGGET: i32 = 1873;
const ADAMANTITE_NUGGET: i32 = 1877;
const COKES: i32 = 1879;
const STEEL: i32 = 1880;
const BLACKSMITHS_FRAME: i32 = 1892;

// Monsters
const SCAVENGER_WERERAT: i32 = 20039;
const TYRANT_KINGPIN: i32 = 20193;
const TUREK_WAR_HOUND: i32 = 20494;
const TRISALIM_TARANTULA: i32 = 20561;

/// Java `Q00422_RepentYourSins`.
pub struct Q00422RepentYourSins;

impl Script for Q00422RepentYourSins {
    fn name(&self) -> &'static str {
        "Q00422_RepentYourSins"
    }

    fn path(&self) -> &'static str {
        "quests/Q00422_RepentYourSins"
    }

    fn id(&self) -> i32 {
        422
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACK_JUDGE]);
        s.add_talk_id(&[
            BLACK_JUDGE,
            BLACKSMITH_PUSHKIN,
            PIOTUR,
            ELDER_CASIAN,
            KATARI,
            MAGISTER_JOAN,
        ]);
        s.add_kill_id(&[
            SCAVENGER_WERERAT,
            TYRANT_KINGPIN,
            TUREK_WAR_HOUND,
            TRISALIM_TARANTULA,
        ]);
        s.register_quest_items(&[
            RATMAN_SCAVENGERS_SKULL,
            TUREK_WAR_HOUNDS_TAIL,
            TYRANT_KINGPINS_HEART,
            TRISALIM_TARANTULAS_VENOM_SAC,
            PENITENTS_MANACLES1,
            MANUAL_OF_MANACLES,
            PENITENTS_MANACLES,
        ]);
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

        match event {
            "ACCEPT" => {
                qs.start_quest(ctx);
                let level = ctx.player_level(player);
                if level > 20 && level < 31 {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 3, false);
                    Some("30981-04.htm".to_owned())
                } else if level < 21 {
                    qs.set_memo_state(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_cond(ctx, 2, false);
                    Some("30981-03.htm".to_owned())
                } else if level > 30 && level < 41 {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 4, false);
                    Some("30981-05.htm".to_owned())
                } else {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 5, false);
                    Some("30981-06.htm".to_owned())
                }
            }
            "30981-11.html" => {
                let memo = qs.get_memo_state(ctx);
                if (9..=12).contains(&memo)
                    && ctx.has_at_least_one_quest_item(player, &[MANACLES_OF_PENITENT, PENITENTS_MANACLES1])
                {
                    if ctx.has_quest_items(player, PENITENTS_MANACLES1) {
                        ctx.take_items(player, PENITENTS_MANACLES1, 1);
                    }
                    if ctx.has_quest_items(player, MANACLES_OF_PENITENT) {
                        ctx.take_items(player, MANACLES_OF_PENITENT, 1);
                    }
                    let level = ctx.player_level(player);
                    qs.set_memo_state_ex(ctx, 1, level);
                    ctx.give_items(player, PENITENTS_MANACLES, 1);
                    qs.set_cond(ctx, 16, false);
                    return Some(event.to_owned());
                }
                None
            }
            "30981-14.html" | "30981-17.html" => {
                if (9..=12).contains(&qs.get_memo_state(ctx)) {
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30981-15t.html" => {
                // Java: the manacles in the inventory; its enchant level is the pet level.
                let pet_item = ctx.has_quest_items(player, PENITENTS_MANACLES);
                let pet_level = ctx.get_enchant_level(player, PENITENTS_MANACLES);
                let memo = qs.get_memo_state(ctx);
                let memo_ex = qs.get_memo_state_ex(ctx, 1);
                if !((9..=12).contains(&memo) && pet_level > memo_ex) {
                    return None;
                }
                if ctx.summon_of(player).is_some() {
                    // A summon in the way: the page only, nothing is taken.
                    return Some(event.to_owned());
                }

                let level = ctx.player_level(player);
                let mut i1 = if level > memo_ex {
                    pet_level - memo_ex - (level - memo_ex)
                } else {
                    pet_level - memo_ex
                };
                if i1 < 0 {
                    i1 = 0;
                }
                let i0 = ctx.get_random(i1) + 1;
                let pk_kills = ctx.pk_kills_of(player);
                ctx.give_items(player, MANACLES_OF_PENITENT, 1);
                if pet_item {
                    ctx.take_items(player, PENITENTS_MANACLES, -1);
                }
                if pk_kills <= i0 {
                    ctx.set_pk_kills(player, 0);
                    qs.exit_quest(ctx, true, true);
                    Some("30981-15.html".to_owned())
                } else {
                    ctx.set_pk_kills(player, pk_kills - i0);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    Some("30981-16.html".to_owned())
                }
            }
            "30981-18.html" => {
                if (9..=12).contains(&qs.get_memo_state(ctx)) {
                    qs.exit_quest(ctx, true, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        match npc.id {
            SCAVENGER_WERERAT => {
                if qs.is_memo_state(ctx, 5) && ctx.get_quest_items_count(killer, RATMAN_SCAVENGERS_SKULL) < 10
                {
                    give_quest_item(ctx, killer, RATMAN_SCAVENGERS_SKULL, 9);
                }
            }
            TYRANT_KINGPIN => {
                if qs.is_memo_state(ctx, 7) && !ctx.has_quest_items(killer, TYRANT_KINGPINS_HEART) {
                    ctx.give_items(killer, TYRANT_KINGPINS_HEART, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                }
            }
            TUREK_WAR_HOUND => {
                if qs.is_memo_state(ctx, 6) && ctx.get_quest_items_count(killer, TUREK_WAR_HOUNDS_TAIL) < 10 {
                    give_quest_item(ctx, killer, TUREK_WAR_HOUNDS_TAIL, 9);
                }
            }
            TRISALIM_TARANTULA => {
                if qs.is_memo_state(ctx, 8)
                    && ctx.get_quest_items_count(killer, TRISALIM_TARANTULAS_VENOM_SAC) < 3
                {
                    give_quest_item(ctx, killer, TRISALIM_TARANTULAS_VENOM_SAC, 2);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);

        if qs.is_created(ctx) {
            if npc.id == BLACK_JUDGE {
                htmltext = if ctx.pk_kills_of(player) == 0 {
                    "30981-01.htm"
                } else {
                    "30981-02.htm"
                }
                .to_owned();
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                BLACK_JUDGE => {
                    if memo == 1000 {
                        ctx.take_items(player, PENITENTS_MANACLES, 1);
                    } else if memo < 9 {
                        htmltext = "30981-07.html".to_owned();
                    } else if (9..=12).contains(&memo) {
                        if let Some(page) = black_judge_page(ctx, player, qs) {
                            htmltext = page.to_owned();
                        }
                    }
                }
                BLACKSMITH_PUSHKIN => {
                    if (9..=12).contains(&memo) {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[PENITENTS_MANACLES1, PENITENTS_MANACLES, MANACLES_OF_PENITENT],
                        ) && ctx.has_quest_items(player, MANUAL_OF_MANACLES)
                        {
                            if ctx.get_quest_items_count(player, BLACKSMITHS_FRAME) > 0
                                && ctx.get_quest_items_count(player, STEEL) >= 5
                                && ctx.get_quest_items_count(player, ADAMANTITE_NUGGET) >= 2
                                && ctx.get_quest_items_count(player, SILVER_NUGGET) >= 10
                                && ctx.get_quest_items_count(player, COKES) >= 10
                            {
                                ctx.take_items(player, SILVER_NUGGET, 10);
                                ctx.take_items(player, ADAMANTITE_NUGGET, 2);
                                ctx.take_items(player, COKES, 10);
                                ctx.take_items(player, STEEL, 5);
                                ctx.take_items(player, BLACKSMITHS_FRAME, 1);
                                ctx.give_items(player, PENITENTS_MANACLES1, 1);
                                ctx.take_items(player, MANUAL_OF_MANACLES, 1);
                                qs.set_cond(ctx, 15, true);
                                htmltext = "30300-01.html".to_owned();
                            } else {
                                htmltext = "30300-02.html".to_owned();
                            }
                        } else if ctx.has_at_least_one_quest_item(
                            player,
                            &[PENITENTS_MANACLES1, PENITENTS_MANACLES, MANACLES_OF_PENITENT],
                        ) {
                            htmltext = "30300-03.html".to_owned();
                        }
                    }
                }
                PIOTUR => match memo {
                    2 => {
                        qs.set_memo_state(ctx, 6);
                        qs.set_cond(ctx, 7, true);
                        htmltext = "30597-01.html".to_owned();
                    }
                    6 => {
                        if ctx.get_quest_items_count(player, TUREK_WAR_HOUNDS_TAIL) < 10 {
                            htmltext = "30597-02.html".to_owned();
                        } else {
                            ctx.take_items(player, TUREK_WAR_HOUNDS_TAIL, -1);
                            qs.set_memo_state(ctx, 10);
                            qs.set_cond(ctx, 11, true);
                            htmltext = "30597-03.html".to_owned();
                        }
                    }
                    10 => htmltext = "30597-04.html".to_owned(),
                    _ => {}
                },
                ELDER_CASIAN => match memo {
                    3 => {
                        qs.set_memo_state(ctx, 7);
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30612-01.html".to_owned();
                    }
                    7 => {
                        if !ctx.has_quest_items(player, TYRANT_KINGPINS_HEART) {
                            htmltext = "30612-02.html".to_owned();
                        } else {
                            ctx.take_items(player, TYRANT_KINGPINS_HEART, -1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 12, true);
                            htmltext = "30612-03.html".to_owned();
                        }
                    }
                    11 => htmltext = "30612-04.html".to_owned(),
                    _ => {}
                },
                KATARI => match memo {
                    1 => {
                        qs.set_memo_state(ctx, 5);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30668-01.html".to_owned();
                    }
                    5 => {
                        if ctx.get_quest_items_count(player, RATMAN_SCAVENGERS_SKULL) < 10 {
                            htmltext = "30668-02.html".to_owned();
                        } else {
                            ctx.take_items(player, RATMAN_SCAVENGERS_SKULL, -1);
                            qs.set_memo_state(ctx, 9);
                            qs.set_cond(ctx, 10, true);
                            htmltext = "30668-03.html".to_owned();
                        }
                    }
                    9 => htmltext = "30668-04.html".to_owned(),
                    _ => {}
                },
                MAGISTER_JOAN => match memo {
                    4 => {
                        qs.set_memo_state(ctx, 8);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30718-01.html".to_owned();
                    }
                    8 => {
                        if ctx.get_quest_items_count(player, TRISALIM_TARANTULAS_VENOM_SAC) < 3 {
                            htmltext = "30718-02.html".to_owned();
                        } else {
                            ctx.take_items(player, TRISALIM_TARANTULAS_VENOM_SAC, -1);
                            qs.set_memo_state(ctx, 12);
                            qs.set_cond(ctx, 13, true);
                            htmltext = "30718-03.html".to_owned();
                        }
                    }
                    12 => htmltext = "30718-04.html".to_owned(),
                    _ => {}
                },
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `BLACK_JUDGE` talk in the memo states 9 to 12: which page the player gets, and the manual
/// of the manacles that the first talk hands out.
/// `None` when no branch matches: Java keeps the no-quest page then.
fn black_judge_page(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs) -> Option<&'static str> {
    if !ctx.has_at_least_one_quest_item(
        player,
        &[
            MANUAL_OF_MANACLES,
            MANACLES_OF_PENITENT,
            PENITENTS_MANACLES1,
            PENITENTS_MANACLES,
        ],
    ) {
        ctx.give_items(player, MANUAL_OF_MANACLES, 1);
        qs.set_cond(ctx, 14, true);
        Some("30981-08.html")
    } else if ctx.has_quest_items(player, MANUAL_OF_MANACLES)
        && !ctx.has_at_least_one_quest_item(
            player,
            &[MANACLES_OF_PENITENT, PENITENTS_MANACLES1, PENITENTS_MANACLES],
        )
    {
        Some("30981-09.html")
    } else if ctx.has_quest_items(player, PENITENTS_MANACLES1)
        && !ctx.has_at_least_one_quest_item(
            player,
            &[MANUAL_OF_MANACLES, MANACLES_OF_PENITENT, PENITENTS_MANACLES],
        )
    {
        Some("30981-10.html")
    } else if ctx.has_quest_items(player, PENITENTS_MANACLES) {
        // Java: the manacles' enchant level (0 without the item) against the memo value + 1.
        let pet_level = ctx.get_enchant_level(player, PENITENTS_MANACLES);
        if pet_level < qs.get_memo_state_ex(ctx, 1) + 1 {
            Some("30981-12.html")
        } else {
            Some("30981-13.html")
        }
    } else if ctx.has_quest_items(player, MANACLES_OF_PENITENT)
        && !ctx.has_quest_items(player, PENITENTS_MANACLES)
    {
        Some("30981-16t.html")
    } else {
        None
    }
}

/// Java `giveItems` + `playSound` of a kill's quest item: the sound is the middle one when the
/// count was `last_count` before this item (the last one that makes the set), else the item one.
fn give_quest_item(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32, last_count: i64) {
    let before = ctx.get_quest_items_count(killer, item);
    ctx.give_items(killer, item, 1);
    if before == last_count {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00422_RepentYourSins";

    const NPCS: &str = r#"
        <npc id="30981" type="Folk" level="70" name="Black Judge"/>
        <npc id="30300" type="Folk" level="70" name="Blacksmith Pushkin"/>
        <npc id="30668" type="Folk" level="70" name="Katari"/>
        <npc id="20039" type="Monster" level="20" name="Scavenger Werewolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4326" type="EtcItem" name="Ratman Scavenger's Skull"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4330" type="EtcItem" name="Penitent's Manacles 1"><set name="is_questitem" val="true"/></item>
        <item id="4331" type="EtcItem" name="Manual of Manacles"><set name="is_questitem" val="true"/></item>
        <item id="4425" type="Armor" name="Penitent's Manacles"><set name="bodypart" val="rfinger;lfinger"/></item>
        <item id="4426" type="EtcItem" name="Manacles of Penitent"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00422RepentYourSins);
        let pages = Pages::of(&Q00422RepentYourSins)
            .page("30981-02.htm", &["ACCEPT"])
            .plain(&[
                "30981-01.htm",
                "30981-03.htm",
                "30981-04.htm",
                "30981-07.html",
                "30981-08.html",
            ])
            .page("30981-10.html", &["30981-11.html"])
            .plain(&[
                "30981-11.html",
                "30981-12.html",
                "30981-13.html",
                "30981-16t.html",
                "30981-09.html",
            ])
            .plain(&[
                "30300-01.html",
                "30300-02.html",
                "30300-03.html",
                "30668-01.html",
                "30668-02.html",
                "30668-03.html",
                "30668-04.html",
            ])
            .npcs(&[BLACK_JUDGE, BLACKSMITH_PUSHKIN, KATARI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let judge = t.spawn(BLACK_JUDGE, 20);
        let katari = t.spawn(KATARI, 40);
        (t, judge, katari)
    }

    fn memo(t: &mut ScriptTest) -> i32 {
        let mut value = 0;
        t.world.run_as_script(NAME, |ctx| {
            value = ctx
                .get_quest_state(Oid(1), false)
                .map_or(-1, |qs| qs.get_memo_state(ctx));
        });
        value
    }

    /// A level 15 character with PK kills and the penitent's manacles of `enchant`, who accepted the
    /// quest and is in memo state 9 (the manacles' step). Level 15 and memo value 0 make Java's
    /// `i1` 0, so the roll `getRandom(0) + 1` is always 1.
    fn at_the_manacles(enchant: i32, pk_kills: i32) -> (ScriptTest, Oid) {
        let (mut t, judge, _) = setup();
        let mut manacles = item(900, PENITENTS_MANACLES, 1);
        manacles.enchant_level = enchant;
        let mut data = player_data(1, 15, vec![manacles]);
        data.pk_kills = pk_kills;
        t.enter_with(data);
        // The first talk makes the state; the accept is called as its link would (the link is on
        // page 02, which only a character with PK kills gets).
        t.quest_page(1, judge);
        t.world.run_as_script(NAME, |ctx| {
            Q00422RepentYourSins.on_event(ctx, "ACCEPT", None, Some(Oid(1)));
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 9);
        });
        t.take_writes();
        (t, judge)
    }

    #[test]
    fn the_manacles_take_the_pk_kills_back_when_the_roll_is_short() {
        let (mut t, _judge) = at_the_manacles(3, 3);
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00422RepentYourSins.on_event(ctx, "30981-15t.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30981-16.html"));
        // The roll takes 1 PK kill; the manacles of the penitent are given, the pet is taken.
        assert_eq!(t.count(1, MANACLES_OF_PENITENT), 1);
        assert_eq!(t.count(1, PENITENTS_MANACLES), 0);
        let mut pk = 0;
        t.world.run_as_script(NAME, |ctx| pk = ctx.pk_kills_of(Oid(1)));
        assert_eq!(pk, 2);
    }

    #[test]
    fn the_manacles_end_the_quest_when_the_character_has_no_pk_kills_left() {
        let (mut t, _judge) = at_the_manacles(3, 0);
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00422RepentYourSins.on_event(ctx, "30981-15t.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30981-15.html"));
        assert_eq!(t.count(1, MANACLES_OF_PENITENT), 1);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_judge_sends_a_character_with_pk_kills_away() {
        let (mut t, judge, _) = setup();
        let mut data = player_data(1, 15, vec![]);
        data.pk_kills = 3;
        t.enter_with(data);
        assert!(t.quest_page(1, judge).contains("30981-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_low_level_character_takes_the_skulls_from_the_wererats() {
        let (mut t, judge, katari) = setup();
        // Java: the accept link is on the page for a character with PK kills (02); 01 has none.
        let mut data = player_data(1, 15, vec![]);
        data.pk_kills = 3;
        t.enter_with(data);
        assert!(t.quest_page(1, judge).contains("30981-02.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30981-03.htm"));
        // The level 15 accept: cond 2 (no sound), memo state 1.
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
        assert_eq!(memo(&mut t), 1);
        assert!(t.quest_page(1, katari).contains("30668-01.html"));
        assert_eq!(memo(&mut t), 5);

        // Ten skulls from the wererats (a kill in range).
        for _ in 0..10 {
            let rat = t.spawn(SCAVENGER_WERERAT, 60);
            t.kill(1, rat);
        }
        assert_eq!(t.count(1, RATMAN_SCAVENGERS_SKULL), 10);

        // Katari takes them and moves on to memo 9; the judge then gives the manual.
        assert!(t.quest_page(1, katari).contains("30668-03.html"));
        assert_eq!(memo(&mut t), 9);
        assert_eq!(t.count(1, RATMAN_SCAVENGERS_SKULL), 0);
        assert!(t.quest_page(1, judge).contains("30981-08.html"));
        assert_eq!(t.count(1, MANUAL_OF_MANACLES), 1);
    }

    #[test]
    fn the_manacles_talk_is_refused_without_the_memo_state() {
        // The events of the judge's last steps only work in memo states 9 to 12.
        let (mut t, judge, _) = setup();
        let mut data = player_data(1, 15, vec![]);
        data.pk_kills = 3;
        t.enter_with(data);
        assert!(t.quest_page(1, judge).contains("30981-02.htm"));
        t.event(1, NAME, "ACCEPT");
        t.take_writes();
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00422RepentYourSins.on_event(ctx, "30981-18.html", None, Some(Oid(1)));
        });
        assert_eq!(out, None);
        assert!(t.take_writes().is_empty());
    }
}
