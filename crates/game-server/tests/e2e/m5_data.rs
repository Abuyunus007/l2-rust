//! What the M5 end-to-end test adds to the datapack of `datapack.rs`, `m3_data.rs` and the hunting
//! data of `m4_data.rs`: the skills of the scenario (a nuke, a speed buff, the two skills of the
//! items), the items (a mana potion, soulshots), a target dummy that can take a beating, and the
//! changes to what a new fighter learns and owns.
//!
//! CI has no datapack, so everything is written into the temporary folder of the test. The skills
//! and items carry the ids and the shape of the shipped files (`data/stats/skills/01100-01199.xml`
//! and friends); where a number differs from the shipped one the comment says why.

use std::path::Path;

use super::m3_data::{GROUND_Z, START};

/// The target dummy: a monster that never attacks first and does not die in this test.
pub const DUMMY_ID: i32 = 20101;
/// 100 units west and 250 north of the start: in view of both players, inside the cast range of
/// Wind Strike (600), clear of the gatekeeper (100 north of the start) and of the geodata wall
/// (east of the start).
pub const DUMMY_POS: (i32, i32, i32) = (START.0 - 100, START.1 + 250, GROUND_Z);
pub const DUMMY_HP: i32 = 100_000;

/// `1177` Wind Strike level 1: a nuke.
pub const WIND_STRIKE: i32 = 1177;
/// `1204` Wind Walk level 1: a speed buff on one target (here: on oneself).
pub const WIND_WALK: i32 = 1204;
/// Wind Walk's `abnormalTime`, in seconds.
pub const WIND_WALK_SECONDS: i32 = 1200;
/// Wind Walk's reuse delay here, in milliseconds. NOTE: the shipped file has 2000. Three minutes
/// keep the reuse timer alive through the whole scenario (the logout and the login included, on
/// a slow machine too), and the store writes it as `reuse_delay`.
pub const WIND_WALK_REUSE_MS: i64 = 180_000;
/// `2039`: the skill of the no-grade soulshot (the animation of the shot).
pub const SOULSHOT_SKILL: i32 = 2039;
/// `10001`: the skill of the mana potion. It restores 100 MP and takes the potion itself.
pub const MANA_POTION_SKILL: i32 = 10001;

/// `728` Mana Potion.
pub const MANA_POTION: i32 = 728;
/// What a new fighter owns besides the starting equipment.
pub const POTIONS_AT_START: i64 = 3;
pub const SOULSHOTS_AT_START: i64 = 50;
/// How many soulshots the weapon uses per shot (`soulshots` of the weapon).
pub const SOULSHOTS_PER_SHOT: i64 = 1;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The skills. `1177`, `1204` (apart from the reuse), `2039` and `10001` are the shipped ones
/// (tables resolved to level 1, comments left out).
const SKILLS: &str = r##"<list>
	<skill id="1177" levels="1" name="Wind Strike">
		<icon>icon.skill1177</icon>
		<operateType>A1</operateType>
		<targetType>ONE</targetType>
		<castRange>600</castRange>
		<effectPoint>-92</effectPoint>
		<effectRange>1100</effectRange>
		<element>2</element>
		<elementPower>20</elementPower>
		<hitTime>4000</hitTime>
		<isMagic>1</isMagic>
		<magicLevel>1</magicLevel>
		<mpConsume>7</mpConsume>
		<mpInitialConsume>2</mpInitialConsume>
		<power>12</power>
		<reuseDelay>1200</reuseDelay>
		<effects>
			<effect name="MagicalDamage" />
		</effects>
	</skill>
	<skill id="1204" levels="1" name="Wind Walk">
		<icon>icon.skill1204</icon>
		<operateType>A2</operateType>
		<targetType>ONE</targetType>
		<abnormalLevel>1</abnormalLevel>
		<abnormalTime>1200</abnormalTime>
		<abnormalType>SPEED_UP</abnormalType>
		<castRange>400</castRange>
		<effectPoint>204</effectPoint>
		<effectRange>900</effectRange>
		<hitTime>4000</hitTime>
		<isMagic>1</isMagic>
		<magicLevel>20</magicLevel>
		<mpConsume>16</mpConsume>
		<mpInitialConsume>4</mpInitialConsume>
		<reuseDelay>180000</reuseDelay>
		<effects>
			<effect name="Buff">
				<add stat="runSpd">20</add>
			</effect>
		</effects>
	</skill>
	<skill id="2039" levels="1" name="Soulshot: No Grade">
		<isMagic>2</isMagic>
		<operateType>A1</operateType>
		<targetType>SELF</targetType>
		<magicLevel>1</magicLevel>
	</skill>
	<skill id="10001" levels="1" name="Mana Potion">
		<operateType>A1</operateType>
		<targetType>SELF</targetType>
		<displayId>2288</displayId>
		<isMagic>2</isMagic>
		<itemConsumeCount>1</itemConsumeCount>
		<itemConsumeId>728</itemConsumeId>
		<magicLevel>1</magicLevel>
		<reuseDelay>500</reuseDelay>
		<effects>
			<effect name="ManaHeal">
				<power>100</power>
			</effect>
		</effects>
	</skill>
</list>"##;

/// The items the scenario uses, in the shape of the shipped files. This replaces the
/// `m4_items.xml` of the hunting data (the plain soulshot and the rat tail), so the soulshot has
/// its handler and its skill.
fn items() -> String {
    format!(
        r#"<list>
	<item id="1835" type="EtcItem" name="Soulshot: No Grade">
		<set name="default_action" val="SOULSHOT" />
		<set name="immediate_effect" val="true" />
		<set name="is_stackable" val="true" />
		<set name="handler" val="SoulShots" />
		<set name="for_npc" val="true" />
		<skills>
			<skill id="{SOULSHOT_SKILL}" level="1" />
		</skills>
	</item>
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
</list>"#
    )
}

/// The target dummy: the rat of `m4_data` with a lot of HP, hits for next to nothing.
fn npcs() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="{DUMMY_ID}" level="1" type="Monster" name="Test Dummy">
		<race>BEAST</race>
		<sex>MALE</sex>
		<acquire exp="0" sp="0" />
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="{DUMMY_HP}" hpRegen="2" mp="40.0" mpRegen="0.9" />
			<attack physical="1" magical="1" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="20" magical="20" />
			<speed>
				<walk ground="20" />
				<run ground="50" />
			</speed>
			<hitTime>370</hitTime>
		</stats>
		<status randomWalk="false" />
		<ai aggroRange="0" clanHelpRange="0" isAggressive="false" />
		<collision>
			<radius normal="10" />
			<height normal="15" />
		</collision>
	</npc>
</list>"#
    )
}

/// Adds the M5 data below `root` (the folder that holds `data/`). Call after
/// `datapack::write_datapack`, `m3_data::write_world_data` and `m4_data::write_hunting_data`: it
/// replaces `m4_items.xml`, the fighter's skill tree and the starting equipment, and patches the
/// squire's sword.
pub fn write_skill_data(root: &Path) {
    write(root, "data/stats/skills/m5_skills.xml", SKILLS);
    write(root, "data/stats/items/m4_items.xml", &items());
    write(root, "data/stats/npcs/20100-20199.xml", &npcs());
    write(
        root,
        "data/spawns/m5.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<spawn name="m5_dummy">
		<npc id="{DUMMY_ID}" x="{}" y="{}" z="{}" heading="0" respawnDelay="60" />
	</spawn>
</list>"#,
            DUMMY_POS.0, DUMMY_POS.1, DUMMY_POS.2
        ),
    );

    // A new fighter knows Wind Strike and Wind Walk from level 1 (the tree of `datapack.rs` plus
    // these two).
    write(
        root,
        "data/stats/players/skillTrees/StartingClass/Fighter.xml",
        r#"<list>
	<skillTree type="classSkillTree" classId="0">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Common Craft" skillId="1322" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Wind Strike" skillId="1177" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Wind Walk" skillId="1204" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
</list>"#,
    );

    // The squire's sword is worn and uses one soulshot per shot; the pack holds potions and
    // soulshots besides the usual first items.
    let items_path = root.join("data/stats/items/00000-99999.xml");
    let text = std::fs::read_to_string(&items_path).unwrap();
    let sword = r#"<item id="2369" type="Weapon" name="Squire's Sword">"#;
    assert!(text.contains(sword), "the sword of the base datapack");
    let patched = text.replace(
        sword,
        &format!(r#"{sword}<set name="soulshots" val="{SOULSHOTS_PER_SHOT}" />"#),
    );
    std::fs::write(&items_path, patched).unwrap();

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
		<item id="{MANA_POTION}" count="{POTIONS_AT_START}" />
		<item id="1835" count="{SOULSHOTS_AT_START}" />
	</equipment>
	<equipment classId="10">
		<item id="6" count="1" equipped="true" />
		<item id="425" count="1" equipped="true" />
		<item id="461" count="1" equipped="true" />
	</equipment>
</list>"#
        ),
    );
}
