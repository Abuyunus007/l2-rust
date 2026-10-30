//! Quest 456: Don't Know, Don't Care (the Separated Souls; the raid bosses' corpses, command channel).
// Java: dist/game/data/scripts/quests/Q00456_DontKnowDontCare/Q00456_DontKnowDontCare.java
//
// Pattern: a command channel of at least 18 members kills a raid boss, and the attackers of the
// same channel near it get a corpse that gives its essence once per player (`on_first_talk`). With
// the three essences the start NPC pays a reward (daily). The allowed players of each corpse are
// the script's own state (`ctx.state`), the corpse goes away after 5 minutes.

use std::collections::{BTreeSet, HashMap};

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SEPARATED_SOUL: [i32; 8] = [32864, 32865, 32866, 32867, 32868, 32869, 32870, 32891];
const DRAKE_LORD_CORPSE: i32 = 32884;
const BEHEMOTH_LEADER_CORPSE: i32 = 32885;
const DRAGON_BEAST_CORPSE: i32 = 32886;

// Items
const DRAKE_LORD_ESSENCE: i32 = 17251;
const BEHEMOTH_LEADER_ESSENCE: i32 = 17252;
const DRAGON_BEAST_ESSENCE: i32 = 17253;
const REGISTERED: [i32; 3] = [DRAKE_LORD_ESSENCE, BEHEMOTH_LEADER_ESSENCE, DRAGON_BEAST_ESSENCE];

// Misc
const MIN_PLAYERS: usize = 18;
const MIN_LEVEL: i32 = 80;
/// Java `MONSTER_NPCS`: the raid boss and the corpse it leaves.
const MONSTER_NPCS: [(i32, i32); 3] = [
    (25725, DRAKE_LORD_CORPSE),
    (25726, BEHEMOTH_LEADER_CORPSE),
    (25727, DRAGON_BEAST_CORPSE),
];
/// Java `MONSTER_ESSENCES`: the essence of each corpse.
const MONSTER_ESSENCES: [(i32, i32); 3] = [
    (DRAKE_LORD_CORPSE, DRAKE_LORD_ESSENCE),
    (BEHEMOTH_LEADER_CORPSE, BEHEMOTH_LEADER_ESSENCE),
    (DRAGON_BEAST_CORPSE, DRAGON_BEAST_ESSENCE),
];

// Rewards
const WEAPONS: [i32; 14] = [
    15558, // Periel Sword
    15559, // Skull Edge
    15560, // Vigwik Axe
    15561, // Devilish Maul
    15562, // Feather Eye Blade
    15563, // Octo Claw
    15564, // Doubletop Spear
    15565, // Rising Star
    15566, // Black Visage
    15567, // Veniplant Sword
    15568, // Skull Carnium Bow
    15569, // Gemtail Rapier
    15570, // Finale Blade
    15571, // Dominion Crossbow
];
const ARMOR: [i32; 17] = [
    15743, // Sealed Vorpal Helmet
    15746, // Sealed Vorpal Breastplate
    15749, // Sealed Vorpal Gaiters
    15752, // Sealed Vorpal Gauntlets
    15755, // Sealed Vorpal Boots
    15758, // Sealed Vorpal Shield
    15744, // Sealed Vorpal Leather Helmet
    15747, // Sealed Vorpal Leather Breastplate
    15750, // Sealed Vorpal Leather Leggings
    15753, // Sealed Vorpal Leather Gloves
    15756, // Sealed Vorpal Leather Boots
    15745, // Sealed Vorpal Circlet
    15748, // Sealed Vorpal Tunic
    15751, // Sealed Vorpal Stockings
    15754, // Sealed Vorpal Gloves
    15757, // Sealed Vorpal Shoes
    15759, // Sealed Vorpal Sigil
];
const ACCESSORIES: [i32; 3] = [
    15763, // Sealed Vorpal Ring
    15764, // Sealed Vorpal Earring
    15765, // Sealed Vorpal Necklace
];
const ATTRIBUTE_CRYSTALS: [i32; 6] = [
    9552, // Fire Crystal
    9553, // Water Crystal
    9554, // Earth Crystal
    9555, // Wind Crystal
    9556, // Dark Crystal
    9557, // Holy Crystal
];
const BLESSED_SCROLL_ENCHANT_WEAPON_S: i32 = 6577;
const BLESSED_SCROLL_ENCHANT_ARMOR_S: i32 = 6578;
const SCROLL_ENCHANT_WEAPON_S: i32 = 959;
const GEMSTONE_S: i32 = 2134;

/// Java `allowedPlayerMap`: the players that may take the essence of each spawned corpse.
#[derive(Default)]
struct AllowedPlayers(HashMap<ObjectId, BTreeSet<ObjectId>>);

/// Java `Q00456_DontKnowDontCare`.
pub struct Q00456DontKnowDontCare;

impl Script for Q00456DontKnowDontCare {
    fn name(&self) -> &'static str {
        "Q00456_DontKnowDontCare"
    }

    fn path(&self) -> &'static str {
        "quests/Q00456_DontKnowDontCare"
    }

    fn id(&self) -> i32 {
        456
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&SEPARATED_SOUL);
        s.add_talk_id(&SEPARATED_SOUL);
        s.add_first_talk_id(&[DRAKE_LORD_CORPSE, BEHEMOTH_LEADER_CORPSE, DRAGON_BEAST_CORPSE]);
        s.add_talk_id(&[DRAKE_LORD_CORPSE, BEHEMOTH_LEADER_CORPSE, DRAGON_BEAST_CORPSE]);
        let bosses: Vec<i32> = MONSTER_NPCS.iter().map(|(boss, _)| *boss).collect();
        s.add_kill_id(&bosses);
        s.register_quest_items(&REGISTERED);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, false);
        let allowed = ctx
            .state::<AllowedPlayers>()
            .0
            .get(&npc.object_id)
            .is_some_and(|players| players.contains(&player));
        let Some(qs) = qs.filter(|qs| qs.is_cond(ctx, 1)).filter(|_| allowed) else {
            return Some(format!("{}-02.html", npc.id));
        };

        let essence = essence_of(npc.id);
        if ctx.has_quest_items(player, essence) {
            return Some(format!("{}-03.html", npc.id));
        }
        ctx.give_items(player, essence, 1);
        let htmltext = format!("{}-01.html", npc.id);
        if ctx.has_all_quest_items(player, &REGISTERED) {
            qs.set_cond(ctx, 2, true);
        } else {
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if !SEPARATED_SOUL.contains(&npc.id) {
            return Some(htmltext);
        }

        let mut state = qs.state(ctx);
        // Java: `case COMPLETED` falls through to `CREATED` once the quest is available again.
        if state == COMPLETED {
            if !qs.is_now_available(ctx) {
                return Some("32864-02.html".to_owned());
            }
            qs.set_state(ctx, CREATED);
            state = CREATED;
        }
        match state {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32864-01.htm"
                } else {
                    "32864-03.html"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if ctx.has_at_least_one_quest_item(player, &REGISTERED) {
                        "32864-09.html"
                    } else {
                        "32864-08.html"
                    }
                    .to_owned();
                }
                2 => {
                    if ctx.has_all_quest_items(player, &REGISTERED) {
                        reward_player(ctx, player, npc);
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        htmltext = "32864-10.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let qs = player.and_then(|player| ctx.get_quest_state(player, false));
        let mut htmltext = None;
        match event {
            "32864-04.htm" | "32864-05.htm" | "32864-06.htm" => {
                if qs.is_some_and(|qs| qs.is_created(ctx)) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32864-07.htm" => {
                if let Some(qs) = qs
                    && qs.is_created(ctx)
                {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "unspawnRaidCorpse" => {
                // NOTE(java-quirk): the timer is started with the raid boss that died (not with
                // the corpse), so this forgets the boss's entry and deletes the boss, which is
                // already dead; the corpse stays until its own despawn.
                if let Some(npc) = npc {
                    ctx.state::<AllowedPlayers>().0.remove(&npc.object_id);
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Only the killing command channel gets the corpse, and only with 18 members.
        if !ctx.world().is_in_command_channel(killer) {
            return;
        }
        let Some(channel) = ctx.world().command_channel_of(killer) else {
            return;
        };
        if ctx.world().command_channel_member_count(channel) < MIN_PLAYERS {
            return;
        }

        // The attackers of the aggro list that are players of the same channel, near the boss.
        let mut allowed = BTreeSet::new();
        let attackers: Vec<ObjectId> = ctx
            .world()
            .aggro_list(npc.object_id)
            .iter()
            .map(|info| info.attacker)
            .collect();
        for attacker in attackers {
            if ctx.world().player(attacker).is_none() {
                continue;
            }
            if ctx.world().command_channel_of(attacker) == Some(channel)
                && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, attacker, true)
            {
                allowed.insert(attacker);
            }
        }

        if allowed.is_empty() {
            return;
        }
        // This depends on the boss respawn delay being at least 5 minutes.
        let corpse = corpse_of(npc.id);
        if let Some(spawned) = ctx.add_spawn_at(corpse, npc.object_id, true, 0) {
            ctx.state::<AllowedPlayers>().0.insert(spawned.object_id, allowed);
            ctx.start_quest_timer("unspawnRaidCorpse", 300_000, Some(npc), None, false);
        }
    }
}

/// Java `MONSTER_NPCS.get(boss)`: the corpse of a raid boss.
fn corpse_of(boss: i32) -> i32 {
    MONSTER_NPCS
        .iter()
        .find(|(id, _)| *id == boss)
        .map_or(boss, |(_, corpse)| *corpse)
}

/// Java `MONSTER_ESSENCES.get(corpse)`: the essence a corpse gives.
fn essence_of(corpse: i32) -> i32 {
    MONSTER_ESSENCES
        .iter()
        .find(|(id, _)| *id == corpse)
        .map_or(0, |(_, essence)| *essence)
}

/// Java `rewardPlayer(player, npc)`: one roll of the reward table, then the NPC tells the player.
fn reward_player(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) {
    let chance = ctx.get_random(10_000);
    let mut count = 1;
    let reward = if chance < 170 {
        ctx.get_random_entry(&ARMOR)
    } else if chance < 200 {
        ctx.get_random_entry(&ACCESSORIES)
    } else if chance < 270 {
        ctx.get_random_entry(&WEAPONS)
    } else if chance < 325 {
        Some(BLESSED_SCROLL_ENCHANT_WEAPON_S)
    } else if chance < 425 {
        Some(BLESSED_SCROLL_ENCHANT_ARMOR_S)
    } else if chance < 925 {
        ctx.get_random_entry(&ATTRIBUTE_CRYSTALS)
    } else if chance < 1100 {
        Some(SCROLL_ENCHANT_WEAPON_S)
    } else {
        count = 3;
        Some(GEMSTONE_S)
    };
    let Some(reward) = reward else {
        return;
    };

    ctx.give_items(player, reward, count);
    let item_name = ctx.world().item_template_name(reward).unwrap_or_default();
    let player_name = ctx.player_name(player);
    ctx.broadcast_npc_say(
        npc,
        ChatType::NpcGeneral,
        NpcStringId::S1ReceivedAS2ItemAsARewardFromTheSeparatedSoul,
        &[&player_name, &item_name],
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to, rows};

    const NAME: &str = "Q00456_DontKnowDontCare";

    const NPCS: &str = r#"
        <npc id="32864" type="Folk" level="70" name="Separated Soul"/>
        <npc id="32884" type="Folk" level="70" name="Drake Lord Corpse"/>
        <npc id="32885" type="Folk" level="70" name="Behemoth Leader Corpse"/>
        <npc id="32886" type="Folk" level="70" name="Dragon Beast Corpse"/>
        <npc id="25725" type="RaidBoss" level="80" name="Drake Lord"/>"#;

    /// The item templates: the essences, adena and every reward (a missing template is refused).
    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17251" type="EtcItem" name="Drake Lord Essence"><set name="is_questitem" val="true"/></item>
        <item id="17252" type="EtcItem" name="Behemoth Leader Essence"><set name="is_questitem" val="true"/></item>
        <item id="17253" type="EtcItem" name="Dragon Beast Essence"><set name="is_questitem" val="true"/></item>"#,
        );
        let rewards = WEAPONS
            .iter()
            .chain(ARMOR.iter())
            .chain(ACCESSORIES.iter())
            .chain(ATTRIBUTE_CRYSTALS.iter())
            .copied()
            .chain([
                BLESSED_SCROLL_ENCHANT_WEAPON_S,
                BLESSED_SCROLL_ENCHANT_ARMOR_S,
                SCROLL_ENCHANT_WEAPON_S,
                GEMSTONE_S,
            ]);
        for id in rewards {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Reward {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        xml
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00456DontKnowDontCare);
        let pages = Pages::of(&Q00456DontKnowDontCare)
            .page("32864-01.htm", &["32864-04.htm"])
            .page("32864-04.htm", &["32864-05.htm"])
            .page("32864-05.htm", &["32864-06.htm"])
            .page("32864-06.htm", &["32864-07.htm"])
            .plain(&[
                "32864-02.html",
                "32864-03.html",
                "32864-07.htm",
                "32864-08.html",
                "32864-09.html",
                "32864-10.html",
                "32884-01.html",
                "32884-02.html",
                "32884-03.html",
                "32885-01.html",
                "32885-02.html",
                "32885-03.html",
                "32886-01.html",
                "32886-02.html",
                "32886-03.html",
            ])
            .npcs(&[32864, 32884, 32885, 32886]);
        let items = items();
        let mut t = ScriptTest::with_pages(registry, NPCS, &items, pages);
        let soul = t.spawn(32864, 20);
        (t, soul)
    }

    /// Lets the player 1 take the essence of `corpse` (what the command channel's aggro list does).
    fn allow(t: &mut ScriptTest, corpse: Oid) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.state::<AllowedPlayers>()
                .0
                .entry(corpse)
                .or_default()
                .insert(Oid(1));
        });
    }

    /// Accepts the quest from the first page (the links 04, 05, 06 and the start 07).
    fn accept(t: &mut ScriptTest, _soul: Oid) {
        assert!(t.event(1, NAME, "32864-04.htm").contains("32864-04.htm"));
        assert!(t.event(1, NAME, "32864-05.htm").contains("32864-05.htm"));
        assert!(t.event(1, NAME, "32864-06.htm").contains("32864-06.htm"));
        assert!(t.event(1, NAME, "32864-07.htm").contains("32864-07.htm"));
    }

    /// Talks to the corpse and returns the page it shows first.
    fn corpse_page(t: &mut ScriptTest, corpse: Oid) -> String {
        t.talk(1, corpse);
        pages_to(&t.take(), 1).into_iter().next().unwrap_or_default()
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, soul) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_writes_the_cond_and_the_state() {
        let (mut t, soul) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-01.htm"));
        assert!(t.event(1, NAME, "32864-04.htm").contains("32864-04.htm"));
        assert!(t.event(1, NAME, "32864-05.htm").contains("32864-05.htm"));
        assert!(t.event(1, NAME, "32864-06.htm").contains("32864-06.htm"));
        assert!(t.event(1, NAME, "32864-07.htm").contains("32864-07.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, soul).contains("32864-08.html"));
    }

    #[test]
    fn a_corpse_gives_its_essence_once_to_an_allowed_player() {
        let (mut t, soul) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-01.htm"));
        accept(&mut t, soul);
        t.take_writes();

        // Not allowed: the "02" page, nothing given.
        let corpse = t.spawn(32884, 40);
        assert!(corpse_page(&mut t, corpse).contains("32884-02.html"));
        assert_eq!(t.count(1, DRAKE_LORD_ESSENCE), 0);

        // Allowed: the essence, the middle sound; the second time the "03" page.
        allow(&mut t, corpse);
        assert!(corpse_page(&mut t, corpse).contains("32884-01.html"));
        assert_eq!(t.count(1, DRAKE_LORD_ESSENCE), 1);
        assert!(corpse_page(&mut t, corpse).contains("32884-03.html"));
        assert_eq!(t.count(1, DRAKE_LORD_ESSENCE), 1);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_essences_set_cond_2_and_the_reward_ends_the_day() {
        let (mut t, soul) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, soul).contains("32864-01.htm"));
        accept(&mut t, soul);
        t.take_writes();

        for (corpse_id, page) in [(32884, "32884-01.html"), (32885, "32885-01.html")] {
            let corpse = t.spawn(corpse_id, 40);
            allow(&mut t, corpse);
            assert!(corpse_page(&mut t, corpse).contains(page));
        }
        let last = t.spawn(32886, 40);
        allow(&mut t, last);
        assert!(corpse_page(&mut t, last).contains("32886-01.html"));
        assert_eq!(t.take_writes().last(), Some(&rows::update(NAME, "cond", "2")));

        // The soul pays: one reward item (3 gemstones at most), the essences go, the day ends.
        assert!(t.quest_page(1, soul).contains("32864-10.html"));
        let rewards: i64 = WEAPONS
            .iter()
            .chain(ARMOR.iter())
            .chain(ACCESSORIES.iter())
            .chain(ATTRIBUTE_CRYSTALS.iter())
            .chain(
                [
                    BLESSED_SCROLL_ENCHANT_WEAPON_S,
                    BLESSED_SCROLL_ENCHANT_ARMOR_S,
                    SCROLL_ENCHANT_WEAPON_S,
                    GEMSTONE_S,
                ]
                .iter(),
            )
            .map(|id| t.count(1, *id))
            .sum();
        assert!((1..=3).contains(&rewards), "{rewards}");
        assert_eq!(t.count(1, DRAGON_BEAST_ESSENCE), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: NAME.to_owned(),
                    repeatable: false,
                },
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, soul).contains("32864-02.html"));
    }

    #[test]
    fn a_kill_without_a_command_channel_spawns_no_corpse() {
        // The corpse needs a command channel of 18 members; a solo kill gives nothing.
        let (mut t, _soul) = setup();
        t.enter(1, 80, vec![]);
        let boss = t.spawn(25725, 60);
        t.kill(1, boss);
        assert!(t.npcs_of(DRAKE_LORD_CORPSE).is_empty());
    }

    #[test]
    fn the_unspawn_timer_forgets_the_boss_and_deletes_it() {
        // The timer is started with the boss that died (Java's quirk), so it is that boss that goes.
        let (mut t, _soul) = setup();
        let boss = t.spawn(25725, 60);
        allow(&mut t, boss);
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(boss).unwrap();
            out = Q00456DontKnowDontCare.on_event(ctx, "unspawnRaidCorpse", Some(npc), None);
        });
        assert_eq!(out, None);
        assert!(!t.in_world(boss));
        let mut left = 0;
        t.world.run_as_script(NAME, |ctx| {
            left = ctx.state::<AllowedPlayers>().0.len();
        });
        assert_eq!(left, 0);
    }
}
