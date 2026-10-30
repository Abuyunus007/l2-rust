//! Quest 328: Sense for Business (Sarien in the Rune town, the eye and basilisk monsters).
// Java: dist/game/data/scripts/quests/Q00328_SenseForBusiness/Q00328_SenseForBusiness.java
//
// Pattern: a kill-count quest with drop chances per monster (`docs/design/scripts.md`, "Quest
// patterns"). One roll (`getRandom(100)`) per kill decides the item: below the first threshold
// the carcass, below the second the lens (eye monsters), or the gizzard (basilisks). The NPC pays
// for everything the player brings, with a bonus at ten items.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SARIEN: i32 = 30436;

// Monsters: the eyes (id, carcass below, lens below) and the basilisks (id, gizzard below)
const MONSTER_EYES: [(i32, i32, i32); 4] =
    [(20055, 61, 62), (20059, 61, 62), (20067, 72, 74), (20068, 78, 79)];
const MONSTER_BASILISKS: [(i32, i32); 2] = [(20070, 60), (20072, 63)];

// Items
const MONSTER_EYE_CARCASS: i32 = 1347;
const MONSTER_EYE_LENS: i32 = 1366;
const BASILISK_GIZZARD: i32 = 1348;

// Misc
const MONSTER_EYE_CARCASS_ADENA: i64 = 25;
const MONSTER_EYE_LENS_ADENA: i64 = 1000;
const BASILISK_GIZZARD_ADENA: i64 = 60;
const BONUS: i64 = 618;
const BONUS_COUNT: i64 = 10;
const MIN_LEVEL: i32 = 21;

/// Java `Q00328_SenseForBusiness`.
pub struct Q00328SenseForBusiness;

impl Script for Q00328SenseForBusiness {
    fn name(&self) -> &'static str {
        "Q00328_SenseForBusiness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00328_SenseForBusiness"
    }

    fn id(&self) -> i32 {
        328
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SARIEN]);
        s.add_talk_id(&[SARIEN]);
        let mut monsters: Vec<i32> = MONSTER_EYES.iter().map(|(id, _, _)| *id).collect();
        monsters.extend(MONSTER_BASILISKS.iter().map(|(id, _)| *id));
        s.add_kill_id(&monsters);
        s.register_quest_items(&[MONSTER_EYE_CARCASS, MONSTER_EYE_LENS, BASILISK_GIZZARD]);
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
            "30436-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30436-06.html" => {
                qs.exit_quest(ctx, true, true);
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
                "30436-01.htm"
            } else {
                "30436-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let carcass = ctx.get_quest_items_count(player, MONSTER_EYE_CARCASS);
            let lens = ctx.get_quest_items_count(player, MONSTER_EYE_LENS);
            let gizzards = ctx.get_quest_items_count(player, BASILISK_GIZZARD);
            let total = carcass + lens + gizzards;
            if total > 0 {
                let bonus = if total >= BONUS_COUNT { BONUS } else { 0 };
                ctx.give_adena(
                    player,
                    carcass * MONSTER_EYE_CARCASS_ADENA
                        + lens * MONSTER_EYE_LENS_ADENA
                        + gizzards * BASILISK_GIZZARD_ADENA
                        + bonus,
                    true,
                );
                ctx.take_items_many(
                    player,
                    -1,
                    &[MONSTER_EYE_CARCASS, MONSTER_EYE_LENS, BASILISK_GIZZARD],
                );
                htmltext = "30436-05.html".to_owned();
            } else {
                htmltext = "30436-04.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
        {
            // One roll for the kill, whatever the monster (Java rolls before the lookups).
            let chance = ctx.get_random(100);
            if let Some((_, carcass_below, lens_below)) =
                MONSTER_EYES.iter().find(|(id, _, _)| *id == npc.id).copied()
            {
                if chance < carcass_below {
                    ctx.give_items(player, MONSTER_EYE_CARCASS, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                } else if chance < lens_below {
                    ctx.give_items(player, MONSTER_EYE_LENS, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
            } else if let Some((_, gizzard_below)) =
                MONSTER_BASILISKS.iter().find(|(id, _)| *id == npc.id).copied()
                && chance < gizzard_below
            {
                ctx.give_items(player, BASILISK_GIZZARD, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
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
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00328_SenseForBusiness";

    const NPCS: &str = r#"
        <npc id="30436" type="Folk" level="1" name="Sarien"/>
        <npc id="20055" type="Monster" level="20" name="Ol Mahum Patrol"/>
        <npc id="20070" type="Monster" level="21" name="Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1347" type="EtcItem" name="Monster Eye Carcass"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1366" type="EtcItem" name="Monster Eye Lens"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1348" type="EtcItem" name="Basilisk Gizzard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00328SenseForBusiness);
        let pages = Pages::of(&Q00328SenseForBusiness)
            .page("30436-02.htm", &["30436-03.htm"])
            .plain(&["30436-01.htm", "30436-03.htm"])
            .page("30436-04.html", &["30436-06.html", "30436-07.html"])
            .page("30436-05.html", &["30436-06.html", "30436-07.html"])
            .plain(&["30436-06.html", "30436-07.html"])
            .npcs(&[SARIEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sarien = t.spawn(SARIEN, 20);
        (t, sarien)
    }

    #[test]
    fn a_level_twenty_character_is_sent_away() {
        let (mut t, sarien) = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, sarien).contains("30436-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_basilisk_gizzard_then_the_reward() {
        let (mut t, sarien) = setup();
        t.enter_as(1, 21, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, sarien).contains("30436-02.htm"));
        assert!(t.event(1, NAME, "30436-03.htm").contains("30436-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, sarien).contains("30436-04.html"));

        // Basilisks drop a gizzard below 60 of 100 rolls: the first one ends the hunt.
        for _ in 0..400 {
            let basilisk = t.spawn(20070, 60);
            t.kill(1, basilisk);
            if t.count(1, BASILISK_GIZZARD) >= 1 {
                break;
            }
        }
        assert_eq!(t.count(1, BASILISK_GIZZARD), 1);
        assert!(t.quest_page(1, sarien).contains("30436-05.html"));
        assert_eq!(t.count(1, 57), BASILISK_GIZZARD_ADENA);
        assert_eq!(t.count(1, BASILISK_GIZZARD), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_items_get_the_bonus_and_the_end_is_repeatable() {
        let (mut t, sarien) = setup();
        t.enter_as(
            1,
            21,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1347, 9), item(901, 1366, 1)],
        );
        t.quest_page(1, sarien);
        assert!(t.event(1, NAME, "30436-03.htm").contains("30436-03.htm"));
        t.take_writes();

        // 9 carcasses (25 each), 1 lens (1000), the bonus of ten items (618).
        assert!(t.quest_page(1, sarien).contains("30436-05.html"));
        assert_eq!(t.count(1, 57), 9 * 25 + 1000 + 618);
        assert_eq!(
            (t.count(1, MONSTER_EYE_CARCASS), t.count(1, MONSTER_EYE_LENS)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());

        // The end: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 30436-06.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30436-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
