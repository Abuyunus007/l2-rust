//! The Alt+G window of a GM: what another character has. (`/gmlist` is `send_gm_list` in
//! `flags.rs`.)
// Java: network/clientpackets/{RequestGMCommand,RequestGmList}.java
// Java: network/serverpackets/{GMViewCharacterInfo,GMHennaInfo,GMViewPledgeInfo,GMViewSkillInfo,
//       GmViewQuestInfo,GMViewItemList,GMViewWarehouseWithdrawList}.java
//
// # Left out
// - The clan warehouse is shown as the world holds it. D-50 asks for a fresh read before items
//   leave a clan warehouse; this window only shows them, and the fresh read needs a player of
//   the clan, which a GM is not.

use l2_protocol_game::server::{
    GmHennaInfo, GmSkillEntry, GmViewCharacterInfo, GmViewQuestInfo, GmViewSkillInfo,
    GmViewWarehouseWithdrawList,
};
use l2_util::ObjectId;

use crate::enter::build_user_info;
use crate::inventory::java_concurrent_map_order;
use crate::world::{World, unix_ms_now};

/// `ItemTemplate`/`ItemId.ADENA`.
const ADENA: i32 = 57;

impl World {
    /// Java `RequestGMCommand.runImpl`: `command` 1 status, 2 clan, 3 skills, 4 quests,
    /// 5 inventory, 6 warehouse.
    pub(crate) fn on_request_gm_command(&mut self, id: ObjectId, target_name: &str, command: i32) {
        // Prevent non GM or low level GMs from viewing player stuff.
        let Some(level) = self.players.get(&id).map(|p| p.data.access_level) else {
            return;
        };
        if !self.is_gm(id) || !self.data.access_level_allows_alt_g(level) {
            return;
        }
        let target = self.player_by_name(target_name);
        let clan_id = self.clan_by_name(target_name).map(|c| c.id);
        // The player name was incorrect?
        if target.is_none() && (clan_id.is_none() || command != 6) {
            return;
        }
        match (command, target) {
            // Player status.
            (1, Some(player)) => {
                self.send_gm_view_character_info(id, player);
                self.send_gm_henna_info(id, player);
            }
            // Player clan.
            (2, Some(player)) => {
                let clan = self.clan_of(player).cloned();
                if let Some(clan) = clan {
                    let name = self
                        .players
                        .get(&player)
                        .map(|p| p.data.name.clone())
                        .unwrap_or_default();
                    self.send(id, &crate::admin::clan::gm_view_pledge_info(self, &clan, &name));
                }
            }
            // Player skills.
            (3, Some(player)) => self.send_gm_view_skill_info(id, player),
            // Player quests.
            (4, Some(player)) => {
                let name = self
                    .players
                    .get(&player)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                // `player.getAllActiveQuests()` and `qs.getCond()` (S-API-03).
                let quests = self.gm_view_quests(player);
                self.send(
                    id,
                    &GmViewQuestInfo {
                        player_name: name,
                        quests,
                    },
                );
            }
            // Player inventory.
            (5, Some(player)) => {
                crate::admin::create_item::send_gm_view_item_list(self, id, player);
                self.send_gm_henna_info(id, player);
            }
            // Player warehouse, or the warehouse of the clan.
            (6, Some(player)) => self.send_gm_view_player_warehouse(id, player),
            (6, None) => {
                if let Some(clan_id) = clan_id {
                    self.send_gm_view_clan_warehouse(id, clan_id);
                }
            }
            _ => {}
        }
    }

    /// Java `new GMViewCharacterInfo(player)`: the values of the player's own `UserInfo`.
    fn send_gm_view_character_info(&self, gm: ObjectId, player: ObjectId) {
        let Some(p) = self.players.get(&player) else {
            return;
        };
        let ui = build_user_info(self, p);
        let info = GmViewCharacterInfo {
            x: ui.x,
            y: ui.y,
            z: ui.z,
            heading: p.core.heading,
            object_id: ui.object_id,
            name: p.data.name.clone(),
            race: ui.race,
            is_female: ui.is_female,
            class_id: ui.class_id,
            level: ui.level,
            exp: ui.exp,
            exp_percent: ui.exp_percent,
            str: ui.str,
            dex: ui.dex,
            con: ui.con,
            int: ui.int,
            wit: ui.wit,
            men: ui.men,
            max_hp: ui.max_hp,
            current_hp: ui.current_hp,
            max_mp: ui.max_mp,
            current_mp: ui.current_mp,
            sp: ui.sp,
            current_load: ui.current_load,
            max_load: ui.max_load,
            pk_kills: ui.pk_kills,
            paperdoll_object_ids: ui.paperdoll_object_ids,
            paperdoll_item_ids: ui.paperdoll_item_ids,
            paperdoll_augmentation_ids: ui.paperdoll_augmentation_ids,
            talisman_slots: ui.talisman_slots,
            can_equip_cloak: ui.can_equip_cloak,
            p_atk: ui.p_atk,
            p_atk_spd: ui.p_atk_spd,
            p_def: ui.p_def,
            evasion: ui.evasion,
            accuracy: ui.accuracy,
            critical: ui.critical,
            m_atk: ui.m_atk,
            m_atk_spd: ui.m_atk_spd,
            m_def: ui.m_def,
            pvp_flag: ui.pvp_flag,
            karma: ui.karma,
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
            is_gm: ui.is_gm,
            // `getTitle()`: the real one, not the "[Invisible]" of `UserInfo`.
            title: p.data.title.clone(),
            clan_id: ui.clan_id,
            clan_crest_id: ui.clan_crest_id,
            ally_id: ui.ally_id,
            mount_type: ui.mount_type,
            private_store_type: ui.private_store_type,
            has_dwarven_craft: ui.has_dwarven_craft,
            pvp_kills: ui.pvp_kills,
            recom_left: ui.recom_left,
            recom_have: ui.recom_have,
            max_cp: ui.max_cp,
            current_cp: ui.current_cp,
            running: ui.running,
            pledge_class: ui.pledge_class,
            noble: ui.noble,
            hero: ui.hero,
            name_color: ui.name_color,
            title_color: ui.title_color,
            attack_element: ui.attack_element,
            attack_element_value: ui.attack_element_value,
            defense: [
                ui.defense_fire,
                ui.defense_water,
                ui.defense_wind,
                ui.defense_earth,
                ui.defense_holy,
                ui.defense_dark,
            ],
            fame: ui.fame,
            vitality_points: ui.vitality_points,
        };
        self.send(gm, &info);
    }

    /// Java `new GMHennaInfo(player)`: the layout and numbers of `HennaInfo`.
    fn send_gm_henna_info(&self, gm: ObjectId, player: ObjectId) {
        let henna = self.henna_info(player);
        self.send(
            gm,
            &GmHennaInfo {
                int: henna.int,
                str: henna.str,
                con: henna.con,
                men: henna.men,
                dex: henna.dex,
                wit: henna.wit,
                dye_ids: henna.dye_ids,
            },
        );
    }

    /// Java `new GMViewSkillInfo(player)`.
    fn send_gm_view_skill_info(&self, gm: ObjectId, player: ObjectId) {
        let Some(p) = self.players.get(&player) else {
            return;
        };
        // `clan.getReputationScore() < 0` disables the clan skills.
        let clan_skills_off = self.clan_of(player).is_some_and(|c| c.reputation_score < 0);
        let known = &p.core.skills;
        let keys: Vec<i32> = known.iter().map(|(skill_id, _)| *skill_id).collect();
        let skills = java_concurrent_map_order(&keys)
            .into_iter()
            .filter_map(|skill_id| known.iter().find(|(s, _)| *s == skill_id).copied())
            .map(|(skill_id, level)| GmSkillEntry {
                passive: self.data.skill_is_passive(skill_id),
                display_level: level,
                display_id: skill_id,
                disabled: clan_skills_off && self.rewards.skill_trees.is_clan_skill(skill_id, level),
                // `SkillData.isEnchantable(displayId)`: unlike the skill list, no route is asked.
                enchantable: self
                    .data
                    .stat_data()
                    .is_some_and(|d| d.skills.is_enchantable(skill_id)),
            })
            .collect();
        self.send(
            gm,
            &GmViewSkillInfo {
                player_name: p.data.name.clone(),
                skills,
            },
        );
    }

    /// Java `new GMViewWarehouseWithdrawList(player)`.
    fn send_gm_view_player_warehouse(&self, gm: ObjectId, player: ObjectId) {
        let Some(p) = self.players.get(&player) else {
            return;
        };
        let now_ms = unix_ms_now();
        let items: Vec<_> = p
            .warehouse
            .private
            .iter()
            .map(|item| self.warehouse_item_info(item, now_ms))
            .collect();
        let adena = p
            .warehouse
            .private
            .iter()
            .filter(|i| i.item_id == ADENA)
            .map(|i| i.count)
            .sum();
        self.send(
            gm,
            &GmViewWarehouseWithdrawList {
                owner_name: p.data.name.clone(),
                adena,
                items,
            },
        );
    }

    /// Java `new GMViewWarehouseWithdrawList(clan)`: under the name of the clan leader.
    fn send_gm_view_clan_warehouse(&self, gm: ObjectId, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let now_ms = unix_ms_now();
        let items: Vec<_> = clan
            .warehouse
            .iter()
            .map(|item| self.warehouse_item_info(item, now_ms))
            .collect();
        let adena = clan
            .warehouse
            .iter()
            .filter(|i| i.item_id == ADENA)
            .map(|i| i.count)
            .sum();
        self.send(
            gm,
            &GmViewWarehouseWithdrawList {
                owner_name: self.clan_leader_name(clan_id),
                adena,
                items,
            },
        );
    }
}
