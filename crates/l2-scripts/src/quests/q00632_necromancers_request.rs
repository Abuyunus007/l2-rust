//! Quest 632: Necromancer's Request (the Mysterious Wizard in the Gludin area; the vampires drop
//! the vampire hearts, the undead drop the zombie brains; 200 hearts buy 120000 adena).
// Java: dist/game/data/scripts/quests/Q00632_NecromancersRequest/Q00632_NecromancersRequest.java
//
// Pattern: two kill drops (a brain without a limit, a heart that sets cond 2 and a memo state
// at 200 hearts, on every later kill too), the memo states that the talk reads, and the reward
// that takes the hearts and sets the memo back.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MYSTERIOUS_WIZARD: i32 = 31522;

// Items
const VAMPIRES_HEART: i32 = 7542;
const ZOMBIES_BRAIN: i32 = 7543;

// Misc
const MIN_LEVEL: i32 = 63;
const REQUIRED_ITEM_COUNT: i64 = 200;
const ADENA_REWARD: i64 = 120_000;

/// Java `BRAIN_MONSTERS`: the brain drop chance of each undead.
const BRAIN_MONSTERS: [(i32, f64); 13] = [
    (21547, 0.565), // Corrupted Knight
    (21548, 0.484), // Resurrected Knight
    (21549, 0.585), // Corrupted Guard
    (21550, 0.597), // Corrupted Guard
    (21551, 0.673), // Resurrected Guard
    (21552, 0.637), // Resurrected Guard
    (21555, 0.575), // Slaughter Executioner
    (21556, 0.560), // Slaughter Executioner
    (21562, 0.631), // Guillotine's Ghost
    (21571, 0.758), // Ghost of Rebellion Soldier
    (21576, 0.647), // Ghost of Guillotine
    (21577, 0.625), // Ghost of Guillotine
    (21579, 0.766), // Ghost of Rebellion Leader
];

/// Java `HEART_MONSTERS`: the heart drop chance of each vampire.
const HEART_MONSTERS: [(i32, f64); 15] = [
    (21568, 0.452), // Devil Bat
    (21569, 0.484), // Devil Bat
    (21573, 0.499), // Atrox
    (21582, 0.522), // Vampire Soldier
    (21585, 0.413), // Vampire Magician
    (21586, 0.496), // Vampire Adept
    (21587, 0.519), // Vampire Warrior
    (21588, 0.428), // Vampire Wizard
    (21589, 0.439), // Vampire Wizard
    (21590, 0.428), // Vampire Magister
    (21591, 0.502), // Vampire Magister
    (21592, 0.370), // Vampire Magister
    (21593, 0.592), // Vampire Warlord
    (21594, 0.554), // Vampire Warlord
    (21595, 0.392), // Vampire Warlord
];

/// Java memo states (`setMemoState`).
const MEMO_STARTED: i32 = 11;
const MEMO_HEARTS_READY: i32 = 12;

/// Java `Q00632_NecromancersRequest`.
pub struct Q00632NecromancersRequest;

impl Script for Q00632NecromancersRequest {
    fn name(&self) -> &'static str {
        "Q00632_NecromancersRequest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00632_NecromancersRequest"
    }

    fn id(&self) -> i32 {
        632
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MYSTERIOUS_WIZARD]);
        s.add_talk_id(&[MYSTERIOUS_WIZARD]);
        let brains: Vec<i32> = BRAIN_MONSTERS.iter().map(|(id, _)| *id).collect();
        let hearts: Vec<i32> = HEART_MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&brains);
        s.add_kill_id(&hearts);
        s.register_quest_items(&[VAMPIRES_HEART, ZOMBIES_BRAIN]);
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
            "31522-104.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, MEMO_STARTED);
                    htmltext = Some(event.to_owned());
                }
            }
            "31522-201.html" => htmltext = Some(event.to_owned()),
            "31522-202.html" => {
                if ctx.get_quest_items_count(player, VAMPIRES_HEART) >= REQUIRED_ITEM_COUNT {
                    ctx.take_items(player, VAMPIRES_HEART, -1);
                    ctx.give_adena(player, ADENA_REWARD, true);
                    qs.set_memo_state(ctx, MEMO_STARTED);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31522-203.html".to_owned());
                }
            }
            "31522-204.html" => {
                ctx.take_items(player, VAMPIRES_HEART, -1);
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(player, -1, 3, npc)`: the killer counts three times.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if let Some(&(_, chance)) = BRAIN_MONSTERS.iter().find(|(id, _)| *id == npc.id) {
            // The drop goes to the killer (Java's `player`), with no limit.
            ctx.give_item_randomly(killer, Some(npc), ZOMBIES_BRAIN, 1, 1, 0, chance, true);
        } else if let Some(&(_, chance)) = HEART_MONSTERS.iter().find(|(id, _)| *id == npc.id) {
            ctx.give_item_randomly(killer, Some(npc), VAMPIRES_HEART, 1, 1, 0, chance, true);
            if ctx.get_quest_items_count(killer, VAMPIRES_HEART) >= REQUIRED_ITEM_COUNT {
                // NOTE(java-quirk): every kill from 200 hearts on sets cond 2 and the memo again.
                qs.set_cond(ctx, 2, false);
                qs.set_memo_state(ctx, MEMO_HEARTS_READY);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31522-101.htm".to_owned()
            } else {
                "31522-103.htm".to_owned()
            };
        } else if state == STARTED {
            if qs.is_memo_state(ctx, MEMO_STARTED) {
                htmltext = "31522-106.html".to_owned();
            } else if qs.is_memo_state(ctx, MEMO_HEARTS_READY)
                && ctx.get_quest_items_count(player, VAMPIRES_HEART) >= REQUIRED_ITEM_COUNT
            {
                htmltext = "31522-105.html".to_owned();
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

    const NAME: &str = "Q00632_NecromancersRequest";

    const NPCS: &str = r#"
        <npc id="31522" type="Folk" level="70" name="Mysterious Wizard"/>
        <npc id="21547" type="Monster" level="60" name="Corrupted Knight"/>
        <npc id="21568" type="Monster" level="60" name="Devil Bat"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7542" type="EtcItem" name="Vampire's Heart"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7543" type="EtcItem" name="Zombie's Brain"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00632NecromancersRequest);
        let pages = Pages::of(&Q00632NecromancersRequest)
            .page("31522-101.htm", &["31522-104.htm"])
            .plain(&[
                "31522-103.htm",
                "31522-104.htm",
                "31522-106.html",
                "31522-203.html",
            ])
            .page("31522-105.html", &["31522-202.html", "31522-204.html"])
            .page("31522-202.html", &["31522-201.html", "31522-204.html"])
            .plain(&["31522-201.html", "31522-204.html"])
            .npcs(&[MYSTERIOUS_WIZARD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wizard = t.spawn(MYSTERIOUS_WIZARD, 20);
        (t, wizard)
    }

    #[test]
    fn a_character_below_level_63_is_sent_away() {
        let (mut t, wizard) = setup();
        t.enter(1, 62, vec![]);
        assert!(t.quest_page(1, wizard).contains("31522-103.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn two_hundred_hearts_pay_the_adena_and_the_quest_ends() {
        let (mut t, wizard) = setup();
        t.enter(1, 63, vec![]);
        assert!(t.quest_page(1, wizard).contains("31522-101.htm"));
        assert!(t.event(1, NAME, "31522-104.htm").contains("31522-104.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "11"),
            ]
        );
        assert!(t.quest_page(1, wizard).contains("31522-106.html"));

        // Brains drop with no limit.
        let mut kills = 0;
        while t.count(1, ZOMBIES_BRAIN) == 0 {
            let brain = t.spawn(21547, 40);
            t.kill(1, brain);
            kills += 1;
            assert!(kills < 100, "no brain dropped");
        }
        assert!(t.take_writes().is_empty());

        // 199 hearts by the script, then the kills of vampires make the 200th.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), VAMPIRES_HEART, 199);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, VAMPIRES_HEART) < REQUIRED_ITEM_COUNT {
            let bat = t.spawn(21568, 40);
            t.kill(1, bat);
            kills += 1;
            assert!(kills < 100, "no heart dropped");
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "memoState", "12")
            ]
        );

        assert!(t.quest_page(1, wizard).contains("31522-105.html"));
        assert!(t.event(1, NAME, "31522-202.html").contains("31522-202.html"));
        assert_eq!(t.count(1, VAMPIRES_HEART), 0);
        assert_eq!(t.count(1, 57), ADENA_REWARD);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "11")]);

        // `exitQuest` from the page: the rest of the items go and the quest ends.
        assert!(t.event(1, NAME, "31522-204.html").contains("31522-204.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
