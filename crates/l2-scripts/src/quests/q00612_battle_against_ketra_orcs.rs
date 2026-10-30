//! Quest 612: Battle against Ketra Orcs (Ashas in the Varka Silenos Village, Ketra monsters).
// Java: dist/game/data/scripts/quests/Q00612_BattleAgainstKetraOrcs/Q00612_BattleAgainstKetraOrcs.java
//
// Pattern: a repeatable collect quest, the same shape as Q00606 (a mane-like drop per monster
// with a chance, a random member at cond 1 gets it; 100 molars for 20 seeds).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASHAS: i32 = 31377;

// Monsters: (id, drop chance out of 1000), Java's `MOBS` map in its order.
const MOBS: [(i32, i32); 15] = [
    (21324, 500), // Ketra Orc Footman
    (21327, 510), // Ketra Orc Raider
    (21328, 522), // Ketra Orc Scout
    (21329, 519), // Ketra Orc Shaman
    (21331, 529), // Ketra Orc Warrior
    (21332, 529), // Ketra Orc Lieutenant
    (21334, 539), // Ketra Orc Medium
    (21336, 548), // Ketra Orc White Captain
    (21338, 558), // Ketra Orc Seer
    (21339, 568), // Ketra Orc General
    (21340, 568), // Ketra Orc Battalion Commander
    (21342, 578), // Ketra Orc Grand Seer
    (21343, 664), // Ketra Commander
    (21345, 713), // Ketra's Head Shaman
    (21347, 738), // Ketra Prophet
];

// Items
const SEED: i32 = 7187;
const MOLAR: i32 = 7234;

// Misc
const MIN_LEVEL: i32 = 74;
const MOLAR_COUNT: i64 = 100;

/// Java `Q00612_BattleAgainstKetraOrcs`.
pub struct Q00612BattleAgainstKetraOrcs;

impl Script for Q00612BattleAgainstKetraOrcs {
    fn name(&self) -> &'static str {
        "Q00612_BattleAgainstKetraOrcs"
    }

    fn path(&self) -> &'static str {
        "quests/Q00612_BattleAgainstKetraOrcs"
    }

    fn id(&self) -> i32 {
        612
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASHAS]);
        s.add_talk_id(&[ASHAS]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[MOLAR]);
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
            "31377-03.htm" => {
                qs.start_quest(ctx);
            }
            "31377-06.html" => {}
            "31377-07.html" => {
                if ctx.get_quest_items_count(player, MOLAR) < MOLAR_COUNT {
                    return Some("31377-08.html".to_owned());
                }
                ctx.take_items(player, MOLAR, MOLAR_COUNT);
                ctx.give_items(player, SEED, 20);
            }
            "31377-09.html" => {
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): the molar goes to a random member at cond 1; `getRandom` is only rolled
        // when a member was found (`member != null && getRandom(1000) < chance`).
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let chance = MOBS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if ctx.get_random(1000) < chance {
            ctx.give_items(member, MOLAR, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31377-01.htm"
                } else {
                    "31377-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(player, MOLAR) {
                    "31377-04.html"
                } else {
                    "31377-05.html"
                }
                .to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00612_BattleAgainstKetraOrcs";

    const NPCS: &str = r#"
        <npc id="31377" type="Folk" level="70" name="Ashas"/>
        <npc id="21347" type="Monster" level="80" name="Ketra Prophet"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7234" type="EtcItem" name="Ketra Orc Molar"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7187" type="EtcItem" name="Seed of Ketra" ><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00612BattleAgainstKetraOrcs);
        let pages = Pages::of(&Q00612BattleAgainstKetraOrcs)
            .page("31377-01.htm", &["31377-03.htm"])
            .plain(&[
                "31377-02.htm",
                "31377-03.htm",
                "31377-05.html",
                "31377-08.html",
                "31377-09.html",
            ])
            .page(
                "31377-04.html",
                &["31377-06.html", "31377-07.html", "31377-09.html"],
            )
            .page("31377-06.html", &["31377-07.html", "31377-09.html"])
            .page(
                "31377-07.html",
                &["31377-06.html", "31377-07.html", "31377-09.html"],
            )
            .npcs(&[ASHAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ashas = t.spawn(ASHAS, 20);
        (t, ashas)
    }

    /// Kills Ketra Prophets (73.8% each) until the player holds `count` molars.
    fn gather_molars(t: &mut ScriptTest, count: i64) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(21347, 60);
            t.kill(1, mob);
            if t.count(1, MOLAR) >= count {
                return kills;
            }
        }
        panic!("no {count} molars after 400 kills");
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, ashas) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, ashas).contains("31377-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_molars_for_twenty_seeds_then_the_end() {
        let (mut t, ashas) = setup();
        t.enter(1, 74, vec![item(100, MOLAR, 50)]);
        assert!(t.quest_page(1, ashas).contains("31377-01.htm"));
        assert!(t.event(1, NAME, "31377-03.htm").contains("31377-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // With 50 molars the NPC asks for 100: `31377-08.html`, nothing is taken.
        assert!(t.quest_page(1, ashas).contains("31377-04.html"));
        assert!(t.event(1, NAME, "31377-07.html").contains("31377-08.html"));
        assert_eq!(t.count(1, MOLAR), 50);

        let kills = gather_molars(&mut t, MOLAR_COUNT);
        assert!(kills >= 25, "{kills}");
        assert_eq!(t.count(1, MOLAR), MOLAR_COUNT);
        assert!(t.take_writes().is_empty());

        assert!(t.quest_page(1, ashas).contains("31377-04.html"));
        assert!(t.event(1, NAME, "31377-07.html").contains("31377-07.html"));
        assert_eq!((t.count(1, MOLAR), t.count(1, SEED)), (0, 20));

        assert!(t.event(1, NAME, "31377-09.html").contains("31377-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_monster_without_a_party_member_at_cond_one_drops_nothing() {
        let (mut t, _ashas) = setup();
        t.enter(1, 74, vec![]);
        // Not started: nobody at cond 1 in the party, nothing rolls.
        let mob = t.spawn(21347, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, MOLAR), 0);
    }
}
