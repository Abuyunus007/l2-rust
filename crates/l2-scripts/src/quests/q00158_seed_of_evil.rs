//! Quest 158: Seed of Evil (Biotin in Gludio, the monster Nerkas).
// Java: dist/game/data/scripts/quests/Q00158_SeedOfEvil/Q00158_SeedOfEvil.java
//
// Pattern: a one-time quest with an item, exp and adena reward (`docs/design/scripts.md`,
// "Quest patterns"): `exit_quest(ctx, false, true)` deletes the variables and keeps the state
// row as `Completed`, so the start NPC shows the "already completed" page afterwards. Also an
// `on_attack` that talks once per life of the NPC (`npc.isScriptValue(0)` / `setScriptValue(1)`).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const BIOTIN: i32 = 30031;

// Monster
const NERKAS: i32 = 27016;

// Items
const ENCHANT_ARMOR_D: i32 = 956;
const CLAY_TABLET: i32 = 1025;

// Misc
const MIN_LEVEL: i32 = 21;

/// Java `Q00158_SeedOfEvil`.
pub struct Q00158SeedOfEvil;

impl Script for Q00158SeedOfEvil {
    fn name(&self) -> &'static str {
        "Q00158_SeedOfEvil"
    }

    fn path(&self) -> &'static str {
        "quests/Q00158_SeedOfEvil"
    }

    fn id(&self) -> i32 {
        158
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BIOTIN]);
        s.add_talk_id(&[BIOTIN]);
        s.add_attack_id(&[NERKAS]);
        s.add_kill_id(&[NERKAS]);
        s.register_quest_items(&[CLAY_TABLET]);
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
        if let Some(qs) = qs
            && event.eq_ignore_ascii_case("30031-03.htm")
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.is_script_value(npc, 0) {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HowDareYouChallengeMe, &[]);
            ctx.set_script_value(npc, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && !ctx.has_quest_items(killer, CLAY_TABLET)
        {
            ctx.give_items(killer, CLAY_TABLET, 1);
            qs.set_cond(ctx, 2, true);
        }
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::ThePowerOfLordBelethRulesTheWholeWorld,
            &[],
        );
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30031-02.htm"
                } else {
                    "30031-01.html"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30031-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, CLAY_TABLET) {
                    ctx.give_items(player, ENCHANT_ARMOR_D, 1);
                    ctx.add_exp_and_sp(player, 17818, 927);
                    ctx.give_adena(player, 1495, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30031-05.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00158_SeedOfEvil";

    const NPCS: &str = r#"
        <npc id="30031" type="Folk" level="70" name="Biotin"/>
        <npc id="27016" type="Monster" level="22" name="Nerkas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"><set name="is_stackable" val="true"/></item>
        <item id="1025" type="EtcItem" name="Clay Tablet"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00158SeedOfEvil);
        let pages = Pages::of(&Q00158SeedOfEvil)
            .page("30031-02.htm", &["30031-03.htm"])
            .plain(&["30031-01.html", "30031-03.htm", "30031-04.html", "30031-05.html"])
            .npcs(&[BIOTIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let biotin = t.spawn(BIOTIN, 20);
        (t, biotin)
    }

    #[test]
    fn the_tablet_then_scroll_exp_and_adena_once() {
        let (mut t, biotin) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, biotin).contains("30031-01.html"));
        t.world.player_mut(Oid(1)).unwrap().core.level = 21;
        assert!(t.quest_page(1, biotin).contains("30031-02.htm"));
        t.event(1, NAME, "30031-03.htm");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, biotin).contains("30031-04.html"));

        // Nerkas dies with its last words; the tablet and cond 2.
        let nerkas = t.spawn(NERKAS, 60);
        t.take();
        t.kill(1, nerkas);
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
        assert_eq!(t.count(1, CLAY_TABLET), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: one scroll, the adena, the end of the quest.
        assert!(t.quest_page(1, biotin).contains("30031-05.html"));
        assert_eq!(
            (
                t.count(1, ENCHANT_ARMOR_D),
                t.count(1, 57),
                t.count(1, CLAY_TABLET)
            ),
            (1, 1495, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, biotin)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn nerkas_talks_on_the_first_hit_only() {
        let (mut t, _biotin) = setup();
        t.enter(1, 21, vec![]);
        let nerkas = t.spawn(NERKAS, 60);
        let npc = ScriptNpc {
            object_id: nerkas,
            id: NERKAS,
        };
        t.take();
        for _ in 0..2 {
            t.world.run_as_script(NAME, |ctx| {
                Q00158SeedOfEvil.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
        }
        let says = names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count();
        assert_eq!(says, 1);
        t.world
            .run_as_script(NAME, |ctx| assert!(ctx.is_script_value(npc, 1)));
    }
}
