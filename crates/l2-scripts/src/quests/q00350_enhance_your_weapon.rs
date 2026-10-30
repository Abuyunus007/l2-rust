//! Quest 350: Enhance Your Weapon (Trader Kakai, Gatekeeper Tamil and Blacksmith Kiyou; the soul
//! crystals of the level-up table).
// Java: dist/game/data/scripts/quests/Q00350_EnhanceYourWeapon/Q00350_EnhanceYourWeapon.java
//
// Pattern: a repeatable quest that hands out the soul crystal stages (red, green, blue) and
// leaves at `exit.htm`. The monsters of `LevelUpCrystalData` level the crystals: the crystal
// skill (2096) on a monster adds the caster to its absorb list (`onSkillSee`), the kill levels
// the crystals of the killer or of the party (`levelSoulCrystals`). The kill and skill-see ids
// come from the loaded table (`setup_with`, Java's constructor loop).

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::QuestSound;
use l2_world::script::soul_crystal::{AbsorbCrystalType, SoulCrystal};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, World};

// NPCs
const STARTING_NPCS: [i32; 3] = [30115, 30856, 30194];

// Items
const RED_SOUL_CRYSTAL0_ID: i32 = 4629;
const GREEN_SOUL_CRYSTAL0_ID: i32 = 4640;
const BLUE_SOUL_CRYSTAL0_ID: i32 = 4651;

// Misc
const MIN_LEVEL: i32 = 40;
/// The soul crystal skill (`skill.getId() != 2096`).
const SOUL_CRYSTAL_SKILL: i32 = 2096;

/// Java `Q00350_EnhanceYourWeapon`.
pub struct Q00350EnhanceYourWeapon;

const NAME: &str = "Q00350_EnhanceYourWeapon";

impl Script for Q00350EnhanceYourWeapon {
    fn name(&self) -> &'static str {
        NAME
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
    }

    fn setup_with(&self, s: &mut ScriptSetup, world: &World) {
        self.setup(s);
        for npc_id in world.level_up_crystal_data().npc_ids() {
            s.add_skill_see_id(&[npc_id]);
            s.add_kill_id(&[npc_id]);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.is_attackable(npc)
            && ctx
                .world()
                .level_up_crystal_data()
                .npc_soul_info(npc.id)
                .is_some()
        {
            level_soul_crystals(ctx, npc, killer);
        }
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if skill.id != SOUL_CRYSTAL_SKILL {
            return;
        }
        if ctx.is_dead(caster) {
            return;
        }
        if !ctx.is_attackable(npc)
            || ctx.is_dead(npc.object_id)
            || ctx
                .world()
                .level_up_crystal_data()
                .npc_soul_info(npc.id)
                .is_none()
        {
            return;
        }
        ctx.world_mut().add_absorber(npc.object_id, caster);
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

/// Java `getSCForPlayer(player)`: the one crystal of the table the player carries while the
/// quest is started; `None` with none or with two.
fn get_sc_for_player(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<SoulCrystal> {
    let started = ctx
        .player_quest_state(player, NAME)
        .is_some_and(|qs| qs.is_started());
    if !started {
        return None;
    }
    let data = ctx.world().level_up_crystal_data();
    let mut ret = None;
    for item in &ctx.world().player(player)?.data.items {
        if item.loc != "INVENTORY" && item.loc != "PAPERDOLL" {
            continue;
        }
        let Some(sc) = data.soul_crystal(item.item_id) else {
            continue;
        };
        if ret.is_some() {
            return None;
        }
        ret = Some(*sc);
    }
    ret
}

/// Java `isPartyLevelingMonster(npcId)`: a level of the monster is not `LAST_HIT`.
fn is_party_leveling_monster(ctx: &ScriptCtx<'_>, npc_id: i32) -> bool {
    ctx.world()
        .level_up_crystal_data()
        .npc_soul_info(npc_id)
        .is_some_and(|info| info.values().any(|i| i.absorb_type != AbsorbCrystalType::LastHit))
}

/// Java `levelCrystal(player, sc, mob)`.
fn level_crystal(ctx: &mut ScriptCtx<'_>, player: ObjectId, sc: Option<SoulCrystal>, mob: ScriptNpc) {
    let Some(sc) = sc else {
        return;
    };
    let Some(info) = ctx.world().level_up_crystal_data().npc_soul_info(mob.id) else {
        return;
    };
    // If the crystal level is way too high for this mob, say that we can't increase it.
    let Some(chance) = info.get(&sc.level).map(|i| i.chance) else {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::TheSoulCrystalIsRefusingToAbsorbTheSoul),
        );
        return;
    };
    if ctx.get_random(100) <= chance {
        ctx.world_mut()
            .exchange_soul_crystal(player, sc.item_id, sc.leveled_item_id, false);
    } else {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::TheSoulCrystalWasNotAbleToAbsorbTheSoul),
        );
    }
}

/// Java `levelSoulCrystals(mob, killer)`: the crystals the kill levels.
fn level_soul_crystals(ctx: &mut ScriptCtx<'_>, mob: ScriptNpc, killer: ObjectId) {
    // Map to store players and their crystals.
    let mut players: HashMap<ObjectId, SoulCrystal> = HashMap::new();
    let mut max_sc_level = 0;
    let levels = |ctx: &ScriptCtx<'_>, level: i32| {
        ctx.world()
            .level_up_crystal_data()
            .npc_soul_info(mob.id)
            .is_some_and(|info| info.contains_key(&level))
    };
    let party = ctx.world().party_of(killer);

    // Gather eligible players and their Soul Crystals.
    if let Some(party) = party
        && is_party_leveling_monster(ctx, mob.id)
    {
        let range = f64::from(ctx.alt_party_range());
        for member in ctx.world().party_members(party) {
            // Skip invalid or out-of-range members.
            if ctx.distance_3d(member, killer).is_none_or(|d| d > range) {
                continue;
            }
            // Skip if no valid Soul Crystal is present.
            let Some(sc) = get_sc_for_player(ctx, member) else {
                continue;
            };
            players.insert(member, sc);
            if sc.level > max_sc_level && levels(ctx, sc.level) {
                // Track the highest-level Soul Crystal.
                max_sc_level = sc.level;
            }
        }
    } else if let Some(sc) = get_sc_for_player(ctx, killer) {
        players.insert(killer, sc);
        if sc.level > max_sc_level && levels(ctx, sc.level) {
            max_sc_level = sc.level;
        }
    }

    // No eligible players or crystals; exit early.
    if players.is_empty() {
        return;
    }

    // Get leveling info for the mob and highest-level crystal.
    let Some(level_info) = ctx
        .world()
        .level_up_crystal_data()
        .npc_soul_info(mob.id)
        .and_then(|info| info.get(&max_sc_level).copied())
    else {
        // No leveling info available for this mob.
        return;
    };

    // Handle special absorb skill checks.
    if level_info.skill_needed {
        if !ctx.world().is_absorbed(mob.object_id) {
            // Absorption not initialized.
            ctx.world_mut().reset_absorb_list(mob.object_id);
            return;
        }
        let max_hp = f64::from(ctx.npc_max_hp(mob));
        let misused = ctx
            .world()
            .absorber_hp(mob.object_id, killer)
            .is_none_or(|hp| hp > max_hp / 2.0);
        if misused {
            // Absorption skill was misused.
            ctx.world_mut().reset_absorb_list(mob.object_id);
            return;
        }
    }

    // Process leveling based on absorb type.
    let members = party.map(|p| ctx.world().party_members(p));
    match level_info.absorb_type {
        AbsorbCrystalType::PartyOneRandom => {
            if let Some(members) = members.filter(|m| !m.is_empty()) {
                let lucky = members[index(ctx.get_random(len(&members)))];
                level_crystal(ctx, lucky, players.get(&lucky).copied(), mob);
            } else {
                level_crystal(ctx, killer, players.get(&killer).copied(), mob);
            }
        }
        AbsorbCrystalType::PartyRandom => {
            if let Some(mut party_members) = members {
                while ctx.get_random(100) < 33 && !party_members.is_empty() {
                    let lucky = party_members.remove(index(ctx.get_random(len(&party_members))));
                    if let Some(lucky_data) = players.get(&lucky).copied() {
                        level_crystal(ctx, lucky, Some(lucky_data), mob);
                    }
                }
            } else if ctx.get_random(100) < 33 {
                level_crystal(ctx, killer, players.get(&killer).copied(), mob);
            }
        }
        AbsorbCrystalType::FullParty => {
            if let Some(members) = members {
                for member in members {
                    level_crystal(ctx, member, players.get(&member).copied(), mob);
                }
            } else {
                level_crystal(ctx, killer, players.get(&killer).copied(), mob);
            }
        }
        AbsorbCrystalType::LastHit => {
            level_crystal(ctx, killer, players.get(&killer).copied(), mob);
        }
    }

    // Reset absorb state after processing.
    ctx.world_mut().reset_absorb_list(mob.object_id);
}

fn len(list: &[ObjectId]) -> i32 {
    i32::try_from(list.len()).unwrap_or(i32::MAX)
}

fn index(random: i32) -> usize {
    usize::try_from(random).unwrap_or(0)
}

/// Java `check(player)`: any item of the soul crystal ids 4629..4664.
fn has_any_soul_crystal_of_the_table(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    (4629..4665).any(|item_id| ctx.has_quest_items(player, item_id))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_util::ObjectId as Oid;
    use l2_world::script::soul_crystal::LevelUpCrystalData;
    use l2_world::{ItemData, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, Sent, decode, item, name_of, names_to, pages_to, rows};

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

    // ------------------------------------------------------------------ leveling (S-API-05)

    const TIMAK_ORC: i32 = 20583;
    const LAST_HIT_MOB: i32 = 20584;
    const NOT_IN_TABLE: i32 = 20585;
    const PARTY_BOSS: i32 = 25163;

    const LEVELING_NPCS: &str = r#"
        <npc id="20583" type="Monster" level="30" name="Timak Orc"/>
        <npc id="20584" type="Monster" level="30" name="Timak Orc Archer"/>
        <npc id="20585" type="Monster" level="30" name="Not In The Table"/>
        <npc id="25163" type="Monster" level="80" name="Boss"/>"#;

    const LEVELING_ITEMS: &str = r#"
        <item id="4629" type="EtcItem" name="Red Soul Crystal - Stage 0"/>
        <item id="4630" type="EtcItem" name="Red Soul Crystal - Stage 1"/>
        <item id="4631" type="EtcItem" name="Red Soul Crystal - Stage 2"/>
        <item id="4640" type="EtcItem" name="Green Soul Crystal - Stage 0"/>
        <item id="4641" type="EtcItem" name="Green Soul Crystal - Stage 1"/>"#;

    /// A small table with Java's shapes: a skill monster (Timak Orc), a last-hit monster of levels
    /// 0..1, a full-party monster of level 1 only.
    const TABLE: &str = r#"<list>
        <crystal>
            <item itemId="4629" level="0" leveledItemId="4630" />
            <item itemId="4630" level="1" leveledItemId="4631" />
            <item itemId="4631" level="2" leveledItemId="4632" />
            <item itemId="4640" level="0" leveledItemId="4641" />
        </crystal>
        <npc>
            <item npcId="20583"><detail chance="9" skill="true" maxLevel="1" /></item>
            <item npcId="20584"><detail chance="9" maxLevel="1" /></item>
            <item npcId="25163"><detail chance="60" absorbType="FULL_PARTY" levelList="1" /></item>
        </npc>
    </list>"#;

    /// `multiplier` is `SoulCrystalChanceMultiplier`: 100 makes every chance 100.
    fn leveling_setup(table: &str, multiplier: f64) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00350EnhanceYourWeapon);
        let data = LevelUpCrystalData::from_xml(table, multiplier).unwrap();
        ScriptTest::new_with(registry, LEVELING_NPCS, LEVELING_ITEMS, &[], |world, _| {
            world.set_level_up_crystal_data(Arc::new(data));
        })
    }

    /// `player` enters with `items` and the quest started.
    fn enter_started(t: &mut ScriptTest, player: i32, items: Vec<ItemData>) {
        t.enter(player, 40, items);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
    }

    /// Java's `onSkillSee` of the crystal skill (the item skill of a soul crystal, cast on `mob`).
    fn see_skill(t: &mut ScriptTest, player: i32, mob: Oid, skill_id: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(mob).unwrap();
            let skill = SkillKey {
                id: skill_id,
                level: 1,
            };
            Q00350EnhanceYourWeapon.on_skill_see(ctx, npc, Oid(player), skill, &[mob], false);
        });
    }

    /// The system message ids sent to `to`.
    fn messages_to(out: &Sent, to: i32) -> Vec<i32> {
        out.iter()
            .filter(|(p, b)| *p == to && name_of(b) == "SYSTEM_MESSAGE")
            .map(|(_, b)| decode::<SystemMessage>(b).id as i32)
            .collect()
    }

    #[test]
    fn a_kill_after_the_absorb_levels_the_crystal() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let orc = t.spawn(TIMAK_ORC, 40);
        // The absorb counts when the monster had at most half of its HP left (`getAbsorbedHp() >
        // getMaxHp() / 2.0` is misuse); the test world's maximum HP is 0.
        t.world.set_current_hp(orc, 0.0, false);
        see_skill(&mut t, 1, orc, SOUL_CRYSTAL_SKILL);
        assert!(t.world.is_absorbed(orc));
        assert_eq!(t.world.absorber_hp(orc, Oid(1)), Some(0.0));
        t.take();
        t.kill(1, orc);
        assert_eq!(t.count(1, 4629), 0);
        assert_eq!(t.count(1, 4630), 1);
        t.wait_ms(1_000);
        let out = t.take();
        assert_eq!(
            messages_to(&out, 1),
            [
                SystemMessageId::TheSoulCrystalSucceededInAbsorbingASoul as i32,
                SystemMessageId::YouHaveEarnedS1 as i32
            ]
        );
        assert!(names_to(&out, 1).contains(&"INVENTORY_UPDATE".to_owned()));
        // The absorb list is reset after the kill.
        assert!(!t.world.is_absorbed(orc));
    }

    #[test]
    fn an_absorb_above_half_the_hp_is_misuse() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let orc = t.spawn(TIMAK_ORC, 40);
        t.world.set_current_hp(orc, 0.0, false);
        // A second absorb keeps the HP of the last one (`setAbsorbedHp`).
        see_skill(&mut t, 1, orc, SOUL_CRYSTAL_SKILL);
        // (The test world caps the HP at 1.)
        t.world.set_current_hp(orc, 10.0, false);
        see_skill(&mut t, 1, orc, SOUL_CRYSTAL_SKILL);
        assert!(t.world.absorber_hp(orc, Oid(1)).is_some_and(|hp| hp > 0.0));
        t.kill(1, orc);
        assert_eq!(t.count(1, 4629), 1);
        assert!(!t.world.is_absorbed(orc));
    }

    #[test]
    fn a_skill_monster_killed_without_the_absorb_levels_nothing() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let orc = t.spawn(TIMAK_ORC, 40);
        t.kill(1, orc);
        assert_eq!(t.count(1, 4629), 1);
        assert_eq!(t.count(1, 4630), 0);
    }

    #[test]
    fn another_skill_than_the_crystal_does_not_absorb() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let orc = t.spawn(TIMAK_ORC, 40);
        see_skill(&mut t, 1, orc, 1177);
        assert!(!t.world.is_absorbed(orc));
    }

    #[test]
    fn a_last_hit_monster_levels_without_the_skill_and_refuses_a_too_high_crystal() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4630, 1)]);
        let mob = t.spawn(LAST_HIT_MOB, 40);
        t.kill(1, mob);
        assert_eq!(t.count(1, 4631), 1);
        t.take();
        // Level 2 is above the monster's levels (0..1): the info of level 0 is taken
        // (`maxSCLevel` stays 0) and the crystal refuses.
        let mob = t.spawn(LAST_HIT_MOB, 40);
        t.kill(1, mob);
        assert_eq!(t.count(1, 4631), 1);
        assert_eq!(
            messages_to(&t.take(), 1),
            [SystemMessageId::TheSoulCrystalIsRefusingToAbsorbTheSoul as i32]
        );

        // NOTE(java-quirk): on a monster without a level-0 entry a too low crystal says nothing:
        // `maxSCLevel` 0 has no info, so `levelSoulCrystals` returns before `levelCrystal`.
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let boss = t.spawn(PARTY_BOSS, 40);
        t.take();
        t.kill(1, boss);
        assert_eq!(t.count(1, 4629), 1);
        assert!(messages_to(&t.take(), 1).is_empty());
    }

    #[test]
    fn a_failed_roll_says_the_crystal_could_not_absorb() {
        // Chance 9 with the seeded RNG: the kills go on until one roll fails.
        let mut t = leveling_setup(TABLE, 1.0);
        let mut failed = false;
        for player in 1..=20 {
            enter_started(&mut t, player, vec![item(200 + player, 4640, 1)]);
            let mob = t.spawn(LAST_HIT_MOB, 40);
            t.take();
            t.kill(player, mob);
            let messages = messages_to(&t.take(), player);
            if t.count(player, 4641) == 0 {
                assert_eq!(
                    messages,
                    [SystemMessageId::TheSoulCrystalWasNotAbleToAbsorbTheSoul as i32]
                );
                assert_eq!(t.count(player, 4640), 1);
                failed = true;
                break;
            }
        }
        assert!(failed);
    }

    #[test]
    fn two_crystals_or_no_quest_level_nothing() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1), item(201, 4640, 1)]);
        t.enter(2, 40, vec![item(300, 4629, 1)]);
        let mob = t.spawn(LAST_HIT_MOB, 40);
        t.kill(1, mob);
        assert_eq!(t.count(1, 4629), 1);
        assert_eq!(t.count(1, 4630), 0);
        let mob = t.spawn(LAST_HIT_MOB, 40);
        t.kill(2, mob);
        assert_eq!(t.count(2, 4630), 0);
    }

    #[test]
    fn a_full_party_monster_levels_every_member_in_range() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4630, 1)]);
        enter_started(&mut t, 2, vec![item(300, 4630, 1)]);
        t.party(1, 2);
        let boss = t.spawn(PARTY_BOSS, 40);
        t.kill(1, boss);
        assert_eq!(t.count(1, 4631), 1);
        assert_eq!(t.count(2, 4631), 1);
    }

    #[test]
    fn only_the_monsters_of_the_table_get_the_listeners() {
        let mut t = leveling_setup(TABLE, 100.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let other = t.spawn(NOT_IN_TABLE, 40);
        t.kill(1, other);
        assert_eq!(t.count(1, 4629), 1);
        // Without the table nothing is registered.
        let mut t = leveling_setup("<list/>", 1.0);
        enter_started(&mut t, 1, vec![item(200, 4629, 1)]);
        let mob = t.spawn(LAST_HIT_MOB, 40);
        t.kill(1, mob);
        assert_eq!(t.count(1, 4629), 1);
    }
}
