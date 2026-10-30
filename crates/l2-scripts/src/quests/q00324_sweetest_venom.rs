//! Quest 324: Sweetest Venom (Astaron in the Dark Elven village, Ratmen-type venom monsters).
// Java: dist/game/data/scripts/quests/Q00324_SweetestVenom/Q00324_SweetestVenom.java
//
// Pattern: a kill-count quest with a drop chance per monster (`docs/design/scripts.md`,
// "Quest patterns"): the chance is a percentage per monster id, the sac is given with `giveItems`
// and the sound until the tenth one.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ASTARON: i32 = 30351;

// Monsters (id, drop chance in percent)
const MONSTERS: [(i32, i32); 3] = [(20034, 26), (20038, 29), (20043, 30)];

// Items
const VENOM_SAC: i32 = 1077;

// Misc
const MIN_LEVEL: i32 = 18;
const REQUIRED_COUNT: i64 = 10;
const ADENA_COUNT: i64 = 5810;

/// Java `Q00324_SweetestVenom`.
pub struct Q00324SweetestVenom;

impl Script for Q00324SweetestVenom {
    fn name(&self) -> &'static str {
        "Q00324_SweetestVenom"
    }

    fn path(&self) -> &'static str {
        "quests/Q00324_SweetestVenom"
    }

    fn id(&self) -> i32 {
        324
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASTARON]);
        s.add_talk_id(&[ASTARON]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[VENOM_SAC]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        let mut htmltext = None;
        if let Some(qs) = qs
            && event == "30351-04.htm"
        {
            qs.start_quest(ctx);
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) < MIN_LEVEL {
                "30351-02.html"
            } else {
                "30351-03.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if qs.is_cond(ctx, 2) {
                ctx.give_adena(player, ADENA_COUNT, true);
                qs.exit_quest(ctx, true, true);
                htmltext = "30351-06.html".to_owned();
            } else {
                htmltext = "30351-05.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): no `checkIfInRange` here (unlike most kill quests): the killer gets the
        // sac wherever the monster died.
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            let mut sacs = ctx.get_quest_items_count(player, VENOM_SAC);
            // Only the registered monsters reach here; any other id never drops.
            let chance = MONSTERS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0, |(_, chance)| *chance);
            if sacs < REQUIRED_COUNT && ctx.get_random(100) < chance {
                ctx.give_items(player, VENOM_SAC, 1);
                sacs += 1;
                if sacs < REQUIRED_COUNT {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                } else {
                    qs.set_cond(ctx, 2, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00324_SweetestVenom";

    const NPCS: &str = r#"
        <npc id="30351" type="Folk" level="1" name="Astaron"/>
        <npc id="20034" type="Monster" level="16" name="Ratman Warrior"/>
        <npc id="20038" type="Monster" level="17" name="Ratman Shaman"/>
        <npc id="20043" type="Monster" level="18" name="Ratman Chief"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1077" type="EtcItem" name="Venom Sac"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00324SweetestVenom);
        let pages = Pages::of(&Q00324SweetestVenom)
            .page("30351-03.htm", &["30351-04.htm"])
            .plain(&["30351-02.html", "30351-04.htm", "30351-05.html", "30351-06.html"])
            .npcs(&[ASTARON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let astaron = t.spawn(ASTARON, 20);
        (t, astaron)
    }

    /// Kills the monster until the sacs are at the limit; returns the number of kills.
    fn hunt(t: &mut ScriptTest, monster: i32) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(monster, 60);
            t.kill(1, mob);
            if t.count(1, VENOM_SAC) >= REQUIRED_COUNT {
                return kills;
            }
        }
        panic!("no 10 sacs after 400 kills");
    }

    #[test]
    fn a_level_seventeen_character_is_sent_away() {
        let (mut t, astaron) = setup();
        t.enter(1, 17, vec![]);
        assert!(t.quest_page(1, astaron).contains("30351-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_sacs_then_adena() {
        let (mut t, astaron) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, astaron).contains("30351-03.htm"));
        assert!(t.event(1, NAME, "30351-04.htm").contains("30351-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, astaron).contains("30351-05.html"));

        // Ratman Warriors (26%); the tenth sac sets cond 2 with the middle sound.
        let kills = hunt(&mut t, 20034);
        assert!(kills >= 10, "{kills}");
        assert_eq!(t.count(1, VENOM_SAC), REQUIRED_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: 5810 adena, the sacs go, the quest ends repeatable.
        t.take();
        t.talk(1, astaron);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30351-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), ADENA_COUNT);
        assert_eq!(t.count(1, VENOM_SAC), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
