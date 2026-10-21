//! Packets that tell a client which objects it can see and how they look: `CharInfo` (other
//! players), `NpcInfo` (NPCs and monsters), `DeleteObject`, plus `RelationChanged`,
//! `StatusUpdate` and `ExSetCompassZoneCode`.
//!
//! Like [`super::userinfo::UserInfo`], every value Java reads from a game object is a plain
//! field here; the doc comment names the Java getter. Fields are in wire order.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::charscreen::paperdoll;
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Java adds this to npc template ids and mount npc ids so the client reads them as npc type
/// ids (`NpcInfo`, `CharInfo`).
const NPC_TYPE_ID_OFFSET: i32 = 1_000_000;

/// The paperdoll slots `CharInfo` writes, in wire order (Java `CharInfo.PAPERDOLL_ORDER`).
///
/// NOTE(java-quirk): this is not the order `UserInfo` uses (`ServerPacket.PAPERDOLL_ORDER`, 26
/// slots), and `RHAND` is in it twice (positions 2 and 9); Java writes the same item id twice.
pub const CHAR_INFO_PAPERDOLL_ORDER: [usize; 21] = [
    paperdoll::UNDER,
    paperdoll::HEAD,
    paperdoll::RHAND,
    paperdoll::LHAND,
    paperdoll::GLOVES,
    paperdoll::CHEST,
    paperdoll::LEGS,
    paperdoll::FEET,
    paperdoll::CLOAK,
    paperdoll::RHAND,
    paperdoll::HAIR,
    paperdoll::HAIR2,
    paperdoll::RBRACELET,
    paperdoll::LBRACELET,
    paperdoll::DECO1,
    paperdoll::DECO2,
    paperdoll::DECO3,
    paperdoll::DECO4,
    paperdoll::DECO5,
    paperdoll::DECO6,
    paperdoll::BELT,
];

// Java: gameserver/network/serverpackets/CharInfo.java
/// Describes another player to a client (`CHAR_INFO`).
///
/// Java has a second constructor for decoys that overrides the object id and the position;
/// the caller passes those values here. Java's `gmSeeInvis` flag (the receiver is a GM) is
/// already applied by the caller to `title`, `invisible` and `abnormal_visual_effects`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharInfo {
    /// `getX()`, or the in-vehicle position when the player is on a vehicle.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getVehicle().getObjectId()` when on a vehicle with an in-vehicle position, else 0.
    pub vehicle_id: i32,
    /// `getObjectId()` (the decoy's id for a decoy).
    pub object_id: i32,
    /// `getAppearance().getVisibleName()`.
    pub name: String,
    /// `getRace().ordinal()`.
    pub race: i32,
    /// `getAppearance().isFemale()`, written as an int.
    pub is_female: bool,
    /// `getBaseClass()`.
    pub base_class: i32,
    /// `getInventory().getPaperdollItemDisplayId(slot)` per slot, indexed by [`paperdoll`]
    /// constants. Only the slots of [`CHAR_INFO_PAPERDOLL_ORDER`] are on the wire.
    pub paperdoll_item_ids: [i32; paperdoll::TOTAL_SLOTS],
    /// `getInventory().getPaperdollAugmentationId(slot)` per slot.
    pub paperdoll_augmentation_ids: [i32; paperdoll::TOTAL_SLOTS],
    /// `getInventory().getTalismanSlots()`.
    pub talisman_slots: i32,
    /// `getInventory().canEquipCloak()`, written as an int.
    pub can_equip_cloak: bool,
    /// `getPvpFlag()`.
    pub pvp_flag: i32,
    /// `getKarma()`.
    pub karma: i32,
    /// `getMAtkSpd()`.
    pub m_atk_spd: i32,
    /// `(int) getPAtkSpd()`.
    pub p_atk_spd: i32,
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
    /// `getAppearance().getVisibleTitle()`, or `"Invisible"` when the receiver is a GM
    /// (`gmSeeInvis`).
    pub title: String,
    /// `getClanId()`; Java writes 0 for all four clan ints while a cursed weapon is equipped.
    pub clan_id: i32,
    /// `getClanCrestId()`.
    pub clan_crest_id: i32,
    /// `getAllyId()`.
    pub ally_id: i32,
    /// `getAllyCrestId()`.
    pub ally_crest_id: i32,
    /// `isSitting()` (Java writes `!isSitting()`: standing = 1).
    pub sitting: bool,
    /// `isRunning()`.
    pub running: bool,
    /// `isInCombat()`.
    pub in_combat: bool,
    /// `!isInOlympiadMode() && isAlikeDead()`.
    pub alike_dead: bool,
    /// `!gmSeeInvis && isInvisible()`.
    pub invisible: bool,
    /// `getMountType().ordinal()`, one byte.
    pub mount_type: u8,
    /// `getPrivateStoreType().getId()`, one byte.
    pub private_store_type: u8,
    /// `getCubics().keySet()`: cubic ids, each a short after a short count.
    pub cubics: Vec<i16>,
    /// `isInPartyMatchRoom()`, one byte.
    pub in_party_match_room: bool,
    /// `getAbnormalVisualEffects()`, OR-ed with `AbnormalVisualEffect.STEALTH.getMask()` when
    /// the receiver is a GM (`gmSeeInvis`).
    pub abnormal_visual_effects: i32,
    /// One byte: 1 inside a water zone, else 2 when flying mounted, else 0.
    pub water_or_flying: u8,
    /// `getRecomHave()`, a short.
    pub recom_have: i16,
    /// `getMountNpcId()`. NOTE(java-quirk): Java adds 1000000 unconditionally, so a player
    /// without a mount sends 1000000 (`UserInfo` sends 0 in that case). `encode` adds it.
    pub mount_npc_id: i32,
    /// `getPlayerClass().getId()`.
    pub class_id: i32,
    /// `isMounted() ? 0 : getEnchantEffect()`, one byte.
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
    /// `getHeading()`.
    pub heading: i32,
    /// `getPledgeClass()`.
    pub pledge_class: i32,
    /// `getPledgeType()`.
    pub pledge_type: i32,
    /// `getAppearance().getTitleColor()`.
    pub title_color: i32,
    /// `isCursedWeaponEquipped() ? CursedWeaponsManager.getLevel(id) : 0`.
    pub cursed_weapon_level: i32,
    /// `getClan() != null ? getClan().getReputationScore() : 0`.
    pub clan_reputation_score: i32,
    /// `getTransformationDisplayId()`.
    pub transformation_display_id: i32,
    /// `getAgathionId()`.
    pub agathion_id: i32,
    /// `getAbnormalVisualEffectSpecial()`.
    pub abnormal_visual_effect_special: i32,
}

impl CharInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::CharInfo;
}

impl Encode for CharInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.x).i32(self.y).i32(self.z);
        w.i32(self.vehicle_id).i32(self.object_id);
        w.string(&self.name);
        w.i32(self.race)
            .i32(i32::from(self.is_female))
            .i32(self.base_class);
        for slot in CHAR_INFO_PAPERDOLL_ORDER {
            w.i32(self.paperdoll_item_ids[slot]);
        }
        for slot in CHAR_INFO_PAPERDOLL_ORDER {
            w.i32(self.paperdoll_augmentation_ids[slot]);
        }
        w.i32(self.talisman_slots).i32(i32::from(self.can_equip_cloak));
        w.i32(self.pvp_flag).i32(self.karma);
        w.i32(self.m_atk_spd).i32(self.p_atk_spd);
        w.i32(0); // "?" in Java
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
        w.string(&self.title);
        w.i32(self.clan_id)
            .i32(self.clan_crest_id)
            .i32(self.ally_id)
            .i32(self.ally_crest_id);
        w.bool(!self.sitting)
            .bool(self.running)
            .bool(self.in_combat)
            .bool(self.alike_dead)
            .bool(self.invisible);
        w.u8(self.mount_type).u8(self.private_store_type);
        // Java: `writeShort(cubics.size())`, the low 16 bits of the count.
        w.u16(self.cubics.len() as u16);
        for &cubic in &self.cubics {
            w.i16(cubic);
        }
        w.bool(self.in_party_match_room);
        w.i32(self.abnormal_visual_effects);
        w.u8(self.water_or_flying);
        w.i16(self.recom_have);
        w.i32(self.mount_npc_id.wrapping_add(NPC_TYPE_ID_OFFSET));
        w.i32(self.class_id);
        w.i32(0); // "?" in Java
        w.u8(self.enchant_effect).u8(self.team);
        w.i32(self.clan_crest_large_id);
        w.bool(self.noble).bool(self.hero);
        w.bool(self.fishing);
        w.i32(self.fish_x).i32(self.fish_y).i32(self.fish_z);
        w.i32(self.name_color);
        w.i32(self.heading);
        w.i32(self.pledge_class)
            .i32(self.pledge_type)
            .i32(self.title_color)
            .i32(self.cursed_weapon_level);
        w.i32(self.clan_reputation_score);
        w.i32(self.transformation_display_id).i32(self.agathion_id);
        w.i32(1); // "T2" in Java, always 1
        w.i32(self.abnormal_visual_effect_special);
    }
}

impl Decode for CharInfo {
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
            ..Self::default()
        };
        // RHAND is on the wire twice with the same value; the second read overwrites the first.
        for slot in CHAR_INFO_PAPERDOLL_ORDER {
            p.paperdoll_item_ids[slot] = r.i32()?;
        }
        for slot in CHAR_INFO_PAPERDOLL_ORDER {
            p.paperdoll_augmentation_ids[slot] = r.i32()?;
        }
        p.talisman_slots = r.i32()?;
        p.can_equip_cloak = r.i32()? != 0;
        p.pvp_flag = r.i32()?;
        p.karma = r.i32()?;
        p.m_atk_spd = r.i32()?;
        p.p_atk_spd = r.i32()?;
        r.skip(4)?; // always 0
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
        p.title = r.string();
        p.clan_id = r.i32()?;
        p.clan_crest_id = r.i32()?;
        p.ally_id = r.i32()?;
        p.ally_crest_id = r.i32()?;
        p.sitting = !r.bool()?;
        p.running = r.bool()?;
        p.in_combat = r.bool()?;
        p.alike_dead = r.bool()?;
        p.invisible = r.bool()?;
        p.mount_type = r.u8()?;
        p.private_store_type = r.u8()?;
        let cubic_count = r.u16()?;
        for _ in 0..cubic_count {
            p.cubics.push(r.i16()?);
        }
        p.in_party_match_room = r.bool()?;
        p.abnormal_visual_effects = r.i32()?;
        p.water_or_flying = r.u8()?;
        p.recom_have = r.i16()?;
        p.mount_npc_id = r.i32()?.wrapping_sub(NPC_TYPE_ID_OFFSET);
        p.class_id = r.i32()?;
        r.skip(4)?; // always 0
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
        p.heading = r.i32()?;
        p.pledge_class = r.i32()?;
        p.pledge_type = r.i32()?;
        p.title_color = r.i32()?;
        p.cursed_weapon_level = r.i32()?;
        p.clan_reputation_score = r.i32()?;
        p.transformation_display_id = r.i32()?;
        p.agathion_id = r.i32()?;
        r.skip(4)?; // always 1
        p.abnormal_visual_effect_special = r.i32()?;
        Ok(p)
    }
}

// Java: gameserver/network/serverpackets/AbstractNpcInfo.java (inner class `NpcInfo`)
/// Describes an NPC or monster to a client (`NPC_INFO`).
///
/// Java's `NpcInfo.writeImpl` sends nothing at all when `isDecayed()`: the caller must not build
/// this packet for a decayed npc. Name and title are the final strings: Java picks them in
/// `writeImpl` from the multilanguage tables (`MULTILANG_ENABLE`), the template's
/// `isUsingServerSideName()` / `isUsingServerSideTitle()` flags, the invisible-GM title
/// (`"Invisible"`), and the optional level/aggression and champion titles. An empty name makes
/// the client use its own `npcname` table.
///
/// The summon variant is [`super::SummonInfo`]; the trap variant (`TrapInfo`) is not ported.
/// Both end one int earlier.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NpcInfo {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getTemplate().getDisplayId()`; `encode` adds 1000000 (the client's npc type id).
    pub display_id: i32,
    /// `isAutoAttackable(attacker)`, written as an int.
    pub attackable: bool,
    /// `getX()`, `getY()`, `getZ()`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getHeading()`.
    pub heading: i32,
    /// `getMAtkSpd()`.
    pub m_atk_spd: i32,
    /// `(int) getPAtkSpd()`.
    pub p_atk_spd: i32,
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
    /// `getTemplate().getFCollisionRadius()`: a float, widened to double; written twice.
    pub collision_radius: f32,
    /// `getTemplate().getFCollisionHeight()`: a float, widened to double; written twice.
    pub collision_height: f32,
    /// `getRightHandItem()`.
    pub right_hand: i32,
    /// `getLeftHandItem()`.
    pub left_hand: i32,
    /// `isRunning()`, one byte.
    pub running: bool,
    /// `isInCombat()`, one byte.
    pub in_combat: bool,
    /// `isAlikeDead()`, one byte.
    pub alike_dead: bool,
    /// `isShowSummonAnimation()`: Java writes 2 for true, else 0.
    pub show_summon_animation: bool,
    /// The name Java writes (see the type comment).
    pub name: String,
    /// The title Java writes (see the type comment).
    pub title: String,
    /// `isInvisible() ? getAbnormalVisualEffects() | STEALTH.getMask() : getAbnormalVisualEffects()`.
    pub abnormal_visual_effects: i32,
    /// Clan and ally of the castle owner, only set for town npcs that show the castle crest
    /// (`_clanId`, `_clanCrest`, `_allyId`, `_allyCrest`); else 0.
    pub clan_id: i32,
    pub clan_crest_id: i32,
    pub ally_id: i32,
    pub ally_crest_id: i32,
    /// One byte: 1 inside a water zone, else 2 when flying, else 0.
    pub water_or_flying: u8,
    /// `getTeam().getId()`, one byte.
    pub team: u8,
    /// `getEnchantEffect()`.
    pub enchant_effect: i32,
    /// `isFlying()`, written as an int.
    pub flying: bool,
    /// `getColorEffect()`.
    pub color_effect: i32,
    /// `isTargetable()`, one byte.
    pub targetable: bool,
    /// `isShowName()`, one byte.
    pub show_name: bool,
    /// `getAbnormalVisualEffectSpecial()`.
    pub abnormal_visual_effect_special: i32,
    /// `getDisplayEffect()`.
    pub display_effect: i32,
}

impl NpcInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::NpcInfo;
}

impl Encode for NpcInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
        w.i32(self.display_id.wrapping_add(NPC_TYPE_ID_OFFSET));
        w.i32(i32::from(self.attackable));
        w.i32(self.x).i32(self.y).i32(self.z).i32(self.heading);
        w.i32(0);
        w.i32(self.m_atk_spd).i32(self.p_atk_spd);
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
        // NOTE(java-quirk): the chest slot is the `_chest` field of AbstractNpcInfo, which
        // `NpcInfo` never sets (only `SummonInfo` does), so it is always 0 here.
        w.i32(self.right_hand).i32(0).i32(self.left_hand);
        w.u8(1); // "name above char"
        w.bool(self.running).bool(self.in_combat).bool(self.alike_dead);
        w.u8(if self.show_summon_animation { 2 } else { 0 });
        w.i32(-1); // High Five NPCString id
        w.string(&self.name);
        w.i32(-1); // High Five NPCString id
        w.string(&self.title);
        w.i32(0); // title color, 0 = client default
        w.i32(0); // pvp flag
        w.i32(0); // karma
        w.i32(self.abnormal_visual_effects);
        w.i32(self.clan_id)
            .i32(self.clan_crest_id)
            .i32(self.ally_id)
            .i32(self.ally_crest_id);
        w.u8(self.water_or_flying).u8(self.team);
        w.f64(f64::from(self.collision_radius));
        w.f64(f64::from(self.collision_height));
        w.i32(self.enchant_effect);
        w.i32(i32::from(self.flying));
        w.i32(0);
        w.i32(self.color_effect);
        w.bool(self.targetable).bool(self.show_name);
        w.i32(self.abnormal_visual_effect_special);
        w.i32(self.display_effect);
    }
}

impl Decode for NpcInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            object_id: r.i32()?,
            display_id: r.i32()?.wrapping_sub(NPC_TYPE_ID_OFFSET),
            attackable: r.i32()? != 0,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
            ..Self::default()
        };
        r.skip(4)?; // always 0
        p.m_atk_spd = r.i32()?;
        p.p_atk_spd = r.i32()?;
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
        p.right_hand = r.i32()?;
        r.skip(4)?; // chest, always 0
        p.left_hand = r.i32()?;
        r.skip(1)?; // "name above char", always 1
        p.running = r.bool()?;
        p.in_combat = r.bool()?;
        p.alike_dead = r.bool()?;
        p.show_summon_animation = r.u8()? == 2;
        r.skip(4)?; // NPCString id, always -1
        p.name = r.string();
        r.skip(4)?; // NPCString id, always -1
        p.title = r.string();
        r.skip(3 * 4)?; // title color, pvp flag, karma: always 0
        p.abnormal_visual_effects = r.i32()?;
        p.clan_id = r.i32()?;
        p.clan_crest_id = r.i32()?;
        p.ally_id = r.i32()?;
        p.ally_crest_id = r.i32()?;
        p.water_or_flying = r.u8()?;
        p.team = r.u8()?;
        r.skip(2 * 8)?; // collision radius and height again
        p.enchant_effect = r.i32()?;
        p.flying = r.i32()? != 0;
        r.skip(4)?;
        p.color_effect = r.i32()?;
        p.targetable = r.bool()?;
        p.show_name = r.bool()?;
        p.abnormal_visual_effect_special = r.i32()?;
        p.display_effect = r.i32()?;
        Ok(p)
    }
}

// Java: gameserver/network/serverpackets/DeleteObject.java
/// Removes an object from the client's world (`DELETE_OBJECT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeleteObject {
    /// `getObjectId()`.
    pub object_id: i32,
}

impl DeleteObject {
    pub const OPCODE: ServerOpcode = ServerOpcode::DeleteObject;
}

impl Encode for DeleteObject {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
        // C2: for a mounted object 0 would dismount it, 1 makes it disappear.
        w.i32(1);
    }
}

impl Decode for DeleteObject {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        r.skip(4)?; // always 1
        Ok(Self { object_id })
    }
}

/// `RelationChanged` relation bits (Java `RelationChanged.RELATION_*`).
pub mod relation {
    /// Party member.
    pub const PARTY1: i32 = 0x00001;
    pub const PARTY2: i32 = 0x00002;
    pub const PARTY3: i32 = 0x00004;
    pub const PARTY4: i32 = 0x00008;
    /// True if the party leader.
    pub const PARTYLEADER: i32 = 0x00010;
    /// True if in a party.
    pub const HAS_PARTY: i32 = 0x00020;
    /// True if in a clan.
    pub const CLAN_MEMBER: i32 = 0x00040;
    /// True if the clan leader.
    pub const LEADER: i32 = 0x00080;
    /// True if in the same clan.
    pub const CLAN_MATE: i32 = 0x00100;
    /// True if in a siege.
    pub const INSIEGE: i32 = 0x00200;
    /// True when attacker.
    pub const ATTACKER: i32 = 0x00400;
    /// Blue siege icon, cannot have if red.
    pub const ALLY: i32 = 0x00800;
    /// True when red icon, does not matter with blue.
    pub const ENEMY: i32 = 0x01000;
    /// Double fist.
    pub const MUTUAL_WAR: i32 = 0x04000;
    /// Single fist.
    pub const ONE_SIDED_WAR: i32 = 0x08000;
    /// Clan is in an alliance.
    pub const ALLY_MEMBER: i32 = 0x10000;
    /// Show the territory war icon.
    pub const TERRITORY_WAR: i32 = 0x80000;
}

// Java: gameserver/network/serverpackets/RelationChanged.java (inner class `Relation`)
/// One player's relation to the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RelationEntry {
    /// `getObjectId()`.
    pub object_id: i32,
    /// A mask of [`relation`] bits.
    pub relation: i32,
    /// The `autoattackable` argument, written as an int.
    pub auto_attackable: bool,
    /// `getKarma()`.
    pub karma: i32,
    /// `getPvpFlag()`.
    pub pvp_flag: i32,
}

// Java: gameserver/network/serverpackets/RelationChanged.java
/// Relations of one or more players to the receiver (`RELATION_CHANGED`).
///
/// Java has two constructors, one entry (`new RelationChanged(player, relation, attackable)`)
/// and a list (`addRelation`); both write a count and then the entries, so one struct covers
/// both. NOTE(java-quirk): `addRelation` silently skips invisible players; the single-entry
/// constructor does not. The caller applies that rule.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationChanged {
    pub relations: Vec<RelationEntry>,
}

impl RelationChanged {
    pub const OPCODE: ServerOpcode = ServerOpcode::RelationChanged;

    /// The single-player form.
    pub fn single(entry: RelationEntry) -> Self {
        Self {
            relations: vec![entry],
        }
    }
}

impl Encode for RelationChanged {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.relations.len() as i32);
        for entry in &self.relations {
            w.i32(entry.object_id)
                .i32(entry.relation)
                .i32(i32::from(entry.auto_attackable))
                .i32(entry.karma)
                .i32(entry.pvp_flag);
        }
    }
}

impl Decode for RelationChanged {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        let count = usize::try_from(count)
            .map_err(|_| PacketError::invalid("relation count", format!("negative: {count}")))?;
        let mut relations = Vec::new();
        for _ in 0..count {
            relations.push(RelationEntry {
                object_id: r.i32()?,
                relation: r.i32()?,
                auto_attackable: r.i32()? != 0,
                karma: r.i32()?,
                pvp_flag: r.i32()?,
            });
        }
        Ok(Self { relations })
    }
}

// Java: gameserver/network/serverpackets/NicknameChanged.java
/// A creature's title changed (`NICK_NAME_CHANGED`): `Player.broadcastTitleInfo` sends it to
/// everybody who sees the player (M6-PVP-01, the PvP title colour system).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NicknameChanged {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getTitle()`.
    pub title: String,
}

impl NicknameChanged {
    pub const OPCODE: ServerOpcode = ServerOpcode::NickNameChanged;
}

impl Encode for NicknameChanged {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).string(&self.title);
    }
}

impl Decode for NicknameChanged {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            title: r.string(),
        })
    }
}

/// `StatusUpdate` attribute ids (Java `StatusUpdate.*`).
pub mod status {
    pub const LEVEL: i32 = 0x01;
    pub const EXP: i32 = 0x02;
    pub const STR: i32 = 0x03;
    pub const DEX: i32 = 0x04;
    pub const CON: i32 = 0x05;
    pub const INT: i32 = 0x06;
    pub const WIT: i32 = 0x07;
    pub const MEN: i32 = 0x08;
    pub const CUR_HP: i32 = 0x09;
    pub const MAX_HP: i32 = 0x0a;
    pub const CUR_MP: i32 = 0x0b;
    pub const MAX_MP: i32 = 0x0c;
    pub const SP: i32 = 0x0d;
    pub const CUR_LOAD: i32 = 0x0e;
    pub const MAX_LOAD: i32 = 0x0f;
    pub const P_ATK: i32 = 0x11;
    pub const ATK_SPD: i32 = 0x12;
    pub const P_DEF: i32 = 0x13;
    pub const EVASION: i32 = 0x14;
    pub const ACCURACY: i32 = 0x15;
    pub const CRITICAL: i32 = 0x16;
    pub const M_ATK: i32 = 0x17;
    pub const CAST_SPD: i32 = 0x18;
    pub const M_DEF: i32 = 0x19;
    pub const PVP_FLAG: i32 = 0x1a;
    pub const KARMA: i32 = 0x1b;
    pub const CUR_CP: i32 = 0x21;
    pub const MAX_CP: i32 = 0x22;
}

// Java: gameserver/network/serverpackets/StatusUpdate.java
/// Changed stats of one object (`STATUS_UPDATE`): a list of `(attribute id, value)` pairs.
///
/// Java collects them with `addAttribute(id, value)`, in call order, and may send the packet
/// with an empty list (`hasAttributes()` is the caller's check).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusUpdate {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `(id, value)` pairs; the ids are the [`status`] constants.
    pub attributes: Vec<(i32, i32)>,
}

impl StatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::StatusUpdate;

    pub fn new(object_id: i32) -> Self {
        Self {
            object_id,
            attributes: Vec::new(),
        }
    }

    /// Java `addAttribute(id, value)`.
    pub fn add_attribute(&mut self, id: i32, value: i32) {
        self.attributes.push((id, value));
    }

    /// Java `hasAttributes()`.
    pub fn has_attributes(&self) -> bool {
        !self.attributes.is_empty()
    }
}

impl Encode for StatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.attributes.len() as i32);
        for &(id, value) in &self.attributes {
            w.i32(id).i32(value);
        }
    }
}

impl Decode for StatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let count = r.i32()?;
        let count = usize::try_from(count)
            .map_err(|_| PacketError::invalid("attribute count", format!("negative: {count}")))?;
        let mut attributes = Vec::new();
        for _ in 0..count {
            attributes.push((r.i32()?, r.i32()?));
        }
        Ok(Self {
            object_id,
            attributes,
        })
    }
}

/// `ExSetCompassZoneCode` zone types (Java `ExSetCompassZoneCode.*`).
pub mod compass_zone {
    pub const ALTEREDZONE: i32 = 0x08;
    pub const SIEGEWARZONE1: i32 = 0x0A;
    pub const SIEGEWARZONE2: i32 = 0x0B;
    pub const PEACEZONE: i32 = 0x0C;
    pub const SEVENSIGNSZONE: i32 = 0x0D;
    pub const PVPZONE: i32 = 0x0E;
    pub const GENERALZONE: i32 = 0x0F;
}

// Java: gameserver/network/serverpackets/ExSetCompassZoneCode.java
/// Sets the zone icon next to the minimap compass (`EX_SET_COMPASS_ZONE_CODE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExSetCompassZoneCode {
    /// One of the [`compass_zone`] constants.
    pub zone_type: i32,
}

impl ExSetCompassZoneCode {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExSetCompassZoneCode;
}

impl Encode for ExSetCompassZoneCode {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.zone_type);
    }
}

impl Decode for ExSetCompassZoneCode {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { zone_type: r.i32()? })
    }
}
