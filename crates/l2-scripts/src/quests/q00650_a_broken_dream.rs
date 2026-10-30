//! Quest 650: A Broken Dream (the Ghost of a Railroad Engineer in the Rune Town station; the
//! forgotten crewmen and vagabonds of the ruins drop the remnants of an old dwarf's dream).
// Java: dist/game/data/scripts/quests/Q00650_ABrokenDream/Q00650_ABrokenDream.java
//
// Pattern: a kill drop that goes to one started player picked from a list (the killer twice,
// every started party member once), the start gate of Q00117 read through `player_quest_state`,
// and the exit that pays nothing.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GHOST_OF_A_RAILROAD_ENGINEER: i32 = 32054;

// Item
const REMNANTS_OF_OLD_DWARVES_DREAMS: i32 = 8514;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00117_TheOceanOfDistantStars.class.getSimpleName()`.
const THE_OCEAN_OF_DISTANT_STARS: &str = "Q00117_TheOceanOfDistantStars";

/// Java `MONSTER_DROP_CHANCES`: the drop chance per mille of each monster.
const MONSTER_DROP_CHANCES: [(i32, i32); 2] = [
    (22027, 575), // Forgotten Crewman
    (22028, 515), // Vagabond of the Ruins
];

/// Java `Q00650_ABrokenDream`.
pub struct Q00650ABrokenDream;

impl Script for Q00650ABrokenDream {
    fn name(&self) -> &'static str {
        "Q00650_ABrokenDream"
    }

    fn path(&self) -> &'static str {
        "quests/Q00650_ABrokenDream"
    }

    fn id(&self) -> i32 {
        650
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GHOST_OF_A_RAILROAD_ENGINEER]);
        s.add_talk_id(&[GHOST_OF_A_RAILROAD_ENGINEER]);
        let mobs: Vec<i32> = MONSTER_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[REMNANTS_OF_OLD_DWARVES_DREAMS]);
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
            "32054-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32054-07.html" | "32054-08.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32054-09.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if ctx.player_level(player) < MIN_LEVEL {
                htmltext = "32054-02.htm".to_owned();
            } else {
                let completed = ctx
                    .player_quest_state(player, THE_OCEAN_OF_DISTANT_STARS)
                    .is_some_and(|q117| q117.state() == COMPLETED);
                htmltext = if completed {
                    "32054-01.htm".to_owned()
                } else {
                    "32054-04.htm".to_owned()
                };
            }
        } else if state == STARTED {
            htmltext = if ctx.has_quest_items(player, REMNANTS_OF_OLD_DWARVES_DREAMS) {
                "32054-05.html".to_owned()
            } else {
                "32054-06.html".to_owned()
            };
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `randomList`: the killer twice when started, then every started party member
        // (the killer again when it is in a party: `getMembers()` holds it too).
        let mut random_list: Vec<ObjectId> = Vec::new();
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_started(ctx)
        {
            random_list.push(killer);
            random_list.push(killer);
        }
        let Some(&(_, monster_chance)) = MONSTER_DROP_CHANCES.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        if ctx.is_in_party(killer) {
            for member in ctx.world().party_members_of(killer) {
                if let Some(qst) = ctx.get_quest_state(member, false)
                    && qst.is_started(ctx)
                {
                    random_list.push(member);
                }
            }
        }
        if random_list.is_empty() {
            return;
        }
        let index = ctx.get_random(random_list.len() as i32);
        let player = random_list[index as usize];
        // Java: the roll comes first, the range check second.
        if ctx.get_random(1000) < monster_chance
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, true)
        {
            ctx.give_items(player, REMNANTS_OF_OLD_DWARVES_DREAMS, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::COMPLETED;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00650_ABrokenDream";

    const NPCS: &str = r#"
        <npc id="32054" type="Folk" level="70" name="Ghost of a Railroad Engineer"/>
        <npc id="22027" type="Monster" level="40" name="Forgotten Crewman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8514" type="EtcItem" name="Remnants of Old Dwarves Dreams"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00117_TheOceanOfDistantStars`, as far as this quest reads it.
    struct OceanOfDistantStars;

    impl Script for OceanOfDistantStars {
        fn name(&self) -> &'static str {
            THE_OCEAN_OF_DISTANT_STARS
        }
        fn path(&self) -> &'static str {
            "quests/Q00117_TheOceanOfDistantStars"
        }
        fn id(&self) -> i32 {
            117
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup(q117_done: bool, level: i32) -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(OceanOfDistantStars);
        registry.add(Q00650ABrokenDream);
        let pages = Pages::of(&Q00650ABrokenDream)
            .page("32054-01.htm", &["32054-03.htm"])
            .plain(&[
                "32054-02.htm",
                "32054-03.htm",
                "32054-04.htm",
                "32054-06.html",
                "32054-08.html",
                "32054-09.html",
            ])
            .page("32054-05.html", &["32054-07.html"])
            .page("32054-07.html", &["32054-08.html", "32054-09.html"])
            .npcs(&[GHOST_OF_A_RAILROAD_ENGINEER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ghost = t.spawn(GHOST_OF_A_RAILROAD_ENGINEER, 20);
        t.enter(1, level, vec![]);
        if q117_done {
            t.world.run_as_script(THE_OCEAN_OF_DISTANT_STARS, |ctx| {
                let qs = ctx.get_quest_state(Oid(1), true).unwrap();
                qs.set_state(ctx, COMPLETED);
            });
            t.take_writes();
        }
        (t, ghost)
    }

    /// A forgotten crewman (drop chance 575 per mille) dies next to the player.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(22027, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_39_is_sent_away() {
        let (mut t, ghost) = setup(true, 38);
        assert!(t.quest_page(1, ghost).contains("32054-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_ocean_of_distant_stars_the_ghost_says_no() {
        let (mut t, ghost) = setup(false, 39);
        assert!(t.quest_page(1, ghost).contains("32054-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_remnants_come_from_the_kills_and_the_exit_pays_nothing() {
        let (mut t, ghost) = setup(true, 39);
        assert!(t.quest_page(1, ghost).contains("32054-01.htm"));
        assert!(t.event(1, NAME, "32054-03.htm").contains("32054-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ghost).contains("32054-06.html"));

        let mut kills = 0;
        while t.count(1, REMNANTS_OF_OLD_DWARVES_DREAMS) < 1 {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 50, "no remnants dropped");
        }
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, ghost).contains("32054-05.html"));
        assert!(t.event(1, NAME, "32054-07.html").contains("32054-07.html"));
        assert!(t.event(1, NAME, "32054-09.html").contains("32054-09.html"));
        // The exit takes the registered remnants back and pays no adena.
        assert_eq!(t.count(1, REMNANTS_OF_OLD_DWARVES_DREAMS), 0);
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
