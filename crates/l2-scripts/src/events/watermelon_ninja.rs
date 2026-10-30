//! Watermelon Ninja: feed nectar to a watermelon seed, then hit the grown watermelon for prizes.
// Java: dist/game/data/scripts/events/WatermelonNinja/WatermelonNinja.java
// Java: dist/game/data/scripts/events/WatermelonNinja/config.xml

use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::broadcast_say_text;
use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

// NPCs
const MANAGER: i32 = 31860;
const INITIAL_WATERMELONS: [i32; 2] = [13271, 13275];
const WATERMELONS_LIST: [i32; 3] = [13272, 13273, 13274];
const HONEY_WATERMELONS_LIST: [i32; 3] = [13276, 13277, 13278];

// Items
const CHRONO_LIST: [i32; 5] = [4202, 5133, 5817, 7058, 8350];

// Skill
const NECTAR_SKILL: i32 = 2005;

// Dialogs.
const INITIAL_SPAWN_TEXT: [&str; 7] = [
    "What are you looking at?",
    "Are you my mommy?",
    "If you raise me well, you'll get prizes! Or not...",
    "Impressive, aren't?",
    "Obey me!!",
    "Ta-da! Here i am",
    "Please! Give me some nectar! I'm hungry!",
];
const EVOLVE_SPAWN_TEXT: [&str; 4] = [
    "Look at you, you managed to evolve me!",
    "Come, kill me if you can...",
    "Are you feeling lucky today?",
    "You wouldn't be able to hit a poor watermelon, would you?",
];
const DIE_TEXT: [&str; 4] = [
    "Now look at that!!, you're done!",
    "I'm not afraid of you.",
    "Better luck next time",
    "Argh, are you happy? Beating a poor watermelon...",
];
const NOCHRONO_TEXT: [&str; 5] = [
    "You cannot kill me without... Chrono",
    "Hehe... keep trying...",
    "Nice try...",
    "Tired?",
    "Go go! haha...",
];
const CHRONO_TEXT: [&str; 5] = [
    "Arghh... Chrono weapon...",
    "My end is coming...",
    "Please leave me!",
    "Heeellpppp...",
    "Somebody help me please...",
];
const INITIAL_WATERMELONS_TEXT: [&str; 4] = [
    "Dont hit me! Gimme a nectar!!",
    "There's no point in hitting me...\nyou know what to do...",
    "Stop hitting me, i need nectar!!",
    "God... ok, continue hitting me...",
];
const WATERMELON_TEXT: [&str; 5] = [
    "Arghh you are killing me...",
    "My end is coming...",
    "Please leave me!",
    "Heeellpppp...",
    "Somebody help me please...",
];
const NECTAR_TEXT: [&str; 5] = [
    "Plase give me more...",
    "Hmmm.. More.. I need more...",
    "I would like you more, if you give me more...",
    "Hmmmmmmm...",
    "My favourite...",
];

/// Java `ItemChanceHolder(id, chance, count)`.
type Drop = (i32, i32, i64);

/// Java `DROPLIST.get(npcId)`: the prizes of a grown watermelon.
fn droplist(npc_id: i32) -> Option<&'static [Drop]> {
    let list: &'static [Drop] = match npc_id {
        13272 => &[
            (1539, 70, 1), // Top-grade HP Replenishing Potion
            (1870, 50, 1), // Coal
            (1872, 50, 1), // Animal Bone
            (1865, 50, 1), // Varnish
            (2287, 50, 1), // Recipe: Atuba Hammer (100%)
            (2267, 50, 1), // Recipe: Gastraphetes (100%)
            (2276, 50, 1), // Recipe: Maingauche (100%)
            (2289, 50, 1), // Recipe: Staff of Life (100%)
            (2272, 50, 1), // Recipe: Sword of Revolution (100%)
            (2049, 50, 1), // Atuba Hammer Head
            (2029, 50, 1), // Gastraphetes Shaft
            (2038, 50, 1), // Maingauche Edge
            (2051, 50, 1), // Staff of Life Shaft
            (2034, 50, 1), // Sword of Revolution Blade
        ],
        13273 => &[
            (1539, 70, 1), // Top-grade HP Replenishing Potion
            (1880, 50, 1), // Steel
            (1877, 50, 1), // Adamantite Nugget
            (1876, 50, 1), // Mithril Ore
            (1882, 50, 1), // Leather
            (1879, 50, 1), // Cokes
            (1881, 50, 1), // Coarse Bone Powder
            (1875, 50, 1), // Stone of Purity
            (2060, 50, 1), // Stormbringer Blade
            (2068, 50, 1), // Stick of Faith Shaft
            (4096, 50, 1), // Sealed Blue Wolf Glove Fabric
            (4090, 50, 1), // Sealed Blue Wolf Boot Design
            (4073, 50, 1), // Sealed Avadon Glove Fragment
            (4098, 50, 1), // Sealed Avadon Boots Design
            (2075, 50, 1), // Orcish Glaive Blade
            (2059, 50, 1), // Flamberge Blade
            (2074, 50, 1), // Frozen Bow Shaft
            (2067, 50, 1), // Crystal Staff Head
            (4080, 50, 1), // Blue Wolf Gaiter Material
            (2063, 50, 1), // Battle Axe Head
            (2301, 50, 1), // Recipe: Battle Axe (100%)
            (4982, 50, 1), // Recipe: Blue Wolf Gaiters (60%)
            (2305, 50, 1), // Recipe: Crystal Staff (100%)
            (2312, 50, 1), // Recipe: Frozen Bow (100%)
            (3017, 50, 1), // Recipe: Divine Gloves (100%)
            (2234, 50, 1), // Recipe: Divine Stockings (100%)
            (2297, 50, 1), // Recipe: Flamberge (100%)
            (3012, 50, 1), // Recipe: Full Plate Helmet (100%)
            (3019, 50, 1), // Recipe: Full Plate Shield (100%)
            (2317, 50, 1), // Recipe: Bec de Corbin (100%)
            (4959, 50, 1), // Recipe: Sealed Avadon Boots (60%)
            (4953, 50, 1), // Recipe: Sealed Avadon Gloves (60%)
            (4992, 50, 1), // Recipe: Sealed Blue Wolf Boots (60%)
            (4998, 50, 1), // Recipe: Sealed Blue Wolf Gloves (60%)
            (2306, 50, 1), // Recipe: Stick of Faith (100%)
            (2298, 50, 1), // Recipe: Stormbringer (100%)
        ],
        13274 => &[
            (160, 5, 1),   // Battle Axe
            (192, 5, 1),   // Crystal Staff
            (281, 5, 1),   // Frozen Bow
            (71, 5, 1),    // Flamberge
            (298, 5, 1),   // Orcish Glaive
            (193, 5, 1),   // Stick of Faith
            (72, 5, 1),    // Stormbringer
            (2463, 5, 1),  // Divine Gloves
            (473, 5, 1),   // Divine Stockings
            (442, 5, 1),   // Divine Tunic
            (401, 5, 1),   // Drake Leather Armor
            (2437, 5, 1),  // Drake Leather Boots
            (356, 5, 1),   // Full Plate Armor
            (2414, 5, 1),  // Full Plate Helmet
            (2497, 5, 1),  // Full Plate Shield
            (1538, 50, 1), // Improved Scroll of Escape
            (3936, 50, 1), // Blessed Scroll of Resurrection
            (5592, 50, 1), // Greater CP Potion
            (1540, 50, 1), // High-grade HP Recovery Potion
            (1459, 50, 1), // C-grade Crystal
            (952, 50, 1),  // Scroll: Enchant C-grade Armor
            (951, 50, 1),  // Scroll: Enchant C-grade Weapon
            (1890, 50, 1), // Mithril Alloy
            (4041, 50, 1), // Mold Hardener
            (1893, 50, 1), // Oriharukon
            (1886, 50, 1), // Silver Mold
        ],
        13276 => &[
            (187, 20, 1),  // Atuba Hammer
            (278, 20, 1),  // Gastraphetes
            (224, 20, 1),  // Maingauche
            (189, 20, 1),  // Staff of Life
            (129, 20, 1),  // Sword of Revolution
            (294, 20, 1),  // War Pick
            (5592, 50, 1), // Greater CP Potion
            (1458, 50, 1), // D-grade Crystal
            (956, 50, 1),  // Scroll: Enchant D-grade Armor
            (955, 50, 1),  // Scroll: Enchant D-grade Weapon
            (3929, 70, 1), // Scroll: Acumen
            (3927, 70, 1), // Scroll: Death Whisper
            (3926, 70, 1), // Scroll: Guidance
            (3930, 70, 1), // Scroll: Haste
            (4218, 70, 1), // Scroll: Mana Regeneration
            (4042, 50, 1), // Enria
            (1890, 50, 1), // Mithril Alloy
            (4041, 50, 1), // Mold Hardener
            (4040, 50, 1), // Mold Lubricant
            (1886, 50, 1), // Silver Mold
            (1887, 50, 1), // Varnish of Purity
        ],
        13277 => &[
            (5592, 60, 1), // Greater CP Potion
            (1540, 60, 1), // High-grade HP Recovery Potion
            (1877, 50, 1), // Adamantite Nugget
            (4043, 50, 1), // Asofe
            (1881, 50, 1), // Coarse Bone Powder
            (1879, 50, 1), // Cokes
            (1885, 50, 1), // High-grade Suede
            (1876, 50, 1), // Mithril Ore
            (4039, 50, 1), // Mold Glue
            (1874, 50, 1), // Oriharukon Ore
            (1880, 50, 1), // Steel
            (1883, 50, 1), // Steel Mold
            (1875, 50, 1), // Stone of Purity
            (1889, 50, 1), // Synthetic Braid
            (1888, 50, 1), // Synthetic Cokes
            (1887, 50, 1), // Varnish of Purity
            (4071, 50, 1), // Avadon Robe Fabric
            (5530, 50, 1), // Berserker Blade Edge
            (4078, 50, 1), // Blue Wolf Breastplate Part
            (2107, 50, 1), // Dark Screamer Edge
            (1988, 50, 1), // Divine Tunic Fabric
            (2121, 50, 1), // Eminence Bow Shaft
            (2108, 50, 1), // Fisted Blade Piece
            (1986, 50, 1), // Full Plate Armor Temper
            (2093, 50, 1), // Poleaxe Blade
            (2109, 50, 1), // Akat Longbow Shaft
            (4072, 50, 1), // Sealed Avadon Circlet Pattern
            (4088, 50, 1), // Sealed Blue Wolf Helmet Design
            (4089, 50, 1), // Sealed Doom Helmet Pattern
            (2095, 50, 1), // Sword of Nightmare Blade
            (4951, 50, 1), // Recipe: Avadon Robe (60%)
            (5436, 50, 1), // Recipe: Berserker Blade (100%)
            (4981, 50, 1), // Recipe: Blue Wolf Breastplate (60%)
            (2345, 50, 1), // Recipe: Dark Screamer (100%)
            (2233, 50, 1), // Recipe: Divine Tunic (100%)
            (2359, 50, 1), // Recipe: Eminence Bow (100%)
            (2346, 50, 1), // Recipe: Fisted Blade (100%)
            (2231, 50, 1), // Recipe: Full Plate Armor (100%)
            (2330, 50, 1), // Recipe: Homunkulus' Sword (100%)
            (4985, 50, 1), // Recipe: Doom Leather Armor (60%)
            (2331, 50, 1), // Recipe: Poleaxe (100%)
            (2341, 50, 1), // Recipe: Sage's Staff (100%)
            (4952, 50, 1), // Recipe: Sealed Avadon Circlet (60%)
            (4990, 50, 1), // Recipe: Sealed Blue Wolf Helmet (60%)
            (4991, 50, 1), // Recipe: Sealed Doom Helmet (60%)
            (2333, 50, 1), // Recipe: Sword of Nightmare (100%)
        ],
        13278 => &[
            (5286, 5, 1),  // Berserker Blade
            (233, 5, 1),   // Dark Screamer
            (286, 5, 1),   // Eminence Bow
            (265, 5, 1),   // Fisted Blade
            (84, 5, 1),    // Homunkulus' Sword
            (95, 5, 1),    // Poleaxe
            (200, 5, 1),   // Sage's Staff
            (134, 5, 1),   // Sword of Nightmare
            (2406, 5, 1),  // Avadon Robe
            (358, 5, 1),   // Blue Wolf Breastplate
            (2380, 5, 1),  // Blue Wolf Gaiters
            (2392, 5, 1),  // Doom Leather Armor
            (600, 10, 1),  // Sealed Avadon Boots
            (2415, 10, 1), // Sealed Avadon Circlet
            (2464, 10, 1), // Sealed Avadon Gloves
            (2439, 10, 1), // Sealed Blue Wolf Boots
            (2487, 10, 1), // Sealed Blue Wolf Gloves
            (2416, 10, 1), // Sealed Blue Wolf Helmet
            (601, 10, 1),  // Sealed Doom Boots
            (2475, 10, 1), // Sealed Doom Gloves
            (2417, 10, 1), // Sealed Doom Helmet
            (1538, 50, 1), // Improved Scroll of Escape
            (3936, 50, 1), // Blessed Scroll of Resurrection
            (1460, 50, 1), // B-grade Crystal
            (1459, 50, 1), // C-grade Crystal
            (5592, 50, 1), // Greater CP Potion
            (1539, 50, 1), // Top-grade HP Replenishing Potion
            (1540, 70, 1), // High-grade HP Recovery Potion
            (952, 50, 1),  // Scroll: Enchant C-grade Armor
            (951, 40, 1),  // Scroll: Enchant C-grade Weapon
        ],
        _ => return None,
    };
    Some(list)
}

// Drop configuration.
const MAX_DROP_COUNT: i32 = 1;

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Watermelon Ninja",
    active: "01 06 2022-30 08 2022",
    drop_period: None,
    drops: &[DropSpec {
        item: 6391,
        min: 1,
        max: 1,
        chance: 10.0,
        min_level: 60,
        max_level: i32::MAX,
        monster_ids: &[],
    }],
    spawns: &[
        SpawnSpec::at(31860, -84008, 243032, -3728, 49744, 0),
        SpawnSpec::at(31860, 45688, 49080, -3056, 49744, 0),
        SpawnSpec::at(31860, -44584, -113608, -192, 49744, 0),
        SpawnSpec::at(31860, 10643, 17034, -4592, 49744, 0),
        SpawnSpec::at(31860, 115656, -178056, -912, 49744, 0),
        SpawnSpec::at(31860, -80485, 149537, -3016, 16383, 0),
        SpawnSpec::at(31860, -14143, 123176, -3096, 30212, 0),
        SpawnSpec::at(31860, 16354, 142848, -2680, 14450, 0),
        SpawnSpec::at(31860, 82312, 148616, -3440, 3355, 0),
        SpawnSpec::at(31860, 82008, 53874, -1472, 3355, 0),
        SpawnSpec::at(31860, 147064, 25928, -2008, 17863, 0),
        SpawnSpec::at(31860, 117380, 76684, -2672, 40961, 0),
    ],
    on_enter_msg: "L2 Big Watermelon: Event ongoing!",
    on_end_msg: "L2 Big Watermelon: Event end!",
    destroy_items_on_end: &[6391, 15366, 15367],
};

/// Java `getRandomEntry(texts)` of a text array.
fn random_entry<'a>(ctx: &mut ScriptCtx<'_>, texts: &[&'a str]) -> &'a str {
    ctx.get_random_entry(texts).unwrap_or("")
}

/// The `...Text(npc)` methods: `npc.broadcastPacket(new CreatureSay(npc, NPC_GENERAL, npc.getName(),
/// getRandomEntry(texts)))`.
fn say(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, texts: &[&str]) {
    let text = random_entry(ctx, texts);
    broadcast_say_text(ctx, npc, ChatType::NpcGeneral, text);
}

/// The same behind a `getRandom(100) < 80` roll.
fn say_sometimes(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, texts: &[&str]) {
    if ctx.get_random(100) < 80 {
        say(ctx, npc, texts);
    }
}

/// Java `Math.max(currentHp - 30, floor)` set as the NPC's HP.
fn hit(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, floor: f64) {
    let new_hp = (ctx.npc_current_hp(npc) - 30.0).max(floor);
    ctx.world_mut().set_current_hp(npc.object_id, new_hp, true);
}

/// Java `randomSpawn(low, medium, high, npc, attacker)`.
fn random_spawn(
    ctx: &mut ScriptCtx<'_>,
    low: i32,
    medium: i32,
    high: i32,
    npc: ScriptNpc,
    attacker: ObjectId,
) {
    let npc_id = npc.id;
    if npc_id == 13274 || npc_id == 13278 {
        // Fully grown.
        return;
    }

    let random = ctx.get_random(100);
    if random < 5 {
        spawn_next(ctx, low, npc);
        ctx.play_sound_named(attacker, "ItemSound3.sys_sow_success");
    } else if random < 10 {
        spawn_next(ctx, medium, npc);
        ctx.play_sound_named(attacker, "ItemSound3.sys_sow_success");
    } else if random < 30 {
        spawn_next(ctx, high, npc);
        ctx.play_sound_named(attacker, "ItemSound3.sys_sow_success");
    }
}

/// Java `spawnNext(npcId, npc)`.
fn spawn_next(ctx: &mut ScriptCtx<'_>, npc_id: i32, npc: ScriptNpc) {
    // The place is read before the old NPC is deleted: a deleted NPC has none.
    let at = ctx.world().position_of(npc.object_id);
    let heading = ctx.heading_of(npc.object_id);
    if let Some((x, y, z)) = at {
        let despawn = if npc_id == 13274 || npc_id == 13278 {
            30_000
        } else {
            180_000
        };
        ctx.add_spawn(npc_id, x, y, z, heading, false, despawn, 0);
    }
    ctx.delete_npc(npc);
}

/// Java `events.WatermelonNinja.WatermelonNinja`.
pub struct WatermelonNinja;

impl Script for WatermelonNinja {
    fn name(&self) -> &'static str {
        "WatermelonNinja"
    }

    fn path(&self) -> &'static str {
        "events/WatermelonNinja"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&INITIAL_WATERMELONS);
        s.add_attack_id(&WATERMELONS_LIST);
        s.add_attack_id(&HONEY_WATERMELONS_LIST);
        s.add_kill_id(&WATERMELONS_LIST);
        s.add_kill_id(&HONEY_WATERMELONS_LIST);
        s.add_spawn_id(&INITIAL_WATERMELONS);
        s.add_spawn_id(&WATERMELONS_LIST);
        s.add_spawn_id(&HONEY_WATERMELONS_LIST);
        s.add_skill_see_id(&INITIAL_WATERMELONS);

        s.add_start_npc(&[MANAGER]);
        s.add_first_talk_id(&[MANAGER]);
        s.add_talk_id(&[MANAGER]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        long_time_event::load(ctx, &CONFIG);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if long_time_event::on_event(ctx, &CONFIG, event, npc) {
            return None;
        }
        let mut htmltext = None;

        if event == "31860-1.htm" {
            htmltext = Some(event.to_owned());
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.htm", npc.id))
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if INITIAL_WATERMELONS.contains(&npc.id) {
            say_sometimes(ctx, npc, &INITIAL_WATERMELONS_TEXT);
            return;
        }

        let weapon = ctx.active_weapon_item_id(attacker);
        if weapon.is_some_and(|w| CHRONO_LIST.contains(&w)) && HONEY_WATERMELONS_LIST.contains(&npc.id) {
            say_sometimes(ctx, npc, &CHRONO_TEXT);
            ctx.world_mut().set_invul(npc.object_id, false);

            // Rolls 10-30 per hit, but never less than 1 HP.
            hit(ctx, npc, 1.0);
        } else if weapon.is_some() && WATERMELONS_LIST.contains(&npc.id) {
            say_sometimes(ctx, npc, &WATERMELON_TEXT);
            ctx.world_mut().set_invul(npc.object_id, false);

            // Rolls 10-30 per hit, but never less than 1 HP.
            hit(ctx, npc, 1.0);
        } else {
            say_sometimes(ctx, npc, &NOCHRONO_TEXT);
            hit(ctx, npc, 0.0);
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
        say_sometimes(ctx, npc, &NECTAR_TEXT);
        if skill.id == NECTAR_SKILL && ctx.world().target_of(caster) == Some(npc.object_id) {
            match npc.id {
                13271 => {
                    // Watermelon Seed
                    random_spawn(ctx, 13274, 13273, 13272, npc, caster);
                }
                13275 => {
                    // Honey Watermelon Seed
                    random_spawn(ctx, 13278, 13277, 13276, npc, caster);
                }
                _ => {}
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_immobilized(npc, true);
        ctx.disable_core_ai(npc, true);
        ctx.world_mut().set_invul(npc.object_id, true);

        if INITIAL_WATERMELONS.contains(&npc.id) {
            say(ctx, npc, &INITIAL_SPAWN_TEXT);
        } else {
            say(ctx, npc, &EVOLVE_SPAWN_TEXT);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Early exit if no drops for this NPC.
        let Some(drops) = droplist(npc.id).filter(|d| !d.is_empty()) else {
            return;
        };

        let size = drops.len();
        // Random starting index.
        let start_index = usize::try_from(ctx.get_random(i32::try_from(size).unwrap_or(1))).unwrap_or(0);
        let mut drop_count = 0;
        let max_drops = ctx.get_random_range(1, MAX_DROP_COUNT);

        // Iterate through the list starting at random index.
        let mut i = 0;
        while i < size && drop_count < max_drops {
            // Wrap around using modulo.
            let current_index = (start_index + i) % size;
            let (item_id, chance, count) = drops[current_index];
            if ctx.get_random(100) < chance {
                ctx.drop_item(npc, Some(killer), item_id, count);
                drop_count += 1;
            }
            i += 1;
        }

        say(ctx, npc, &DIE_TEXT);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item, names_to};

    const NPCS: &str = r#"
        <npc id="31860" type="Folk" level="70" name="Watermelon Manager"/>
        <npc id="13271" type="Monster" level="1" name="Watermelon Seed"/>
        <npc id="13272" type="Monster" level="1" name="Defective Watermelon"/>
        <npc id="13273" type="Monster" level="1" name="Prime Watermelon"/>
        <npc id="13274" type="Monster" level="1" name="Large Prime Watermelon"/>"#;
    /// 2022-07-01 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_656_676_800_000;

    fn items_xml() -> String {
        let mut ids: Vec<i32> = vec![6391, 15366, 15367];
        ids.extend(droplist(13272).unwrap().iter().map(|d| d.0));
        ids.sort_unstable();
        ids.dedup();
        ids.iter()
            .map(|id| format!(r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_stackable" val="true"/></item>"#))
            .collect()
    }

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(WatermelonNinja);
        let pages = Pages::of(&WatermelonNinja)
            .page("31860.htm", &["31860-1.htm"])
            .plain(&["31860-1.htm"])
            .npcs(&[MANAGER]);
        ScriptTest::with_pages(registry, NPCS, &items_xml(), pages)
    }

    #[test]
    fn inside_the_dates_the_nectar_drops_and_the_managers_stand() {
        let t = setup(Some(INSIDE));
        let drops = t.world.event_drops_of("WatermelonNinja").unwrap();
        assert_eq!(drops.len(), 1);
        assert_eq!(
            (
                drops[0].item_id,
                drops[0].chance,
                drops[0].min_level,
                drops[0].max_level
            ),
            (6391, 10.0, 60, i32::MAX)
        );
        assert_eq!(t.npcs_of(MANAGER).len(), 12);
    }

    #[test]
    fn outside_the_dates_nothing_starts_and_the_event_items_are_destroyed() {
        // The event ended in 2022: `destroyItemsOnEnd` ran at the start, with nobody online.
        let t = setup(None);
        assert!(t.world.event_drops_of("WatermelonNinja").is_none());
        assert!(t.npcs_of(MANAGER).is_empty());
    }

    #[test]
    fn the_end_of_the_event_takes_the_nectar_from_the_players() {
        let mut t = setup(Some(INSIDE));
        t.enter(
            1,
            70,
            vec![item(1000, 6391, 5), item(1001, 15366, 1), item(1002, 57, 1)],
        );
        t.world.run_as_script("WatermelonNinja", |ctx| {
            long_time_event::on_event(ctx, &CONFIG, "LongTimeEvent_ScheduleEnd", None);
        });
        assert_eq!(t.count(1, 6391), 0);
        assert_eq!(t.count(1, 15366), 0);
        assert!(t.world.event_drops_of("WatermelonNinja").is_none());
        // Items of the list the server has no template for stay (Java skips unknown items).
        assert_eq!(t.count(1, 57), 1);
    }

    /// `addSpawn` of an NPC next to player 1: only a script spawn fires `onSpawn`.
    fn script_spawn(t: &mut ScriptTest, npc_id: i32) -> ObjectId {
        let mut spawned = None;
        t.world.run_as_script("WatermelonNinja", |ctx| {
            let (x, y, z) = ctx.world().position_of(ObjectId(1)).unwrap();
            spawned = ctx.add_spawn(npc_id, x + 20, y, z, 0, false, 0, 0);
        });
        spawned.unwrap().object_id
    }

    #[test]
    fn a_new_seed_is_frozen_and_talks() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 70, vec![]);
        t.take();
        let seed = script_spawn(&mut t, 13271);
        assert!(t.world.is_invul(seed));
        assert!(names_to(&t.take(), 1).contains(&"SAY2".to_owned()));
    }

    #[test]
    fn nectar_makes_the_seed_grow_into_a_watermelon() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 70, vec![]);
        let mut seed = t.spawn(13271, 20);
        let mut grown = Vec::new();
        for _ in 0..200 {
            t.world.set_target(ObjectId(1), Some(seed));
            let npc = ScriptNpc {
                object_id: seed,
                id: 13271,
            };
            t.world.run_as_script("WatermelonNinja", |ctx| {
                WatermelonNinja.on_skill_see(
                    ctx,
                    npc,
                    ObjectId(1),
                    SkillKey::new(NECTAR_SKILL, 1),
                    &[seed],
                    false,
                );
            });
            grown = [13272, 13273, 13274]
                .iter()
                .flat_map(|id| t.npcs_of(*id))
                .collect();
            if !grown.is_empty() {
                break;
            }
            seed = t.npcs_of(13271)[0];
        }
        assert_eq!(grown.len(), 1);
        // The seed is gone.
        assert!(t.npcs_of(13271).is_empty());
        // A skill that is not nectar does nothing.
        let melon = grown[0];
        assert!(t.in_world(melon));
    }

    #[test]
    fn the_grown_watermelon_drops_a_prize_when_it_dies() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 70, vec![]);
        let melon = script_spawn(&mut t, 13272);
        t.take();
        t.kill(1, melon);
        let out = names_to(&t.take(), 1);
        assert!(out.contains(&"DROP_ITEM".to_owned()), "a prize fell: {out:?}");
        assert!(
            out.contains(&"SAY2".to_owned()),
            "and the watermelon said goodbye"
        );
    }

    #[test]
    fn a_hit_takes_30_hp_but_a_watermelon_keeps_one() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 70, vec![]);
        let melon = t.spawn(13272, 20);
        let npc = ScriptNpc {
            object_id: melon,
            id: 13272,
        };
        t.world.set_current_hp(melon, 100.0, false);
        let before = t.world.npc(melon).unwrap().core.cur_hp;
        // Hands (no weapon): the hit is the "no chrono" one, which may kill (floor 0).
        t.world.run_as_script("WatermelonNinja", |ctx| {
            WatermelonNinja.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        let after = t.world.npc(melon).unwrap().core.cur_hp;
        assert!(after <= before, "{before} -> {after}");
    }

    #[test]
    fn the_manager_page_is_the_only_event() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 70, vec![]);
        let manager = t.spawn(MANAGER, 20);
        t.talk(1, manager);
        assert!(
            t.event(1, "WatermelonNinja", "31860-1.htm")
                .contains("31860-1.htm")
        );
    }
}
