//! Quest 382: Kail's Magic Coin (Head Blacksmith Vergara, the Fallen Orcs of the Forsaken Plains).
// Java: dist/game/data/scripts/quests/Q00382_KailsMagicCoin/Q00382_KailsMagicCoin.java
//
// Pattern: kills give the coins with a chance, to the killer who has the Royal Membership and
// stands in the alt party range (`check_if_in_range`). No quest item is registered and the quest
// is never finished: the NPC trades (the multisell list is the NPC's own bypass, not the script).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const VERGARA: i32 = 30687;

// Monsters
const FALLEN_ORC: i32 = 21017;
const FALLEN_ORC_ARCHER: i32 = 21019;
const FALLEN_ORC_SHAMAN: i32 = 21020;
const FALLEN_ORC_CAPTAIN: i32 = 21022;

// Items
const ROYAL_MEMBERSHIP: i32 = 5898;
const KAILS_SILVER_BASILISK: i32 = 5961;
const KAILS_GOLD_GOLEM: i32 = 5962;
const KAILS_BLOOD_DRAGON: i32 = 5963;

// Drops
const ORC_CAPTAIN_DROP_CHANCE: f64 = 0.069;
/// Java `MONSTER_DROPS`: monster, coin, chance.
const MONSTER_DROPS: [(i32, i32, f64); 3] = [
    (FALLEN_ORC, KAILS_SILVER_BASILISK, 0.073),
    (FALLEN_ORC_ARCHER, KAILS_GOLD_GOLEM, 0.075),
    (FALLEN_ORC_SHAMAN, KAILS_BLOOD_DRAGON, 0.073),
];

// Misc
const MIN_LEVEL: i32 = 55;

/// Java `Q00382_KailsMagicCoin`.
pub struct Q00382KailsMagicCoin;

impl Script for Q00382KailsMagicCoin {
    fn name(&self) -> &'static str {
        "Q00382_KailsMagicCoin"
    }

    fn path(&self) -> &'static str {
        "quests/Q00382_KailsMagicCoin"
    }

    fn id(&self) -> i32 {
        382
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VERGARA]);
        s.add_talk_id(&[VERGARA]);
        s.add_kill_id(&[
            FALLEN_ORC,
            FALLEN_ORC_ARCHER,
            FALLEN_ORC_SHAMAN,
            FALLEN_ORC_CAPTAIN,
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
        let mut htmltext = None;
        match event {
            "30687-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            // NOTE(java-quirk): the "Back" links of 30687-05/06 send `30687-04.htm`, which
            // `onEvent` does not handle: nothing is shown for it.
            "30687-05.htm" | "30687-06.htm" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        if qs.is_created(ctx) {
            htmltext =
                if ctx.player_level(talker) >= MIN_LEVEL && ctx.has_quest_items(talker, ROYAL_MEMBERSHIP) {
                    "30687-02.htm"
                } else {
                    "30687-01.htm"
                }
                .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = "30687-04.htm".to_owned();
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some()
            && ctx.has_quest_items(killer, ROYAL_MEMBERSHIP)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
        {
            if npc.id == FALLEN_ORC_CAPTAIN {
                // Java: `KAILS_SILVER_BASILISK + getRandom(3)` is evaluated before the roll.
                let coin = KAILS_SILVER_BASILISK + ctx.get_random(3);
                ctx.give_item_randomly(killer, None, coin, 1, 1, 0, ORC_CAPTAIN_DROP_CHANCE, true);
            } else if let Some((_, coin, chance)) = MONSTER_DROPS.iter().find(|(id, _, _)| *id == npc.id) {
                ctx.give_item_randomly(killer, None, *coin, 1, 1, 0, *chance, true);
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

    const NAME: &str = "Q00382_KailsMagicCoin";

    const NPCS: &str = r#"
        <npc id="30687" type="Folk" level="70" name="Head Blacksmith Vergara"/>
        <npc id="21017" type="Monster" level="50" name="Fallen Orc"/>
        <npc id="21019" type="Monster" level="50" name="Fallen Orc Archer"/>
        <npc id="21020" type="Monster" level="50" name="Fallen Orc Shaman"/>
        <npc id="21022" type="Monster" level="53" name="Fallen Orc Captain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5898" type="EtcItem" name="Royal Membership"/>
        <item id="5961" type="EtcItem" name="Kail's Silver Basilisk"><set name="is_stackable" val="true"/></item>
        <item id="5962" type="EtcItem" name="Kail's Gold Golem"><set name="is_stackable" val="true"/></item>
        <item id="5963" type="EtcItem" name="Kail's Blood Dragon"><set name="is_stackable" val="true"/></item>"#;

    const COINS: [i32; 3] = [KAILS_SILVER_BASILISK, KAILS_GOLD_GOLEM, KAILS_BLOOD_DRAGON];

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00382KailsMagicCoin);
        let pages = Pages::of(&Q00382KailsMagicCoin)
            .plain(&["30687-01.htm"])
            .page("30687-02.htm", &["30687-03.htm"])
            .page("30687-03.htm", &["30687-04.htm"])
            .page("30687-04.htm", &["30687-05.htm", "30687-06.htm"])
            .page("30687-05.htm", &["30687-04.htm"])
            .page("30687-06.htm", &["30687-04.htm"])
            .npcs(&[VERGARA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let vergara = t.spawn(VERGARA, 20);
        (t, vergara)
    }

    fn coins(t: &ScriptTest) -> i64 {
        COINS.iter().map(|id| t.count(1, *id)).sum()
    }

    #[test]
    fn only_a_royal_of_level_55_is_offered_the_quest() {
        let (mut t, vergara) = setup();
        t.enter(1, 54, vec![item(100, ROYAL_MEMBERSHIP, 1)]);
        assert!(t.quest_page(1, vergara).contains("30687-01.htm"));
        assert!(t.take_writes().is_empty());
        t.enter(2, 55, vec![]);
        assert!(t.quest_page(2, vergara).contains("30687-01.htm"));
    }

    #[test]
    fn the_trade_pages_and_the_start() {
        let (mut t, vergara) = setup();
        t.enter(1, 55, vec![item(100, ROYAL_MEMBERSHIP, 1)]);
        assert!(t.quest_page(1, vergara).contains("30687-02.htm"));
        assert!(t.event(1, NAME, "30687-03.htm").contains("30687-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, vergara).contains("30687-04.htm"));
        assert!(t.event(1, NAME, "30687-05.htm").contains("30687-05.htm"));
        // "Back" is not an event of the script: nothing is shown.
        assert!(t.event(1, NAME, "30687-04.htm").is_empty());
        assert!(t.quest_page(1, vergara).contains("30687-04.htm"));
        assert!(t.event(1, NAME, "30687-06.htm").contains("30687-06.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fallen_orcs_drop_the_coins_for_a_royal_in_range() {
        let (mut t, vergara) = setup();
        t.enter(1, 55, vec![item(100, ROYAL_MEMBERSHIP, 1)]);
        assert!(t.quest_page(1, vergara).contains("30687-02.htm"));
        t.event(1, NAME, "30687-03.htm");
        t.take_writes();

        // A kill out of the alt party range gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                FALLEN_ORC,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(coins(&t), 0);

        // A Fallen Orc drops its basilisk (7.3%): hunt until one comes.
        for _ in 0..200 {
            if t.count(1, KAILS_SILVER_BASILISK) > 0 {
                break;
            }
            let orc = t.spawn(FALLEN_ORC, 60);
            t.kill(1, orc);
        }
        assert!(t.count(1, KAILS_SILVER_BASILISK) > 0);
        assert_eq!(t.count(1, KAILS_GOLD_GOLEM) + t.count(1, KAILS_BLOOD_DRAGON), 0);

        // The captain drops one of the three coins (6.9%), picked at random.
        let before = coins(&t);
        for _ in 0..300 {
            if coins(&t) > before {
                break;
            }
            let captain = t.spawn(FALLEN_ORC_CAPTAIN, 60);
            t.kill(1, captain);
        }
        assert!(coins(&t) > before);
        // The quest is never finished: no quest write for the coins.
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_killer_without_the_quest_state_gets_nothing() {
        let (mut t, _vergara) = setup();
        t.enter(1, 55, vec![item(100, ROYAL_MEMBERSHIP, 1)]);
        for _ in 0..20 {
            let shaman = t.spawn(FALLEN_ORC_SHAMAN, 60);
            t.kill(1, shaman);
        }
        assert_eq!(coins(&t), 0);
    }
}
