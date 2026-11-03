//! The list, set and map getters the generated game config uses (M9-L03).
// Java: gameserver/config/**/*Config.java (the `split(..)` loops in `load()`)
//
// The Java config classes have no list getter: each one splits a `getString` by hand. The
// generator (since removed) knew six shapes and emits a call to one of the
// `IniFile` methods below instead of a `()` placeholder:
//
// | Java type              | method                | separators                       |
// |------------------------|-----------------------|----------------------------------|
// | `Set<Integer>`         | `get_int_set`         | `,`                              |
// | `List<Integer>`        | `get_int_list`        | `,`                              |
// | `Map<Integer,Integer>` | `get_int_map`         | `;` between pairs, `,` inside    |
// | `List<ItemHolder>`     | `get_item_holder_list`| `;` between items, `,` inside    |
// | `Map<String,Location>` | `get_location_map`    | `;` between entries, `,` inside  |
// | `List<String>`         | `get_string_list`     | one char the generator passes    |
//
// Error handling: Java calls `Integer.parseInt` on every piece, so a broken piece throws
// `NumberFormatException` and the server does not start. A generated `from_ini` cannot fail, so
// every parser here skips a broken piece with a warning and keeps the others (the same choice the
// earlier hand-ported lists made). On valid input the result is what Java builds.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tracing::warn;

use crate::IniFile;
use crate::game_manual::{Location, java_split, java_trim};

/// A Java `ItemHolder`: an item id and an amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemHolder {
    pub id: i32,
    pub count: i64,
}

/// Pieces of `text` split at `separator` the way Java's `split` does, with `String.trim()` applied
/// to each and the empty ones dropped, parsed as `i32`. A piece that is not a number is skipped.
fn int_pieces(file: &str, key: &str, text: &str, separator: char) -> Vec<i32> {
    let mut out = Vec::new();
    for piece in java_split(text, separator) {
        let piece = java_trim(piece);
        if piece.is_empty() {
            continue;
        }
        match piece.parse::<i32>() {
            Ok(value) => out.push(value),
            Err(_) => warn!("{file}: {key} has an invalid number: {piece:?}"),
        }
    }
    out
}

impl IniFile {
    /// A `Set<Integer>` read from `a,b,c` (`getString` + `split(",")` + `Integer.parseInt`).
    /// Blanks around a number and empty pieces are accepted; a broken piece is skipped.
    pub fn get_int_set(&self, key: &str, default: &str) -> BTreeSet<i32> {
        self.get_int_list(key, default).into_iter().collect()
    }

    /// A `List<Integer>` read from `a,b,c`, in file order (duplicates kept).
    pub fn get_int_list(&self, key: &str, default: &str) -> Vec<i32> {
        int_pieces(self.file_name(), key, &self.get_string(key, default), ',')
    }

    /// A `Map<Integer, Integer>` read from `a,b;c,d`. A pair needs exactly two numbers; a later
    /// pair with the same key replaces the earlier one (`HashMap.put`).
    // NOTE(java-quirk): Java uses the first two numbers of a longer pair after warning, and
    // throws `ArrayIndexOutOfBoundsException` for a one-number pair; here the one-number pair is
    // skipped. Empty pieces are skipped without a warning (the empty default makes Java swallow a
    // `NumberFormatException` silently).
    pub fn get_int_map(&self, key: &str, default: &str) -> HashMap<i32, i32> {
        let mut map = HashMap::new();
        for pair in java_split(&self.get_string(key, default), ';') {
            if pair.is_empty() {
                continue;
            }
            let pieces = java_split(pair, ',');
            if pieces.len() != 2 {
                warn!("{}: {key} has an invalid pair: {pair:?}", self.file_name());
            }
            let (Some(a), Some(b)) = (pieces.first(), pieces.get(1)) else {
                continue;
            };
            match (a.parse::<i32>(), b.parse::<i32>()) {
                (Ok(a), Ok(b)) => {
                    map.insert(a, b);
                }
                _ => warn!("{}: {key} has an invalid pair: {pair:?}", self.file_name()),
            }
        }
        map
    }

    /// A `List<ItemHolder>` read from `id,count;id,count`. Empty pieces are skipped; a piece
    /// without two numbers is skipped with a warning. Only the first two numbers count.
    pub fn get_item_holder_list(&self, key: &str, default: &str) -> Vec<ItemHolder> {
        let mut items = Vec::new();
        for entry in java_split(&self.get_string(key, default), ';') {
            if entry.is_empty() {
                continue;
            }
            let pieces = java_split(entry, ',');
            let id = pieces.first().and_then(|p| p.parse::<i32>().ok());
            let count = pieces.get(1).and_then(|p| p.parse::<i64>().ok());
            match (id, count) {
                (Some(id), Some(count)) => items.push(ItemHolder { id, count }),
                _ => warn!("{}: {key} has an invalid item: {entry:?}", self.file_name()),
            }
        }
        items
    }

    /// A `Map<String, Location>` read from `name,x,y,z;name,x,y,z` (a fourth number is the
    /// heading). The name is kept as written; a later entry with the same name replaces the
    /// earlier one.
    pub fn get_location_map(&self, key: &str, default: &str) -> BTreeMap<String, Location> {
        let mut map = BTreeMap::new();
        for entry in java_split(&self.get_string(key, default), ';') {
            if entry.is_empty() {
                continue;
            }
            let pieces = java_split(entry, ',');
            let number = |at: usize| pieces.get(at).and_then(|p| p.parse::<i32>().ok());
            match (pieces.first(), number(1), number(2), number(3)) {
                (Some(name), Some(x), Some(y), Some(z)) => {
                    let heading = number(4).unwrap_or(0);
                    map.insert((*name).to_owned(), Location { x, y, z, heading });
                }
                _ => warn!("{}: {key} has an invalid location: {entry:?}", self.file_name()),
            }
        }
        map
    }

    /// A `List<String>` read from `a<separator>b`. A blank value is the empty list; the pieces are
    /// kept as written (Java's `Arrays.asList(value.split(";"))`, no trimming).
    pub fn get_string_list(&self, key: &str, default: &str, separator: char) -> Vec<String> {
        let value = self.get_string(key, default);
        if java_trim(&value).is_empty() {
            return Vec::new();
        }
        java_split(&value, separator)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> IniFile {
        IniFile::parse("test.ini", text).unwrap()
    }

    #[test]
    fn int_sets_and_lists() {
        let f = ini("A = 3, 1,2,,x,1\nB =\n");
        assert_eq!(f.get_int_set("A", ""), BTreeSet::from([1, 2, 3]));
        assert_eq!(f.get_int_list("A", ""), [3, 1, 2, 1]);
        assert!(f.get_int_set("B", "9").is_empty());
        // Missing key: the default is parsed.
        assert_eq!(f.get_int_set("C", "0"), BTreeSet::from([0]));
        assert_eq!(f.get_int_list("C", "30827"), [30827]);
    }

    #[test]
    fn int_map_keeps_the_last_pair_and_skips_broken_ones() {
        let f = ini("M = 1,2;x,3;4;5, 6;7,8,9;1,10;\n");
        let map = f.get_int_map("M", "");
        assert_eq!(map, HashMap::from([(1, 10), (7, 8)]));
        assert!(f.get_int_map("none", "").is_empty());
    }

    #[test]
    fn item_holders() {
        let f = ini("I = 4356,10;;4357,5000000000;bad;1\n");
        assert_eq!(
            f.get_item_holder_list("I", ""),
            [
                ItemHolder { id: 4356, count: 10 },
                ItemHolder {
                    id: 4357,
                    count: 5_000_000_000
                }
            ]
        );
        assert_eq!(
            f.get_item_holder_list("none", "4356,10"),
            [ItemHolder { id: 4356, count: 10 }]
        );
    }

    #[test]
    fn locations() {
        let f = ini("L = Giran,82698,148638,-3473;Bad,1,2;Home,1,2,3,4;\n");
        let map = f.get_location_map("L", "");
        assert_eq!(map.len(), 2);
        assert_eq!(
            map["Giran"],
            Location {
                x: 82698,
                y: 148638,
                z: -3473,
                heading: 0
            }
        );
        assert_eq!(
            map["Home"],
            Location {
                x: 1,
                y: 2,
                z: 3,
                heading: 4
            }
        );
    }

    #[test]
    fn string_lists() {
        let f = ini("S = a,1;b ;c\nE =  \n");
        assert_eq!(f.get_string_list("S", "", ';'), ["a,1", "b ", "c"]);
        assert!(f.get_string_list("E", "x", ';').is_empty());
        assert_eq!(f.get_string_list("none", "q", ';'), ["q"]);
    }
}
