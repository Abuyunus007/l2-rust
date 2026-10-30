//! Quest 412: Path of the Dark Wizard (Varika, Charkeren, Annika and Arkenia).
// Java: dist/game/data/scripts/quests/Q00412_PathOfTheDarkWizard/Q00412_PathOfTheDarkWizard.java
//
// Pattern: four seeds, each made from a few monster drops and a key or item from a guild NPC
// (`docs/design/scripts.md`, "Quest patterns"). The quest never calls `setCond`: the cond stays
// 1 until the reward. Drops roll `getRandom(2) == 0` after the item checks.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CHARKEREN: i32 = 30415;
const ANNIKA: i32 = 30418;
const ARKENIA: i32 = 30419;
const VARIKA: i32 = 30421;

// Items
const SEEDS_OF_ANGER: i32 = 1253;
const SEEDS_OF_DESPAIR: i32 = 1254;
const SEEDS_OF_HORROR: i32 = 1255;
const SEEDS_OF_LUNACY: i32 = 1256;
const FAMILYS_REMAINS: i32 = 1257;
const KNEE_BONE: i32 = 1259;
const HEART_OF_LUNACY: i32 = 1260;
const LUCKY_KEY: i32 = 1277;
const CANDLE: i32 = 1278;
const HUB_SCENT: i32 = 1279;

// Reward
const JEWEL_OF_DARKNESS: i32 = 1261;

// Monsters
const MARSH_ZOMBIE: i32 = 20015;
const MISERY_SKELETON: i32 = 20022;
const SKELETON_SCOUT: i32 = 20045;
const SKELETON_HUNTER: i32 = 20517;
const SKELETON_HUNTER_ARCHER: i32 = 20518;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00412_PathOfTheDarkWizard`.
pub struct Q00412PathOfTheDarkWizard;

impl Script for Q00412PathOfTheDarkWizard {
    fn name(&self) -> &'static str {
        "Q00412_PathOfTheDarkWizard"
    }

    fn path(&self) -> &'static str {
        "quests/Q00412_PathOfTheDarkWizard"
    }

    fn id(&self) -> i32 {
        412
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VARIKA]);
        s.add_talk_id(&[VARIKA, CHARKEREN, ANNIKA, ARKENIA]);
        s.add_kill_id(&[
            MARSH_ZOMBIE,
            MISERY_SKELETON,
            SKELETON_SCOUT,
            SKELETON_HUNTER,
            SKELETON_HUNTER_ARCHER,
        ]);
        s.register_quest_items(&[
            SEEDS_OF_ANGER,
            SEEDS_OF_DESPAIR,
            SEEDS_OF_HORROR,
            SEEDS_OF_LUNACY,
            FAMILYS_REMAINS,
            KNEE_BONE,
            HEART_OF_LUNACY,
            LUCKY_KEY,
            CANDLE,
            HUB_SCENT,
        ]);
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
            "ACCEPT" => {
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::DARK_MAGE {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, JEWEL_OF_DARKNESS) {
                                "30421-04.htm"
                            } else {
                                qs.start_quest(ctx);
                                ctx.give_items(player, SEEDS_OF_DESPAIR, 1);
                                "30421-05.htm"
                            }
                        } else {
                            "30421-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::DARK_WIZARD {
                        "30421-02a.htm"
                    } else {
                        "30421-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30421-06.html" => {
                htmltext = Some(
                    if ctx.has_quest_items(player, SEEDS_OF_ANGER) {
                        event
                    } else {
                        "30421-07.html"
                    }
                    .to_owned(),
                );
            }
            "30421-09.html" => {
                htmltext = Some(
                    if ctx.has_quest_items(player, SEEDS_OF_HORROR) {
                        event
                    } else {
                        "30421-10.html"
                    }
                    .to_owned(),
                );
            }
            "30421-11.html" => {
                if ctx.has_quest_items(player, SEEDS_OF_LUNACY) {
                    htmltext = Some(event.to_owned());
                } else if !ctx.has_quest_items(player, SEEDS_OF_LUNACY)
                    && ctx.has_quest_items(player, SEEDS_OF_DESPAIR)
                {
                    htmltext = Some("30421-12.html".to_owned());
                }
            }
            "30421-08.html" | "30415-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30415-03.html" => {
                ctx.give_items(player, LUCKY_KEY, 1);
                htmltext = Some(event.to_owned());
            }
            "30418-02.html" => {
                ctx.give_items(player, CANDLE, 1);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            MARSH_ZOMBIE => {
                if ctx.has_quest_items(killer, LUCKY_KEY)
                    && ctx.get_quest_items_count(killer, FAMILYS_REMAINS) < 3
                    && ctx.get_random(2) == 0
                {
                    ctx.give_items(killer, FAMILYS_REMAINS, 1);
                    if ctx.get_quest_items_count(killer, FAMILYS_REMAINS) == 3 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MISERY_SKELETON | SKELETON_HUNTER | SKELETON_HUNTER_ARCHER => {
                if ctx.has_quest_items(killer, CANDLE)
                    && ctx.get_quest_items_count(killer, KNEE_BONE) < 2
                    && ctx.get_random(2) == 0
                {
                    ctx.give_items(killer, KNEE_BONE, 1);
                    if ctx.get_quest_items_count(killer, KNEE_BONE) == 2 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            SKELETON_SCOUT => {
                if ctx.has_quest_items(killer, HUB_SCENT)
                    && ctx.get_quest_items_count(killer, HEART_OF_LUNACY) < 3
                    && ctx.get_random(2) == 0
                {
                    ctx.give_items(killer, HEART_OF_LUNACY, 1);
                    if ctx.get_quest_items_count(killer, HEART_OF_LUNACY) == 3 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == VARIKA {
                htmltext = if !ctx.has_quest_items(player, JEWEL_OF_DARKNESS) {
                    "30421-01.htm"
                } else {
                    "30421-04.htm"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                VARIKA => {
                    if ctx.has_all_quest_items(
                        player,
                        &[SEEDS_OF_DESPAIR, SEEDS_OF_HORROR, SEEDS_OF_LUNACY, SEEDS_OF_ANGER],
                    ) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, JEWEL_OF_DARKNESS, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 28_630);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 28_630);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 35_328);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30421-13.html".to_owned();
                    } else if ctx.has_quest_items(player, SEEDS_OF_DESPAIR) {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                FAMILYS_REMAINS,
                                LUCKY_KEY,
                                CANDLE,
                                HUB_SCENT,
                                KNEE_BONE,
                                HEART_OF_LUNACY,
                            ],
                        ) {
                            htmltext = "30421-14.html".to_owned();
                        } else if !ctx.has_quest_items(player, SEEDS_OF_ANGER) {
                            htmltext = "30421-08.html".to_owned();
                        } else if !ctx.has_quest_items(player, SEEDS_OF_HORROR) {
                            htmltext = "30421-15.html".to_owned();
                        } else if !ctx.has_quest_items(player, SEEDS_OF_LUNACY) {
                            htmltext = "30421-12.html".to_owned();
                        }
                    }
                }
                CHARKEREN => {
                    if !ctx.has_quest_items(player, SEEDS_OF_ANGER)
                        && ctx.has_quest_items(player, SEEDS_OF_DESPAIR)
                    {
                        if !ctx.has_at_least_one_quest_item(player, &[FAMILYS_REMAINS, LUCKY_KEY]) {
                            htmltext = "30415-01.html".to_owned();
                        } else if ctx.has_quest_items(player, LUCKY_KEY)
                            && ctx.get_quest_items_count(player, FAMILYS_REMAINS) < 3
                        {
                            htmltext = "30415-04.html".to_owned();
                        } else {
                            ctx.give_items(player, SEEDS_OF_ANGER, 1);
                            ctx.take_items(player, FAMILYS_REMAINS, -1);
                            ctx.take_items(player, LUCKY_KEY, 1);
                            htmltext = "30415-05.html".to_owned();
                        }
                    } else {
                        htmltext = "30415-06.html".to_owned();
                    }
                }
                ANNIKA => {
                    if !ctx.has_quest_items(player, SEEDS_OF_HORROR)
                        && ctx.has_quest_items(player, SEEDS_OF_DESPAIR)
                    {
                        if !ctx.has_at_least_one_quest_item(player, &[CANDLE, KNEE_BONE]) {
                            htmltext = "30418-01.html".to_owned();
                        } else if ctx.has_quest_items(player, CANDLE)
                            && ctx.get_quest_items_count(player, KNEE_BONE) < 2
                        {
                            htmltext = "30418-03.html".to_owned();
                        } else {
                            ctx.give_items(player, SEEDS_OF_HORROR, 1);
                            ctx.take_items(player, KNEE_BONE, -1);
                            ctx.take_items(player, CANDLE, 1);
                            htmltext = "30418-04.html".to_owned();
                        }
                    }
                }
                ARKENIA => {
                    if !ctx.has_quest_items(player, SEEDS_OF_LUNACY) {
                        if !ctx.has_at_least_one_quest_item(player, &[HUB_SCENT, HEART_OF_LUNACY]) {
                            ctx.give_items(player, HUB_SCENT, 1);
                            htmltext = "30419-01.html".to_owned();
                        } else if ctx.has_quest_items(player, HUB_SCENT)
                            && ctx.get_quest_items_count(player, HEART_OF_LUNACY) < 3
                        {
                            htmltext = "30419-02.html".to_owned();
                        } else {
                            ctx.give_items(player, SEEDS_OF_LUNACY, 1);
                            ctx.take_items(player, HEART_OF_LUNACY, -1);
                            ctx.take_items(player, HUB_SCENT, 1);
                            htmltext = "30419-03.html".to_owned();
                        }
                    }
                }
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00412_PathOfTheDarkWizard";

    const NPCS: &str = r#"
        <npc id="30421" type="Folk" level="70" name="Varika"/>
        <npc id="30415" type="Folk" level="70" name="Charkeren"/>
        <npc id="30418" type="Folk" level="70" name="Annika"/>
        <npc id="30419" type="Folk" level="70" name="Arkenia"/>
        <npc id="20015" type="Monster" level="20" name="Marsh Zombie"/>
        <npc id="20022" type="Monster" level="20" name="Misery Skeleton"/>
        <npc id="20045" type="Monster" level="20" name="Skeleton Scout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1253" type="EtcItem" name="Seeds of Anger"><set name="is_questitem" val="true"/></item>
        <item id="1254" type="EtcItem" name="Seeds of Despair"><set name="is_questitem" val="true"/></item>
        <item id="1255" type="EtcItem" name="Seeds of Horror"><set name="is_questitem" val="true"/></item>
        <item id="1256" type="EtcItem" name="Seeds of Lunacy"><set name="is_questitem" val="true"/></item>
        <item id="1257" type="EtcItem" name="Family's Remains"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1259" type="EtcItem" name="Knee Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1260" type="EtcItem" name="Heart of Lunacy"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1277" type="EtcItem" name="Lucky Key"><set name="is_questitem" val="true"/></item>
        <item id="1278" type="EtcItem" name="Candle"><set name="is_questitem" val="true"/></item>
        <item id="1279" type="EtcItem" name="Hub Scent"><set name="is_questitem" val="true"/></item>
        <item id="1261" type="EtcItem" name="Jewel of Darkness"/>"#;

    struct Npcs {
        varika: Oid,
        charkeren: Oid,
        annika: Oid,
        arkenia: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00412PathOfTheDarkWizard)
            .page("30421-01.htm", &["ACCEPT"])
            .page(
                "30421-05.htm",
                &["30421-06.html", "30421-09.html", "30421-11.html"],
            )
            .page(
                "30421-14.html",
                &["30421-06.html", "30421-09.html", "30421-11.html"],
            )
            .page("30415-01.html", &["30415-02.html"])
            .page("30415-02.html", &["30415-03.html"])
            .page("30418-01.html", &["30418-02.html"])
            .plain(&[
                "30421-02.htm",
                "30421-02a.htm",
                "30421-03.htm",
                "30421-04.htm",
                "30421-06.html",
                "30421-07.html",
                "30421-08.html",
                "30421-09.html",
                "30421-10.html",
                "30421-11.html",
                "30421-12.html",
                "30421-13.html",
                "30421-15.html",
                "30415-03.html",
                "30415-04.html",
                "30415-05.html",
                "30415-06.html",
                "30418-02.html",
                "30418-03.html",
                "30418-04.html",
                "30419-01.html",
                "30419-02.html",
                "30419-03.html",
            ])
            .npcs(&[VARIKA, CHARKEREN, ANNIKA, ARKENIA])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00412PathOfTheDarkWizard);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            varika: t.spawn(VARIKA, 20),
            charkeren: t.spawn(CHARKEREN, 40),
            annika: t.spawn(ANNIKA, 60),
            arkenia: t.spawn(ARKENIA, 80),
        };
        (t, npcs)
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_jewel() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.varika).contains("30421-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30421-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::DARK_WIZARD, vec![]);
        t.quest_page(1, npcs.varika);
        assert!(t.event(1, NAME, "ACCEPT").contains("30421-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        t.quest_page(1, npcs.varika);
        assert!(t.event(1, NAME, "ACCEPT").contains("30421-03.htm"));

        // With the jewel the talk already shows the page (no link to ACCEPT on it).
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::DARK_ELF,
            player_class::DARK_MAGE,
            vec![item(5000, 1261, 1)],
        );
        assert!(t.quest_page(1, npcs.varika).contains("30421-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_despair_to_the_jewel() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.varika).contains("30421-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30421-05.htm"));
        assert_eq!(t.count(1, SEEDS_OF_DESPAIR), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Varika's page without any seed in the bag; the anger is still missing, so the event
        // shows its page.
        assert!(t.quest_page(1, npcs.varika).contains("30421-14.html"));
        assert!(t.event(1, NAME, "30421-06.html").contains("30421-07.html"));
        assert!(t.quest_page(1, npcs.varika).contains("30421-14.html"));

        // Charkeren: the lucky key from the first page, then the remains from the zombies.
        assert!(t.quest_page(1, npcs.charkeren).contains("30415-01.html"));
        assert!(t.event(1, NAME, "30415-02.html").contains("30415-02.html"));
        assert!(t.event(1, NAME, "30415-03.html").contains("30415-03.html"));
        assert_eq!(t.count(1, LUCKY_KEY), 1);
        assert!(t.quest_page(1, npcs.charkeren).contains("30415-04.html"));
        hunt(&mut t, MARSH_ZOMBIE, FAMILYS_REMAINS, 3);
        assert!(t.quest_page(1, npcs.charkeren).contains("30415-05.html"));
        assert_eq!(
            (
                t.count(1, FAMILYS_REMAINS),
                t.count(1, LUCKY_KEY),
                t.count(1, SEEDS_OF_ANGER)
            ),
            (0, 0, 1)
        );
        assert!(t.take_writes().is_empty());

        // Annika: the candle from her first page, then two knee bones from the skeletons.
        assert!(t.quest_page(1, npcs.annika).contains("30418-01.html"));
        assert!(t.event(1, NAME, "30418-02.html").contains("30418-02.html"));
        assert_eq!(t.count(1, CANDLE), 1);
        assert!(t.quest_page(1, npcs.annika).contains("30418-03.html"));
        hunt(&mut t, MISERY_SKELETON, KNEE_BONE, 2);
        assert!(t.quest_page(1, npcs.annika).contains("30418-04.html"));
        assert_eq!(
            (
                t.count(1, KNEE_BONE),
                t.count(1, CANDLE),
                t.count(1, SEEDS_OF_HORROR)
            ),
            (0, 0, 1)
        );

        // Arkenia: the hub scent first, then three hearts of lunacy from the scouts.
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-01.html"));
        assert_eq!(t.count(1, HUB_SCENT), 1);
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-02.html"));
        hunt(&mut t, SKELETON_SCOUT, HEART_OF_LUNACY, 3);
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-03.html"));
        assert_eq!(
            (
                t.count(1, HEART_OF_LUNACY),
                t.count(1, HUB_SCENT),
                t.count(1, SEEDS_OF_LUNACY)
            ),
            (0, 0, 1)
        );
        assert!(t.take_writes().is_empty());

        // Varika's talk with the four seeds is the reward: adena, the jewel, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.varika).contains("30421-13.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, JEWEL_OF_DARKNESS), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the jewel is already here.
        assert!(t.quest_page(1, npcs.varika).contains("30421-04.htm"));
    }
}
