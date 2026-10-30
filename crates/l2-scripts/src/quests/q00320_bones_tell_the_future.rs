//! Quest 320: Bones Tell the Future (Tetrarch Kaitar in the Dark Elven village, Skeleton Hunters).
// Java: dist/game/data/scripts/quests/Q00320_BonesTellTheFuture/Q00320_BonesTellTheFuture.java
//
// Pattern: a party kill, a random member at cond N (`docs/design/scripts.md`, "Quest patterns"):
// `random_party_member_state` draws the member, the bone goes to the killer.

use l2_util::ObjectId;
use l2_world::script::race;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TETRACH_KAITAR: i32 = 30359;

// Item
const BONE_FRAGMENT: i32 = 809;

// Misc
const MIN_LEVEL: i32 = 10;
const REQUIRED_BONE_COUNT: i64 = 10;
const DROP_CHANCE: f64 = 0.18;

// Monsters
const SKELETON_HUNTER: i32 = 20517;
const SKELETON_HUNTER_ARCHER: i32 = 20518;

/// Java `Q00320_BonesTellTheFuture`.
pub struct Q00320BonesTellTheFuture;

impl Script for Q00320BonesTellTheFuture {
    fn name(&self) -> &'static str {
        "Q00320_BonesTellTheFuture"
    }

    fn path(&self) -> &'static str {
        "quests/Q00320_BonesTellTheFuture"
    }

    fn id(&self) -> i32 {
        320
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TETRACH_KAITAR]);
        s.add_talk_id(&[TETRACH_KAITAR]);
        s.add_kill_id(&[SKELETON_HUNTER, SKELETON_HUNTER_ARCHER]);
        s.register_quest_items(&[BONE_FRAGMENT]);
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
            && event == "30359-04.htm"
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): the bone goes to the killer, not to the member drawn, while the
        // drawn member's quest moves to cond 2 (`random_party_member_state` returns that member).
        if let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc))
            && ctx.give_item_randomly(
                killer,
                Some(npc),
                BONE_FRAGMENT,
                1,
                1,
                REQUIRED_BONE_COUNT,
                DROP_CHANCE,
                true,
            )
        {
            qs.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_race(player) == race::DARK_ELF {
                if ctx.player_level(player) >= MIN_LEVEL {
                    "30359-03.htm"
                } else {
                    "30359-02.htm"
                }
            } else {
                "30359-00.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if ctx.get_quest_items_count(player, BONE_FRAGMENT) >= REQUIRED_BONE_COUNT {
                htmltext = "30359-06.html".to_owned();
                ctx.give_adena(player, 8470, true);
                qs.exit_quest(ctx, true, true);
            } else {
                htmltext = "30359-05.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00320_BonesTellTheFuture";

    const NPCS: &str = r#"
        <npc id="30359" type="Folk" level="1" name="Tetrach Kaitar"/>
        <npc id="20517" type="Monster" level="11" name="Skeleton Hunter"/>
        <npc id="20518" type="Monster" level="12" name="Skeleton Hunter Archer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="809" type="EtcItem" name="Bone Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00320BonesTellTheFuture);
        let pages = Pages::of(&Q00320BonesTellTheFuture)
            .page("30359-03.htm", &["30359-04.htm"])
            .plain(&[
                "30359-00.htm",
                "30359-02.htm",
                "30359-04.htm",
                "30359-05.html",
                "30359-06.html",
            ])
            .npcs(&[TETRACH_KAITAR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kaitar = t.spawn(TETRACH_KAITAR, 20);
        (t, kaitar)
    }

    /// Kills Skeleton Hunters until the bones are at the limit; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let hunter = t.spawn(SKELETON_HUNTER, 60);
            t.kill(1, hunter);
            if t.count(1, BONE_FRAGMENT) >= REQUIRED_BONE_COUNT {
                return kills;
            }
        }
        panic!("no 10 bones after 400 kills");
    }

    #[test]
    fn only_dark_elves_of_level_ten_start_it() {
        let (mut t, kaitar) = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, kaitar).contains("30359-00.htm"));
        t.enter_as(2, 9, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(2, kaitar).contains("30359-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_bones_then_adena() {
        let (mut t, kaitar) = setup();
        t.enter_as(1, 10, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, kaitar).contains("30359-03.htm"));
        assert!(t.event(1, NAME, "30359-04.htm").contains("30359-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kaitar).contains("30359-05.html"));

        // The bones go to the killer; the drawn member (the killer) moves to cond 2 at the limit.
        let kills = hunt(&mut t);
        assert!(kills >= 10, "{kills}");
        assert_eq!(t.count(1, BONE_FRAGMENT), REQUIRED_BONE_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: 8470 adena, the bones go, the quest ends repeatable.
        t.take();
        t.talk(1, kaitar);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30359-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), 8470);
        assert_eq!(t.count(1, BONE_FRAGMENT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
