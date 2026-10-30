//! Quest 235: Mimir's Elixir (Ladd in the Gludin ... sends the characters of level 75 and up with
//! the Star of Destiny through Joan, the chimera and the bloody guardian, and the Alchemists'
//! Mixing Urn to the Mimir's Elixir).
// Java: dist/game/data/scripts/quests/Q00235_MimirsElixir/Q00235_MimirsElixir.java
//
// Pattern: the step is the memo state (`memoState`, `set_memo_state`), the cond follows it. The
// party drops are rolled on the NPC with `random_party_member_for_npc` and the member's
// `checkPartyMember` (memo 3 or 6). The choice events of the urn are `bypass -h` links, see the
// NOTE in `on_event`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const JOAN: i32 = 30718;
const LADD: i32 = 30721;
const ALCHEMISTS_MIXING_URN: i32 = 31149;

// Items
const STAR_OF_DESTINY: i32 = 5011;
const MAGISTERS_MIXING_STONE: i32 = 5905;
const BLOOD_FIRE: i32 = 6318;
const MIMIRS_ELIXIR: i32 = 6319;
const PURE_SILVER: i32 = 6320;
const TRUE_GOLD: i32 = 6321;
const SAGES_STONE: i32 = 6322;

// Reward
const ENCHANT_WEAPON_A: i32 = 729;

// Misc
const MIN_LEVEL: i32 = 75;

// Skill: the quest's Mimir's Elixir cast
const QUEST_MIMIRS_ELIXIR: SkillKey = SkillKey { id: 4339, level: 1 };

/// Java `MOBS`: the monster, the item it drops and the memo state (and cond) the drop sets.
const MOBS: [(i32, i32, i32); 2] = [
    // chimera_piece: Sage's Stone, memo 4
    (20965, SAGES_STONE, 4),
    // bloody_guardian: Blood Fire, memo 7
    (21090, BLOOD_FIRE, 7),
];

/// Java `Q00235_MimirsElixir`.
pub struct Q00235MimirsElixir;

impl Script for Q00235MimirsElixir {
    fn name(&self) -> &'static str {
        "Q00235_MimirsElixir"
    }

    fn path(&self) -> &'static str {
        "quests/Q00235_MimirsElixir"
    }

    fn id(&self) -> i32 {
        235
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LADD]);
        s.add_talk_id(&[LADD, JOAN, ALCHEMISTS_MIXING_URN]);
        for (mob, _, _) in MOBS {
            s.add_kill_id(&[mob]);
        }
        s.register_quest_items(&[
            MAGISTERS_MIXING_STONE,
            BLOOD_FIRE,
            MIMIRS_ELIXIR,
            TRUE_GOLD,
            SAGES_STONE,
        ]);
    }

    fn check_party_member(&self, ctx: &mut ScriptCtx<'_>, member: ObjectId, _npc: Option<ScriptNpc>) -> bool {
        ctx.get_quest_state(member, false)
            .is_some_and(|qs| qs.is_memo_state(ctx, 3) || qs.is_memo_state(ctx, 6))
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "30721-02.htm" | "30721-03.htm" | "30721-04.htm" | "30721-05.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30721-06.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30721-12.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30721-15.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.give_items(player, MAGISTERS_MIXING_STONE, 1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30721-18.html" => {
                if qs.is_memo_state(ctx, 8) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30721-19.html" => {
                if qs.is_memo_state(ctx, 8)
                    && ctx.has_all_quest_items(player, &[MAGISTERS_MIXING_STONE, MIMIRS_ELIXIR])
                {
                    if let Some(npc) = npc {
                        // Java `npc.setTarget(player); npc.doCast(skill)`.
                        ctx.cast_skill(npc, player, QUEST_MIMIRS_ELIXIR);
                    }
                    ctx.take_items(player, STAR_OF_DESTINY, -1);
                    ctx.give_items(player, ENCHANT_WEAPON_A, 1);
                    qs.exit_quest(ctx, false, true);
                    // FOLLOW-UP(engine): Java `player.sendPacket(new SocialAction(objectId, 3))`:
                    // no public world call sends a social action to one player yet.
                    htmltext = Some(event.to_owned());
                }
            }
            "30718-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30718-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30718-06.html" => {
                if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(player, SAGES_STONE) {
                    ctx.give_items(player, TRUE_GOLD, 1);
                    ctx.take_items(player, SAGES_STONE, -1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31149-02.html" | "31149-05.html" | "31149-07.html" | "31149-09.html" | "31149-10.html" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(event.to_owned());
                }
            }
            // NOTE(java-quirk): the three choice events below come from `bypass -h <name>` links,
            // which `RequestBypassToServer` hands to `BypassHandler` (no handler: "not handled"),
            // so no link reaches them in Java either. The arms are kept as Java has them.
            "PURE_SILVER" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(
                        if ctx.has_quest_items(player, PURE_SILVER) {
                            "31149-04.html"
                        } else {
                            "31149-03.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "TRUE_GOLD" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(
                        if ctx.has_quest_items(player, TRUE_GOLD) {
                            "31149-06.html"
                        } else {
                            "31149-03.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "BLOOD_FIRE" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(
                        if ctx.has_quest_items(player, BLOOD_FIRE) {
                            "31149-08.html"
                        } else {
                            "31149-03.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "31149-11.html" => {
                if qs.is_memo_state(ctx, 7)
                    && ctx.has_all_quest_items(player, &[BLOOD_FIRE, PURE_SILVER, TRUE_GOLD])
                {
                    ctx.give_items(player, MIMIRS_ELIXIR, 1);
                    ctx.take_items_many(player, -1, &[BLOOD_FIRE, PURE_SILVER, TRUE_GOLD]);
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_random(5) != 0 {
            return;
        }
        let Some(lucky) = ctx.random_party_member_for_npc(killer, Some(npc)) else {
            return;
        };
        let Some(&(_, item, chance)) = MOBS.iter().find(|(mob, _, _)| *mob == npc.id) else {
            return;
        };
        // `giveItemRandomly(player, npc, item, 1, 1, 1, true)`: one item, limit 1, every time.
        if ctx.give_item_randomly(lucky, Some(npc), item, 1, 1, 1, 1.0, true)
            && let Some(qs) = ctx.get_quest_state(lucky, false)
        {
            qs.set_memo_state(ctx, chance);
            qs.set_cond(ctx, chance, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == LADD {
                    htmltext = if ctx.player_race(player) == race::KAMAEL {
                        "30721-09.html"
                    } else if ctx.player_level(player) < MIN_LEVEL {
                        "30721-08.html"
                    } else if ctx.has_quest_items(player, STAR_OF_DESTINY) {
                        "30721-01.htm"
                    } else {
                        "30721-07.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                LADD => {
                    htmltext = match qs.get_memo_state(ctx) {
                        1 => {
                            if ctx.has_quest_items(player, PURE_SILVER) {
                                "30721-11.html"
                            } else {
                                "30721-10.html"
                            }
                        }
                        2..=4 => "30721-13.html",
                        5 => "30721-14.html",
                        6 | 7 => "30721-16.html",
                        8 => "30721-17.html",
                        _ => return Some(htmltext),
                    }
                    .to_owned();
                }
                JOAN => {
                    htmltext = match qs.get_memo_state(ctx) {
                        2 => "30718-01.html",
                        3 => "30718-04.html",
                        4 => "30718-05.html",
                        _ => return Some(htmltext),
                    }
                    .to_owned();
                }
                ALCHEMISTS_MIXING_URN => {
                    if qs.is_memo_state(ctx, 7) && ctx.has_quest_items(player, MAGISTERS_MIXING_STONE) {
                        htmltext = "31149-01.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == LADD {
                    htmltext = ctx.get_already_completed_msg(player);
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
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00235_MimirsElixir";

    const NPCS: &str = r#"
        <npc id="30721" type="Folk" level="70" name="Ladd"/>
        <npc id="30718" type="Folk" level="70" name="Joan"/>
        <npc id="31149" type="Folk" level="70" name="Alchemists Mixing Urn"/>
        <npc id="20965" type="Monster" level="70" name="Chimera Piece"/>
        <npc id="21090" type="Monster" level="70" name="Bloody Guardian"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5011" type="EtcItem" name="Star of Destiny"/>
        <item id="5905" type="EtcItem" name="Magister's Mixing Stone"><set name="is_questitem" val="true"/></item>
        <item id="6318" type="EtcItem" name="Blood Fire"><set name="is_questitem" val="true"/></item>
        <item id="6319" type="EtcItem" name="Mimir's Elixir"><set name="is_questitem" val="true"/></item>
        <item id="6320" type="EtcItem" name="Pure Silver"/>
        <item id="6321" type="EtcItem" name="True Gold"><set name="is_questitem" val="true"/></item>
        <item id="6322" type="EtcItem" name="Sage's Stone"><set name="is_questitem" val="true"/></item>
        <item id="729" type="Weapon" name="Enchant Weapon A"><set name="weapon_type" val="sword"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00235MimirsElixir);
        let pages = Pages::of(&Q00235MimirsElixir)
            .page("30721-01.htm", &["30721-02.htm"])
            .page("30721-02.htm", &["30721-03.htm"])
            .page("30721-03.htm", &["30721-04.htm"])
            .page("30721-04.htm", &["30721-05.htm"])
            .page("30721-05.htm", &["30721-06.htm"])
            .plain(&[
                "30721-06.htm",
                "30721-07.html",
                "30721-08.html",
                "30721-09.html",
                "30721-10.html",
            ])
            .plain(&["30721-13.html", "30721-16.html"])
            .page("30721-11.html", &["30721-12.html"])
            .plain(&["30721-12.html"])
            .page("30721-14.html", &["30721-15.html"])
            .plain(&["30721-15.html"])
            .page("30721-17.html", &["30721-18.html"])
            .page("30721-18.html", &["30721-19.html"])
            .plain(&["30721-19.html"])
            .page("30718-01.html", &["30718-02.html"])
            .page("30718-02.html", &["30718-03.html"])
            .plain(&["30718-03.html", "30718-04.html"])
            .page("30718-05.html", &["30718-06.html"])
            .plain(&["30718-06.html"])
            .plain(&["31149-01.html", "31149-03.html", "31149-04.html", "31149-06.html"])
            .page("31149-08.html", &["31149-11.html"])
            .plain(&["31149-11.html", "31149-02.html"])
            .npcs(&[LADD, JOAN, ALCHEMISTS_MIXING_URN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ladd = t.spawn(LADD, 20);
        let joan = t.spawn(JOAN, 40);
        let urn = t.spawn(ALCHEMISTS_MIXING_URN, 60);
        (t, ladd, joan, urn)
    }

    /// Kills monsters of `mob` (one spawn each) until the player has `item`, as the party did.
    fn kill_until(t: &mut ScriptTest, killer: i32, mob: i32, item_id: i32) {
        for _ in 0..500 {
            if t.count(killer, item_id) >= 1 {
                return;
            }
            let m = t.spawn(mob, 20);
            t.kill(killer, m);
        }
        panic!("no drop of {item_id} in 500 kills");
    }

    #[test]
    fn kamael_low_level_and_the_missing_star_are_sent_away() {
        let (mut t, ladd, _, _) = setup();
        t.enter_as(
            1,
            75,
            race::KAMAEL,
            player_class::FIGHTER,
            vec![item(10, 5011, 1)],
        );
        assert!(t.quest_page(1, ladd).contains("30721-09.html"));
        t.enter_as(2, 74, race::HUMAN, player_class::FIGHTER, vec![item(20, 5011, 1)]);
        assert!(t.quest_page(2, ladd).contains("30721-08.html"));
        t.enter_as(3, 75, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(3, ladd).contains("30721-07.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_elixir_from_ladd_through_joan_the_monsters_and_the_urn() {
        let (mut t, ladd, joan, urn) = setup();
        t.enter_as(
            1,
            75,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(10, 5011, 1), item(11, 6320, 1)],
        );
        // Start (memo state 1), then Ladd takes the Pure Silver (memo state 2, cond 2).
        assert!(t.quest_page(1, ladd).contains("30721-01.htm"));
        for event in ["30721-02.htm", "30721-03.htm", "30721-04.htm", "30721-05.htm"] {
            t.event(1, NAME, event);
        }
        assert!(t.event(1, NAME, "30721-06.htm").contains("30721-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ladd).contains("30721-11.html"));
        assert!(t.event(1, NAME, "30721-12.html").contains("30721-12.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Joan (memo state 3, cond 3).
        assert!(t.quest_page(1, joan).contains("30718-01.html"));
        assert!(t.event(1, NAME, "30718-02.html").contains("30718-02.html"));
        assert!(t.event(1, NAME, "30718-03.html").contains("30718-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // The chimera gives the Sage's Stone to the party member at memo state 3: memo 4, cond 4.
        kill_until(&mut t, 1, 20965, SAGES_STONE);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Joan trades it for the True Gold (memo state 5, cond 5).
        assert!(t.quest_page(1, joan).contains("30718-05.html"));
        assert!(t.event(1, NAME, "30718-06.html").contains("30718-06.html"));
        assert_eq!(t.count(1, TRUE_GOLD), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // Ladd gives the Magister's Mixing Stone (memo state 6, cond 6).
        assert!(t.quest_page(1, ladd).contains("30721-14.html"));
        assert!(t.event(1, NAME, "30721-15.html").contains("30721-15.html"));
        assert_eq!(t.count(1, MAGISTERS_MIXING_STONE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "6")
            ]
        );

        // The bloody guardian gives the Blood Fire to the member at memo state 6 (memo 7, cond 7).
        kill_until(&mut t, 1, 21090, BLOOD_FIRE);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "7")
            ]
        );

        // The urn shows its page; the choice (a `bypass -h` link in Java) is called directly.
        assert!(t.quest_page(1, urn).contains("31149-01.html"));
        t.world
            .run_as_script(NAME, |ctx| ctx.notify_event("BLOOD_FIRE", None, Some(Oid(1))));
        assert!(t.event(1, NAME, "31149-11.html").contains("31149-11.html"));
        assert_eq!(t.count(1, MIMIRS_ELIXIR), 1);
        assert_eq!(
            (
                t.count(1, BLOOD_FIRE),
                t.count(1, PURE_SILVER),
                t.count(1, TRUE_GOLD)
            ),
            (0, 0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "8"),
                rows::update(NAME, "cond", "8")
            ]
        );

        // Ladd: the elixir for the enchant weapon, the quest ends.
        assert!(t.quest_page(1, ladd).contains("30721-17.html"));
        assert!(t.event(1, NAME, "30721-18.html").contains("30721-18.html"));
        assert!(t.event(1, NAME, "30721-19.html").contains("30721-19.html"));
        assert_eq!(t.count(1, ENCHANT_WEAPON_A), 1);
        assert_eq!(t.count(1, STAR_OF_DESTINY), 0);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
