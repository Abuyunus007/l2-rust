#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The pet manager's evolve and restore (M6-FU-14): `PetManager.doEvolve` / `doRestore`, their
//! level and item checks, the new collar, the `pets` rows and the packets. Expected values follow
//! `PetManager.java`.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PetDataTable, PetSkillData, PlayerTemplates, SkillTemplates, SkillTrees,
    SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    InventoryUpdate, MagicSkillUse, NpcHtmlMessage, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Vec<u8>>>>);

impl Log {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(bytes);
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    fn summon_writes(&self) -> Vec<SummonWrite> {
        let mut commands = self.0.lock().unwrap();
        let writes = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Summon(w) => Some(w.clone()),
                _ => None,
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::Summon(_)));
        writes
    }
}

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>1000</hp><mp>500</mp><cp>200</cp></level></lvlUpgainData></list>"#;

/// 2046 has the shape of the datapack's Wolf's Necklace (the collar's skill).
const SKILLS: &str = r##"<list>
<skill id="2046" levels="2" name="Wolf's Necklace">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<hitTime>500</hitTime>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<effects><effect name="SummonPet" /></effects>
</skill>
</list>"##;

/// The collars of the four pets: 6648 -> 10311 is `evolve 3`, 10307 -> 9882 is `restore 1`.
const WOLF_COLLAR: i32 = 6648;
const GREAT_WOLF_COLLAR: i32 = 10311;
const GRADE_COLLAR: i32 = 9882;
const EVOLVED_COLLAR: i32 = 10307;
const WOLF_NPC: i32 = 12780;
const GREAT_WOLF_NPC: i32 = 12781;
const GRADE_NPC: i32 = 12782;
const EVOLVED_NPC: i32 = 12783;
const PET_MANAGER: i32 = 30731;

fn collar_item(id: i32) -> String {
    format!(
        r#"<item id="{id}" type="EtcItem" name="Collar {id}">
	<set name="default_action" val="SKILL_MAINTAIN" /><set name="etcitem_type" val="PET_COLLAR" />
	<set name="weight" val="20" /><set name="handler" val="SummonItems" />
	<skills><skill id="2046" level="1" /></skills>
</item>"#
    )
}

fn pet_npc(id: i32) -> String {
    format!(
        r#"<npc id="{id}" level="1" type="Pet" name="Pet {id}">
    <race>BEAST</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="460.96247" hpRegen="3.5" mp="259.2" mpRegen="1.5" />
      <attack physical="64.83203" magical="44.27187" random="10" critical="4" accuracy="4.75" attackSpeed="253" type="FIST" range="40" distance="80" width="120" />
      <defence physical="101.83533" magical="74.51868" />
      <speed><walk ground="30" /><run ground="120" /></speed>
    </stats>
    <shots soul="2" spirit="2" />
    <collision><radius normal="12"/><height normal="15"/></collision>
  </npc>"#
    )
}

fn items_xml() -> String {
    let collars: String = [WOLF_COLLAR, GREAT_WOLF_COLLAR, GRADE_COLLAR, EVOLVED_COLLAR]
        .into_iter()
        .map(collar_item)
        .collect();
    format!("<list>{collars}</list>")
}

fn npcs_xml() -> String {
    let pets: String = [WOLF_NPC, GREAT_WOLF_NPC, GRADE_NPC, EVOLVED_NPC]
        .into_iter()
        .map(pet_npc)
        .collect();
    format!(
        r#"<?xml version="1.0"?><list>{pets}
<npc id="{PET_MANAGER}" type="PetManager" level="70" name="Pet Manager">
    <race>HUMAN</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="FIST" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#
    )
}

fn pet_level(level: i32, exp: i64) -> String {
    format!(
        r#"<stat level="{level}"><set name="max_meal" val="20"/><set name="exp" val="{exp}"/>
        <set name="get_exp_type" val="70"/><set name="consume_meal_in_battle" val="3"/>
        <set name="consume_meal_in_normal" val="2"/><set name="org_pattack" val="10"/>
        <set name="org_pdefend" val="20"/><set name="org_mattack" val="15"/><set name="org_mdefend" val="25"/>
        <set name="org_hp" val="{}"/><set name="org_mp" val="80"/><set name="org_hp_regen" val="6"/>
        <set name="org_mp_regen" val="3"/><set name="soulshot_count" val="2"/>
        <set name="spiritshot_count" val="1"/></stat>"#,
        100 + level * 10
    )
}

/// The exp a level starts at in the lower forms (levels 1 to 60): `(level - 1)^2 * 10`.
fn low_exp(level: i32) -> i64 {
    i64::from((level - 1) * (level - 1)) * 10
}

/// The exp a level starts at in the evolved forms (levels 55 to 60): 30000 and 5000 per level.
/// Level 55 here is above level 55 of the lower forms (29160): the "non-linear baby pet exp".
fn high_exp(level: i32) -> i64 {
    30_000 + i64::from(level - 55) * 5_000
}

fn pet_data() -> Arc<PetDataTable> {
    let low: String = (1..=60).map(|l| pet_level(l, low_exp(l))).collect();
    let high: String = (55..=60).map(|l| pet_level(l, high_exp(l))).collect();
    let pet = |npc: i32, item: i32, levels: &str| {
        format!(r#"<pet id="{npc}" itemId="{item}"><stats>{levels}</stats></pet>"#)
    };
    Arc::new(
        PetDataTable::from_xml(&format!(
            "<pets>{}{}{}{}</pets>",
            pet(WOLF_NPC, WOLF_COLLAR, &low),
            pet(GREAT_WOLF_NPC, GREAT_WOLF_COLLAR, &high),
            pet(GRADE_NPC, GRADE_COLLAR, &low),
            pet(EVOLVED_NPC, EVOLVED_COLLAR, &high),
        ))
        .unwrap(),
    )
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let template = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: template.display_id,
            type2: template.type2 as i16,
            body_part: 0,
            quest_item: template.quest_item,
            weight: template.weight,
            shared_reuse_group: template.shared_reuse_group,
            weapon: template.is_weapon(),
        })
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(&items_xml()).unwrap()),
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
    }
}

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=81)
        .map(|l| format!(r#"<experience level="{l}" tolevel="{}" />"#, i64::from(l) * 1000))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="80" maxPetLevel="60">{rows}</table>"#
        ))
        .unwrap(),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const CONTROL: ObjectId = ObjectId(0x2100_0001);

fn collar(item_id: i32, enchant_level: i32) -> ItemData {
    ItemData {
        object_id: CONTROL,
        item_id,
        count: 1,
        loc: "INVENTORY".into(),
        enchant_level,
        ..Default::default()
    }
}

fn player_data(items: Vec<ItemData>, profile: PlayerProfile) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Owner".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 70,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 1000.0,
        max_hp: 1000.0,
        cur_mp: 500.0,
        max_mp: 500.0,
        cur_cp: 200.0,
        max_cp: 200.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile,
    }
}

struct Scene {
    world: World,
    me: Log,
    persisted: Persisted,
    manager: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    /// The owner holds the collar `item_id` (shown as level `enchant`) whose pet is at `level`
    /// with the exp, name and food the row says; the pet manager stands next to him.
    fn new(item_id: i32, enchant: i32, level: i32, exp: i64, name: Option<&str>) -> Self {
        let row = SavedPet {
            control_object_id: CONTROL,
            name: name.map(str::to_owned),
            level,
            cur_hp: 100,
            cur_mp: 50,
            exp,
            sp: 0,
            fed: 7,
            restore: false,
        };
        let profile = PlayerProfile {
            pet_rows: vec![row],
            ..PlayerProfile::default()
        };
        Self::build(vec![collar(item_id, enchant)], profile)
    }

    fn build(items: Vec<ItemData>, profile: PlayerProfile) -> Self {
        // The pet manager's pages: the first one holds every link the tests click (a bypass is
        // only accepted when a page the player was shown contains it); the others name themselves.
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        let links: String = [
            "evolve 3",
            "evolve 4",
            "evolve 9",
            "evolve x",
            "restore 1",
            "exchange 1",
        ]
        .iter()
        .map(|c| format!(r#"<a action="bypass -h npc_%objectId%_{c}">{c}</a><br>"#))
        .collect();
        write(
            &format!("data/html/petmanager/{PET_MANAGER}.htm"),
            &format!("<html><body>PAGE=petmanager/{PET_MANAGER}.htm;{links}</body></html>"),
        );
        for page in ["evolve_no", "restore_no", "exchange_no"] {
            write(
                &format!("data/html/petmanager/{page}.htm"),
                &format!("<html><body>PAGE=petmanager/{page}.htm;</body></html>"),
            );
        }
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(&npcs_xml(), &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::default()),
            Arc::new(KarmaLoss::default()),
            Arc::new(SkillTrees::default()),
        );
        world.set_pet_skill_data(Arc::new(
            PetSkillData::from_xml("<list></list>", &|_, _| true).unwrap(),
        ));
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_pet_data(pet_data());
        world.set_pet_settings(PetSettings {
            player_spawn_protection: 600,
            ..PetSettings::default()
        });
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(items, profile)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let manager = world.spawn_npc_at(PET_MANAGER, X + 50, Y, Z, 0).unwrap();
        let mut scene = Self {
            world,
            me,
            persisted,
            manager,
            _root: root,
        };
        scene.advance(1000);
        // The player talked to the manager: the links of its page are valid now.
        scene.world.show_chat_window_default(manager, ME);
        scene.me.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// The collar double clicked, then the summon is over.
    fn use_collar(&mut self) -> ObjectId {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::UseItem {
                object_id: CONTROL.0,
                ctrl_pressed: false,
            },
        });
        self.advance(1000);
        let pet = self.world.summon_of(ME).expect("a pet");
        self.me.take();
        self.persisted.summon_writes();
        pet
    }

    /// The player clicks a link of the manager's page (shown again first: every page that
    /// reaches the player replaces the links he may click).
    fn bypass(&mut self, command: &str) {
        self.world.show_chat_window_default(self.manager, ME);
        self.me.take();
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Bypass {
                command: format!("npc_{}_{command}", self.manager.0),
            },
        });
    }

    fn items(&self) -> Vec<ItemData> {
        self.world.player(ME).unwrap().data.items.clone()
    }

    fn has_item(&self, item_id: i32) -> bool {
        self.items().iter().any(|i| i.item_id == item_id)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn of<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

fn pages(packets: &[Vec<u8>]) -> Vec<String> {
    of(packets, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|p| decode::<NpcHtmlMessage>(p).html)
        .collect()
}

/// Whether the pet manager sent the page `petmanager/<file>` (the test pages name themselves).
fn page_is(packets: &[Vec<u8>], file: &str) -> bool {
    pages(packets)
        .iter()
        .any(|p| p.starts_with(&format!("<html><body>PAGE=petmanager/{file};")))
}

fn level_of(world: &World, id: ObjectId) -> i32 {
    world.stats(id).unwrap().level()
}

/// `doEvolve`: the pet of 6648 at level 56 becomes the pet of 10311. The old collar goes without
/// a "has disappeared" message (`destroyControlItem(owner, true)`), the new collar shows the level
/// the pet starts at, the pet keeps its name and its exp (30250 is above the 30000 the new level
/// 55 needs), is full of HP, MP and food, and stands where the old one stood.
#[test]
fn a_pet_evolves_and_keeps_its_exp_and_name() {
    let mut s = Scene::new(WOLF_COLLAR, 56, 56, low_exp(56), Some("Buffy"));
    let old = s.use_collar();
    let spot = s.world.position_of(old).unwrap();
    assert_eq!(level_of(&s.world, old), 56);

    s.bypass("evolve 3");
    s.advance(150);
    let mine = s.me.take();

    assert!(!s.has_item(WOLF_COLLAR), "the old collar is the fee");
    let new_collar = s
        .items()
        .into_iter()
        .find(|i| i.item_id == GREAT_WOLF_COLLAR)
        .expect("the new collar");
    let pet = s.world.summon_of(ME).expect("the new pet");
    assert_ne!(pet, old);
    assert!(!s.world.is_summon(old), "the old pet is gone");
    assert_eq!(s.world.summon_npc_id(pet), Some(GREAT_WOLF_NPC));
    assert_eq!(s.world.pet_control_object_id(pet), Some(new_collar.object_id));
    // Level 55 of the new pet: the exp 30250 of the old pet is above 30000 (the new minimum).
    assert_eq!(level_of(&s.world, pet), 55);
    assert_eq!(s.world.summon(pet).unwrap().exp, low_exp(56));
    assert_eq!(
        new_collar.enchant_level, 55,
        "`item.setEnchantLevel(petSummon.getLevel())`"
    );
    // Name, title, food.
    let summon = s.world.summon(pet).unwrap();
    assert_eq!(summon.name, "Buffy");
    assert_eq!(summon.title, "Owner");
    assert_eq!((s.world.pet_current_fed(pet), s.world.pet_max_fed(pet)), (20, 20));
    // The pet appears where the old pet was.
    assert_eq!(s.world.position_of(pet), Some(spot));
    // The collar of a named pet is flagged as named.
    assert_eq!(new_collar.custom_type2, 1);

    // Messages and packets: "Summoning your pet." and the skill animation, no "disappeared".
    let ids = message_ids(&mine);
    assert!(ids.contains(&SystemMessageId::SummoningYourPet), "{ids:?}");
    assert!(!ids.contains(&SystemMessageId::S1HasDisappeared));
    let use_packet: MagicSkillUse = decode(of(&mine, "MAGIC_SKILL_USE")[0]);
    assert_eq!(
        (
            use_packet.creature_id,
            use_packet.target_id,
            use_packet.skill_id,
            use_packet.skill_level
        ),
        (s.manager.0, s.manager.0, 2046, 1)
    );
    assert_eq!((use_packet.hit_time, use_packet.reuse_delay), (1000, 600_000));
    // The removed collar is announced to the owner.
    let removed: Vec<i32> = of(&mine, "INVENTORY_UPDATE")
        .into_iter()
        .flat_map(|p| decode::<InventoryUpdate>(p).updates)
        .filter(|u| u.change == InventoryUpdate::REMOVED)
        .map(|u| u.item.object_id)
        .collect();
    assert_eq!(removed, vec![CONTROL.0]);
    assert!(!page_is(&mine, "evolve_no.htm"), "{:?}", pages(&mine));

    // The rows: the old `pets` row goes, the new one is inserted with the name and the exp.
    let writes = s.persisted.summon_writes();
    assert!(writes.contains(&SummonWrite::DeletePet {
        control_object_id: CONTROL
    }));
    let stored = writes
        .iter()
        .find_map(|w| match w {
            SummonWrite::StorePet { insert: true, pet } if pet.control_object_id == new_collar.object_id => {
                Some(pet.clone())
            }
            _ => None,
        })
        .expect("the new row");
    assert_eq!(stored.name.as_deref(), Some("Buffy"));
    assert_eq!((stored.level, stored.exp, stored.fed), (55, low_exp(56), 20));
    assert_eq!(stored.owner, ME);

    // 900 ms after the bypass `EvolveFinalizer`: the skill is over, the pet follows, no more animation.
    assert!(of(&mine, "MAGIC_SKILL_LAUNCHED").is_empty(), "not before 900 ms");
    s.advance(1000);
    let later = s.me.take();
    assert_eq!(of(&later, "MAGIC_SKILL_LAUNCHED").len(), 1);
    let summon = s.world.summon(pet).unwrap();
    assert!(summon.follow && !summon.show_summon_animation);
}

/// "Fix for non-linear baby pet exp": an old pet whose exp is below the new pet's level 55 gets
/// the exp of that level.
#[test]
fn the_evolved_pet_gets_at_least_the_exp_of_the_minimum_level() {
    let mut s = Scene::new(WOLF_COLLAR, 55, 55, low_exp(55), None);
    let old = s.use_collar();
    assert_eq!(s.world.summon(old).unwrap().exp, low_exp(55));
    s.bypass("evolve 3");
    let pet = s.world.summon_of(ME).unwrap();
    assert_eq!(s.world.summon(pet).unwrap().exp, high_exp(55));
    assert_eq!(level_of(&s.world, pet), 55);
    // An unnamed pet stays unnamed, and its collar is not flagged.
    assert_eq!(s.world.summon(pet).unwrap().name, "");
    let collar = s
        .items()
        .into_iter()
        .find(|i| i.item_id == GREAT_WOLF_COLLAR)
        .unwrap();
    assert_eq!(collar.custom_type2, 0);
}

/// Every refusal is the "no" page and changes nothing: no pet, a pet below the level, another
/// pet, a bypass that names no evolution.
#[test]
fn the_refusals_show_the_no_page_and_change_nothing() {
    // No pet at all.
    let mut s = Scene::new(WOLF_COLLAR, 56, 56, low_exp(56), None);
    s.bypass("evolve 3");
    assert!(page_is(&s.me.take(), "evolve_no.htm"));
    assert!(s.has_item(WOLF_COLLAR));

    // A pet one level below the requirement.
    let mut s = Scene::new(WOLF_COLLAR, 54, 54, low_exp(54), None);
    let pet = s.use_collar();
    s.bypass("evolve 3");
    assert!(page_is(&s.me.take(), "evolve_no.htm"));
    assert_eq!(s.world.summon_of(ME), Some(pet));
    assert!(s.has_item(WOLF_COLLAR) && !s.has_item(GREAT_WOLF_COLLAR));
    assert!(s.persisted.summon_writes().is_empty());

    // The pet of another collar: evolve 4 takes 6650. (The bypass flood protector wants a pause.)
    s.advance(1000);
    s.bypass("evolve 4");
    assert!(page_is(&s.me.take(), "evolve_no.htm"));
    assert_eq!(s.world.summon_of(ME), Some(pet));

    // A number that is no evolution, and one that is not a number at all.
    s.advance(1000);
    s.bypass("evolve 9");
    assert!(page_is(&s.me.take(), "evolve_no.htm"));
    s.advance(1000);
    s.bypass("evolve x");
    assert!(pages(&s.me.take()).is_empty());
    assert_eq!(s.world.summon_of(ME), Some(pet));
}

/// A dead pet is the "death pet exploit": `handleIllegalPlayerAction` and the "no" page.
#[test]
fn a_dead_pet_cannot_be_evolved() {
    let mut s = Scene::new(WOLF_COLLAR, 56, 56, low_exp(56), None);
    let pet = s.use_collar();
    s.world.do_die(pet, None);
    s.me.take();
    s.bypass("evolve 3");
    let mine = s.me.take();
    assert!(page_is(&mine, "evolve_no.htm"));
    let texts: Vec<String> = of(&mine, "SYSTEM_MESSAGE")
        .into_iter()
        .flat_map(|p| decode::<SystemMessage>(p).params)
        .filter_map(|p| match p {
            SystemMessageParam::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("You will be kicked for illegal action")),
        "{texts:?}"
    );
    assert!(s.has_item(WOLF_COLLAR) && !s.has_item(GREAT_WOLF_COLLAR));
}

/// `doRestore`: the collar 10307 at level 58 becomes the collar 9882 whose pet is summoned at
/// that level with the exp of the level; the old collar goes with a "has disappeared" message
/// and its `pets` row.
#[test]
fn a_restore_gives_the_old_form_back_at_the_level_of_the_collar() {
    let mut s = Scene::build(vec![collar(EVOLVED_COLLAR, 58)], PlayerProfile::default());
    s.bypass("restore 1");
    s.advance(150);
    let mine = s.me.take();

    assert!(!s.has_item(EVOLVED_COLLAR));
    let new_collar = s
        .items()
        .into_iter()
        .find(|i| i.item_id == GRADE_COLLAR)
        .expect("the new collar");
    let pet = s.world.summon_of(ME).expect("the restored pet");
    assert_eq!(s.world.summon_npc_id(pet), Some(GRADE_NPC));
    assert_eq!(level_of(&s.world, pet), 58);
    assert_eq!(s.world.summon(pet).unwrap().exp, low_exp(58));
    assert_eq!(new_collar.enchant_level, 58);
    assert_eq!(s.world.summon(pet).unwrap().title, "Owner");
    assert_eq!((s.world.pet_current_fed(pet), s.world.pet_max_fed(pet)), (20, 20));
    // "Has disappeared" for the old collar, then "Summoning your pet.".
    let ids = message_ids(&mine);
    let gone = ids
        .iter()
        .position(|i| *i == SystemMessageId::S1HasDisappeared)
        .unwrap();
    let summoning = ids
        .iter()
        .position(|i| *i == SystemMessageId::SummoningYourPet)
        .unwrap();
    assert!(gone < summoning, "{ids:?}");
    assert_eq!(of(&mine, "MAGIC_SKILL_USE").len(), 1);
    let writes = s.persisted.summon_writes();
    assert!(writes.contains(&SummonWrite::DeletePet {
        control_object_id: CONTROL
    }));
    assert!(writes.iter().any(|w| matches!(
        w,
        SummonWrite::StorePet { insert: true, pet } if pet.control_object_id == new_collar.object_id
    )));
    s.advance(1000);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_LAUNCHED").len(), 1);
}

/// A collar that shows a level below the requirement is restored at the requirement.
#[test]
fn a_restore_is_at_least_at_the_minimum_level() {
    let mut s = Scene::build(vec![collar(EVOLVED_COLLAR, 10)], PlayerProfile::default());
    s.bypass("restore 1");
    let pet = s.world.summon_of(ME).expect("the restored pet");
    assert_eq!(level_of(&s.world, pet), 55);
    assert_eq!(s.world.summon(pet).unwrap().exp, low_exp(55));
}

/// Without the collar (or with a collar that has no pet data) `restore` is the "no" page.
#[test]
fn a_restore_needs_the_collar() {
    let mut s = Scene::build(vec![collar(WOLF_COLLAR, 56)], PlayerProfile::default());
    s.bypass("restore 1");
    assert!(page_is(&s.me.take(), "restore_no.htm"));
    assert!(s.has_item(WOLF_COLLAR));
    assert!(s.world.summon_of(ME).is_none());
}
