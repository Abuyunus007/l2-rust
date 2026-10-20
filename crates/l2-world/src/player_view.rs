//! What other players see of a player: `CharInfo`, `ExBrExtraUserInfo` and `RelationChanged`.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (sendInfo, getKnownRelations; the
//       relation and isAutoAttackable are in `pvp.rs`)
// Java: org/l2jmobius/gameserver/network/serverpackets/CharInfo.java
//
// The values come from the same computation as the player's own `UserInfo`
// (`enter::build_user_info`), so both views agree.
// Compared field by field with Java's CharInfo constructor and writeImpl (M3-PLAYERS-01): the
// speeds, flags and colours come from the same getters as UserInfo; only the GM-invisibility
// handling differs. Not here yet: boats/airships, private store messages, summons' relations.

use l2_protocol_game::server::{CharInfo, ExBrExtraUserInfo, RelationChanged, RelationEntry};
use l2_util::ObjectId;

use crate::enter::build_user_info;

/// Java `AbnormalVisualEffect.STEALTH.getMask()`.
pub(crate) const STEALTH_MASK: i32 = 0x0010_0000;
use crate::world::World;

impl World {
    /// Java `Player.sendInfo(player)` for an ordinary player (not in a boat or airship).
    pub(crate) fn send_player_info(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(info) = self.char_info(subject, viewer) else {
            return;
        };
        self.send(viewer, &info);
        self.send(
            viewer,
            &ExBrExtraUserInfo {
                object_id: subject.0,
                abnormal_visual_effects_event: self.abnormal_visual_effects(subject).2,
                lecture_mark: 1,
            },
        );
        // Java: RelationChanged only when it differs from what this viewer was told before.
        let relation = self.relation(subject, viewer);
        let auto_attackable = self.is_auto_attackable(subject, viewer);
        let Some(player) = self.players.get_mut(&subject) else {
            return;
        };
        let known = player.known_relations.get(&viewer).copied();
        if known != Some((relation, auto_attackable)) {
            player.known_relations.insert(viewer, (relation, auto_attackable));
            let entry = RelationEntry {
                object_id: subject.0,
                relation,
                auto_attackable,
                karma: player.data.karma,
                pvp_flag: self.pvp_flag_of(subject),
            };
            self.send(viewer, &RelationChanged::single(entry));
        }
    }

    /// `new CharInfo(player, gmSeeInvis)` as `viewer` gets it.
    pub fn char_info(&self, subject: ObjectId, viewer: ObjectId) -> Option<CharInfo> {
        let player = self.players.get(&subject)?;
        let ui = build_user_info(self, player);
        let invisible = self.objects.get(&subject).is_some_and(|e| e.invisible);
        let gm_sees_invisible = invisible && self.is_visible_for(subject, viewer);
        // `Creature.getAbnormalVisualEffects()` and the special mask, without the stealth bit
        // `UserInfo` adds for an invisible player.
        let (visual_effects, visual_effects_special, _) = self.abnormal_visual_effects(subject);
        Some(CharInfo {
            x: ui.x,
            y: ui.y,
            z: ui.z,
            vehicle_id: ui.vehicle_id,
            object_id: ui.object_id,
            name: ui.name,
            race: ui.race,
            is_female: ui.is_female,
            base_class: ui.base_class,
            paperdoll_item_ids: ui.paperdoll_item_ids,
            paperdoll_augmentation_ids: ui.paperdoll_augmentation_ids,
            talisman_slots: ui.talisman_slots,
            can_equip_cloak: ui.can_equip_cloak,
            pvp_flag: ui.pvp_flag,
            karma: ui.karma,
            m_atk_spd: ui.m_atk_spd,
            p_atk_spd: ui.p_atk_spd,
            run_speed: ui.run_speed,
            walk_speed: ui.walk_speed,
            swim_run_speed: ui.swim_run_speed,
            swim_walk_speed: ui.swim_walk_speed,
            fly_run_speed: ui.fly_run_speed,
            fly_walk_speed: ui.fly_walk_speed,
            move_multiplier: ui.move_multiplier,
            attack_speed_multiplier: ui.attack_speed_multiplier,
            collision_radius: ui.collision_radius,
            collision_height: ui.collision_height,
            hair_style: ui.hair_style,
            hair_color: ui.hair_color,
            face: ui.face,
            // Java: a GM looking at an invisible player sees the title "Invisible".
            title: if gm_sees_invisible {
                "Invisible".to_string()
            } else {
                ui.title
            },
            clan_id: ui.clan_id,
            clan_crest_id: ui.clan_crest_id,
            ally_id: ui.ally_id,
            ally_crest_id: ui.ally_crest_id,
            sitting: self.is_sitting(subject),
            running: ui.running,
            in_combat: false,
            // Java: `!_player.isInOlympiadMode() && _player.isAlikeDead()`.
            alike_dead: self.is_alike_dead(subject),
            // Java: `!gmSeeInvis && isInvisible()` (a GM sees the player, a ghostly one).
            invisible: !gm_sees_invisible && invisible,
            mount_type: ui.mount_type,
            private_store_type: ui.private_store_type,
            cubics: ui.cubics,
            in_party_match_room: ui.in_party_match_room,
            // Java: a GM viewer gets `AbnormalVisualEffect.STEALTH` on top.
            abnormal_visual_effects: if gm_sees_invisible {
                visual_effects | STEALTH_MASK
            } else {
                visual_effects
            },
            water_or_flying: ui.water_or_flying,
            recom_have: ui.recom_have,
            mount_npc_id: ui.mount_npc_id,
            class_id: ui.class_id,
            enchant_effect: ui.enchant_effect,
            team: ui.team,
            clan_crest_large_id: ui.clan_crest_large_id,
            noble: ui.noble,
            hero: ui.hero,
            fishing: ui.fishing,
            fish_x: ui.fish_x,
            fish_y: ui.fish_y,
            fish_z: ui.fish_z,
            name_color: ui.name_color,
            heading: player.core.heading,
            pledge_class: ui.pledge_class,
            pledge_type: ui.pledge_type,
            title_color: ui.title_color,
            cursed_weapon_level: ui.cursed_weapon_level,
            clan_reputation_score: 0,
            transformation_display_id: ui.transformation_display_id,
            agathion_id: 0,
            abnormal_visual_effect_special: visual_effects_special,
        })
    }
}
