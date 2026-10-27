#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Cursed weapons (M8-CURSED-01): the drop from a monster, the pickup that binds the weapon to a
//! player, the form and the skills, the kill count, the life timer, the drop on the owner's death,
//! the end, and the side by side rule of D-53 (no drop without `CycleDatabaseWrites`).
//! Java: managers/CursedWeaponsManager.java, entity/actor/holders/player/CursedWeapon.java,
//! entity/actor/Player.java (addItem, doDie, onKillUpdatePvPKarma), entity/actor/Attackable.java
//! (doItemDrop), network/clientpackets/EnterWorld.java, RequestCursedWeaponList.java,
//! RequestCursedWeaponLocation.java, scripts/handlers/chat/commands/admin/AdminCursedWeapons.java.
//!
//! The scenes run on the real world with small synthetic data: two cursed weapons in the
//! `CursedWeapons.xml` shape (the drop rate and the life times are the scene's), the cursed forms
//! 301 and 302, and a monster with an empty drop list.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, CursedWeaponDefs, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, Transforms,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    Earthquake, ExCursedWeaponList, ExCursedWeaponLocation, ExRedSky, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- the data ----------------------------------------------------------------------------------

const ZARICHE: i32 = 8190;
const AKAMANAH: i32 = 8689;

/// `CursedWeapons.xml` of the scene: a drop rate of 100000 is certain, the life is 1 minute and
/// each kill takes 1 minute off, so a weapon ends after 5 checks of 12 seconds.
fn defs(drop_rate: i32, disapear_chance: i32) -> CursedWeaponDefs {
    CursedWeaponDefs::from_xml(&format!(
        r#"<list>
<item id="8190" name="Demonic Sword Zariche" skillId="3603">
  <disapearChance val="{disapear_chance}" /><dropRate val="{drop_rate}" /><duration val="300" />
  <durationLost val="3" /><stageKills val="10" />
</item>
<item id="8689" name="Blood Sword Akamanah" skillId="3629">
  <disapearChance val="{disapear_chance}" /><dropRate val="{drop_rate}" /><duration val="300" />
  <durationLost val="3" /><stageKills val="10" />
</item>
</list>"#
    ))
    .unwrap()
}

/// The short-lived variant: 1 minute of life, so the 12 second checks end it within a minute.
fn short_defs() -> CursedWeaponDefs {
    CursedWeaponDefs::from_xml(
        r#"<list>
<item id="8190" name="Demonic Sword Zariche" skillId="3603">
  <disapearChance val="50" /><dropRate val="100000" /><duration val="1" />
  <durationLost val="1" /><stageKills val="10" />
</item>
</list>"#,
    )
    .unwrap()
}

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="5" /></item>
<item id="8190" type="Weapon" name="Demonic Sword Zariche"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="lrhand" /><set name="weight" val="5" /></item>
<item id="8689" type="Weapon" name="Blood Sword Akamanah"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="lrhand" /><set name="weight" val="5" /></item>
</list>"#;

const FORMS: &str = r#"<list>
<transform id="301" type="CURSED" setName="Zariche" setTitle="">
  <Male><common><collision radius="8" height="9"/><moving walk="40" run="140"/></common></Male>
</transform>
<transform id="302" type="CURSED" setName="Akamanah" setTitle="">
  <Male><common><collision radius="8" height="9"/><moving walk="40" run="140"/></common></Male>
</transform>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="3603" levels="4" name="Demon Sword Zariche"><operateType>P</operateType><targetType>SELF</targetType></skill>
<skill id="3629" levels="4" name="Blood Sword Akamanah"><operateType>P</operateType><targetType>SELF</targetType></skill>
<skill id="3630" levels="1" name="Void Burst"><operateType>A1</operateType><targetType>SELF</targetType></skill>
<skill id="3631" levels="1" name="Void Flow"><operateType>A1</operateType><targetType>SELF</targetType></skill>
</list>"##;

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

/// A monster with 300 HP and no drops: the cursed weapon is its only possible drop.
const MONSTER: &str = r#"<list><npc id="9001" type="Monster" level="1" name="Plain Monster">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

struct Data {
    stats: StatData,
    forms: Arc<Transforms>,
    rights: AdminRights,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.stats.items.get(item_id)?;
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
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
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
        Some(&self.stats)
    }
    fn transforms(&self) -> Option<&Arc<Transforms>> {
        Some(&self.forms)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.stats.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.stats.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
}

// ---- a recording world -------------------------------------------------------------------------

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
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }

    /// The cursed weapon writes, in order.
    fn cursed(&self) -> Vec<CursedWrite> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                PersistCommand::Cursed(write) => Some(write),
                _ => None,
            })
            .collect()
    }
}

struct RecOutbox(Persisted);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn of(sent: &Sent, to: i32, opcode: ServerOpcode) -> Vec<&[u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == opcode.name())
        .map(|(_, b)| b.as_slice())
        .collect()
}

fn messages(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    of(sent, to, ServerOpcode::SystemMessage)
        .into_iter()
        .map(|b| decode::<SystemMessage>(b).id)
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;
const OTHER: i32 = 0x2000_0002;
const GM: i32 = 0x2000_0003;
/// The base of the clock the scenes run on (unix milliseconds).
const CLOCK: i64 = 1_800_000_000_000;

fn player_data(id: i32, name: &str, level: i32, items: Vec<ItemData>) -> PlayerData {
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
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: if id == GM { 100 } else { 0 },
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn item(object_id: i32, item_id: i32, loc: &str, slot: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        loc: loc.into(),
        loc_data: slot,
        ..Default::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
}

impl Scene {
    /// `cycle`: `CycleDatabaseWrites` (D-53).
    fn new(defs: &CursedWeaponDefs, cycle: bool, load: CursedLoad) -> Self {
        Self::with_players(defs, cycle, load, true)
    }

    fn with_players(defs: &CursedWeaponDefs, cycle: bool, load: CursedLoad, enter: bool) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(RecOutbox(persisted.clone())));
        let mut rights = AdminRights::default();
        rights.add_level(100, 0);
        for command in [
            "admin_cw_info",
            "admin_cw_info_menu",
            "admin_cw_add",
            "admin_cw_remove",
            "admin_cw_goto",
            "admin_cw_reload",
        ] {
            rights.add_command(command, 100, false);
        }
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                SKILLS,
                &l2_data::skill::EnchantSkillGroups::default(),
                &l2_data::SkillLoadOptions::default(),
            )),
        };
        world.set_static_data(Box::new(Data {
            stats,
            forms: Arc::new(Transforms::from_xml(FORMS).unwrap()),
            rights,
        }));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(MONSTER, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_cursed_settings(CursedSettings {
            allow_cursed_weapons: true,
            cycle_database_writes: cycle,
            clock_override_unix_ms: Some(CLOCK),
        });
        world.load_cursed_weapons(defs, load);
        let mut scene = Self {
            world,
            log,
            persisted,
        };
        if enter {
            scene.enter(player_data(ME, "Owner", 1, vec![]));
            scene.enter(player_data(OTHER, "Victim", 1, vec![]));
            scene.log.take();
            scene.persisted.take();
        }
        scene
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

    /// Seconds of world time.
    fn seconds(&mut self, s: u32) {
        self.ticks(s * 20);
    }

    /// A monster dies of `killer`'s hit.
    fn kill_monster(&mut self, killer: i32) -> ObjectId {
        let monster = self.world.spawn_npc_at(9001, X, Y, Z, 0).unwrap();
        self.world
            .reduce_current_hp(monster, 1_000_000.0, Some(ObjectId(killer)));
        monster
    }

    /// The cursed weapon lying on the ground, if any.
    fn ground_weapon(&self, item_id: i32) -> Option<ObjectId> {
        self.world
            .ground_item_ids()
            .into_iter()
            .find(|id| self.world.ground_item(*id).unwrap().data.item_id == item_id)
    }

    /// `ME` walks to the item and picks it up; the delayed packets come after.
    fn pick_up(&mut self, who: i32, item: ObjectId) {
        self.input(
            who,
            ClientInput::Action {
                object_id: item.0,
                action_id: 0,
            },
        );
        for _ in 0..400 {
            self.world.tick();
            if self.world.ground_item(item).is_none() {
                break;
            }
        }
        assert!(
            self.world.ground_item(item).is_none(),
            "the item was not picked up"
        );
        self.ticks(20);
    }

    fn player(&self, id: i32) -> &Player {
        self.world.player(ObjectId(id)).unwrap()
    }

    fn skill_level(&self, id: i32, skill_id: i32) -> Option<i32> {
        self.player(id)
            .core
            .skills
            .iter()
            .find(|(s, _)| *s == skill_id)
            .map(|(_, l)| *l)
    }

    fn worn(&self, id: i32) -> Option<i32> {
        self.player(id)
            .data
            .items
            .iter()
            .find(|i| i.loc == "PAPERDOLL" && i.item_id == ZARICHE)
            .map(|i| i.item_id)
    }
}

fn zone_and_item(sent: &Sent, to: i32, wanted: SystemMessageId) -> Vec<Vec<SystemMessageParam>> {
    of(sent, to, ServerOpcode::SystemMessage)
        .into_iter()
        .map(decode::<SystemMessage>)
        .filter(|m| m.id == wanted)
        .map(|m| m.params)
        .collect()
}

// ---- the drop ----------------------------------------------------------------------------------

/// With `CycleDatabaseWrites = False` the weapon is Sieghardt's (D-53): a monster never drops it,
/// and the drop chance is not even rolled (the RNG is left alone).
#[test]
fn no_drop_with_the_switch_off() {
    let mut off = Scene::new(&defs(100_000, 50), false, CursedLoad::default());
    off.kill_monster(ME);
    assert!(off.ground_weapon(ZARICHE).is_none());
    assert!(off.ground_weapon(AKAMANAH).is_none());
    assert!(off.world.cursed_weapons().iter().all(|w| !w.is_active()));
    assert!(off.persisted.cursed().is_empty());

    // The same seed without any cursed weapon: the next draw is the same, so none was taken.
    let mut none = Scene::new(&CursedWeaponDefs::default(), false, CursedLoad::default());
    none.kill_monster(ME);
    assert_eq!(
        off.world.rng().below(1_000_000),
        none.world.rng().below(1_000_000)
    );
}

/// `AllowCursedWeapons = False`: nothing loads.
#[test]
fn nothing_loads_without_allow_cursed_weapons() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.world.set_cursed_settings(CursedSettings {
        allow_cursed_weapons: false,
        cycle_database_writes: true,
        clock_override_unix_ms: Some(CLOCK),
    });
    s.world
        .load_cursed_weapons(&defs(100_000, 50), CursedLoad::default());
    assert!(s.world.cursed_weapons().is_empty());
    s.kill_monster(ME);
    assert!(s.ground_weapon(ZARICHE).is_none());
}

/// `Rnd.get(100000) < dropRate`: a monster drops the first weapon of Java's `HashMap` order
/// (Akamanah), the sky turns red, the earth shakes and everybody is told where. Only one weapon
/// drops per kill, and a weapon that is out is not dropped again.
#[test]
fn a_monster_drops_a_cursed_weapon_with_the_red_sky() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.kill_monster(ME);
    let sent = s.log.take();

    let ground = s.ground_weapon(AKAMANAH).expect("Akamanah lies on the ground");
    assert!(s.ground_weapon(ZARICHE).is_none(), "one weapon per kill");
    let lying = s.world.ground_item(ground).unwrap();
    // `_item.setDropTime(0)`: never removed by the auto destroy.
    assert_eq!(lying.drop_time_ms, None);
    assert!(!s.world.is_in_auto_destroy(ground));

    let weapon = s.world.cursed_weapon(AKAMANAH).unwrap();
    assert!(weapon.is_dropped && !weapon.is_activated && weapon.is_active());
    assert_eq!(weapon.item, Some(ground));
    // `System.currentTimeMillis() + duration * 60000` on the world clock.
    assert!(weapon.end_time >= CLOCK + 300 * 60_000);

    for who in [ME, OTHER] {
        let red_sky: ExRedSky = decode(of(&sent, who, ServerOpcode::ExRedSky)[0]);
        assert_eq!(red_sky.duration, 10);
        let quake: Earthquake = decode(of(&sent, who, ServerOpcode::Earthquake)[0]);
        assert_eq!(quake.intensity, 14);
        assert_eq!(quake.duration, 3);
        // The place is the killer's.
        assert_eq!((quake.x, quake.y, quake.z), (X, Y, Z));
        let told = zone_and_item(&sent, who, SystemMessageId::S2WasDroppedInTheS1Region);
        assert_eq!(told.len(), 1);
        assert_eq!(
            told[0],
            vec![
                SystemMessageParam::ZoneName { x: X, y: Y, z: Z },
                SystemMessageParam::ItemName(AKAMANAH)
            ]
        );
    }

    // The next kill drops the other one: the weapon that is out is skipped.
    s.kill_monster(ME);
    assert!(s.ground_weapon(ZARICHE).is_some());
    // And a third kill has nothing left to drop.
    let before = s.world.ground_item_ids().len();
    s.kill_monster(ME);
    assert_eq!(s.world.ground_item_ids().len(), before);
}

/// The drop chance is `Rnd.get(100000) < dropRate`: a rate of 0 never drops.
#[test]
fn a_zero_drop_rate_never_drops() {
    let mut s = Scene::new(&defs(0, 50), true, CursedLoad::default());
    for _ in 0..20 {
        s.kill_monster(ME);
    }
    assert!(s.world.cursed_weapons().iter().all(|w| !w.is_active()));
}

// ---- the pickup --------------------------------------------------------------------------------

/// Picking the weapon up binds it: the owner's karma and PK count are kept and replaced
/// (9999999 and 0), the form and the skills come, the weapon is worn, the owner is healed, the
/// row is written and everybody hears about the owner.
#[test]
fn picking_the_weapon_up_activates_it() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    {
        let me = s.world.player_mut(ObjectId(ME)).unwrap();
        me.data.karma = 5;
        me.data.pk_kills = 3;
        me.core.cur_hp = 1.0;
    }
    s.kill_monster(ME);
    let weapon = s.ground_weapon(AKAMANAH).unwrap();
    s.log.take();
    s.persisted.take();
    s.pick_up(ME, weapon);
    let sent = s.log.take();

    let cw = s.world.cursed_weapon(AKAMANAH).unwrap();
    // NOTE(java-quirk): `activate` never clears `_isDropped`, so a carried weapon is both.
    assert!(cw.is_activated && cw.is_dropped);
    assert_eq!((cw.player, cw.player_id), (Some(ObjectId(ME)), ME));
    assert_eq!((cw.player_karma, cw.player_pk_kills), (5, 3));
    assert_eq!(s.player(ME).cursed_weapon_equipped_id, AKAMANAH);
    assert!(s.world.is_cursed_weapon_equipped(ObjectId(ME)));
    assert_eq!(s.player(ME).data.karma, 9_999_999);
    assert_eq!(s.player(ME).data.pk_kills, 0);

    // Transformation 302 for Akamanah, the passive skill at level 1, Void Burst and Void Flow.
    assert!(s.world.is_transformed(ObjectId(ME)));
    assert_eq!(s.world.transformation_id(ObjectId(ME)), 302);
    assert_eq!(s.skill_level(ME, 3629), Some(1));
    assert_eq!(s.skill_level(ME, 3630), Some(1));
    assert_eq!(s.skill_level(ME, 3631), Some(1));
    assert!(s.world.has_transform_skill(ObjectId(ME), 3630));

    // The weapon is worn, the owner is healed.
    assert!(
        s.player(ME)
            .data
            .items
            .iter()
            .any(|i| i.item_id == AKAMANAH && i.loc == "PAPERDOLL")
    );
    assert_eq!(
        s.player(ME).core.cur_hp,
        f64::from(s.world.stats(ObjectId(ME)).unwrap().max_hp())
    );
    assert!(s.player(ME).core.cur_hp > 1.0);

    // The row: itemId, charId, karma, pk kills, kills, end time.
    let writes = s.persisted.cursed();
    assert!(
        writes.iter().any(|w| matches!(
            w,
            CursedWrite::Save { item_id: AKAMANAH, row: Some(row) }
                if row.char_id == ME && row.player_karma == 5 && row.player_pk_kills == 3 && row.nb_kills == 0
        )),
        "{writes:?}"
    );

    let ids = messages(&sent, ME);
    assert!(ids.contains(&SystemMessageId::YouHaveEquippedYourS1), "{ids:?}");
    // The owner stands where the pickup took him, not where the monster died.
    let (x, y, z) = (s.player(ME).core.x, s.player(ME).core.y, s.player(ME).core.z);
    for who in [ME, OTHER] {
        let told = zone_and_item(&sent, who, SystemMessageId::TheOwnerOfS2HasAppearedInTheS1Region);
        assert_eq!(
            told,
            vec![vec![
                SystemMessageParam::ZoneName { x, y, z },
                SystemMessageParam::ItemName(AKAMANAH)
            ]]
        );
        // The victory pose, 300 ms later, for the player and the neighbours.
        assert_eq!(of(&sent, who, ServerOpcode::SocialAction).len(), 1);
    }
}

/// A cursed weapon is not tradeable, not destroyable and a second weapon is erased at once: the
/// owner of one weapon gets the bonus level of Java's "temp fix" instead.
#[test]
fn a_second_weapon_is_erased_and_levels_the_first() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.kill_monster(ME);
    let first = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, first);
    s.kill_monster(ME);
    let second = s.ground_weapon(ZARICHE).unwrap();
    s.log.take();
    s.persisted.take();
    // ME walks to the second one: picking it up is `addItem` -> `activate` with a weapon equipped.
    s.pick_up(ME, second);
    let sent = s.log.take();

    // `cw2.setNbKills(stageKills - 1); cw2.increaseKills()`: the first weapon is at 10 kills.
    let first = s.world.cursed_weapon(AKAMANAH).unwrap();
    assert_eq!(first.nb_kills, 10);
    assert_eq!(s.player(ME).data.pk_kills, 10);
    assert_eq!(s.skill_level(ME, 3629), Some(2));
    // The second weapon is gone: not in the inventory, not on the ground, not active.
    assert!(!s.player(ME).data.items.iter().any(|i| i.item_id == ZARICHE));
    assert!(s.ground_weapon(ZARICHE).is_none());
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
    for who in [ME, OTHER] {
        assert!(messages(&sent, who).contains(&SystemMessageId::S1HasDisappeared));
    }
}

// ---- the kills ---------------------------------------------------------------------------------

/// Every player kill counts: the PK count shows the kills, a stage (10 kills) raises the skill,
/// and each kill takes `durationLost` minutes off the life and saves the row.
#[test]
fn kills_count_raise_the_skill_and_shorten_the_life() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.kill_monster(ME);
    let weapon = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, weapon);
    let end_before = s.world.cursed_weapon(AKAMANAH).unwrap().end_time;
    s.persisted.take();

    for _ in 0..9 {
        s.world.on_kill_update_pvp_karma(ObjectId(ME), ObjectId(OTHER));
    }
    assert_eq!(s.world.cursed_weapon(AKAMANAH).unwrap().nb_kills, 9);
    assert_eq!(s.player(ME).data.pk_kills, 9);
    assert_eq!(s.skill_level(ME, 3629), Some(1), "no stage yet");
    // The owner's karma is not touched by a cursed kill (Java returns right after the count).
    assert_eq!(s.player(ME).data.karma, 9_999_999);

    s.world.on_kill_update_pvp_karma(ObjectId(ME), ObjectId(OTHER));
    assert_eq!(s.world.cursed_weapon(AKAMANAH).unwrap().nb_kills, 10);
    assert_eq!(s.skill_level(ME, 3629), Some(2), "10 kills: the second stage");
    assert_eq!(s.world.cursed_weapon(AKAMANAH).unwrap().level(), 1);
    // `_endTime -= durationLost * 60000`, ten times 3 minutes.
    assert_eq!(
        s.world.cursed_weapon(AKAMANAH).unwrap().end_time,
        end_before - 10 * 3 * 60_000
    );
    let saves = s.persisted.cursed();
    assert_eq!(saves.len(), 10);
    assert!(matches!(
        saves.last().unwrap(),
        CursedWrite::Save { item_id: AKAMANAH, row: Some(row) } if row.nb_kills == 10
    ));

    // The level shows in UserInfo and CharInfo (`getLevel`).
    assert_eq!(s.world.user_info(ObjectId(ME)).unwrap().cursed_weapon_level, 1);
}

/// A kill by an owner of a cursed weapon takes no items, no clan reputation and no experience
/// from the victim (`pk == null || !pk.isCursedWeaponEquipped()`).
#[test]
fn a_victim_of_a_cursed_owner_loses_no_experience() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.kill_monster(ME);
    let weapon = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, weapon);
    {
        let victim = s.world.player_mut(ObjectId(OTHER)).unwrap();
        victim.data.exp = 5_000_000;
    }
    s.world.do_die(ObjectId(OTHER), Some(ObjectId(ME)));
    assert!(s.world.is_dead(ObjectId(OTHER)));
    assert_eq!(s.player(OTHER).data.exp, 5_000_000);
}

// ---- the owner dies ----------------------------------------------------------------------------

/// The owner dies: with `disapearChance` out of reach the weapon is dropped where the owner
/// lies, the owner gets the old karma and PK count back and loses the form and the skills.
#[test]
fn the_owner_dies_and_the_weapon_drops() {
    let mut s = Scene::new(&defs(100_000, -1), true, CursedLoad::default());
    {
        let me = s.world.player_mut(ObjectId(ME)).unwrap();
        me.data.karma = 300;
        me.data.pk_kills = 2;
    }
    s.kill_monster(ME);
    let weapon = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, weapon);
    s.log.take();

    s.world.do_die(ObjectId(ME), Some(ObjectId(OTHER)));
    let sent = s.log.take();

    let cw = s.world.cursed_weapon(AKAMANAH).unwrap();
    assert!(cw.is_dropped && !cw.is_activated);
    let on_ground = s.ground_weapon(AKAMANAH).expect("the weapon lies on the ground");
    assert_eq!(cw.item, Some(on_ground));
    assert_eq!(s.player(ME).cursed_weapon_equipped_id, 0);
    // The old karma comes back (300), and then Java's "karma reduction tempfix" of `doDie` takes
    // a quarter of it.
    assert_eq!((s.player(ME).data.karma, s.player(ME).data.pk_kills), (225, 2));
    assert!(!s.world.is_transformed(ObjectId(ME)));
    assert_eq!(s.skill_level(ME, 3629), None);
    assert_eq!(s.skill_level(ME, 3630), None);
    assert!(!s.player(ME).data.items.iter().any(|i| i.item_id == AKAMANAH));
    for who in [ME, OTHER] {
        assert_eq!(
            zone_and_item(&sent, who, SystemMessageId::S2WasDroppedInTheS1Region).len(),
            1
        );
    }
    // Somebody else can pick it up: the game goes on.
    s.pick_up(OTHER, on_ground);
    assert!(s.world.is_cursed_weapon_equipped(ObjectId(OTHER)));
    assert_eq!(
        s.world.cursed_weapon(AKAMANAH).unwrap().player,
        Some(ObjectId(OTHER))
    );
}

/// `Rnd.get(100) <= disapearChance`: with 100 the weapon always disappears with its owner.
#[test]
fn the_weapon_can_disappear_with_its_owner() {
    let mut s = Scene::new(&defs(100_000, 100), true, CursedLoad::default());
    s.kill_monster(ME);
    let weapon = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, weapon);
    s.log.take();
    s.persisted.take();

    s.world.do_die(ObjectId(ME), Some(ObjectId(OTHER)));
    let sent = s.log.take();

    assert!(s.ground_weapon(AKAMANAH).is_none());
    assert!(!s.world.cursed_weapon(AKAMANAH).unwrap().is_active());
    assert_eq!(s.player(ME).cursed_weapon_equipped_id, 0);
    assert_eq!((s.player(ME).data.karma, s.player(ME).data.pk_kills), (0, 0));
    assert!(!s.world.is_transformed(ObjectId(ME)));
    assert!(!s.player(ME).data.items.iter().any(|i| i.item_id == AKAMANAH));
    for who in [ME, OTHER] {
        assert!(messages(&sent, who).contains(&SystemMessageId::S1HasDisappeared));
    }
    let writes = s.persisted.cursed();
    assert!(
        writes.contains(&CursedWrite::Remove { item_id: AKAMANAH }),
        "{writes:?}"
    );
    // The owner is stored before the item goes (`storeMe`).
}

// ---- the end -----------------------------------------------------------------------------------

/// The life task checks every `durationLost * 12000` ms and ends the weapon when its time is up:
/// the item goes from the owner, who gets the karma back, and everybody is told.
#[test]
fn the_weapon_ends_when_its_time_is_up() {
    let mut s = Scene::new(&short_defs(), true, CursedLoad::default());
    s.kill_monster(ME);
    let weapon = s.ground_weapon(ZARICHE).unwrap();
    s.pick_up(ME, weapon);
    assert!(s.world.cursed_weapon(ZARICHE).unwrap().is_activated);
    s.log.take();
    s.persisted.take();

    // One minute of life (less the seconds the pickup took): the 12 second checks find it up
    // at the latest after another minute.
    s.seconds(75);
    let sent = s.log.take();

    let cw = s.world.cursed_weapon(ZARICHE).unwrap();
    assert!(!cw.is_active());
    assert_eq!((cw.end_time, cw.player_id, cw.nb_kills), (0, 0, 0));
    assert_eq!(s.player(ME).cursed_weapon_equipped_id, 0);
    assert_eq!(s.player(ME).data.karma, 0);
    assert!(!s.world.is_transformed(ObjectId(ME)));
    assert!(s.worn(ME).is_none());
    assert!(!s.player(ME).data.items.iter().any(|i| i.item_id == ZARICHE));
    assert!(messages(&sent, OTHER).contains(&SystemMessageId::S1HasDisappeared));
    assert!(
        s.persisted
            .cursed()
            .contains(&CursedWrite::Remove { item_id: ZARICHE })
    );
}

/// A weapon on the ground ends too: the item leaves the world.
#[test]
fn a_weapon_on_the_ground_ends() {
    let mut s = Scene::new(&short_defs(), true, CursedLoad::default());
    s.kill_monster(ME);
    let weapon = s.ground_weapon(ZARICHE).unwrap();
    s.seconds(75);
    assert!(s.world.ground_item(weapon).is_none());
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
}

/// The owner is offline when the time is up: Java deletes the item row and restores the karma in
/// the database (the character's own rows, whatever the switch says).
#[test]
fn an_offline_owner_is_cleaned_up_in_the_database() {
    let mut s = Scene::new(&short_defs(), true, CursedLoad::default());
    {
        let me = s.world.player_mut(ObjectId(ME)).unwrap();
        me.data.karma = 4;
        me.data.pk_kills = 1;
    }
    s.kill_monster(ME);
    let weapon = s.ground_weapon(ZARICHE).unwrap();
    s.pick_up(ME, weapon);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(ME).unwrap()),
    });
    // The weapon keeps its owner's id and forgets the player.
    let cw = s.world.cursed_weapon(ZARICHE).unwrap();
    assert!(cw.is_activated);
    assert_eq!((cw.player, cw.player_id), (None, ME));
    s.persisted.take();

    s.seconds(75);
    let writes = s.persisted.cursed();
    assert!(
        writes.contains(&CursedWrite::OfflineEnd {
            player_id: ME,
            item_id: ZARICHE,
            karma: 4,
            pk_kills: 1
        }),
        "{writes:?}"
    );
    assert!(writes.contains(&CursedWrite::Remove { item_id: ZARICHE }));
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
}

// ---- the owner logs in -------------------------------------------------------------------------

fn row(char_id: i32, nb_kills: i32, end_time: i64) -> CursedRow {
    CursedRow {
        item_id: ZARICHE,
        char_id,
        player_karma: 6,
        player_pk_kills: 2,
        nb_kills,
        end_time,
    }
}

/// `restore()` re-activates a row, `checkPlayer` finds the owner at the login and `EnterWorld`
/// calls `cursedOnLogin`: the form, the skill of the kills' stage and the messages.
#[test]
fn the_owner_is_known_again_at_the_login() {
    let end = CLOCK + 90 * 60_000;
    let mut s = Scene::with_players(
        &defs(100_000, 50),
        true,
        CursedLoad {
            rows: vec![row(ME, 25, end)],
        },
        false,
    );
    // Loaded: held by a character who is not online.
    let cw = s.world.cursed_weapon(ZARICHE).unwrap();
    assert!(cw.is_activated);
    assert_eq!(
        (cw.player, cw.player_id, cw.nb_kills, cw.end_time),
        (None, ME, 25, end)
    );
    assert_eq!((cw.player_karma, cw.player_pk_kills), (6, 2));

    let mut data = player_data(ME, "Owner", 40, vec![item(0x2000_0101, ZARICHE, "PAPERDOLL", 7)]);
    data.karma = 9_999_999;
    data.pk_kills = 25;
    s.enter(player_data(OTHER, "Watcher", 40, vec![]));
    s.enter(data);
    s.ticks(20);
    let sent = s.log.take();

    assert_eq!(s.player(ME).cursed_weapon_equipped_id, ZARICHE);
    assert_eq!(s.world.cursed_weapon(ZARICHE).unwrap().player, Some(ObjectId(ME)));
    assert!(s.world.is_transformed(ObjectId(ME)));
    assert_eq!(s.world.transformation_id(ObjectId(ME)), 301);
    // 25 kills: `1 + 25 / 10` = level 3.
    assert_eq!(s.skill_level(ME, 3603), Some(3));
    assert_eq!(s.skill_level(ME, 3630), Some(1));
    assert_eq!(s.world.cursed_weapon(ZARICHE).unwrap().level(), 2);
    // `checkPlayer` (the name as a string) and `cursedOnLogin` (the item name): 90 minutes left.
    let ids = messages(&sent, ME);
    assert_eq!(
        ids.iter()
            .filter(|i| **i == SystemMessageId::S1HasS2MinuteSOfUsageTimeRemaining)
            .count(),
        2,
        "{ids:?}"
    );
    let params = zone_and_item(&sent, ME, SystemMessageId::S1HasS2MinuteSOfUsageTimeRemaining);
    assert!(
        params
            .iter()
            .all(|p| p.last() == Some(&SystemMessageParam::IntNumber(90)))
    );
    // The login is announced to everybody who is online.
    assert_eq!(
        zone_and_item(&sent, OTHER, SystemMessageId::S2SOwnerHasLoggedIntoTheS1Region).len(),
        1
    );
}

/// A row whose time is up ends the weapon when it is loaded (`reActivate`).
#[test]
fn an_expired_row_ends_at_the_load() {
    let s = Scene::with_players(
        &defs(100_000, 50),
        true,
        CursedLoad {
            rows: vec![row(ME, 3, CLOCK - 1000)],
        },
        false,
    );
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
    let writes = s.persisted.cursed();
    assert!(
        writes.contains(&CursedWrite::OfflineEnd {
            player_id: ME,
            item_id: ZARICHE,
            karma: 6,
            pk_kills: 2
        }),
        "{writes:?}"
    );
    assert!(writes.contains(&CursedWrite::Remove { item_id: ZARICHE }));
}

/// The rows are Sieghardt's without the switch (D-53): they are not read, and the weapon
/// item a character carries is left alone at the login (Java would destroy it).
#[test]
fn the_rows_are_ignored_without_the_switch() {
    let mut s = Scene::with_players(
        &defs(100_000, 50),
        false,
        CursedLoad {
            rows: vec![row(ME, 25, CLOCK + 90 * 60_000)],
        },
        false,
    );
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
    s.enter(player_data(
        ME,
        "Owner",
        40,
        vec![item(0x2000_0101, ZARICHE, "PAPERDOLL", 7)],
    ));
    assert_eq!(s.player(ME).cursed_weapon_equipped_id, 0);
    assert!(s.player(ME).data.items.iter().any(|i| i.item_id == ZARICHE));
    assert!(s.persisted.cursed().is_empty());
}

/// "Remove demonic weapon if character is not cursed weapon equipped" (with the switch on).
#[test]
fn a_demonic_weapon_without_a_row_is_destroyed_at_the_login() {
    let mut s = Scene::with_players(&defs(100_000, 50), true, CursedLoad::default(), false);
    s.enter(player_data(
        ME,
        "Owner",
        40,
        vec![item(0x2000_0101, ZARICHE, "INVENTORY", 0)],
    ));
    assert!(!s.player(ME).data.items.iter().any(|i| i.item_id == ZARICHE));
}

// ---- the requests ------------------------------------------------------------------------------

/// `RequestCursedWeaponList` answers the ids; `RequestCursedWeaponLocation` the places of the
/// weapons that are out, and nothing when none is.
#[test]
fn the_list_and_the_location_requests() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.input(ME, ClientInput::Cursed(CursedInput::List));
    let sent = s.log.take();
    let list: ExCursedWeaponList = decode(of(&sent, ME, ServerOpcode::ExCursedWeaponList)[0]);
    assert_eq!(list.ids, vec![AKAMANAH, ZARICHE]);

    s.input(ME, ClientInput::Cursed(CursedInput::Location));
    assert!(s.log.take().is_empty(), "nothing is out");

    s.kill_monster(ME);
    let ground = s.ground_weapon(AKAMANAH).unwrap();
    let at = s.world.ground_item(ground).unwrap().clone();
    s.log.take();
    s.input(OTHER, ClientInput::Cursed(CursedInput::Location));
    let sent = s.log.take();
    let location: ExCursedWeaponLocation = decode(of(&sent, OTHER, ServerOpcode::ExCursedWeaponLocation)[0]);
    assert_eq!(location.locations.len(), 1);
    let one = location.locations[0];
    assert_eq!(
        (one.id, one.activated, one.x, one.y, one.z),
        (AKAMANAH, 0, at.x, at.y, at.z)
    );

    // Carried: the place is the owner's and the weapon is activated.
    s.pick_up(ME, ground);
    s.log.take();
    s.input(OTHER, ClientInput::Cursed(CursedInput::Location));
    let sent = s.log.take();
    let location: ExCursedWeaponLocation = decode(of(&sent, OTHER, ServerOpcode::ExCursedWeaponLocation)[0]);
    let one = location.locations[0];
    assert_eq!((one.id, one.activated), (AKAMANAH, 1));
}

// ---- what the owner cannot do ------------------------------------------------------------------

/// The restrictions of a cursed weapon owner: no shout or trade chat, no party, no dismount of the
/// weapon, no destroy of the item, and the items of the weapon are not saved to the ground table.
#[test]
fn the_owner_is_restricted() {
    let mut s = Scene::new(&defs(100_000, 50), true, CursedLoad::default());
    s.kill_monster(ME);
    let ground = s.ground_weapon(AKAMANAH).unwrap();
    // The ground table skips cursed items ("prevent double save").
    assert!(s.world.ground_item_rows().iter().all(|r| r.item_id != AKAMANAH));
    s.pick_up(ME, ground);
    s.log.take();

    s.input(
        ME,
        ClientInput::Say {
            chat_type: 1,
            text: "hello".into(),
            target: None,
        },
    );
    let sent = s.log.take();
    assert!(
        messages(&sent, ME)
            .contains(&SystemMessageId::ShoutAndTradeChattingCannotBeUsedWhilePossessingACursedWeapon)
    );
    assert!(of(&sent, OTHER, ServerOpcode::Say2).is_empty());

    // The item cannot be destroyed (`CursedWeaponsManager.isCursed(itemId)`).
    let object = s
        .player(ME)
        .data
        .items
        .iter()
        .find(|i| i.item_id == AKAMANAH)
        .map(|i| i.object_id)
        .unwrap();
    s.input(
        ME,
        ClientInput::DestroyItem {
            object_id: object.0,
            count: 1,
        },
    );
    let sent = s.log.take();
    assert!(messages(&sent, ME).contains(&SystemMessageId::ThisItemCannotBeDiscarded));
    assert!(s.player(ME).data.items.iter().any(|i| i.item_id == AKAMANAH));
}

/// A character of level 20 or less cannot be attacked by an owner, and cannot attack one.
#[test]
fn a_cursed_owner_and_a_low_level_player_keep_apart() {
    let mut s = Scene::with_players(&defs(100_000, 50), true, CursedLoad::default(), false);
    s.enter(player_data(ME, "Owner", 1, vec![]));
    s.enter(player_data(OTHER, "Newbie", 20, vec![]));
    s.kill_monster(ME);
    let ground = s.ground_weapon(AKAMANAH).unwrap();
    s.pick_up(ME, ground);
    s.log.take();

    // The owner may not attack the newbie ...
    s.world.set_intention_attack(ObjectId(ME), ObjectId(OTHER));
    let sent = s.log.take();
    assert!(messages(&sent, ME).contains(&SystemMessageId::ThatIsAnIncorrectTarget));
    // ... and the newbie may not attack the owner.
    s.world.set_intention_attack(ObjectId(OTHER), ObjectId(ME));
    let sent = s.log.take();
    assert!(messages(&sent, OTHER).contains(&SystemMessageId::ThatIsAnIncorrectTarget));
}

// ---- //cw_* ------------------------------------------------------------------------------------

/// `//cw_add` gives the weapon to the target (or the GM), `//cw_info` lists the state and
/// `//cw_remove` ends the weapon; an unknown weapon is refused.
#[test]
fn the_admin_commands() {
    let mut s = Scene::with_players(&defs(100_000, 50), true, CursedLoad::default(), false);
    s.enter(player_data(GM, "Boss", 85, vec![]));
    s.enter(player_data(ME, "Owner", 1, vec![]));
    s.log.take();

    s.world.admin_command(ObjectId(GM), "admin_cw_add nothing", false);
    let sent = s.log.take();
    assert!(text_messages(&sent, GM).contains(&"Unknown cursed weapon ID.".to_owned()));

    s.world.admin_command(ObjectId(GM), "admin_cw_info", false);
    let sent = s.log.take();
    let texts = text_messages(&sent, GM);
    assert!(
        texts.contains(&"====== Cursed Weapons: ======".to_owned()),
        "{texts:?}"
    );
    assert!(texts.contains(&"> Blood Sword Akamanah (8689)".to_owned()));
    assert!(texts.contains(&"  Don't exist in the world.".to_owned()));

    // By a part of the name, with `_` for a space.
    let added_at = s.world.world_ms();
    s.world.admin_command(ObjectId(GM), "admin_cw_add zariche", false);
    s.ticks(20);
    let sent = s.log.take();
    assert!(s.world.is_cursed_weapon_equipped(ObjectId(GM)));
    let cw = s.world.cursed_weapon(ZARICHE).unwrap();
    assert!(cw.is_activated);
    assert_eq!(cw.end_time, CLOCK + added_at + 300 * 60_000);
    assert!(!sent.is_empty());

    s.world.admin_command(ObjectId(GM), "admin_cw_add 8190", false);
    assert!(text_messages(&s.log.take(), GM).contains(&"This cursed weapon is already active.".to_owned()));

    s.world.admin_command(ObjectId(GM), "admin_cw_info", false);
    let texts = text_messages(&s.log.take(), GM);
    assert!(texts.contains(&"  Player holding: Boss".to_owned()), "{texts:?}");
    assert!(
        texts.contains(&"    Time Remaining: 299 min.".to_owned()),
        "{texts:?}"
    );

    s.world.admin_command(ObjectId(GM), "admin_cw_info_menu", false);
    let sent = s.log.take();
    assert_eq!(of(&sent, GM, ServerOpcode::NpcHtmlMessage).len(), 1);

    s.world.admin_command(ObjectId(GM), "admin_cw_remove 8190", false);
    assert!(!s.world.cursed_weapon(ZARICHE).unwrap().is_active());
    assert!(!s.world.is_cursed_weapon_equipped(ObjectId(GM)));
}

/// The texts of `sendSysMessage` / `sendMessage` (a system message with one string).
fn text_messages(sent: &Sent, to: i32) -> Vec<String> {
    of(sent, to, ServerOpcode::SystemMessage)
        .into_iter()
        .map(decode::<SystemMessage>)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .collect()
}
