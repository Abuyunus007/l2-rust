//! Quest 319: Scent of Death (Minaless in the Dwarven village, Marsh Zombies).
// Java: dist/game/data/scripts/quests/Q00319_ScentOfDeath/Q00319_ScentOfDeath.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`,
// "Quest patterns"). Unlike Q00295 the drop is rolled by hand (`getRandom(10) > 7`) and each
// skin is given with `giveItems`, with the item sound until the fifth one.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MINALESS: i32 = 30138;

// Monsters
const MARSH_ZOMBIE: i32 = 20015;
const MARSH_ZOMBIE_LORD: i32 = 20020;

// Item
const ZOMBIES_SKIN: i32 = 1045;
const LESSER_HEALING_POTION: i32 = 1060;

// Misc
const MIN_LEVEL: i32 = 11;
const MIN_CHANCE: i32 = 7;
const REQUIRED_ITEM_COUNT: i64 = 5;

/// Java `Q00319_ScentOfDeath`.
pub struct Q00319ScentOfDeath;

impl Script for Q00319ScentOfDeath {
    fn name(&self) -> &'static str {
        "Q00319_ScentOfDeath"
    }

    fn path(&self) -> &'static str {
        "quests/Q00319_ScentOfDeath"
    }

    fn id(&self) -> i32 {
        319
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MINALESS]);
        s.add_talk_id(&[MINALESS]);
        s.add_kill_id(&[MARSH_ZOMBIE, MARSH_ZOMBIE_LORD]);
        s.register_quest_items(&[ZOMBIES_SKIN]);
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
        if ctx.player_level(player) >= MIN_LEVEL && event == "30138-04.htm" {
            qs.start_quest(ctx);
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
            && ctx.get_quest_items_count(killer, ZOMBIES_SKIN) < REQUIRED_ITEM_COUNT
            && ctx.get_random(10) > MIN_CHANCE
        {
            ctx.give_items(killer, ZOMBIES_SKIN, 1);
            if ctx.get_quest_items_count(killer, ZOMBIES_SKIN) < REQUIRED_ITEM_COUNT {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            } else {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30138-03.htm"
            } else {
                "30138-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match qs.get_cond(ctx) {
                1 => {
                    htmltext = "30138-05.html".to_owned();
                }
                2 => {
                    ctx.give_adena(player, 3350, false);
                    ctx.give_items(player, LESSER_HEALING_POTION, 1);
                    ctx.take_items(player, ZOMBIES_SKIN, -1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30138-06.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00319_ScentOfDeath";

    const NPCS: &str = r#"
        <npc id="30138" type="Folk" level="1" name="Minaless"/>
        <npc id="20015" type="Monster" level="3" name="Marsh Zombie"/>
        <npc id="20020" type="Monster" level="4" name="Marsh Zombie Lord"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1045" type="EtcItem" name="Zombie's Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00319ScentOfDeath);
        let pages = Pages::of(&Q00319ScentOfDeath)
            .page("30138-03.htm", &["30138-04.htm"])
            .plain(&["30138-02.htm", "30138-04.htm", "30138-05.html", "30138-06.html"])
            .npcs(&[MINALESS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let minaless = t.spawn(MINALESS, 20);
        (t, minaless)
    }

    /// Kills Marsh Zombies until the quest is at cond 2; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let zombie = t.spawn(MARSH_ZOMBIE, 60);
            t.kill(1, zombie);
            if t.count(1, ZOMBIES_SKIN) >= REQUIRED_ITEM_COUNT {
                return kills;
            }
        }
        panic!("no 5 skins after 400 kills");
    }

    #[test]
    fn a_level_ten_character_is_sent_away() {
        let (mut t, minaless) = setup();
        t.enter(1, 10, vec![]);
        assert!(t.quest_page(1, minaless).contains("30138-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_skins_then_the_potion_and_adena() {
        let (mut t, minaless) = setup();
        t.enter(1, 11, vec![]);
        assert!(t.quest_page(1, minaless).contains("30138-03.htm"));
        assert!(t.event(1, NAME, "30138-04.htm").contains("30138-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, minaless).contains("30138-05.html"));

        // A kill out of range of the killer gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                MARSH_ZOMBIE,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, ZOMBIES_SKIN), 0);

        // The fifth skin sets cond 2 with the middle sound (`setCond(2, true)`).
        let kills = hunt(&mut t);
        assert!(kills >= 5, "{kills}");
        assert_eq!(t.count(1, ZOMBIES_SKIN), REQUIRED_ITEM_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: the potion, 3350 adena, the skins go, the quest ends repeatable.
        t.take();
        t.talk(1, minaless);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30138-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, LESSER_HEALING_POTION), 1);
        assert_eq!(t.count(1, 57), 3350);
        assert_eq!(t.count(1, ZOMBIES_SKIN), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
