//! Quest 461: Rumble in the Base (Stan in the Aden Castle Base, Shiny Salmon and Shoes String).
// Java: dist/game/data/scripts/quests/Q00461_RumbleInTheBase/Q00461_RumbleInTheBase.java
//
// Pattern: a daily kill quest. Each monster drops an item with a chance (the roll comes first,
// for every monster); the NPC pays the experience once a day. Java's `getRandomPartyMember(player,
// 1)` decides who gets the cond, while the item goes to the killer (NOTE(java-quirk) below).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const STAN: i32 = 30200;

// Items
const SHINY_SALMON: i32 = 15503;
const SHOES_STRING_OF_SEL_MAHUM: i32 = 16382;

// Monsters: (id, drop chance out of 1000), Java's `MONSTERS` map.
const MONSTERS: [(i32, i32); 7] = [
    (22780, 581),
    (22781, 772),
    (22782, 581),
    (22783, 563),
    (22784, 581),
    (22785, 271),
    (18908, 782),
];
/// The monster whose drop is the shiny salmon (`npc.getId() == 18908`).
const SALMON_MONSTER: i32 = 18908;

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00252_ItSmellsDelicious.class.getSimpleName()`: the prerequisite.
const IT_SMELLS_DELICIOUS: &str = "Q00252_ItSmellsDelicious";
const EXP: i64 = 224_784;
const SP: i32 = 342_528;

/// Java `Q00461_RumbleInTheBase`.
pub struct Q00461RumbleInTheBase;

impl Script for Q00461RumbleInTheBase {
    fn name(&self) -> &'static str {
        "Q00461_RumbleInTheBase"
    }

    fn path(&self) -> &'static str {
        "quests/Q00461_RumbleInTheBase"
    }

    fn id(&self) -> i32 {
        461
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STAN]);
        s.add_talk_id(&[STAN]);
        let mobs: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SHINY_SALMON, SHOES_STRING_OF_SEL_MAHUM]);
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
        if event.eq_ignore_ascii_case("30200-05.htm") {
            qs.start_quest(ctx);
            htmltext = Some(event.to_owned());
        } else if event.eq_ignore_ascii_case("30200-04.htm") {
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java rolls first, for every monster; a missing chance (never registered) never drops.
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if ctx.get_random(1000) >= chance {
            return;
        }
        if npc.id == SALMON_MONSTER {
            let qs = ctx.get_quest_state(killer, false);
            if let Some(qs) = qs
                && qs.is_cond(ctx, 1)
                && ctx.get_quest_items_count(killer, SHINY_SALMON) < 5
            {
                ctx.give_items(killer, SHINY_SALMON, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                if ctx.get_quest_items_count(killer, SHINY_SALMON) >= 5
                    && ctx.get_quest_items_count(killer, SHOES_STRING_OF_SEL_MAHUM) >= 10
                {
                    qs.set_cond(ctx, 2, true);
                }
            }
        } else {
            let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
                return;
            };
            let qs = ctx.get_quest_state(member, false);
            if ctx.get_quest_items_count(killer, SHOES_STRING_OF_SEL_MAHUM) < 10 {
                // NOTE(java-quirk): the shoes string goes to the killer, but the cond is set on
                // the state of the member drawn above.
                ctx.give_items(killer, SHOES_STRING_OF_SEL_MAHUM, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                if ctx.get_quest_items_count(killer, SHINY_SALMON) >= 5
                    && ctx.get_quest_items_count(killer, SHOES_STRING_OF_SEL_MAHUM) >= 10
                    && let Some(qs) = qs
                {
                    qs.set_cond(ctx, 2, true);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        // Java `player.getQuestState(Q00252...)`, read before the switch.
        let prev = ctx
            .player_quest_state(player, IT_SMELLS_DELICIOUS)
            .map(|prev| prev.state());
        let level_ok = ctx.player_level(player) >= MIN_LEVEL;
        match qs.state(ctx) {
            CREATED => {
                htmltext = if level_ok && prev == Some(COMPLETED) {
                    "30200-01.htm"
                } else {
                    "30200-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30200-06.html".to_owned();
                } else {
                    ctx.add_exp_and_sp(player, EXP, SP);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "30200-07.html".to_owned();
                }
            }
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "30200-03.htm".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if level_ok && prev == Some(COMPLETED) {
                        "30200-01.htm"
                    } else {
                        "30200-02.htm"
                    }
                    .to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00461_RumbleInTheBase";

    const NPCS: &str = r#"
        <npc id="30200" type="Folk" level="70" name="Stan"/>
        <npc id="22781" type="Monster" level="80" name="Monster A"/>
        <npc id="18908" type="Monster" level="80" name="Monster B"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15503" type="EtcItem" name="Shiny Salmon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="16382" type="EtcItem" name="Shoes String of Sel Mahum"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00252_ItSmellsDelicious` as far as this quest reads it: a script of that name.
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
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ItSmellsDelicious);
        registry.add(Q00461RumbleInTheBase);
        let pages = Pages::of(&Q00461RumbleInTheBase)
            .page("30200-01.htm", &["30200-04.htm"])
            .plain(&["30200-02.htm", "30200-03.htm", "30200-06.html", "30200-07.html"])
            .page("30200-04.htm", &["30200-05.htm"])
            .plain(&["30200-05.htm"])
            .npcs(&[STAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let stan = t.spawn(STAN, 20);
        (t, stan)
    }

    /// Makes the player's `Q00252` state completed, as the prerequisite of the quest.
    fn complete_prerequisite(t: &mut ScriptTest) {
        t.world.run_as_script(IT_SMELLS_DELICIOUS, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_prerequisite_and_the_level_are_needed() {
        let (mut t, stan) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, stan).contains("30200-02.htm"));
        t.take_writes();
        t.take();
        complete_prerequisite(&mut t);
        t.take();
        assert!(t.quest_page(1, stan).contains("30200-01.htm"));
    }

    #[test]
    fn a_level_81_character_is_sent_away_even_with_the_prerequisite() {
        let (mut t, stan) = setup();
        t.enter(1, 81, vec![]);
        complete_prerequisite(&mut t);
        assert!(t.quest_page(1, stan).contains("30200-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_salmon_and_the_shoes_then_the_daily_experience() {
        let (mut t, stan) = setup();
        t.enter(1, 82, vec![]);
        complete_prerequisite(&mut t);
        assert!(t.quest_page(1, stan).contains("30200-01.htm"));
        assert!(t.event(1, NAME, "30200-04.htm").contains("30200-04.htm"));
        assert!(t.event(1, NAME, "30200-05.htm").contains("30200-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, stan).contains("30200-06.html"));

        // Shoes come from the other monsters (the killer is the only party member at cond 1).
        for _ in 0..200 {
            if t.count(1, SHOES_STRING_OF_SEL_MAHUM) >= 10 {
                break;
            }
            let mob = t.spawn(22781, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, SHOES_STRING_OF_SEL_MAHUM), 10);
        assert_eq!(t.count(1, SHINY_SALMON), 0);
        assert!(t.take_writes().is_empty());

        // The salmon: the cond becomes 2 with the fifth one.
        for _ in 0..200 {
            if t.count(1, SHINY_SALMON) >= 5 {
                break;
            }
            let mob = t.spawn(SALMON_MONSTER, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, SHINY_SALMON), 5);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: the experience, `exitQuest(DAILY, true)`: the daily lock.
        assert_eq!(t.count(1, SHOES_STRING_OF_SEL_MAHUM), 10);
        assert!(t.quest_page(1, stan).contains("30200-07.html"));
        // `exitQuest` removes the registered quest items.
        assert_eq!(
            (t.count(1, SHINY_SALMON), t.count(1, SHOES_STRING_OF_SEL_MAHUM)),
            (0, 0)
        );
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(matches!(&writes[2], l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, stan).contains("30200-03.htm"));
    }
}
