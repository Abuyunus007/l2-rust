//! Client packets of skill enchanting: the info windows and the four ways to change an enchanted
//! skill (enchant, safe enchant, untrain, change route).
//!
//! These only read the packet. The checks and the costs are the world's job
//! (`l2_world::enchant::skill`).
//!
//! The ex ids come from the generated table. The `Format` comments of the Java classes name older
//! ids (`0x06`, `0x31`...): the ids registered in `ExClientPackets` are the ones that count.

// Java: gameserver/network/clientpackets/RequestExEnchantSkillInfo.java
client_packet!(
    /// The player selected an enchantable skill in the skill window (`0xD0` 0x000E).
    RequestExEnchantSkillInfo,
    ex RequestExEnchantSkillInfo {
        skill_id: i32,
        skill_level: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExEnchantSkillInfoDetail.java
client_packet!(
    /// The player picked an enchant route or an action in the enchant window (`0xD0` 0x0046).
    RequestExEnchantSkillInfoDetail,
    ex RequestExEnchantSkillInfoDetail {
        /// 0 enchant, 1 safe enchant, 2 untrain, 3 change route (Java `_type`).
        kind: i32,
        skill_id: i32,
        skill_level: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExEnchantSkill.java
client_packet!(
    /// "Enchant" (`0xD0` 0x000F). `skill_level` is the level to reach (`101`, `102`, ...).
    RequestExEnchantSkill,
    ex RequestExEnchantSkill {
        skill_id: i32,
        skill_level: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExEnchantSkillSafe.java
client_packet!(
    /// "Safe enchant": a failure keeps the level (`0xD0` 0x0032).
    RequestExEnchantSkillSafe,
    ex RequestExEnchantSkillSafe {
        skill_id: i32,
        skill_level: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExEnchantSkillUntrain.java
client_packet!(
    /// "Untrain": one enchant level back (`0xD0` 0x0033). `skill_level` is the level to return to.
    RequestExEnchantSkillUntrain,
    ex RequestExEnchantSkillUntrain {
        skill_id: i32,
        skill_level: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExEnchantSkillRouteChange.java
client_packet!(
    /// "Change route" (`0xD0` 0x0034). `skill_level` is the same step on the new route.
    RequestExEnchantSkillRouteChange,
    ex RequestExEnchantSkillRouteChange {
        skill_id: i32,
        skill_level: i32,
    }
);
