#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Fishing (M6-FU-08): the `Fishing` skill (the checks, the place of the bait, the cast), the bite
//! task, the fight (`Fishing.java`) with pumping and reeling, the catch, the fishing shots, the
//! lure going to the left hand, what a fisher cannot do, and the championship.
//!
//! The expected messages, packets and numbers are read off the Java sources named in each test:
//! `mechanics/fishing/Fishing.java`, `entity/actor/Player.java` (`startFishing`, `endFishing`, ...),
//! `dist/game/data/scripts/handlers/skill/effects/{Fishing,Pumping,Reeling}.java`,
//! `handlers/items/FishShots.java`, `managers/FishingChampionshipManager.java`.
//! Java: mechanics/fishing/Fishing.java

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, FishData, FishingMonstersData, FishingRodsData, Hennas, HtmCache,
    ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, SpawnData, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    CharInfo, ExFishingEnd, ExFishingHpRegen, ExFishingStart, ExFishingStartCombat, MagicSkillUse,
    NpcHtmlMessage, PlaySound, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- the data ----------------------------------------------------------------------------------

/// One class; HP 1000, MP 500 and CP 20 at every level.
fn class_xml() -> String {
    let rows: String = (1..=85)
        .map(|l| format!("<level val=\"{l}\"><hp>1000</hp><mp>500</mp><cp>20</cp></level>"))
        .collect();
    format!(
        r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="6529" type="Weapon" name="Baby Duck Rod">
	<set name="weapon_type" val="FISHINGROD" /><set name="bodypart" val="lrhand" />
	<set name="weight" val="1000" /><set name="isAttackWeapon" val="false" />
</item>
<item id="6530" type="Weapon" name="Albatross Rod">
	<set name="weapon_type" val="FISHINGROD" /><set name="bodypart" val="lrhand" />
	<set name="crystal_type" val="D" /><set name="weight" val="1000" /><set name="isAttackWeapon" val="false" />
</item>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="5" /></item>
<item id="6519" type="EtcItem" name="Green Colored Lure - Low Grade">
	<set name="etcitem_type" val="LURE" /><set name="bodypart" val="lhand" /><set name="weight" val="5" />
	<set name="is_stackable" val="true" />
</item>
<item id="7807" type="EtcItem" name="Green Colored Lure - For Beginners">
	<set name="etcitem_type" val="LURE" /><set name="bodypart" val="lhand" /><set name="weight" val="5" />
	<set name="is_stackable" val="true" />
</item>
<item id="8506" type="EtcItem" name="Green Luminous Lure">
	<set name="etcitem_type" val="LURE" /><set name="bodypart" val="lhand" /><set name="weight" val="5" />
	<set name="is_stackable" val="true" />
</item>
<item id="6411" type="EtcItem" name="Small Nimble Green Fish"><set name="weight" val="1" /></item>
<item id="6412" type="EtcItem" name="Small Ugly Green Fish"><set name="weight" val="1" /></item>
<item id="6413" type="EtcItem" name="Small Pudgy Green Fish"><set name="weight" val="1" /></item>
<item id="8547" type="EtcItem" name="Old Box"><set name="weight" val="1" /></item>
<item id="1060" type="EtcItem" name="Potion"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="6535" type="EtcItem" name="Fishing Shot (No-Grade)">
	<set name="default_action" val="FISHINGSHOT" /><set name="weight" val="3" />
	<set name="is_stackable" val="true" /><set name="handler" val="FishShots" />
	<skills><skill id="2181" level="1" /></skills>
</item>
<item id="6536" type="EtcItem" name="Fishing Shot (D-Grade)">
	<set name="default_action" val="FISHINGSHOT" /><set name="crystal_type" val="D" /><set name="weight" val="3" />
	<set name="is_stackable" val="true" /><set name="handler" val="FishShots" />
	<skills><skill id="2181" level="1" /></skills>
</item>
</list>"#;

/// The datapack's fishing skills (`01300-01399.xml`) and Fisherman's Potion - Green (`2274`, only
/// the first level), with the comments left out. 9001 is a plain self buff.
const SKILLS: &str = r##"<list>
<skill id="1312" levels="1" name="Fishing">
	<icon>icon.skill1312</icon>
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<mpConsume>1</mpConsume>
	<conditions msgId="113" addName="1">
		<using kind="FISHINGROD" />
	</conditions>
	<effects>
		<effect name="Fishing" />
	</effects>
</skill>
<skill id="1313" levels="27" name="Pumping">
	<table name="#power">0.66 0.9375 1.26 1.55 1.7 1.85 2 2.15 2.3 2.45 2.6 2.75 2.9 3.05 3.2 3.35 3.5 3.65 3.8 3.95 4.1 4.25 4.4 4.55 4.7 4.85 5</table>
	<icon>icon.skill1313</icon>
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<mpConsume>2</mpConsume>
	<reuseDelay>2000</reuseDelay>
	<conditions msgId="113" addName="1">
		<using kind="FISHINGROD" />
	</conditions>
	<effects>
		<effect name="Pumping">
			<power>#power</power>
		</effect>
	</effects>
</skill>
<skill id="1314" levels="27" name="Reeling">
	<table name="#power">0.66 0.9375 1.26 1.55 1.7 1.85 2 2.15 2.3 2.45 2.6 2.75 2.9 3.05 3.2 3.35 3.5 3.65 3.8 3.95 4.1 4.25 4.4 4.55 4.7 4.85 5</table>
	<icon>icon.skill1314</icon>
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<mpConsume>2</mpConsume>
	<reuseDelay>2000</reuseDelay>
	<conditions msgId="113" addName="1">
		<using kind="FISHINGROD" />
	</conditions>
	<effects>
		<effect name="Reeling">
			<power>#power</power>
		</effect>
	</effects>
</skill>
<skill id="1315" levels="27" name="Fishing Expertise">
	<table name="#power">0.66 0.9375 1.26 1.55 1.7 1.85 2 2.15 2.3 2.45 2.6 2.75 2.9 3.05 3.2 3.35 3.5 3.65 3.8 3.95 4.1 4.25 4.4 4.55 4.7 4.85 5</table>
	<icon>icon.skill1315</icon>
	<operateType>P</operateType>
	<targetType>SELF</targetType>
	<magicLevel>1</magicLevel>
	<effects>
		<effect name="Buff">
			<set stat="fishingExpertise">#power</set>
		</effect>
	</effects>
</skill>
<skill id="2274" levels="8" name="Fisherman's Potion - Green">
	<table name="#power">2 5 8 11 14 17 20 23</table>
	<icon>icon.etc_oblivion_green_i00</icon>
	<operateType>A2</operateType>
	<targetType>SELF</targetType>
	<abnormalLevel>1</abnormalLevel>
	<abnormalTime>3600</abnormalTime>
	<abnormalType>FISHING_MASTERY_DOWN</abnormalType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<power>#power</power>
	<effects>
		<effect name="ChangeFishingMastery" />
	</effects>
</skill>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime><isMagic>1</isMagic><mpConsume>5</mpConsume>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
</list>"##;

const NPCS: &str = r#"<list>
<npc id="18322" type="Monster" level="10" name="Fishing Monster">
	<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
		<vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
		<attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
		<defence physical="44.44444" magical="29.5" />
		<speed><walk ground="20" /><run ground="50" /></speed>
	</stats>
	<collision><radius normal="10"/><height normal="20"/></collision>
</npc>
<npc id="31561" type="Fisherman" level="1" name="Fisher Guild">
	<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
		<vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
		<attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
		<defence physical="44.44444" magical="29.5" />
		<speed><walk ground="20" /><run ground="50" /></speed>
	</stats>
	<collision><radius normal="10"/><height normal="20"/></collision>
</npc>
</list>"#;

const RODS: &str = r#"<list>
<fishingRod fishingRodId="1" fishingRodItemId="6529" fishingRodLevel="20" fishingRodName="Baby Duck Rod" fishingRodDamage="20.0" />
<fishingRod fishingRodId="2" fishingRodItemId="6530" fishingRodLevel="40" fishingRodName="Albatross Rod" fishingRodDamage="23.0" />
</list>"#;

/// The fishing monster of the datapack for levels 36 to 45, with a chance of `probability`.
fn monsters(probability: i32) -> String {
    format!(
        r#"<list><fishingMonster userMinLevel="6" userMaxLevel="15" fishingMonsterId="18322" probability="{probability}" /></list>"#
    )
}

/// One `<fish>` line.
#[allow(clippy::too_many_arguments)]
fn fish_line(
    id: i32,
    item: i32,
    group: &str,
    level: i32,
    grade: &str,
    guts: f64,
    hp: i32,
    regen: f64,
    start: i32,
    duration: i32,
) -> String {
    format!(
        r#"<fish fishId="{id}" itemId="{item}" itemName="Fish{id}" fishGroup="{group}" fishLevel="{level}" fishBiteRate="100.0" fishGuts="{guts}" fishHp="{hp}" fishMaxLength="30000" fishLengthRate="10.0" hpRegen="{regen}" startCombatTime="{start}" combatDuration="{duration}" gutsCheckTime="5" gutsCheckProbability="50.0" cheatingProb="0.0" fishGrade="{grade}" />"#
    )
}

/// What the fish of a test are like. `guts` 100 means every check is a bite.
#[derive(Clone)]
struct Fish {
    guts: f64,
    hp: i32,
    regen: f64,
    start: i32,
    duration: i32,
}

impl Default for Fish {
    fn default() -> Self {
        Self {
            guts: 100.0,
            hp: 1000,
            regen: 4.0,
            start: 20,
            duration: 600,
        }
    }
}

/// Every group of the lures of the tests, for the levels 1 to 3 (the expertise skill gives 1 or
/// 2, the potion up to 24): normal, beginner and luminous grades.
fn fish_xml(f: &Fish) -> String {
    let mut lines = String::new();
    let mut id = 0;
    for level in 1..=27 {
        for (group, grade, item) in [
            ("wide", "fish_normal", 6413),
            ("swift", "fish_normal", 6411),
            ("ugly", "fish_normal", 6412),
            ("fish_box", "fish_normal", 8547),
            ("easy_wide", "fish_easy", 6413),
            ("easy_swift", "fish_easy", 6411),
            ("easy_ugly", "fish_easy", 6412),
            ("hard_wide", "fish_hard", 6413),
            ("hard_swift", "fish_hard", 6411),
            ("hard_ugly", "fish_hard", 6412),
        ] {
            id += 1;
            // HP follows the level so that a test can tell which level was drawn.
            lines += &fish_line(
                id,
                item,
                group,
                level,
                grade,
                f.guts,
                f.hp * level,
                f.regen,
                f.start,
                f.duration,
            );
        }
    }
    format!("<list>{lines}</list>")
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// The Z of the water zone: the bait lands there.
const WATER_Z: i32 = -3_200;
const ME: i32 = 0x2000_0001;
const P2: i32 = 0x2000_0002;

const ROD: i32 = 6529;
const D_ROD: i32 = 6530;
const LURE: i32 = 6519;
const BEGINNER_LURE: i32 = 7807;
const LUMINOUS_LURE: i32 = 8506;
const FISHING: i32 = 1312;
const PUMPING: i32 = 1313;
const REELING: i32 = 1314;
const EXPERTISE: i32 = 1315;
const POTION: i32 = 2274;
const MIGHT: i32 = 9001;

const ROD_OBJECT: i32 = 0x1000_0001;
const LURE_OBJECT: i32 = 0x1000_0002;
const SHOT_OBJECT: i32 = 0x1000_0003;
const OTHER_OBJECT: i32 = 0x1000_0004;

// ---- the fixture -------------------------------------------------------------------------------

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A datapack folder with the zones, the map region and the pages of the championship.
struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

/// The parts of a scene that do not change between seeds.
struct Fixture {
    zones: Arc<Zones>,
    regions: Arc<MapRegions>,
    html: Arc<HtmCache>,
    items: Arc<ItemTemplates>,
    skills: Arc<SkillTemplates>,
    npcs: Arc<NpcTemplates>,
    _root: Root,
}

impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "l2-world-fishing-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let zone = |id: i32, kind: &str, max_z: i32| {
            format!(
                "<zone id=\"{id}\" name=\"z{id}\" type=\"{kind}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"{max_z}\">\
                 <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
                X - 500,
                Y - 500,
                X + 500,
                Y + 500
            )
        };
        write(
            &dir,
            "data/zones/test.xml",
            &format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{}{}</list>\n",
                zone(1, "FishingZone", -3_000),
                zone(2, "WaterZone", WATER_Z)
            ),
        );
        write(
            &dir,
            "data/mapregion/talking_island_town.xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n</region>\n</list>\n",
        );
        // The pages the championship fills in (the datapack's pages have the same placeholders).
        let page = |text: &str| format!("<html><body>{text}</body></html>");
        write(
            &dir,
            "data/html/fisherman/championship/fish_event001.htm",
            &page(
                "WINNERS %refresh% min %objectId% <a action=\"bypass -h npc_%objectId%_FishingReward\">reward</a> %prizeItem% %prizeFirst% %prizeTwo% %prizeThree% %prizeFour% %prizeFive% <table>%TABLE%</table>",
            ),
        );
        write(
            &dir,
            "data/html/fisherman/championship/fish_event002.htm",
            &page(
                "RANKING %prizeItem% %prizeFirst% %prizeTwo% %prizeThree% %prizeFour% %prizeFive% <table>%TABLE%</table>",
            ),
        );
        write(
            &dir,
            "data/html/fisherman/championship/fish_event003.htm",
            &page("PLEASE WAIT"),
        );
        write(
            &dir,
            "data/html/fisherman/championship/fish_event_reward001.htm",
            &page("REWARD"),
        );
        write(
            &dir,
            "data/html/fisherman/championship/no_fish_event001.htm",
            &page("NO EVENT"),
        );
        write(
            &dir,
            "data/html/fisherman/championship/no_fish_event_reward001.htm",
            &page("NO REWARD FOR YOU"),
        );
        write(
            &dir,
            "data/html/fisherman/31561.htm",
            &page(
                "<a action=\"bypass -h npc_%objectId%_FishingChampionship\">ranking</a> <a action=\"bypass -h npc_%objectId%_FishingReward\">reward</a>",
            ),
        );
        Self {
            zones: Arc::new(Zones::load(&dir)),
            regions: Arc::new(MapRegions::load(&dir)),
            html: Arc::new(HtmCache::load(&dir, false)),
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            skills: Arc::new(SkillTemplates::from_xml(
                SKILLS,
                &EnchantSkillGroups::default(),
                &SkillLoadOptions::default(),
            )),
            npcs: Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            _root: Root(dir),
        }
    }
}

// ---- the scene ---------------------------------------------------------------------------------

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
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

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.0.skills.iter().any(|s| s.id == skill_id && s.is_passive())
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

/// What a scene is made of.
#[derive(Clone)]
struct Opts {
    seed: u64,
    level: i32,
    /// The level of the expertise skill (0: not known).
    expertise: i32,
    pumping: i32,
    reeling: i32,
    /// The lure in the left hand (0: none).
    lure: i32,
    /// The rod in the hands (0: none).
    rod: i32,
    zones: bool,
    night: bool,
    fish: Fish,
    monster_chance: i32,
    fishing: FishingSettings,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            seed: 1,
            level: 10,
            expertise: 1,
            pumping: 1,
            reeling: 1,
            lure: LURE,
            rod: ROD,
            zones: true,
            night: false,
            fish: Fish::default(),
            monster_chance: -1,
            fishing: FishingSettings::default(),
        }
    }
}

fn item(object_id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: loc.into(),
        loc_data,
        ..Default::default()
    }
}

fn player_data(id: i32, name: &str, opts: &Opts) -> PlayerData {
    let mut items = Vec::new();
    if opts.rod != 0 {
        items.push(item(ROD_OBJECT, opts.rod, 1, "PAPERDOLL", 5));
    }
    if opts.lure != 0 {
        items.push(item(LURE_OBJECT, opts.lure, 10, "PAPERDOLL", 7));
    }
    items.push(item(SHOT_OBJECT, 6535, 10, "INVENTORY", 0));
    items.push(item(OTHER_OBJECT, 1060, 5, "INVENTORY", 0));
    let mut skills = vec![SkillData {
        skill_id: FISHING,
        skill_level: 1,
    }];
    for (skill_id, level) in [
        (PUMPING, opts.pumping),
        (REELING, opts.reeling),
        (EXPERTISE, opts.expertise),
    ] {
        if level > 0 {
            skills.push(SkillData {
                skill_id,
                skill_level: level,
            });
        }
    }
    skills.push(SkillData {
        skill_id: MIGHT,
        skill_level: 1,
    });
    skills.push(SkillData {
        skill_id: POTION,
        skill_level: 7,
    });
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: opts.level,
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
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills,
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
}

impl Scene {
    fn new(fx: &Fixture, opts: &Opts) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(opts.seed), Box::new(persisted.clone()));
        let mut settings = WorldSettings::default();
        settings.vitality.enable_vitality = false;
        settings.fishing = FishingSettings {
            clock_override_night: Some(opts.night),
            ..opts.fishing.clone()
        };
        world.set_settings(settings);
        world.set_static_data(Box::new(Data(StatData {
            items: Arc::clone(&fx.items),
            player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::clone(&fx.skills),
        })));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            cp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(Arc::clone(&fx.npcs), Arc::new(SpawnData::default()));
        if opts.zones {
            world.set_zone_data(Arc::clone(&fx.zones), Arc::clone(&fx.regions));
        }
        world.set_html_cache(Arc::clone(&fx.html));
        world.set_fishing_data(
            Arc::new(FishData::from_xml(&fish_xml(&opts.fish)).unwrap()),
            Arc::new(FishingRodsData::from_xml(RODS).unwrap()),
            Arc::new(FishingMonstersData::from_xml(&monsters(opts.monster_chance)).unwrap()),
        );
        let mut scene = Self {
            world,
            log,
            persisted,
        };
        scene.enter(ME, "Fisher", opts);
        scene.ticks(20);
        scene.log.take();
        scene
    }

    fn enter(&mut self, id: i32, name: &str, opts: &Opts) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, opts)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// Runs the world for `ms` milliseconds and returns everything sent in that time.
    fn run(&mut self, ms: u64) -> Sent {
        let mut all = Vec::new();
        for _ in 0..(ms / 50) {
            self.world.tick();
            all.extend(self.log.take());
        }
        all
    }

    fn use_skill(&mut self, skill_id: i32) {
        self.input(
            ME,
            ClientInput::MagicSkillUse {
                magic_id: skill_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        );
    }

    /// Casts the line and lets the cast finish: everything sent meanwhile.
    fn cast(&mut self) -> Sent {
        self.log.take();
        self.use_skill(FISHING);
        self.run(1000)
    }

    /// Runs until the fish bites; `None` when it does not within `max_ms`.
    fn wait_for_bite(&mut self, max_ms: u64) -> Option<(Sent, ExFishingStartCombat)> {
        let mut all = Vec::new();
        for _ in 0..(max_ms / 50) {
            self.world.tick();
            let got = self.log.take();
            let combat = of::<ExFishingStartCombat>(&got, ME, "EX_FISHING_START_COMBAT");
            all.extend(got);
            if let Some(c) = combat.into_iter().next() {
                return Some((all, c));
            }
        }
        None
    }

    fn me(&self) -> &Player {
        self.world.player(ObjectId(ME)).unwrap()
    }

    fn count(&self, item_id: i32) -> i64 {
        self.me()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
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

fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn of<P: Decode>(sent: &Sent, to: i32, name: &str) -> Vec<P> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    of::<SystemMessage>(sent, to, "SYSTEM_MESSAGE")
}

fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|m| m.id).collect()
}

fn regens(sent: &Sent, to: i32) -> Vec<ExFishingHpRegen> {
    of(sent, to, "EX_FISHING_HP_REGEN")
}

fn ends(sent: &Sent, to: i32) -> Vec<ExFishingEnd> {
    of(sent, to, "EX_FISHING_END")
}

/// The pages `to` got.
fn pages(sent: &Sent, to: i32) -> Vec<String> {
    of::<NpcHtmlMessage>(sent, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

/// The first int parameter of a message (`$s1` as a number).
fn int_param(m: &SystemMessage) -> i32 {
    m.params
        .iter()
        .find_map(|p| match p {
            SystemMessageParam::IntNumber(v) => Some(*v),
            _ => None,
        })
        .unwrap()
}

/// A scene whose fish bit with `wanted` (mode, deceptive mode); the packets of the cast and the
/// bite and the first packet of the fight. Seeds are tried one by one: the mode is a draw.
fn bite_with(fx: &Fixture, opts: &Opts, wanted: (u8, u8)) -> (Scene, ExFishingStartCombat) {
    for seed in 1..4000 {
        let mut scene = Scene::new(fx, &Opts { seed, ..opts.clone() });
        scene.cast();
        let Some((_, combat)) = scene.wait_for_bite(15_000) else {
            panic!("the fish did not bite (seed {seed})");
        };
        if (combat.mode, combat.deceptive_mode) == wanted {
            return (scene, combat);
        }
    }
    panic!("no seed gives the fight {wanted:?}");
}

// ================================================================================== the cast

/// `Fishing.onStart`, `Player.startFishing`: the lure is used up, the player is held, the
/// message, the `ExFishingStart` for everybody (the bait lands at the water's height, 90 to 250
/// in front of the fisher), the sound of the cast. A second player entering sees the fisher in
/// fishing mode (`CharInfo`).
#[test]
fn the_cast_uses_a_lure_holds_the_player_and_shows_the_bait() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.enter(P2, "Watcher", &Opts::default());
    s.log.take();
    s.use_skill(FISHING);
    let sent = s.run(1000);

    assert_eq!(s.count(LURE), 9, "one lure is used up");
    assert!(s.world.is_fishing(ObjectId(ME)));
    assert!(s.world.is_immobilized(ObjectId(ME)));

    let ids = message_ids(&sent, ME);
    assert!(
        ids.contains(&SystemMessageId::YouCastYourLineAndStartToFish),
        "{ids:?}"
    );

    let start: Vec<ExFishingStart> = of(&sent, ME, "EX_FISHING_START");
    assert_eq!(start.len(), 1);
    let start = start[0];
    assert_eq!(start.object_id, ME);
    // Heading 0 is east: the bait is 90 to 250 further along X, at the water's height.
    assert!((X + 90..=X + 250).contains(&start.x), "{start:?}");
    assert_eq!((start.y, start.z), (Y, WATER_Z));
    assert!(!start.night_lure);
    // Lure 6519 is a normal grade lure: fish of type 0 to 3 (never 4 and up).
    assert!((0..=3).contains(&start.fish_type), "{start:?}");
    // The watcher got the same packet, and the fisher the cast sound (`SF_P_01`).
    assert_eq!(of::<ExFishingStart>(&sent, P2, "EX_FISHING_START"), [start]);
    let sounds: Vec<PlaySound> = of(&sent, ME, "PLAY_SOUND");
    assert_eq!(
        sounds
            .iter()
            .map(|p| (p.unknown1, p.sound_file.as_str()))
            .collect::<Vec<_>>(),
        [(1, "SF_P_01")]
    );

    // A player who comes into view later is told: fishing mode and the bait's place.
    let mut late = Scene::new(&fx, &Opts::default());
    late.cast();
    late.enter(P2, "Late", &Opts::default());
    let info: Vec<CharInfo> = of(&late.log.take(), P2, "CHAR_INFO");
    assert_eq!(info.len(), 1);
    assert!(info[0].fishing);
    assert_eq!(
        (info[0].fish_y, info[0].fish_z),
        (Y, WATER_Z),
        "the bait is where ExFishingStart said"
    );
}

/// `Fishing.onStart`'s refusals, in Java's order: no rod, no lure, then (after the bait place is
/// worked out) no fishing zone. None of them uses the lure up.
#[test]
fn the_cast_is_refused_without_a_rod_a_lure_or_a_fishing_zone() {
    let fx = Fixture::new();

    // Without a fishing rod the skill's own condition (`using kind="FISHINGROD"`) refuses it.
    let mut s = Scene::new(
        &fx,
        &Opts {
            rod: 0,
            ..Opts::default()
        },
    );
    s.use_skill(FISHING);
    let sent = s.run(1000);
    assert!(!s.world.is_fishing(ObjectId(ME)));
    assert!(ends(&sent, ME).is_empty());
    assert_eq!(s.count(LURE), 10);

    // A rod and no lure.
    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: 0,
            ..Opts::default()
        },
    );
    s.use_skill(FISHING);
    let sent = s.run(1000);
    assert!(message_ids(&sent, ME).contains(&SystemMessageId::YouMustPutBaitOnYourHookBeforeYouCanFish));
    assert!(!s.world.is_fishing(ObjectId(ME)));

    // No zones at all: "You can't fish here", and the lure stays.
    let mut s = Scene::new(
        &fx,
        &Opts {
            zones: false,
            ..Opts::default()
        },
    );
    s.use_skill(FISHING);
    let sent = s.run(1000);
    assert!(message_ids(&sent, ME).contains(&SystemMessageId::YouCanTFishHere));
    assert!(!s.world.is_fishing(ObjectId(ME)));
    assert_eq!(s.count(LURE), 10);
    assert!(!s.world.is_immobilized(ObjectId(ME)));
}

/// `AllowFishing = False` refuses everybody but a GM, with Java's text.
#[test]
fn fishing_can_be_switched_off() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            fishing: FishingSettings {
                allow_fishing: false,
                ..FishingSettings::default()
            },
            ..Opts::default()
        },
    );
    s.use_skill(FISHING);
    let sent = s.run(1000);
    assert!(!s.world.is_fishing(ObjectId(ME)));
    assert_eq!(s.count(LURE), 10);
    assert!(
        messages(&sent, ME)
            .iter()
            .any(|m| m.params == [SystemMessageParam::Text("Fishing is disabled!".into())]),
        "{:?}",
        names(&sent, ME)
    );
}

/// Using the skill while fishing, with no fish on the line, cancels it: `endFishing` (the bait is
/// lost, the end packet, the reel-in message), then "Your attempt at fishing has been cancelled".
#[test]
fn casting_again_cancels_the_fishing() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.cast();
    s.log.take();
    s.use_skill(FISHING);
    let sent = s.run(1000);
    assert!(!s.world.is_fishing(ObjectId(ME)));
    assert!(!s.world.is_immobilized(ObjectId(ME)));
    assert_eq!(
        message_ids(&sent, ME),
        [
            SystemMessageId::TheBaitHasBeenLostBecauseTheFishGotAway,
            SystemMessageId::YouReelYourLineInAndStopFishing,
            SystemMessageId::YourAttemptAtFishingHasBeenCancelled,
        ]
    );
    assert_eq!(
        ends(&sent, ME),
        [ExFishingEnd {
            object_id: ME,
            win: false
        }]
    );
    // The bite task is gone with it: nothing bites later.
    assert!(s.wait_for_bite(40_000).is_none());
}

// ================================================================================== the bite

/// `LookingForFishTask`: the first check is 10 s after the cast. With a sure bite (`fishGuts`
/// 100) the fight starts then: `ExFishingStartCombat` (time, HP, mode, lure type, deceptive
/// mode), the fight sound `SF_S_01`, "You've got a bite", then once a second the HP packet.
#[test]
fn a_fish_bites_after_ten_seconds_and_the_fight_begins() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.cast();
    // Nothing before the 10 s mark (the cast took 1 s of it).
    assert!(s.wait_for_bite(8_000).is_none());
    let (sent, combat) = s.wait_for_bite(3_000).expect("a bite");

    assert_eq!(combat.object_id, ME);
    assert_eq!(combat.time, 600, "the fish's combat duration");
    // The test fish have 1000 HP per level; the expertise skill (level 1) draws level 1 or 2.
    assert!([1000, 2000].contains(&combat.hp), "{combat:?}");
    assert!(combat.mode <= 1);
    assert_eq!(combat.lure_type, 1, "a normal lure");
    assert_eq!(combat.deceptive_mode, 0, "only upper grade fish are deceptive");

    let sounds: Vec<PlaySound> = of(&sent, ME, "PLAY_SOUND");
    assert_eq!(sounds.last().map(|p| p.sound_file.as_str()), Some("SF_S_01"));
    assert!(message_ids(&sent, ME).contains(&SystemMessageId::YouVeGotABite));
    assert!(s.world.has_fish_combat(ObjectId(ME)));

    // Once a second: the HP packet, time counting down (`_time--`), only the fisher gets it.
    let sent = s.run(3_000);
    let times: Vec<i32> = regens(&sent, ME).iter().map(|r| r.time).collect();
    assert_eq!(times, [599, 598, 597]);
}

/// A beginner lure fights as `lureType 0`, a luminous one as 2 (and needs the night).
#[test]
fn the_lure_type_of_the_fight_follows_the_lure() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: BEGINNER_LURE,
            ..Opts::default()
        },
    );
    s.cast();
    let (_, combat) = s.wait_for_bite(15_000).expect("a bite");
    // Lure 7807 is for beginners: easy fish (the test data has them for every level).
    assert_eq!(combat.lure_type, 0);
    assert_eq!(combat.deceptive_mode, 0);

    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: LUMINOUS_LURE,
            night: true,
            ..Opts::default()
        },
    );
    let sent = s.cast();
    let start: Vec<ExFishingStart> = of(&sent, ME, "EX_FISHING_START");
    assert!(start[0].night_lure);
    assert!((7..=9).contains(&start[0].fish_type), "{:?}", start[0]);
    let (_, combat) = s.wait_for_bite(15_000).expect("a bite");
    assert_eq!(combat.lure_type, 2);
}

/// A lure at the wrong time of day gives a fish of type -1 that never bites; after
/// `startCombatTime` + 10 s the bait is lost (`LookingForFishTask` calls `endFishing(false)`).
#[test]
fn a_lure_at_the_wrong_time_of_day_never_bites_and_the_bait_is_lost() {
    let fx = Fixture::new();
    // The luminous lure is for the night; here it is day.
    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: LUMINOUS_LURE,
            night: false,
            ..Opts::default()
        },
    );
    let sent = s.cast();
    let start: Vec<ExFishingStart> = of(&sent, ME, "EX_FISHING_START");
    assert_eq!(start[0].fish_type, -1);
    assert!(s.wait_for_bite(25_000).is_none());
    assert!(s.world.is_fishing(ObjectId(ME)), "20 s + 10 s are not over yet");
    let sent = s.run(8_000);
    assert!(!s.world.is_fishing(ObjectId(ME)));
    assert_eq!(
        message_ids(&sent, ME),
        [
            SystemMessageId::TheBaitHasBeenLostBecauseTheFishGotAway,
            SystemMessageId::YouReelYourLineInAndStopFishing,
        ]
    );
    assert_eq!(
        ends(&sent, ME),
        [ExFishingEnd {
            object_id: ME,
            win: false
        }]
    );
}

/// `getRandomFishLvl` with Fisherman's Potion: the potion skill's power (level 7 of the skill is
/// power 20) replaces the expertise level, so the fish is of level 19, 20 or 21.
#[test]
fn a_fishermans_potion_sets_the_level_of_the_fish() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.use_skill(POTION);
    s.run(1000);
    s.cast();
    let (_, combat) = s.wait_for_bite(15_000).expect("a bite");
    assert!(
        [19_000, 20_000, 21_000].contains(&combat.hp),
        "fish of level 19 to 21 have 19000 to 21000 HP: {combat:?}"
    );
}

// ================================================================================== the fight

/// `Pumping.onStart` + `Fishing.usePumping`: pumping a resting fish (mode 0, not deceptive) works.
/// Damage: `(int) ((rod damage + FISHING_EXPERTISE + power) * (rod level * 0.1) * ss)` with the
/// rod's 20.0 damage and level 20, the expertise skill's 0.66 (level 1, a `set`) and the skill's
/// power 0.66: `(20 + 0.66 + 0.66) * 2 = 42.64`, so 42. Message, `ExFishingHpRegen` with `goodUse`
/// 1 and `anim` 1 for everybody, the fish's HP down by the damage.
#[test]
fn pumping_a_resting_fish_hurts_it() {
    let fx = Fixture::new();
    let (mut s, combat) = bite_with(&fx, &Opts::default(), (0, 0));
    let mut tried = 0;
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        tried += 1;
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        assert_eq!(
            used.len(),
            1,
            "{:?} {:?}",
            names(&sent, ME),
            message_ids(&sent, ME)
        );
        let regen = used[0];
        if regen.good_use == 0 {
            // The 9 % the fish resists: Java sends this message and changes no HP.
            assert!(
                message_ids(&sent, ME).contains(&SystemMessageId::TheFishHasResistedYourAttemptToBringItIn)
            );
            assert!(tried < 20);
            s.run(1500);
            continue;
        }
        let ids = message_ids(&sent, ME);
        assert!(
            ids.contains(&SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage),
            "{ids:?}"
        );
        let message = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage)
            .unwrap();
        assert_eq!(int_param(&message), 42);
        assert_eq!(regen.good_use, 1);
        assert_eq!(regen.penalty, 0);
        assert_eq!(regen.hp_bar_color, 0);
        // The fish had `combat.hp`; its regen only runs in mode 1.
        assert_eq!(regen.fish_hp, combat.hp - 42);
        break;
    }
}

/// The same skill against a fighting fish (mode 1) fails: "You failed to do anything with the fish
/// and it regains $s1 HP", `goodUse` 2, and the fish's HP goes up by the damage.
#[test]
fn pumping_a_fighting_fish_fails_and_it_regains_hp() {
    let fx = Fixture::new();
    let (mut s, combat) = bite_with(&fx, &Opts::default(), (1, 0));
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        assert_eq!(used.len(), 1);
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        let message = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YouFailedToDoAnythingWithTheFishAndItRegainsS1Hp)
            .expect("the failure message");
        assert_eq!(int_param(&message), 42);
        assert_eq!(used[0].good_use, 2);
        // The fish regained 42, and a fighting fish also regenerated 4 HP at its seconds
        // (`hpRegen` 4.0 while mode is 1): the HP of the packet is the HP right after the use.
        assert!(used[0].fish_hp >= combat.hp + 42, "{used:?} vs {combat:?}");
        break;
    }
}

/// Reeling works on a fighting fish, with `anim` 2.
#[test]
fn reeling_a_fighting_fish_hurts_it() {
    let fx = Fixture::new();
    let (mut s, _) = bite_with(&fx, &Opts::default(), (1, 0));
    loop {
        s.log.take();
        s.use_skill(REELING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 2).collect();
        assert_eq!(used.len(), 1);
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        let message = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YouReelThatFishInCloserAndCauseS1Damage)
            .expect("the success message");
        assert_eq!(int_param(&message), 42);
        assert_eq!(used[0].good_use, 1);
        break;
    }
}

/// The deceptive mode (the purple HP bar) turns it round: pumping a fighting deceptive fish works,
/// reeling it fails.
#[test]
fn the_deceptive_mode_reverses_pumping_and_reeling() {
    let fx = Fixture::new();
    let opts = Opts {
        lure: LUMINOUS_LURE,
        night: true,
        ..Opts::default()
    };
    // Mode 1 and deceptive: pumping works.
    let (mut s, combat) = bite_with(&fx, &opts, (1, 1));
    assert_eq!(combat.lure_type, 2);
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        assert_eq!(used[0].good_use, 1, "pumping a fighting deceptive fish works");
        assert_eq!(used[0].hp_bar_color, 1, "the purple bar");
        break;
    }
    // Reeling the same kind of fish fails.
    let (mut s, _) = bite_with(&fx, &opts, (1, 1));
    loop {
        s.log.take();
        s.use_skill(REELING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 2).collect();
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        assert_eq!(used[0].good_use, 2, "reeling a fighting deceptive fish fails");
        break;
    }
}

/// `Pumping.onStart`: a skill level three or more above the fishing expertise costs 5 % of the
/// damage (`pen = (int) (dmg * 0.05)`) and says so. Pumping level 4 has power 1.55:
/// `(20 + 0.66 + 1.55) * 2 = 44.42`, 44, penalty 2, damage 42.
#[test]
fn a_skill_above_the_expertise_is_penalised() {
    let fx = Fixture::new();
    let (mut s, _) = bite_with(
        &fx,
        &Opts {
            pumping: 4,
            ..Opts::default()
        },
        (0, 0),
    );
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        let ids = message_ids(&sent, ME);
        assert!(ids.contains(
            &SystemMessageId::DueToYourReelingAndOrPumpingSkillBeingThreeOrMoreLevelsHigherThanYourFishingSkillA50DamagePenaltyWillBeApplied
        ));
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        let damage = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage)
            .unwrap();
        assert_eq!(int_param(&damage), 42);
        let penalty = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingWasSuccessfulMasteryPenaltyS1)
            .unwrap();
        assert_eq!(int_param(&penalty), 2);
        assert_eq!(used[0].penalty, 2);
        break;
    }
}

/// `FishShots.onItemUse`: a fishing shot charges the rod (the skill 2181 animation for
/// everybody, the shot used up) and the next pump or reel does twice the damage. Then the charge
/// is gone. A shot of the wrong grade is refused.
#[test]
fn a_fishing_shot_doubles_the_damage_once() {
    let fx = Fixture::new();
    let (mut s, _) = bite_with(&fx, &Opts::default(), (0, 0));
    s.log.take();
    // The wrong grade: a D-grade shot for the no-grade rod.
    s.world.add_item(ObjectId(ME), 6536, 5, false);
    let d_shot = s
        .me()
        .data
        .items
        .iter()
        .find(|i| i.item_id == 6536)
        .unwrap()
        .object_id
        .0;
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: d_shot,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(100);
    assert!(
        message_ids(&sent, ME).contains(&SystemMessageId::ThatIsTheWrongGradeOfSoulshotForThatFishingPole)
    );
    assert_eq!(s.count(6536), 5);

    // The right one: allowed while fishing (items 6535 to 6540), used up, animated.
    s.log.take();
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: SHOT_OBJECT,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(100);
    assert_eq!(s.count(6535), 9);
    let skill: Vec<MagicSkillUse> = of(&sent, ME, "MAGIC_SKILL_USE");
    assert_eq!(skill.len(), 1);
    assert_eq!((skill[0].skill_id, skill[0].skill_level), (2181, 1));
    assert!(s.world.is_charged_shot(ObjectId(ME), ShotType::FishSoulshots));
    // Charged already: nothing happens, the shot stays.
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: SHOT_OBJECT,
            ctrl_pressed: false,
        },
    );
    s.run(100);
    assert_eq!(s.count(6535), 9);

    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        // `Pumping.onStart` discharges the rod before `usePumping`, so even a use the fish
        // resists (9 %) uses the charge up.
        assert!(!s.world.is_charged_shot(ObjectId(ME), ShotType::FishSoulshots));
        if used[0].good_use == 0 {
            s.input(
                ME,
                ClientInput::UseItem {
                    object_id: SHOT_OBJECT,
                    ctrl_pressed: false,
                },
            );
            s.run(1500);
            assert!(s.world.is_charged_shot(ObjectId(ME), ShotType::FishSoulshots));
            continue;
        }
        let damage = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage)
            .unwrap();
        // (20 + 0.66 + 0.66) * 2 * 2 = 85.28
        assert_eq!(int_param(&damage), 85);
        break;
    }

    // The charge was for one use: the next one does the plain 42.
    s.run(1500);
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        let damage = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage)
            .unwrap();
        assert_eq!(int_param(&damage), 42);
        break;
    }
}

/// The D-grade rod (`fishingRods.xml`: level 40, damage 23.0) with a D-grade shot: `(23 + 0.66 +
/// 0.66) * (40 * 0.1) * 2 = 194.56`, 194. The no-grade shot is the wrong grade for it.
#[test]
fn the_damage_follows_the_rod_and_its_grade_of_shot() {
    let fx = Fixture::new();
    let (mut s, _) = bite_with(
        &fx,
        &Opts {
            rod: D_ROD,
            ..Opts::default()
        },
        (0, 0),
    );
    s.world.add_item(ObjectId(ME), 6536, 5, false);
    s.log.take();
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: SHOT_OBJECT,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(100);
    assert!(
        message_ids(&sent, ME).contains(&SystemMessageId::ThatIsTheWrongGradeOfSoulshotForThatFishingPole)
    );
    let d_shot = s
        .me()
        .data
        .items
        .iter()
        .find(|i| i.item_id == 6536)
        .unwrap()
        .object_id
        .0;
    loop {
        s.input(
            ME,
            ClientInput::UseItem {
                object_id: d_shot,
                ctrl_pressed: false,
            },
        );
        s.run(100);
        assert!(s.world.is_charged_shot(ObjectId(ME), ShotType::FishSoulshots));
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        let used: Vec<_> = regens(&sent, ME).into_iter().filter(|r| r.anim == 1).collect();
        if used[0].good_use == 0 {
            s.run(1500);
            continue;
        }
        let damage = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YourPumpingIsSuccessfulCausingS1Damage)
            .unwrap();
        assert_eq!(int_param(&damage), 194);
        break;
    }
}

/// `Pumping`/`Reeling` without a fish: `Player.checkUseMagicConditions` refuses them with the two
/// messages and `ActionFailed`, before any reuse delay starts. Any other skill is refused while
/// fishing with "Only fishing skills may be used at this time" and, a quirk of Java, no
/// `ActionFailed`.
#[test]
fn skills_are_refused_when_there_is_nothing_to_pump_or_while_fishing() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.use_skill(PUMPING);
    let sent = s.run(500);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouMayOnlyUseThePumpingSkillWhileYouAreFishing]
    );
    assert!(names(&sent, ME).contains(&"ACTION_FAIL".to_owned()));
    s.use_skill(REELING);
    let sent = s.run(500);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouMayOnlyUseTheReelingSkillWhileYouAreFishing]
    );

    // Fishing, no bite yet: Pumping gets the same refusal, Might the other one.
    s.cast();
    s.log.take();
    s.use_skill(PUMPING);
    let sent = s.run(500);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouMayOnlyUseThePumpingSkillWhileYouAreFishing]
    );
    s.log.take();
    s.use_skill(MIGHT);
    let sent = s.run(500);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::OnlyFishingSkillsMayBeUsedAtThisTime]
    );
    // Might is a self buff: `RequestMagicSkillUse` first sends the player to where it stands (an
    // `ActionFailed`), then `checkUseMagicConditions` refuses with the message and, a quirk of
    // Java, no `ActionFailed` of its own.
    assert_eq!(names(&sent, ME), ["ACTION_FAIL", "SYSTEM_MESSAGE"]);
}

// ================================================================================== the catch

/// A win (`changeHp` brings the fish to 0 HP): the fish item comes ("You caught something"), the
/// end packet says `win`, and the reel-in message follows. Test fish of level 1 have 1 HP, so one
/// use of the skill is enough (the expertise skill level 1 only draws level 1 or 2: 1 or 2 HP).
#[test]
fn killing_the_fish_gives_the_item_and_ends_the_fishing() {
    let fx = Fixture::new();
    let opts = Opts {
        fish: Fish {
            hp: 1,
            regen: 0.0,
            ..Fish::default()
        },
        // `Rnd.get(100) <= -1` never holds: no monster.
        monster_chance: -1,
        ..Opts::default()
    };
    let (mut s, _) = bite_with(&fx, &opts, (0, 0));
    let before = s.count(6411) + s.count(6412) + s.count(6413);
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        if s.world.is_fishing(ObjectId(ME)) {
            // Resisted (9 %); try again.
            s.run(1500);
            continue;
        }
        let ids = message_ids(&sent, ME);
        // The caught message, the item message, the length (the championship is on), the reel-in.
        let caught = ids
            .iter()
            .position(|i| *i == SystemMessageId::YouCaughtSomething)
            .expect("caught something");
        let obtained = ids
            .iter()
            .position(|i| *i == SystemMessageId::YouHaveObtainedS1)
            .expect("the item");
        let length = ids
            .iter()
            .position(|i| *i == SystemMessageId::YouCaughtAFishS1InLength)
            .expect("the length");
        let reel = ids
            .iter()
            .position(|i| *i == SystemMessageId::YouReelYourLineInAndStopFishing)
            .expect("reeled in");
        assert!(caught < obtained && obtained < length && length < reel, "{ids:?}");
        assert_eq!(
            ends(&sent, ME),
            [ExFishingEnd {
                object_id: ME,
                win: true
            }]
        );
        assert_eq!(s.count(6411) + s.count(6412) + s.count(6413), before + 1);
        assert!(!s.world.is_immobilized(ObjectId(ME)));
        break;
    }
}

/// `Fishing.doDie(true)` with a fishing monster of the fisher's level: with a chance of 100 the
/// fisher gets "something smelly and scary" and the monster appears at the fisher's feet with the
/// fisher as its target; no fish item.
#[test]
fn a_fishing_monster_can_come_up_instead_of_the_fish() {
    let fx = Fixture::new();
    let opts = Opts {
        fish: Fish {
            hp: 1,
            regen: 0.0,
            ..Fish::default()
        },
        monster_chance: 100,
        ..Opts::default()
    };
    let (mut s, _) = bite_with(&fx, &opts, (0, 0));
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        if s.world.is_fishing(ObjectId(ME)) {
            s.run(1500);
            continue;
        }
        let ids = message_ids(&sent, ME);
        assert!(ids.contains(&SystemMessageId::YouCaughtSomethingSmellyAndScaryMaybeYouShouldThrowItBack));
        assert!(!ids.contains(&SystemMessageId::YouCaughtSomething));
        assert_eq!(s.count(6411) + s.count(6412) + s.count(6413), 0);
        assert_eq!(
            ends(&sent, ME),
            [ExFishingEnd {
                object_id: ME,
                win: true
            }]
        );
        let monster = s
            .world
            .npc_ids()
            .into_iter()
            .filter_map(|id| s.world.npc(id))
            .find(|n| n.npc_id == 18322)
            .expect("the fishing monster");
        assert_eq!((monster.core.x, monster.core.y, monster.core.z), (X, Y, Z));
        assert_eq!(
            s.world.creature_target(monster.core.object_id),
            Some(ObjectId(ME))
        );
        break;
    }
}

/// Java gives nothing at all when no fishing monster covers the fisher's level (the shipped file
/// starts at level 6): no fish, no message, only the end of the fishing.
#[test]
fn a_fisher_below_level_6_gets_nothing_from_a_won_fight() {
    let fx = Fixture::new();
    let opts = Opts {
        level: 5,
        fish: Fish {
            hp: 1,
            regen: 0.0,
            ..Fish::default()
        },
        ..Opts::default()
    };
    let (mut s, _) = bite_with(&fx, &opts, (0, 0));
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        if s.world.is_fishing(ObjectId(ME)) {
            s.run(1500);
            continue;
        }
        let ids = message_ids(&sent, ME);
        assert!(!ids.contains(&SystemMessageId::YouCaughtSomething), "{ids:?}");
        assert_eq!(s.count(6411) + s.count(6412) + s.count(6413), 0);
        assert_eq!(
            ends(&sent, ME),
            [ExFishingEnd {
                object_id: ME,
                win: true
            }]
        );
        break;
    }
}

// ================================================================================== the losses

/// `Fishing.run`: when the time is up (`_time <= 0`) the fish spits the hook.
#[test]
fn a_fight_that_takes_too_long_is_lost() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            fish: Fish {
                duration: 3,
                ..Fish::default()
            },
            ..Opts::default()
        },
    );
    s.cast();
    s.wait_for_bite(15_000).expect("a bite");
    // Runs at +1 s (time 3 -> 2), +2 s (-> 1), +3 s (-> 0), +4 s: time is up.
    let sent = s.run(4_000);
    let ids = message_ids(&sent, ME);
    assert_eq!(
        ids,
        [
            SystemMessageId::ThatFishIsMoreDeterminedThanYouAreItSpitTheHook,
            SystemMessageId::YouReelYourLineInAndStopFishing,
        ]
    );
    assert_eq!(
        ends(&sent, ME),
        [ExFishingEnd {
            object_id: ME,
            win: false
        }]
    );
    assert!(!s.world.is_fishing(ObjectId(ME)));
    // No bait-lost message: the fish did take the bait.
    assert!(!ids.contains(&SystemMessageId::TheBaitHasBeenLostBecauseTheFishGotAway));
}

/// A fighting fish regains `(int) hpRegen` every second (`aiTask`). When its HP reaches twice
/// the maximum, the next run says the bait was stolen.
#[test]
fn a_fish_that_regains_too_much_steals_the_bait() {
    let fx = Fixture::new();
    let (mut s, combat) = bite_with(
        &fx,
        &Opts {
            fish: Fish {
                hp: 10,
                regen: 1000.0,
                ..Fish::default()
            },
            ..Opts::default()
        },
        (1, 0),
    );
    let max = combat.hp;
    let sent = s.run(3_000);
    // The mode may flip at the first check (30 %); either way the HP goes up by 1000 while it
    // fights, so within a few seconds the fish is above twice its maximum.
    let ids = message_ids(&sent, ME);
    if ids.contains(&SystemMessageId::YourBaitWasStolenByThatFish) {
        assert!(regens(&sent, ME).iter().any(|r| r.fish_hp >= 2 * max));
        assert_eq!(
            ends(&sent, ME),
            [ExFishingEnd {
                object_id: ME,
                win: false
            }]
        );
        assert!(!s.world.is_fishing(ObjectId(ME)));
    } else {
        // The mode flipped to resting and stayed there for the 3 s: the HP did not move.
        assert!(s.world.is_fishing(ObjectId(ME)));
    }
}

/// `changeHp`: a failed use that brings the fish above twice its maximum ends the fight with
/// `doDie(false)` and no "bait stolen" message (that one is `run`'s). The HP packet is built
/// before the clamp, so it still shows the HP the use gave.
#[test]
fn a_failed_use_can_push_the_fish_over_the_limit() {
    let fx = Fixture::new();
    // 10 HP per level: a failed use gives it 42 HP, over twice its 10 (level 1) or 20 (level 2).
    let (mut s, combat) = bite_with(
        &fx,
        &Opts {
            fish: Fish {
                hp: 10,
                regen: 0.0,
                ..Fish::default()
            },
            ..Opts::default()
        },
        (1, 0),
    );
    loop {
        s.log.take();
        s.use_skill(PUMPING);
        let sent = s.run(1000);
        if s.world.is_fishing(ObjectId(ME)) {
            s.run(1500);
            continue;
        }
        let ids = message_ids(&sent, ME);
        assert!(ids.contains(&SystemMessageId::YouFailedToDoAnythingWithTheFishAndItRegainsS1Hp));
        assert!(!ids.contains(&SystemMessageId::YourBaitWasStolenByThatFish));
        let regen = regens(&sent, ME).into_iter().find(|r| r.anim == 1).unwrap();
        assert_eq!(regen.good_use, 2);
        assert_eq!(regen.fish_hp, combat.hp + 42, "the HP before the clamp");
        assert_eq!(
            ends(&sent, ME),
            [ExFishingEnd {
                object_id: ME,
                win: false
            }]
        );
        break;
    }
}

// ================================================================================== what a fisher cannot do

/// While fishing: items other than fishing shots ("You cannot do anything else while fishing"),
/// dropping ("You can't mount, dismount, break and drop items while fishing"), the social
/// actions, and a duel.
#[test]
fn a_fisher_cannot_use_drop_or_challenge() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.enter(P2, "Other", &Opts::default());
    s.cast();
    s.log.take();

    s.input(
        ME,
        ClientInput::UseItem {
            object_id: OTHER_OBJECT,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(100);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouCannotDoThatWhileFishing3]
    );
    assert_eq!(s.count(1060), 5);

    s.input(
        ME,
        ClientInput::DropItem {
            object_id: OTHER_OBJECT,
            count: 1,
            x: X,
            y: Y,
            z: Z,
        },
    );
    let sent = s.run(100);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouCannotDoThatWhileFishing2]
    );
    assert_eq!(s.count(1060), 5);

    // A social action (the action window's "Hello", id 12).
    s.input(
        ME,
        ClientInput::ActionUse {
            action_id: 12,
            ctrl_pressed: false,
            shift_pressed: false,
        },
    );
    let sent = s.run(100);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouCannotDoThatWhileFishing3]
    );

    // A duel: the fisher cannot be challenged and cannot challenge.
    assert_eq!(
        s.world.can_duel(ObjectId(ME)),
        Err(SystemMessageId::C1CannotDuelBecauseC1IsCurrentlyFishing)
    );
    assert_eq!(s.world.can_duel(ObjectId(P2)), Ok(()));
}

/// With a rod in the hands a lure used from the inventory goes to the left hand (`UseItem`'s
/// rod branch) and the client gets the new `UserInfo` and the item list.
#[test]
fn a_lure_used_with_a_rod_goes_to_the_left_hand() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: 0,
            ..Opts::default()
        },
    );
    // The lure lies in the inventory.
    s.world.add_item(ObjectId(ME), LURE, 7, false);
    let object = s
        .me()
        .data
        .items
        .iter()
        .find(|i| i.item_id == LURE)
        .unwrap()
        .object_id
        .0;
    s.log.take();
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: object,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(500);
    let lure = s.me().data.items.iter().find(|i| i.item_id == LURE).unwrap();
    assert_eq!((lure.loc.as_str(), lure.loc_data), ("PAPERDOLL", 7));
    let names = names(&sent, ME);
    assert!(names.contains(&"USER_INFO".to_owned()), "{names:?}");
    assert!(names.contains(&"ITEM_LIST".to_owned()), "{names:?}");

    // Without a rod the same item does nothing (it has no handler).
    let mut s = Scene::new(
        &fx,
        &Opts {
            lure: 0,
            rod: 0,
            ..Opts::default()
        },
    );
    s.world.add_item(ObjectId(ME), LURE, 7, false);
    let object = s
        .me()
        .data
        .items
        .iter()
        .find(|i| i.item_id == LURE)
        .unwrap()
        .object_id
        .0;
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: object,
            ctrl_pressed: false,
        },
    );
    s.run(500);
    let lure = s.me().data.items.iter().find(|i| i.item_id == LURE).unwrap();
    assert_eq!(lure.loc, "INVENTORY");
}

/// `Player.deleteMe`: a fisher who leaves takes the tasks with it; nothing fires afterwards.
#[test]
fn leaving_while_fishing_cancels_the_tasks() {
    let fx = Fixture::new();
    for in_fight in [false, true] {
        let mut s = Scene::new(&fx, &Opts::default());
        s.cast();
        if in_fight {
            s.wait_for_bite(15_000).expect("a bite");
        }
        s.world.handle(WorldCommand::Disconnected {
            session: SessionId(u64::try_from(ME).unwrap()),
        });
        s.log.take();
        // The bite check, the fight task and everything else would have fired by now.
        s.run(40_000);
        assert!(s.world.player(ObjectId(ME)).is_none());
        assert!(s.log.take().is_empty());
    }
}

// ================================================================================== the championship

fn championship_opts(rewards: [i32; 5]) -> Opts {
    Opts {
        fishing: FishingSettings {
            championship_rewards: rewards,
            championship_database_writes: true,
            clock_override_unix_ms: Some(NOW),
            ..FishingSettings::default()
        },
        ..Opts::default()
    }
}

const NOW: i64 = 1_792_368_000_000 + 12 * 3_600_000;

fn utc(_: i64) -> i64 {
    0
}

fn row(name: &str, length: f64, rewarded: i32) -> ChampionshipRow {
    ChampionshipRow {
        name: name.into(),
        length,
        rewarded,
    }
}

/// `newFish`: every catch is measured (60.000 to 89.999, `Rnd.get(60, 89) + Rnd.get(0, 1000) /
/// 1000.0`; the prize lures add up to 3 more) and told as "You caught a fish $s1 in length"; the
/// first five fishers are registered, a longer fish of the same fisher replaces theirs, a fish
/// shorter than the shortest of five is not registered.
#[test]
fn caught_fish_are_measured_and_ranked() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &championship_opts([1; 5]));
    for (id, name) in [
        (0x2000_0010, "Aa"),
        (0x2000_0011, "Bb"),
        (0x2000_0012, "Cc"),
        (0x2000_0013, "Dd"),
    ] {
        s.enter(id, name, &Opts::default());
    }
    s.world
        .load_fishing_championship_at(ChampionshipData::default(), NOW, utc);
    s.log.take();

    // Fisher catches: the same fisher twice, the second try is the better one only sometimes.
    let mut best: f64 = 0.0;
    for _ in 0..6 {
        s.log.take();
        s.world.championship_new_fish(ObjectId(ME), LURE);
        let sent = s.log.take();
        let message = messages(&sent, ME)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YouCaughtAFishS1InLength)
            .unwrap();
        let text = match &message.params[0] {
            SystemMessageParam::Text(t) => t.clone(),
            other => panic!("{other:?}"),
        };
        let length: f64 = text.parse().unwrap();
        assert!((60.0..90.0).contains(&length), "{length}");
        assert!(text.contains('.'), "Java prints 75.0, not 75: {text}");
        let registered = message_ids(&sent, ME)
            .contains(&SystemMessageId::BecauseOfTheSizeOfFishCaughtYouWillBeRegisteredInTheRanking);
        // Only a longer fish than the fisher's best is registered again.
        assert_eq!(registered, length > best, "{length} vs {best}");
        best = best.max(length);
    }
    let (running, winners) = s.world.fishing_championship_rows();
    assert_eq!(running.len(), 1, "one entry per fisher");
    assert_eq!(running[0].name, "Fisher");
    assert_eq!(running[0].length, best);
    assert!(winners.is_empty());
}

/// With five fishers on the list a new fish only gets in when it is longer than the shortest one,
/// which then leaves; a fisher who is on the list only changes his own entry. The test follows
/// the rule on a model, with the lengths the messages told.
#[test]
fn a_full_ranking_keeps_the_five_longest() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &championship_opts([1; 5]));
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: NOW + 3_600_000,
            rows: vec![
                row("A", 61.0, 0),
                row("B", 70.0, 0),
                row("C", 80.0, 0),
                row("D", 85.0, 0),
                row("E", 89.5, 0),
            ],
        },
        NOW,
        utc,
    );
    let mut model = vec![
        ("A".to_owned(), 61.0),
        ("B".to_owned(), 70.0),
        ("C".to_owned(), 80.0),
        ("D".to_owned(), 85.0),
        ("E".to_owned(), 89.5),
    ];
    let mut entered = 0;
    for _ in 0..60 {
        s.log.take();
        s.world.championship_new_fish(ObjectId(ME), LURE);
        let sent = s.log.take();
        let length: f64 = match &messages(&sent, ME)[0].params[0] {
            SystemMessageParam::Text(t) => t.parse().unwrap(),
            other => panic!("{other:?}"),
        };
        // The rule of `newFish` (the list is full or becomes full).
        if let Some(entry) = model.iter_mut().find(|(n, _)| n == "Fisher") {
            if entry.1 < length {
                entry.1 = length;
            }
        } else {
            let min = model.iter().map(|(_, l)| *l).fold(99_999.0, f64::min);
            if min < length {
                let at = model.iter().position(|(_, l)| *l == min).unwrap();
                model.remove(at);
                model.push(("Fisher".to_owned(), length));
                entered += 1;
            }
        }
        let (running, _) = s.world.fishing_championship_rows();
        let mut got: Vec<(String, f64)> = running.into_iter().map(|r| (r.name, r.length)).collect();
        let mut want = model.clone();
        got.sort_by(|a, b| a.0.cmp(&b.0));
        want.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(got, want);
        assert_eq!(got.len(), 5);
    }
    // 60 fish of 60 to 90 meters: the fisher got in for sure (the shortest is 61).
    assert_eq!(entered, 1);
}

/// `ALT_FISH_CHAMPIONSHIP_ENABLED = False`: no measuring, no ranking, and the Fisherman says so.
#[test]
fn a_disabled_championship_measures_nothing() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            fishing: FishingSettings {
                championship_enabled: false,
                ..FishingSettings::default()
            },
            ..Opts::default()
        },
    );
    s.world.championship_new_fish(ObjectId(ME), LURE);
    assert!(s.log.take().is_empty());
    assert!(s.world.fishing_championship_rows().0.is_empty());
    // `RequestExFishRanking` does nothing either.
    s.input(ME, ClientInput::FishRanking);
    assert!(s.log.take().is_empty());
}

/// `RequestExFishRanking` -> `showMidResult`: the first request shows the "please wait" page and
/// rebuilds the ranking; for the next minute the ranking page is shown with the table ("null"
/// first, a quirk of Java's string building), the prizes and the item's name; after a minute the
/// ranking is rebuilt again.
#[test]
fn the_ranking_page_is_rebuilt_at_most_once_a_minute() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &championship_opts([800_000, 500_000, 300_000, 200_000, 100_000]),
    );
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: NOW + 3_600_000,
            rows: vec![row("Bravo", 71.5, 0), row("Alpha", 88.25, 0), row("Old", 90.0, 1)],
        },
        NOW,
        utc,
    );
    s.log.take();

    s.input(ME, ClientInput::FishRanking);
    let first = pages(&s.log.take(), ME);
    assert_eq!(first.len(), 1);
    assert!(first[0].contains("PLEASE WAIT"), "{first:?}");

    s.input(ME, ClientInput::FishRanking);
    let second = pages(&s.log.take(), ME);
    assert_eq!(second.len(), 1);
    let page = &second[0];
    assert!(page.contains("RANKING"), "{page}");
    assert!(page.contains("Adena"), "the prize item's name: {page}");
    assert!(page.contains("800000") && page.contains("100000"));
    // Longest first, `None` and `0` for the empty places, "null" in front of the first row.
    let a = page.find("Alpha").unwrap();
    let b = page.find("Bravo").unwrap();
    assert!(a < b);
    assert!(page.contains("88.25") && page.contains("71.5"));
    assert!(page.contains("<td width=110 align=center>None</td><td width=80 align=center>0</td>"));
    assert!(page.contains("null<tr>"), "{page}");
    assert!(
        !page.contains("Old"),
        "last week's winners are on the Fisherman's page"
    );

    // A fish caught now is not on the page until the minute is over.
    s.world.championship_new_fish(ObjectId(ME), LURE);
    s.log.take();
    s.input(ME, ClientInput::FishRanking);
    assert!(!pages(&s.log.take(), ME)[0].contains("Fisher"));
    s.run(60_000);
    s.log.take();
    s.input(ME, ClientInput::FishRanking);
    assert!(pages(&s.log.take(), ME)[0].contains("PLEASE WAIT"));
}

/// The end of a period (`finishChamp`): the running week becomes the winners (`rewarded` 1, the
/// longest first), the end moves to the Tuesday 19:00 after 6 days, and the rows go to the
/// database (when the owner allowed it, D-49) with the variable.
#[test]
fn the_end_of_a_period_makes_the_winners_and_saves_them() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &championship_opts([1; 5]));
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: NOW - 1,
            rows: vec![row("Bravo", 71.5, 0), row("Alpha", 88.25, 0), row("Old", 90.0, 2)],
        },
        NOW,
        utc,
    );
    // The period was over at the start: it ended at once.
    let (running, winners) = s.world.fishing_championship_rows();
    assert!(running.is_empty());
    assert_eq!(
        winners,
        [row("Alpha", 88.25, 1), row("Bravo", 71.5, 1)],
        "only this week's fish, longest first; last week's winner is gone"
    );
    // `_enddate = now`, then `setEndOfChamp`: NOW is Monday 2026-10-19 12:00 UTC, + 6 days is
    // Sunday the 25th, the Tuesday of that (Sunday-first) week is the 27th at 19:00.
    let expected_end = 1_792_368_000_000 + 8 * 86_400_000 + 19 * 3_600_000;
    assert_eq!(s.world.fishing_championship_end(), expected_end);

    let saved: Vec<_> = s
        .persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::FishingChampionship(data) => Some((**data).clone()),
            _ => None,
        })
        .collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].end_date, expected_end);
    assert_eq!(saved[0].rows, [row("Alpha", 88.25, 1), row("Bravo", 71.5, 1)]);

    // Without the switch nothing is written (the Java server shares the rows).
    let mut s = Scene::new(&fx, &Opts::default());
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: NOW - 1,
            rows: vec![row("Alpha", 88.25, 0)],
        },
        NOW,
        utc,
    );
    assert_eq!(s.world.fishing_championship_rows().1, [row("Alpha", 88.25, 1)]);
    assert!(
        !s.persisted
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|c| matches!(c, PersistCommand::FishingChampionship(_)))
    );
}

/// A period that is not over is kept: the rows as read, the end as read, and the timer fires at
/// the end.
#[test]
fn a_running_period_is_kept_until_its_end() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &championship_opts([1; 5]));
    let end = NOW + 2_000;
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: end,
            rows: vec![row("Alpha", 88.25, 0), row("Won", 80.0, 1)],
        },
        NOW,
        utc,
    );
    let (running, winners) = s.world.fishing_championship_rows();
    assert_eq!(running, [row("Alpha", 88.25, 0)]);
    assert_eq!(winners, [row("Won", 80.0, 1)]);
    assert_eq!(s.world.fishing_championship_end(), end);
    // Shutdown saves it (all rows, winners first) with the same end.
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    let commands = s.persisted.0.lock().unwrap();
    let saved = commands
        .iter()
        .find_map(|c| match c {
            PersistCommand::FishingChampionship(data) => Some((**data).clone()),
            _ => None,
        })
        .expect("the shutdown save");
    assert_eq!(saved.end_date, end);
    assert_eq!(saved.rows, [row("Won", 80.0, 1), row("Alpha", 88.25, 0)]);
}

/// The Fisherman: `FishingChampionship` shows the winners' page (time left, the NPC's object id,
/// the table), `FishingReward` gives a winner the prize of his place once (`rewarded` 2) and
/// shows the thanks page; anybody else gets the "no reward" page.
#[test]
fn the_fisherman_shows_the_winners_and_pays_the_prizes_once() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &championship_opts([800_000, 500_000, 300_000, 200_000, 100_000]),
    );
    s.enter(P2, "Loser", &Opts::default());
    s.world.load_fishing_championship_at(
        ChampionshipData {
            end_date: NOW + 90 * 60_000,
            rows: vec![
                row("Fisher", 80.0, 1),
                row("Other", 90.5, 1),
                row("Third", 70.0, 1),
            ],
        },
        NOW,
        utc,
    );
    let npc = s.world.spawn_npc_at(31561, X + 40, Y, Z, 0).unwrap();
    // Click twice: the first selects, the second opens the chat window with the links.
    for _ in 0..2 {
        s.input(
            ME,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
    }
    s.log.take();

    s.input(
        ME,
        ClientInput::Bypass {
            command: format!("npc_{}_FishingChampionship", npc.0),
        },
    );
    let page = pages(&s.log.take(), ME);
    assert_eq!(page.len(), 1, "one page");
    let page = &page[0];
    assert!(page.contains("WINNERS 90 min"), "{page}");
    assert!(page.contains(&format!("{}", npc.0)));
    // Other (90.5) is first, Fisher (80.0) second, Third (70.0) third.
    assert!(page.find("Other").unwrap() < page.find("Fisher").unwrap());
    assert!(page.contains("80.0") && page.contains("90.5"));

    // Fisher is second: 500000 adena, once.
    s.input(
        ME,
        ClientInput::Bypass {
            command: format!("npc_{}_FishingReward", npc.0),
        },
    );
    let sent = s.log.take();
    assert_eq!(s.world.adena_of(ObjectId(ME)), 500_000);
    assert!(pages(&sent, ME)[0].contains("REWARD"));
    assert_eq!(
        s.world
            .fishing_championship_rows()
            .1
            .iter()
            .find(|r| r.name == "Fisher")
            .unwrap()
            .rewarded,
        2
    );
    // Every page replaces the links of the one before, so the dialog is opened again first.
    s.input(
        ME,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    s.input(
        ME,
        ClientInput::Bypass {
            command: format!("npc_{}_FishingReward", npc.0),
        },
    );
    let sent = s.log.take();
    assert_eq!(s.world.adena_of(ObjectId(ME)), 500_000, "the prize is paid once");
    // A winner who took the prize gets neither a page nor more adena (`rewardType == 2`).
    assert!(pages(&sent, ME).is_empty(), "{:?}", names(&sent, ME));

    // Somebody who did not win.
    for _ in 0..2 {
        s.input(
            P2,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
    }
    s.log.take();
    s.input(
        P2,
        ClientInput::Bypass {
            command: format!("npc_{}_FishingReward", npc.0),
        },
    );
    let sent = s.log.take();
    assert!(pages(&sent, P2)[0].contains("NO REWARD FOR YOU"));
    assert_eq!(s.world.adena_of(ObjectId(P2)), 0);
}

/// With the championship off the Fisherman says there is no event.
#[test]
fn the_fisherman_has_no_event_when_the_championship_is_off() {
    let fx = Fixture::new();
    let mut s = Scene::new(
        &fx,
        &Opts {
            fishing: FishingSettings {
                championship_enabled: false,
                ..FishingSettings::default()
            },
            ..Opts::default()
        },
    );
    let npc = s.world.spawn_npc_at(31561, X + 40, Y, Z, 0).unwrap();
    for _ in 0..2 {
        s.input(
            ME,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
    }
    s.log.take();
    for command in ["FishingChampionship", "FishingReward"] {
        // The dialog is opened again: the page of the first answer has no links.
        s.input(
            ME,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        s.log.take();
        s.input(
            ME,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        );
        let sent = s.log.take();
        assert!(!pages(&sent, ME).is_empty(), "{:?}", names(&sent, ME));
        assert!(pages(&sent, ME)[0].contains("NO EVENT"));
    }
}

/// Prize lures (8484 to 8486) add up to 3 to the length.
#[test]
fn prize_lures_give_longer_fish() {
    let fx = Fixture::new();
    let mut s = Scene::new(&fx, &Opts::default());
    s.world
        .load_fishing_championship_at(ChampionshipData::default(), NOW, utc);
    let mut longest: f64 = 0.0;
    for _ in 0..200 {
        s.log.take();
        s.world.championship_new_fish(ObjectId(ME), 8485);
        for m in messages(&s.log.take(), ME) {
            if let Some(SystemMessageParam::Text(t)) = m.params.first() {
                longest = longest.max(t.parse::<f64>().unwrap());
            }
        }
    }
    // Plain fish are below 90; with the bonus (up to 3) 200 catches pass 90 for sure.
    assert!(longest > 90.0 && longest < 93.0, "{longest}");
}
