//! Quest 132: Matras' Curiosity (Matras sends a level 76 character after the two blueprint bosses).
// Java: dist/game/data/scripts/quests/Q00132_MatrasCuriosity/Q00132_MatrasCuriosity.java
//
// Pattern: a party kill quest with a random member. Each boss (Ranku, Demon Prince) gives its
// blueprint to a random party member whose `rewarded_*` variable is still "1" (the variables are
// set at the start); that member's variable goes to "2". With both blueprints in hand the quest
// moves to cond 3, and Matras gives the reward and ends the quest (one-time).

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MATRAS: i32 = 32245;

// Monsters
const DEMON_PRINCE: i32 = 25540;
const RANKU: i32 = 25542;

// Items
const FIRE: i32 = 10521;
const WATER: i32 = 10522;
const EARTH: i32 = 10523;
const WIND: i32 = 10524;
const DARKNESS: i32 = 10525;
const DIVINITY: i32 = 10526;
const BLUEPRINT_RANKU: i32 = 9800;
const BLUEPRINT_PRINCE: i32 = 9801;

// Misc
const MIN_LEVEL: i32 = 76;

/// Java `Q00132_MatrasCuriosity`.
pub struct Q00132MatrasCuriosity;

impl Script for Q00132MatrasCuriosity {
    fn name(&self) -> &'static str {
        "Q00132_MatrasCuriosity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00132_MatrasCuriosity"
    }

    fn id(&self) -> i32 {
        132
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MATRAS]);
        s.add_talk_id(&[MATRAS]);
        s.add_kill_id(&[RANKU, DEMON_PRINCE]);
        s.register_quest_items(&[BLUEPRINT_RANKU, BLUEPRINT_PRINCE]);
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
        let mut htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("32245-03.htm")
            && ctx.player_level(player) >= MIN_LEVEL
            && !qs.is_completed(ctx)
        {
            if qs.is_created(ctx) {
                qs.start_quest(ctx);
                qs.set(ctx, "rewarded_prince", "1");
                qs.set(ctx, "rewarded_ranku", "1");
            } else {
                htmltext = "32245-03a.htm".to_owned();
            }
        } else if event.eq_ignore_ascii_case("32245-07.htm") && qs.is_cond(ctx, 3) && !qs.is_completed(ctx) {
            ctx.give_adena(player, 65_884, true);
            ctx.add_exp_and_sp(player, 50_541, 5_094);
            ctx.give_items(player, FIRE, 1);
            ctx.give_items(player, WATER, 1);
            ctx.give_items(player, EARTH, 1);
            ctx.give_items(player, WIND, 1);
            ctx.give_items(player, DARKNESS, 1);
            ctx.give_items(player, DIVINITY, 1);
            qs.exit_quest(ctx, false, true);
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        // Java: the blueprint goes to the killer (`player`) while the variable and the cond go to
        // the random member `pl`.
        // NOTE(java-quirk): Java gives the blueprint to the killer, not to the member drawn.
        let (var, blueprint, other_blueprint) = match npc.id {
            DEMON_PRINCE => ("rewarded_prince", BLUEPRINT_PRINCE, BLUEPRINT_RANKU),
            RANKU => ("rewarded_ranku", BLUEPRINT_RANKU, BLUEPRINT_PRINCE),
            _ => return,
        };
        let Some(pl) = ctx.get_random_party_member_var(player, Some(var), "1") else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(pl, false) else {
            return;
        };
        ctx.give_items(player, blueprint, 1);
        qs.set(ctx, var, "2");
        if ctx.has_quest_items(player, other_blueprint) {
            qs.set_cond(ctx, 2, true);
        } else {
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32245-01.htm".to_owned()
            } else {
                "32245-02.htm".to_owned()
            };
        } else if qs.is_completed(ctx) {
            htmltext = ctx.get_already_completed_msg(player);
        } else if qs.is_started(ctx) {
            match qs.get_cond(ctx) {
                1 | 2 => {
                    if ctx.has_quest_items(player, BLUEPRINT_RANKU)
                        && ctx.has_quest_items(player, BLUEPRINT_PRINCE)
                    {
                        ctx.take_items(player, BLUEPRINT_RANKU, -1);
                        ctx.take_items(player, BLUEPRINT_PRINCE, -1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "32245-05.htm".to_owned();
                    } else {
                        htmltext = "32245-04.htm".to_owned();
                    }
                }
                3 => htmltext = "32245-06.htm".to_owned(),
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00132_MatrasCuriosity";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="32245" type="Folk" level="1" name="Matras"/>
        <npc id="25540" type="RaidBoss" level="80" name="Demon Prince"/>
        <npc id="25542" type="RaidBoss" level="80" name="Ranku"/>"#;

    const ITEMS: &str = r#"
        <item id="9800" type="EtcItem" name="Blueprint: Ranku"><set name="is_questitem" val="true"/></item>
        <item id="9801" type="EtcItem" name="Blueprint: Demon Prince"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10521" type="EtcItem" name="Fire"/>
        <item id="10522" type="EtcItem" name="Water"/>
        <item id="10523" type="EtcItem" name="Earth"/>
        <item id="10524" type="EtcItem" name="Wind"/>
        <item id="10525" type="EtcItem" name="Darkness"/>
        <item id="10526" type="EtcItem" name="Divinity"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00132MatrasCuriosity);
        let pages = Pages::of(&Q00132MatrasCuriosity)
            .page("32245-01.htm", &["32245-03.htm"])
            .page("32245-06.htm", &["32245-07.htm"])
            .plain(&[
                "32245-02.htm",
                "32245-03.htm",
                "32245-03a.htm",
                "32245-04.htm",
                "32245-05.htm",
                "32245-07.htm",
            ])
            .npcs(&[MATRAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let matras = t.spawn(MATRAS, 20);
        let prince = t.spawn(DEMON_PRINCE, 40);
        let ranku = t.spawn(RANKU, 60);
        (t, matras, prince, ranku)
    }

    #[test]
    fn a_character_below_level_76_is_sent_away() {
        let (mut t, matras, _, _) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, matras).contains("32245-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_blueprints_then_the_reward() {
        let (mut t, matras, prince, ranku) = setup();
        t.enter(1, 76, vec![]);
        assert!(t.quest_page(1, matras).contains("32245-01.htm"));
        assert!(t.event(1, NAME, "32245-03.htm").contains("32245-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "rewarded_prince", "1"),
                rows::insert(NAME, "rewarded_ranku", "1"),
            ]
        );

        // The Demon Prince gives its blueprint; no cond yet (no Ranku blueprint).
        t.kill(1, prince);
        assert_eq!(t.count(1, BLUEPRINT_PRINCE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "rewarded_prince", "2")]);
        assert!(t.quest_page(1, matras).contains("32245-04.htm"));

        // Ranku's blueprint completes the pair: cond 2.
        t.kill(1, ranku);
        assert_eq!(t.count(1, BLUEPRINT_RANKU), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "rewarded_ranku", "2"),
                rows::update(NAME, "cond", "2"),
            ]
        );

        // Matras takes both blueprints: cond 3.
        assert!(t.quest_page(1, matras).contains("32245-05.htm"));
        assert_eq!(t.count(1, BLUEPRINT_RANKU), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, matras).contains("32245-06.htm"));

        // The reward page: adena, exp, six items, `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32245-07.htm").contains("32245-07.htm"));
        assert_eq!(t.count(1, ADENA), 65_884);
        for item in [FIRE, WATER, EARTH, WIND, DARKNESS, DIVINITY] {
            assert_eq!(t.count(1, item), 1);
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
