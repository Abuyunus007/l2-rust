//! What the M6 end-to-end test adds to the datapack of `datapack.rs`, `m3_data.rs`, `m4_data.rs`
//! and `m5_data.rs`: a fighter who can reach level 10 (clans need it), three more NPCs next to the
//! start (a merchant, a warehouse keeper, a village master), a buy list and a multisell list, a
//! party buff, a servitor, the enchant data, and the items every new fighter starts with.
//!
//! CI has no datapack, so everything is written into the temporary folder of the test. The ids and
//! the shape of the skills, items and files are those of the shipped datapack (`dist/game/data`);
//! where a number differs from the shipped one the comment says so.

use std::path::Path;

use super::m3_data::{GROUND_Z, START};
use super::m5_data::{MANA_POTION, MANA_POTION_SKILL};

// ---- NPCs ----------------------------------------------------------------------------------------

/// A `Merchant` with a buy list and an exchange list.
pub const SMITH_ID: i32 = 30101;
pub const SMITH_POS: (i32, i32, i32) = (START.0 - 120, START.1 + 60, GROUND_Z);
/// A `Warehouse` keeper.
pub const KEEPER_ID: i32 = 30102;
pub const KEEPER_POS: (i32, i32, i32) = (START.0 - 120, START.1 - 60, GROUND_Z);
/// A `VillageMasterFighter`: clans are created through it.
pub const MASTER_ID: i32 = 30103;
pub const MASTER_POS: (i32, i32, i32) = (START.0 - 60, START.1 - 120, GROUND_Z);
/// The servitor `Summon Kat the Cat` calls (the shipped Kat, stats rounded).
pub const KAT_ID: i32 = 14111;

// ---- skills --------------------------------------------------------------------------------------

/// `1007` Chant of Battle level 1: a buff on the whole party (`PARTY`), here with the MP cost
/// cut to 4 + 10 (the shipped 12 + 48 is more than the 30 MP of a level 1 fighter) and the pAtk
/// factor 1.5 (the shipped 1.08 of a pAtk of 6 is cut back to 6, which no packet would show).
pub const CHANT_OF_BATTLE: i32 = 1007;
pub const CHANT_MP_INITIAL: i32 = 4;
pub const CHANT_MP: i32 = 10;
pub const CHANT_SECONDS: i32 = 1200;
/// `1111` Summon Kat the Cat. Here without the spirit ore the shipped skill eats and with a
/// longer life (the shipped 1200 s are kept; the consume interval is the default of Java).
pub const SUMMON_KAT: i32 = 1111;
pub const KAT_LIFE_SECONDS: i32 = 1200;

// ---- items ---------------------------------------------------------------------------------------

/// A weapon of grade D that can be enchanted (the items of the real game have other ids; this one
/// carries the numbers of a D grade sword).
pub const TEST_BLADE: i32 = 9902;
/// `955` Scroll: Enchant Weapon (D-grade).
pub const ENCHANT_SCROLL_D: i32 = 955;
/// A stackable item merchants buy: 100 adena a piece, so 50 from a merchant.
pub const GEM: i32 = 9903;
pub const GEM_PRICE: i64 = 100;
/// The ingredient and the product of the exchange list.
pub const ORE: i32 = 9904;
pub const INGOT: i32 = 9905;
/// `1419` Blood Mark: the price of clan level 3.
pub const BLOOD_MARK: i32 = 1419;

/// What a new fighter starts with besides the items of `m5_data` (soulshots, mana potions).
pub const BLADES_AT_START: i64 = 1;
pub const SCROLLS_AT_START: i64 = 4;
pub const GEMS_AT_START: i64 = 20;
pub const ORE_AT_START: i64 = 10;
pub const BLOOD_MARKS_AT_START: i64 = 1;
/// `StartingAdena` of this test: enough for the clan levels 1 and 2 (650 000 + 2 500 000).
pub const START_ADENA: i64 = 4_000_000;
/// SP of the clan leader in the scenario (levels 1 to 3 cost 20 000 + 100 000 + 350 000).
pub const LEADER_SP: i64 = 500_000;
/// Level of the clan leader (`Clan creation` needs 10) and the exp that goes with it here.
pub const LEADER_LEVEL: i32 = 10;
pub const LEADER_EXP: i64 = 46_930;

/// The buy list of the smith and its two products.
pub const BUY_LIST: i32 = 300101;
pub const SOULSHOT_PRICE: i64 = 20;
/// The exchange list of the smith: 5 ore and 100 adena for 2 ingots.
pub const MULTISELL_LIST: i32 = 400101;
pub const EXCHANGE_ENTRY: i32 = 1;
pub const EXCHANGE_ORE: i64 = 5;
pub const EXCHANGE_ADENA: i64 = 100;
pub const EXCHANGE_INGOTS: i64 = 2;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// `experience.xml` up to level 10. Levels 1 to 6 are the shipped numbers; 7 to 10 are made up.
const EXPERIENCE: &str = r#"<table maxLevel="10" maxPetLevel="10">
	<experience level="1" tolevel="0" /><experience level="2" tolevel="68" />
	<experience level="3" tolevel="363" /><experience level="4" tolevel="1168" />
	<experience level="5" tolevel="2884" /><experience level="6" tolevel="6038" />
	<experience level="7" tolevel="11287" /><experience level="8" tolevel="19429" />
	<experience level="9" tolevel="31040" /><experience level="10" tolevel="46930" />
	<experience level="11" tolevel="69100" />
</table>"#;

/// The human fighter of `m4_data.rs` with the levels 6 to 10 added (made up, like the rest).
fn fighter() -> String {
    let mut levels = String::new();
    let table = [
        (80.0, 30.0, 32.0),
        (91.83, 35.46, 36.732),
        (103.79, 40.98, 41.516),
        (115.88, 46.56, 46.352),
        (128.1, 52.2, 51.24),
        (140.5, 57.9, 56.2),
        (153.0, 63.7, 61.2),
        (165.8, 69.6, 66.3),
        (178.7, 75.5, 71.5),
        (191.8, 81.5, 76.7),
    ];
    for (i, (hp, mp, cp)) in table.iter().enumerate() {
        levels.push_str(&format!(
            "\t\t<level val=\"{}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp><hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>\n",
            i + 1
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<classId>0</classId>
	<staticData>
		<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON>
		<baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
		<creationPoints><node x="{}" y="{}" z="{}" /></creationPoints>
		<basePAtk>4</basePAtk>
		<baseCritRate>4</baseCritRate>
		<baseAtkType>FIST</baseAtkType>
		<basePAtkSpd>300</basePAtkSpd>
		<basePDef>
			<chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak>
		</basePDef>
		<baseMAtk>6</baseMAtk>
		<baseMDef>
			<rear>9</rear><lear>9</lear><rfinger>5</rfinger><lfinger>5</lfinger><neck>13</neck>
		</baseMDef>
		<baseAtkRange>20</baseAtkRange>
		<baseRndDam>10</baseRndDam>
		<baseMoveSpd><walk>80</walk><run>115</run><slowSwim>50</slowSwim><fastSwim>50</fastSwim></baseMoveSpd>
		<baseBreath>100</baseBreath>
		<baseSafeFall>250</baseSafeFall>
		<collisionMale><radius>9.0</radius><height>23</height></collisionMale>
		<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
	</staticData>
	<lvlUpgainData>
{levels}	</lvlUpgainData>
</list>"#,
        START.0, START.1, START.2
    )
}

/// The skills the scenario adds to the fighter's tree. `1007` and `1111` carry the shipped shape.
fn skills() -> String {
    format!(
        r##"<list>
	<skill id="{CHANT_OF_BATTLE}" levels="1" name="Chant of Battle">
		<icon>icon.skill1007</icon>
		<operateType>A2</operateType>
		<targetType>PARTY</targetType>
		<abnormalLevel>1</abnormalLevel>
		<abnormalTime>{CHANT_SECONDS}</abnormalTime>
		<abnormalType>PA_UP</abnormalType>
		<affectRange>1000</affectRange>
		<effectPoint>162</effectPoint>
		<hitTime>2500</hitTime>
		<isMagic>1</isMagic>
		<magicLevel>14</magicLevel>
		<mpConsume>{CHANT_MP}</mpConsume>
		<mpInitialConsume>{CHANT_MP_INITIAL}</mpInitialConsume>
		<reuseDelay>5000</reuseDelay>
		<effects>
			<effect name="Buff">
				<mul stat="pAtk">1.5</mul>
			</effect>
		</effects>
	</skill>
	<skill id="{SUMMON_KAT}" levels="1" name="Summon Kat the Cat">
		<icon>icon.skill1111</icon>
		<operateType>A1</operateType>
		<targetType>SELF</targetType>
		<hitTime>500</hitTime>
		<isMagic>1</isMagic>
		<magicLevel>1</magicLevel>
		<reuseDelay>2000</reuseDelay>
		<conditions>
			<player canSummon="true" />
		</conditions>
		<effects>
			<effect name="Summon">
				<npcId>{KAT_ID}</npcId>
				<lifeTime>{KAT_LIFE_SECONDS}</lifeTime>
				<expMultiplier>0.5</expMultiplier>
			</effect>
		</effects>
	</skill>
</list>"##
    )
}

/// The items of this scenario, in the shape of the shipped files.
fn items() -> String {
    format!(
        r#"<list>
	<item id="{TEST_BLADE}" type="Weapon" name="Test Blade">
		<set name="crystal_type" val="D" />
		<set name="crystal_count" val="20" />
		<set name="weapon_type" val="SWORD" />
		<set name="bodypart" val="rhand" />
		<set name="enchant_enabled" val="true" />
		<set name="price" val="5000" />
		<stats><stat type="pAtk">6</stat></stats>
	</item>
	<item id="{ENCHANT_SCROLL_D}" type="EtcItem" name="Scroll: Enchant Weapon (D-grade)">
		<set name="etcitem_type" val="SCRL_ENCHANT_WP" />
		<set name="handler" val="EnchantScrolls" />
		<set name="is_stackable" val="true" />
	</item>
	<item id="{GEM}" type="EtcItem" name="Test Gem">
		<set name="is_stackable" val="true" />
		<set name="price" val="{GEM_PRICE}" />
	</item>
	<item id="{ORE}" type="EtcItem" name="Test Ore"><set name="is_stackable" val="true" /></item>
	<item id="{INGOT}" type="EtcItem" name="Test Ingot"><set name="is_stackable" val="true" /></item>
	<item id="{BLOOD_MARK}" type="EtcItem" name="Blood Mark"><set name="is_stackable" val="true" /></item>
	<item id="9901" type="EtcItem" name="Rat Tail"><set name="is_stackable" val="false" /></item>
	<item id="{MANA_POTION}" type="EtcItem" name="Mana Potion">
		<set name="default_action" val="SKILL_REDUCE" />
		<set name="etcitem_type" val="POTION" />
		<set name="immediate_effect" val="true" />
		<set name="is_stackable" val="true" />
		<set name="handler" val="ItemSkills" />
		<set name="for_npc" val="true" />
		<conditions msgId="113" addName="1">
			<player flyMounted="false" />
		</conditions>
		<skills>
			<skill id="{MANA_POTION_SKILL}" level="1" />
		</skills>
	</item>
	<item id="1835" type="EtcItem" name="Soulshot: No Grade">
		<set name="default_action" val="SOULSHOT" />
		<set name="immediate_effect" val="true" />
		<set name="is_stackable" val="true" />
		<set name="handler" val="SoulShots" />
		<set name="for_npc" val="true" />
		<set name="price" val="{SOULSHOT_PRICE}" />
		<skills>
			<skill id="2039" level="1" />
		</skills>
	</item>
</list>"#
    )
}

/// The three NPCs next to the start. They stand inside the town zone of `m3_data.rs`, so a trade
/// or a mail is allowed (and fights between players are not).
fn npcs() -> String {
    let npc = |id: i32, kind: &str, name: &str| {
        format!(
            r#"	<npc id="{id}" level="70" type="{kind}" name="{name}" title="Test">
		<race>HUMAN</race>
		<sex>MALE</sex>
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="2444.46819" hpRegen="7.5" mp="1345.8" mpRegen="2.7" />
			<attack physical="688.86373" magical="470.40463" random="30" critical="4" accuracy="5" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="295.91597" magical="216.53847" />
			<speed>
				<walk ground="50" />
				<run ground="120" />
			</speed>
		</stats>
		<status attackable="false" />
	</npc>
"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
{}{}{}	<npc id="{KAT_ID}" level="22" type="Pet" name="Kat the Cat">
		<race>ELEMENTAL</race>
		<stats str="40" int="21" dex="30" wit="20" con="43" men="25">
			<vitals hp="387" hpRegen="3.5" mp="218" mpRegen="1.5" />
			<attack physical="51" magical="35" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="92" magical="67" />
			<speed><walk ground="80" /><run ground="120" /></speed>
		</stats>
		<shots soul="1" spirit="1" />
		<collision><radius normal="9"/><height normal="16"/></collision>
	</npc>
</list>"#,
        npc(SMITH_ID, "Merchant", "Smith"),
        npc(KEEPER_ID, "Warehouse", "Keeper"),
        npc(MASTER_ID, "VillageMasterFighter", "Master"),
    )
}

fn spawn(id: i32, pos: (i32, i32, i32)) -> String {
    format!(
        "\t<spawn name=\"m6_{id}\">\n\t\t<npc id=\"{id}\" x=\"{}\" y=\"{}\" z=\"{}\" heading=\"0\" respawnDelay=\"60\" />\n\t</spawn>\n",
        pos.0, pos.1, pos.2
    )
}

/// Adds the M6 data below `root` (the folder that holds `data/`). Call after the four writers
/// before it: it replaces the experience table, the fighter's template, the fighter's skill tree,
/// the starting equipment and `m4_items.xml`.
pub fn write_social_data(root: &Path) {
    write(root, "data/stats/players/experience.xml", EXPERIENCE);
    write(
        root,
        "data/stats/players/templates/StartingClass/HumanFighter.xml",
        &fighter(),
    );
    write(root, "data/stats/skills/m6_skills.xml", &skills());
    write(root, "data/stats/npcs/30100-30199.xml", &npcs());
    write(
        root,
        "data/spawns/m6.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n{}{}{}</list>",
            spawn(SMITH_ID, SMITH_POS),
            spawn(KEEPER_ID, KEEPER_POS),
            spawn(MASTER_ID, MASTER_POS)
        ),
    );

    // The fighter learns the party buff and the summon from level 1, besides the skills of M5.
    write(
        root,
        "data/stats/players/skillTrees/StartingClass/Fighter.xml",
        &format!(
            r#"<list>
	<skillTree type="classSkillTree" classId="0">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Common Craft" skillId="1322" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Wind Strike" skillId="1177" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Wind Walk" skillId="1204" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Chant of Battle" skillId="{CHANT_OF_BATTLE}" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Summon Kat the Cat" skillId="{SUMMON_KAT}" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
</list>"#
        ),
    );

    // Items. This replaces `m4_items.xml` of M5: the soulshot gets a price (the buy list sells
    // it); the rat tail and the mana potion are written again as they were.
    write(root, "data/stats/items/m4_items.xml", &items());

    write(
        root,
        "data/stats/players/initialEquipment.xml",
        &format!(
            r#"<list>
	<equipment classId="0">
		<item id="2369" count="1" equipped="true" />
		<item id="1146" count="1" equipped="true" />
		<item id="1147" count="1" equipped="true" />
		<item id="5588" count="1" />
		<item id="{mana}" count="{potions}" />
		<item id="1835" count="{shots}" />
		<item id="{TEST_BLADE}" count="{BLADES_AT_START}" />
		<item id="{ENCHANT_SCROLL_D}" count="{SCROLLS_AT_START}" />
		<item id="{GEM}" count="{GEMS_AT_START}" />
		<item id="{ORE}" count="{ORE_AT_START}" />
		<item id="{BLOOD_MARK}" count="{BLOOD_MARKS_AT_START}" />
	</equipment>
	<equipment classId="10">
		<item id="6" count="1" equipped="true" />
		<item id="425" count="1" equipped="true" />
		<item id="461" count="1" equipped="true" />
	</equipment>
</list>"#,
            mana = super::m5_data::MANA_POTION,
            potions = super::m5_data::POTIONS_AT_START,
            shots = super::m5_data::SOULSHOTS_AT_START,
        ),
    );

    // The smith sells soulshots and mana potions.
    write(
        root,
        &format!("data/buylists/{BUY_LIST}.xml"),
        &format!(
            "<list><npcs><npc>{SMITH_ID}</npc></npcs><item id=\"1835\" price=\"{SOULSHOT_PRICE}\" /><item id=\"{}\" price=\"50\" /></list>",
            super::m5_data::MANA_POTION
        ),
    );
    // ...and exchanges ore for ingots.
    write(
        root,
        &format!("data/multisell/{MULTISELL_LIST}.xml"),
        &format!(
            "<list><npcs><npc>{SMITH_ID}</npc></npcs><item><ingredient count=\"{EXCHANGE_ORE}\" id=\"{ORE}\"/><ingredient count=\"{EXCHANGE_ADENA}\" id=\"57\"/><production count=\"{EXCHANGE_INGOTS}\" id=\"{INGOT}\"/></item></list>"
        ),
    );

    // The pages the NPCs show; the links are what the client sends back.
    write(
        root,
        &format!("data/html/merchant/{SMITH_ID}.htm"),
        &format!(
            "<html><body>Smith %npcname%:<br>\n\
             <a action=\"bypass -h npc_%objectId%_Buy {BUY_LIST}\">Buy</a><br>\n\
             <a action=\"bypass -h npc_%objectId%_multisell {MULTISELL_LIST}\">Exchange</a><br>\n\
             </body></html>"
        ),
    );
    write(
        root,
        &format!("data/html/warehouse/{KEEPER_ID}.htm"),
        "<html><body>Keeper %npcname%:<br>\n\
         <a action=\"bypass -h npc_%objectId%_DepositP\">Deposit</a><br>\n\
         <a action=\"bypass -h npc_%objectId%_WithdrawP\">Withdraw</a><br>\n\
         </body></html>",
    );
    write(
        root,
        &format!("data/html/villagemaster/{MASTER_ID}.htm"),
        "<html><body>Master %npcname%:<br>\n\
         <edit var=\"clan_name\" width=120 height=12>\n\
         <button value=\"Create\" action=\"bypass -h npc_%objectId%_create_clan $clan_name\" width=70 height=21>\n\
         <a action=\"bypass -h npc_%objectId%_increase_clan_level\">Level up</a><br>\n\
         </body></html>",
    );

    // The enchant scroll: grade D weapons up to +16; the rate groups say 100 % up to +2 for a
    // weapon (shortened from the shipped `EnchantItemGroups.xml`, which has other numbers).
    write(
        root,
        "data/EnchantItemData.xml",
        &format!(
            "<list><enchant id=\"{ENCHANT_SCROLL_D}\" targetGrade=\"D\" maxEnchant=\"16\" /></list>"
        ),
    );
    write(
        root,
        "data/EnchantItemGroups.xml",
        r#"<list>
<enchantRateGroup name="FIGHTER_WEAPON_GROUP"><current enchant="0-2" chance="100" /><current enchant="3" chance="0" /></enchantRateGroup>
<enchantScrollGroup id="0">
<enchantRate group="FIGHTER_WEAPON_GROUP"><item slot="rhand" magicWeapon="false" /><item slot="lrhand" magicWeapon="false" /></enchantRate>
</enchantScrollGroup>
</list>"#,
    );
}
