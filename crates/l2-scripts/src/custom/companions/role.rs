//! The roles a companion can be hired for.

use super::brain::{Behavior, DamageDealer, Healer, Mage, Tank};

/// What a companion does in the party.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Tank,
    Healer,
    /// A melee damage dealer with a sword.
    Warrior,
    /// A ranged damage dealer.
    Archer,
    /// A melee damage dealer with a dagger: fewer hit points, more critical hits.
    Dagger,
    /// A damage dealer that casts spells.
    Mage,
    /// A caster that fights with a summoned creature.
    Summoner,
    /// Buffs its owner and his party.
    Buffer,
}

/// The NPC templates (`data/stats/npcs/50000-50099.xml`), one per role.
pub const NPC_TANK: i32 = 50021;
pub const NPC_HEALER: i32 = 50022;
pub const NPC_WARRIOR: i32 = 50023;
pub const NPC_ARCHER: i32 = 50024;
pub const NPC_DAGGER: i32 = 50025;
pub const NPC_MAGE: i32 = 50026;
pub const NPC_SUMMONER: i32 = 50027;
pub const NPC_BUFFER: i32 = 50028;

static TANK: Tank = Tank;
static HEALER: Healer = Healer;
static WARRIOR: DamageDealer = DamageDealer { ranged: false };
static ARCHER: DamageDealer = DamageDealer { ranged: true };
static MAGE: Mage = Mage;

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Tank,
        Role::Healer,
        Role::Warrior,
        Role::Archer,
        Role::Dagger,
        Role::Mage,
        Role::Summoner,
        Role::Buffer,
    ];

    /// Used in the page links.
    pub fn key(self) -> &'static str {
        match self {
            Role::Tank => "tank",
            Role::Healer => "healer",
            Role::Warrior => "warrior",
            Role::Archer => "archer",
            Role::Dagger => "dagger",
            Role::Mage => "mage",
            Role::Summoner => "summoner",
            Role::Buffer => "buffer",
        }
    }

    pub fn from_key(key: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            Role::Tank => "Tank",
            Role::Healer => "Healer",
            Role::Warrior => "Warrior",
            Role::Archer => "Archer",
            Role::Dagger => "Dagger",
            Role::Mage => "Mage",
            Role::Summoner => "Summoner",
            Role::Buffer => "Buffer",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Role::Tank => "Takes the monsters from you and keeps them busy.",
            Role::Healer => "Heals whoever falls below 60% HP.",
            Role::Warrior => "Fights next to you and hits what you hit.",
            Role::Archer => "Shoots at what you hit from a distance.",
            Role::Dagger => "Stabs what you hit, with many critical hits.",
            Role::Mage => "Casts spells at what you hit from a distance.",
            Role::Summoner => "Fights from a distance with a summoned creature.",
            Role::Buffer => "Keeps you and your party buffed.",
        }
    }

    /// The NPC template of the role.
    pub fn npc_id(self) -> i32 {
        match self {
            Role::Tank => NPC_TANK,
            Role::Healer => NPC_HEALER,
            Role::Warrior => NPC_WARRIOR,
            Role::Archer => NPC_ARCHER,
            Role::Dagger => NPC_DAGGER,
            Role::Mage => NPC_MAGE,
            Role::Summoner => NPC_SUMMONER,
            Role::Buffer => NPC_BUFFER,
        }
    }

    /// The group of the class buffer that a companion of this role buffs itself with.
    pub fn buff_group_key(self) -> &'static str {
        match self {
            Role::Summoner => "mage",
            Role::Buffer => "healer",
            other => other.key(),
        }
    }

    /// What it does in combat.
    pub fn behavior(self) -> &'static dyn Behavior {
        match self {
            Role::Tank => &TANK,
            Role::Healer => &HEALER,
            Role::Warrior | Role::Dagger => &WARRIOR,
            Role::Archer => &ARCHER,
            // The summoner's creature and the buffer's buffs come with their classes' skills;
            // until then they fight like a mage and keep the party healed.
            Role::Mage | Role::Summoner => &MAGE,
            Role::Buffer => &HEALER,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_is_found_by_its_key_and_every_role_has_a_template_and_a_buff_group() {
        for role in Role::ALL {
            assert_eq!(Role::from_key(role.key()), Some(role));
            assert!(crate::custom::class_buffer_data::group_by_key(role.buff_group_key()).is_some());
        }
        assert_eq!(Role::from_key("bard"), None);
        let mut ids: Vec<i32> = Role::ALL.iter().map(|r| r.npc_id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Role::ALL.len(), "one template each");
    }
}
