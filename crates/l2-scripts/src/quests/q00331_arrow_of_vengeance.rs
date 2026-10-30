//! Quest 331: Arrow of Vengeance (Belton in the Rune town, Harpies, Medusae and Wyrms).
// Java: dist/game/data/scripts/quests/Q00331_ArrowOfVengeance/Q00331_ArrowOfVengeance.java
//
// Pattern: a kill-count quest with a drop chance per monster (`docs/design/scripts.md`, "Quest
// patterns"): one roll per kill decides the item of the monster; the NPC pays for everything the
// player brings, with a bonus at ten items.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BELTON: i32 = 30125;

// Items
const HARPY_FEATHER: i32 = 1452;
const MEDUSA_VENOM: i32 = 1453;
const WYRMS_TOOTH: i32 = 1454;

// Monsters (id, drop chance in percent)
const HARPY: i32 = 20145;
const MEDUSA: i32 = 20158;
const WYRM: i32 = 20176;
const MONSTERS: [(i32, i32); 3] = [(HARPY, 59), (MEDUSA, 61), (WYRM, 60)];

// Misc
const MIN_LEVEL: i32 = 32;
const HARPY_FEATHER_ADENA: i64 = 78;
const MEDUSA_VENOM_ADENA: i64 = 88;
const WYRMS_TOOTH_ADENA: i64 = 92;
const BONUS: i64 = 3100;
const BONUS_COUNT: i64 = 10;

/// Java `Q00331_ArrowOfVengeance`.
pub struct Q00331ArrowOfVengeance;

impl Script for Q00331ArrowOfVengeance {
    fn name(&self) -> &'static str {
        "Q00331_ArrowOfVengeance"
    }

    fn path(&self) -> &'static str {
        "quests/Q00331_ArrowOfVengeance"
    }

    fn id(&self) -> i32 {
        331
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BELTON]);
        s.add_talk_id(&[BELTON]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[HARPY_FEATHER, MEDUSA_VENOM, WYRMS_TOOTH]);
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
            "30125-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            // NOTE(java-quirk): the pages of this quest link to `30125-6.html` and `30125-7.html`
            // (no leading zero), which are not these events and not files: those links do
            // nothing, so the end (`30125-06.html`) is out of reach from the page.
            "30125-06.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30125-07.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) < MIN_LEVEL {
                "30125-01.htm"
            } else {
                "30125-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let harpy_feathers = ctx.get_quest_items_count(player, HARPY_FEATHER);
            let medusa_venoms = ctx.get_quest_items_count(player, MEDUSA_VENOM);
            let wyrms_teeth = ctx.get_quest_items_count(player, WYRMS_TOOTH);
            let total = harpy_feathers + medusa_venoms + wyrms_teeth;
            if total > 0 {
                let bonus = if total >= BONUS_COUNT { BONUS } else { 0 };
                ctx.give_adena(
                    player,
                    harpy_feathers * HARPY_FEATHER_ADENA
                        + medusa_venoms * MEDUSA_VENOM_ADENA
                        + wyrms_teeth * WYRMS_TOOTH_ADENA
                        + bonus,
                    true,
                );
                ctx.take_items_many(player, -1, &[HARPY_FEATHER, MEDUSA_VENOM, WYRMS_TOOTH]);
                htmltext = "30125-05.html".to_owned();
            } else {
                htmltext = "30125-04.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java checks only that the quest state exists (not that it is started).
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() {
            // Only the registered monsters reach here.
            let chance = MONSTERS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0, |(_, chance)| *chance);
            if ctx.get_random(100) < chance {
                let item = match npc.id {
                    HARPY => HARPY_FEATHER,
                    MEDUSA => MEDUSA_VENOM,
                    _ => WYRMS_TOOTH,
                };
                ctx.give_items(killer, item, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
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
    use crate::testing::{Pages, ScriptTest, item, names_to, rows};

    const NAME: &str = "Q00331_ArrowOfVengeance";

    const NPCS: &str = r#"
        <npc id="30125" type="Folk" level="1" name="Belton"/>
        <npc id="20145" type="Monster" level="30" name="Harpy"/>
        <npc id="20158" type="Monster" level="31" name="Medusa"/>
        <npc id="20176" type="Monster" level="31" name="Wyrm"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1452" type="EtcItem" name="Harpy's Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1453" type="EtcItem" name="Medusa's Venom"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1454" type="EtcItem" name="Wyrm's Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00331ArrowOfVengeance);
        // The real page links `30125-6.html` and `30125-7.html` (see the NOTE above).
        let pages = Pages::of(&Q00331ArrowOfVengeance)
            .page("30125-02.htm", &["30125-03.htm"])
            .plain(&["30125-01.htm", "30125-03.htm", "30125-06.html", "30125-07.html"])
            .page("30125-04.html", &["30125-6.html", "30125-7.html"])
            .page("30125-05.html", &["30125-6.html", "30125-7.html"])
            .npcs(&[BELTON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let belton = t.spawn(BELTON, 20);
        (t, belton)
    }

    #[test]
    fn a_level_thirty_one_character_is_sent_away() {
        let (mut t, belton) = setup();
        t.enter_as(1, 31, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, belton).contains("30125-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_feather_then_the_reward_and_the_broken_links() {
        let (mut t, belton) = setup();
        t.enter_as(1, 32, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, belton).contains("30125-02.htm"));
        assert!(t.event(1, NAME, "30125-03.htm").contains("30125-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, belton).contains("30125-04.html"));

        // Harpies drop a feather at 59%: the first one ends the hunt.
        for _ in 0..400 {
            let harpy = t.spawn(HARPY, 60);
            t.kill(1, harpy);
            if t.count(1, HARPY_FEATHER) >= 1 {
                break;
            }
        }
        assert_eq!(t.count(1, HARPY_FEATHER), 1);
        assert!(t.quest_page(1, belton).contains("30125-05.html"));
        assert_eq!(t.count(1, 57), HARPY_FEATHER_ADENA);
        assert_eq!(t.count(1, HARPY_FEATHER), 0);
        assert!(t.take_writes().is_empty());

        // The pages' links `30125-6.html` / `30125-7.html` show nothing.
        t.quest_page(1, belton);
        assert!(t.event(1, NAME, "30125-6.html").is_empty());
    }

    #[test]
    fn ten_items_get_the_bonus_and_the_end_is_repeatable() {
        let (mut t, belton) = setup();
        t.enter_as(
            1,
            32,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1452, 4), item(901, 1453, 3), item(902, 1454, 3)],
        );
        t.quest_page(1, belton);
        assert!(t.event(1, NAME, "30125-03.htm").contains("30125-03.htm"));
        t.take_writes();

        // 4 * 78 + 3 * 88 + 3 * 92 + the bonus of ten (3100).
        assert!(t.quest_page(1, belton).contains("30125-05.html"));
        assert_eq!(t.count(1, 57), 4 * 78 + 3 * 88 + 3 * 92 + 3100);
        assert_eq!(
            t.count(1, HARPY_FEATHER) + t.count(1, MEDUSA_VENOM) + t.count(1, WYRMS_TOOTH),
            0
        );
        assert!(t.take_writes().is_empty());

        // The end, `30125-06.html`: no page of the quest links it (see the NOTE), so the hook is
        // called directly, the way a correct link would.
        t.take();
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00331ArrowOfVengeance.on_event(ctx, "30125-06.html", None, Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("30125-06.html"));
        });
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
