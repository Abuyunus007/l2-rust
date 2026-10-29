//! Test helpers of the Gracia scripts (S-AI-07): skill templates, zones and command channels on
//! top of `crate::testing::ScriptTest`.
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;

use l2_data::{ItemTemplates, MapRegions, SkillTemplates, Zones};
use l2_util::ObjectId;
use l2_world::*;

/// The item 8871 (Strategy Guide) lets a party leader open a command channel.
pub const STRATEGY_GUIDE: i32 = 8871;

/// `crate::testing`'s static data with skill templates as well (`<skill>` elements).
struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: if item_id == 57 { 4 } else { 5 },
            body_part: 0,
            quest_item: t.quest_item,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

/// Installs item and skill templates (the `<item>` and `<skill>` elements) in a test world.
pub fn set_items_and_skills(world: &mut World, items: &str, skills: &str) {
    let items = Arc::new(ItemTemplates::from_xml(&format!("<list>{items}</list>")).unwrap());
    let skills = Arc::new(SkillTemplates::from_xml(
        &format!("<list>{skills}</list>"),
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    ));
    world.set_static_data(Box::new(Data(StatData {
        items,
        skills,
        ..StatData::default()
    })));
}

/// Loads the zones of the test datapack (`data/zones/*.xml` pages) into the world.
pub fn load_zones(world: &mut World, root: &std::path::Path) {
    world.set_zone_data(Arc::new(Zones::load(root)), Arc::new(MapRegions::load(root)));
}

/// A square `NPoly` zone around `(x, y)` (half side `r`) between `min_z` and `max_z`.
pub fn square_zone(
    name: &str,
    id: i32,
    ty: &str,
    (x, y): (i32, i32),
    r: i32,
    z: (i32, i32),
    stats: &str,
) -> String {
    format!(
        r#"<zone name="{name}" id="{id}" type="{ty}" shape="NPoly" minZ="{}" maxZ="{}">{stats}
            <node X="{}" Y="{}" /><node X="{}" Y="{}" /><node X="{}" Y="{}" /><node X="{}" Y="{}" /></zone>"#,
        z.0,
        z.1,
        x - r,
        y - r,
        x + r,
        y - r,
        x + r,
        y + r,
        x - r,
        y + r
    )
}

/// A zone file of the test datapack with these `<zone>` elements.
pub fn zone_file(zones: &[String]) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><list enabled="true">{}</list>"#,
        zones.concat()
    )
}

/// Makes the players of `parties` (each a list of player ids, the first one its leader) a
/// command channel led by the first party's leader, who must carry a [`STRATEGY_GUIDE`].
pub fn command_channel(t: &mut crate::testing::ScriptTest, parties: &[&[i32]]) {
    for party in parties {
        for member in &party[1..] {
            t.party(party[0], *member);
        }
    }
    let leader = parties[0][0];
    for party in &parties[1..] {
        let name = t.world.player(ObjectId(party[0])).unwrap().data.name.clone();
        t.input(leader, ClientInput::AskJoinMpcc { name });
        t.input(party[0], ClientInput::AcceptJoinMpcc { response: 1 });
    }
    assert!(
        t.world.is_in_command_channel(ObjectId(leader)),
        "the command channel was not made"
    );
}
