//! Quest 251: No Secrets (Pinaps in the Dark Elven village collects diaries and tables from the
//! monsters of the Ant and Shilen areas).
// Java: dist/game/data/scripts/quests/Q00251_NoSecrets/Q00251_NoSecrets.java
//
// Pattern: a kill count with two drop chances and two item limits (`docs/design/scripts.md`,
// "Kills, drops and parties"). The drops are rolled by hand as Java does, not by
// `giveItemRandomly`, so the order of the rolls is Java's.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PINAPS: i32 = 30201;

// Items
const DIARY: i32 = 15508;
const TABLE: i32 = 15509;

// Monsters: the diary drops, the table drops
const MOBS: [i32; 5] = [22783, 22785, 22780, 22782, 22784];
const MOBS2: [i32; 3] = [22775, 22776, 22778];

/// Java `Q00251_NoSecrets`.
pub struct Q00251NoSecrets;

impl Script for Q00251NoSecrets {
    fn name(&self) -> &'static str {
        "Q00251_NoSecrets"
    }

    fn path(&self) -> &'static str {
        "quests/Q00251_NoSecrets"
    }

    fn id(&self) -> i32 {
        251
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PINAPS]);
        s.add_talk_id(&[PINAPS]);
        s.add_kill_id(&MOBS);
        s.add_kill_id(&MOBS2);
        s.register_quest_items(&[DIARY, TABLE]);
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
        if event == "30201-03.htm" {
            qs.start_quest(ctx);
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && qs.is_cond(ctx, 1) {
            if MOBS.contains(&npc.id)
                && ctx.get_random(100) < 10
                && ctx.get_quest_items_count(killer, DIARY) < 10
            {
                ctx.give_items(killer, DIARY, 1);
                if ctx.get_quest_items_count(killer, DIARY) >= 10
                    && ctx.get_quest_items_count(killer, TABLE) >= 5
                {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            } else if MOBS2.contains(&npc.id)
                && ctx.get_random(100) < 5
                && ctx.get_quest_items_count(killer, TABLE) < 5
            {
                ctx.give_items(killer, TABLE, 1);
                if ctx.get_quest_items_count(killer, DIARY) >= 10
                    && ctx.get_quest_items_count(killer, TABLE) >= 5
                {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) > 81 {
                    "30201-01.htm"
                } else {
                    "30201-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30201-05.htm".to_owned();
                } else if qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, DIARY) >= 10
                    && ctx.get_quest_items_count(player, TABLE) >= 5
                {
                    htmltext = "30201-04.htm".to_owned();
                    ctx.give_adena(player, 313355, true);
                    ctx.add_exp_and_sp(player, 56787, 160578);
                    qs.exit_quest(ctx, false, true);
                }
            }
            COMPLETED => htmltext = "30201-06.htm".to_owned(),
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

    const NAME: &str = "Q00251_NoSecrets";

    const NPCS: &str = r#"
        <npc id="30201" type="Folk" level="70" name="Pinaps"/>
        <npc id="22783" type="Monster" level="70" name="Diary Monster"/>
        <npc id="22785" type="Monster" level="70" name="Diary Monster"/>
        <npc id="22780" type="Monster" level="70" name="Diary Monster"/>
        <npc id="22782" type="Monster" level="70" name="Diary Monster"/>
        <npc id="22784" type="Monster" level="70" name="Diary Monster"/>
        <npc id="22775" type="Monster" level="70" name="Table Monster"/>
        <npc id="22776" type="Monster" level="70" name="Table Monster"/>
        <npc id="22778" type="Monster" level="70" name="Table Monster"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15508" type="EtcItem" name="Diary"><set name="is_questitem" val="true"/></item>
        <item id="15509" type="EtcItem" name="Table"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00251NoSecrets);
        let pages = Pages::of(&Q00251NoSecrets)
            .page("30201-01.htm", &["30201-02.htm"])
            .page("30201-02.htm", &["30201-03.htm"])
            .plain(&[
                "30201-00.htm",
                "30201-03.htm",
                "30201-04.htm",
                "30201-05.htm",
                "30201-06.htm",
            ])
            .npcs(&[30201]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let pinaps = t.spawn(30201, 20);
        (t, pinaps)
    }

    /// Kills monsters of `mobs` (one spawn each) until the player has `diaries` diaries or
    /// `tables` tables, as the players did. The world RNG is seeded, so the count is the same
    /// every run.
    fn kill_until(t: &mut ScriptTest, mobs: &[i32], diaries: i64, tables: i64) {
        for i in 0..2_000 {
            let done = t.count(1, DIARY) >= diaries && t.count(1, TABLE) >= tables;
            if done {
                return;
            }
            let mob = t.spawn(mobs[i % mobs.len()], 20);
            t.kill(1, mob);
        }
        panic!("the drops did not come in 2000 kills");
    }

    #[test]
    fn a_character_at_level_81_is_sent_away() {
        let (mut t, pinaps) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, pinaps).contains("30201-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_from_start_to_the_reward() {
        let (mut t, pinaps) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, pinaps).contains("30201-01.htm"));
        assert!(t.event(1, NAME, "30201-02.htm").contains("30201-02.htm"));
        assert!(t.event(1, NAME, "30201-03.htm").contains("30201-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, pinaps).contains("30201-05.htm"));

        // Diaries from the first group, tables from the second: the last one sets cond 2.
        kill_until(&mut t, &MOBS, 10, 0);
        assert_eq!(t.count(1, DIARY), 10);
        kill_until(&mut t, &MOBS2, 10, 5);
        assert_eq!((t.count(1, DIARY), t.count(1, TABLE)), (10, 5));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Pinaps takes the items and pays.
        assert!(t.quest_page(1, pinaps).contains("30201-04.htm"));
        assert_eq!(t.count(1, 57), 313355);
        // `exitQuest` removes the registered quest items.
        assert_eq!((t.count(1, DIARY), t.count(1, TABLE)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, pinaps).contains("30201-06.htm"));
    }
}
