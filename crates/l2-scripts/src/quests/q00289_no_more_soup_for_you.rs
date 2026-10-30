//! Quest 289: No More Soup For You (Stan in the Town of Gludin buys the soup of the sel mahum monsters).
// Java: dist/game/data/scripts/quests/Q00289_NoMoreSoupForYou/Q00289_NoMoreSoupForYou.java
//
// Pattern: a repeatable collect quest with a fixed drop (`docs/design/scripts.md`, "Quest
// patterns"): every sel mahum monster gives five soups; Stan trades 100 soups for one random armour
// piece or 500 soups for one random weapon. The quest never ends by itself. It needs the quest
// Q00252_ItSmellsDelicious completed (read through `player_quest_state`) and level 82.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const STAN: i32 = 30200;

// Item
const SOUP: i32 = 15712;

// Misc
const RATE: i64 = 5;
/// Java `Q00252_ItSmellsDelicious.class.getSimpleName()`: the quest that must be completed.
const IT_SMELLS_DELICIOUS: &str = "Q00252_ItSmellsDelicious";
const MIN_LEVEL: i32 = 82;

/// Java `MOBS`.
const MOBS: [i32; 5] = [18908, 22779, 22786, 22787, 22788];

/// Java `WEAPONS`: `(item, count)`.
const WEAPONS: [(i32, i64); 7] = [
    (10377, 1),
    (10401, 1),
    (10401, 2),
    (10401, 3),
    (10401, 4),
    (10401, 5),
    (10401, 6),
];

/// Java `ARMORS`: `(item, count)`.
const ARMORS: [(i32, i64); 18] = [
    (15812, 1),
    (15813, 1),
    (15814, 1),
    (15791, 1),
    (15787, 1),
    (15784, 1),
    (15781, 1),
    (15778, 1),
    (15775, 1),
    (15774, 5),
    (15773, 5),
    (15772, 5),
    (15693, 5),
    (15657, 5),
    (15654, 5),
    (15651, 5),
    (15648, 5),
    (15645, 5),
];

/// Java `Q00289_NoMoreSoupForYou`.
pub struct Q00289NoMoreSoupForYou;

impl Script for Q00289NoMoreSoupForYou {
    fn name(&self) -> &'static str {
        "Q00289_NoMoreSoupForYou"
    }

    fn path(&self) -> &'static str {
        "quests/Q00289_NoMoreSoupForYou"
    }

    fn id(&self) -> i32 {
        289
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STAN]);
        s.add_talk_id(&[STAN]);
        s.add_kill_id(&MOBS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // NOTE(java-quirk): without a quest state Java returns the event itself as the text.
        let mut htmltext = Some(event.to_owned());
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return htmltext;
        };
        // Java rolls both numbers for every event, before it looks at the event.
        let b = ctx.get_random(18) as usize;
        let c = ctx.get_random(7) as usize;
        if npc.is_some_and(|n| n.id == STAN) {
            if event.eq_ignore_ascii_case("30200-03.htm") {
                qs.start_quest(ctx);
            } else if event.eq_ignore_ascii_case("30200-05.htm") {
                if ctx.get_quest_items_count(player, SOUP) >= 500 {
                    let (item, count) = WEAPONS[c];
                    ctx.give_items(player, item, count);
                    ctx.take_items(player, SOUP, 500);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("30200-04.htm".to_owned());
                } else {
                    htmltext = Some("30200-07.htm".to_owned());
                }
            } else if event.eq_ignore_ascii_case("30200-06.htm") {
                if ctx.get_quest_items_count(player, SOUP) >= 100 {
                    let (item, count) = ARMORS[b];
                    ctx.give_items(player, item, count);
                    ctx.take_items(player, SOUP, 100);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("30200-04.htm".to_owned());
                } else {
                    htmltext = Some("30200-07.htm".to_owned());
                }
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        if MOBS.contains(&npc.id) {
            ctx.give_items(killer, SOUP, RATE);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == STAN {
            match qs.state(ctx) {
                CREATED => {
                    let completed = ctx
                        .player_quest_state(player, IT_SMELLS_DELICIOUS)
                        .is_some_and(|prev| prev.is_completed());
                    htmltext = if completed && ctx.player_level(player) >= MIN_LEVEL {
                        "30200-01.htm"
                    } else {
                        "30200-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = if ctx.get_quest_items_count(player, SOUP) >= 100 {
                            "30200-04.htm"
                        } else {
                            "30200-03.htm"
                        }
                        .to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00289_NoMoreSoupForYou";

    /// The stand-in of Q00252: an NPC of its own, and the quest completed when talked to.
    const STAND_IN_NPC: i32 = 30203;

    const NPCS: &str = r#"
        <npc id="30200" type="Folk" level="85" name="Stan"/>
        <npc id="30203" type="Folk" level="70" name="Stand-in"/>
        <npc id="18908" type="Monster" level="80" name="Sel Mahum Drill Sergeant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15712" type="EtcItem" name="Soup"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15812" type="EtcItem" name="Material 15812"/>
        <item id="15813" type="EtcItem" name="Material 15813"/>
        <item id="15814" type="EtcItem" name="Material 15814"/>
        <item id="15791" type="EtcItem" name="Material 15791"/>
        <item id="15787" type="EtcItem" name="Material 15787"/>
        <item id="15784" type="EtcItem" name="Material 15784"/>
        <item id="15781" type="EtcItem" name="Material 15781"/>
        <item id="15778" type="EtcItem" name="Material 15778"/>
        <item id="15775" type="EtcItem" name="Material 15775"/>
        <item id="15774" type="EtcItem" name="Material 15774"/>
        <item id="15773" type="EtcItem" name="Material 15773"/>
        <item id="15772" type="EtcItem" name="Material 15772"/>
        <item id="15693" type="EtcItem" name="Material 15693"/>
        <item id="15657" type="EtcItem" name="Material 15657"/>
        <item id="15654" type="EtcItem" name="Material 15654"/>
        <item id="15651" type="EtcItem" name="Material 15651"/>
        <item id="15648" type="EtcItem" name="Material 15648"/>
        <item id="15645" type="EtcItem" name="Material 15645"/>
        <item id="10401" type="Weapon" name="Weapon A"><set name="bodypart" val="rhand"/></item>"#;

    /// Java `Q00252_ItSmellsDelicious` as far as this quest needs it: its NPC completes it.
    struct ItSmellsDelicious;

    impl Script for ItSmellsDelicious {
        fn name(&self) -> &'static str {
            IT_SMELLS_DELICIOUS
        }
        fn path(&self) -> &'static str {
            "quests/Q00252_ItSmellsDelicious"
        }
        fn id(&self) -> i32 {
            252
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[STAND_IN_NPC]);
            s.add_talk_id(&[STAND_IN_NPC]);
        }
        fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
            if let Some(qs) = ctx.get_quest_state(player, true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
            None
        }
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ItSmellsDelicious);
        registry.add(Q00289NoMoreSoupForYou);
        let pages = Pages::of(&Q00289NoMoreSoupForYou)
            .page("30200-01.htm", &["30200-02.htm"])
            .page("30200-02.htm", &["30200-03.htm"])
            .plain(&["30200-00.htm", "30200-03.htm", "30200-07.htm"])
            .page("30200-04.htm", &["30200-05.htm", "30200-06.htm"])
            .npcs(&[STAN, STAND_IN_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let stan = t.spawn(STAN, 20);
        let stand_in = t.spawn(STAND_IN_NPC, 30);
        (t, stan, stand_in)
    }

    /// Kills the sel mahum monsters until the soup count reaches `soups`.
    fn hunt(t: &mut ScriptTest, soups: i64) {
        for _ in 0..200 {
            if t.count(1, SOUP) >= soups {
                return;
            }
            let monster = t.spawn(18908, 60);
            t.kill(1, monster);
        }
        panic!("no {soups} soups after 200 kills");
    }

    #[test]
    fn without_the_it_smells_delicious_quest_stan_says_no() {
        let (mut t, stan, _stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, stan).contains("30200-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn soup_for_an_armour_piece_and_the_quest_goes_on() {
        let (mut t, stan, stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, stand_in);
        t.take_writes();

        assert!(t.quest_page(1, stan).contains("30200-01.htm"));
        assert!(t.event(1, NAME, "30200-02.htm").contains("30200-02.htm"));
        assert!(t.event(1, NAME, "30200-03.htm").contains("30200-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Five soups a monster; 100 buy an armour piece.
        assert!(t.quest_page(1, stan).contains("30200-03.htm"));
        hunt(&mut t, 100);
        assert!(t.quest_page(1, stan).contains("30200-04.htm"));
        assert!(t.event(1, NAME, "30200-06.htm").contains("30200-04.htm"));
        assert_eq!(t.count(1, SOUP), 0);
        let armour: i64 = ARMORS.iter().map(|&(item, _)| t.count(1, item)).sum();
        assert!(armour >= 1, "{armour}");
        assert!(t.take_writes().is_empty());

        // Less than 500 soups: the weapon exchange is refused with page 07.
        hunt(&mut t, 100);
        assert!(t.quest_page(1, stan).contains("30200-04.htm"));
        assert!(t.event(1, NAME, "30200-05.htm").contains("30200-07.htm"));
        assert_eq!(t.count(1, SOUP), 100);
        assert!(t.take_writes().is_empty());
    }
}
