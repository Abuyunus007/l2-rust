//! A small datapack on disk for the character screen tests: two starting classes (a fighter and a
//! mystic whose clothes add max MP), a dark fighter for the race check, and the tables around
//! them. The numbers are the real ones of those classes where the capture of a Java server
//! showed them (human fighter 126 HP / 38 MP, dark mystic 95 HP / 100 MP).

use std::path::Path;
use std::sync::Arc;

use l2_data::Datapack;
use tempfile::TempDir;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn template(class_id: i32, stats: [i32; 6], node: (i32, i32, i32), levels: &[(f64, f64, f64)]) -> String {
    let [str_, con, dex, int, wit, men] = stats;
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<classId>{class_id}</classId>
	<staticData>
		<baseINT>{int}</baseINT><baseSTR>{str_}</baseSTR><baseCON>{con}</baseCON>
		<baseMEN>{men}</baseMEN><baseDEX>{dex}</baseDEX><baseWIT>{wit}</baseWIT>
		<creationPoints><node x="{}" y="{}" z="{}" /></creationPoints>
		<basePAtk>4</basePAtk>
		<collisionMale><radius>9.0</radius><height>23</height></collisionMale>
		<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
	</staticData>
	<lvlUpgainData>
"#,
        node.0, node.1, node.2
    );
    for (i, (hp, mp, cp)) in levels.iter().enumerate() {
        xml.push_str(&format!(
            "\t\t<level val=\"{}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp><hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2.0</cpRegen></level>\n",
            i + 1
        ));
    }
    xml.push_str("\t</lvlUpgainData>\n</list>");
    xml
}

fn item(id: i32, kind: &str, name: &str, sets: &str, stats: &str) -> String {
    let stats = if stats.is_empty() {
        String::new()
    } else {
        format!("<stats>{stats}</stats>")
    };
    format!("<item id=\"{id}\" type=\"{kind}\" name=\"{name}\">{sets}{stats}</item>\n")
}

/// Writes the datapack into a fresh folder and loads it.
pub fn datapack() -> (TempDir, Arc<Datapack>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "data/stats/players/classList.xml",
        r#"<list>
	<class classId="0" name="Human Fighter" />
	<class classId="1" name="Warrior" parentClassId="0" />
	<class classId="10" name="Human Mystic" />
	<class classId="31" name="Dark Fighter" />
</list>"#,
    );
    write(
        root,
        "data/stats/players/experience.xml",
        r#"<table maxLevel="5" maxPetLevel="5">
	<experience level="1" tolevel="0" /><experience level="2" tolevel="68" />
	<experience level="3" tolevel="363" /><experience level="4" tolevel="1168" />
	<experience level="5" tolevel="2884" /><experience level="6" tolevel="6038" />
</table>"#,
    );
    let levels = [
        (80.0, 30.0, 32.0),
        (91.83, 35.46, 36.732),
        (103.79, 40.98, 41.516),
    ];
    write(
        root,
        "data/stats/players/templates/StartingClass/HumanFighter.xml",
        &template(0, [40, 43, 30, 21, 11, 25], (-71338, 258271, -3104), &levels),
    );
    write(
        root,
        "data/stats/players/templates/1stClass/Warrior.xml",
        &template(1, [40, 43, 30, 21, 11, 25], (0, 0, 0), &levels),
    );
    write(
        root,
        "data/stats/players/templates/StartingClass/DarkMystic.xml",
        &template(
            10,
            [23, 24, 23, 44, 19, 37],
            (28295, 11063, -4224),
            &[(106.0, 40.0, 53.0), (121.47, 49.0, 60.0)],
        ),
    );
    write(
        root,
        "data/stats/players/templates/StartingClass/DarkFighter.xml",
        &template(31, [41, 32, 34, 25, 12, 26], (28000, 11000, -4200), &levels),
    );
    write(
        root,
        "data/stats/statBonus.xml",
        r#"<list>
	<STR><stat value="40" bonus="1.2" /></STR>
	<CON><stat value="24" bonus="0.9" /><stat value="32" bonus="1.1" /><stat value="43" bonus="1.58" /></CON>
	<MEN><stat value="25" bonus="1.28" /><stat value="26" bonus="1.3" /><stat value="37" bonus="1.45" /></MEN>
</list>"#,
    );
    write(
        root,
        "data/stats/players/skillTrees/StartingClass/Fighter.xml",
        r#"<list>
	<skillTree type="classSkillTree" classId="0">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Common Craft" skillId="1322" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Expertise D" skillId="239" skillLevel="1" getLevel="4" autoGet="true" />
		<skill skillName="Expertise C" skillId="239" skillLevel="2" getLevel="5" autoGet="true" />
		<skill skillName="Power Strike" skillId="3" skillLevel="1" getLevel="3" learnedByNpc="true" />
		<skill skillName="Power Strike" skillId="3" skillLevel="2" getLevel="3" learnedByNpc="true" />
		<skill skillName="Not given" skillId="7" skillLevel="1" getLevel="1" />
	</skillTree>
	<skillTree type="classSkillTree" classId="10">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Common Craft" skillId="1322" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Wind Strike" skillId="1177" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Self Heal" skillId="1216" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
</list>"#,
    );
    let equip = |sword: bool| {
        if sword {
            r#"<equipment classId="0">
		<item id="2369" count="1" equipped="true" />
		<item id="10" count="1" />
		<item id="1146" count="1" equipped="true" />
		<item id="1147" count="1" equipped="true" />
		<item id="5588" count="1" />
		<item id="10650" count="5" />
	</equipment>"#
        } else {
            r#"<equipment classId="10">
		<item id="6" count="1" equipped="true" />
		<item id="425" count="1" equipped="true" />
		<item id="461" count="1" equipped="true" />
		<item id="5588" count="1" />
		<item id="10650" count="5" />
	</equipment>"#
        }
    };
    write(
        root,
        "data/stats/players/initialEquipment.xml",
        &format!("<list>{}{}</list>", equip(true), equip(false)),
    );
    write(
        root,
        "data/stats/players/initialShortcuts.xml",
        r#"<list>
	<shortcuts>
		<page pageId="0">
			<slot slotId="0" shortcutType="ACTION" shortcutId="2" />
			<slot slotId="3" shortcutType="ACTION" shortcutId="5" />
			<slot slotId="10" shortcutType="ACTION" shortcutId="0" />
			<slot slotId="11" shortcutType="ITEM" shortcutId="5588" />
		</page>
	</shortcuts>
	<shortcuts classId="10">
		<page pageId="0">
			<slot slotId="1" shortcutType="SKILL" shortcutId="1177" shortcutLevel="1" />
			<slot slotId="9" shortcutType="SKILL" shortcutId="1216" shortcutLevel="1" />
		</page>
	</shortcuts>
</list>"#,
    );
    let weapon = |id, name, bodypart| {
        item(
            id,
            "Weapon",
            name,
            &format!(
                "<set name=\"weapon_type\" val=\"SWORD\" /><set name=\"bodypart\" val=\"{bodypart}\" /><set name=\"is_dropable\" val=\"false\" />"
            ),
            "<stat type=\"pAtk\">6</stat>",
        )
    };
    let armor = |id, name, bodypart, stats: &str| {
        item(
            id,
            "Armor",
            name,
            &format!("<set name=\"armor_type\" val=\"LIGHT\" /><set name=\"bodypart\" val=\"{bodypart}\" />"),
            stats,
        )
    };
    let etc = |id, name, stackable| {
        item(
            id,
            "EtcItem",
            name,
            &format!("<set name=\"is_stackable\" val=\"{stackable}\" />"),
            "",
        )
    };
    let mut items = String::from("<list>\n");
    items.push_str(&etc(57, "Adena", true));
    items.push_str(&weapon(10, "Dagger", "rhand"));
    items.push_str(&weapon(6, "Apprentice's Wand", "rhand"));
    items.push_str(&weapon(2369, "Squire's Sword", "rhand"));
    items.push_str(&armor(1146, "Squire's Shirt", "chest", ""));
    items.push_str(&armor(1147, "Squire's Pants", "legs", ""));
    items.push_str(&armor(
        425,
        "Apprentice's Tunic",
        "chest",
        "<stat type=\"pDef\">17</stat><stat type=\"maxMp\">19</stat>",
    ));
    items.push_str(&armor(
        461,
        "Apprentice's Stockings",
        "legs",
        "<stat type=\"pDef\">10</stat><stat type=\"maxMp\">10</stat>",
    ));
    items.push_str(&etc(5588, "Tutorial Guide", false));
    items.push_str(&etc(10650, "Adventurer's Scroll of Escape", true));
    items.push_str("</list>");
    write(root, "data/stats/items/00000-99999.xml", &items);
    let pack = Datapack::load(root).unwrap();
    (dir, Arc::new(pack))
}
