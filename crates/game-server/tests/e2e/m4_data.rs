//! What the M4 end-to-end test adds to the datapack of `datapack.rs` and the world data of
//! `m3_data.rs`: the fighter with the numbers a fight needs (attack speed, movement speed, base
//! defence), the exp-loss tables, a harmless monster with a certain drop, an aggressive monster
//! that cannot be killed, their spawns and the items they drop.
//!
//! CI has no datapack, so everything is written into the temporary folder of the test. The numbers
//! are made up; the layout is that of the real files (`data/stats/npcs`, `data/spawns`,
//! `data/stats/items`, `data/stats/players`). The monsters are `type="Monster"` with `<acquire>`,
//! `<ai>` and `<dropLists>` exactly like the shipped ones.

use std::path::Path;

use super::m3_data::{GROUND_Z, START};

/// The weak monster: level 1, 30 HP, never attacks first, a certain drop.
pub const RAT_ID: i32 = 20001;
/// The strong monster: level 3, aggressive, cannot be killed by a level 2 fighter.
pub const WOLF_ID: i32 = 20002;

/// Exp and SP of the rat. 100 exp is 32 above the 68 a level 2 needs and 263 short of level 3.
pub const RAT_EXP: i64 = 100;
pub const RAT_SP: i64 = 12;
/// The rat's drop: adena (a stack that merges with the one in the inventory), a stackable item
/// (a new stack) and a plain item (a new item of its own).
pub const RAT_ADENA: i64 = 25;
pub const SOULSHOT_ID: i32 = 1835;
pub const RAT_SOULSHOTS: i64 = 10;
pub const RAT_TAIL_ID: i32 = 9901;

/// Where the rat stands: inside the town (a peace zone only stops fights between players), 450
/// units from the start, so the walk takes a few seconds and the fight is in view of the player
/// who waits at the start.
pub const RAT_POS: (i32, i32, i32) = (START.0 - 250, START.1 + 450, GROUND_Z);
/// How long the rat's corpse stays (`DefaultCorpseTime`, seconds) and how long after the decay the
/// rat comes back (`respawnDelay`, seconds).
pub const CORPSE_SECONDS: i32 = 2;
pub const RESPAWN_SECONDS: i32 = 5;

/// Where the wolf stands: outside the town box (the zones end 1000 units north of the start).
pub const WOLF_POS: (i32, i32, i32) = (START.0, START.1 + 1450, GROUND_Z);
/// The wolf's `aggroRange` (the datapack clamps a monster's range to `MaxAggroRange`, 450).
pub const WOLF_AGGRO_RANGE: i32 = 450;
/// The hunter walks here: 350 units from the wolf, inside its aggro range, and just outside the
/// town zones (so the death does not depend on `AltMobAgroInPeaceZone`).
pub const NEAR_THE_WOLF: (i32, i32, i32) = (START.0, START.1 + 1100, GROUND_Z);

/// `experienceLoss.xml` of the shipped datapack, levels 1 to 5.
pub const EXP_LOSS: [f64; 5] = [10.0, 9.875, 9.75, 9.625, 9.5];

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The human fighter of the real `HumanFighter.xml` (stats, speeds and defence), with the HP, MP
/// and CP of levels 1 to 5. The base datapack's version has no attack speed and no speeds; a
/// fight needs both.
const FIGHTER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<classId>0</classId>
	<staticData>
		<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON>
		<baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
		<creationPoints><node x="-71338" y="258271" z="-3104" /></creationPoints>
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
		<level val="1"><hp>80.0</hp><mp>30.0</mp><cp>32.0</cp><hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>
		<level val="2"><hp>91.83</hp><mp>35.46</mp><cp>36.732</cp><hpRegen>2.05</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>
		<level val="3"><hp>103.79</hp><mp>40.98</mp><cp>41.516</cp><hpRegen>2.1</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>
		<level val="4"><hp>115.88</hp><mp>46.56</mp><cp>46.352</cp><hpRegen>2.15</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>
		<level val="5"><hp>128.1</hp><mp>52.2</mp><cp>51.24</cp><hpRegen>2.2</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>
	</lvlUpgainData>
</list>"#;

/// Adds the hunting data below `root` (the folder that holds `data/`). Call after
/// `datapack::write_datapack` and `m3_data::write_world_data`: it replaces the fighter's template
/// and adds files of its own.
pub fn write_hunting_data(root: &Path) {
    write(
        root,
        "data/stats/players/templates/StartingClass/HumanFighter.xml",
        FIGHTER,
    );

    let mut loss = String::from("<list>\n");
    for (index, percent) in EXP_LOSS.iter().enumerate() {
        loss.push_str(&format!(
            "\t<experience level=\"{}\" val=\"{percent}\" />\n",
            index + 1
        ));
    }
    loss.push_str("</list>");
    write(root, "data/stats/players/experienceLoss.xml", &loss);

    write(
        root,
        "data/stats/items/m4_items.xml",
        &format!(
            r#"<list>
	<item id="{SOULSHOT_ID}" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true" /></item>
	<item id="{RAT_TAIL_ID}" type="EtcItem" name="Rat Tail"><set name="is_stackable" val="false" /></item>
</list>"#
        ),
    );

    // The rat hits for 3 (a level 2 fighter has about 130 HP); the wolf for 400.
    write(
        root,
        "data/stats/npcs/20000-20099.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="{RAT_ID}" level="1" type="Monster" name="Test Rat">
		<race>BEAST</race>
		<sex>MALE</sex>
		<acquire exp="{RAT_EXP}" sp="{RAT_SP}" />
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="30" hpRegen="2" mp="40.0" mpRegen="0.9" />
			<attack physical="3" magical="3" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="20" magical="20" />
			<speed>
				<walk ground="20" />
				<run ground="50" />
			</speed>
			<hitTime>370</hitTime>
		</stats>
		<status randomWalk="false" />
		<ai aggroRange="0" clanHelpRange="0" isAggressive="false" />
		<dropLists>
			<drop>
				<item id="57" min="{RAT_ADENA}" max="{RAT_ADENA}" chance="100" />
				<item id="{SOULSHOT_ID}" min="{RAT_SOULSHOTS}" max="{RAT_SOULSHOTS}" chance="100" />
				<item id="{RAT_TAIL_ID}" min="1" max="1" chance="100" />
			</drop>
		</dropLists>
		<collision>
			<radius normal="10" />
			<height normal="15" />
		</collision>
	</npc>
	<npc id="{WOLF_ID}" level="3" type="Monster" name="Test Wolf">
		<race>BEAST</race>
		<sex>MALE</sex>
		<acquire exp="500" sp="50" />
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="1000000" hpRegen="2" mp="40.0" mpRegen="0.9" />
			<attack physical="400" magical="5" random="30" critical="4" accuracy="50" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="100" magical="50" />
			<speed>
				<walk ground="40" />
				<run ground="120" />
			</speed>
			<hitTime>370</hitTime>
		</stats>
		<status randomWalk="false" />
		<ai aggroRange="{WOLF_AGGRO_RANGE}" clanHelpRange="0" isAggressive="true" />
		<collision>
			<radius normal="12" />
			<height normal="20" />
		</collision>
	</npc>
</list>"#
        ),
    );

    write(
        root,
        "data/spawns/m4.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<spawn name="m4_rat">
		<npc id="{RAT_ID}" x="{}" y="{}" z="{}" heading="0" respawnDelay="{RESPAWN_SECONDS}" />
	</spawn>
	<spawn name="m4_wolf">
		<npc id="{WOLF_ID}" x="{}" y="{}" z="{}" heading="32768" respawnDelay="60" />
	</spawn>
</list>"#,
            RAT_POS.0, RAT_POS.1, RAT_POS.2, WOLF_POS.0, WOLF_POS.1, WOLF_POS.2
        ),
    );
}
