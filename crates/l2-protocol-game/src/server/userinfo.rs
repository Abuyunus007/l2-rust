//! `UserInfo`: the full description of the player's own character, sent on entering the world
//! and whenever something on the character window changes.
//!
//! Java computes most of these values in the packet's constructor and `writeImpl` from the
//! `Player`. Here every value is a plain field the caller prepares; the doc comment on each
//! field names the Java getter and any arithmetic Java does on it. Fields are in wire order.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::charscreen::paperdoll;
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// The `weapon` int: Java writes 40 when `getActiveWeaponItem() != null`, else 20.
const WEAPON_EQUIPPED: i32 = 40;
const NO_WEAPON: i32 = 20;

// Java: gameserver/network/serverpackets/UserInfo.java
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserInfo {
    /// `getX()`, `getY()`, `getZ()`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getVehicle().getObjectId()`, 0 when not on a vehicle (boat, airship).
    pub vehicle_id: i32,
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getAppearance().getVisibleName()`.
    pub name: String,
    /// `getRace().ordinal()`.
    pub race: i32,
    /// `getAppearance().isFemale()`, written as an int.
    pub is_female: bool,
    /// `getBaseClass()`.
    pub base_class: i32,
    /// `getLevel()`.
    pub level: i32,
    /// `getExp()`.
    pub exp: i64,
    /// Java: `(float) (exp - expForLevel(level)) / (expForLevel(level + 1) - expForLevel(level))`
    /// widened to double; compute it with [`super::exp_percent`].
    pub exp_percent: f64,
    /// `getSTR()`.
    pub str: i32,
    /// `getDEX()`.
    pub dex: i32,
    /// `getCON()`.
    pub con: i32,
    /// `getINT()`.
    pub int: i32,
    /// `getWIT()`.
    pub wit: i32,
    /// `getMEN()`.
    pub men: i32,
    /// `getMaxHp()`.
    pub max_hp: i32,
    /// `(int) Math.round(getCurrentHp())`.
    pub current_hp: i32,
    /// `getMaxMp()`.
    pub max_mp: i32,
    /// `(int) Math.round(getCurrentMp())`.
    pub current_mp: i32,
    /// `(int) getSp()`: the long truncated to an int.
    pub sp: i32,
    /// `getCurrentLoad()`.
    pub current_load: i32,
    /// `getMaxLoad()`.
    pub max_load: i32,
    /// `getActiveWeaponItem() != null` (written as 40, else 20).
    pub weapon_equipped: bool,
    /// `getInventory().getPaperdollObjectId(slot)` per slot, indexed by [`paperdoll`] constants.
    pub paperdoll_object_ids: [i32; paperdoll::TOTAL_SLOTS],
    /// `getInventory().getPaperdollItemDisplayId(slot)` per slot.
    pub paperdoll_item_ids: [i32; paperdoll::TOTAL_SLOTS],
    /// `getInventory().getPaperdollAugmentationId(slot)` per slot.
    pub paperdoll_augmentation_ids: [i32; paperdoll::TOTAL_SLOTS],
    /// `getInventory().getTalismanSlots()`.
    pub talisman_slots: i32,
    /// `getInventory().canEquipCloak()`, written as an int.
    pub can_equip_cloak: bool,
    /// `(int) getPAtk(null)`.
    pub p_atk: i32,
    /// `(int) getPAtkSpd()`; Java writes it twice, see `encode`.
    pub p_atk_spd: i32,
    /// `(int) getPDef(null)`.
    pub p_def: i32,
    /// `getEvasionRate(null)`.
    pub evasion: i32,
    /// `getAccuracy()`.
    pub accuracy: i32,
    /// `getCriticalHit(null, null)`.
    pub critical: i32,
    /// `(int) getMAtk(null, null)`.
    pub m_atk: i32,
    /// `getMAtkSpd()`.
    pub m_atk_spd: i32,
    /// `(int) getMDef(null, null)`.
    pub m_def: i32,
    /// `getPvpFlag()`.
    pub pvp_flag: i32,
    /// `getKarma()`.
    pub karma: i32,
    /// `(int) Math.round(getRunSpeed() / moveMultiplier)`.
    pub run_speed: i32,
    /// `(int) Math.round(getWalkSpeed() / moveMultiplier)`.
    pub walk_speed: i32,
    /// `(int) Math.round(getSwimRunSpeed() / moveMultiplier)`.
    pub swim_run_speed: i32,
    /// `(int) Math.round(getSwimWalkSpeed() / moveMultiplier)`.
    pub swim_walk_speed: i32,
    /// `isFlying() ? runSpeed : 0`; written twice.
    pub fly_run_speed: i32,
    /// `isFlying() ? walkSpeed : 0`; written twice.
    pub fly_walk_speed: i32,
    /// `getMovementSpeedMultiplier()`.
    pub move_multiplier: f64,
    /// `getAttackSpeedMultiplier()`: a Java `float`, widened to double on the wire.
    pub attack_speed_multiplier: f32,
    /// `getCollisionRadius()`: a Java `float`, widened to double on the wire.
    pub collision_radius: f32,
    /// `getCollisionHeight()`: a Java `float`, widened to double on the wire.
    pub collision_height: f32,
    /// `getAppearance().getHairStyle()`.
    pub hair_style: i32,
    /// `getAppearance().getHairColor()`.
    pub hair_color: i32,
    /// `getAppearance().getFace()`.
    pub face: i32,
    /// `isGM()`, written as an int ("builder level").
    pub is_gm: bool,
    /// `getTitle()`, or `"[Invisible]"` for an invisible GM.
    pub title: String,
    /// `getClanId()`.
    pub clan_id: i32,
    /// `getClanCrestId()`.
    pub clan_crest_id: i32,
    /// `getAllyId()`.
    pub ally_id: i32,
    /// `getAllyCrestId()`.
    pub ally_crest_id: i32,
    /// The relation mask built in the constructor: 0x40 clan leader, siege flags 0x180 (attacker
    /// without territory), 0x1000 (attacker with territory), 0x80 (defender).
    pub relation: i32,
    /// `getMountType().ordinal()`, one byte.
    pub mount_type: u8,
    /// `getPrivateStoreType().getId()`, one byte.
    pub private_store_type: u8,
    /// `hasDwarvenCraft()`, one byte.
    pub has_dwarven_craft: bool,
    /// `getPkKills()`.
    pub pk_kills: i32,
    /// `getPvpKills()`.
    pub pvp_kills: i32,
    /// `getCubics().keySet()`: cubic ids, each written as a short after a short count.
    pub cubics: Vec<i16>,
    /// `isInPartyMatchRoom()`, one byte.
    pub in_party_match_room: bool,
    /// `getAbnormalVisualEffects()`, OR-ed with `AbnormalVisualEffect.STEALTH.getMask()` when
    /// the player is invisible.
    pub abnormal_visual_effects: i32,
    /// One byte: 1 inside a water zone, else 2 when flying mounted, else 0.
    pub water_or_flying: u8,
    /// `getClanPrivileges().getMask()`.
    pub clan_privileges: i32,
    /// `getRecomLeft()`, a short.
    pub recom_left: i16,
    /// `getRecomHave()`, a short.
    pub recom_have: i16,
    /// `getMountNpcId() > 0 ? getMountNpcId() + 1000000 : 0`.
    pub mount_npc_id: i32,
    /// `getInventoryLimit()`, a short.
    pub inventory_limit: i16,
    /// `getPlayerClass().getId()`.
    pub class_id: i32,
    /// `getMaxCp()`.
    pub max_cp: i32,
    /// `(int) getCurrentCp()`.
    pub current_cp: i32,
    /// `isMounted() || airShipHelm != 0 ? 0 : getEnchantEffect()`, one byte.
    pub enchant_effect: u8,
    /// `getTeam().getId()`, one byte.
    pub team: u8,
    /// `getClanCrestLargeId()`.
    pub clan_crest_large_id: i32,
    /// `isNoble()`, one byte.
    pub noble: bool,
    /// `isHero() || (isGM() && GeneralConfig.GM_HERO_AURA)`, one byte.
    pub hero: bool,
    /// `isFishing()`, one byte.
    pub fishing: bool,
    /// `getFishX()`, `getFishY()`, `getFishZ()`.
    pub fish_x: i32,
    pub fish_y: i32,
    pub fish_z: i32,
    /// `getAppearance().getNameColor()`.
    pub name_color: i32,
    /// `isRunning()`, one byte.
    pub running: bool,
    /// `getPledgeClass()`.
    pub pledge_class: i32,
    /// `getPledgeType()`.
    pub pledge_type: i32,
    /// `getAppearance().getTitleColor()`.
    pub title_color: i32,
    /// `isCursedWeaponEquipped() ? CursedWeaponsManager.getLevel(id) : 0`.
    pub cursed_weapon_level: i32,
    /// `getTransformationDisplayId()`.
    pub transformation_display_id: i32,
    /// `getAttackElement()`, a byte widened to a short (-2 means none).
    pub attack_element: i16,
    /// `getAttackElementValue(attackElement)`, a short.
    pub attack_element_value: i16,
    /// `getDefenseElementValue(Elementals.FIRE/WATER/WIND/EARTH/HOLY/DARK)`, shorts.
    pub defense_fire: i16,
    pub defense_water: i16,
    pub defense_wind: i16,
    pub defense_earth: i16,
    pub defense_holy: i16,
    pub defense_dark: i16,
    /// `getAgathionId()`.
    pub agathion_id: i32,
    /// `getFame()`.
    pub fame: i32,
    /// `isMinimapAllowed()`, written as an int.
    pub minimap_allowed: bool,
    /// `getVitalityPoints()`.
    pub vitality_points: i32,
    /// `getAbnormalVisualEffectSpecial()`.
    pub abnormal_visual_effect_special: i32,
}

impl UserInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::UserInfo;
}

impl Encode for UserInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z);
        w.i32(self.vehicle_id).i32(self.object_id);
        w.string(&self.name);
        w.i32(self.race)
            .i32(i32::from(self.is_female))
            .i32(self.base_class)
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
        w.i32(self.sp).i32(self.current_load).i32(self.max_load);
        w.i32(if self.weapon_equipped {
            WEAPON_EQUIPPED
        } else {
            NO_WEAPON
        });
        // Three passes over the paperdoll order: object ids, display ids, augmentations.
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
        // NOTE(java-quirk): the physical attack speed is written a second time here, after the
        // magic attack speed (Java calls `getPAtkSpd()` twice).
        w.i32(self.p_atk_spd).i32(self.m_def);
        w.i32(self.pvp_flag).i32(self.karma);
        w.i32(self.run_speed)
            .i32(self.walk_speed)
            .i32(self.swim_run_speed)
            .i32(self.swim_walk_speed);
        // Java writes each fly speed twice in a row of four.
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
        w.i32(self.clan_id)
            .i32(self.clan_crest_id)
            .i32(self.ally_id)
            .i32(self.ally_crest_id);
        w.i32(self.relation);
        w.u8(self.mount_type)
            .u8(self.private_store_type)
            .bool(self.has_dwarven_craft);
        w.i32(self.pk_kills).i32(self.pvp_kills);
        // Java: `writeShort(cubics.size())`, the low 16 bits of the count.
        w.u16(self.cubics.len() as u16);
        for &cubic in &self.cubics {
            w.i16(cubic);
        }
        w.bool(self.in_party_match_room);
        w.i32(self.abnormal_visual_effects);
        w.u8(self.water_or_flying);
        w.i32(self.clan_privileges);
        w.i16(self.recom_left).i16(self.recom_have);
        w.i32(self.mount_npc_id);
        w.i16(self.inventory_limit);
        w.i32(self.class_id);
        w.i32(0); // "special effects? circles around player"
        w.i32(self.max_cp).i32(self.current_cp);
        w.u8(self.enchant_effect).u8(self.team);
        w.i32(self.clan_crest_large_id);
        w.bool(self.noble).bool(self.hero);
        w.bool(self.fishing);
        w.i32(self.fish_x).i32(self.fish_y).i32(self.fish_z);
        w.i32(self.name_color);
        w.bool(self.running);
        w.i32(self.pledge_class)
            .i32(self.pledge_type)
            .i32(self.title_color)
            .i32(self.cursed_weapon_level);
        w.i32(self.transformation_display_id);
        w.i16(self.attack_element).i16(self.attack_element_value);
        w.i16(self.defense_fire)
            .i16(self.defense_water)
            .i16(self.defense_wind)
            .i16(self.defense_earth)
            .i16(self.defense_holy)
            .i16(self.defense_dark);
        w.i32(self.agathion_id);
        w.i32(self.fame)
            .i32(i32::from(self.minimap_allowed))
            .i32(self.vitality_points)
            .i32(self.abnormal_visual_effect_special);
    }
}

impl Decode for UserInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            vehicle_id: r.i32()?,
            object_id: r.i32()?,
            name: r.string(),
            race: r.i32()?,
            is_female: r.i32()? != 0,
            base_class: r.i32()?,
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
            ..Self::default()
        };
        p.weapon_equipped = match r.i32()? {
            WEAPON_EQUIPPED => true,
            NO_WEAPON => false,
            other => {
                return Err(PacketError::invalid(
                    "weapon flag",
                    format!("expected 20 or 40, got {other}"),
                ));
            }
        };
        for ids in [
            &mut p.paperdoll_object_ids,
            &mut p.paperdoll_item_ids,
            &mut p.paperdoll_augmentation_ids,
        ] {
            for slot in paperdoll::WIRE_ORDER {
                ids[slot] = r.i32()?;
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
        r.skip(4)?; // p_atk_spd again
        p.m_def = r.i32()?;
        p.pvp_flag = r.i32()?;
        p.karma = r.i32()?;
        p.run_speed = r.i32()?;
        p.walk_speed = r.i32()?;
        p.swim_run_speed = r.i32()?;
        p.swim_walk_speed = r.i32()?;
        p.fly_run_speed = r.i32()?;
        p.fly_walk_speed = r.i32()?;
        r.skip(2 * 4)?; // fly speeds again
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
        p.ally_crest_id = r.i32()?;
        p.relation = r.i32()?;
        p.mount_type = r.u8()?;
        p.private_store_type = r.u8()?;
        p.has_dwarven_craft = r.bool()?;
        p.pk_kills = r.i32()?;
        p.pvp_kills = r.i32()?;
        let cubic_count = r.u16()?;
        for _ in 0..cubic_count {
            p.cubics.push(r.i16()?);
        }
        p.in_party_match_room = r.bool()?;
        p.abnormal_visual_effects = r.i32()?;
        p.water_or_flying = r.u8()?;
        p.clan_privileges = r.i32()?;
        p.recom_left = r.i16()?;
        p.recom_have = r.i16()?;
        p.mount_npc_id = r.i32()?;
        p.inventory_limit = r.i16()?;
        p.class_id = r.i32()?;
        r.skip(4)?; // special effects, always 0
        p.max_cp = r.i32()?;
        p.current_cp = r.i32()?;
        p.enchant_effect = r.u8()?;
        p.team = r.u8()?;
        p.clan_crest_large_id = r.i32()?;
        p.noble = r.bool()?;
        p.hero = r.bool()?;
        p.fishing = r.bool()?;
        p.fish_x = r.i32()?;
        p.fish_y = r.i32()?;
        p.fish_z = r.i32()?;
        p.name_color = r.i32()?;
        p.running = r.bool()?;
        p.pledge_class = r.i32()?;
        p.pledge_type = r.i32()?;
        p.title_color = r.i32()?;
        p.cursed_weapon_level = r.i32()?;
        p.transformation_display_id = r.i32()?;
        p.attack_element = r.i16()?;
        p.attack_element_value = r.i16()?;
        p.defense_fire = r.i16()?;
        p.defense_water = r.i16()?;
        p.defense_wind = r.i16()?;
        p.defense_earth = r.i16()?;
        p.defense_holy = r.i16()?;
        p.defense_dark = r.i16()?;
        p.agathion_id = r.i32()?;
        p.fame = r.i32()?;
        p.minimap_allowed = r.i32()? != 0;
        p.vitality_points = r.i32()?;
        p.abnormal_visual_effect_special = r.i32()?;
        Ok(p)
    }
}
