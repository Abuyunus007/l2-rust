//! Quest 268: Traces of Evil (Kunai in the Dark Elven area sends the player after the kasha spiders).
// Java: dist/game/data/scripts/quests/Q00268_TracesOfEvil/Q00268_TracesOfEvil.java
//
// Pattern: a kill-count quest (`docs/design/scripts.md`, "Quest patterns"): every kasha spider
// gives one venom while cond 1 lasts; the 30th sets cond 2 and the NPC pays and ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KUNAI: i32 = 30559;

// Item
const CONTAMINATED_KASHA_SPIDER_VENOM: i32 = 10869;

// Monsters
const MONSTERS: [i32; 3] = [
    20474, // Kasha Spider
    20476, // Kasha Fang Spider
    20478, // Kasha Blade Spider
];

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00268_TracesOfEvil`.
pub struct Q00268TracesOfEvil;

impl Script for Q00268TracesOfEvil {
    fn name(&self) -> &'static str {
        "Q00268_TracesOfEvil"
    }

    fn path(&self) -> &'static str {
        "quests/Q00268_TracesOfEvil"
    }

    fn id(&self) -> i32 {
        268
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KUNAI]);
        s.add_talk_id(&[KUNAI]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[CONTAMINATED_KASHA_SPIDER_VENOM]);
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
        if event.eq_ignore_ascii_case("30559-03.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            ctx.give_items(killer, CONTAMINATED_KASHA_SPIDER_VENOM, 1);
            if ctx.get_quest_items_count(killer, CONTAMINATED_KASHA_SPIDER_VENOM) >= 30 {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30559-02.htm"
                } else {
                    "30559-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if !ctx.has_quest_items(player, CONTAMINATED_KASHA_SPIDER_VENOM) {
                        "30559-04.html"
                    } else {
                        "30559-05.html"
                    }
                    .to_owned();
                }
                2 => {
                    // NOTE(java-quirk): with fewer than 30 venoms the page stays the default
                    // "no quest" message (Java's `htmltext` is not changed in that branch).
                    if ctx.get_quest_items_count(player, CONTAMINATED_KASHA_SPIDER_VENOM) >= 30 {
                        ctx.give_adena(player, 2474, true);
                        ctx.add_exp_and_sp(player, 8738, 409);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30559-06.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00268_TracesOfEvil";

    const NPCS: &str = r#"
        <npc id="30559" type="Folk" level="70" name="Kunai"/>
        <npc id="20474" type="Monster" level="20" name="Kasha Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10869" type="EtcItem" name="Contaminated Kasha Spider Venom"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00268TracesOfEvil);
        let pages = Pages::of(&Q00268TracesOfEvil)
            .page("30559-02.htm", &["30559-03.htm"])
            .plain(&[
                "30559-01.htm",
                "30559-03.htm",
                "30559-04.html",
                "30559-05.html",
                "30559-06.html",
            ])
            .npcs(&[KUNAI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kunai = t.spawn(KUNAI, 20);
        (t, kunai)
    }

    #[test]
    fn a_character_below_level_fifteen_is_sent_away() {
        let (mut t, kunai) = setup();
        t.enter_as(1, 14, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, kunai).contains("30559-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn thirty_venoms_then_adena_and_exp() {
        let (mut t, kunai) = setup();
        t.enter_as(1, 15, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, kunai).contains("30559-02.htm"));
        assert!(t.event(1, NAME, "30559-03.htm").contains("30559-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kunai).contains("30559-04.html"));

        // The first venom changes the page; the 30th venom sets cond 2 (`setCond(2, true)`).
        let spider = t.spawn(20474, 60);
        t.kill(1, spider);
        assert!(t.quest_page(1, kunai).contains("30559-05.html"));
        for _ in 1..30 {
            let spider = t.spawn(20474, 60);
            t.kill(1, spider);
        }
        assert_eq!(t.count(1, CONTAMINATED_KASHA_SPIDER_VENOM), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Kunai pays 2474 adena and the exp, then the quest is repeatable again.
        assert!(t.quest_page(1, kunai).contains("30559-06.html"));
        assert_eq!(t.count(1, 57), 2474);
        assert_eq!(t.count(1, CONTAMINATED_KASHA_SPIDER_VENOM), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
