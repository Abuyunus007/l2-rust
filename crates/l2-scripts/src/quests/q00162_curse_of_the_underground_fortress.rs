//! Quest 162: Curse of the Underground Fortress (Unoren sends a non-Dark Elf after skulls and bones).
// Java: dist/game/data/scripts/quests/Q00162_CurseOfTheUndergroundFortress/Q00162_CurseOfTheUndergroundFortress.java
//
// Pattern: a kill count with drop chances up to two limits. The skull monsters drop an elf skull
// (up to 3), the bone monsters a bone fragment (up to 10); the kill that completes both limits
// sets cond 2. Unoren pays when 13 items of the two kinds are in hand (one-time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const UNOREN: i32 = 30147;

// Monsters: id, the drop chance in percent (Java `MONSTERS_SKULLS` and `MONSTERS_BONES`).
const MONSTERS_SKULLS: [(i32, i32); 3] = [
    (20033, 25), // Shade Horror
    (20345, 26), // Dark Terror
    (20371, 23), // Mist Terror
];
const MONSTERS_BONES: [(i32, i32); 3] = [
    (20463, 25), // Dungeon Skeleton Archer
    (20464, 23), // Dungeon Skeleton
    (20504, 26), // Dread Soldier
];

// Items
const BONE_SHIELD: i32 = 625;
const BONE_FRAGMENT: i32 = 1158;
const ELF_SKULL: i32 = 1159;

// Misc
const MIN_LEVEL: i32 = 12;
const REQUIRED_COUNT: i64 = 13;

/// Java `Q00162_CurseOfTheUndergroundFortress`.
pub struct Q00162CurseOfTheUndergroundFortress;

impl Script for Q00162CurseOfTheUndergroundFortress {
    fn name(&self) -> &'static str {
        "Q00162_CurseOfTheUndergroundFortress"
    }

    fn path(&self) -> &'static str {
        "quests/Q00162_CurseOfTheUndergroundFortress"
    }

    fn id(&self) -> i32 {
        162
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[UNOREN]);
        s.add_talk_id(&[UNOREN]);
        let skulls: Vec<i32> = MONSTERS_SKULLS.iter().map(|(id, _)| *id).collect();
        let bones: Vec<i32> = MONSTERS_BONES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&skulls);
        s.add_kill_id(&bones);
        s.register_quest_items(&[BONE_FRAGMENT, ELF_SKULL]);
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
        match event {
            "30147-03.htm" => Some(event.to_owned()),
            "30147-04.htm" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            if let Some(&(_, chance)) = MONSTERS_SKULLS.iter().find(|(id, _)| *id == npc.id) {
                if ctx.get_random(100) < chance {
                    let mut skulls = ctx.get_quest_items_count(killer, ELF_SKULL);
                    if skulls < 3 {
                        ctx.give_items(killer, ELF_SKULL, 1);
                        skulls += 1;
                        if skulls >= 3 && ctx.get_quest_items_count(killer, BONE_FRAGMENT) >= 10 {
                            qs.set_cond(ctx, 2, true);
                        } else {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
            } else if let Some(&(_, chance)) = MONSTERS_BONES.iter().find(|(id, _)| *id == npc.id)
                && ctx.get_random(100) < chance
            {
                let mut bones = ctx.get_quest_items_count(killer, BONE_FRAGMENT);
                if bones < 10 {
                    ctx.give_items(killer, BONE_FRAGMENT, 1);
                    bones += 1;
                    if bones >= 10 && ctx.get_quest_items_count(killer, ELF_SKULL) >= 3 {
                        qs.set_cond(ctx, 2, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) != race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30147-02.htm".to_owned()
                    } else {
                        "30147-01.htm".to_owned()
                    }
                } else {
                    "30147-00.htm".to_owned()
                };
            }
            STARTED => {
                let total = ctx.get_quest_items_count(player, BONE_FRAGMENT)
                    + ctx.get_quest_items_count(player, ELF_SKULL);
                if total >= REQUIRED_COUNT {
                    ctx.give_items(player, BONE_SHIELD, 1);
                    ctx.add_exp_and_sp(player, 22_652, 1_004);
                    ctx.give_adena(player, 24_000, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30147-06.html".to_owned();
                } else {
                    htmltext = "30147-05.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00162_CurseOfTheUndergroundFortress";
    const ADENA: i32 = 57;
    const SHADE_HORROR: i32 = 20033;
    const DUNGEON_SKELETON: i32 = 20464;

    const NPCS: &str = r#"
        <npc id="30147" type="Folk" level="1" name="Unoren"/>
        <npc id="20033" type="Monster" level="20" name="Shade Horror"/>
        <npc id="20464" type="Monster" level="20" name="Dungeon Skeleton"/>"#;

    const ITEMS: &str = r#"
        <item id="625" type="EtcItem" name="Bone Shield"/>
        <item id="1158" type="EtcItem" name="Bone Fragment"><set name="is_questitem" val="true"/></item>
        <item id="1159" type="EtcItem" name="Elf Skull"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00162CurseOfTheUndergroundFortress);
        let pages = Pages::of(&Q00162CurseOfTheUndergroundFortress)
            .page("30147-02.htm", &["30147-03.htm"])
            .page("30147-03.htm", &["30147-04.htm"])
            .plain(&[
                "30147-00.htm",
                "30147-01.htm",
                "30147-04.htm",
                "30147-05.html",
                "30147-06.html",
            ])
            .npcs(&[UNOREN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let unoren = t.spawn(UNOREN, 20);
        (t, unoren)
    }

    #[test]
    fn a_dark_elf_or_a_low_level_character_is_sent_away() {
        let (mut t, unoren) = setup();
        t.enter_as(1, 20, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, unoren).contains("30147-00.htm"));
        t.enter_as(2, 11, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, unoren).contains("30147-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn skulls_and_bones_then_the_reward() {
        let (mut t, unoren) = setup();
        t.enter_as(1, 12, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, unoren).contains("30147-02.htm"));
        assert!(t.event(1, NAME, "30147-03.htm").contains("30147-03.htm"));
        assert!(t.event(1, NAME, "30147-04.htm").contains("30147-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Nothing collected yet: Unoren asks for more.
        assert!(t.quest_page(1, unoren).contains("30147-05.html"));
        assert!(t.take_writes().is_empty());

        // Shade Horrors for the skulls, skeletons for the bones, until 3 + 10: cond 2.
        let mut kills = 0;
        while t.count(1, ELF_SKULL) < 3 || t.count(1, BONE_FRAGMENT) < 10 {
            let mob = if t.count(1, ELF_SKULL) < 3 {
                SHADE_HORROR
            } else {
                DUNGEON_SKELETON
            };
            let spawn = t.spawn(mob, 40);
            t.kill(1, spawn);
            kills += 1;
            assert!(kills < 1_000, "the monsters never dropped enough");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Unoren: 13 items in hand, the shield, exp, 24000 adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, unoren).contains("30147-06.html"));
        assert_eq!(t.count(1, BONE_SHIELD), 1);
        assert_eq!(t.count(1, ADENA), 24_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
