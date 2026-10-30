//! Quest 300: Hunting Leto Lizardman (Rath in the Town of Aden sends level 34 players after the Leto lizardmen).
// Java: dist/game/data/scripts/quests/Q00300_HuntingLetoLizardman/Q00300_HuntingLetoLizardman.java
//
// Pattern: a party kill quest with a count target (`docs/design/scripts.md`, "Quest patterns"):
// a random party member in cond 1 gets a bracelet from a Leto lizardman with a chance per monster,
// the 60th bracelet sets cond 2; Rath gives a random reward (adena or an animal item) and ends
// the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RATH: i32 = 30126;

// Items
const BRACELET_OF_LIZARDMAN: i32 = 7139;
const REWARD_ADENA: (i32, i64) = (57, 30000);
const REWARD_ANIMAL_BONE: (i32, i64) = (1872, 50);
const REWARD_ANIMAL_SKIN: (i32, i64) = (1867, 50);

// Misc
const MIN_LEVEL: i32 = 34;
const REQUIRED_BRACELET_COUNT: i64 = 60;

/// Java `MOBS_SAC`: the Leto lizardmen and their drop chance out of 1000.
fn bracelet_chance(npc_id: i32) -> i32 {
    match npc_id {
        20577 => 360, // Leto Lizardman
        20578 => 390, // Leto Lizardman Archer
        20579 => 410, // Leto Lizardman Soldier
        20580 => 790, // Leto Lizardman Warrior
        20582 => 890, // Leto Lizardman Overlord
        _ => 0,
    }
}

/// Java `Q00300_HuntingLetoLizardman`.
pub struct Q00300HuntingLetoLizardman;

impl Script for Q00300HuntingLetoLizardman {
    fn name(&self) -> &'static str {
        "Q00300_HuntingLetoLizardman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00300_HuntingLetoLizardman"
    }

    fn id(&self) -> i32 {
        300
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RATH]);
        s.add_talk_id(&[RATH]);
        s.add_kill_id(&[20577, 20578, 20579, 20580, 20582]);
        s.register_quest_items(&[BRACELET_OF_LIZARDMAN]);
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
            "30126-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30126-06.html" => {
                if ctx.get_quest_items_count(player, BRACELET_OF_LIZARDMAN) >= REQUIRED_BRACELET_COUNT {
                    ctx.take_items(player, BRACELET_OF_LIZARDMAN, -1);
                    let rand = ctx.get_random(1000);
                    if rand < 500 {
                        ctx.give_items(player, REWARD_ADENA.0, REWARD_ADENA.1);
                    } else if rand < 750 {
                        ctx.give_items(player, REWARD_ANIMAL_SKIN.0, REWARD_ANIMAL_SKIN.1);
                    } else if rand < 1000 {
                        ctx.give_items(player, REWARD_ANIMAL_BONE.0, REWARD_ANIMAL_BONE.1);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30126-07.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(member) = ctx.get_random_party_member_cond(killer, 1) {
            let qs = ctx.get_quest_state(member, false);
            if let Some(qs) = qs
                && qs.is_cond(ctx, 1)
                && ctx.get_random(1000) < bracelet_chance(npc.id)
            {
                // NOTE(java-quirk): the bracelet goes to the killer, the cond 2 to the party member
                // drawn above (Java gives to `player` and sets the cond on `partyMember`).
                ctx.give_items(killer, BRACELET_OF_LIZARDMAN, 1);
                if ctx.get_quest_items_count(killer, BRACELET_OF_LIZARDMAN) == REQUIRED_BRACELET_COUNT {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30126-01.htm"
                } else {
                    "30126-02.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30126-04.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, BRACELET_OF_LIZARDMAN) >= REQUIRED_BRACELET_COUNT {
                        htmltext = "30126-05.html".to_owned();
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

    const NAME: &str = "Q00300_HuntingLetoLizardman";

    const NPCS: &str = r#"
        <npc id="30126" type="Folk" level="70" name="Rath"/>
        <npc id="20577" type="Monster" level="36" name="Leto Lizardman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7139" type="EtcItem" name="Bracelet of Lizardman"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1867" type="EtcItem" name="Animal Skin"><set name="is_stackable" val="true"/></item>
        <item id="1872" type="EtcItem" name="Animal Bone"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00300HuntingLetoLizardman);
        let pages = Pages::of(&Q00300HuntingLetoLizardman)
            .page("30126-01.htm", &["30126-03.htm"])
            .plain(&["30126-02.htm", "30126-03.htm", "30126-04.html", "30126-07.html"])
            .page("30126-05.html", &["30126-06.html"])
            .plain(&["30126-06.html"])
            .npcs(&[RATH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rath = t.spawn(RATH, 20);
        (t, rath)
    }

    #[test]
    fn a_level_thirty_three_character_is_sent_away() {
        let (mut t, rath) = setup();
        t.enter_as(1, 33, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, rath).contains("30126-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn sixty_bracelets_then_a_random_reward() {
        let (mut t, rath) = setup();
        t.enter_as(1, 34, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, rath).contains("30126-01.htm"));
        assert!(t.event(1, NAME, "30126-03.htm").contains("30126-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, rath).contains("30126-04.html"));

        // The 60th bracelet sets cond 2 (`setCond(2, true)`).
        for _ in 0..2000 {
            if t.count(1, BRACELET_OF_LIZARDMAN) >= REQUIRED_BRACELET_COUNT {
                break;
            }
            let lizard = t.spawn(20577, 60);
            t.kill(1, lizard);
        }
        assert_eq!(t.count(1, BRACELET_OF_LIZARDMAN), REQUIRED_BRACELET_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Rath: the bracelets go, one reward (adena, skin or bone), `exitQuest(true, true)`.
        assert!(t.quest_page(1, rath).contains("30126-05.html"));
        assert!(t.event(1, NAME, "30126-06.html").contains("30126-06.html"));
        assert_eq!(t.count(1, BRACELET_OF_LIZARDMAN), 0);
        let rewards = (t.count(1, 57), t.count(1, 1867), t.count(1, 1872));
        assert!(
            rewards == (30000, 0, 0) || rewards == (0, 50, 0) || rewards == (0, 0, 50),
            "{rewards:?}"
        );
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
