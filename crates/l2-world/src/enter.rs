//! Entering the world: the packets Java's `EnterWorld` sends to a plain character, in Java's
//! order, and the builders for each of them.
// Java: network/clientpackets/EnterWorld.java (runImpl)
// Java: entity/actor/Player.java (restore, checkRecoBonusTask, sendSkillList, refreshOverloaded)
// Java: network/serverpackets/{UserInfo,ItemList,ExQuestItemList,SendMacroList,ShortcutInit,
// Java: EtcStatusUpdate,ExStorageMaxCount,ExVoteSystemInfo,SkillList}.java
//
// The builders are plain functions of the player's data, so the game server can reuse them
// (`SkillList` is sent when the character is selected, before the world is involved) and tests
// can call them directly.
//
// Skipped on purpose, because the system behind them does not exist yet (the matching Java
// packets are therefore not sent either):
// - visibility (M3): `spawnMe` and every `NpcInfo`/`CharInfo`, `broadcastUserInfo`'s `CharInfo`
// - zones: `ExSetCompassZoneCode` and the zone messages
// - clans, sieges, Seven Signs (the period message), announcements, the server news HTML
// - quests and scripts: `QuestList` is empty, the tutorial packets of `Quest.playerEnter`
// - friends, contacts, teleport bookmarks, the mail check, premium items
// - effects (`AbnormalStatusUpdate`), skill reuse (`SkillCoolTime` is empty)
// - GM startup, birthday gifts, over-enchant protection, HWID checks, offline traders

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExStorageMaxCount, ExUiSetting,
    ExVoteSystemInfo, FriendList, HennaInfo, OtherShortcut, QuestList, ShortcutEntry, ShortcutInit,
    SkillList, SkillListEntry, ValidateLocation,
};
use l2_protocol_game::server::{
    ActionFailed, ExBrExtraUserInfo, ExQuestItemList, ExRotation, ExShowContactList, ItemInfo, ItemList,
    MacroCommandEntry, MacroEntry, MacroList, SystemMessage, UserInfo,
};
use l2_util::ObjectId;
use tracing::debug;

use crate::data::{ItemStatic, RACE_DWARF, StaticData, WorldSettings};
use crate::player::{ItemData, MacroData, Player, PlayerData, ShortcutData};
use crate::rewards::nevit::{
    NEVIT_HOURGLASS_ACTIVE, NEVIT_HOURGLASS_MAINTAIN, NEVIT_HOURGLASS_PAUSED, hourglass_bonus,
};
use crate::world::World;

/// Java `Inventory` item location names of equipped items (`ItemLocation.PAPERDOLL`, `PET_EQUIP`).
const LOC_PAPERDOLL: &str = "PAPERDOLL";
const LOC_PET_EQUIP: &str = "PET_EQUIP";
/// Items that make up the inventory (`Inventory.restore` loads these two locations).
const LOC_INVENTORY: &str = "INVENTORY";

/// Java `Item.isEquipped()`.
fn is_equipped(item: &ItemData) -> bool {
    item.loc == LOC_PAPERDOLL || item.loc == LOC_PET_EQUIP
}

/// The inventory items (what `Inventory.getItems()` returns): inventory and equipped.
fn inventory_items(data: &PlayerData) -> impl Iterator<Item = &ItemData> {
    data.items
        .iter()
        .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
}

/// The item in each paperdoll slot (Java `Inventory._paperdoll`).
pub fn paperdoll_items(data: &PlayerData) -> [Option<&ItemData>; paperdoll::TOTAL_SLOTS] {
    let mut slots = [None; paperdoll::TOTAL_SLOTS];
    for item in data.items.iter().filter(|i| i.loc == LOC_PAPERDOLL) {
        if let Some(slot) = usize::try_from(item.loc_data).ok().and_then(|s| slots.get_mut(s)) {
            *slot = Some(item);
        }
    }
    slots
}

fn item_static(st: &dyn StaticData, item: &ItemData) -> ItemStatic {
    st.item(item.item_id)
        .unwrap_or_else(|| ItemStatic::unknown(item.item_id))
}

/// The datapack template of an item, for the fields that need more than [`ItemStatic`].
fn item_template<'a>(st: &'a dyn StaticData, item: &ItemData) -> Option<&'a ItemTemplate> {
    st.stat_data().and_then(|d| d.items.get(item.item_id))
}

/// Java `new ItemInfo(item)` as `AbstractItemPacket.writeItem` reads it.
///
/// `template` is the item's template when the datapack has it: the element fields
/// (`getAttackElementType`, `getAttackElementPower`, `getElementDefAttr`) read the template
/// first and the instance (`item_elementals`) second.
pub fn item_info(item: &ItemData, st: &ItemStatic, template: Option<&ItemTemplate>, now_ms: i64) -> ItemInfo {
    ItemInfo {
        object_id: item.object_id.0,
        item_id: st.display_id,
        location: item.loc_data,
        count: item.count,
        type2: st.type2,
        // Java writes the low 16 bits of these ints.
        custom_type1: item.custom_type1 as i16,
        equipped: i16::from(is_equipped(item)),
        body_part: st.body_part,
        enchant: item.enchant_level as i16,
        custom_type2: item.custom_type2 as i16,
        // `item.isAugmented() ? item.getAugmentation().getAugmentationId() : 0`
        augmentation: item.augmentation.unwrap_or(0),
        attack_element_type: item.attack_element_type(template) as i16,
        attack_element_power: item.attack_element_power(template) as i16,
        element_defense: std::array::from_fn(|element| {
            item.element_def_attr(element as i32, template) as i16
        }),
        mana: item.mana_left,
        // `isTimeLimitedItem() ? (int) (getRemainingTime() / 1000) : -9999`
        time: if item.time > 0 {
            ((item.time - now_ms) / 1000) as i32
        } else {
            -9999
        },
        ..ItemInfo::plain(item.object_id.0, st.display_id, item.count)
    }
}

/// Java `new ItemList(player, false)`: every inventory item that is not a quest item.
pub fn item_list(data: &PlayerData, st: &dyn StaticData, now_ms: i64) -> ItemList {
    let items = inventory_items(data)
        .filter_map(|item| {
            let stat = item_static(st, item);
            (!stat.quest_item).then(|| item_info(item, &stat, item_template(st, item), now_ms))
        })
        .collect();
    ItemList {
        show_window: false,
        items,
        block: None,
    }
}

/// Java `new ExQuestItemList(player)`.
pub fn quest_item_list(data: &PlayerData, st: &dyn StaticData, now_ms: i64) -> ExQuestItemList {
    let items = inventory_items(data)
        .filter_map(|item| {
            let stat = item_static(st, item);
            stat.quest_item
                .then(|| item_info(item, &stat, item_template(st, item), now_ms))
        })
        .collect();
    ExQuestItemList { items, block: None }
}

/// Java `new GMViewItemList(player)`: every inventory item, quest items included, with the
/// character's name and inventory limit (`getInventoryLimit()`).
pub fn gm_view_item_list(
    data: &PlayerData,
    limit: i32,
    st: &dyn StaticData,
    now_ms: i64,
) -> l2_protocol_game::server::GmViewItemList {
    l2_protocol_game::server::GmViewItemList {
        player_name: data.name.clone(),
        limit,
        items: inventory_items(data)
            .map(|item| item_info(item, &item_static(st, item), item_template(st, item), now_ms))
            .collect(),
    }
}

/// Java `Player.sendSkillList()` for a character without transformation or clan.
pub fn skill_list(data: &PlayerData, st: &dyn StaticData) -> SkillList {
    SkillList {
        skills: data
            .skills
            .iter()
            .map(|skill| SkillListEntry {
                id: skill.skill_id,
                level: skill.skill_level,
                passive: st.skill_is_passive(skill.skill_id),
                // Java: a clan skill of a clan with negative reputation; `enchanted` needs the
                // skill enchant tables. Neither exists yet.
                disabled: false,
                enchanted: false,
            })
            .collect(),
    }
}

/// Java `ShortcutInit(player)`.
///
/// Item shortcuts whose item is gone are left out, like `Shortcuts.restoreMe` does (Java also
/// deletes their rows; the persistence task never deletes rows it did not see in a snapshot, so
/// the row stays until the Java game server cleans it up).
pub fn shortcut_init(data: &PlayerData, st: &dyn StaticData) -> ShortcutInit {
    let mut shortcuts = Vec::new();
    for sc in &data.shortcuts {
        if let Some(entry) = shortcut_entry(data, st, sc) {
            shortcuts.push(entry);
        }
    }
    ShortcutInit { shortcuts }
}

fn shortcut_entry(data: &PlayerData, st: &dyn StaticData, sc: &ShortcutData) -> Option<ShortcutEntry> {
    // Java: `getSlot() + getPage() * 12`
    let slot = sc.slot + sc.page * 12;
    // `ShortcutType` ordinals: NONE, ITEM, SKILL, ACTION, MACRO, RECIPE, BOOKMARK.
    Some(match sc.kind {
        0 => ShortcutEntry::Empty { slot },
        1 => {
            let item = data.items.iter().find(|i| i.object_id.0 == sc.shortcut_id)?;
            ShortcutEntry::Item {
                slot,
                id: sc.shortcut_id,
                shared_reuse_group: item_static(st, item).shared_reuse_group,
            }
        }
        2 => ShortcutEntry::Skill {
            slot,
            id: sc.shortcut_id,
            level: sc.level,
        },
        3 => ShortcutEntry::Other {
            kind: OtherShortcut::Action,
            slot,
            id: sc.shortcut_id,
        },
        4 => ShortcutEntry::Other {
            kind: OtherShortcut::Macro,
            slot,
            id: sc.shortcut_id,
        },
        5 => ShortcutEntry::Other {
            kind: OtherShortcut::Recipe,
            slot,
            id: sc.shortcut_id,
        },
        6 => ShortcutEntry::Other {
            kind: OtherShortcut::Bookmark,
            slot,
            id: sc.shortcut_id,
        },
        // Java: `ShortcutType.values()[type]` throws, which drops all shortcuts. A bad row is
        // left out here and the rest is shown.
        _ => return None,
    })
}

fn macro_entry(m: &MacroData) -> MacroEntry {
    MacroEntry {
        id: m.id,
        name: m.name.clone(),
        descr: m.descr.clone(),
        acronym: m.acronym.clone(),
        icon: m.icon as u8,
        commands: m
            .commands
            .iter()
            .map(|c| MacroCommandEntry {
                kind: c.kind as u8,
                d1: c.d1,
                d2: c.d2 as u8,
                cmd: c.cmd.clone(),
            })
            .collect(),
    }
}

/// Java `MacroList.sendUpdate()`: the revision goes up from 1 to 2 on the first send; one packet
/// per macro, or a single packet without a macro when there are none.
pub fn macro_lists(macros: &[MacroData]) -> Vec<MacroList> {
    const REVISION: i32 = 2;
    if macros.is_empty() {
        return vec![MacroList {
            revision: REVISION,
            count: 0,
            macro_entry: None,
        }];
    }
    macros
        .iter()
        .map(|m| MacroList {
            revision: REVISION,
            count: macros.len() as u8,
            macro_entry: Some(macro_entry(m)),
        })
        .collect()
}

/// Java `Player.getCurrentLoad()` (`Inventory.getTotalWeight()`): the weight of everything in the
/// inventory, equipped items included.
pub fn current_load(data: &PlayerData, st: &dyn StaticData) -> i32 {
    let total: i64 = inventory_items(data)
        .map(|item| i64::from(item_static(st, item).weight) * item.count)
        .sum();
    // Java's total is an int.
    total as i32
}

/// Java `Player.refreshOverloaded`: the weight penalty level 0 to 4.
pub fn weight_penalty(current_load: i32, max_load: i32) -> i32 {
    if max_load <= 0 {
        return 0;
    }
    // `(getCurrentLoad() - getBonusWeightPenalty()) * 1000 / getMaxLoad()`, bonus penalty 0.
    let proc = i64::from(current_load) * 1000 / i64::from(max_load);
    match proc {
        ..500 => 0,
        500..666 => 1,
        666..800 => 2,
        800..1000 => 3,
        _ => 4,
    }
}

/// Java `Player.getInventoryLimit()` without the `INV_LIM` stat (no skills yet).
pub fn inventory_limit(player: &PlayerData, is_gm: bool, settings: &WorldSettings) -> i32 {
    if is_gm {
        settings.inventory_maximum_gm
    } else if player.race == RACE_DWARF {
        settings.inventory_maximum_dwarf
    } else {
        settings.inventory_maximum_no_dwarf
    }
}

/// Java `new ExStorageMaxCount(player)` without the stat bonuses (`INV_LIM`, `WH_LIM`...).
pub fn storage_max_count(player: &PlayerData, is_gm: bool, settings: &WorldSettings) -> ExStorageMaxCount {
    let dwarf = player.race == RACE_DWARF;
    ExStorageMaxCount {
        inventory: inventory_limit(player, is_gm, settings),
        warehouse: if dwarf {
            settings.warehouse_slots_dwarf
        } else {
            settings.warehouse_slots_no_dwarf
        },
        clan: settings.warehouse_slots_clan,
        private_sell: if dwarf {
            settings.max_pvtstoresell_slots_dwarf
        } else {
            settings.max_pvtstoresell_slots_other
        },
        private_buy: if dwarf {
            settings.max_pvtstorebuy_slots_dwarf
        } else {
            settings.max_pvtstorebuy_slots_other
        },
        dwarven_recipe: settings.dwarf_recipe_limit,
        common_recipe: settings.common_recipe_limit,
        inventory_extra_slots: 0,
        inventory_quest_items: settings.inventory_maximum_quest_items,
    }
}

/// Java `new ExVoteSystemInfo(player)`, for an hourglass with `hourglass_secs` seconds left
/// (`Player.getNevitHourglassTime`), `paused` when the player stands in a peace zone
/// (`PlayerStat.hasPausedNevitHourglass`).
pub fn vote_system_info(player: &Player, hourglass_secs: i32, paused: bool) -> ExVoteSystemInfo {
    let time = hourglass_secs.max(0);
    // Java `getNevitHourglassBonus`: only while online with recommendations and an active hourglass.
    let bonus = if player.recom_have != 0 && (time > 0 || paused) {
        hourglass_bonus(player.core.level, player.recom_have)
    } else {
        0
    };
    ExVoteSystemInfo {
        recom_left: player.recom_left,
        recom_have: player.recom_have,
        bonus_time: time,
        bonus_value: bonus,
        bonus_type: if paused {
            NEVIT_HOURGLASS_PAUSED
        } else if time > 0 {
            NEVIT_HOURGLASS_ACTIVE
        } else {
            NEVIT_HOURGLASS_MAINTAIN
        },
    }
}

/// Java `ExUISetting`: `PlayerVariables.UI_KEY_MAPPING` is the stored bytes as signed decimal
/// numbers, each followed by a tab.
pub fn ui_key_mapping(player: &PlayerData) -> Vec<u8> {
    let Some(value) = player.profile.variables.get("UI_KEY_MAPPING") else {
        return Vec::new();
    };
    // Java `String.split("\t")` drops trailing empty strings.
    let mut parts: Vec<&str> = value.split('\t').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    // `Byte.parseByte`; a part that does not parse leaves its byte at 0.
    parts
        .iter()
        .map(|p| p.parse::<i8>().map_or(0, |b| b as u8))
        .collect()
}

/// Java `new ExUISetting(player)`; `None` when `StoreCharUiSettings` is off (Java then sends
/// nothing).
pub fn ui_setting(player: &PlayerData, settings: &WorldSettings) -> Option<ExUiSetting> {
    settings.store_ui_settings.then(|| ExUiSetting {
        ui_key_mapping: ui_key_mapping(player),
    })
}

impl World {
    /// Java `new UserInfo(player)` for the player's current state.
    pub fn user_info(&self, id: ObjectId) -> Option<UserInfo> {
        let player = self.players.get(&id)?;
        Some(build_user_info(self, player))
    }

    /// Java `Player.updateUserInfo()`: `UserInfo` and `ExBrExtraUserInfo`.
    pub(crate) fn send_user_info(&self, id: ObjectId) {
        let Some(info) = self.user_info(id) else {
            return;
        };
        self.send(id, &info);
        self.send(
            id,
            &ExBrExtraUserInfo {
                object_id: id.0,
                abnormal_visual_effects_event: self.abnormal_visual_effects(id).2,
                lecture_mark: 1,
            },
        );
    }

    /// The packets of Java's `EnterWorld.runImpl`, in its order (see the module comment for what
    /// is left out).
    pub(crate) fn on_entered(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        let now_ms = crate::world::unix_ms_now();

        // Java `EnterWorld`: `GMStartupInvulnerable` and `AdminData.hasAccess("admin_invul", ..)`
        // for a GM (`player.setInvul(true)`). The flag shows in no packet, so it is set first.
        if self.settings.gm_startup_invulnerable
            && self.is_gm(id)
            && self.players.get(&id).is_some_and(|p| {
                self.data
                    .admin_rights()
                    .is_some_and(|rights| rights.has_access("admin_invul", p.data.access_level))
            })
        {
            self.set_invul(id, true);
        }
        // `new Player(...)` -> `startVitalityTask()`.
        self.start_vitality_task(id);

        // player.sendPacket(new UserInfo(player)), without ExBrExtraUserInfo. The
        // recommendations are not loaded yet (`checkRecoBonusTask` below), so this one shows 0,
        // like the Java capture.
        if let Some(info) = self.user_info(id) {
            self.send(id, &info);
        }

        // `if (!player.isGM()) player.updatePvpTitleAndColor(false)`: the custom title for the
        // PvP kills of the character (`Custom/PvpTitleColor.ini`).
        if !self.is_gm(id) {
            self.update_pvp_title_and_color(id, false);
        }

        // `if (ENABLE_VITALITY && RECOVER_VITALITY_ON_RECONNECT)`: the time offline gives vitality
        // back (quietly, after the first `UserInfo`).
        self.recover_vitality_on_reconnect(id, now_ms);

        // `if (PlayerConfig.NEVIT_ENABLED) player.checkRecoBonusTask()`: loads the
        // recommendations, starts the hourglass (`ExVoteSystemInfo`) and stores them. The store is
        // the next one of the persistence task (`PlayerProfile::reco_store`).
        if self.settings.nevit_enabled {
            self.check_reco_bonus_task(id);
        }

        // Java `Player.load` -> `restoreItemReuse()`: the item reuse timers of the last logout.
        self.restore_item_reuse(id);

        // Macros, items, bookmarks, shortcuts, actions, hennas, quests, rotation, status icons,
        // storage limits, friends, the welcome message and the skill cool times.
        for bytes in self.enter_packets(id, now_ms) {
            self.send_bytes(id, bytes);
        }
        // `player.spawnMe(x, y, z)`: everybody around sees the player, and the player sees them
        // (NpcInfo, CharInfo, ... come here, between QuestList and ExRotation), then the zones
        // the player stands in say so (`onSpawn` -> `revalidateZone(true)`: zone messages and
        // the compass icon).
        if let Some((x, y, z)) = self.position_of(id) {
            self.spawn_at(id, x, y, z);
        }
        // Built after the spawn: `EtcStatusUpdate` shows the danger icon of a zone.
        for bytes in self.after_spawn_packets(id) {
            self.send_bytes(id, bytes);
        }
        // Java `onPlayerEnter` -> `restoreEffects()`: the buffs of the last logout come back with
        // the time they had left (`effects/save.rs`).
        self.restore_saved_effects(id);

        if self.settings.nevit_enabled {
            // `player.sendPacket(new ExVoteSystemInfo(player))`
            self.send_vote_system_info(id);
        }
        // ExShowContactList: the contacts are not loaded.
        self.send(id, &ExShowContactList::default());
        if self.settings.nevit_enabled {
            // Nevit.onPlayerLogin: the hunting points and time are player variables.
            self.nevit_on_player_login(id);
        }

        // NOTE(java-quirk): Java builds the second UserInfo (`broadcastUserInfo`) before it calls
        // `setEnteredWorld()`, but writes it after, so the client is told it cannot equip a cloak
        // only in the first one (the Java capture shows exactly that).
        if let Some(p) = self.players.get_mut(&id) {
            p.entered_world = true;
        }
        self.send_user_info(id);
        let Some(p) = self.players.get(&id) else {
            return;
        };
        // "Prevent relogin in game gfx."
        self.send(
            id,
            &ValidateLocation {
                object_id: id.0,
                x: p.core.x,
                y: p.core.y,
                z: p.core.z,
                heading: p.core.heading,
            },
        );
        // "Unstuck players that had client open when server crashed."
        self.send(id, &ActionFailed);
        // The `sendSkillList()` of EnterWorld is queued in Java and reaches the client last.
        self.send(id, &skill_list(&p.data, &*self.data));
        debug!("enter-world packets sent to {id}");
        // `ON_PLAYER_LOGIN`: `JailHandler.onPlayerLogin`.
        self.punishment_on_player_login(id);
    }

    /// Java `player.sendPacket(new ExVoteSystemInfo(player))`: the recommendations and the state of
    /// the Nevit hourglass now.
    pub(crate) fn send_vote_system_info(&self, id: ObjectId) {
        if let Some(p) = self.players.get(&id) {
            let packet = vote_system_info(p, self.nevit_hourglass_time(id), self.nevit_hourglass_paused(id));
            self.send(id, &packet);
        }
    }

    /// Java `new HennaInfo(player)`: the six dye bonuses from the stat engine (`Player.getHennaStat*`),
    /// 3 slots, and the dye ids of the slots in use. A dye id the datapack does not know is an
    /// empty slot (Java's `HennaData.getHenna` gives null for it), so it is neither sent nor
    /// counted.
    pub(crate) fn henna_info(&self, id: ObjectId) -> HennaInfo {
        let Some(player) = self.players.get(&id) else {
            return HennaInfo::default();
        };
        let known = |dye_id: i32| {
            self.data
                .stat_data()
                .is_some_and(|d| dye_id != 0 && d.hennas.get(dye_id).is_some())
        };
        let dye_ids: Vec<i32> = player
            .data
            .profile
            .hennas
            .iter()
            .copied()
            .filter(|dye_id| known(*dye_id))
            .collect();
        let stats = self
            .stats(id)
            .and_then(|view| view.henna_stats())
            .unwrap_or_default();
        // Java `writeByte`: each value goes out as its low byte (the dye bonuses fit in one).
        HennaInfo {
            int: stats.int as i8,
            str: stats.str as i8,
            con: stats.con as i8,
            men: stats.men as i8,
            dex: stats.dex as i8,
            wit: stats.wit as i8,
            dye_ids,
        }
    }

    /// The encoded packets from the macro list to `QuestList`, which Java sends before
    /// `spawnMe`.
    fn enter_packets(&self, id: ObjectId, now_ms: i64) -> Vec<Vec<u8>> {
        use l2_net::Encode;

        let Some(player) = self.players.get(&id) else {
            return Vec::new();
        };
        let data = &player.data;
        let st = &*self.data;
        let mut packets: Vec<Vec<u8>> = Vec::new();
        // player.getMacros().sendUpdate()
        for list in macro_lists(&data.profile.macros) {
            packets.push(list.to_bytes());
        }
        // player.sendPacket(new ItemList(player, false)), then ItemList.runImpl: ExQuestItemList
        packets.push(item_list(data, st, now_ms).to_bytes());
        packets.push(quest_item_list(data, st, now_ms).to_bytes());
        // ExGetBookMarkInfoPacket. NOTE: the bookmarks (`character_tpbookmark`) are not loaded.
        packets.push(
            ExGetBookMarkInfoPacket {
                slot: data.profile.bookmark_slot,
                bookmarks: Vec::new(),
            }
            .to_bytes(),
        );
        packets.push(shortcut_init(data, st).to_bytes());
        packets.push(ExBasicActionList::default_list().to_bytes());
        // HennaInfo, then QuestList: not loaded (no quest system yet).
        packets.push(self.henna_info(id).to_bytes());
        packets.push(QuestList::default().to_bytes());
        packets
    }

    /// The encoded packets from `ExRotation` to `SkillCoolTime`, which Java sends after
    /// `spawnMe`.
    fn after_spawn_packets(&self, id: ObjectId) -> Vec<Vec<u8>> {
        use l2_net::Encode;

        let Some(player) = self.players.get(&id) else {
            return Vec::new();
        };
        let data = &player.data;
        let is_gm = self.data.access_level(data.access_level).is_gm;
        let mut packets: Vec<Vec<u8>> = Vec::new();
        packets.push(
            ExRotation {
                char_id: id.0,
                heading: player.core.heading,
            }
            .to_bytes(),
        );
        // EtcStatusUpdate (the weight penalty comes from refreshOverloaded)
        packets.extend(self.etc_status_update(id).map(|p| p.to_bytes()));
        packets.push(storage_max_count(data, is_gm, &self.settings).to_bytes());
        // FriendList: not loaded.
        packets.push(FriendList::default().to_bytes());
        packets.push(SystemMessage::new(SystemMessageId::WelcomeToTheWorldOfLineageIi).to_bytes());
        // Seven Signs period message, announcements, server news: not ported.
        // "Dead or fake dead": no broadcast needed since the player will already spawn dead to
        // others.
        if self.is_alike_dead(id) {
            packets.push(self.die_packet(id).to_bytes());
        }
        // SkillCoolTime: the reuse timers come back later (`onPlayerEnter` -> `restoreEffects`),
        // so this list is empty, as in Java.
        packets.push(self.skill_cool_time(id).to_bytes());
        packets
    }

    /// Java `new EtcStatusUpdate(player)`: the weight penalty, the grade penalties, the death
    /// penalty and the danger icon of the zone the player stands in. The charges and the souls
    /// are the effects' (`effects/charges.rs`); the refusal flags are 0 until their systems exist.
    pub(crate) fn etc_status_update(&self, id: ObjectId) -> Option<EtcStatusUpdate> {
        let player = self.players.get(&id)?;
        let data = &player.data;
        Some(EtcStatusUpdate {
            weight_penalty: weight_penalty(
                current_load(data, &*self.data),
                self.stats(id).map_or(0, |s| s.max_load()),
            ),
            charges: self.charges_of(id),
            charged_souls: self.charged_souls_of(id),
            // `getMessageRefusal() || isChatBanned() || isSilenceMode()`: the refusal and the
            // silence mode are not modelled.
            chat_blocked: self.is_chat_banned(id),
            danger_area: player.zones.is_inside(crate::zones::ZoneId::DangerArea),
            expertise_weapon_penalty: player.expertise_weapon_penalty,
            expertise_armor_penalty: player.expertise_armor_penalty,
            death_penalty_level: data.profile.death_penalty_level,
            ..EtcStatusUpdate::default()
        })
    }

    /// `sendPacket(new EtcStatusUpdate(player))`.
    pub(crate) fn send_etc_status_update(&self, id: ObjectId) {
        if let Some(packet) = self.etc_status_update(id) {
            self.send(id, &packet);
        }
    }
}

/// Java `new UserInfo(player)`. Every number Java takes from the stat system comes from the stat
/// engine (`World::stats`).
pub fn build_user_info(world: &World, player: &Player) -> UserInfo {
    let st = &*world.data;
    let settings = &world.settings;
    let data = &player.data;
    let core = &player.core;
    let view = crate::stats::StatView::of_player(world, player);
    let access = st.access_level(data.access_level);
    // `Creature.getAbnormalVisualEffects()` and the special mask; an invisible player also
    // shows the stealth effect to itself.
    let (visual_effects, visual_effects_special, _) = world.abnormal_visual_effects(core.object_id);
    let invisible = world.objects.get(&core.object_id).is_some_and(|e| e.invisible);
    let slots = paperdoll_items(data);
    let mut object_ids = [0; paperdoll::TOTAL_SLOTS];
    let mut item_ids = [0; paperdoll::TOTAL_SLOTS];
    for (slot, item) in slots.iter().enumerate() {
        if let Some(item) = item {
            object_ids[slot] = item.object_id.0;
            // `getPaperdollItemDisplayId`
            item_ids[slot] = item_static(st, item).display_id;
        }
    }
    // `getMovementSpeedMultiplier()` and the speeds divided by it, `(int) Math.round(...)`.
    let move_multiplier = view.movement_speed_multiplier();
    let speed = |value: f64| crate::stats::java_round(value / move_multiplier) as i32;
    let attack_element = view.attack_element();
    let defense = |element: i8| view.defense_element_value(element) as i16;
    let enchant_effect = slots[paperdoll::RHAND].map_or(0, |w| w.enchant_level.min(127));
    UserInfo {
        x: core.x,
        y: core.y,
        z: core.z,
        vehicle_id: 0,
        object_id: core.object_id.0,
        name: data.name.clone(),
        race: data.race,
        is_female: data.sex != 0,
        base_class: data.base_class,
        level: core.level,
        exp: data.exp,
        exp_percent: st.exp_percent(data.exp, core.level),
        str: view.str(),
        dex: view.dex(),
        con: view.con(),
        int: view.int(),
        wit: view.wit(),
        men: view.men(),
        max_hp: view.max_hp(),
        // `(int) Math.round(getCurrentHp())`
        current_hp: core.cur_hp.round() as i32,
        max_mp: view.max_mp(),
        current_mp: core.cur_mp.round() as i32,
        sp: data.sp as i32,
        current_load: current_load(data, st),
        max_load: view.max_load(),
        // NOTE(java-quirk): `getActiveWeaponItem()` returns the fists weapon for an unarmed
        // character, never null, so Java always writes 40.
        weapon_equipped: true,
        paperdoll_object_ids: object_ids,
        paperdoll_item_ids: item_ids,
        paperdoll_augmentation_ids: [0; paperdoll::TOTAL_SLOTS],
        talisman_slots: view.talisman_slots(),
        // `PlayerStat.canEquipCloak()`: true until the player has entered the world.
        can_equip_cloak: view.can_equip_cloak(),
        // `(int) getPAtk(null)`, `(int) getPAtkSpd()`, ...
        p_atk: view.p_atk(None) as i32,
        p_atk_spd: view.p_atk_spd() as i32,
        p_def: view.p_def(None) as i32,
        evasion: view.evasion_rate(None),
        accuracy: view.accuracy(),
        critical: view.critical_hit(None, None),
        m_atk: view.m_atk(None, None) as i32,
        m_atk_spd: view.m_atk_spd(),
        m_def: view.m_def(None, None) as i32,
        pvp_flag: world.pvp_flag_of(core.object_id),
        karma: data.karma,
        run_speed: speed(view.run_speed()),
        walk_speed: speed(view.walk_speed()),
        swim_run_speed: speed(view.swim_run_speed()),
        swim_walk_speed: speed(view.swim_walk_speed()),
        // `isFlying() ? run : 0`: nothing flies yet.
        fly_run_speed: 0,
        fly_walk_speed: 0,
        move_multiplier,
        attack_speed_multiplier: view.attack_speed_multiplier(),
        collision_radius: view.collision_radius(),
        collision_height: view.collision_height(),
        hair_style: data.hair_style,
        hair_color: data.hair_color,
        face: data.face,
        is_gm: access.is_gm,
        title: data.title.clone(),
        clan_id: 0,
        clan_crest_id: 0,
        ally_id: 0,
        ally_crest_id: 0,
        relation: 0,
        mount_type: 0,
        private_store_type: 0,
        // `getSkillLevel(CommonSkill.CREATE_DWARVEN) >= 1`
        has_dwarven_craft: data
            .skills
            .iter()
            .any(|s| s.skill_id == 172 && s.skill_level >= 1),
        pk_kills: data.pk_kills,
        pvp_kills: data.pvp_kills,
        cubics: Vec::new(),
        in_party_match_room: false,
        abnormal_visual_effects: if invisible {
            visual_effects | crate::player_view::STEALTH_MASK
        } else {
            visual_effects
        },
        water_or_flying: 0,
        clan_privileges: 0,
        recom_left: player.recom_left as i16,
        recom_have: player.recom_have as i16,
        mount_npc_id: 0,
        inventory_limit: view.inventory_limit() as i16,
        class_id: data.class_id,
        max_cp: view.max_cp(),
        current_cp: core.cur_cp as i32,
        enchant_effect: enchant_effect as u8,
        team: 0,
        clan_crest_large_id: 0,
        noble: data.profile.noble,
        // `isHero() || (isGM() && GM_HERO_AURA)`; the heroes table is not loaded.
        hero: access.is_gm && settings.gm_hero_aura,
        fishing: false,
        fish_x: 0,
        fish_y: 0,
        fish_z: 0,
        name_color: access.name_color,
        running: view.is_running(),
        // Restore: no clan gives 5 for a noble (hero 8 once heroes are loaded), else 0.
        pledge_class: if data.profile.noble { 5 } else { 0 },
        pledge_type: 0,
        title_color: if data.profile.title_color == crate::data::DEFAULT_TITLE_COLOR {
            access.title_color
        } else {
            data.profile.title_color
        },
        cursed_weapon_level: 0,
        transformation_display_id: 0,
        attack_element: i16::from(attack_element),
        attack_element_value: view.attack_element_value(attack_element) as i16,
        defense_fire: defense(0),
        defense_water: defense(1),
        defense_wind: defense(2),
        defense_earth: defense(3),
        defense_holy: defense(4),
        defense_dark: defense(5),
        agathion_id: 0,
        fame: data.profile.fame,
        minimap_allowed: false,
        vitality_points: data.profile.vitality_points,
        abnormal_visual_effect_special: visual_effects_special,
    }
}
