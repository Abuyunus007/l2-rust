//! Class skill trees, reduced to what character creation needs: skill id, skill level, the level a
//! skill is learned at and its auto-learn flags. Skill definitions and effects are not loaded here.
//! The clan trees (`pledgeSkillTree`, `subPledgeSkillTree`) are kept too (M6-CLAN-03), and the
//! fishing tree (`fishingSkillTree`, M6-FU-10) and the certification tree (`subClassSkillTree`,
//! S-VM-01).
//!
//! Java: `gameserver/data/xml/SkillTreeData.java` (`classSkillTree` parsing, the parent class chain,
//! `getCompleteClassSkillTree`, `getAvailableSkills`) and `network/clientpackets/CharacterCreate.java`
//! (which skills a new character gets).
//! Data: `data/stats/players/skillTrees/**/*.xml` (`xsd/skillTrees.xsd`).
//!
//! Java keys every skill by `skillId * 1021 + skillLevel`. This table keys by `(skill_id, skill_level)`,
//! which orders and identifies the same skills as long as no skill level reaches 1021.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::class_list::{ClassId, Race};
use crate::loader::{DataError, load_dir, log_loaded};

/// Java `CommonSkill.DIVINE_INSPIRATION`. Auto-learn skips it unless `AutoLearnDivineInspiration`
/// is enabled in `config/Player.ini` (default `False`).
pub const DIVINE_INSPIRATION_ID: i32 = 1405;

/// An item a skill costs when it is learned. Java `ItemHolder` in `SkillLearn.getRequiredItems`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillLearnItem {
    pub item_id: i32,
    /// Java `long getCount()`.
    pub count: i64,
}

/// One line of a skill tree. Java `mechanics/skill/holders/SkillLearn`, without the social class,
/// residence and sub-class parts (clan and sub-class trees are not loaded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLearn {
    /// Java `getSkillId()`.
    pub skill_id: i32,
    /// Java `getSkillLevel()`.
    pub skill_level: i32,
    /// Java `getGetLevel()`: the character level the skill is available from.
    pub get_level: i32,
    /// Java `isAutoGet()`: delivered automatically.
    pub auto_get: bool,
    /// Java `isLearnedByNpc()`.
    pub learned_by_npc: bool,
    /// Java `isLearnedByFS()`: learned with a Forgotten Scroll.
    pub learned_by_fs: bool,
    /// Java `getLevelUpSp()`: the SP the skill costs (clan reputation in the pledge trees).
    pub level_up_sp: i32,
    /// Java `getRequiredItems()`, in file order. Java keeps them in a `HashSet`, so its order is
    /// arbitrary; no shipped skill has more than one.
    pub required_items: Vec<SkillLearnItem>,
    /// Java `getPreReqSkills()`: `(skill id, skill level)` the player must know.
    pub pre_req_skills: Vec<(i32, i32)>,
    /// Java `getRaces()`: empty means every race.
    pub races: Vec<Race>,
    /// Java `getSocialClass()`: the pledge class a clan member needs for the clan skill (the
    /// first `socialClass` element of the line). `None` for the lines of the class trees.
    pub social_class: Option<SocialClass>,
    /// Java `isResidencialSkill()` (`residenceSkill`): a castle or fortress skill.
    pub residence_skill: bool,
    /// Java `getResidenceIds()`.
    pub residence_ids: Vec<i32>,
    /// Java `getSubClassConditions()`: `(slot, lvl)` of each `subClassConditions` element of a
    /// sub-class tree line, in file order (S-VM-01).
    pub sub_class_conditions: Vec<(i32, i32)>,
}

/// Java `SocialClass`: the pledge classes a clan skill asks for. The order is the Java ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SocialClass {
    Vagabond,
    Vassal,
    Apprentice,
    Heir,
    Knight,
    Elder,
    Baron,
    Viscount,
    Count,
    Marquis,
    Duke,
    GrandDuke,
    DistinguishedKing,
    Emperor,
}

impl SocialClass {
    /// Java `Enum.valueOf(SocialClass.class, name)`: `None` where Java throws.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim() {
            "VAGABOND" => Self::Vagabond,
            "VASSAL" => Self::Vassal,
            "APPRENTICE" => Self::Apprentice,
            "HEIR" => Self::Heir,
            "KNIGHT" => Self::Knight,
            "ELDER" => Self::Elder,
            "BARON" => Self::Baron,
            "VISCOUNT" => Self::Viscount,
            "COUNT" => Self::Count,
            "MARQUIS" => Self::Marquis,
            "DUKE" => Self::Duke,
            "GRAND_DUKE" => Self::GrandDuke,
            "DISTINGUISHED_KING" => Self::DistinguishedKing,
            "EMPEROR" => Self::Emperor,
            _ => return None,
        })
    }

    /// Java `ordinal()`.
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

/// All class skill trees, immutable after loading. Java `SkillTreeData`, class part only.
#[derive(Debug, Default)]
pub struct SkillTrees {
    /// Java `_completeClassSkillTree`: for every class that has its own tree, the common skills plus
    /// the skills of the class and its ancestors (a skill of the same id and level defined lower in
    /// the chain replaces the one above it).
    complete: BTreeMap<ClassId, BTreeMap<(i32, i32), SkillLearn>>,
    /// The keys of `complete` in the order Java's `LinkedHashMap` lists them (see
    /// [`SkillTrees::complete_class_tree_java_order`]).
    java_order: BTreeMap<ClassId, Vec<(i32, i32)>>,
    /// Java `_pledgeSkillTree`, in the order Java's `ConcurrentHashMap` lists it.
    pledge: Vec<SkillLearn>,
    /// Java `_subPledgeSkillTree`, in the order Java's `ConcurrentHashMap` lists it.
    sub_pledge: Vec<SkillLearn>,
    /// Java `_fishingSkillTree`, in the order Java's `ConcurrentHashMap` lists it.
    fishing: Vec<SkillLearn>,
    /// Java `_nobleSkillTree`, in the order Java's `ConcurrentHashMap` lists it.
    noble: Vec<SkillLearn>,
    /// Java `_heroSkillTree`, in the order Java's `ConcurrentHashMap` lists it.
    hero: Vec<SkillLearn>,
    /// Java `_collectSkillTree` (Gracia, the collect skills Tolonis teaches), in the order Java's
    /// `ConcurrentHashMap` lists it.
    collect: Vec<SkillLearn>,
    /// Java `_gameMasterSkillTree` and `_gameMasterAuraSkillTree` (`//switch_gm_buffs`), in the
    /// order of Java's `ConcurrentHashMap`.
    game_master: Vec<SkillLearn>,
    game_master_aura: Vec<SkillLearn>,
    /// Java `_subClassSkillTree` (the certification skills), in the order of Java's
    /// `ConcurrentHashMap`.
    sub_class: Vec<SkillLearn>,
    /// Java `_transformSkillTree` (AvantGarde's transformation skills), in the order of Java's
    /// `ConcurrentHashMap`.
    transform: Vec<SkillLearn>,
    /// Java `_transferSkillTrees`: the transfer skills (the healers' pomander skills) of each class
    /// that has a tree, every tree in the order of Java's `HashMap`.
    transfer: BTreeMap<ClassId, Vec<SkillLearn>>,
}

impl SkillTrees {
    const LOADER: &'static str = "SkillTreeData";

    /// Loads every file below `<root>/data/stats/players/skillTrees` and logs Java's count line
    /// (`Loaded N Class Skills for M class skill trees.`).
    pub fn load(root: &Path) -> Self {
        let mut gathered = Gathered::default();
        load_dir(
            &root.join("data/stats/players/skillTrees"),
            true,
            |doc: RawSkillTreeFile| {
                gathered.add(doc);
                Ok(())
            },
        );
        log_loaded(
            Self::LOADER,
            gathered.skill_count(),
            &format!("Class Skills for {} class skill trees", gathered.own.len()),
        );
        let residential = gathered.pledge.values().filter(|l| l.residence_skill).count();
        log_loaded(
            Self::LOADER,
            gathered.pledge.len(),
            &format!(
                "clan skills, {} for clan and {residential} residential",
                gathered.pledge.len() - residential
            ),
        );
        log_loaded(Self::LOADER, gathered.sub_pledge.len(), "sub-pledge skills");
        log_loaded(Self::LOADER, gathered.sub_class.len(), "sub-class skills");
        log_loaded(Self::LOADER, gathered.transform.len(), "transform skills");
        log_loaded(
            Self::LOADER,
            gathered.transfer.values().map(|(tree, _)| tree.len()).sum(),
            &format!(
                "transfer skills for {} transfer skill trees",
                gathered.transfer.len()
            ),
        );
        let dwarven_only = gathered
            .fishing
            .values()
            .filter(|l| l.races.contains(&Race::Dwarf))
            .count();
        log_loaded(
            Self::LOADER,
            gathered.fishing.len(),
            &format!("fishing skills, {dwarven_only} Dwarven only fishing skills"),
        );
        Self::build(gathered)
    }

    /// Builds the tables from the text of one skill tree file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut gathered = Gathered::default();
        gathered.add(doc);
        Ok(Self::build(gathered))
    }

    fn build(gathered: Gathered) -> Self {
        let Gathered {
            parents,
            common,
            common_order,
            own,
            own_order,
            pledge,
            pledge_order,
            sub_pledge,
            sub_pledge_order,
            fishing,
            fishing_order,
            noble,
            noble_order,
            hero,
            hero_order,
            collect,
            collect_order,
            game_master,
            game_master_order,
            game_master_aura,
            game_master_aura_order,
            sub_class,
            sub_class_order,
            transform,
            transform_order,
            transfer,
        } = gathered;
        let mut complete = BTreeMap::new();
        let mut java_order = BTreeMap::new();
        for &class in own.keys() {
            let mut tree = common.clone();
            // `skillTree.putAll(_commonSkillTree)`, then `putAll(_classSkillTrees.get(cid))` for the
            // class chain, root first. A key that is already in the map keeps its place.
            let mut order: Vec<(i32, i32)> = Vec::new();
            let place = |keys: &[(i32, i32)], order: &mut Vec<(i32, i32)>| {
                for key in java_hash_map_order(keys) {
                    if !order.contains(&key) {
                        order.push(key);
                    }
                }
            };
            place(&common_order, &mut order);
            for ancestor in class_chain(&parents, class) {
                if let Some(own_tree) = own.get(&ancestor) {
                    tree.extend(own_tree.clone());
                    if let Some(keys) = own_order.get(&ancestor) {
                        place(keys, &mut order);
                    }
                }
            }
            complete.insert(class, tree);
            java_order.insert(class, order);
        }
        let in_java_order = |mut tree: BTreeMap<(i32, i32), SkillLearn>, order: Vec<(i32, i32)>| {
            java_concurrent_map_order(&order)
                .into_iter()
                .filter_map(|key| tree.remove(&key))
                .collect()
        };
        Self {
            complete,
            java_order,
            pledge: in_java_order(pledge, pledge_order),
            sub_pledge: in_java_order(sub_pledge, sub_pledge_order),
            fishing: in_java_order(fishing, fishing_order),
            noble: in_java_order(noble, noble_order),
            hero: in_java_order(hero, hero_order),
            collect: in_java_order(collect, collect_order),
            game_master: in_java_order(game_master, game_master_order),
            game_master_aura: in_java_order(game_master_aura, game_master_aura_order),
            sub_class: in_java_order(sub_class, sub_class_order),
            transform: in_java_order(transform, transform_order),
            transfer: transfer
                .into_iter()
                .map(|(class, (mut tree, order))| {
                    let lines = java_hash_map_order(&order)
                        .into_iter()
                        .filter_map(|key| tree.remove(&key))
                        .collect();
                    (class, lines)
                })
                .collect(),
        }
    }

    /// [`Self::complete_class_tree`] in the order Java lists it: the iteration order of the
    /// `LinkedHashMap` that `getCompleteClassSkillTree` returns, which `getAvailableSkills` follows
    /// (so the skill lists a trainer shows). Each class tree is a `HashMap` keyed by
    /// `skillId * 1021 + skillLevel`, so the order looks shuffled; it is the same on every run.
    pub fn complete_class_tree_java_order(&self, class: ClassId) -> Vec<&SkillLearn> {
        let (Some(tree), Some(order)) = (self.complete.get(&class), self.java_order.get(&class)) else {
            return Vec::new();
        };
        order.iter().filter_map(|key| tree.get(key)).collect()
    }

    /// Java `getCompleteClassSkillTree(playerClass)`: every skill the class can have, in id and
    /// level order. Empty for a class without its own tree.
    pub fn complete_class_tree(&self, class: ClassId) -> impl Iterator<Item = &SkillLearn> + '_ {
        self.complete
            .get(&class)
            .into_iter()
            .flat_map(|tree| tree.values())
    }

    /// The skills a new character of `class` at `level` gets automatically. Java
    /// `SkillTreeData.getAvailableSkills(newChar, class, includeByFs = false, includeAutoGet = true)`
    /// as `CharacterCreate` calls it, for a character that knows no skill yet: the skill must be
    /// auto-get or learned by NPC, the level must reach its `get_level`, and only level 1 of each
    /// skill qualifies. Java does not check race here, so this does not either.
    ///
    /// `auto_learn_divine_inspiration` is the `AutoLearnDivineInspiration` setting.
    pub fn auto_get_skills(
        &self,
        class: ClassId,
        level: i32,
        auto_learn_divine_inspiration: bool,
    ) -> Vec<&SkillLearn> {
        self.complete_class_tree(class)
            .filter(|skill| auto_learn_divine_inspiration || skill.skill_id != DIVINE_INSPIRATION_ID)
            .filter(|skill| skill.auto_get || skill.learned_by_npc)
            .filter(|skill| level >= skill.get_level)
            .filter(|skill| skill.skill_level == 1)
            .collect()
    }

    /// Java `getClassSkill(id, level, playerClass)`: the line of the complete tree of `class`.
    pub fn class_skill(&self, class: ClassId, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.complete.get(&class)?.get(&(skill_id, skill_level))
    }

    /// Java `getPledgeSkillTree().values()`: the clan skills (and the castle and fortress ones),
    /// in the order of Java's `ConcurrentHashMap` (`getAvailablePledgeSkills` follows it, so the
    /// list the clan trainer shows).
    pub fn pledge_skills(&self) -> &[SkillLearn] {
        &self.pledge
    }

    /// Java `getSubPledgeSkillTree().values()`: the squad skills, in Java's map order.
    pub fn sub_pledge_skills(&self) -> &[SkillLearn] {
        &self.sub_pledge
    }

    /// Java `getCollectSkillTree().values()`.
    pub fn collect_skills(&self) -> &[SkillLearn] {
        &self.collect
    }

    /// Java `getCollectSkill(id, level)`.
    pub fn collect_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.collect
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `getNobleSkillTree().values()`: the skills `Player.setNoble` gives and takes.
    pub fn noble_skills(&self) -> &[SkillLearn] {
        &self.noble
    }

    /// Java `getHeroSkillTree().values()`: the skills `Player.setHero` gives and takes.
    pub fn hero_skills(&self) -> &[SkillLearn] {
        &self.hero
    }

    /// Java `isHeroSkill(skillId, skillLevel)`: the skill (a level of -1 matches any level) is in
    /// the hero tree.
    pub fn is_hero_skill(&self, skill_id: i32, skill_level: i32) -> bool {
        self.hero
            .iter()
            .any(|l| l.skill_id == skill_id && (skill_level == -1 || l.skill_level == skill_level))
    }

    /// Java `getFishingSkillTree().values()`: the fishing skills, in the order of Java's
    /// `ConcurrentHashMap` (`getAvailableFishingSkills` follows it, so the list the fisherman
    /// shows).
    /// Java `getGMSkillTree().values()` source: the Game Master skills (`gameMasterSkillTree`).
    pub fn game_master_skills(&self) -> &[SkillLearn] {
        &self.game_master
    }

    /// Java `getGMAuraSkillTree().values()` source: the Game Master aura skills.
    pub fn game_master_aura_skills(&self) -> &[SkillLearn] {
        &self.game_master_aura
    }

    pub fn fishing_skills(&self) -> &[SkillLearn] {
        &self.fishing
    }

    /// Java `getFishingSkill(id, level)`.
    pub fn fishing_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.fishing
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `getPledgeSkill(id, level)`.
    pub fn pledge_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.pledge
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `getSubPledgeSkill(id, level)`.
    pub fn sub_pledge_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.sub_pledge
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `isClanSkill(id, level)`: the line is in the clan or the squad tree.
    pub fn is_clan_skill(&self, skill_id: i32, skill_level: i32) -> bool {
        self.pledge_skill(skill_id, skill_level).is_some()
            || self.sub_pledge_skill(skill_id, skill_level).is_some()
    }

    /// Number of classes that have a complete tree.
    pub fn len(&self) -> usize {
        self.complete.len()
    }

    /// Java `getSubClassSkillTree().values()`: the certification skills, in the order of Java's
    /// `ConcurrentHashMap`.
    pub fn sub_class_skills(&self) -> &[SkillLearn] {
        &self.sub_class
    }

    /// Java `getSubClassSkill(id, lvl)`.
    pub fn sub_class_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.sub_class
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `getTransformSkillTree().values()`: the transformation skills, in the order of Java's
    /// `ConcurrentHashMap`.
    pub fn transform_skills(&self) -> &[SkillLearn] {
        &self.transform
    }

    /// Java `getTransformSkill(id, lvl)`.
    pub fn transform_skill(&self, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.transform
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `getTransferSkillTree(playerClass).values()`: the transfer tree of the class, or of its
    /// parent from the third class on ("if new classes are implemented over 3rd class, we use a
    /// recursive call"). Java throws a `NullPointerException` on `.values()` of a class without a
    /// tree; this returns an empty slice.
    pub fn transfer_skill_tree(&self, class: ClassId) -> &[SkillLearn] {
        let mut class = class;
        while class.class_level() >= 3 {
            let Some(parent) = class.enum_parent() else {
                break;
            };
            class = parent;
        }
        self.transfer.get(&class).map_or(&[], Vec::as_slice)
    }

    /// The tree `getAvailableTransferSkills` walks: the class's own, or its parent's for a third
    /// class (one step only, "a different way should be implemented" over the third class).
    pub fn available_transfer_tree(&self, class: ClassId) -> &[SkillLearn] {
        let class = if class.class_level() == 3 {
            class.enum_parent().unwrap_or(class)
        } else {
            class
        };
        self.transfer.get(&class).map_or(&[], Vec::as_slice)
    }

    /// Java `getTransferSkill(id, lvl, playerClass)`: the line in the transfer tree of the
    /// parent of `class` (the trees belong to the third class's parent).
    pub fn transfer_skill(&self, skill_id: i32, skill_level: i32, class: ClassId) -> Option<&SkillLearn> {
        let parent = class.enum_parent()?;
        self.transfer
            .get(&parent)?
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// True when no class tree was loaded.
    pub fn is_empty(&self) -> bool {
        self.complete.is_empty()
    }
}

/// The order Java's `HashMap<Integer, _>` iterates keys that were put in the order `keys` (all
/// different) with `SkillData.getSkillHashCode` as the key: by bucket `(h ^ (h >>> 16)) & (cap - 1)`
/// of the final table size, then by insertion inside a bucket (a resize splits a bucket without
/// reordering it).
// NOTE: a bucket of 8 or more keys in a table of 64 or more becomes a tree in Java, which can change
// the order inside that bucket; the shipped trees have no bucket that full.
fn java_hash_map_order(keys: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut capacity: usize = 16;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |&(skill_id, skill_level): &(i32, i32)| {
        let h = skill_id.wrapping_mul(1021).wrapping_add(skill_level) as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize, (i32, i32))> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (bucket(key), index, *key))
        .collect();
    indexed.sort_unstable();
    indexed.into_iter().map(|(_, _, key)| key).collect()
}

/// The order Java's `ConcurrentHashMap<Integer, _>` iterates keys that were put in the order
/// `keys` (all different) with `SkillData.getSkillHashCode` (`id * 1021 + level`) as the key.
///
/// The table starts with 16 bins and doubles when a put brings the count to 3/4 of the table
/// (`sizeCtl`). A bin keeps its entries in put order, but a resize splits it the way
/// `ConcurrentHashMap.transfer` does: the entries after the last change of side keep their order,
/// the ones before are put in front one by one, which reverses them.
// NOTE: a bin of 8 or more entries in a table below 64 bins makes Java grow the table at once
// (`treeifyBin`); the shipped clan trees do not have such a bin.
fn java_concurrent_map_order(keys: &[(i32, i32)]) -> Vec<(i32, i32)> {
    type Entry = ((i32, i32), u32);
    let spread = |&(skill_id, skill_level): &(i32, i32)| {
        let h = skill_id.wrapping_mul(1021).wrapping_add(skill_level) as u32;
        // `ConcurrentHashMap.spread`: `(h ^ (h >>> 16)) & HASH_BITS`.
        (h ^ (h >> 16)) & 0x7fff_ffff
    };
    let mut table: Vec<Vec<Entry>> = vec![Vec::new(); 16];
    let mut size_ctl = 12;
    for (count, key) in keys.iter().enumerate() {
        let hash = spread(key);
        let bin = (hash as usize) & (table.len() - 1);
        table[bin].push((*key, hash));
        if count + 1 < size_ctl {
            continue;
        }
        // `transfer` into a table twice as big.
        let n = table.len();
        let high = |hash: u32| (hash as usize) & n != 0;
        let mut grown: Vec<Vec<Entry>> = vec![Vec::new(); n * 2];
        for (i, entries) in table.iter().enumerate() {
            let Some(first) = entries.first() else {
                continue;
            };
            let mut run_bit = high(first.1);
            let mut last_run = 0;
            for (j, entry) in entries.iter().enumerate().skip(1) {
                if high(entry.1) != run_bit {
                    run_bit = high(entry.1);
                    last_run = j;
                }
            }
            let run = entries[last_run..].to_vec();
            let (mut low_list, mut high_list) = if run_bit {
                (Vec::new(), run)
            } else {
                (run, Vec::new())
            };
            for entry in &entries[..last_run] {
                if high(entry.1) {
                    high_list.insert(0, *entry);
                } else {
                    low_list.insert(0, *entry);
                }
            }
            grown[i] = low_list;
            grown[i + n] = high_list;
        }
        table = grown;
        size_ctl = table.len() - table.len() / 4;
    }
    table.into_iter().flatten().map(|(key, _)| key).collect()
}

/// The class and its ancestors, root first. Java `getCompleteClassSkillTree` walks the same chain.
fn class_chain(parents: &BTreeMap<ClassId, Option<ClassId>>, class: ClassId) -> Vec<ClassId> {
    let mut chain = Vec::new();
    let mut current = Some(class);
    while let Some(c) = current {
        if chain.contains(&c) {
            // NOTE(java-quirk): Java loops forever on a parent cycle. The data has none; stop instead.
            warn!("SkillTreeData: parent class cycle at class {}.", c.0);
            break;
        }
        chain.push(c);
        current = parents.get(&c).copied().flatten();
    }
    chain.reverse();
    chain
}

/// Java `PlayerClass.getPlayerClass(id)`: `None` for ids that are not player classes.
fn player_class(id: i32) -> Option<ClassId> {
    u16::try_from(id)
        .ok()
        .map(ClassId)
        .filter(|class| class.race().is_some())
}

/// One tree with the keys in the order they were first put.
type GatheredTree = (BTreeMap<(i32, i32), SkillLearn>, Vec<(i32, i32)>);

/// What the files say, before the complete trees are built.
#[derive(Default)]
struct Gathered {
    /// Java `_parentClassMap`: the first parent a class is given wins. `None` when the parent id
    /// is not a player class.
    parents: BTreeMap<ClassId, Option<ClassId>>,
    /// Java `_commonSkillTree`: `classSkillTree` entries without a `classId`.
    common: BTreeMap<(i32, i32), SkillLearn>,
    /// The keys of `common` in the order they were first added.
    common_order: Vec<(i32, i32)>,
    /// Java `_classSkillTrees`: the own tree of every class.
    own: BTreeMap<ClassId, BTreeMap<(i32, i32), SkillLearn>>,
    /// The keys of each `own` tree in the order they were first added (the `HashMap`'s
    /// insertion order, which decides the order inside one bucket).
    own_order: BTreeMap<ClassId, Vec<(i32, i32)>>,
    /// Java `_pledgeSkillTree` and the keys in the order they were first put.
    pledge: BTreeMap<(i32, i32), SkillLearn>,
    pledge_order: Vec<(i32, i32)>,
    /// Java `_subPledgeSkillTree` and the keys in the order they were first put.
    sub_pledge: BTreeMap<(i32, i32), SkillLearn>,
    sub_pledge_order: Vec<(i32, i32)>,
    /// Java `_fishingSkillTree` and the keys in the order they were first put.
    fishing: BTreeMap<(i32, i32), SkillLearn>,
    fishing_order: Vec<(i32, i32)>,
    /// Java `_nobleSkillTree` and the keys in the order they were first put.
    noble: BTreeMap<(i32, i32), SkillLearn>,
    noble_order: Vec<(i32, i32)>,
    /// Java `_heroSkillTree` and the keys in the order they were first put.
    hero: BTreeMap<(i32, i32), SkillLearn>,
    hero_order: Vec<(i32, i32)>,
    /// Java `_collectSkillTree` and the keys in the order they were first put.
    collect: BTreeMap<(i32, i32), SkillLearn>,
    collect_order: Vec<(i32, i32)>,
    /// Java `_gameMasterSkillTree` and `_gameMasterAuraSkillTree`, with their keys in put order.
    game_master: BTreeMap<(i32, i32), SkillLearn>,
    game_master_order: Vec<(i32, i32)>,
    game_master_aura: BTreeMap<(i32, i32), SkillLearn>,
    game_master_aura_order: Vec<(i32, i32)>,
    /// Java `_subClassSkillTree` and the keys in the order they were first put.
    sub_class: BTreeMap<(i32, i32), SkillLearn>,
    sub_class_order: Vec<(i32, i32)>,
    /// Java `_transformSkillTree` and the keys in the order they were first put.
    transform: BTreeMap<(i32, i32), SkillLearn>,
    transform_order: Vec<(i32, i32)>,
    /// Java `_transferSkillTrees`: a tree element replaces the tree of its class.
    transfer: BTreeMap<ClassId, GatheredTree>,
}

impl Gathered {
    fn add(&mut self, doc: RawSkillTreeFile) {
        for tree in doc.trees {
            let class_id = tree.class_id.unwrap_or(-1);
            // Java reads the parent from every skill tree element that has both ids, whatever its type.
            if let Some(parent_id) = tree.parent_class_id {
                if class_id > -1 && parent_id > -1 && class_id != parent_id {
                    if let Some(class) = player_class(class_id) {
                        self.parents
                            .entry(class)
                            .or_insert_with(|| player_class(parent_id));
                    }
                }
            }
            if tree.kind == "transferSkillTree" {
                let Some(class) = player_class(class_id) else {
                    warn!("SkillTreeData: transfer skill tree for unknown class id {class_id} skipped.");
                    continue;
                };
                let mut map = BTreeMap::new();
                let mut order = Vec::new();
                for skill in tree.skills {
                    let learn = skill.into_learn();
                    let key = (learn.skill_id, learn.skill_level);
                    if map.insert(key, learn).is_none() {
                        order.push(key);
                    }
                }
                self.transfer.insert(class, (map, order));
                continue;
            }
            // The clan trees are kept for the clan trainers (M6-CLAN-03), the fishing tree for the
            // fishermen (M6-FU-10).
            if matches!(
                tree.kind.as_str(),
                "pledgeSkillTree"
                    | "subPledgeSkillTree"
                    | "fishingSkillTree"
                    | "nobleSkillTree"
                    | "heroSkillTree"
                    | "collectSkillTree"
                    | "gameMasterSkillTree"
                    | "gameMasterAuraSkillTree"
                    | "subClassSkillTree"
                    | "transformSkillTree"
            ) {
                let (map, order) = match tree.kind.as_str() {
                    "pledgeSkillTree" => (&mut self.pledge, &mut self.pledge_order),
                    "subPledgeSkillTree" => (&mut self.sub_pledge, &mut self.sub_pledge_order),
                    "nobleSkillTree" => (&mut self.noble, &mut self.noble_order),
                    "heroSkillTree" => (&mut self.hero, &mut self.hero_order),
                    "collectSkillTree" => (&mut self.collect, &mut self.collect_order),
                    "gameMasterSkillTree" => (&mut self.game_master, &mut self.game_master_order),
                    "gameMasterAuraSkillTree" => {
                        (&mut self.game_master_aura, &mut self.game_master_aura_order)
                    }
                    "subClassSkillTree" => (&mut self.sub_class, &mut self.sub_class_order),
                    "transformSkillTree" => (&mut self.transform, &mut self.transform_order),
                    _ => (&mut self.fishing, &mut self.fishing_order),
                };
                for skill in tree.skills {
                    let learn = skill.into_learn();
                    let key = (learn.skill_id, learn.skill_level);
                    if map.insert(key, learn).is_none() {
                        order.push(key);
                    }
                }
                continue;
            }
            // Only the trees above and the class trees are kept.
            if tree.kind != "classSkillTree" {
                continue;
            }
            if class_id <= -1 {
                for skill in tree.skills {
                    let learn = skill.into_learn();
                    let key = (learn.skill_id, learn.skill_level);
                    if self.common.insert(key, learn).is_none() {
                        self.common_order.push(key);
                    }
                }
                continue;
            }
            let Some(class) = player_class(class_id) else {
                warn!("SkillTreeData: skill tree for unknown class id {class_id} skipped.");
                continue;
            };
            let own = self.own.entry(class).or_default();
            let order = self.own_order.entry(class).or_default();
            for skill in tree.skills {
                let learn = skill.into_learn();
                let key = (learn.skill_id, learn.skill_level);
                if own.insert(key, learn).is_none() {
                    order.push(key);
                }
            }
        }
    }

    /// Java `getClassSkillTree` count: skills in all own trees.
    fn skill_count(&self) -> usize {
        self.own.values().map(BTreeMap::len).sum()
    }
}

// ---- Raw XML shape (xsd/skillTrees.xsd) ----------------------------------------------------
// Attributes without a default are the ones Java reads without one (a missing value is an error).

#[derive(Deserialize)]
struct RawSkillTreeFile {
    #[serde(rename = "skillTree", default)]
    trees: Vec<RawSkillTree>,
}

#[derive(Deserialize)]
struct RawSkillTree {
    #[serde(rename = "@type")]
    kind: String,
    #[serde(rename = "@classId")]
    class_id: Option<i32>,
    #[serde(rename = "@parentClassId")]
    parent_class_id: Option<i32>,
    #[serde(rename = "skill", default)]
    skills: Vec<RawSkill>,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@skillId")]
    skill_id: i32,
    #[serde(rename = "@skillLevel")]
    skill_level: i32,
    #[serde(rename = "@getLevel")]
    get_level: i32,
    #[serde(rename = "@autoGet", default)]
    auto_get: bool,
    #[serde(rename = "@learnedByNpc", default)]
    learned_by_npc: bool,
    #[serde(rename = "@learnedByFS", default)]
    learned_by_fs: bool,
    #[serde(rename = "@levelUpSp", default)]
    level_up_sp: i32,
    #[serde(rename = "item", default)]
    items: Vec<RawItem>,
    #[serde(rename = "preRequisiteSkill", default)]
    pre_req_skills: Vec<RawPreReq>,
    #[serde(rename = "race", default)]
    races: Vec<String>,
    #[serde(rename = "@residenceSkill", default)]
    residence_skill: bool,
    #[serde(rename = "residenceId", default)]
    residence_ids: Vec<i32>,
    #[serde(rename = "socialClass", default)]
    social_classes: Vec<String>,
    #[serde(rename = "subClassConditions", default)]
    sub_class_conditions: Vec<RawSubClassCondition>,
}

#[derive(Deserialize)]
struct RawSubClassCondition {
    #[serde(rename = "@slot")]
    slot: i32,
    #[serde(rename = "@lvl")]
    lvl: i32,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@count")]
    count: i64,
}

#[derive(Deserialize)]
struct RawPreReq {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "@lvl")]
    level: i32,
}

/// Java `Race.valueOf(text)` for the playable races. `None` for a name Java would throw on.
fn race_from_name(name: &str) -> Option<Race> {
    Some(match name.trim() {
        "HUMAN" => Race::Human,
        "ELF" => Race::Elf,
        "DARK_ELF" => Race::DarkElf,
        "ORC" => Race::Orc,
        "DWARF" => Race::Dwarf,
        "KAMAEL" => Race::Kamael,
        _ => return None,
    })
}

impl RawSkill {
    fn into_learn(self) -> SkillLearn {
        SkillLearn {
            skill_id: self.skill_id,
            skill_level: self.skill_level,
            get_level: self.get_level,
            auto_get: self.auto_get,
            learned_by_npc: self.learned_by_npc,
            learned_by_fs: self.learned_by_fs,
            level_up_sp: self.level_up_sp,
            required_items: self
                .items
                .into_iter()
                .map(|item| SkillLearnItem {
                    item_id: item.id,
                    count: item.count,
                })
                .collect(),
            pre_req_skills: self.pre_req_skills.into_iter().map(|r| (r.id, r.level)).collect(),
            races: self
                .races
                .iter()
                .filter_map(|name| {
                    let race = race_from_name(name);
                    if race.is_none() {
                        warn!(
                            "SkillTreeData: unknown race {name:?} on skill {} skipped.",
                            self.skill_id
                        );
                    }
                    race
                })
                .collect(),
            // `setSocialClass` keeps the first one that is set.
            social_class: self.social_classes.iter().find_map(|name| {
                let class = SocialClass::from_name(name);
                if class.is_none() {
                    warn!(
                        "SkillTreeData: unknown social class {name:?} on skill {} skipped.",
                        self.skill_id
                    );
                }
                class
            }),
            residence_skill: self.residence_skill,
            residence_ids: self.residence_ids,
            sub_class_conditions: self
                .sub_class_conditions
                .into_iter()
                .map(|c| (c.slot, c.lvl))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../../../../xsd/skillTrees.xsd">
	<skillTree type="classSkillTree" classId="0">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Sword Mastery" skillId="205" skillLevel="1" getLevel="1" learnedByNpc="true" />
		<skill skillName="Expertise D" skillId="239" skillLevel="1" getLevel="20" autoGet="true" />
		<skill skillName="Plain" skillId="100" skillLevel="1" getLevel="1" />
		<skill skillName="Stun" skillId="100" skillLevel="2" getLevel="1" autoGet="true" />
		<skill skillName="Divine Inspiration" skillId="1405" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="1" parentClassId="0">
		<skill skillName="Warrior skill" skillId="300" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="4" parentClassId="0">
		<skill skillName="Knight skill" skillId="410" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="4" parentClassId="1">
		<skill skillName="Knight skill 2" skillId="411" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="transferSkillTree" classId="16">
		<skill skillName="Transfer" skillId="500" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
</list>"#;

    fn keys(skills: &[&SkillLearn]) -> Vec<(i32, i32)> {
        skills.iter().map(|s| (s.skill_id, s.skill_level)).collect()
    }

    #[test]
    fn the_transfer_tree_belongs_to_the_class_before_the_third_class() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        // Bishop (16) has the tree; Cardinal (97, third class) uses its parent's.
        assert_eq!(trees.transfer_skill_tree(ClassId(16)).len(), 1);
        assert_eq!(trees.transfer_skill_tree(ClassId(97)).len(), 1);
        assert_eq!(trees.available_transfer_tree(ClassId(97)).len(), 1);
        // The class of another branch has none (Java would throw).
        assert!(trees.transfer_skill_tree(ClassId(0)).is_empty());
        // `getTransferSkill` looks in the tree of the *parent* of the class.
        assert!(trees.transfer_skill(500, 1, ClassId(97)).is_some());
        assert!(trees.transfer_skill(500, 2, ClassId(97)).is_none());
        assert!(trees.transfer_skill(500, 1, ClassId(16)).is_none());
    }

    #[test]
    fn level_one_auto_get_and_npc_skills_are_given() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 1, false);
        // Lucky (auto-get) and Sword Mastery (learned by NPC). Expertise D needs level 20, the plain
        // skill has no flag, Stun level 2 needs level 1 of Stun first, Divine Inspiration is off.
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1)]);
    }

    #[test]
    fn get_level_gates_the_skill() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 20, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (239, 1)]);
    }

    #[test]
    fn divine_inspiration_follows_the_setting() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 1, true);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (1405, 1)]);
    }

    #[test]
    fn parent_skills_are_inherited() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(1), 1, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (300, 1)]);
    }

    #[test]
    fn first_parent_wins_and_parent_zero_counts() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        // Class 4 names parent 0 first, so class 1's skill 300 is not inherited.
        let skills = trees.auto_get_skills(ClassId(4), 1, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (410, 1), (411, 1)]);
    }

    #[test]
    fn class_without_own_tree_has_nothing() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        assert!(trees.auto_get_skills(ClassId(2), 1, false).is_empty());
        assert_eq!(trees.complete_class_tree(ClassId(2)).count(), 0);
    }

    #[test]
    fn transfer_trees_are_not_class_trees() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        assert!(trees.auto_get_skills(ClassId(16), 1, false).is_empty());
        assert_eq!(trees.len(), 3);
    }

    #[test]
    fn common_skills_reach_every_class_tree() {
        let xml = r#"<list>
			<skillTree type="classSkillTree"><skill skillId="7" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="0"><skill skillId="8" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="1" parentClassId="0"></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(0), 1, false)),
            vec![(7, 1), (8, 1)]
        );
        // A class with an empty own tree still counts and inherits.
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(1), 1, false)),
            vec![(7, 1), (8, 1)]
        );
    }

    #[test]
    fn parent_cycle_stops_instead_of_hanging() {
        let xml = r#"<list>
			<skillTree type="classSkillTree" classId="2" parentClassId="3"><skill skillId="20" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="3" parentClassId="2"><skill skillId="30" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(2), 1, false)),
            vec![(20, 1), (30, 1)]
        );
    }

    #[test]
    fn unknown_class_id_is_skipped() {
        let xml = r#"<list>
			<skillTree type="classSkillTree" classId="58"><skill skillId="9" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert!(trees.is_empty());
    }

    #[test]
    fn learn_costs_items_and_requirements_are_read() {
        let xml = r#"<list><skillTree type="classSkillTree" classId="0">
			<skill skillName="Book" skillId="1405" skillLevel="1" getLevel="52" levelUpSp="9000" learnedByNpc="true">
				<preRequisiteSkill id="10" lvl="2" />
				<item id="8618" count="1" />
				<item id="57" count="150" />
			</skill>
			<skill skillName="FS" skillId="818" skillLevel="1" getLevel="74" learnedByFS="true">
				<race>DWARF</race>
				<race>NOPE</race>
			</skill>
		</skillTree></list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        let book = trees.class_skill(ClassId(0), 1405, 1).unwrap();
        assert_eq!(book.level_up_sp, 9000);
        assert!(book.learned_by_npc && !book.learned_by_fs);
        assert_eq!(book.pre_req_skills, vec![(10, 2)]);
        assert_eq!(
            book.required_items,
            vec![
                SkillLearnItem {
                    item_id: 8618,
                    count: 1
                },
                SkillLearnItem {
                    item_id: 57,
                    count: 150
                }
            ]
        );
        let fs = trees.class_skill(ClassId(0), 818, 1).unwrap();
        assert!(fs.learned_by_fs);
        assert_eq!(fs.level_up_sp, 0);
        // An unknown race name is skipped (Java would throw).
        assert_eq!(fs.races, vec![Race::Dwarf]);
        assert!(trees.class_skill(ClassId(0), 818, 2).is_none());
        assert!(trees.class_skill(ClassId(1), 818, 1).is_none());
    }

    #[test]
    fn java_hash_map_order_is_bucket_then_insertion() {
        // Worked out by hand: bucket = (h ^ (h >>> 16)) & 15 with h = id * 1021 + level, so
        // (194,1) -> 8, (205,1) -> 9, (239,1) -> 7, (100,1) -> 4, (100,2) -> 7, (1405,1) -> 15,
        // (300,1) -> 9, (410,1) -> 5, (411,1) -> 6. Nine keys fit the 16 buckets of a new HashMap.
        let keys = [
            (194, 1),
            (205, 1),
            (239, 1),
            (100, 1),
            (100, 2),
            (1405, 1),
            (300, 1),
            (410, 1),
            (411, 1),
        ];
        assert_eq!(
            java_hash_map_order(&keys),
            vec![
                (100, 1),
                (410, 1),
                (411, 1),
                (239, 1),
                (100, 2),
                (194, 1),
                (205, 1),
                (300, 1),
                (1405, 1)
            ]
        );
    }

    #[test]
    fn complete_tree_in_java_order_lists_each_class_tree_in_hash_order_root_first() {
        let xml = r#"<list>
			<skillTree type="classSkillTree" classId="0">
				<skill skillId="194" skillLevel="1" getLevel="1" />
				<skill skillId="205" skillLevel="1" getLevel="1" />
				<skill skillId="239" skillLevel="1" getLevel="1" />
			</skillTree>
			<skillTree type="classSkillTree" classId="1" parentClassId="0">
				<skill skillId="300" skillLevel="1" getLevel="1" />
				<skill skillId="239" skillLevel="1" getLevel="5" />
				<skill skillId="100" skillLevel="1" getLevel="1" />
			</skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        // Class 0: buckets 239 -> 7, 194 -> 8, 205 -> 9.
        assert_eq!(
            keys(&trees.complete_class_tree_java_order(ClassId(0))),
            vec![(239, 1), (194, 1), (205, 1)]
        );
        // Class 1: the parent's tree first in its own order, then the child's keys that are new in
        // the child's hash order (100 -> 4, 300 -> 9). 239 keeps its first place, with the child's line.
        let order = trees.complete_class_tree_java_order(ClassId(1));
        assert_eq!(
            keys(&order),
            vec![(239, 1), (194, 1), (205, 1), (100, 1), (300, 1)]
        );
        assert_eq!(order[0].get_level, 5);
        // The sorted view is unchanged.
        assert_eq!(
            trees
                .complete_class_tree(ClassId(1))
                .map(|s| (s.skill_id, s.skill_level))
                .collect::<Vec<_>>(),
            vec![(100, 1), (194, 1), (205, 1), (239, 1), (300, 1)]
        );
        assert!(trees.complete_class_tree_java_order(ClassId(2)).is_empty());
    }

    #[test]
    fn concurrent_map_order_matches_java() {
        // 30 keys put in this order into a `ConcurrentHashMap<Integer, _>` on a JDK 25
        // (`id * 1021 + level` as the key) list as below: three resizes, with bins that hold
        // several keys when they split.
        let keys: Vec<(i32, i32)> = (100..130).map(|id| (id, 1 + id % 3)).collect();
        let java = [
            (128, 3),
            (107, 3),
            (127, 2),
            (106, 2),
            (126, 1),
            (105, 1),
            (104, 3),
            (103, 2),
            (125, 3),
            (102, 1),
            (124, 2),
            (123, 1),
            (122, 3),
            (101, 3),
            (121, 2),
            (100, 2),
            (120, 1),
            (119, 3),
            (118, 2),
            (117, 1),
            (116, 3),
            (115, 2),
            (114, 1),
            (113, 3),
            (112, 2),
            (111, 1),
            (110, 3),
            (109, 2),
            (108, 1),
            (129, 1),
        ];
        assert_eq!(java_concurrent_map_order(&keys), java);
    }

    #[test]
    fn clan_trees_keep_social_class_residence_and_items() {
        let xml = r#"<list>
			<skillTree type="pledgeSkillTree">
				<skill skillName="Clan Body" skillId="370" skillLevel="1" getLevel="5" levelUpSp="1500">
					<socialClass>APPRENTICE</socialClass>
					<item id="9816" count="10" />
				</skill>
				<skill skillName="Residence Body" skillId="590" skillLevel="1" getLevel="4" residenceSkill="true">
					<socialClass>APPRENTICE</socialClass>
					<residenceId>1</residenceId>
					<residenceId>8</residenceId>
				</skill>
			</skillTree>
			<skillTree type="subPledgeSkillTree">
				<skill skillName="Fire Squad" skillId="611" skillLevel="1" getLevel="8" levelUpSp="12000">
					<socialClass>KNIGHT</socialClass>
					<socialClass>EMPEROR</socialClass>
					<item id="9910" count="8" />
				</skill>
			</skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        // The class part is untouched.
        assert!(trees.is_empty());
        assert_eq!(trees.pledge_skills().len(), 2);
        let body = trees.pledge_skill(370, 1).unwrap();
        assert_eq!((body.get_level, body.level_up_sp), (5, 1500));
        assert_eq!(body.social_class, Some(SocialClass::Apprentice));
        assert_eq!(body.social_class.unwrap().ordinal(), 2);
        assert!(!body.residence_skill);
        assert_eq!(
            body.required_items,
            vec![SkillLearnItem {
                item_id: 9816,
                count: 10
            }]
        );
        let residence = trees.pledge_skill(590, 1).unwrap();
        assert!(residence.residence_skill);
        assert_eq!(residence.residence_ids, vec![1, 8]);
        // `setSocialClass` keeps the first one.
        let squad = trees.sub_pledge_skill(611, 1).unwrap();
        assert_eq!(squad.social_class, Some(SocialClass::Knight));
        assert!(trees.pledge_skill(611, 1).is_none());
        assert!(trees.is_clan_skill(370, 1) && trees.is_clan_skill(611, 1));
        assert!(!trees.is_clan_skill(370, 2));
    }

    #[test]
    fn fishing_tree_is_kept_with_races_items_and_java_order() {
        let xml = r#"<list>
			<skillTree type="fishingSkillTree">
				<skill skillName="Fishing" skillId="1312" skillLevel="1" getLevel="1" learnedByNpc="true">
					<item id="57" count="1000" />
				</skill>
				<skill skillName="Expand Dwarven Craft" skillId="1368" skillLevel="1" getLevel="5" learnedByNpc="true">
					<race>DWARF</race>
					<item id="7609" count="2000" />
				</skill>
				<skill skillName="Expertise" skillId="1315" skillLevel="1" getLevel="1" learnedByNpc="true" />
			</skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        // The class part is untouched.
        assert!(trees.is_empty());
        assert_eq!(trees.fishing_skills().len(), 3);
        let dwarven = trees.fishing_skill(1368, 1).unwrap();
        assert_eq!(dwarven.races, vec![Race::Dwarf]);
        assert_eq!(
            dwarven.required_items,
            vec![SkillLearnItem {
                item_id: 7609,
                count: 2000
            }]
        );
        assert!(trees.fishing_skill(1368, 2).is_none());
        // Worked out by hand, h = id * 1021 + 1 and bin = (h ^ h >>> 16) & 15: 1312 -> 5,
        // 1368 -> 12, 1315 -> 12 (1368 was put first, so it stays first in the bin).
        assert_eq!(
            trees
                .fishing_skills()
                .iter()
                .map(|l| l.skill_id)
                .collect::<Vec<_>>(),
            vec![1312, 1368, 1315]
        );
        // Clan lookups do not see fishing lines.
        assert!(!trees.is_clan_skill(1312, 1));
    }

    #[test]
    fn social_class_ordinals_are_javas() {
        assert_eq!(SocialClass::Vagabond.ordinal(), 0);
        assert_eq!(SocialClass::Knight.ordinal(), 4);
        assert_eq!(SocialClass::Baron.ordinal(), 6);
        assert_eq!(SocialClass::Emperor.ordinal(), 13);
        assert_eq!(SocialClass::from_name("GRAND_DUKE"), Some(SocialClass::GrandDuke));
        assert_eq!(SocialClass::from_name("NOPE"), None);
    }

    #[test]
    fn missing_required_attribute_is_an_error() {
        let xml = r#"<list><skillTree type="classSkillTree" classId="0">
			<skill skillId="1" skillLevel="1" autoGet="true" />
		</skillTree></list>"#;
        assert!(matches!(SkillTrees::from_xml(xml), Err(DataError::Xml { .. })));
    }

    #[test]
    fn game_master_trees_are_kept_apart() {
        let xml = r#"<list>
			<skillTree type="gameMasterSkillTree">
				<skill skillName="GM Haste" skillId="7029" skillLevel="1" getLevel="1" learnedByNpc="false"/>
			</skillTree>
			<skillTree type="gameMasterAuraSkillTree">
				<skill skillName="Aura" skillId="7041" skillLevel="1" getLevel="1" learnedByNpc="false"/>
				<skill skillName="Aura 2" skillId="7042" skillLevel="1" getLevel="1" learnedByNpc="false"/>
			</skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(
            trees
                .game_master_skills()
                .iter()
                .map(|s| s.skill_id)
                .collect::<Vec<_>>(),
            vec![7029]
        );
        assert_eq!(trees.game_master_aura_skills().len(), 2);
        // Not a class, clan or fishing tree: nothing else is kept.
        assert!(trees.pledge_skills().is_empty() && trees.fishing_skills().is_empty());
    }

    #[test]
    fn the_collect_tree_is_kept() {
        let xml = r#"<list><skillTree type="collectSkillTree">
			<skill skillName="Star Stone Gathering" skillId="932" skillLevel="1" getLevel="1" learnedByNpc="true"><item id="13728" count="1"/></skill>
			<skill skillName="Star Stone Gathering" skillId="932" skillLevel="2" getLevel="1" learnedByNpc="true"><item id="57" count="400000"/></skill>
		</skillTree></list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(trees.collect_skills().len(), 2);
        let second = trees.collect_skill(932, 2).unwrap();
        assert_eq!(second.required_items[0].count, 400_000);
        assert!(trees.collect_skill(932, 3).is_none());
        assert!(trees.fishing_skills().is_empty());
    }

    #[test]
    fn the_noble_tree_is_kept() {
        let xml = r#"<list><skillTree type="nobleSkillTree">
            <skill skillName="Noblesse Blessing" skillId="1323" skillLevel="1" getLevel="1" />
            <skill skillName="Wyvern Aegis" skillId="327" skillLevel="1" getLevel="1" />
        </skillTree></list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        let mut ids: Vec<i32> = trees.noble_skills().iter().map(|l| l.skill_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![327, 1323]);
        assert!(trees.fishing_skills().is_empty());
    }

    #[test]
    fn the_hero_tree_is_kept() {
        let xml = r#"<list><skillTree type="heroSkillTree">
            <skill skillName="Heroic Miracle" skillId="395" skillLevel="1" getLevel="1" />
            <skill skillName="Heroic Berserker" skillId="396" skillLevel="1" getLevel="1" />
        </skillTree></list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        let mut ids: Vec<i32> = trees.hero_skills().iter().map(|l| l.skill_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![395, 396]);
        assert!(trees.is_hero_skill(395, 1));
        assert!(trees.is_hero_skill(396, -1));
        assert!(!trees.is_hero_skill(395, 2));
        assert!(!trees.is_hero_skill(1323, -1));
        assert!(trees.noble_skills().is_empty());
    }

    #[test]
    fn the_sub_class_tree_keeps_its_slot_conditions() {
        let trees = SkillTrees::from_xml(
            r#"<list><skillTree type="subClassSkillTree">
                <skill skillName="Emergent Ability - Attack" skillId="631" skillLevel="1" getLevel="1">
                    <subClassConditions slot="1" lvl="65" />
                    <subClassConditions slot="2" lvl="65" />
                    <item id="10280" count="1" />
                </skill>
            </skillTree></list>"#,
        )
        .unwrap();
        let line = trees.sub_class_skill(631, 1).unwrap();
        assert_eq!(line.sub_class_conditions, [(1, 65), (2, 65)]);
        assert_eq!(
            line.required_items,
            [SkillLearnItem {
                item_id: 10280,
                count: 1
            }]
        );
        assert_eq!(trees.sub_class_skills().len(), 1);
        assert!(trees.sub_class_skill(631, 2).is_none());
    }

    #[test]
    fn the_transform_tree_is_kept() {
        let trees = SkillTrees::from_xml(
            r#"<list><skillTree type="transformSkillTree">
                <skill skillName="Transform Onyx Beast" skillId="617" skillLevel="1" getLevel="50">
                    <item id="9648" count="1" />
                </skill>
                <skill skillName="Transform Death Blader" skillId="618" skillLevel="1" getLevel="55">
                    <preRequisiteSkill id="617" lvl="1" />
                    <race>HUMAN</race>
                </skill>
            </skillTree></list>"#,
        )
        .unwrap();
        assert_eq!(trees.transform_skills().len(), 2);
        let line = trees.transform_skill(618, 1).unwrap();
        assert_eq!(line.get_level, 55);
        assert_eq!(line.pre_req_skills, [(617, 1)]);
        assert_eq!(line.races, [Race::Human]);
        assert!(trees.transform_skill(618, 2).is_none());
        // Not mixed into the other trees.
        assert!(trees.sub_class_skills().is_empty());
    }

    #[test]
    fn the_shipped_transform_tree_loads() {
        let root = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game");
        if !root.is_dir() {
            eprintln!("skipped: {} does not exist", root.display());
            return;
        }
        let trees = SkillTrees::load(root);
        assert!(trees.transform_skill(617, 1).is_some_and(|l| l.get_level == 50));
        assert!(trees.transform_skills().len() > 20);
    }
}
