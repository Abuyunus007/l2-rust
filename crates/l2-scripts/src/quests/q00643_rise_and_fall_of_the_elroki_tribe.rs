//! Quest 643: Rise and Fall of the Elroki Tribe (Singsing in the Dion/Plains area, Karakawei in the
//! Dinosaur Tracks; the plains dinosaurs drop their bones, 300 of them buy five pieces of a weapon
//! or 1374 adena each).
// Java: dist/game/data/scripts/quests/Q00643_RiseAndFallOfTheElrokiTribe/Q00643_RiseAndFallOfTheElrokiTribe.java
//
// Pattern: a repeatable kill drop (the bones: one or two per kill of the first group, a chance
// for the second group and for the Deinonychus), a random reward (pieces) and the adena per bone.
// Karakawei's first talk is a field of the Java script (`isFirstTalk`): it is per script, so it
// lives in `ctx.state` (D: script state is owned by the world).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SINGSING: i32 = 32106;
const KARAKAWEI: i32 = 32117;

// Item
const BONES_OF_A_PLAINS_DINOSAUR: i32 = 8776;

// Misc
const MIN_LEVEL: i32 = 75;
const CHANCE_MOBS1: i32 = 116;
const CHANCE_MOBS2: i32 = 360;
const CHANCE_DEINO: i32 = 558;
const ADENA_PER_BONE: i64 = 1374;
const BONES_FOR_PIECES: i64 = 300;

// Rewards
const PIECE: [i32; 11] = [
    8712, // Sirra's Blade Edge
    8713, // Sword of Ipos Blade
    8714, // Barakiel's Axe Piece
    8715, // Behemoth's Tuning Fork Piece
    8716, // Naga Storm Piece
    8717, // Tiphon's Spear Edge
    8718, // Shyeed's Bow Shaft
    8719, // Sobekk's Hurricane Edge
    8720, // Themis' Tongue Piece
    8721, // Cabrio's Hand Head
    8722, // Daimon Crystal Fragment
];

// Mobs
const MOBS1: [i32; 19] = [
    22200, 22201, 22202, 22204, 22205, 22208, 22209, 22210, 22211, 22212, 22213, 22219, 22220, 22221, 22222,
    22224, 22225, 22226, 22227,
];
const MOBS2: [i32; 4] = [22742, 22743, 22744, 22745];
const DEINONYCHUS: i32 = 22203;

/// Java `isFirstTalk`: Karakawei's first talk in the game (the field of the script).
struct KarakaweiTalks {
    first_talk: bool,
}

impl Default for KarakaweiTalks {
    fn default() -> Self {
        Self { first_talk: true }
    }
}

/// Java `Q00643_RiseAndFallOfTheElrokiTribe`.
pub struct Q00643RiseAndFallOfTheElrokiTribe;

impl Script for Q00643RiseAndFallOfTheElrokiTribe {
    fn name(&self) -> &'static str {
        "Q00643_RiseAndFallOfTheElrokiTribe"
    }

    fn path(&self) -> &'static str {
        "quests/Q00643_RiseAndFallOfTheElrokiTribe"
    }

    fn id(&self) -> i32 {
        643
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SINGSING]);
        s.add_talk_id(&[SINGSING, KARAKAWEI]);
        s.add_kill_id(&MOBS1);
        s.add_kill_id(&MOBS2);
        s.add_kill_id(&[DEINONYCHUS]);
        s.register_quest_items(&[BONES_OF_A_PLAINS_DINOSAUR]);
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
            "32106-02.htm" | "32106-04.htm" | "32106-05.html" | "32106-10.html" | "32106-13.html"
            | "32117-02.html" | "32117-06.html" | "32117-07.html" => htmltext = Some(event.to_owned()),
            "quest_accept" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some("32106-03.html".to_owned());
                } else {
                    htmltext = Some("32106-07.html".to_owned());
                }
            }
            "32106-09.html" => {
                let bones = ctx.get_quest_items_count(player, BONES_OF_A_PLAINS_DINOSAUR);
                ctx.give_adena(player, ADENA_PER_BONE * bones, true);
                ctx.take_items(player, BONES_OF_A_PLAINS_DINOSAUR, -1);
                htmltext = Some(event.to_owned());
            }
            "exit" => {
                if ctx.has_quest_items(player, BONES_OF_A_PLAINS_DINOSAUR) {
                    let bones = ctx.get_quest_items_count(player, BONES_OF_A_PLAINS_DINOSAUR);
                    ctx.give_adena(player, ADENA_PER_BONE * bones, true);
                    htmltext = Some("32106-12.html".to_owned());
                } else {
                    htmltext = Some("32106-11.html".to_owned());
                }
                qs.exit_quest(ctx, true, true);
            }
            "exchange" => {
                if ctx.get_quest_items_count(player, BONES_OF_A_PLAINS_DINOSAUR) < BONES_FOR_PIECES {
                    htmltext = Some("32117-04.html".to_owned());
                } else {
                    if let Some(piece) = ctx.get_random_entry(&PIECE) {
                        ctx.reward_items(player, piece, 5);
                    }
                    ctx.take_items(player, BONES_OF_A_PLAINS_DINOSAUR, BONES_FOR_PIECES);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some("32117-05.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(player, 1)`: no range check.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if MOBS1.contains(&npc.id) {
            let bones: i64 = if ctx.get_random(1000) < CHANCE_MOBS1 { 2 } else { 1 };
            ctx.reward_items(member, BONES_OF_A_PLAINS_DINOSAUR, bones);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
        if MOBS2.contains(&npc.id) && ctx.get_random(1000) < CHANCE_MOBS2 {
            ctx.reward_items(member, BONES_OF_A_PLAINS_DINOSAUR, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
        if npc.id == DEINONYCHUS && ctx.get_random(1000) < CHANCE_DEINO {
            ctx.reward_items(member, BONES_OF_A_PLAINS_DINOSAUR, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32106-01.htm".to_owned()
            } else {
                "32106-06.html".to_owned()
            };
        } else if state == STARTED {
            if npc.id == SINGSING {
                htmltext = if ctx.has_quest_items(player, BONES_OF_A_PLAINS_DINOSAUR) {
                    "32106-08.html".to_owned()
                } else {
                    "32106-14.html".to_owned()
                };
            } else if npc.id == KARAKAWEI {
                let talks = ctx.state::<KarakaweiTalks>();
                htmltext = if talks.first_talk {
                    talks.first_talk = false;
                    "32117-01.html".to_owned()
                } else {
                    "32117-03.html".to_owned()
                };
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

    const NAME: &str = "Q00643_RiseAndFallOfTheElrokiTribe";

    const NPCS: &str = r#"
        <npc id="32106" type="Folk" level="70" name="Singsing"/>
        <npc id="32117" type="Folk" level="70" name="Karakawei"/>
        <npc id="22200" type="Monster" level="60" name="Ornithomimus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8776" type="EtcItem" name="Bones of a Plains Dinosaur"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8712" type="EtcItem" name="Sirra's Blade Edge"><set name="is_stackable" val="true"/></item>
        <item id="8713" type="EtcItem" name="Sword of Ipos Blade"><set name="is_stackable" val="true"/></item>
        <item id="8714" type="EtcItem" name="Barakiel's Axe Piece"><set name="is_stackable" val="true"/></item>
        <item id="8715" type="EtcItem" name="Behemoth's Tuning Fork Piece"><set name="is_stackable" val="true"/></item>
        <item id="8716" type="EtcItem" name="Naga Storm Piece"><set name="is_stackable" val="true"/></item>
        <item id="8717" type="EtcItem" name="Tiphon's Spear Edge"><set name="is_stackable" val="true"/></item>
        <item id="8718" type="EtcItem" name="Shyeed's Bow Shaft"><set name="is_stackable" val="true"/></item>
        <item id="8719" type="EtcItem" name="Sobekk's Hurricane Edge"><set name="is_stackable" val="true"/></item>
        <item id="8720" type="EtcItem" name="Themis' Tongue Piece"><set name="is_stackable" val="true"/></item>
        <item id="8721" type="EtcItem" name="Cabrio's Hand Head"><set name="is_stackable" val="true"/></item>
        <item id="8722" type="EtcItem" name="Daimon Crystal Fragment"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00643RiseAndFallOfTheElrokiTribe);
        let pages = Pages::of(&Q00643RiseAndFallOfTheElrokiTribe)
            .page("32106-01.htm", &["32106-02.htm"])
            .page("32106-02.htm", &["quest_accept", "32106-04.htm"])
            .plain(&[
                "32106-03.html",
                "32106-04.htm",
                "32106-06.html",
                "32106-07.html",
                "32106-11.html",
                "32106-12.html",
                "32106-14.html",
            ])
            .page("32106-08.html", &["32106-09.html", "32106-13.html"])
            .page("32106-09.html", &["exit"])
            .plain(&["32106-13.html"])
            .page("32117-01.html", &["32117-02.html"])
            .page("32117-02.html", &["exchange"])
            .page("32117-03.html", &["exchange"])
            .plain(&["32117-04.html", "32117-05.html"])
            .npcs(&[SINGSING, KARAKAWEI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let singsing = t.spawn(SINGSING, 20);
        let karakawei = t.spawn(KARAKAWEI, 22);
        (t, singsing, karakawei)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, singsing, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, singsing).contains("32106-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_bones_buy_pieces_and_the_bones_buy_adena_when_the_quest_ends() {
        let (mut t, singsing, karakawei) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, singsing).contains("32106-01.htm"));
        assert!(t.event(1, NAME, "32106-02.htm").contains("32106-02.htm"));
        assert!(t.event(1, NAME, "quest_accept").contains("32106-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Karakawei: the first talk, then the exchange page.
        assert!(t.quest_page(1, karakawei).contains("32117-01.html"));
        assert!(t.quest_page(1, karakawei).contains("32117-03.html"));

        // 299 bones by the script, then the bones of a kill make 300 for the exchange.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BONES_OF_A_PLAINS_DINOSAUR, 299);
        });
        t.take();
        let mob = t.spawn(22200, 40);
        t.kill(1, mob);
        let bones = t.count(1, BONES_OF_A_PLAINS_DINOSAUR);
        assert!(bones >= BONES_FOR_PIECES);
        assert!(t.take_writes().is_empty());

        assert!(t.quest_page(1, karakawei).contains("32117-03.html"));
        assert!(t.event(1, NAME, "exchange").contains("32117-05.html"));
        let pieces: i64 = PIECE.iter().map(|p| t.count(1, *p)).sum();
        assert_eq!(pieces, 5);
        assert_eq!(t.count(1, BONES_OF_A_PLAINS_DINOSAUR), bones - BONES_FOR_PIECES);

        // Singsing pays the rest (ten more bones first, so the talk has some) and the exit ends.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BONES_OF_A_PLAINS_DINOSAUR, 10);
        });
        t.take();
        let left = bones - BONES_FOR_PIECES + 10;
        assert!(t.quest_page(1, singsing).contains("32106-08.html"));
        assert!(t.event(1, NAME, "32106-09.html").contains("32106-09.html"));
        assert_eq!(t.count(1, BONES_OF_A_PLAINS_DINOSAUR), 0);
        assert_eq!(t.count(1, 57), ADENA_PER_BONE * left);
        assert!(t.event(1, NAME, "exit").contains("32106-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
