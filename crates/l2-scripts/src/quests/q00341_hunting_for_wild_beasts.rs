//! Quest 341: Hunting for Wild Beasts (Pano the grocer in Dion, bear pelts).
// Java: dist/game/data/scripts/quests/Q00341_HuntingForWildBeasts/Q00341_HuntingForWildBeasts.java
//
// Pattern: a kill-drop quest with a per-monster chance up to a limit; the limit moves to cond 2
// and the NPC pays once for the pelts.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PANO: i32 = 30078;

// Monsters: (id, drop chance in percent), Java's `MONSTERS` map.
const MONSTERS: [(i32, i32); 4] = [(20203, 99), (20310, 87), (20021, 83), (20335, 87)];

// Item
const BEAR_SKIN: i32 = 4259;

// Misc
const MIN_LEVEL: i32 = 20;
const ADENA_COUNT: i64 = 3710;
const REQUIRED_COUNT: i64 = 20;

/// Java `MONSTERS.get(npc.getId())`.
fn chance_of(npc_id: i32) -> i32 {
    MONSTERS
        .iter()
        .find(|(id, _)| *id == npc_id)
        .map_or(0, |(_, chance)| *chance)
}

/// Java `Q00341_HuntingForWildBeasts`.
pub struct Q00341HuntingForWildBeasts;

impl Script for Q00341HuntingForWildBeasts {
    fn name(&self) -> &'static str {
        "Q00341_HuntingForWildBeasts"
    }

    fn path(&self) -> &'static str {
        "quests/Q00341_HuntingForWildBeasts"
    }

    fn id(&self) -> i32 {
        341
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PANO]);
        s.add_talk_id(&[PANO]);
        let ids: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[BEAR_SKIN]);
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
            "30078-03.htm" => htmltext = Some(event.to_owned()),
            "30078-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30078-01.html"
                } else {
                    "30078-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, BEAR_SKIN) >= REQUIRED_COUNT {
                    ctx.give_adena(player, ADENA_COUNT, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30078-05.html".to_owned();
                } else {
                    htmltext = "30078-06.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            let mut skins = ctx.get_quest_items_count(killer, BEAR_SKIN);
            if skins < REQUIRED_COUNT && ctx.get_random(100) < chance_of(npc.id) {
                ctx.give_items(killer, BEAR_SKIN, 1);
                skins += 1;
                if skins < REQUIRED_COUNT {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00341_HuntingForWildBeasts";

    const NPCS: &str = r#"
        <npc id="30078" type="Folk" level="30" name="Pano"/>
        <npc id="20203" type="Monster" level="30" name="Dion Grizzly"/>
        <npc id="20310" type="Monster" level="30" name="Red Bear"/>
        <npc id="20021" type="Monster" level="30" name="Grizzly Bear"/>
        <npc id="20335" type="Monster" level="30" name="Brown Bear"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4259" type="EtcItem" name="Bear Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00341HuntingForWildBeasts);
        let pages = Pages::of(&Q00341HuntingForWildBeasts)
            .page("30078-01.html", &["30078-03.htm"])
            .plain(&["30078-02.htm", "30078-04.htm", "30078-05.html", "30078-06.html"])
            .page("30078-03.htm", &["30078-04.htm"])
            .npcs(&[PANO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let pano = t.spawn(PANO, 20);
        (t, pano)
    }

    /// Kills the bears (rotating over the four monsters) until `skins` pelts are in the bag.
    fn hunt(t: &mut ScriptTest, skins: i64) {
        let ids = MONSTERS.map(|(id, _)| id);
        for kill in 0..400 {
            if t.count(1, BEAR_SKIN) >= skins {
                return;
            }
            let bear = t.spawn(ids[kill % ids.len()], 60);
            t.kill(1, bear);
        }
        panic!("no {skins} skins after 400 kills");
    }

    #[test]
    fn a_character_below_level_twenty_is_sent_away() {
        let (mut t, pano) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, pano).contains("30078-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hunt_starts_at_level_twenty() {
        let (mut t, pano) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, pano).contains("30078-01.html"));
        assert!(t.event(1, NAME, "30078-03.htm").contains("30078-03.htm"));
        assert!(t.event(1, NAME, "30078-04.htm").contains("30078-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, pano).contains("30078-06.html"));
    }

    #[test]
    fn twenty_pelts_then_the_adena_and_the_quest_ends() {
        let (mut t, pano) = setup();
        t.enter(1, 20, vec![]);
        t.quest_page(1, pano);
        t.event(1, NAME, "30078-03.htm");
        t.event(1, NAME, "30078-04.htm");
        t.take_writes();

        // Nineteen pelts: still cond 1, no cond write yet.
        hunt(&mut t, 19);
        assert_eq!(t.count(1, BEAR_SKIN), 19);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, pano).contains("30078-06.html"));

        // The twentieth pelt sets cond 2 with the middle sound.
        hunt(&mut t, 20);
        assert_eq!(t.count(1, BEAR_SKIN), 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // More bears after cond 2 give nothing.
        let bear = t.spawn(20203, 60);
        t.kill(1, bear);
        assert_eq!(t.count(1, BEAR_SKIN), 20);

        // The reward: 3710 adena; `exitQuest(true, true)` takes the registered pelts with the quest.
        t.talk(1, pano);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30078-05.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), 3710);
        assert_eq!(t.count(1, BEAR_SKIN), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_without_the_quest_gives_nothing() {
        let (mut t, _pano) = setup();
        t.enter(1, 20, vec![]);
        let bear = t.spawn(20310, 60);
        t.kill(1, bear);
        assert_eq!(t.count(1, BEAR_SKIN), 0);
        assert!(t.take_writes().is_empty());
    }
}
