//! Quest 350: Enhance Your Weapon (Trader Kakai, Gatekeeper Tamil and Blacksmith Kiyou; the soul
//! crystals of the level-up table).
// Java: dist/game/data/scripts/quests/Q00350_EnhanceYourWeapon/Q00350_EnhanceYourWeapon.java
//
// Pattern: a repeatable quest that hands out the soul crystal stages (red, green, blue) and
// leaves at `exit.htm`. The part that levels the crystals on a kill (`levelSoulCrystals`,
// `onSkillSee` with the absorb list) is not ported yet: see the FOLLOW-UP below.

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const STARTING_NPCS: [i32; 3] = [30115, 30856, 30194];

// Items
const RED_SOUL_CRYSTAL0_ID: i32 = 4629;
const GREEN_SOUL_CRYSTAL0_ID: i32 = 4640;
const BLUE_SOUL_CRYSTAL0_ID: i32 = 4651;

// Misc
const MIN_LEVEL: i32 = 40;

/// Java `Q00350_EnhanceYourWeapon`.
pub struct Q00350EnhanceYourWeapon;

impl Script for Q00350EnhanceYourWeapon {
    fn name(&self) -> &'static str {
        "Q00350_EnhanceYourWeapon"
    }

    fn path(&self) -> &'static str {
        "quests/Q00350_EnhanceYourWeapon"
    }

    fn id(&self) -> i32 {
        350
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&STARTING_NPCS);
        s.add_talk_id(&STARTING_NPCS);
        // FOLLOW-UP(engine): Java adds `addSkillSeeId` and `addKillId` for every NPC of
        // `LevelUpCrystalData.getNpcsSoulInfo()`. The engine has no loader for
        // `data/LevelUpCrystalData.xml` yet, so no kill or skill-see listener is registered.
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if event.ends_with("-04.htm") {
            if let Some(qs) = qs {
                qs.start_quest(ctx);
            }
        } else if event.ends_with("-09.htm") {
            ctx.give_items(player, RED_SOUL_CRYSTAL0_ID, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        } else if event.ends_with("-10.htm") {
            ctx.give_items(player, GREEN_SOUL_CRYSTAL0_ID, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        } else if event.ends_with("-11.htm") {
            ctx.give_items(player, BLUE_SOUL_CRYSTAL0_ID, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        } else if event.eq_ignore_ascii_case("exit.htm") {
            if let Some(qs) = qs {
                qs.exit_quest(ctx, true, true);
            }

            // Java: `NpcHtmlMessage` (not the quest window) with the page's `%npcname%` replaced.
            if let Some(npc) = npc
                && let Some(html) = ctx.get_htm(player, "exit.htm")
            {
                let name = ctx.world().npc_template_name(npc.object_id);
                let html = html.replace("%npcname%", &name);
                if let Some(packet) = NpcHtmlMessage::new(npc.object_id.0, &html) {
                    ctx.world().send(player, &packet);
                }
            }
            return None;
        }
        // NOTE(java-quirk): every other event is echoed back as the page to show (`htmltext = event`).
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) < MIN_LEVEL {
                format!("{}-lvl.htm", npc.id)
            } else {
                format!("{}-01.htm", npc.id)
            };
        } else if has_any_soul_crystal_of_the_table(ctx, player) {
            htmltext = format!("{}-03.htm", npc.id);
        } else if !ctx.has_quest_items(player, RED_SOUL_CRYSTAL0_ID)
            && !ctx.has_quest_items(player, GREEN_SOUL_CRYSTAL0_ID)
            && !ctx.has_quest_items(player, BLUE_SOUL_CRYSTAL0_ID)
        {
            htmltext = format!("{}-21.htm", npc.id);
        }
        Some(htmltext)
    }
}

/// Java `check(player)`: any item of the soul crystal ids 4629..4664.
fn has_any_soul_crystal_of_the_table(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    (4629..4665).any(|item_id| ctx.has_quest_items(player, item_id))
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00350_EnhanceYourWeapon";

    const NPCS: &str = r#"
        <npc id="30115" type="Folk" level="40" name="Kakai"/>
        <npc id="30856" type="Folk" level="40" name="Tamil"/>
        <npc id="30194" type="Folk" level="40" name="Kiyou"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4629" type="EtcItem" name="Red Soul Crystal - Stage 0"><set name="is_questitem" val="true"/></item>
        <item id="4640" type="EtcItem" name="Green Soul Crystal - Stage 0"><set name="is_questitem" val="true"/></item>
        <item id="4651" type="EtcItem" name="Blue Soul Crystal - Stage 0"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00350EnhanceYourWeapon);
        let pages = Pages::of(&Q00350EnhanceYourWeapon)
            .page("30115-01.htm", &["30115-02.htm"])
            .page("30115-02.htm", &["30115-04.htm"])
            .page("30115-04.htm", &["30115-05.htm"])
            .page("30115-05.htm", &["30115-06.htm"])
            .page("30115-06.htm", &["30115-07.htm"])
            .page("30115-07.htm", &["30115-08.htm"])
            .page("30115-08.htm", &["30115-09.htm", "30115-10.htm", "30115-11.htm"])
            .plain(&["30115-lvl.htm", "30115-03.htm", "30115-09.htm", "30115-10.htm", "30115-11.htm"])
            .page("30115-21.htm", &["30115-08.htm", "exit.htm"])
            .plain(&["30115-12.htm"])
            .file(
                "data/scripts/quests/Q00350_EnhanceYourWeapon/exit.htm",
                "<html><body>%npcname%:<br>Yes, you do look rather exhausted! Take a rest, my friend.</body></html>",
            )
            .npcs(&STARTING_NPCS);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kakai = t.spawn(30115, 20);
        (t, kakai)
    }

    #[test]
    fn a_character_below_level_forty_is_sent_away() {
        let (mut t, kakai) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, kakai).contains("30115-lvl.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_page_chain_starts_the_quest() {
        let (mut t, kakai) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, kakai).contains("30115-01.htm"));
        assert!(t.event(1, NAME, "30115-02.htm").contains("30115-02.htm"));
        assert!(t.take_writes().is_empty());
        // `-04.htm` starts the quest (cond 1, state Started).
        assert!(t.event(1, NAME, "30115-04.htm").contains("30115-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
    }

    /// The start, then the pages of the trader up to the page with the crystal links (08).
    fn to_crystal_page(t: &mut ScriptTest, kakai: Oid) {
        t.enter(1, 40, vec![]);
        t.quest_page(1, kakai);
        for page in [
            "30115-02.htm",
            "30115-04.htm",
            "30115-05.htm",
            "30115-06.htm",
            "30115-07.htm",
            "30115-08.htm",
        ] {
            t.event(1, NAME, page);
        }
        t.take();
        t.take_writes();
    }

    #[test]
    fn the_red_crystal_is_given_with_the_middle_sound() {
        let (mut t, kakai) = setup();
        to_crystal_page(&mut t, kakai);
        t.bypass(1, &format!("Script {NAME} 30115-09.htm"));
        assert_eq!(t.count(1, 4629), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_green_crystal_is_given() {
        let (mut t, kakai) = setup();
        to_crystal_page(&mut t, kakai);
        t.event(1, NAME, "30115-10.htm");
        assert_eq!(t.count(1, 4640), 1);
        assert_eq!(t.count(1, 4629), 0);
    }

    #[test]
    fn the_blue_crystal_is_given() {
        let (mut t, kakai) = setup();
        to_crystal_page(&mut t, kakai);
        t.event(1, NAME, "30115-11.htm");
        assert_eq!(t.count(1, 4651), 1);
    }

    #[test]
    fn a_crystal_in_the_bag_sends_the_player_to_the_crystal_page() {
        let (mut t, kakai) = setup();
        t.enter(1, 40, vec![item(100, 4640, 1)]);
        assert!(t.quest_page(1, kakai).contains("30115-01.htm"));
        t.event(1, NAME, "30115-02.htm");
        t.event(1, NAME, "30115-04.htm");
        assert!(t.quest_page(1, kakai).contains("30115-03.htm"));
    }

    #[test]
    fn exit_takes_the_quest_and_shows_the_page_with_the_npc_name() {
        let (mut t, kakai) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, kakai);
        t.event(1, NAME, "30115-02.htm");
        t.event(1, NAME, "30115-04.htm");
        t.take_writes();
        t.take();

        // With no crystals the trader says 21; its link leaves the quest.
        assert!(t.quest_page(1, kakai).contains("30115-21.htm"));
        t.take();
        t.bypass(1, &format!("Script {NAME} exit.htm"));
        let out = t.take();
        let html = pages_to(&out, 1).pop().unwrap();
        assert!(html.contains("Kakai:<br>"), "{html}");
        assert!(html.contains("Yes, you do look rather exhausted!"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
    }
}
