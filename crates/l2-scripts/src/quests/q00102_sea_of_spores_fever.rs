//! Quest 102: Sea of Spores Fever (Alberius sends an elf to Cobendell, the dryads give ten tears,
//! and the medicine goes to the five sentinels; the last one puts the reward on Alberius).
// Java: dist/game/data/scripts/quests/Q00102_SeaOfSporesFever/Q00102_SeaOfSporesFever.java
//
// Pattern: a kill chance (`getRandom(10) < 3`) that counts up to ten items, then a list of NPCs
// that each take one item for the next step, with the NewbieGuide-free reward of Java.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const COBENDELL: i32 = 30156;
const BERROS: i32 = 30217;
const VELTRESS: i32 = 30219;
const RAYEN: i32 = 30221;
const ALBERIUS: i32 = 30284;
const GARTRANDELL: i32 = 30285;

// Monsters
const DRYAD: i32 = 20013;
const DRYAD_ELDER: i32 = 20019;

// Items
const SWORD_OF_SENTINEL: i32 = 743;
const STAFF_OF_SENTINEL: i32 = 744;
const ALBERIUS_LIST: i32 = 746;
const ALBERIUS_LETTER: i32 = 964;
const EVERGREEN_AMULET: i32 = 965;
const DRYADS_TEAR: i32 = 966;
const LESSER_HEALING_POTION: i32 = 1060;
const COBENDELLS_MEDICINE1: i32 = 1130;
const COBENDELLS_MEDICINE2: i32 = 1131;
const COBENDELLS_MEDICINE3: i32 = 1132;
const COBENDELLS_MEDICINE4: i32 = 1133;
const COBENDELLS_MEDICINE5: i32 = 1134;
const SOULSHOT_NO_GRADE: i32 = 1835;
const SPIRITSHOT_NO_GRADE: i32 = 2509;
const ECHO_CRYSTAL_THEME_OF_BATTLE: i32 = 4412;
const ECHO_CRYSTAL_THEME_OF_LOVE: i32 = 4413;
const ECHO_CRYSTAL_THEME_OF_SOLITUDE: i32 = 4414;
const ECHO_CRYSTAL_THEME_OF_FEAST: i32 = 4415;
const ECHO_CRYSTAL_THEME_OF_CELEBRATION: i32 = 4416;

// Misc
const MIN_LEVEL: i32 = 12;

const MEDICINES: [i32; 5] = [
    COBENDELLS_MEDICINE1,
    COBENDELLS_MEDICINE2,
    COBENDELLS_MEDICINE3,
    COBENDELLS_MEDICINE4,
    COBENDELLS_MEDICINE5,
];

/// Java `SENTINELS`: the sentinel NPC -> the medicine it takes.
fn sentinel_medicine(npc_id: i32) -> Option<i32> {
    match npc_id {
        GARTRANDELL => Some(COBENDELLS_MEDICINE5),
        RAYEN => Some(COBENDELLS_MEDICINE4),
        VELTRESS => Some(COBENDELLS_MEDICINE3),
        BERROS => Some(COBENDELLS_MEDICINE2),
        ALBERIUS => Some(COBENDELLS_MEDICINE1),
        _ => None,
    }
}

/// Java `Q00102_SeaOfSporesFever`.
pub struct Q00102SeaOfSporesFever;

impl Script for Q00102SeaOfSporesFever {
    fn name(&self) -> &'static str {
        "Q00102_SeaOfSporesFever"
    }

    fn path(&self) -> &'static str {
        "quests/Q00102_SeaOfSporesFever"
    }

    fn id(&self) -> i32 {
        102
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALBERIUS]);
        s.add_talk_id(&[ALBERIUS, COBENDELL, GARTRANDELL, BERROS, VELTRESS, RAYEN]);
        s.add_kill_id(&[DRYAD, DRYAD_ELDER]);
        s.register_quest_items(&[
            ALBERIUS_LIST,
            ALBERIUS_LETTER,
            EVERGREEN_AMULET,
            DRYADS_TEAR,
            COBENDELLS_MEDICINE1,
            COBENDELLS_MEDICINE2,
            COBENDELLS_MEDICINE3,
            COBENDELLS_MEDICINE4,
            COBENDELLS_MEDICINE5,
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
        if event == "30284-02.htm" {
            qs.start_quest(ctx);
            ctx.give_items(player, ALBERIUS_LETTER, 1);
            return Some(event.to_owned());
        }
        // Java `super.onEvent`: a plain `Quest` returns null.
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 2) && ctx.get_random(10) < 3 {
            ctx.give_items(killer, DRYADS_TEAR, 1);
            if ctx.get_quest_items_count(killer, DRYADS_TEAR) < 10 {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            } else {
                qs.set_cond(ctx, 3, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ALBERIUS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::ELF {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30284-07.htm"
                        } else {
                            "30284-08.htm"
                        }
                    } else {
                        "30284-00.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(player, ALBERIUS_LETTER) {
                            htmltext = "30284-03.html".to_owned();
                        }
                    }
                    2 => {
                        if ctx.has_quest_items(player, EVERGREEN_AMULET) {
                            htmltext = "30284-09.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.has_quest_items(player, COBENDELLS_MEDICINE1) {
                            ctx.take_items(player, COBENDELLS_MEDICINE1, 1);
                            ctx.give_items(player, ALBERIUS_LIST, 1);
                            qs.set_cond(ctx, 5, false);
                            htmltext = "30284-04.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_at_least_one_quest_item(player, &MEDICINES) {
                            htmltext = "30284-05.html".to_owned();
                        }
                    }
                    6 => {
                        if !ctx.has_at_least_one_quest_item(player, &MEDICINES) {
                            ctx.give_items(player, LESSER_HEALING_POTION, 100);
                            ctx.give_items(player, ECHO_CRYSTAL_THEME_OF_BATTLE, 10);
                            ctx.give_items(player, ECHO_CRYSTAL_THEME_OF_LOVE, 10);
                            ctx.give_items(player, ECHO_CRYSTAL_THEME_OF_SOLITUDE, 10);
                            ctx.give_items(player, ECHO_CRYSTAL_THEME_OF_FEAST, 10);
                            ctx.give_items(player, ECHO_CRYSTAL_THEME_OF_CELEBRATION, 10);
                            if ctx.is_mage_class(player) {
                                ctx.give_items(player, STAFF_OF_SENTINEL, 1);
                                ctx.give_items(player, SPIRITSHOT_NO_GRADE, 500);
                            } else {
                                ctx.give_items(player, SWORD_OF_SENTINEL, 1);
                                ctx.give_items(player, SOULSHOT_NO_GRADE, 500);
                            }
                            ctx.add_exp_and_sp(player, 30202, 1339);
                            ctx.give_adena(player, 6331, true);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30284-06.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            COBENDELL => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, ALBERIUS_LETTER) {
                        ctx.take_items(player, ALBERIUS_LETTER, 1);
                        ctx.give_items(player, EVERGREEN_AMULET, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30156-03.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_quest_items(player, EVERGREEN_AMULET)
                        && ctx.get_quest_items_count(player, DRYADS_TEAR) < 10
                    {
                        htmltext = "30156-04.html".to_owned();
                    }
                }
                3 => {
                    if ctx.get_quest_items_count(player, DRYADS_TEAR) >= 10 {
                        ctx.take_items(player, EVERGREEN_AMULET, -1);
                        ctx.take_items(player, DRYADS_TEAR, -1);
                        for medicine in MEDICINES {
                            ctx.give_items(player, medicine, 1);
                        }
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30156-05.html".to_owned();
                    }
                }
                4 => {
                    if ctx.has_at_least_one_quest_item(player, &MEDICINES) {
                        htmltext = "30156-06.html".to_owned();
                    }
                }
                5 => {
                    if ctx.has_at_least_one_quest_item(player, &MEDICINES) {
                        htmltext = "30156-07.html".to_owned();
                    }
                }
                _ => {}
            },
            GARTRANDELL | RAYEN | VELTRESS | BERROS => {
                if let Some(medicine) = sentinel_medicine(npc.id)
                    && ctx.has_all_quest_items(player, &[ALBERIUS_LIST, medicine])
                {
                    ctx.take_items(player, medicine, -1);
                    if !ctx.has_at_least_one_quest_item(player, &MEDICINES) {
                        qs.set_cond(ctx, 6, false);
                    }
                    htmltext = format!("{}-01.html", npc.id);
                }
            }
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

    use super::{ALBERIUS, BERROS, COBENDELL, GARTRANDELL, Q00102SeaOfSporesFever as Quest, RAYEN, VELTRESS};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00102_SeaOfSporesFever";
    const LETTER: i32 = 964;
    const AMULET: i32 = 965;
    const TEAR: i32 = 966;
    const LIST: i32 = 746;
    const MEDICINE1: i32 = 1130;
    const MEDICINE2: i32 = 1131;
    const MEDICINE3: i32 = 1132;
    const MEDICINE4: i32 = 1133;
    const MEDICINE5: i32 = 1134;

    const NPCS: &str = r#"
        <npc id="30284" type="Folk" level="70" name="Alberius"/>
        <npc id="30156" type="Folk" level="70" name="Cobendell"/>
        <npc id="30217" type="Folk" level="70" name="Berros"/>
        <npc id="30219" type="Folk" level="70" name="Veltress"/>
        <npc id="30221" type="Folk" level="70" name="Rayen"/>
        <npc id="30285" type="Folk" level="70" name="Gartrandell"/>
        <npc id="20013" type="Monster" level="20" name="Dryad"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="743" type="Weapon" name="Sword of Sentinel"/>
        <item id="744" type="Weapon" name="Staff of Sentinel"/>
        <item id="746" type="EtcItem" name="Alberius' List"><set name="is_questitem" val="true"/></item>
        <item id="964" type="EtcItem" name="Alberius' Letter"><set name="is_questitem" val="true"/></item>
        <item id="965" type="EtcItem" name="Evergreen Amulet"><set name="is_questitem" val="true"/></item>
        <item id="966" type="EtcItem" name="Dryad's Tear"><set name="is_questitem" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="1130" type="EtcItem" name="Cobendell's Medicine 1"><set name="is_questitem" val="true"/></item>
        <item id="1131" type="EtcItem" name="Cobendell's Medicine 2"><set name="is_questitem" val="true"/></item>
        <item id="1132" type="EtcItem" name="Cobendell's Medicine 3"><set name="is_questitem" val="true"/></item>
        <item id="1133" type="EtcItem" name="Cobendell's Medicine 4"><set name="is_questitem" val="true"/></item>
        <item id="1134" type="EtcItem" name="Cobendell's Medicine 5"><set name="is_questitem" val="true"/></item>
        <item id="1835" type="EtcItem" name="Soulshot"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30284-07.htm", &["30284-02.htm"])
            .plain(&[
                "30284-00.htm",
                "30284-02.htm",
                "30284-03.html",
                "30284-04.html",
                "30284-05.html",
                "30284-06.html",
                "30284-08.htm",
                "30284-09.html",
                "30156-03.html",
                "30156-04.html",
                "30156-05.html",
                "30156-06.html",
                "30156-07.html",
                "30217-01.html",
                "30219-01.html",
                "30221-01.html",
                "30285-01.html",
            ])
            .npcs(&[ALBERIUS, COBENDELL, GARTRANDELL, RAYEN, BERROS, VELTRESS])
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let alberius = t.spawn(ALBERIUS, 20);
        let cobendell = t.spawn(COBENDELL, 30);
        (t, alberius, cobendell)
    }

    #[test]
    fn an_elf_of_level_12_starts_and_gets_the_letter() {
        let (mut t, alberius, _) = setup();
        t.enter_as(1, 12, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, alberius).contains("30284-07.htm"));
        assert!(t.event(1, NAME, "30284-02.htm").contains("30284-02.htm"));
        assert_eq!(t.count(1, LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_is_refused_below_12_and_for_other_races() {
        let (mut t, alberius, _) = setup();
        t.enter_as(1, 11, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, alberius).contains("30284-08.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, alberius).contains("30284-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_tears_the_medicine_the_sentinels_and_the_reward() {
        let (mut t, alberius, cobendell) = setup();
        t.enter_as(1, 20, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, alberius).contains("30284-07.htm"));
        assert!(t.event(1, NAME, "30284-02.htm").contains("30284-02.htm"));
        assert_eq!(t.count(1, LETTER), 1);
        t.take_writes();

        // Cobendell: the letter goes, the amulet comes, cond 2.
        assert!(t.quest_page(1, cobendell).contains("30156-03.html"));
        assert_eq!((t.count(1, LETTER), t.count(1, AMULET)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The dryads: a tear in three of ten kills, ten of them gives cond 3.
        for _ in 0..500 {
            if t.count(1, TEAR) == 10 {
                break;
            }
            let dryad = t.spawn(20013, 60);
            t.kill(1, dryad);
        }
        assert_eq!(t.count(1, TEAR), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Cobendell: the tears and the amulet go, the five medicines come, cond 4.
        assert!(t.quest_page(1, cobendell).contains("30156-05.html"));
        assert_eq!(
            (t.count(1, AMULET), t.count(1, TEAR), t.count(1, MEDICINE5)),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Alberius: the first medicine goes for the list, cond 5 (no sound).
        assert!(t.quest_page(1, alberius).contains("30284-04.html"));
        assert_eq!((t.count(1, MEDICINE1), t.count(1, LIST)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The sentinels take their medicines; the last one that is left sets cond 6.
        for (npc, page, medicine) in [
            (GARTRANDELL, "30285-01.html", MEDICINE5),
            (RAYEN, "30221-01.html", MEDICINE4),
            (VELTRESS, "30219-01.html", MEDICINE3),
        ] {
            let sentinel = t.spawn(npc, 40);
            assert!(t.quest_page(1, sentinel).contains(page));
            assert_eq!(t.count(1, medicine), 0);
            assert!(t.take_writes().is_empty());
        }
        let berros = t.spawn(BERROS, 40);
        assert!(t.quest_page(1, berros).contains("30217-01.html"));
        assert_eq!(t.count(1, MEDICINE2), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Alberius: the sword (a fighter), the potions and the crystals, the exit.
        assert!(t.quest_page(1, alberius).contains("30284-06.html"));
        assert_eq!(t.count(1, 743), 1);
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(t.count(1, 1835), 500);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
