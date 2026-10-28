//! The `Player` / `Npc` calls the quest ports make beyond `Quest`'s helpers (S-PILOT-01): race
//! and class of a player, the NPC's script value and busy flag, the class change of the sagas,
//! the state Java scripts share through a `static` field of their base class.
// Java: entity/actor/Player.java (getRace, getPlayerClass, getBaseClass, isMageClass,
//       setPlayerClass, setBaseClass, broadcastUserInfo)
// Java: entity/actor/Npc.java (get/set/isScriptValue, setBusy, isBusy, dropItem)
// Java: entity/actor/enums/creature/Race.java, entity/actor/enums/player/PlayerClass.java
//
// The race and class values are Java's ints (`Race.ordinal()`, `PlayerClass.getId()`): a port
// compares `ctx.player_race(p) == race::DARK_ELF` where Java has `player.getRace() == Race.DARK_ELF`
// and `ctx.player_class_id(p) == player_class::ORC_MAGE` for `getPlayerClass() == ORC_MAGE`.

use std::any::{Any, TypeId};

use l2_data::ClassId;
use l2_protocol_game::server::{MagicSkillUse, SkillPosition};
use l2_util::ObjectId;
use tracing::warn;

use super::api::ScriptNpc;
use super::ctx::ScriptCtx;

/// Java `Race` as `ordinal()`.
pub mod race {
    pub const HUMAN: i32 = 0;
    pub const ELF: i32 = 1;
    pub const DARK_ELF: i32 = 2;
    pub const ORC: i32 = 3;
    pub const DWARF: i32 = 4;
    pub const KAMAEL: i32 = 5;
}

/// Java `PlayerClass` as `getId()`, with Java's names.
pub mod player_class {
    pub const FIGHTER: i32 = 0;
    pub const WARRIOR: i32 = 1;
    pub const GLADIATOR: i32 = 2;
    pub const WARLORD: i32 = 3;
    pub const KNIGHT: i32 = 4;
    pub const PALADIN: i32 = 5;
    pub const DARK_AVENGER: i32 = 6;
    pub const ROGUE: i32 = 7;
    pub const TREASURE_HUNTER: i32 = 8;
    pub const HAWKEYE: i32 = 9;
    pub const MAGE: i32 = 10;
    pub const WIZARD: i32 = 11;
    pub const SORCERER: i32 = 12;
    pub const NECROMANCER: i32 = 13;
    pub const WARLOCK: i32 = 14;
    pub const CLERIC: i32 = 15;
    pub const BISHOP: i32 = 16;
    pub const PROPHET: i32 = 17;
    pub const ELVEN_FIGHTER: i32 = 18;
    pub const ELVEN_KNIGHT: i32 = 19;
    pub const TEMPLE_KNIGHT: i32 = 20;
    pub const SWORDSINGER: i32 = 21;
    pub const ELVEN_SCOUT: i32 = 22;
    pub const PLAINS_WALKER: i32 = 23;
    pub const SILVER_RANGER: i32 = 24;
    pub const ELVEN_MAGE: i32 = 25;
    pub const ELVEN_WIZARD: i32 = 26;
    pub const SPELLSINGER: i32 = 27;
    pub const ELEMENTAL_SUMMONER: i32 = 28;
    pub const ORACLE: i32 = 29;
    pub const ELDER: i32 = 30;
    pub const DARK_FIGHTER: i32 = 31;
    pub const PALUS_KNIGHT: i32 = 32;
    pub const SHILLIEN_KNIGHT: i32 = 33;
    pub const BLADEDANCER: i32 = 34;
    pub const ASSASSIN: i32 = 35;
    pub const ABYSS_WALKER: i32 = 36;
    pub const PHANTOM_RANGER: i32 = 37;
    pub const DARK_MAGE: i32 = 38;
    pub const DARK_WIZARD: i32 = 39;
    pub const SPELLHOWLER: i32 = 40;
    pub const PHANTOM_SUMMONER: i32 = 41;
    pub const SHILLIEN_ORACLE: i32 = 42;
    pub const SHILLIEN_ELDER: i32 = 43;
    pub const ORC_FIGHTER: i32 = 44;
    pub const ORC_RAIDER: i32 = 45;
    pub const DESTROYER: i32 = 46;
    pub const ORC_MONK: i32 = 47;
    pub const TYRANT: i32 = 48;
    pub const ORC_MAGE: i32 = 49;
    pub const ORC_SHAMAN: i32 = 50;
    pub const OVERLORD: i32 = 51;
    pub const WARCRYER: i32 = 52;
    pub const DWARVEN_FIGHTER: i32 = 53;
    pub const SCAVENGER: i32 = 54;
    pub const BOUNTY_HUNTER: i32 = 55;
    pub const ARTISAN: i32 = 56;
    pub const WARSMITH: i32 = 57;
    pub const DUELIST: i32 = 88;
    pub const DREADNOUGHT: i32 = 89;
    pub const PHOENIX_KNIGHT: i32 = 90;
    pub const HELL_KNIGHT: i32 = 91;
    pub const SAGITTARIUS: i32 = 92;
    pub const ADVENTURER: i32 = 93;
    pub const ARCHMAGE: i32 = 94;
    pub const SOULTAKER: i32 = 95;
    pub const ARCANA_LORD: i32 = 96;
    pub const CARDINAL: i32 = 97;
    pub const HIEROPHANT: i32 = 98;
    pub const EVA_TEMPLAR: i32 = 99;
    pub const SWORD_MUSE: i32 = 100;
    pub const WIND_RIDER: i32 = 101;
    pub const MOONLIGHT_SENTINEL: i32 = 102;
    pub const MYSTIC_MUSE: i32 = 103;
    pub const ELEMENTAL_MASTER: i32 = 104;
    pub const EVA_SAINT: i32 = 105;
    pub const SHILLIEN_TEMPLAR: i32 = 106;
    pub const SPECTRAL_DANCER: i32 = 107;
    pub const GHOST_HUNTER: i32 = 108;
    pub const GHOST_SENTINEL: i32 = 109;
    pub const STORM_SCREAMER: i32 = 110;
    pub const SPECTRAL_MASTER: i32 = 111;
    pub const SHILLIEN_SAINT: i32 = 112;
    pub const TITAN: i32 = 113;
    pub const GRAND_KHAVATARI: i32 = 114;
    pub const DOMINATOR: i32 = 115;
    pub const DOOMCRYER: i32 = 116;
    pub const FORTUNE_SEEKER: i32 = 117;
    pub const MAESTRO: i32 = 118;
    pub const MALE_SOLDIER: i32 = 123;
    pub const FEMALE_SOLDIER: i32 = 124;
    pub const TROOPER: i32 = 125;
    pub const WARDER: i32 = 126;
    pub const BERSERKER: i32 = 127;
    pub const MALE_SOULBREAKER: i32 = 128;
    pub const FEMALE_SOULBREAKER: i32 = 129;
    pub const ARBALESTER: i32 = 130;
    pub const DOOMBRINGER: i32 = 131;
    pub const MALE_SOUL_HOUND: i32 = 132;
    pub const FEMALE_SOUL_HOUND: i32 = 133;
    pub const TRICKSTER: i32 = 134;
    pub const INSPECTOR: i32 = 135;
    pub const JUDICATOR: i32 = 136;
}

/// Java `PlayerClass.getRace()` as `ordinal()`; -1 for an id that is not a player class.
pub fn class_race(class_id: i32) -> i32 {
    match class_id {
        0..=17 | 88..=98 => race::HUMAN,
        18..=30 | 99..=105 => race::ELF,
        31..=43 | 106..=112 => race::DARK_ELF,
        44..=52 | 113..=116 => race::ORC,
        53..=57 | 117..=118 => race::DWARF,
        123..=136 => race::KAMAEL,
        _ => -1,
    }
}

fn class_of(class_id: i32) -> ClassId {
    ClassId(u16::try_from(class_id).unwrap_or(u16::MAX))
}

/// What scripts of one family share: Java's `static` fields of a base class
/// ([`ScriptCtx::shared_state`]), by type.
#[derive(Default)]
pub(crate) struct SharedStates(std::collections::HashMap<TypeId, Box<dyn Any + Send>>);

impl ScriptCtx<'_> {
    // ------------------------------------------------------------------ player

    /// Java `player.getRace().ordinal()` ([`race`]); -1 for no player.
    pub fn player_race(&self, player: ObjectId) -> i32 {
        self.world.player(player).map_or(-1, |p| p.data.race)
    }

    /// Java `player.getPlayerClass().getId()` ([`player_class`]): the active class; -1 for no
    /// player.
    pub fn player_class_id(&self, player: ObjectId) -> i32 {
        self.world.player(player).map_or(-1, |p| p.data.class_id)
    }

    /// Java `player.getBaseClass()`; -1 for no player.
    pub fn player_base_class(&self, player: ObjectId) -> i32 {
        self.world.player(player).map_or(-1, |p| p.data.base_class)
    }

    /// Java `player.getPlayerClass().level()`: 0 for a starting class, 1 after the first class
    /// change, ...
    pub fn player_class_level(&self, player: ObjectId) -> i32 {
        class_of(self.player_class_id(player)).class_level()
    }

    /// Java `player.getPlayerClass().getRace().ordinal()`: the race of the active class (a sub
    /// class can be of another race than the character).
    pub fn player_class_race(&self, player: ObjectId) -> i32 {
        class_race(self.player_class_id(player))
    }

    /// Java `player.isMageClass()`: `getPlayerClass().isMage()`.
    pub fn is_mage_class(&self, player: ObjectId) -> bool {
        class_of(self.player_class_id(player)).is_mage()
    }

    /// Java `creature.isDead()`.
    pub fn is_dead(&self, creature: ObjectId) -> bool {
        self.world.is_dead(creature)
    }

    /// Java `player.isInParty()`.
    pub fn is_in_party(&self, player: ObjectId) -> bool {
        self.world.is_in_party(player)
    }

    /// Java `PlayerConfig.ALT_PARTY_RANGE` (`AltPartyRange`).
    pub fn alt_party_range(&self) -> i32 {
        self.world.settings.party.alt_party_range
    }

    /// Java `LocationUtil.checkIfInRange(range, a, b, includeZAxis)`. The kill quests'
    /// `checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, killer, true)` is
    /// `ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)`.
    pub fn check_if_in_range(&self, range: i32, a: ObjectId, b: ObjectId, include_z: bool) -> bool {
        self.world.check_if_in_range(range, a, b, include_z)
    }

    /// Java `a.isInsideRadius2D(b, radius)`: `false` when either is not in the world.
    pub fn is_inside_radius_2d(&self, a: ObjectId, b: ObjectId, radius: i32) -> bool {
        match (self.world.position_of(a), self.world.position_of(b)) {
            (Some((ax, ay, _)), Some((bx, by, _))) => {
                let dx = f64::from(ax - bx);
                let dy = f64::from(ay - by);
                dx * dx + dy * dy <= f64::from(radius) * f64::from(radius)
            }
            _ => false,
        }
    }

    /// Java `player.setPlayerClass(id)`: the class change (message, skills, party and clan
    /// icons), as `//setclass` does it.
    pub fn set_player_class(&mut self, player: ObjectId, class_id: i32) {
        crate::admin::edit_char::set_player_class(self.world, player, class_id);
    }

    /// Java `player.setBaseClass(id)`.
    pub fn set_base_class(&mut self, player: ObjectId, class_id: i32) {
        if let Some(p) = self.world.players.get_mut(&player) {
            p.data.base_class = class_id;
        }
    }

    /// Java `player.isSubClassActive()`.
    pub fn is_sub_class_active(&self, player: ObjectId) -> bool {
        self.world.is_subclass_active(player)
    }

    /// Java `player.setLastQuestNpcObject(npc.getObjectId())`: the NPC the quest window and
    /// `bypass Script` events are checked against.
    pub fn set_last_quest_npc_object(&mut self, player: ObjectId, npc: ScriptNpc) {
        if let Some(p) = self.world.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.object_id.0;
        }
    }

    /// Java `player.broadcastUserInfo()`.
    pub fn broadcast_user_info(&mut self, player: ObjectId) {
        self.world.broadcast_user_info(player);
    }

    /// Java `from.broadcastSkillPacket(new MagicSkillUse(caster, target, skillId, level,
    /// hitTime, reuseDelay), ...)`: the animation to whoever sees `from` (and `from` itself when
    /// it is a player). `from` is usually the caster.
    #[allow(clippy::too_many_arguments)]
    pub fn broadcast_magic_skill_use(
        &mut self,
        from: ObjectId,
        caster: ObjectId,
        target: ObjectId,
        skill_id: i32,
        skill_level: i32,
        hit_time: i32,
        reuse_delay: i32,
    ) {
        let (Some(c), Some(t)) = (self.world.position_of(caster), self.world.position_of(target)) else {
            return;
        };
        let packet = MagicSkillUse {
            creature_id: caster.0,
            target_id: target.0,
            skill_id,
            skill_level,
            hit_time,
            reuse_delay,
            creature: SkillPosition {
                x: c.0,
                y: c.1,
                z: c.2,
            },
            ground_location: None,
            target: SkillPosition {
                x: t.0,
                y: t.1,
                z: t.2,
            },
        };
        self.world.broadcast_creature_packet(from, &packet);
    }

    // ------------------------------------------------------------------ npc

    /// Java `npc.getScriptValue()`.
    pub fn get_script_value(&self, npc: ScriptNpc) -> i32 {
        self.world
            .scripts
            .npc_script_values
            .get(&npc.object_id)
            .copied()
            .unwrap_or(0)
    }

    /// Java `npc.setScriptValue(value)`. `Npc.onDecay` sets it back to 0.
    pub fn set_script_value(&mut self, npc: ScriptNpc, value: i32) {
        if !self.world.npcs.contains_key(&npc.object_id) {
            return;
        }
        if value == 0 {
            self.world.scripts.npc_script_values.remove(&npc.object_id);
        } else {
            self.world.scripts.npc_script_values.insert(npc.object_id, value);
        }
    }

    /// Java `npc.isScriptValue(value)`.
    pub fn is_script_value(&self, npc: ScriptNpc, value: i32) -> bool {
        self.get_script_value(npc) == value
    }

    /// Java `npc.isBusy()`.
    pub fn is_npc_busy(&self, npc: ScriptNpc) -> bool {
        self.world
            .talk
            .busy
            .get(&npc.object_id)
            .is_some_and(|(busy, _)| *busy)
    }

    /// Java `npc.setBusy(busy)`.
    pub fn set_npc_busy(&mut self, npc: ScriptNpc, busy: bool) {
        self.world.set_npc_busy(npc.object_id, busy);
    }

    /// Java `npc.getAI().setIntentionMoveTo(new Location(x, y, z))`.
    pub fn npc_move_to(&mut self, npc: ScriptNpc, x: i32, y: i32, z: i32) {
        self.world.npc_set_intention_move_to(npc.object_id, x, y, z);
    }

    /// Java `npc.deleteMe()`.
    pub fn delete_npc(&mut self, npc: ScriptNpc) {
        self.world.delete_npc(npc.object_id);
    }

    /// Java `npc.getHeading()` / `player.getHeading()`.
    pub fn heading_of(&self, object: ObjectId) -> i32 {
        self.world.heading_of(object)
    }

    /// Java `npc.dropItem(creature, itemId, count)`: on the ground near the NPC, protected for
    /// `creature`.
    pub fn drop_item(&mut self, npc: ScriptNpc, creature: Option<ObjectId>, item_id: i32, count: i64) {
        self.world.npc_drop_item(npc.object_id, creature, item_id, count);
    }

    /// Java `npc.showChatWindow(player)`: the NPC's own page.
    pub fn show_chat_window(&mut self, npc: ScriptNpc, player: ObjectId) {
        self.world.show_chat_window_default(npc.object_id, player);
    }

    // ------------------------------------------------------------------ shared state

    /// A Java `static` field that several scripts share (the `SPAWN_LIST` of
    /// `AbstractSagaQuest`, used by all 34 sagas): one value per type for every script, made
    /// with `T::default()` the first time. A field of one script is [`ScriptCtx::state`].
    pub fn shared_state<T: Any + Send + Default>(&mut self) -> &mut T {
        let slot = self
            .world
            .scripts
            .shared_states
            .0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()));
        if !slot.is::<T>() {
            warn!("script shared state of the wrong type replaced.");
            *slot = Box::new(T::default());
        }
        match slot.downcast_mut::<T>() {
            Some(value) => value,
            // Unreachable: the slot is keyed by the type.
            None => unreachable!("script shared state of the wrong type"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_races_follow_player_class() {
        assert_eq!(class_race(player_class::FIGHTER), race::HUMAN);
        assert_eq!(class_race(player_class::WARLOCK), race::HUMAN);
        assert_eq!(class_race(player_class::ELVEN_MAGE), race::ELF);
        assert_eq!(class_race(player_class::SPECTRAL_MASTER), race::DARK_ELF);
        assert_eq!(class_race(player_class::DOOMCRYER), race::ORC);
        assert_eq!(class_race(player_class::MAESTRO), race::DWARF);
        assert_eq!(class_race(player_class::JUDICATOR), race::KAMAEL);
        assert_eq!(class_race(119), -1);
    }
}
