//! The windows of the Alt+G command (`RequestGMCommand`): the status, dyes, skills, quests and
//! warehouse of another character. The inventory window is `GmViewItemList` in
//! [`super::gm_view`], the clan window `GmViewPledgeInfo` in [`super::pledge`].
//!
//! Values only: the world fills them from the character that is looked at.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::charscreen::paperdoll;
use super::items::{ItemInfo, read_item, write_item};
use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/GMViewCharacterInfo.java
/// The status window a GM sees of another character (`GM_VIEW_CHARACTER_INFO`). The values are
/// the ones of that character's `UserInfo`; the fields are in wire order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GmViewCharacterInfo {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub object_id: i32,
    pub name: String,
    /// `getRace().ordinal()`.
    pub race: i32,
    pub is_female: bool,
    /// `getPlayerClass().getId()` (written again further down).
    pub class_id: i32,
    pub level: i32,
    pub exp: i64,
    /// `(float) (exp - expForLevel) / (expForNextLevel - expForLevel)` as a double.
    pub exp_percent: f64,
    pub str: i32,
    pub dex: i32,
    pub con: i32,
    pub int: i32,
    pub wit: i32,
    pub men: i32,
    pub max_hp: i32,
    pub current_hp: i32,
    pub max_mp: i32,
    pub current_mp: i32,
    /// `(int) getSp()`.
    pub sp: i32,
    pub current_load: i32,
    pub max_load: i32,
    pub pk_kills: i32,
    pub paperdoll_object_ids: [i32; paperdoll::TOTAL_SLOTS],
    pub paperdoll_item_ids: [i32; paperdoll::TOTAL_SLOTS],
    pub paperdoll_augmentation_ids: [i32; paperdoll::TOTAL_SLOTS],
    pub talisman_slots: i32,
    pub can_equip_cloak: bool,
    pub p_atk: i32,
    pub p_atk_spd: i32,
    pub p_def: i32,
    pub evasion: i32,
    pub accuracy: i32,
    pub critical: i32,
    pub m_atk: i32,
    pub m_atk_spd: i32,
    pub m_def: i32,
    pub pvp_flag: i32,
    pub karma: i32,
    pub run_speed: i32,
    pub walk_speed: i32,
    pub swim_run_speed: i32,
    pub swim_walk_speed: i32,
    pub fly_run_speed: i32,
    pub fly_walk_speed: i32,
    pub move_multiplier: f64,
    pub attack_speed_multiplier: f32,
    pub collision_radius: f32,
    pub collision_height: f32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub face: i32,
    pub is_gm: bool,
    pub title: String,
    pub clan_id: i32,
    pub clan_crest_id: i32,
    pub ally_id: i32,
    pub mount_type: u8,
    pub private_store_type: u8,
    pub has_dwarven_craft: bool,
    pub pvp_kills: i32,
    pub recom_left: i16,
    pub recom_have: i16,
    pub max_cp: i32,
    pub current_cp: i32,
    pub running: bool,
    pub pledge_class: i32,
    pub noble: bool,
    pub hero: bool,
    pub name_color: i32,
    pub title_color: i32,
    pub attack_element: i16,
    pub attack_element_value: i16,
    /// Fire, water, wind, earth, holy, dark.
    pub defense: [i16; 6],
    pub fame: i32,
    pub vitality_points: i32,
}

impl GmViewCharacterInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewCharacterInfo;
}

impl Encode for GmViewCharacterInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z).i32(self.heading);
        w.i32(self.object_id).string(&self.name);
        w.i32(self.race)
            .i32(i32::from(self.is_female))
            .i32(self.class_id)
            .i32(self.level);
        w.i64(self.exp).f64(self.exp_percent);
        w.i32(self.str)
            .i32(self.dex)
            .i32(self.con)
            .i32(self.int)
            .i32(self.wit)
            .i32(self.men);
        w.i32(self.max_hp)
            .i32(self.current_hp)
            .i32(self.max_mp)
            .i32(self.current_mp);
        w.i32(self.sp)
            .i32(self.current_load)
            .i32(self.max_load)
            .i32(self.pk_kills);
        for ids in [
            &self.paperdoll_object_ids,
            &self.paperdoll_item_ids,
            &self.paperdoll_augmentation_ids,
        ] {
            for slot in paperdoll::WIRE_ORDER {
                w.i32(ids[slot]);
            }
        }
        w.i32(self.talisman_slots).i32(i32::from(self.can_equip_cloak));
        w.i32(self.p_atk).i32(self.p_atk_spd).i32(self.p_def);
        w.i32(self.evasion).i32(self.accuracy).i32(self.critical);
        w.i32(self.m_atk).i32(self.m_atk_spd);
        // NOTE(java-quirk): the physical attack speed is written a second time, like in UserInfo.
        w.i32(self.p_atk_spd).i32(self.m_def);
        w.i32(self.pvp_flag).i32(self.karma);
        w.i32(self.run_speed)
            .i32(self.walk_speed)
            .i32(self.swim_run_speed)
            .i32(self.swim_walk_speed);
        w.i32(self.fly_run_speed)
            .i32(self.fly_walk_speed)
            .i32(self.fly_run_speed)
            .i32(self.fly_walk_speed);
        w.f64(self.move_multiplier);
        w.f64(f64::from(self.attack_speed_multiplier));
        w.f64(f64::from(self.collision_radius));
        w.f64(f64::from(self.collision_height));
        w.i32(self.hair_style).i32(self.hair_color).i32(self.face);
        w.i32(i32::from(self.is_gm));
        w.string(&self.title);
        w.i32(self.clan_id).i32(self.clan_crest_id).i32(self.ally_id);
        w.u8(self.mount_type)
            .u8(self.private_store_type)
            .bool(self.has_dwarven_craft);
        w.i32(self.pk_kills).i32(self.pvp_kills);
        w.i16(self.recom_left).i16(self.recom_have);
        w.i32(self.class_id);
        // "special effects? circles around player..."
        w.i32(0);
        w.i32(self.max_cp).i32(self.current_cp);
        w.bool(self.running);
        // NOTE(java-quirk): `writeByte(321)` writes the low byte, 65.
        w.u8(65);
        w.i32(self.pledge_class);
        w.bool(self.noble).bool(self.hero);
        w.i32(self.name_color).i32(self.title_color);
        w.i16(self.attack_element).i16(self.attack_element_value);
        for value in self.defense {
            w.i16(value);
        }
        w.i32(self.fame).i32(self.vitality_points);
    }
}

impl Decode for GmViewCharacterInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
            object_id: r.i32()?,
            name: r.string(),
            race: r.i32()?,
            is_female: r.i32()? != 0,
            class_id: r.i32()?,
            level: r.i32()?,
            exp: r.i64()?,
            exp_percent: r.f64()?,
            str: r.i32()?,
            dex: r.i32()?,
            con: r.i32()?,
            int: r.i32()?,
            wit: r.i32()?,
            men: r.i32()?,
            max_hp: r.i32()?,
            current_hp: r.i32()?,
            max_mp: r.i32()?,
            current_mp: r.i32()?,
            sp: r.i32()?,
            current_load: r.i32()?,
            max_load: r.i32()?,
            pk_kills: r.i32()?,
            ..Self::default()
        };
        // The slot written twice (RHAND) is read twice into the same place.
        for which in 0..3 {
            for slot in paperdoll::WIRE_ORDER {
                let value = r.i32()?;
                match which {
                    0 => p.paperdoll_object_ids[slot] = value,
                    1 => p.paperdoll_item_ids[slot] = value,
                    _ => p.paperdoll_augmentation_ids[slot] = value,
                }
            }
        }
        p.talisman_slots = r.i32()?;
        p.can_equip_cloak = r.i32()? != 0;
        p.p_atk = r.i32()?;
        p.p_atk_spd = r.i32()?;
        p.p_def = r.i32()?;
        p.evasion = r.i32()?;
        p.accuracy = r.i32()?;
        p.critical = r.i32()?;
        p.m_atk = r.i32()?;
        p.m_atk_spd = r.i32()?;
        r.skip(4)?; // the physical attack speed again
        p.m_def = r.i32()?;
        p.pvp_flag = r.i32()?;
        p.karma = r.i32()?;
        p.run_speed = r.i32()?;
        p.walk_speed = r.i32()?;
        p.swim_run_speed = r.i32()?;
        p.swim_walk_speed = r.i32()?;
        p.fly_run_speed = r.i32()?;
        p.fly_walk_speed = r.i32()?;
        r.skip(8)?; // the fly speeds again
        p.move_multiplier = r.f64()?;
        p.attack_speed_multiplier = r.f64()? as f32;
        p.collision_radius = r.f64()? as f32;
        p.collision_height = r.f64()? as f32;
        p.hair_style = r.i32()?;
        p.hair_color = r.i32()?;
        p.face = r.i32()?;
        p.is_gm = r.i32()? != 0;
        p.title = r.string();
        p.clan_id = r.i32()?;
        p.clan_crest_id = r.i32()?;
        p.ally_id = r.i32()?;
        p.mount_type = r.u8()?;
        p.private_store_type = r.u8()?;
        p.has_dwarven_craft = r.bool()?;
        r.skip(4)?; // the pk kills again
        p.pvp_kills = r.i32()?;
        p.recom_left = r.i16()?;
        p.recom_have = r.i16()?;
        r.skip(8)?; // the class id again, the effects int
        p.max_cp = r.i32()?;
        p.current_cp = r.i32()?;
        p.running = r.bool()?;
        r.skip(1)?; // 65
        p.pledge_class = r.i32()?;
        p.noble = r.bool()?;
        p.hero = r.bool()?;
        p.name_color = r.i32()?;
        p.title_color = r.i32()?;
        p.attack_element = r.i16()?;
        p.attack_element_value = r.i16()?;
        for value in &mut p.defense {
            *value = r.i16()?;
        }
        p.fame = r.i32()?;
        p.vitality_points = r.i32()?;
        Ok(p)
    }
}

// Java: gameserver/network/serverpackets/GMHennaInfo.java
/// The dyes a GM sees of another character (`GM_HENNA_INFO`): the layout of `HennaInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmHennaInfo {
    /// Stat bonus of the dyes, in wire order INT, STR, CON, MEN, DEX, WIT (one byte each).
    pub int: i8,
    pub str: i8,
    pub con: i8,
    pub men: i8,
    pub dex: i8,
    pub wit: i8,
    /// Dye ids of the slots in use.
    pub dye_ids: Vec<i32>,
}

impl GmHennaInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmHennaInfo;
    /// Java writes the constant 3 slots.
    const SLOTS: i32 = 3;
}

impl Encode for GmHennaInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i8(self.int)
            .i8(self.str)
            .i8(self.con)
            .i8(self.men)
            .i8(self.dex)
            .i8(self.wit);
        w.i32(Self::SLOTS).i32(self.dye_ids.len() as i32);
        for dye_id in &self.dye_ids {
            w.i32(*dye_id).i32(1);
        }
    }
}

impl Decode for GmHennaInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (int, str, con, men, dex, wit) = (r.i8()?, r.i8()?, r.i8()?, r.i8()?, r.i8()?, r.i8()?);
        r.skip(4)?;
        let count = read_count(r, "henna count")?;
        let mut dye_ids = Vec::new();
        for _ in 0..count {
            dye_ids.push(r.i32()?);
            r.skip(4)?;
        }
        Ok(Self {
            int,
            str,
            con,
            men,
            dex,
            wit,
            dye_ids,
        })
    }
}

/// One skill of [`GmViewSkillInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GmSkillEntry {
    pub passive: bool,
    pub display_level: i32,
    pub display_id: i32,
    /// `clanReputation < 0 && skill.isClanSkill()`.
    pub disabled: bool,
    /// `SkillData.isEnchantable(displayId)`.
    pub enchantable: bool,
}

// Java: gameserver/network/serverpackets/GMViewSkillInfo.java
/// The skills a GM sees of another character (`GM_VIEW_SKILL_INFO`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmViewSkillInfo {
    pub player_name: String,
    pub skills: Vec<GmSkillEntry>,
}

impl GmViewSkillInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewSkillInfo;
}

impl Encode for GmViewSkillInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.player_name).i32(self.skills.len() as i32);
        for s in &self.skills {
            w.i32(i32::from(s.passive))
                .i32(s.display_level)
                .i32(s.display_id)
                .bool(s.disabled)
                .bool(s.enchantable);
        }
    }
}

impl Decode for GmViewSkillInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_name = r.string();
        let count = read_count(r, "skills")?;
        let mut skills = Vec::new();
        for _ in 0..count {
            skills.push(GmSkillEntry {
                passive: r.i32()? != 0,
                display_level: r.i32()?,
                display_id: r.i32()?,
                disabled: r.bool()?,
                enchantable: r.bool()?,
            });
        }
        Ok(Self { player_name, skills })
    }
}

// Java: gameserver/network/serverpackets/GmViewQuestInfo.java
/// The active quests a GM sees of another character (`GM_VIEW_QUEST_INFO`): `(quest id, cond)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmViewQuestInfo {
    pub player_name: String,
    pub quests: Vec<(i32, i32)>,
}

impl GmViewQuestInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewQuestInfo;
}

impl Encode for GmViewQuestInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.player_name);
        if self.quests.is_empty() {
            // NOTE(java-quirk): the empty list is a byte and two shorts, all 0.
            w.u8(0).u16(0).u16(0);
            return;
        }
        w.u16(self.quests.len() as u16);
        for (quest_id, cond) in &self.quests {
            w.i32(*quest_id).i32(*cond);
        }
    }
}

impl Decode for GmViewQuestInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_name = r.string();
        let count = r.u16()?;
        if count == 0 {
            // The rest of the empty form: the first byte and short made the count.
            r.skip(3)?;
            return Ok(Self {
                player_name,
                quests: Vec::new(),
            });
        }
        let mut quests = Vec::new();
        for _ in 0..count {
            quests.push((r.i32()?, r.i32()?));
        }
        Ok(Self { player_name, quests })
    }
}

// Java: gameserver/network/serverpackets/GMViewWarehouseWithdrawList.java
/// The warehouse a GM sees (`GM_VIEW_WAREHOUSE_WITHDRAW_LIST`): of a player, or of a clan under
/// the name of its leader.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmViewWarehouseWithdrawList {
    pub owner_name: String,
    pub adena: i64,
    pub items: Vec<ItemInfo>,
}

impl GmViewWarehouseWithdrawList {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewWarehouseWithdrawList;
}

impl Encode for GmViewWarehouseWithdrawList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.owner_name).i64(self.adena);
        w.u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
            w.i32(item.object_id);
        }
    }
}

impl Decode for GmViewWarehouseWithdrawList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let owner_name = r.string();
        let adena = r.i64()?;
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
            r.skip(4)?;
        }
        Ok(Self {
            owner_name,
            adena,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P) {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn character_info_round_trips_and_has_the_java_size() {
        let mut info = GmViewCharacterInfo {
            x: 1,
            y: -2,
            z: 3,
            heading: 4,
            object_id: 0x1000_0001,
            name: "Target".into(),
            race: 2,
            is_female: true,
            class_id: 20,
            level: 40,
            exp: 123_456,
            exp_percent: 0.5,
            max_hp: 1000,
            defense: [1, 2, 3, 4, 5, 6],
            attack_speed_multiplier: 1.5,
            collision_radius: 7.0,
            collision_height: 23.5,
            title: "Boss".into(),
            ..Default::default()
        };
        info.paperdoll_object_ids[paperdoll::RHAND] = 77;
        info.paperdoll_item_ids[paperdoll::CHEST] = 88;
        round_trip(&info);
        let bytes = info.to_bytes();
        assert_eq!(bytes[0], 0x95);
        // 4 coordinates/heading + id, "Target\0", race/sex/class/level, exp, percent, 6 stats,
        // hp/mp, sp/load/max load/pk, 3 x 26 paperdoll ints, talisman/cloak, 10 combat ints,
        // pvp/karma, 8 speeds, 4 doubles, 3 looks, gm, "Boss\0", 3 clan ints, 3 bytes,
        // pk/pvp kills, 2 shorts, class, effects, cp x2, running + 65, pledge class, 2 bytes,
        // 2 colours, 8 shorts, fame, vitality.
        let string = |s: &str| (s.len() + 1) * 2;
        let expected = 1
            + 4 * 5
            + string("Target")
            + 4 * 4
            + 8
            + 8
            + 6 * 4
            + 4 * 4
            + 4 * 4
            + 3 * 26 * 4
            + 2 * 4
            + 10 * 4
            + 2 * 4
            + 8 * 4
            + 4 * 8
            + 3 * 4
            + 4
            + string("Boss")
            + 3 * 4
            + 3
            + 2 * 4
            + 2 * 2
            + 4
            + 4
            + 2 * 4
            + 2
            + 4
            + 2
            + 2 * 4
            + 8 * 2
            + 2 * 4;
        assert_eq!(bytes.len(), expected);
    }

    #[test]
    fn the_other_windows_round_trip() {
        round_trip(&GmHennaInfo {
            int: 1,
            str: -1,
            con: 0,
            men: 2,
            dex: 0,
            wit: 3,
            dye_ids: vec![4445, 4446],
        });
        round_trip(&GmViewSkillInfo {
            player_name: "Target".into(),
            skills: vec![GmSkillEntry {
                passive: true,
                display_level: 3,
                display_id: 1216,
                disabled: false,
                enchantable: true,
            }],
        });
        round_trip(&GmViewQuestInfo {
            player_name: "Target".into(),
            quests: vec![(255, 2), (401, 1)],
        });
        round_trip(&GmViewQuestInfo {
            player_name: "Target".into(),
            quests: vec![],
        });
        round_trip(&GmViewWarehouseWithdrawList {
            owner_name: "Target".into(),
            adena: 5_000_000_000,
            items: vec![ItemInfo::plain(268_435_456, 57, 5_000)],
        });
    }

    /// `GmViewQuestInfo.writeImpl`: no quests is a zero byte and two zero shorts (5 bytes).
    #[test]
    fn empty_quest_list_is_the_java_quirk() {
        let bytes = GmViewQuestInfo {
            player_name: "A".into(),
            quests: vec![],
        }
        .to_bytes();
        assert_eq!(bytes[0], 0x99);
        assert_eq!(&bytes[1..5], &[b'A', 0, 0, 0]);
        assert_eq!(&bytes[5..], &[0, 0, 0, 0, 0]);
    }

    /// The warehouse view writes every item followed by its object id.
    #[test]
    fn warehouse_item_is_followed_by_its_object_id() {
        let item = ItemInfo::plain(0x0102_0304, 57, 1);
        let one = GmViewWarehouseWithdrawList {
            owner_name: String::new(),
            adena: 0,
            items: vec![item.clone()],
        }
        .to_bytes();
        let none = GmViewWarehouseWithdrawList::default().to_bytes();
        assert_eq!(one[0], 0x9B);
        // opcode + "\0" + adena + count, then the item (68 bytes) + 4.
        assert_eq!(none.len(), 1 + 2 + 8 + 2);
        assert_eq!(one.len(), none.len() + 68 + 4);
        assert_eq!(&one[one.len() - 4..], &[4, 3, 2, 1]);
    }

    /// `GMHennaInfo` has the layout of `HennaInfo`: six bytes, 3 slots, the dyes with a 1 each.
    #[test]
    fn henna_layout() {
        let bytes = GmHennaInfo {
            int: 1,
            dye_ids: vec![9],
            ..Default::default()
        }
        .to_bytes();
        assert_eq!(bytes[0], 0xF0);
        assert_eq!(&bytes[1..7], &[1, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[7..11], &[3, 0, 0, 0]);
        assert_eq!(&bytes[11..15], &[1, 0, 0, 0]);
        assert_eq!(&bytes[15..23], &[9, 0, 0, 0, 1, 0, 0, 0]);
    }
}
