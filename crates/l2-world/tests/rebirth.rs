#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! `Rebirth.ini` (M9-C05): the request, the token shop and the switch.
//!
//! Expected behaviour comes from `RebirthManager` and the `Rebirth` bypass handler (Java:
//! managers/RebirthManager.java, dist/game/data/scripts/handlers/bypass/npc/Rebirth.java).

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees,
    SpawnData, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ------------------------------------------------------------------ harness

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

impl Persisted {
    fn rebirth_writes(&self) -> Vec<RebirthWrite> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Rebirth(w) => Some(w.clone()),
                _ => None,
            })
            .collect()
    }

    fn stored(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|c| matches!(c, PersistCommand::StorePlayer(_)))
            .count()
    }

    fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
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
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The text parameters of the system messages one client got (Java `sendMessage`).
fn texts(sent: &Sent, to: i32) -> Vec<String> {
    of(sent, to, "SYSTEM_MESSAGE")
        .into_iter()
        .flat_map(|raw| decode::<SystemMessage>(raw).params)
        .filter_map(|p| match p {
            SystemMessageParam::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn pages(sent: &Sent, to: i32) -> Vec<String> {
    of(sent, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|raw| decode::<NpcHtmlMessage>(raw).html)
        .collect()
}

// ------------------------------------------------------------------ data

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1232" type="EtcItem" name="Rebirth Token"><set name="is_stackable" val="true" /></item>
</list>"#;

fn passive(id: i32, name: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="{name}"><operateType>P</operateType><targetType>SELF</targetType>
        <effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>"#
    )
}

fn skills_xml() -> String {
    format!(
        "<list>{}{}{}{}{}</list>",
        passive(1001, "Class Skill"),
        passive(1059, "Empower"),
        passive(1086, "Haste"),
        passive(5000, "Plain Skill"),
        passive(5002, "Kept Skill"),
    )
}

/// The class tree has `1001` at level 1, so the starting class group protects it.
const TREES: &str = r#"<list><skillTree type="classSkillTree" classId="0">
<skill skillName="A" skillId="1001" skillLevel="1" getLevel="1" autoGet="true" />
</skillTree></list>"#;

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=85)
        .map(|l: i64| {
            format!(
                r#"<experience level="{l}" tolevel="{}" />"#,
                (l - 1) * (l - 1) * 1000
            )
        })
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

fn exp_of_level(level: i64) -> i64 {
    (level - 1) * (level - 1) * 1000
}

fn class_xml() -> String {
    let rows: String = (1..=85)
        .map(|l| {
            format!(
                "<level val=\"{l}\"><hp>{}</hp><mp>{}</mp><cp>{}</cp></level>",
                100 * l,
                50 * l,
                20 * l
            )
        })
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

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            &skills_xml(),
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const OTHER: ObjectId = ObjectId(0x2000_0002);
const ADENA_STACK: ObjectId = ObjectId(0x3000_0001);
const TOKEN_STACK: ObjectId = ObjectId(0x3000_0002);

fn stack(object_id: ObjectId, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id,
        item_id,
        count,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn player_data(id: ObjectId, name: &str, level: i32, items: Vec<ItemData>, skills: &[i32]) -> PlayerData {
    let (hp, mp, cp) = (
        f64::from(100 * level),
        f64::from(50 * level),
        f64::from(20 * level),
    );
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: exp_of_level(i64::from(level)),
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: hp,
        max_hp: hp,
        cur_mp: mp,
        max_mp: mp,
        cur_cp: cp,
        max_cp: cp,
        online_time: 0,
        items,
        skills: skills
            .iter()
            .map(|&skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

/// The real `70001_dynamic.htm` of the repository's datapack snapshot.
fn dynamic_page() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist/game/data/html/default/70001_dynamic.htm");
    std::fs::read_to_string(path).unwrap_or_else(|_| {
        "<html><body>%rebirth_count% %rebirth_icons% %next_skill_lvl% %rebirth_button%</body></html>"
            .to_owned()
    })
}

fn settings(allow: bool) -> RebirthSettings {
    RebirthSettings {
        allow_rebirth: allow,
        min_level: 10,
        max_count: 3,
        item_id: 57,
        item_amount: 1000,
        reward_item_id: 1232,
        reward_item_amount: 10,
        max_selected_skills: 3,
        skill_refund_mode: "MID".to_owned(),
        fighter_skills: vec!["1059,1,6".to_owned(), "1086,1,2".to_owned()],
        skill_delete_whitelist: [5002].into_iter().collect(),
        delete_protected_skill_groups: ["STARTING_CLASS".to_owned()].into_iter().collect(),
        finish_social_id: 7,
        finish_effect_skill_id: 0,
        select_social_id: 0,
        select_effect_skill_id: 0,
        screen_message: "Congratulations %player%! [%rebirth_count%/%max_rebirth%]".to_owned(),
        global_announcement: "%player% is reborn %rebirth_count%".to_owned(),
        ..RebirthSettings::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    dir: std::path::PathBuf,
}

impl Scene {
    fn new(tag: &str, rebirth: RebirthSettings) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-rebirth-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["data/html/default", "data/zones", "data/mapregion"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("data/html/default/70001_dynamic.htm"), dynamic_page()).unwrap();
        std::fs::write(
            dir.join("data/zones/test.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\"></list>\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_settings(WorldSettings {
            rebirth,
            death: DeathSettings {
                disconnect_after_death: false,
                ..DeathSettings::default()
            },
            vitality: VitalitySettings {
                enable_vitality: false,
                ..VitalitySettings::default()
            },
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(Arc::new(Zones::load(&dir)), Arc::new(MapRegions::load(&dir)));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml("<list></list>", &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_html_cache(Arc::new(HtmCache::load(&dir, false)));
        Self {
            world,
            log,
            persisted,
            dir,
        }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.ticks(20);
        self.log.take();
        self.persisted.clear();
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// `bypass -h <command>` from the Rebirth manager's page: the handler `BypassHandler` finds.
    fn bypass(&mut self, command: &str) -> bool {
        let handler = BypassHandlers::default().get(command).expect("registered");
        let handled = handler(&mut self.world, ME, command, None);
        self.ticks(10);
        handled
    }

    fn me(&self) -> &Player {
        self.world.player(ME).unwrap()
    }

    fn count(&self, item_id: i32) -> i64 {
        self.world.inventory_item_count(ME, item_id)
    }

    fn skills(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.me().core.skills.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn hero(adena: i64, level: i32) -> PlayerData {
    player_data(
        ME,
        "Hero",
        level,
        vec![stack(ADENA_STACK, 57, adena)],
        &[1001, 5000, 5002],
    )
}

// ------------------------------------------------------------------ tests

/// `RebirthAllow = false` (the shipped value): the request is refused, nothing is written and
/// the skill hook of `rewardSkills` does not run.
#[test]
fn rebirth_does_nothing_while_it_is_off() {
    let mut s = Scene::new("off", settings(false));
    let mut data = hero(5000, 20);
    data.profile.rebirth = RebirthData::from_columns(1, "1059");
    s.enter(data);

    assert!(s.bypass("rebirth_confirmrequest"));
    let sent = s.log.take();
    assert_eq!(texts(&sent, ME.0), ["Rebirth is disabled by configuration."]);
    assert_eq!((s.me().core.level, s.count(57)), (20, 5000));
    assert_eq!(s.skills(), [1001, 5000, 5002]);
    assert!(s.persisted.rebirth_writes().is_empty());
    assert_eq!(s.persisted.stored(), 0);

    // A level up does not hand out the selected skill either.
    s.world.add_level(ME, 1);
    s.ticks(20);
    assert!(!s.skills().contains(&1059));
}

/// The refusals of `requestRebirth`, in Java's order, each with its message and no change.
#[test]
fn the_request_is_refused_without_level_items_or_room() {
    let mut s = Scene::new("refused", settings(true));
    s.enter(hero(500, 5));
    s.bypass("rebirth_confirmrequest");
    assert_eq!(
        texts(&s.log.take(), ME.0),
        ["You do not meet the minimum level for Rebirth (10)."]
    );

    let mut s = Scene::new("refused-items", settings(true));
    s.enter(hero(500, 20));
    s.bypass("rebirth_confirmrequest");
    assert_eq!(
        texts(&s.log.take(), ME.0),
        ["You need at least 1000 [ Adena ] for rebirth request."]
    );
    assert_eq!((s.me().core.level, s.count(57)), (20, 500));

    let mut s = Scene::new("refused-max", settings(true));
    let mut data = hero(5000, 20);
    data.profile.rebirth = RebirthData::from_columns(3, "");
    s.enter(data);
    s.bypass("rebirth_confirmrequest");
    assert_eq!(
        texts(&s.log.take(), ME.0),
        ["You are currently at your maximum rebirth count."]
    );
    assert_eq!(s.count(57), 5000);
}

/// The first rebirth: the item is paid, the character is level 1 again with the skills the
/// settings keep, the count row is inserted, the tokens are handed out and everybody hears it.
#[test]
fn the_first_rebirth_resets_the_character_and_pays_the_tokens() {
    let mut s = Scene::new("first", settings(true));
    s.enter(hero(5000, 20));
    s.enter(player_data(OTHER, "Bystander", 20, vec![], &[]));

    assert!(s.bypass("rebirth_confirmrequest"));

    assert_eq!(s.me().core.level, 1);
    assert_eq!(s.me().data.exp, 0);
    // The whitelisted skill and the starting class skill stay, the plain one goes.
    assert_eq!(s.skills(), [1001, 5002]);
    assert_eq!(s.count(57), 4000);
    assert_eq!(s.count(1232), 10);
    assert_eq!(s.world.rebirth_level(ME), 1);
    assert_eq!(
        s.persisted.rebirth_writes(),
        [RebirthWrite::Insert { char_id: ME.0 }]
    );
    assert!(s.persisted.stored() >= 1);

    let sent = s.log.take();
    let mine = texts(&sent, ME.0);
    assert!(
        mine.contains(&"You received 10 rebirth token(s).".to_owned()),
        "{mine:?}"
    );
    assert!(mine.contains(&"Rebirth applied successfully. You have been reborn!".to_owned()));
    assert!(mine.contains(&"Congratulations Hero! You have been Reborn!".to_owned()));
    assert!(mine.contains(&"Use your rebirth tokens to purchase skills.".to_owned()));
    // The screen message, the social action for the neighbours, the announcement for all.
    assert_eq!(of(&sent, ME.0, "EX_SHOW_SCREEN_MESSAGE").len(), 1);
    assert_eq!(of(&sent, OTHER.0, "SOCIAL_ACTION").len(), 1);
    assert_eq!(of(&sent, OTHER.0, "SAY2").len(), 1);
    // The menu page, with the real file's placeholders filled in.
    let page = pages(&sent, ME.0).pop().unwrap();
    assert!(page.contains("1/3"), "{page}");
    assert!(!page.contains("Max Rebirth reached."));
    assert!(page.contains("rebirth_confirmrequest"));
    assert!(!page.contains("%rebirth_count%"));
}

/// A second rebirth is an `UPDATE` of the count.
#[test]
fn a_later_rebirth_updates_the_count() {
    let mut s = Scene::new("second", settings(true));
    let mut data = hero(5000, 20);
    data.profile.rebirth = RebirthData::from_columns(1, "1059");
    s.enter(data);
    s.bypass("rebirth_confirmrequest");
    assert_eq!(s.world.rebirth_level(ME), 2);
    assert_eq!(
        s.persisted.rebirth_writes(),
        [RebirthWrite::Count {
            char_id: ME.0,
            count: 2
        }]
    );
    // The skill bought before is handed out again after the reset (`grantRebirthSkills`).
    assert!(s.skills().contains(&1059));
}

/// The token shop: the preview, the purchase (cost, row, skill), the limit of `min(count, max)`
/// skills, and the reset with the `MID` refund.
#[test]
fn skills_are_bought_with_tokens_and_reset_with_a_refund() {
    let mut s = Scene::new("shop", settings(true));
    let mut data = hero(0, 20);
    data.items = vec![stack(TOKEN_STACK, 1232, 10)];
    data.profile.rebirth = RebirthData::from_columns(1, "");
    s.enter(data);

    // The selection window lists the pool, the preview offers "Confirm".
    s.bypass("rebirth_selectskills");
    let page = pages(&s.log.take(), ME.0).pop().unwrap();
    assert!(page.contains("Maximum allowed: 1 skill(s)"));
    assert!(page.contains("rebirth_previewSkill 1059") && page.contains("rebirth_previewSkill 1086"));
    s.bypass("rebirth_previewSkill 1059");
    let page = pages(&s.log.take(), ME.0).pop().unwrap();
    assert!(page.contains("rebirth_confirmSkill 1059") && page.contains("consumes 6 token(s)"));

    s.bypass("rebirth_confirmSkill 1059");
    assert_eq!(s.count(1232), 4);
    assert!(s.skills().contains(&1059));
    assert_eq!(
        s.persisted.rebirth_writes(),
        [RebirthWrite::Skills {
            char_id: ME.0,
            skills: "1059".to_owned()
        }]
    );
    assert_eq!(s.me().data.profile.rebirth.selected_skills, [1059]);
    let sent = s.log.take();
    assert!(texts(&sent, ME.0).contains(&"Rebirth skill purchased successfully.".to_owned()));

    // One rebirth allows one skill.
    s.bypass("rebirth_confirmSkill 1086");
    assert_eq!(
        texts(&s.log.take(), ME.0),
        ["You have reached the maximum number of skills allowed. (1)."]
    );
    assert!(!s.skills().contains(&1086));
    assert_eq!(s.count(1232), 4);

    // Reset: the skill goes, half of the cost (6 / 2) comes back.
    s.persisted.clear();
    s.bypass("rebirth_resetSkill 1059");
    assert!(!s.skills().contains(&1059));
    assert_eq!(s.count(1232), 7);
    assert_eq!(
        s.persisted.rebirth_writes(),
        [RebirthWrite::Skills {
            char_id: ME.0,
            skills: String::new()
        }]
    );
    let sent = s.log.take();
    assert!(texts(&sent, ME.0).contains(&"Refunded 3 rebirth token(s).".to_owned()));
    s.bypass("rebirth_resetSkill 1059");
    assert_eq!(texts(&s.log.take(), ME.0), ["You do not own this rebirth skill."]);
}

/// Not enough tokens: the item message and the preview again; a bad number is "Invalid skill.".
#[test]
fn buying_without_tokens_or_with_a_bad_id_is_refused() {
    let mut s = Scene::new("poor", settings(true));
    let mut data = hero(0, 20);
    data.items = vec![stack(TOKEN_STACK, 1232, 2)];
    data.profile.rebirth = RebirthData::from_columns(1, "");
    s.enter(data);
    s.bypass("rebirth_confirmSkill 1059");
    let sent = s.log.take();
    assert_eq!(
        texts(&sent, ME.0),
        ["You need at least 6 [ Rebirth Token ] for rebirth token."]
    );
    assert!(pages(&sent, ME.0).pop().unwrap().contains("Not enough tokens."));
    assert_eq!(s.count(1232), 2);
    assert!(s.persisted.rebirth_writes().is_empty());

    s.bypass("rebirth_previewSkill abc");
    assert_eq!(texts(&s.log.take(), ME.0), ["Invalid skill."]);
    s.bypass("rebirth_confirmSkill");
    assert_eq!(texts(&s.log.take(), ME.0), ["Invalid skill."]);
}

/// A character that never was reborn cannot open the skill window.
#[test]
fn the_skill_window_needs_a_rebirth() {
    let mut s = Scene::new("window", settings(true));
    s.enter(hero(0, 20));
    s.bypass("rebirth_selectskills");
    assert_eq!(
        texts(&s.log.take(), ME.0),
        ["You must perform at least one Rebirth before selecting skills."]
    );
}
