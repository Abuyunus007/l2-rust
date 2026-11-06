//! The roles a companion can be hired for.

use super::brain::{Behavior, DamageDealer, Healer, Tank};

/// What a companion does in the party.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Tank,
    Healer,
    /// A melee damage dealer.
    Warrior,
    /// A ranged damage dealer.
    Archer,
}

/// The NPC templates (`data/stats/npcs/50000-50099.xml`), one per role.
pub const NPC_TANK: i32 = 50021;
pub const NPC_HEALER: i32 = 50022;
pub const NPC_WARRIOR: i32 = 50023;
pub const NPC_ARCHER: i32 = 50024;

static TANK: Tank = Tank;
static HEALER: Healer = Healer;
static WARRIOR: DamageDealer = DamageDealer { ranged: false };
static ARCHER: DamageDealer = DamageDealer { ranged: true };

impl Role {
    pub const ALL: [Role; 4] = [Role::Tank, Role::Healer, Role::Warrior, Role::Archer];

    /// Used in the page links.
    pub fn key(self) -> &'static str {
        match self {
            Role::Tank => "tank",
            Role::Healer => "healer",
            Role::Warrior => "warrior",
            Role::Archer => "archer",
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
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Role::Tank => "Holds the monsters: takes them from you and your friends and keeps them busy.",
            Role::Healer => "Heals whoever is hurt, you first if it is close, and stays behind the fighters.",
            Role::Warrior => "Fights next to you and hits what you hit.",
            Role::Archer => "Shoots from a distance at what you hit, and steps back to you when pressed.",
        }
    }

    /// The NPC template of the role.
    pub fn npc_id(self) -> i32 {
        match self {
            Role::Tank => NPC_TANK,
            Role::Healer => NPC_HEALER,
            Role::Warrior => NPC_WARRIOR,
            Role::Archer => NPC_ARCHER,
        }
    }

    /// The group of the class buffer that a companion of this role buffs itself with.
    pub fn buff_group_key(self) -> &'static str {
        match self {
            Role::Tank => "tank",
            Role::Healer => "healer",
            Role::Warrior => "warrior",
            Role::Archer => "archer",
        }
    }

    /// What it does in combat.
    pub fn behavior(self) -> &'static dyn Behavior {
        match self {
            Role::Tank => &TANK,
            Role::Healer => &HEALER,
            Role::Warrior => &WARRIOR,
            Role::Archer => &ARCHER,
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
