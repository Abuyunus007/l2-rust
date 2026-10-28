//! The test bench of the class change scripts (unit tests only): one script on a few village
//! masters, stand-in pages that carry the real pages' links, players of a given class.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::subclass::ClassId;
use l2_world::{ItemData, PlayerData, Script, ScriptRegistry};

use crate::testing::{ScriptTest, pages_to, player_data};

pub use crate::testing::item;

/// The items the class change scripts give and take, as quest items (stackable where Java's are).
pub const ITEMS: &str = r#"
    <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
    <item id="8869" type="EtcItem" name="Shadow Item Exchange Coupon (D-Grade)"><set name="is_stackable" val="true"/></item>
    <item id="8870" type="EtcItem" name="Shadow Item Exchange Coupon (C-Grade)"><set name="is_stackable" val="true"/></item>"#;

/// The pages and NPCs of one script before the world is built.
pub struct VmBuilder {
    registry: ScriptRegistry,
    dir: String,
    name: String,
    npcs: Vec<(i32, String)>,
    pages: Vec<(String, String)>,
    items: String,
}

/// A world with one class change script and its village masters.
pub struct Vm {
    pub t: ScriptTest,
    name: String,
    pub npcs: HashMap<i32, ObjectId>,
}

impl Vm {
    /// `npcs` are `(npc id, NPC type)` of the masters to spawn.
    pub fn new(script: impl Script, dir: &str, name: &str, npcs: &[(i32, &str)]) -> VmBuilder {
        let mut registry = ScriptRegistry::new();
        registry.add(script);
        VmBuilder {
            registry,
            dir: dir.to_owned(),
            name: name.to_owned(),
            npcs: npcs.iter().map(|(id, kind)| (*id, (*kind).to_owned())).collect(),
            pages: Vec::new(),
            items: ITEMS.to_owned(),
        }
    }
}

impl VmBuilder {
    /// A stand-in page of the script folder with a `bypass -h Script <name> <event>` link for each
    /// of `links`.
    pub fn page(&mut self, file: &str, links: &[&str]) {
        let links: String = links
            .iter()
            .map(|e| format!(r#"<a action="bypass -h Script {} {e}">{e}</a>"#, self.name))
            .collect();
        self.pages.push((
            format!("{}/{file}", self.dir),
            format!("<html><body>{file}{links}</body></html>"),
        ));
    }

    /// A page anywhere in the datapack.
    pub fn raw_page(&mut self, path: &str, text: &str) {
        self.pages.push((path.to_owned(), text.to_owned()));
    }

    /// More `<item>` templates.
    pub fn items(&mut self, items: &str) {
        self.items.push_str(items);
    }

    /// Another script in the same world (registered after the tested one).
    pub fn also(&mut self, script: impl Script) {
        self.registry.add(script);
    }

    pub fn build(mut self) -> Vm {
        let mut npc_xml = String::new();
        for (id, kind) in &self.npcs {
            npc_xml.push_str(&format!(
                r#"<npc id="{id}" type="{kind}" level="70" name="Master {id}"/>"#
            ));
            // The master's own page links to the script like the real one
            // (`data/html/villagemaster/30066.htm`: `bypass -h Script ElfHumanFighterChange1`).
            let link = format!(r#"<a action="bypass -h Script {}">Class</a>"#, self.name);
            for folder in ["villagemaster", "default"] {
                self.pages.push((
                    format!("data/html/{folder}/{id}.htm"),
                    format!("<html><body>master {id} {link}</body></html>"),
                ));
            }
        }
        self.pages.push((
            "data/html/noquest.htm".to_owned(),
            "<html><body>noquest</body></html>".to_owned(),
        ));
        let pages: Vec<(&str, &str)> = self.pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(self.registry, &npc_xml, &self.items, &pages);
        let mut npcs = HashMap::new();
        for (i, (id, _)) in self.npcs.iter().enumerate() {
            let dx = 20 + 20 * i32::try_from(i).unwrap();
            npcs.insert(*id, t.spawn(*id, dx));
        }
        Vm {
            t,
            name: self.name,
            npcs,
        }
    }
}

/// A test character of `class_id` (base class too, the race of the class).
pub fn character(id: i32, level: i32, class_id: i32, items: Vec<ItemData>) -> PlayerData {
    let mut data = player_data(id, level, items);
    data.class_id = class_id;
    data.base_class = class_id;
    data.race = ClassId(u16::try_from(class_id).unwrap())
        .race()
        .map_or(0, |r| i32::from(r as u8));
    data
}

impl Vm {
    /// A character of `class_id` enters the world.
    pub fn enter(&mut self, id: i32, level: i32, class_id: i32, items: Vec<ItemData>) {
        self.t.enter_with(character(id, level, class_id, items));
        self.t.take();
    }

    /// Talks to the master and opens its quest link; the last page shown.
    pub fn talk(&mut self, player: i32, npc_id: i32) -> String {
        let npc = self.npcs[&npc_id];
        self.t.talk(player, npc);
        self.t.take();
        let name = self.name.clone();
        self.t.bypass(player, &format!("Script {name}"));
        pages_to(&self.t.take(), player).pop().unwrap_or_default()
    }

    /// Clicks a link of the last page (`bypass -h Script <name> <event>`); the last page shown.
    pub fn click(&mut self, player: i32, event: &str) -> String {
        let name = self.name.clone();
        self.t.bypass(player, &format!("Script {name} {event}"));
        pages_to(&self.t.take(), player).pop().unwrap_or_default()
    }

    /// The active and the base class of the player.
    pub fn assert_class(&self, player: i32, class_id: i32) {
        let p = self.t.world.player(ObjectId(player)).unwrap();
        assert_eq!((p.data.class_id, p.data.base_class), (class_id, class_id));
    }
}
