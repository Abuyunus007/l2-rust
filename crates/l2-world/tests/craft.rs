#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Crafting (M6-CRAFT-01): the recipe book, making an item for oneself and for a customer of a
//! manufacture shop, the alt crafting with its passes, with the messages, packets and rows of
//! Java's `RecipeManager`, `Player`, `Recipes` and the `RequestRecipe*` packets. The expected
//! values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::recipe::RecipeData;
use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{
    RecipeShopListEntry, RequestRecipeBookDestroy, RequestRecipeBookOpen, RequestRecipeItemMakeInfo,
    RequestRecipeItemMakeSelf, RequestRecipeShopListSet, RequestRecipeShopMakeInfo,
    RequestRecipeShopMakeItem, RequestRecipeShopMessageSet,
};
use l2_protocol_game::server::{
    RecipeBookItemList, RecipeItemMakeInfo, RecipeShopItemInfo, RecipeShopManageList, RecipeShopMsg,
    RecipeShopSellList, SetupGauge, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

// ---- the datapack -------------------------------------------------------------------------------

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>5000</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const ADENA: i32 = 57;
const SWORD: i32 = 3;
const ARROW: i32 = 17;
const MAT_A: i32 = 1864;
const MAT_B: i32 = 1869;
const RARE_ARROW: i32 = 910;
const RECIPE_ARROW: i32 = 1666;
const RECIPE_COMMON: i32 = 1700;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="3" type="Weapon" name="Sword"><set name="price" val="50" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /></item>
<item id="17" type="EtcItem" name="Wooden Arrow"><set name="price" val="2" /><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="910" type="EtcItem" name="Rare Arrow"><set name="price" val="9" /><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1864" type="EtcItem" name="Stem"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1869" type="EtcItem" name="Iron Ore"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1666" type="EtcItem" name="Recipe: Wooden Arrow"><set name="etcitem_type" val="RECIPE" /><set name="handler" val="Recipes" /><set name="weight" val="1" /></item>
<item id="1700" type="EtcItem" name="Recipe: Common"><set name="etcitem_type" val="RECIPE" /><set name="handler" val="Recipes" /><set name="weight" val="1" /></item>
</list>"#;

/// Recipe 1: the wooden arrow (dwarven, 4 + 2 materials, 500 arrows, 30 MP, always works).
/// Recipe 2: dwarven, never works. Recipe 3: common, level 5. Recipe 4: always a rare product.
/// Recipe 5: dwarven, a Sword for 1 stem and 100 HP.
const RECIPES: &str = r#"<list>
<item id="1" recipeId="1666" name="arrow" craftLevel="1" type="dwarven" successRate="100">
  <ingredient id="1864" count="4" /><ingredient id="1869" count="2" /><production id="17" count="500" /><statUse name="MP" value="30" />
</item>
<item id="2" recipeId="1701" name="never" craftLevel="1" type="dwarven" successRate="0">
  <ingredient id="1864" count="1" /><production id="3" count="1" /><statUse name="MP" value="10" />
</item>
<item id="3" recipeId="1700" name="common" craftLevel="5" type="common" successRate="100">
  <ingredient id="1864" count="1" /><production id="3" count="1" /><statUse name="MP" value="10" />
</item>
<item id="4" recipeId="1702" name="rare" craftLevel="1" type="dwarven" successRate="100">
  <ingredient id="1864" count="1" /><production id="17" count="1" /><productionRare id="910" count="2" rarity="100" /><statUse name="MP" value="10" />
</item>
<item id="5" recipeId="1703" name="blood" craftLevel="1" type="dwarven" successRate="100">
  <ingredient id="1864" count="1" /><production id="3" count="1" /><statUse name="HP" value="100" />
</item>
</list>"#;

const SKILLS: &str = r#"<list>
<skill id="172" levels="3" name="Create Item"><operateType>P</operateType></skill>
<skill id="1320" levels="5" name="Common Craft"><operateType>P</operateType></skill>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn access_level_allows_transaction(&self, _level: i32) -> bool {
        true
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        ..Default::default()
    }
}

fn player(id: ObjectId, name: &str, x: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 4,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y: 1000,
        z: 0,
        heading: 0,
        cur_hp: 500.0,
        max_hp: 500.0,
        cur_mp: 100.0,
        max_mp: 100.0,
        cur_cp: 0.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

/// Alice is a dwarf with Create Item (172) at 3 and Common Craft (1320) at 1.
fn crafter(items: Vec<ItemData>) -> PlayerData {
    let mut p = player(A, "Alice", 1000, items);
    p.skills = vec![
        SkillData {
            skill_id: 172,
            skill_level: 3,
        },
        SkillData {
            skill_id: 1320,
            skill_level: 1,
        },
    ];
    p
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new() -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_recipe_data(Arc::new(RecipeData::from_xml(RECIPES).unwrap()));
        Self { world, log }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// Alice (the crafter) and Bob stand 50 apart; what the enter sent is dropped.
    fn pair(alice: PlayerData, bob: Vec<ItemData>) -> Self {
        let mut s = Self::new();
        s.enter(alice);
        s.enter(player(B, "Bob", 1050, bob));
        s.log.take();
        s
    }

    /// Alice with the recipes 1, 2, 4 and 5 in her dwarven book and 3 in her common book.
    fn with_book(alice_items: Vec<ItemData>, bob: Vec<ItemData>) -> Self {
        let mut alice = crafter(alice_items);
        alice.profile.recipes = RecipeLoad {
            book: vec![
                book(1, true, 0),
                book(2, true, 0),
                book(3, false, 0),
                book(4, true, 0),
                book(5, true, 0),
            ],
            shop: vec![],
        };
        Self::pair(alice, bob)
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn craft(&mut self, who: ObjectId, input: CraftInput) {
        self.input(who, ClientInput::Craft(input));
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn count_of(&self, who: ObjectId, item_id: i32) -> i64 {
        self.world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    fn mp(&self, who: ObjectId) -> f64 {
        self.world.player(who).unwrap().core.cur_mp
    }
}

fn book(recipe_id: i32, dwarven: bool, class_index: i32) -> RecipeBookEntry {
    RecipeBookEntry {
        recipe_id,
        dwarven,
        class_index,
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn of(out: &[Out], to: ObjectId, op: ServerOpcode) -> Vec<&[u8]> {
    packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == op.name())
        .collect()
}

fn messages(out: &[Out], to: ObjectId) -> Vec<SystemMessage> {
    of(out, to, ServerOpcode::SystemMessage)
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

fn message_ids(out: &[Out], to: ObjectId) -> Vec<SystemMessageId> {
    messages(out, to).into_iter().map(|m| m.id).collect()
}

fn make_infos(out: &[Out], to: ObjectId) -> Vec<RecipeItemMakeInfo> {
    of(out, to, ServerOpcode::RecipeItemMakeInfo)
        .iter()
        .map(|raw| decode::<RecipeItemMakeInfo>(raw))
        .collect()
}

fn recipe_writes(out: &[Out]) -> Vec<RecipeWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Recipe(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn item_name(id: i32) -> SystemMessageParam {
    SystemMessageParam::ItemName(id)
}

// ---- the recipe book ----------------------------------------------------------------------------

/// `Player.restoreRecipeBook`: dwarven rows count for the active class index only, the book
/// window lists them in the order of Java's `ConcurrentHashMap` (the ids below the table size
/// ascending).
#[test]
fn the_book_is_restored_and_listed() {
    let mut alice = crafter(vec![]);
    alice.profile.recipes = RecipeLoad {
        book: vec![
            book(5, true, 0),
            book(1, true, 0),
            // A dwarven recipe of another class index is not in this book.
            book(2, true, 1),
            book(3, false, 0),
        ],
        shop: vec![],
    };
    let mut s = Scene::pair(alice, vec![]);
    assert_eq!(s.world.dwarven_recipe_book(A), [1, 5]);
    assert_eq!(s.world.common_recipe_book(A), [3]);

    // `RequestRecipeBookOpen`: 0 is the dwarven book. `RecipeBookItemList` writes `!isDwarven`,
    // the max MP and the ids with their position.
    s.craft(A, CraftInput::BookOpen(RequestRecipeBookOpen { recipe_type: 0 }));
    let out = s.log.take();
    let list: RecipeBookItemList = decode(of(&out, A, ServerOpcode::RecipeBookItemList)[0]);
    assert_eq!(
        list,
        RecipeBookItemList {
            is_common: false,
            max_mp: 5000,
            recipes: vec![1, 5]
        }
    );
    s.craft(A, CraftInput::BookOpen(RequestRecipeBookOpen { recipe_type: 1 }));
    let out = s.log.take();
    let list: RecipeBookItemList = decode(of(&out, A, ServerOpcode::RecipeBookItemList)[0]);
    assert_eq!((list.is_common, list.recipes), (true, vec![3]));
}

/// `Player.restoreRecipeBook`: an unknown recipe id throws in `registerDwarvenRecipeList(null)`
/// and the `catch` ends the read.
#[test]
fn an_unknown_recipe_row_ends_the_restore() {
    let mut alice = crafter(vec![]);
    alice.profile.recipes = RecipeLoad {
        book: vec![book(1, true, 0), book(999, true, 0), book(4, true, 0)],
        shop: vec![],
    };
    let s = Scene::pair(alice, vec![]);
    assert_eq!(s.world.dwarven_recipe_book(A), [1]);
}

#[test]
fn forgetting_a_recipe_deletes_the_row_and_its_shortcut() {
    let mut alice = crafter(vec![]);
    alice.profile.recipes = RecipeLoad {
        book: vec![book(1, true, 0), book(3, false, 0)],
        shop: vec![],
    };
    alice.shortcuts = vec![
        ShortcutData {
            slot: 0,
            page: 0,
            kind: 5,
            shortcut_id: 1,
            level: 0,
        },
        ShortcutData {
            slot: 1,
            page: 0,
            kind: 5,
            shortcut_id: 3,
            level: 0,
        },
    ];
    let mut s = Scene::pair(alice, vec![]);
    s.craft(
        A,
        CraftInput::BookDestroy(RequestRecipeBookDestroy { recipe_id: 1 }),
    );
    let out = s.log.take();
    assert_eq!(
        recipe_writes(&out),
        [RecipeWrite::Delete {
            char_id: 1,
            recipe_id: 1,
            class_index: 0
        }]
    );
    let list: RecipeBookItemList = decode(of(&out, A, ServerOpcode::RecipeBookItemList)[0]);
    assert_eq!((list.is_common, list.recipes), (false, vec![]));
    let shortcuts: Vec<i32> = s
        .world
        .player(A)
        .unwrap()
        .data
        .shortcuts
        .iter()
        .map(|c| c.shortcut_id)
        .collect();
    assert_eq!(shortcuts, [3]);

    // The common recipe: class index 0, the common window.
    s.craft(
        A,
        CraftInput::BookDestroy(RequestRecipeBookDestroy { recipe_id: 3 }),
    );
    let out = s.log.take();
    assert_eq!(
        recipe_writes(&out),
        [RecipeWrite::Delete {
            char_id: 1,
            recipe_id: 3,
            class_index: 0
        }]
    );
    let list: RecipeBookItemList = decode(of(&out, A, ServerOpcode::RecipeBookItemList)[0]);
    assert!(list.is_common);
    // An id that is no recipe is ignored.
    s.craft(
        A,
        CraftInput::BookDestroy(RequestRecipeBookDestroy { recipe_id: 77 }),
    );
    assert!(s.log.take().is_empty());
}

#[test]
fn make_info_shows_the_craft_window() {
    let mut s = Scene::with_book(vec![], vec![]);
    s.craft(A, CraftInput::ItemMakeInfo(RequestRecipeItemMakeInfo { id: 3 }));
    let out = s.log.take();
    assert_eq!(
        make_infos(&out, A),
        [RecipeItemMakeInfo {
            id: 3,
            is_common: true,
            cur_mp: 100,
            max_mp: 5000,
            success: true
        }]
    );
}

// ---- the Recipes item ---------------------------------------------------------------------------

fn use_item(s: &mut Scene, who: ObjectId, object_id: i32) {
    s.input(
        who,
        ClientInput::UseItem {
            object_id,
            ctrl_pressed: false,
        },
    );
}

/// `Recipes.onItemUse`: the recipe joins the dwarven book (row with type 1 and the class index),
/// the item is used up and "S1 has been added" follows.
#[test]
fn a_recipe_item_joins_the_book() {
    let mut s = Scene::pair(
        crafter(vec![inv(100, RECIPE_ARROW, 1), inv(101, RECIPE_ARROW, 1)]),
        vec![],
    );
    use_item(&mut s, A, 100);
    let out = s.log.take();
    assert_eq!(s.world.dwarven_recipe_book(A), [1]);
    assert_eq!(
        recipe_writes(&out),
        [RecipeWrite::Insert {
            char_id: 1,
            recipe_id: 1,
            class_index: 0,
            dwarven: true
        }]
    );
    let sent = messages(&out, A);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, SystemMessageId::S1HasBeenAdded);
    assert_eq!(sent[0].params, [item_name(RECIPE_ARROW)]);
    assert_eq!(s.count_of(A, RECIPE_ARROW), 1);

    // The same recipe again: refused, the item stays.
    use_item(&mut s, A, 101);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::ThatRecipeIsAlreadyRegistered]
    );
    assert!(recipe_writes(&out).is_empty());
    assert_eq!(s.count_of(A, RECIPE_ARROW), 1);
}

#[test]
fn a_recipe_item_needs_the_skill_the_level_and_room() {
    // No Common Craft at all.
    let mut alice = player(A, "Alice", 1000, vec![inv(100, RECIPE_COMMON, 1)]);
    alice.skills = vec![SkillData {
        skill_id: 172,
        skill_level: 3,
    }];
    let mut s = Scene::pair(alice, vec![]);
    use_item(&mut s, A, 100);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::TheRecipeCannotBeRegisteredYouDoNotHaveTheAbilityToCreateItems]
    );

    // Common Craft 1 against a level 5 recipe.
    let mut s = Scene::pair(crafter(vec![inv(100, RECIPE_COMMON, 1)]), vec![]);
    use_item(&mut s, A, 100);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YourCreateItemLevelIsTooLowToRegisterThisRecipe]
    );
    assert_eq!(s.count_of(A, RECIPE_COMMON), 1);

    // `DwarfRecipeLimit` 1 with one recipe known.
    let mut alice = crafter(vec![inv(100, RECIPE_ARROW, 1)]);
    alice.profile.recipes = RecipeLoad {
        book: vec![book(2, true, 0)],
        shop: vec![],
    };
    let mut s = Scene::pair(alice, vec![]);
    s.world.set_settings(WorldSettings {
        dwarf_recipe_limit: 1,
        ..WorldSettings::default()
    });
    use_item(&mut s, A, 100);
    let out = s.log.take();
    let sent = messages(&out, A);
    assert_eq!(sent[0].id, SystemMessageId::UpToS1RecipesCanBeRegistered);
    assert_eq!(sent[0].params, [SystemMessageParam::IntNumber(1)]);
}

// ---- making an item -----------------------------------------------------------------------------

fn materials() -> Vec<ItemData> {
    vec![inv(200, MAT_A, 10), inv(201, MAT_B, 2)]
}

fn make_self(s: &mut Scene, recipe: i32) {
    s.craft(
        A,
        CraftInput::ItemMakeSelf(RequestRecipeItemMakeSelf { id: recipe }),
    );
}

/// `RecipeItemMaker` without `AltGameCreation`: the checks, `calculateStatUse(false, true)` (30
/// MP), the materials, the dice, the product, the messages and three `RecipeItemMakeInfo` (the
/// constructor, `rewardPlayer` and `finishCrafting`).
#[test]
fn crafting_for_oneself_works() {
    let mut s = Scene::with_book(materials(), vec![]);
    make_self(&mut s, 1);
    let out = s.log.take();

    assert_eq!(s.mp(A), 70.0);
    assert_eq!(s.count_of(A, MAT_A), 6);
    // Two ore used up entirely: the item is gone.
    assert_eq!(s.count_of(A, MAT_B), 0);
    assert_eq!(s.count_of(A, ARROW), 500);
    assert!(!s.world.is_crafting(A));

    let infos = make_infos(&out, A);
    assert_eq!(infos.len(), 3);
    assert!(infos.iter().all(|i| i.success && i.id == 1 && !i.is_common));
    // The constructor's info has the MP before the craft, the last one after.
    assert_eq!(infos[0].cur_mp, 100);
    assert_eq!(infos[2].cur_mp, 70);

    let sent = messages(&out, A);
    let ids: Vec<SystemMessageId> = sent.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        [
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::YouHaveEarnedS2S1S
        ]
    );
    assert_eq!(
        sent[0].params,
        [item_name(MAT_A), SystemMessageParam::LongNumber(4)]
    );
    assert_eq!(
        sent[2].params,
        [item_name(ARROW), SystemMessageParam::LongNumber(500)]
    );
    // The client gets the whole item list (the craft window counts come from it).
    assert_eq!(of(&out, A, ServerOpcode::ItemList).len(), 1);
}

#[test]
fn the_chance_decides_and_a_failure_still_costs_the_materials() {
    let mut s = Scene::with_book(materials(), vec![]);
    make_self(&mut s, 2);
    let out = s.log.take();
    assert_eq!(s.count_of(A, SWORD), 0);
    assert_eq!(s.count_of(A, MAT_A), 9);
    assert_eq!(s.mp(A), 90.0);
    assert!(message_ids(&out, A).contains(&SystemMessageId::YouFailedAtMixingTheItem));
    let infos = make_infos(&out, A);
    // The constructor's, then the failed one.
    assert_eq!(infos.iter().map(|i| i.success).collect::<Vec<_>>(), [true, false]);
}

/// `rewardPlayer`: the rare product replaces the normal one when `Rnd.get(100) <
/// rarity * CraftMasterworkChanceRate`; with `rarity` 100 it always does.
#[test]
fn a_rare_product_replaces_the_normal_one() {
    let mut s = Scene::with_book(materials(), vec![]);
    make_self(&mut s, 4);
    let out = s.log.take();
    assert_eq!(s.count_of(A, RARE_ARROW), 2);
    assert_eq!(s.count_of(A, ARROW), 0);
    let earned = messages(&out, A).pop().unwrap();
    assert_eq!(earned.id, SystemMessageId::YouHaveEarnedS2S1S);
    assert_eq!(
        earned.params,
        [item_name(RARE_ARROW), SystemMessageParam::LongNumber(2)]
    );
}

/// NOTE(java-quirk): a success draws `Rnd.get(100)` for the rare product also when the recipe has
/// none (its rare item id is 0, not -1), while `CraftMasterwork` is on. The draws of two worlds
/// that differ only in that switch are one apart.
#[test]
fn masterwork_rolls_the_dice_even_without_a_rare_product() {
    let mut next = Vec::new();
    for masterwork in [true, false] {
        let mut s = Scene::with_book(materials(), vec![]);
        s.world.set_settings(WorldSettings {
            craft: CraftSettings {
                craft_masterwork: masterwork,
                ..CraftSettings::default()
            },
            ..WorldSettings::default()
        });
        make_self(&mut s, 1);
        next.push(s.world.rng().below(1_000_000));
    }
    // With the switch on the world drew once more before the probe.
    let mut reference = WorldRng::from_seed(1);
    let draws: Vec<i32> = (0..3).map(|_| reference.below(1_000_000)).collect();
    assert_ne!(next[0], next[1]);
    assert!(draws.contains(&next[0]) && draws.contains(&next[1]));
}

#[test]
fn a_missing_material_stops_the_craft() {
    let mut s = Scene::with_book(vec![inv(200, MAT_A, 3), inv(201, MAT_B, 2)], vec![]);
    make_self(&mut s, 1);
    let out = s.log.take();
    let sent = messages(&out, A);
    assert_eq!(sent[0].id, SystemMessageId::YouAreMissingS2S1RequiredToCreateThat);
    // One stem short (S1 is the name, S2 the missing count).
    assert_eq!(
        sent[0].params,
        [item_name(MAT_A), SystemMessageParam::LongNumber(1)]
    );
    assert_eq!(s.mp(A), 100.0);
    assert_eq!(s.count_of(A, MAT_A), 3);
    assert!(!s.world.is_crafting(A));
    assert_eq!(
        make_infos(&out, A).iter().map(|i| i.success).collect::<Vec<_>>(),
        [false]
    );
}

#[test]
fn the_skill_level_and_the_mp_are_checked() {
    // Recipe 3 is level 5, the common craft skill is 1: ActionFailed and the failed window.
    let mut s = Scene::with_book(materials(), vec![]);
    make_self(&mut s, 3);
    let out = s.log.take();
    assert_eq!(of(&out, A, ServerOpcode::ActionFail).len(), 1);
    assert_eq!(
        make_infos(&out, A).iter().map(|i| i.success).collect::<Vec<_>>(),
        [false]
    );

    // 20 MP for a recipe that wants 30.
    let mut alice = crafter(materials());
    alice.cur_mp = 20.0;
    alice.profile.recipes = RecipeLoad {
        book: vec![book(1, true, 0)],
        shop: vec![],
    };
    let mut s = Scene::pair(alice, vec![]);
    make_self(&mut s, 1);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::NotEnoughMp]);
    assert_eq!(s.count_of(A, MAT_A), 10);
}

#[test]
fn hp_is_a_cost_too_but_never_the_last_point() {
    let mut s = Scene::with_book(vec![inv(200, MAT_A, 1)], vec![]);
    make_self(&mut s, 5);
    assert_eq!(s.world.player(A).unwrap().core.cur_hp, 400.0);
    assert_eq!(s.count_of(A, SWORD), 1);

    // `getCurrentHp() <= value`: with exactly 100 HP the craft is refused.
    let mut alice = crafter(vec![inv(200, MAT_A, 1)]);
    alice.cur_hp = 100.0;
    alice.profile.recipes = RecipeLoad {
        book: vec![book(5, true, 0)],
        shop: vec![],
    };
    let mut s = Scene::pair(alice, vec![]);
    make_self(&mut s, 5);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::NotEnoughHp]);
    assert_eq!(s.count_of(A, MAT_A), 1);
}

#[test]
fn a_recipe_that_is_not_in_the_book_is_an_illegal_action() {
    let mut s = Scene::pair(crafter(materials()), vec![]);
    make_self(&mut s, 1);
    let out = s.log.take();
    // Nothing is made, nothing is sent but the punishment path (no window for a false id).
    assert!(make_infos(&out, A).is_empty());
    assert_eq!(s.count_of(A, MAT_A), 10);
    // A recipe id that does not exist: Java's `sendMessage`.
    make_self(&mut s, 9999);
    let out = s.log.take();
    assert_eq!(
        l2_world_texts(&out, A),
        ["RecipeData: No recipe for: 9999".to_string()]
    );
}

/// The texts of `sendMessage` (system message `S12` with one text parameter).
fn l2_world_texts(out: &[Out], to: ObjectId) -> Vec<String> {
    messages(out, to)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

// ---- the manufacture shop -------------------------------------------------------------------------

fn open_shop(s: &mut Scene, recipes: &[(i32, i64)]) {
    s.craft(
        A,
        CraftInput::ShopMessageSet(RequestRecipeShopMessageSet {
            name: "Arrows".into(),
        }),
    );
    s.craft(
        A,
        CraftInput::ShopListSet(RequestRecipeShopListSet {
            items: Some(
                recipes
                    .iter()
                    .map(|&(recipe_id, cost)| RecipeShopListEntry { recipe_id, cost })
                    .collect(),
            ),
        }),
    );
}

#[test]
fn setting_up_the_shop() {
    let mut s = Scene::with_book(vec![], vec![]);
    open_shop(&mut s, &[(1, 1000), (4, 50)]);
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Manufacture);
    assert!(s.world.is_sitting(A));
    assert_eq!(s.world.manufacture_items(A), [(1, 1000), (4, 50)]);
    // `broadcastToSelfAndVisiblePlayers(RecipeShopMsg)`: Alice and Bob.
    for who in [A, B] {
        let msg: RecipeShopMsg = decode(of(&out, who, ServerOpcode::RecipeShopMsg)[0]);
        assert_eq!(
            msg,
            RecipeShopMsg {
                object_id: 1,
                message: "Arrows".into()
            }
        );
    }

    // Closing it: the type goes, she stands up.
    s.craft(A, CraftInput::ShopManageQuit);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
}

#[test]
fn a_recipe_she_does_not_know_is_refused() {
    let mut s = Scene::with_book(vec![], vec![]);
    open_shop(&mut s, &[(1, 10), (88, 10)]);
    // `getRecipeList(88)` is null: the packet never reaches `runImpl`.
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    assert!(s.world.manufacture_items(A).is_empty());

    // A bad list (here: a negative price, read as `None`) closes the shop.
    s.craft(
        A,
        CraftInput::ShopListSet(RequestRecipeShopListSet { items: None }),
    );
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);

    // A recipe of the data that is not in her book: `handleIllegalPlayerAction`, the list is
    // cleared and no shop opens.
    let mut alice = crafter(vec![]);
    alice.profile.recipes = RecipeLoad {
        book: vec![book(1, true, 0)],
        shop: vec![],
    };
    let mut s = Scene::pair(alice, vec![]);
    open_shop(&mut s, &[(4, 10)]);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    assert!(s.world.manufacture_items(A).is_empty());
}

#[test]
fn the_title_may_not_overflow() {
    let mut s = Scene::with_book(vec![], vec![]);
    let long: String = "x".repeat(30);
    s.craft(
        A,
        CraftInput::ShopMessageSet(RequestRecipeShopMessageSet { name: long }),
    );
    assert_eq!(s.world.recipe_store_name(A), "");
    s.craft(
        A,
        CraftInput::ShopMessageSet(RequestRecipeShopMessageSet { name: "x".repeat(29) }),
    );
    assert_eq!(s.world.recipe_store_name(A).len(), 29);
}

/// `RequestActionUse` 37 and 51: the manage window. It drops the shop items of the other kind.
#[test]
fn the_manage_window_filters_the_shop() {
    let mut s = Scene::with_book(vec![], vec![]);
    open_shop(&mut s, &[(1, 1000), (4, 50)]);
    s.log.take();
    s.input(
        A,
        ClientInput::ActionUse {
            action_id: 37,
            ctrl_pressed: false,
            shift_pressed: false,
        },
    );
    let out = s.log.take();
    // The shop closes (type NONE) and she stands up; the window is the dwarven one.
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    let list: RecipeShopManageList = decode(of(&out, A, ServerOpcode::RecipeShopManageList)[0]);
    assert!(!list.is_common);
    assert_eq!(list.seller_id, 1);
    assert_eq!(list.recipes, [1, 2, 4, 5]);
    assert_eq!(list.items.len(), 2);

    // The common window (51) drops the dwarven shop items.
    s.input(
        A,
        ClientInput::ActionUse {
            action_id: 51,
            ctrl_pressed: false,
            shift_pressed: false,
        },
    );
    let out = s.log.take();
    let list: RecipeShopManageList = decode(of(&out, A, ServerOpcode::RecipeShopManageList)[0]);
    assert!(list.is_common);
    assert_eq!(list.recipes, [3]);
    assert!(list.items.is_empty());
    assert!(s.world.manufacture_items(A).is_empty());
}

/// A customer looks at the shop (`doInteract`), asks about a recipe and orders it: the customer
/// brings the materials and pays, the crafter spends the MP.
#[test]
fn a_customer_orders_an_item() {
    let mut s = Scene::with_book(
        vec![inv(300, ADENA, 5)],
        vec![inv(310, ADENA, 1500), inv(311, MAT_A, 10), inv(312, MAT_B, 2)],
    );
    open_shop(&mut s, &[(1, 1000)]);
    s.log.take();

    // Click on the manufacturer: ActionFailed, then the list.
    for _ in 0..2 {
        s.input(
            B,
            ClientInput::Action {
                object_id: A.0,
                action_id: 0,
            },
        );
    }
    s.advance(500);
    let out = s.log.take();
    let list: RecipeShopSellList = decode(of(&out, B, ServerOpcode::RecipeShopSellList)[0]);
    assert_eq!(list.manufacturer_id, 1);
    assert_eq!((list.cur_mp, list.max_mp), (100, 5000));
    assert_eq!(list.buyer_adena, 1500);
    assert_eq!(list.items.len(), 1);
    assert_eq!((list.items[0].recipe_id, list.items[0].cost), (1, 1000));

    s.craft(
        B,
        CraftInput::ShopMakeInfo(RequestRecipeShopMakeInfo {
            player_object_id: 1,
            recipe_id: 1,
        }),
    );
    let out = s.log.take();
    let info: RecipeShopItemInfo = decode(of(&out, B, ServerOpcode::RecipeShopItemInfo)[0]);
    assert_eq!(
        info,
        RecipeShopItemInfo {
            object_id: 1,
            recipe_id: 1,
            cur_mp: 100,
            max_mp: 5000
        }
    );

    s.craft(
        B,
        CraftInput::ShopMakeItem(RequestRecipeShopMakeItem {
            id: 1,
            recipe_id: 1,
            unknown: 0,
        }),
    );
    let out = s.log.take();
    // MP from the crafter, materials and adena from the customer, the arrows to the customer.
    assert_eq!(s.mp(A), 70.0);
    assert_eq!(s.count_of(B, ADENA), 500);
    assert_eq!(s.count_of(A, ADENA), 1005);
    assert_eq!(s.count_of(B, MAT_A), 6);
    assert_eq!(s.count_of(B, ARROW), 500);
    assert!(!s.world.is_crafting(A));

    let to_crafter = messages(&out, A);
    assert_eq!(
        to_crafter.iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::S2S3HaveBeenCreatedForC1AtThePriceOfS4Adena]
    );
    assert_eq!(
        to_crafter[0].params,
        [
            SystemMessageParam::Text("Bob".into()),
            SystemMessageParam::IntNumber(500),
            item_name(ARROW),
            SystemMessageParam::LongNumber(1000)
        ]
    );
    let to_customer = message_ids(&out, B);
    assert!(to_customer.contains(&SystemMessageId::C1CreatedS2S3AtThePriceOfS4Adena));
    assert!(to_customer.contains(&SystemMessageId::YouHaveEarnedS2S1S));
    // The customer's windows are `RecipeShopItemInfo`s (before, and twice after a success).
    assert!(of(&out, B, ServerOpcode::RecipeShopItemInfo).len() >= 2);
    assert!(make_infos(&out, B).is_empty());
}

#[test]
fn a_customer_without_the_price_is_turned_away() {
    let mut s = Scene::with_book(
        vec![],
        vec![inv(310, ADENA, 999), inv(311, MAT_A, 10), inv(312, MAT_B, 2)],
    );
    open_shop(&mut s, &[(1, 1000)]);
    s.log.take();
    s.craft(
        B,
        CraftInput::ShopMakeItem(RequestRecipeShopMakeItem {
            id: 1,
            recipe_id: 1,
            unknown: 0,
        }),
    );
    let out = s.log.take();
    assert_eq!(message_ids(&out, B), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    assert_eq!(s.mp(A), 100.0);
    assert_eq!(s.count_of(B, MAT_A), 10);
}

#[test]
fn only_an_open_shop_in_range_can_be_ordered_from() {
    let mut s = Scene::with_book(
        vec![],
        vec![inv(310, ADENA, 5000), inv(311, MAT_A, 10), inv(312, MAT_B, 2)],
    );
    // No shop open: nothing happens, not even a message.
    s.craft(
        B,
        CraftInput::ShopMakeItem(RequestRecipeShopMakeItem {
            id: 1,
            recipe_id: 1,
            unknown: 0,
        }),
    );
    assert!(s.log.take().is_empty());
    open_shop(&mut s, &[(1, 1000)]);
    s.log.take();
    // A shopper who is in a store himself cannot craft.
    s.craft(
        B,
        CraftInput::ShopMakeItem(RequestRecipeShopMakeItem {
            id: 1,
            recipe_id: 1,
            unknown: 0,
        }),
    );
    assert_eq!(s.mp(A), 70.0);
}

// ---- the alt crafting ---------------------------------------------------------------------------

fn alt_settings() -> WorldSettings {
    WorldSettings {
        craft: CraftSettings {
            alt_game_creation: true,
            ..CraftSettings::default()
        },
        ..WorldSettings::default()
    }
}

/// `AltGameCreation`: the 6 materials go in passes of 3 (the skill level 3), the MP cost is cut
/// by the 2 passes (15 each), every pass shows the skill and the gauge, and the craft ends after
/// the last gauge.
#[test]
fn the_alt_crafting_takes_the_materials_in_passes() {
    let mut s = Scene::with_book(materials(), vec![]);
    s.world.set_settings(alt_settings());
    make_self(&mut s, 1);
    let out = s.log.take();
    // The constructor's window; nothing is taken yet; the maker is registered.
    assert_eq!(make_infos(&out, A).len(), 1);
    assert_eq!(s.mp(A), 100.0);
    assert_eq!(s.count_of(A, MAT_A), 10);

    // While busy, the book cannot be opened and a second craft is refused.
    s.craft(A, CraftInput::BookOpen(RequestRecipeBookOpen { recipe_type: 0 }));
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouMayNotAlterYourRecipeBookWhileEngagedInManufacturing]
    );
    make_self(&mut s, 1);
    let busy = messages(&s.log.take(), A);
    assert_eq!(busy[0].id, SystemMessageId::S2S1);
    assert_eq!(
        busy[0].params,
        [
            item_name(ARROW),
            SystemMessageParam::Text("You are busy creating.".into())
        ]
    );

    // The first pass runs 100 ms after the request: 15 MP, 3 stems "equipped", the gauge.
    s.advance(100);
    let out = s.log.take();
    assert_eq!(s.mp(A), 85.0);
    let sent = messages(&out, A);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, SystemMessageId::EquippedS1S2);
    assert_eq!(
        sent[0].params,
        [SystemMessageParam::LongNumber(3), item_name(MAT_A)]
    );
    let gauge: SetupGauge = decode(of(&out, A, ServerOpcode::SetupGauge)[0]);
    assert_eq!((gauge.color, gauge.time), (SetupGauge::BLUE, 1000));
    assert_eq!(of(&out, A, ServerOpcode::MagicSkillUse).len(), 1);
    // Nothing is consumed before the end.
    assert_eq!(s.count_of(A, MAT_A), 10);

    // The second pass comes `100 + delay` ms later and finishes the materials: no more skill
    // animation, one more gauge, and the craft ends `delay` ms after that.
    s.advance(1100);
    let out = s.log.take();
    assert_eq!(s.mp(A), 70.0);
    assert_eq!(of(&out, A, ServerOpcode::MagicSkillUse).len(), 0);
    assert_eq!(of(&out, A, ServerOpcode::SetupGauge).len(), 1);
    assert_eq!(s.count_of(A, ARROW), 0);
    s.advance(1000);
    let out = s.log.take();
    assert_eq!(s.count_of(A, ARROW), 500);
    assert_eq!(s.count_of(A, MAT_A), 6);
    assert!(!s.world.is_crafting(A));
    // SP: `(2 * 500 / 1) / 10`, cut by 4 for each skill level above the recipe's (3 > 1: twice),
    // so 6. (The exp of the same sum, 62, is not added: the test world has no experience table.)
    assert_eq!(s.world.player(A).unwrap().data.sp, 6);
    assert!(make_infos(&out, A).iter().all(|i| i.success));

    // Free again.
    make_self(&mut s, 1);
    assert_eq!(make_infos(&s.log.take(), A).len(), 1);
}

#[test]
fn leaving_ends_a_waiting_maker() {
    let mut s = Scene::with_book(materials(), vec![]);
    s.world.set_settings(alt_settings());
    make_self(&mut s, 1);
    s.log.take();
    s.input(A, ClientInput::Logout);
    s.log.take();
    // The timer fires for a player who is gone: nothing happens.
    s.advance(3000);
    assert!(s.world.player(A).is_none());
}

// ---- rows ---------------------------------------------------------------------------------------

/// `Player.storeRecipeShopList`: with `StoreRecipeShopList` a character that has a shop writes
/// it with its store; without the switch, or without a shop, nothing is written.
#[test]
fn the_shop_is_stored_only_when_asked_for() {
    for (switch, expected) in [(false, None), (true, Some(vec![(1, 1000), (4, 50)]))] {
        let mut s = Scene::with_book(vec![], vec![]);
        s.world.set_settings(WorldSettings {
            craft: CraftSettings {
                store_recipe_shoplist: switch,
                ..CraftSettings::default()
            },
            ..WorldSettings::default()
        });
        open_shop(&mut s, &[(1, 1000), (4, 50)]);
        s.log.take();
        s.input(A, ClientInput::Logout);
        let out = s.log.take();
        let stored = out
            .iter()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::StorePlayer(p)) => Some(p.profile.recipe_shop_to_store.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(stored, expected, "switch {switch}");
    }
}

/// `Player.restoreRecipeShopList`: with the switch on, the rows come back as the shop (the type
/// is not restored: a restart does not reopen the store).
#[test]
fn the_shop_is_restored_with_the_switch() {
    for (switch, expected) in [(false, vec![]), (true, vec![(4, 50), (1, 1000)])] {
        let mut alice = crafter(vec![]);
        alice.profile.recipes = RecipeLoad {
            book: vec![book(1, true, 0), book(4, true, 0)],
            shop: vec![(4, 50), (1, 1000)],
        };
        let mut s = Scene::new();
        s.world.set_settings(WorldSettings {
            craft: CraftSettings {
                store_recipe_shoplist: switch,
                ..CraftSettings::default()
            },
            ..WorldSettings::default()
        });
        s.enter(alice);
        assert_eq!(s.world.manufacture_items(A), expected);
        assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    }
}
