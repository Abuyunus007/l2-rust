#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers

//! The scene the party tests share: a world with a few synthetic players, a monster that drops
//! loot, the experience table of the shipped datapack, and helpers to read the packets back.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

pub type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
pub struct Log(Arc<Mutex<Sent>>);

impl Log {
    pub fn take(&self) -> Sent {
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
pub struct Persisted(pub Arc<Mutex<Vec<PersistCommand>>>);

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
    fn access_level_allows_alt_g(&self, level: i32) -> bool {
        level >= 100
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

pub fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

pub fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// The names of the packets one client got, in order.
pub fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

/// The packets of one kind one client got.
pub fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The system messages one client got.
pub fn messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    of(sent, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

/// Only the ids of those messages.
pub fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|m| m.id).collect()
}

/// `experience.xml` of the shipped datapack: the exp each level 1 to 86 starts at.
pub const EXP: [i64; 86] = [
    0,
    68,
    363,
    1168,
    2884,
    6038,
    11287,
    19423,
    31378,
    48229,
    71202,
    101677,
    141193,
    191454,
    254330,
    331867,
    426288,
    540000,
    675596,
    835862,
    1023784,
    1242546,
    1495543,
    1786379,
    2118876,
    2497077,
    2925250,
    3407897,
    3949754,
    4555796,
    5231246,
    5981576,
    6812513,
    7730044,
    8740422,
    9850166,
    11066072,
    12395215,
    13844951,
    15422929,
    17137087,
    18995665,
    21007203,
    23180550,
    25524868,
    28049635,
    30764654,
    33680052,
    36806289,
    40154162,
    45525133,
    51262490,
    57383988,
    63907911,
    70853089,
    80700831,
    91162654,
    102265881,
    114038596,
    126509653,
    146308200,
    167244337,
    189364894,
    212717908,
    237352644,
    271975263,
    308443198,
    346827154,
    387199547,
    429634523,
    474207979,
    532694979,
    606322775,
    696381369,
    804225364,
    931275828,
    1151275834,
    1511275834,
    2044287599,
    3075966164,
    4295351949,
    5766985062,
    7793077345,
    10235368963,
    13180481103,
    16890558728,
];

fn experience() -> Arc<ExperienceTable> {
    let rows: String = EXP
        .iter()
        .enumerate()
        .map(|(i, exp)| format!(r#"<experience level="{}" tolevel="{exp}" />"#, i + 1))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

/// One class whose HP, MP and CP are 100, 50 and 20 per level, so a level up is easy to see.
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

pub const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="5" /></item>
<item id="1060" type="EtcItem" name="Potion"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1000" type="EtcItem" name="Dagger Bit"><set name="weight" val="1" /></item>
<item id="13016" type="EtcItem" name="Title Color Change Scroll"><set name="handler" val="NicknameColor" /><set name="is_stackable" val="true" /></item>
</list>"#;

/// The monster of the scenes: level 10 (`exp` and `sp` as asked), 300 HP, certain drops: an
/// adena group of 50, a sword and 3 potions. The spoil list has 4 potions.
pub fn monster_xml(level: i32, exp: i32, sp: i32) -> String {
    format!(
        r#"<npc id="9001" type="Monster" level="{level}" name="Loot Monster">
    <acquire exp="{exp}" sp="{sp}" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <dropLists>
      <drop>
        <group chance="100"><item id="57" min="50" max="50" chance="100"/></group>
        <item id="1" min="1" max="1" chance="100"/>
        <item id="1060" min="3" max="3" chance="100"/>
      </drop>
      <spoil><item id="1060" min="4" max="4" chance="100"/></spoil>
    </dropLists>
  </npc>"#
    )
}

pub const X: i32 = -71_338;
pub const Y: i32 = 258_271;
pub const Z: i32 = -3_104;

pub const P1: i32 = 0x2000_0001;
pub const P2: i32 = 0x2000_0002;
pub const P3: i32 = 0x2000_0003;
pub const P4: i32 = 0x2000_0004;
pub const P5: i32 = 0x2000_0005;

pub fn id(raw: i32) -> ObjectId {
    ObjectId(raw)
}

pub fn player_data(id: i32, name: &str, level: i32, (x, y, z): (i32, i32, i32)) -> PlayerData {
    let (hp, mp, cp) = (
        f64::from(100 * level),
        f64::from(50 * level),
        f64::from(20 * level),
    );
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
        level,
        exp: EXP[usize::try_from(level).unwrap() - 1],
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y,
        z,
        heading: 0,
        cur_hp: hp,
        max_hp: hp,
        cur_mp: mp,
        max_mp: mp,
        cur_cp: cp,
        max_cp: cp,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

pub fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
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

/// What a scene is made of.
pub struct Config {
    pub settings: WorldSettings,
    pub skills_xml: String,
    pub monster_level: i32,
    pub monster_exp: i32,
    pub monster_sp: i32,
}

impl Default for Config {
    fn default() -> Self {
        let mut settings = WorldSettings::default();
        settings.death.disconnect_after_death = false;
        // These tests are about the party. A character with `lastAccess` 0 would be given all its
        // vitality back at login.
        settings.vitality.enable_vitality = false;
        Self {
            settings,
            skills_xml: "<list></list>".into(),
            monster_level: 10,
            monster_exp: 1000,
            monster_sp: 100,
        }
    }
}

pub struct Scene {
    pub world: World,
    pub log: Log,
    pub persisted: Persisted,
}

impl Scene {
    pub fn new() -> Self {
        Self::with(Config::default())
    }

    pub fn with(config: Config) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_settings(config.settings);
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                &config.skills_xml,
                &EnchantSkillGroups::default(),
                &SkillLoadOptions::default(),
            )),
        };
        world.set_static_data(Box::new(Data(stats)));
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &format!(
                    "<list>{}</list>",
                    monster_xml(config.monster_level, config.monster_exp, config.monster_sp)
                ),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        world.set_npc_data(npcs, Arc::new(SpawnData::default()));
        Self {
            world,
            log,
            persisted,
        }
    }

    /// A level 10 player at the scene's center.
    pub fn enter(&mut self, id: i32, name: &str) {
        self.enter_at(id, name, 10, (X, Y, Z));
    }

    pub fn enter_level(&mut self, id: i32, name: &str, level: i32) {
        self.enter_at(id, name, level, (X, Y, Z));
    }

    pub fn enter_at(&mut self, id: i32, name: &str, level: i32, pos: (i32, i32, i32)) {
        self.enter_data(player_data(id, name, level, pos));
    }

    pub fn enter_data(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.log.take();
    }

    pub fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    pub fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// `from` invites `to_name` with the given loot rule.
    pub fn invite(&mut self, from: i32, to_name: &str, distribution_type_id: i32) {
        self.input(
            from,
            ClientInput::JoinParty {
                name: to_name.into(),
                distribution_type_id,
            },
        );
    }

    pub fn answer(&mut self, who: i32, response: i32) {
        self.input(who, ClientInput::AnswerJoinParty { response });
    }

    /// `leader` invites each of `members` (id, name) in turn, loot rule `type_id`, and each accepts.
    pub fn form_party(&mut self, leader_id: i32, members: &[(i32, &str)], type_id: i32) {
        for (id, name) in members {
            self.invite(leader_id, name, type_id);
            self.answer(*id, 1);
        }
        self.log.take();
    }

    pub fn party_of(&self, id: i32) -> Option<PartyId> {
        self.world.party_of(ObjectId(id))
    }

    pub fn members(&self, id: i32) -> Vec<i32> {
        self.party_of(id)
            .map(|p| self.world.party_members(p).into_iter().map(|m| m.0).collect())
            .unwrap_or_default()
    }

    pub fn player(&self, id: i32) -> &Player {
        self.world.player(ObjectId(id)).unwrap()
    }

    pub fn spawn_monster(&mut self) -> ObjectId {
        self.world.spawn_npc_at(9001, X, Y, Z, 0).unwrap()
    }

    /// `attacker` hits the monster for `damage` (hate does not matter here).
    pub fn hit(&mut self, monster: ObjectId, attacker: i32, damage: i64) {
        self.world.add_damage_hate(monster, ObjectId(attacker), damage, 1);
    }

    pub fn inventory_count(&self, who: i32, item_id: i32) -> i64 {
        self.player(who)
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }
}
