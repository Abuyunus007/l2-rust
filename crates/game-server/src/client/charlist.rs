//! The character list of an account (`CharSelectionInfo`), read from the shared database.
// Java: org/l2jmobius/gameserver/network/serverpackets/CharSelectionInfo.java
//   (loadCharacterSelectInfo, restoreChar, loadCharacterSubclassInfo)
// Java: org/l2jmobius/gameserver/entity/itemcontainer/PlayerInventory.java (restoreVisibleInventory)
//
// The list is built from read-only queries. Nothing here writes to the database (D-39).

use std::sync::Arc;

use l2_data::Datapack;
use l2_db::{CharacterRepository, CharacterRow, ItemRepository, PaperdollItemRow, Pool, SubclassRepository};
use l2_protocol_game::server::charscreen::{CharSelectEntry, delete_seconds, paperdoll};
use l2_util::time::now_millis;
use tracing::warn;

use super::BoxFuture;

/// One character of the list, plus what the session needs later (selection, deletion).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterInfo {
    pub entry: CharSelectEntry,
    /// `lastAccess`: the entry with the highest one is highlighted.
    pub last_access: i64,
    /// `deletetime` (ms), 0 when the character is not marked for deletion.
    pub delete_time_ms: i64,
    pub access_level: i32,
    /// `title` (empty when the column is NULL), for `CharSelected`.
    pub title: String,
}

/// Where the character list comes from. The database in the server, a fake in tests.
pub trait CharacterSource: Send + Sync + 'static {
    /// The characters of `account`, in creation order. Errors are logged and give the
    /// characters read so far (Java catches and logs, the list stays partial or empty).
    fn character_list<'a>(&'a self, account: &'a str) -> BoxFuture<'a, Vec<CharacterInfo>>;
}

/// Exp, sp and level of the active subclass (`character_subclasses`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubclassStats {
    pub exp: i64,
    pub sp: i64,
    pub level: i32,
}

/// The paperdoll of the list entry: `[slot] -> (object id, display item id, enchant)`.
/// Java's array has 31 slots; the packet only shows the first [`paperdoll::TOTAL_SLOTS`].
// Java: PlayerInventory.restoreVisibleInventory
pub fn visible_paperdoll(
    rows: &[PaperdollItemRow],
    datapack: &Datapack,
) -> [(i32, i32, i32); paperdoll::TOTAL_SLOTS] {
    let mut slots = [(0, 0, 0); paperdoll::TOTAL_SLOTS];
    for row in rows {
        let Some(slot) = row.loc_data.and_then(|s| usize::try_from(s).ok()) else {
            continue;
        };
        let Some(target) = slots.get_mut(slot) else {
            continue;
        };
        let item_id = row.item_id.unwrap_or(0);
        // Java (transmog off): the template's display id, or the item id if the template is
        // unknown. Transmog (TransmogConfig.ENABLE_TRANSMOG) is not supported yet.
        let display_id = datapack.items.get(item_id).map_or(item_id, |t| t.display_id);
        *target = (row.object_id, display_id, row.enchant_level.unwrap_or(0));
    }
    slots
}

/// Builds one list entry the way `restoreChar` does. `None` for a character whose deletion time
/// has passed.
// Java: CharSelectionInfo.restoreChar + the per-entry part of writeImpl
pub fn build_character(
    row: &CharacterRow,
    subclass: Option<SubclassStats>,
    paperdoll_slots: &[(i32, i32, i32); paperdoll::TOTAL_SLOTS],
    augmentation: Option<i32>,
    datapack: &Datapack,
    now_ms: i64,
) -> Option<CharacterInfo> {
    let delete_time_ms = i64::try_from(row.deletetime).unwrap_or(i64::MAX);
    if delete_time_ms > 0 && now_ms > delete_time_ms {
        // Java deletes the character right here; `DbCharacterList::load` does that before it
        // gets here, so this only keeps the list free of it.
        return None;
    }
    let base_class = i32::from(row.base_class);
    let active_class = i32::from(row.classid.unwrap_or(0));
    let mut level = i32::from(row.level.unwrap_or(0));
    let mut exp = row.exp.map_or(0, |e| i64::try_from(e).unwrap_or(i64::MAX));
    let mut sp = i64::try_from(row.sp).unwrap_or(i64::MAX);
    if base_class != active_class
        && let Some(sub) = subclass
    {
        exp = sub.exp;
        sp = sub.sp;
        level = sub.level;
    }
    let mut entry = CharSelectEntry {
        name: row.char_name.clone(),
        object_id: row.object_id(),
        clan_id: row.clanid.map_or(0, |c| c as i32),
        sex: i32::from(row.sex.unwrap_or(0)),
        race: i32::from(row.race.unwrap_or(0)),
        // Java: characters from before `base_class` existed show their active class.
        base_class_id: if base_class == 0 && active_class > 0 {
            active_class
        } else {
            base_class
        },
        x: row.x.unwrap_or(0),
        y: row.y.unwrap_or(0),
        z: row.z.unwrap_or(0),
        current_hp: f64::from(row.cur_hp.unwrap_or(0)),
        current_mp: f64::from(row.cur_mp.unwrap_or(0)),
        // Java: `(int) sp` of a long.
        sp: sp as i32,
        exp,
        exp_percent: datapack.experience.exp_percent(exp, level).unwrap_or(0.0),
        level,
        karma: row.karma.map_or(0, |k| k as i32),
        pk_kills: i32::from(row.pkkills.unwrap_or(0)),
        pvp_kills: i32::from(row.pvpkills.unwrap_or(0)),
        paperdoll: [0; paperdoll::TOTAL_SLOTS],
        hair_style: i32::from(row.hair_style.unwrap_or(0)),
        hair_color: i32::from(row.hair_color.unwrap_or(0)),
        face: i32::from(row.face.unwrap_or(0)),
        // Java reads maxhp/maxmp with getInt.
        max_hp: f64::from(row.max_hp.map_or(0, |v| v as i32)),
        max_mp: f64::from(row.max_mp.map_or(0, |v| v as i32)),
        delete_seconds: delete_seconds(row.accesslevel.unwrap_or(0), delete_time_ms, now_ms),
        class_id: active_class,
        selected: false,
        enchant_effect: paperdoll_slots[paperdoll::RHAND].2,
        // Java: -1 in the table means "none".
        augmentation_id: match augmentation {
            Some(-1) | None => 0,
            Some(aug) => aug,
        },
        vitality_points: i32::from(row.vitality_points),
    };
    for (slot, item) in paperdoll_slots.iter().enumerate() {
        entry.paperdoll[slot] = item.1;
    }
    Some(CharacterInfo {
        entry,
        last_access: i64::try_from(row.last_access).unwrap_or(i64::MAX),
        delete_time_ms,
        access_level: row.accesslevel.unwrap_or(0),
        title: row.title.clone().unwrap_or_default(),
    })
}

/// The list from the game database.
pub struct DbCharacterList {
    characters: CharacterRepository,
    /// `WeddingConfig.ALLOW_WEDDING`: `deleteCharByObjId` then also clears `mods_wedding`.
    delete_wedding: bool,
    items: ItemRepository,
    subclasses: SubclassRepository,
    datapack: Arc<Datapack>,
}

impl DbCharacterList {
    pub fn new(pool: Pool, datapack: Arc<Datapack>, delete_wedding: bool) -> Self {
        Self {
            delete_wedding,
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            subclasses: SubclassRepository::new(pool),
            datapack,
        }
    }

    async fn load(&self, account: &str) -> Vec<CharacterInfo> {
        let rows = match self.characters.characters_of_account(account).await {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Could not restore char info: {e}");
                return Vec::new();
            }
        };
        let now = now_millis();
        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            let object_id = row.object_id();
            // See if the char must be deleted.
            let delete_time = i64::try_from(row.deletetime).unwrap_or(i64::MAX);
            if delete_time > 0 && now > delete_time {
                // NOTE: Java first removes the character from its clan (`Clan.removeClanMember`);
                // clans are not modelled yet, so the clan keeps a member row that points nowhere.
                if let Err(e) = self
                    .characters
                    .delete_character(object_id, self.delete_wedding)
                    .await
                {
                    warn!("Error deleting character: {e}");
                }
                continue;
            }
            let subclass = if row.base_class != row.classid.unwrap_or(0) {
                self.active_subclass(object_id, i32::from(row.classid.unwrap_or(0)))
                    .await
            } else {
                None
            };
            let paperdoll_rows = self.items.paperdoll_items(object_id).await.unwrap_or_else(|e| {
                warn!("Could not restore inventory: {e}");
                Vec::new()
            });
            let slots = visible_paperdoll(&paperdoll_rows, &self.datapack);
            let weapon = slots[paperdoll::RHAND].0;
            let augmentation = if weapon > 0 {
                self.items.augmentation_of(weapon).await.unwrap_or_else(|e| {
                    warn!("Could not restore augmentation info: {e}");
                    None
                })
            } else {
                None
            };
            if let Some(info) = build_character(&row, subclass, &slots, augmentation, &self.datapack, now) {
                list.push(info);
            }
        }
        list
    }

    // Java: CharSelectionInfo.loadCharacterSubclassInfo
    async fn active_subclass(&self, object_id: i32, class_id: i32) -> Option<SubclassStats> {
        match self.subclasses.subclasses_of(object_id).await {
            Ok(subs) => subs
                .iter()
                .find(|s| s.class_id == class_id)
                .map(|s| SubclassStats {
                    exp: s.exp,
                    sp: s.sp,
                    level: s.level,
                }),
            Err(e) => {
                warn!("Could not restore char subclass info: {e}");
                None
            }
        }
    }
}

impl CharacterSource for DbCharacterList {
    fn character_list<'a>(&'a self, account: &'a str) -> BoxFuture<'a, Vec<CharacterInfo>> {
        Box::pin(self.load(account))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn datapack() -> Datapack {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("data")).unwrap();
        Datapack::load(dir.path()).unwrap()
    }

    pub(crate) fn row(name: &str, id: u32) -> CharacterRow {
        CharacterRow {
            account_name: Some("acc".into()),
            char_id: id,
            char_name: name.into(),
            level: Some(20),
            max_hp: Some(500),
            cur_hp: Some(400),
            max_cp: Some(100),
            cur_cp: Some(100),
            max_mp: Some(300),
            cur_mp: Some(200),
            face: Some(1),
            hair_style: Some(2),
            hair_color: Some(3),
            sex: Some(1),
            heading: Some(0),
            x: Some(-71338),
            y: Some(258271),
            z: Some(-3104),
            exp: Some(123_456),
            exp_before_death: Some(0),
            sp: 5_000_000_000,
            karma: Some(0),
            fame: 0,
            pvpkills: Some(3),
            pkkills: Some(1),
            clanid: Some(0),
            race: Some(1),
            classid: Some(18),
            base_class: 18,
            transform_id: 0,
            deletetime: 0,
            cancraft: Some(0),
            title: None,
            title_color: 0xECF9A2,
            accesslevel: Some(0),
            online: Some(0),
            onlinetime: Some(0),
            char_slot: Some(0),
            newbie: Some(1),
            last_access: 1_000,
            clan_privs: Some(0),
            wantspeace: Some(0),
            isin7sdungeon: 0,
            power_grade: Some(0),
            nobless: 0,
            subpledge: 0,
            lvl_joined_academy: 0,
            apprentice: 0,
            sponsor: 0,
            clan_join_expiry_time: 0,
            clan_create_expiry_time: 0,
            death_penalty_level: 0,
            bookmarkslot: 0,
            vitality_points: 20000,
            language: None,
            faction: 0,
            pccafe_points: 0,
        }
    }

    #[test]
    fn a_row_becomes_a_list_entry_like_java() {
        let pack = datapack();
        let mut slots = [(0, 0, 0); paperdoll::TOTAL_SLOTS];
        slots[paperdoll::RHAND] = (0x6000_0001, 2369, 7);
        slots[paperdoll::CHEST] = (0x6000_0002, 1101, 0);
        let info = build_character(&row("Hero", 0x6000_0000), None, &slots, Some(-1), &pack, 5_000).unwrap();
        let e = &info.entry;
        assert_eq!(e.name, "Hero");
        assert_eq!(e.object_id, 0x6000_0000);
        assert_eq!(
            (e.level, e.exp, e.class_id, e.base_class_id),
            (20, 123_456, 18, 18)
        );
        // `(int) sp` truncates like Java.
        assert_eq!(e.sp, 5_000_000_000_i64 as i32);
        assert_eq!(
            (e.current_hp, e.max_hp, e.current_mp, e.max_mp),
            (400.0, 500.0, 200.0, 300.0)
        );
        assert_eq!(e.paperdoll[paperdoll::RHAND], 2369);
        assert_eq!(e.paperdoll[paperdoll::CHEST], 1101);
        assert_eq!(e.enchant_effect, 7);
        assert_eq!(e.augmentation_id, 0, "-1 means no augmentation");
        assert_eq!(e.delete_seconds, 0);
        assert_eq!(e.vitality_points, 20000);
        assert_eq!(info.last_access, 1_000);
    }

    #[test]
    fn subclass_values_replace_level_exp_and_sp() {
        let pack = datapack();
        let mut r = row("Sub", 1);
        r.classid = Some(25);
        let sub = SubclassStats {
            exp: 99,
            sp: 7,
            level: 41,
        };
        let slots = [(0, 0, 0); paperdoll::TOTAL_SLOTS];
        let e = build_character(&r, Some(sub), &slots, None, &pack, 0)
            .unwrap()
            .entry;
        assert_eq!(
            (e.level, e.exp, e.sp, e.class_id, e.base_class_id),
            (41, 99, 7, 25, 18)
        );
    }

    #[test]
    fn old_rows_without_base_class_show_the_active_class() {
        let pack = datapack();
        let mut r = row("Old", 1);
        r.base_class = 0;
        let slots = [(0, 0, 0); paperdoll::TOTAL_SLOTS];
        let e = build_character(&r, None, &slots, None, &pack, 0).unwrap().entry;
        assert_eq!(e.base_class_id, 18);
    }

    #[test]
    fn deletion_timers_and_expired_characters() {
        let pack = datapack();
        let slots = [(0, 0, 0); paperdoll::TOTAL_SLOTS];
        let mut r = row("Leaving", 1);
        r.deletetime = 10_000;
        let info = build_character(&r, None, &slots, None, &pack, 4_000).unwrap();
        assert_eq!(info.entry.delete_seconds, 6);
        assert!(build_character(&r, None, &slots, None, &pack, 10_001).is_none());
        let mut banned = row("Banned", 2);
        banned.accesslevel = Some(-1);
        assert_eq!(
            build_character(&banned, None, &slots, None, &pack, 0)
                .unwrap()
                .entry
                .delete_seconds,
            -1
        );
    }

    #[test]
    fn paperdoll_rows_map_to_slots_and_ignore_bad_ones() {
        let pack = datapack();
        let rows = vec![
            PaperdollItemRow {
                object_id: 10,
                item_id: Some(57),
                loc_data: Some(paperdoll::RHAND as i32),
                enchant_level: Some(3),
            },
            PaperdollItemRow {
                object_id: 11,
                item_id: Some(58),
                loc_data: Some(29), // a Java-only slot above the packet's slots
                enchant_level: None,
            },
            PaperdollItemRow {
                object_id: 12,
                item_id: Some(59),
                loc_data: None,
                enchant_level: None,
            },
        ];
        let slots = visible_paperdoll(&rows, &pack);
        // Unknown template (empty datapack): the item id is shown.
        assert_eq!(slots[paperdoll::RHAND], (10, 57, 3));
        assert_eq!(slots.iter().filter(|s| s.0 != 0).count(), 1);
    }
}
