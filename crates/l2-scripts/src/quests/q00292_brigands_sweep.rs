//! Quest 292: Brigands Sweep (Spiron in the Dwarven Village buys the goblin necklaces and pendants).
// Java: dist/game/data/scripts/quests/Q00292_BrigandsSweep/Q00292_BrigandsSweep.java
//
// Pattern: a repeatable collect quest with a second item chain (`docs/design/scripts.md`, "Quest
// patterns"): the goblins drop necklaces, pendants and lord pendants (six rolls out of ten) and,
// on the fifth roll, a suspicious memo while the player has no contract. Three memos turn into one
// suspicious contract (cond 2); Balanki buys the contract for 620 adena, Spiron pays for the
// trinkets and the memos, and the quest is left with `exitQuest(true, true)`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SPIRON: i32 = 30532;
const BALANKI: i32 = 30533;

// Items
const GOBLIN_NECKLACE: i32 = 1483;
const GOBLIN_PENDANT: i32 = 1484;
const GOBLIN_LORD_PENDANT: i32 = 1485;
const SUSPICIOUS_MEMO: i32 = 1486;
const SUSPICIOUS_CONTRACT: i32 = 1487;

// Misc
const MIN_LEVEL: i32 = 5;

/// Java `MOB_ITEM_DROP`: the trinket each goblin drops.
fn mob_item_drop(npc_id: i32) -> Option<i32> {
    match npc_id {
        20322 => Some(GOBLIN_NECKLACE),     // Goblin Brigand
        20323 => Some(GOBLIN_PENDANT),      // Goblin Brigand Leader
        20324 => Some(GOBLIN_NECKLACE),     // Goblin Brigand Lieutenant
        20327 => Some(GOBLIN_NECKLACE),     // Goblin Snooper
        20528 => Some(GOBLIN_LORD_PENDANT), // Goblin Lord
        _ => None,
    }
}

/// Java `Q00292_BrigandsSweep`.
pub struct Q00292BrigandsSweep;

impl Script for Q00292BrigandsSweep {
    fn name(&self) -> &'static str {
        "Q00292_BrigandsSweep"
    }

    fn path(&self) -> &'static str {
        "quests/Q00292_BrigandsSweep"
    }

    fn id(&self) -> i32 {
        292
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SPIRON]);
        s.add_talk_id(&[SPIRON, BALANKI]);
        s.add_kill_id(&[20322, 20323, 20324, 20327, 20528]);
        s.register_quest_items(&[
            GOBLIN_NECKLACE,
            GOBLIN_PENDANT,
            GOBLIN_LORD_PENDANT,
            SUSPICIOUS_MEMO,
            SUSPICIOUS_CONTRACT,
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
        let mut html = None;
        match event {
            "30532-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    html = Some(event.to_owned());
                }
            }
            "30532-06.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    html = Some(event.to_owned());
                }
            }
            "30532-07.html" => {
                if qs.is_started(ctx) {
                    html = Some(event.to_owned());
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        let Some(qs) = qs else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        let chance = ctx.get_random(10);
        if chance > 5 {
            if let Some(item) = mob_item_drop(npc.id) {
                // Java's 7-argument overload: one trinket, no limit, always dropped.
                ctx.give_item_randomly(killer, Some(npc), item, 1, 1, 0, 1.0, true);
            }
        } else if qs.is_cond(ctx, 1) && chance > 4 && !ctx.has_quest_items(killer, SUSPICIOUS_CONTRACT) {
            let memos = ctx.get_quest_items_count(killer, SUSPICIOUS_MEMO);
            if memos < 3 {
                // Limit 3: the third memo is the one that returns true.
                if ctx.give_item_randomly(killer, Some(npc), SUSPICIOUS_MEMO, 1, 1, 3, 1.0, false) {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    ctx.give_items(killer, SUSPICIOUS_CONTRACT, 1);
                    ctx.take_items(killer, SUSPICIOUS_MEMO, -1);
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);
        match npc.id {
            SPIRON => match qs.state(ctx) {
                CREATED => {
                    html = if ctx.player_race(talker) == race::DWARF {
                        if ctx.player_level(talker) >= MIN_LEVEL {
                            "30532-02.htm"
                        } else {
                            "30532-01.htm"
                        }
                    } else {
                        "30532-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    let registered = [
                        GOBLIN_NECKLACE,
                        GOBLIN_PENDANT,
                        GOBLIN_LORD_PENDANT,
                        SUSPICIOUS_MEMO,
                        SUSPICIOUS_CONTRACT,
                    ];
                    if !ctx.has_at_least_one_quest_item(talker, &registered) {
                        html = "30532-04.html".to_owned();
                    } else {
                        let necklaces = ctx.get_quest_items_count(talker, GOBLIN_NECKLACE);
                        let pendants = ctx.get_quest_items_count(talker, GOBLIN_PENDANT);
                        let lord_pendants = ctx.get_quest_items_count(talker, GOBLIN_LORD_PENDANT);
                        let sum = necklaces + pendants + lord_pendants;
                        if sum > 0 {
                            let bonus = if sum >= 10 { 1000 } else { 0 };
                            ctx.give_adena(
                                talker,
                                necklaces * 12 + pendants * 36 + lord_pendants * 33 + bonus,
                                true,
                            );
                            ctx.take_items_many(
                                talker,
                                -1,
                                &[GOBLIN_NECKLACE, GOBLIN_PENDANT, GOBLIN_LORD_PENDANT],
                            );
                        }
                        if sum > 0
                            && !ctx
                                .has_at_least_one_quest_item(talker, &[SUSPICIOUS_MEMO, SUSPICIOUS_CONTRACT])
                        {
                            html = "30532-05.html".to_owned();
                        } else {
                            let memos = ctx.get_quest_items_count(talker, SUSPICIOUS_MEMO);
                            if memos == 0 && ctx.has_quest_items(talker, SUSPICIOUS_CONTRACT) {
                                ctx.give_adena(talker, 1120, true);
                                // Retail like, reward is given in 2 pieces if both conditions are meet.
                                ctx.take_items_many(talker, -1, &[SUSPICIOUS_CONTRACT]);
                                html = "30532-10.html".to_owned();
                            } else if memos == 1 {
                                html = "30532-08.html".to_owned();
                            } else if memos >= 2 {
                                html = "30532-09.html".to_owned();
                            }
                        }
                    }
                }
                _ => {}
            },
            BALANKI => {
                if qs.is_started(ctx) {
                    if ctx.has_quest_items(talker, SUSPICIOUS_CONTRACT) {
                        ctx.give_adena(talker, 620, true);
                        ctx.take_items(talker, SUSPICIOUS_CONTRACT, -1);
                        html = "30533-02.html".to_owned();
                    } else {
                        html = "30533-01.html".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00292_BrigandsSweep";

    const NPCS: &str = r#"
        <npc id="30532" type="Folk" level="70" name="Spiron"/>
        <npc id="30533" type="Folk" level="70" name="Balanki"/>
        <npc id="20322" type="Monster" level="6" name="Goblin Brigand"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1483" type="EtcItem" name="Goblin Necklace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1484" type="EtcItem" name="Goblin Pendant"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1485" type="EtcItem" name="Goblin Lord Pendant"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1486" type="EtcItem" name="Suspicious Memo"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1487" type="EtcItem" name="Suspicious Contract"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00292BrigandsSweep);
        let pages = Pages::of(&Q00292BrigandsSweep)
            .page("30532-02.htm", &["30532-03.htm"])
            .plain(&["30532-00.htm", "30532-01.htm", "30532-03.htm", "30532-09.html"])
            .page("30532-04.html", &["30532-06.html", "30532-07.html"])
            .page("30532-05.html", &["30532-06.html", "30532-07.html"])
            .page("30532-08.html", &["30532-06.html", "30532-07.html"])
            .plain(&[
                "30532-06.html",
                "30532-07.html",
                "30532-10.html",
                "30533-01.html",
                "30533-02.html",
            ])
            .npcs(&[SPIRON, BALANKI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let spiron = t.spawn(SPIRON, 20);
        let balanki = t.spawn(BALANKI, 40);
        (t, spiron, balanki)
    }

    /// Kills goblin brigands until the player has `count` of `item`.
    fn hunt_until(t: &mut ScriptTest, item: i32, count: i64) {
        for _ in 0..2000 {
            if t.count(1, item) >= count {
                return;
            }
            let brigand = t.spawn(20322, 60);
            t.kill(1, brigand);
        }
        panic!("no {count} of {item} after 2000 kills");
    }

    #[test]
    fn only_dwarves_of_level_five_can_take_it() {
        let (mut t, spiron, _balanki) = setup();
        t.enter_as(1, 5, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, spiron).contains("30532-00.htm"));
        t.enter_as(2, 4, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, spiron).contains("30532-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn memos_then_the_contract_then_the_payments() {
        let (mut t, spiron, balanki) = setup();
        t.enter_as(1, 5, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, spiron).contains("30532-02.htm"));
        assert!(t.event(1, NAME, "30532-03.htm").contains("30532-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, spiron).contains("30532-04.html"));

        // The first memo: Spiron pays for the necklaces so far and says so (08).
        hunt_until(&mut t, SUSPICIOUS_MEMO, 1);
        let necklaces = t.count(1, GOBLIN_NECKLACE);
        let before = t.count(1, 57);
        assert!(t.quest_page(1, spiron).contains("30532-08.html"));
        let bonus = if necklaces >= 10 { 1000 } else { 0 };
        assert_eq!(t.count(1, 57) - before, necklaces * 12 + bonus);
        assert_eq!(t.count(1, GOBLIN_NECKLACE), 0);
        assert!(t.take_writes().is_empty());

        // Three memos make the contract (cond 2) and the memos go.
        hunt_until(&mut t, SUSPICIOUS_CONTRACT, 1);
        assert_eq!(t.count(1, SUSPICIOUS_MEMO), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Balanki buys the contract for 620 adena.
        let before = t.count(1, 57);
        assert!(t.quest_page(1, balanki).contains("30533-02.html"));
        assert_eq!(t.count(1, 57) - before, 620);
        assert_eq!(t.count(1, SUSPICIOUS_CONTRACT), 0);

        // Balanki has no contract left any more.
        assert!(t.quest_page(1, balanki).contains("30533-01.html"));

        // Spiron pays for the trinkets of the meantime (05); then the player leaves the quest
        // (`exitQuest(true, true)` from that page).
        let necklaces = t.count(1, GOBLIN_NECKLACE);
        assert!(necklaces > 0);
        let before = t.count(1, 57);
        assert!(t.quest_page(1, spiron).contains("30532-05.html"));
        let bonus = if necklaces >= 10 { 1000 } else { 0 };
        assert_eq!(t.count(1, 57) - before, necklaces * 12 + bonus);
        assert!(t.event(1, NAME, "30532-06.html").contains("30532-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
