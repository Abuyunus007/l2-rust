//! Quest 10274: Collecting in the Air (Lekon sends the adventurer to pop the star stones).
// Java: dist/game/data/scripts/quests/Q10274_CollectingInTheAir/Q10274_CollectingInTheAir.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const LEKON: i32 = 32557;

// Items
const SCROLL: i32 = 13844;
const RED: i32 = 13858;
const BLUE: i32 = 13859;
const GREEN: i32 = 13860;
const REWARD: i32 = 13728;

// Monsters: the Red, Blue and Green Star Stones.
const MOBS: [i32; 9] = [18684, 18685, 18686, 18687, 18688, 18689, 18690, 18691, 18692];

/// Java `Q10273_GoodDayToFly.class.getSimpleName()`.
const Q10273: &str = "Q10273_GoodDayToFly";

/// Java `Q10274_CollectingInTheAir`.
pub struct Q10274CollectingInTheAir;

impl Script for Q10274CollectingInTheAir {
    fn name(&self) -> &'static str {
        "Q10274_CollectingInTheAir"
    }

    fn path(&self) -> &'static str {
        "quests/Q10274_CollectingInTheAir"
    }

    fn id(&self) -> i32 {
        10274
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LEKON]);
        s.add_talk_id(&[LEKON]);
        s.add_skill_see_id(&MOBS);
        s.register_quest_items(&[SCROLL, RED, BLUE, GREEN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        if event == "32557-03.html" {
            qs.start_quest(ctx);
            ctx.give_items(player, SCROLL, 8);
        }
        Some(event.to_owned())
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(caster, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }

        if qs.is_cond(ctx, 1) && skill.id == 2630 {
            match npc.id {
                18684..=18686 => ctx.give_items(caster, RED, 1),
                18687..=18689 => ctx.give_items(caster, BLUE, 1),
                18690..=18692 => ctx.give_items(caster, GREEN, 1),
                _ => {}
            }
            ctx.play_sound(caster, QuestSound::ItemsoundQuestItemget);
            ctx.do_die(npc, caster);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;

        match qs.state(ctx) {
            COMPLETED => htmltext = "32557-0a.html".to_owned(),
            CREATED => {
                htmltext = match ctx.player_quest_state(player, Q10273) {
                    Some(q) if ctx.player_level(player) >= 75 && q.is_completed() => "32557-01.htm",
                    _ => "32557-00.html",
                }
                .to_owned();
            }
            STARTED => {
                let stones = ctx.get_quest_items_count(player, RED)
                    + ctx.get_quest_items_count(player, BLUE)
                    + ctx.get_quest_items_count(player, GREEN);
                if stones >= 8 {
                    htmltext = "32557-05.html".to_owned();
                    ctx.give_items(player, REWARD, 1);
                    ctx.add_exp_and_sp(player, 25_160, 2_525);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = "32557-04.html".to_owned();
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
    use l2_world::script::COMPLETED;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10274_CollectingInTheAir";

    const NPCS: &str = r#"
        <npc id="32557" type="Folk" level="80" name="Lekon"/>
        <npc id="18684" type="Monster" level="60" name="Red Star Stone"/>
        <npc id="18687" type="Monster" level="60" name="Blue Star Stone"/>
        <npc id="18690" type="Monster" level="60" name="Green Star Stone"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13844" type="EtcItem" name="Scroll of Collecting"><set name="is_questitem" val="true"/></item>
        <item id="13858" type="EtcItem" name="Red Star Stone Fragment"><set name="is_questitem" val="true"/></item>
        <item id="13859" type="EtcItem" name="Blue Star Stone Fragment"><set name="is_questitem" val="true"/></item>
        <item id="13860" type="EtcItem" name="Green Star Stone Fragment"><set name="is_questitem" val="true"/></item>
        <item id="13728" type="EtcItem" name="Reward"/>"#;

    /// Java `Q10273_GoodDayToFly`, as far as this quest reads it: its state.
    struct GoodDayToFly;

    impl Script for GoodDayToFly {
        fn name(&self) -> &'static str {
            Q10273
        }
        fn path(&self) -> &'static str {
            "quests/Q10273_GoodDayToFly"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10274CollectingInTheAir);
        registry.add(GoodDayToFly);
        let pages = Pages::of(&Q10274CollectingInTheAir)
            .plain(&["32557-00.html", "32557-04.html", "32557-05.html", "32557-0a.html"])
            .page("32557-01.htm", &["32557-02.htm"])
            .page("32557-02.htm", &["32557-03.html"])
            .plain(&["32557-03.html"])
            .npcs(&[LEKON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lekon = t.spawn(LEKON, 20);
        (t, lekon)
    }

    /// Sets the state of Q10273 for player 1 (Good Day to Fly, the quest before this one).
    fn complete_good_day_to_fly(t: &mut ScriptTest) {
        t.world.run_as_script(Q10273, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
    }

    /// The star stone `npc` is hit by a skill of `skill_id`, cast by player 1 (Java `onSkillSee`).
    fn see_skill(t: &mut ScriptTest, npc: Oid, skill_id: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(stone) = ctx.script_npc(npc) {
                Q10274CollectingInTheAir.on_skill_see(
                    ctx,
                    stone,
                    Oid(1),
                    SkillKey::new(skill_id, 1),
                    &[],
                    false,
                );
            }
        });
    }

    #[test]
    fn the_quest_needs_good_day_to_fly_and_level_75() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, lekon).contains("32557-00.html"));
        complete_good_day_to_fly(&mut t);
        t.take_writes();
        t.enter(2, 74, vec![]);
        t.world.run_as_script(Q10273, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(2), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        assert!(t.quest_page(2, lekon).contains("32557-00.html"));
    }

    #[test]
    fn eight_star_stones_then_the_reward() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        complete_good_day_to_fly(&mut t);
        t.take_writes();
        assert!(t.quest_page(1, lekon).contains("32557-01.htm"));
        assert!(t.event(1, NAME, "32557-02.htm").contains("32557-02.htm"));
        assert!(t.event(1, NAME, "32557-03.html").contains("32557-03.html"));
        assert_eq!(t.count(1, SCROLL), 8);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // A red stone hit by the right skill gives a red fragment and dies; another skill does not.
        let red = t.spawn(18684, 60);
        see_skill(&mut t, red, 1);
        assert_eq!(t.count(1, RED), 0);
        see_skill(&mut t, red, 2630);
        assert_eq!(t.count(1, RED), 1);
        let mut dead = false;
        t.world.run_as_script(NAME, |ctx| dead = ctx.is_dead(red));
        assert!(dead);

        // Seven more stones: the reward needs eight.
        for _ in 0..3 {
            let stone = t.spawn(18687, 60);
            see_skill(&mut t, stone, 2630);
        }
        for _ in 0..3 {
            let stone = t.spawn(18690, 60);
            see_skill(&mut t, stone, 2630);
        }
        assert_eq!(t.count(1, BLUE), 3);
        assert!(t.quest_page(1, lekon).contains("32557-04.html"));
        let stone = t.spawn(18690, 60);
        see_skill(&mut t, stone, 2630);
        assert_eq!(t.count(1, GREEN), 4);

        // The reward: `exitQuest(false, true)` takes the fragments with the quest.
        assert!(t.quest_page(1, lekon).contains("32557-05.html"));
        assert_eq!(t.count(1, REWARD), 1);
        assert_eq!(t.count(1, RED) + t.count(1, BLUE) + t.count(1, GREEN), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, lekon).contains("32557-0a.html"));
    }
}
