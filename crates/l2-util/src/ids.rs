//! Identifier newtypes.
//!
//! Every cross-reference between game objects is an id, never a pointer (D-11).

use std::fmt;

use serde::{Deserialize, Serialize};

/// The id of a world object (player, NPC, item, door, ...), as the client sees it.
///
/// Java uses a plain `int`. Ids are allocated by the IdManager and are never reused while
/// an object exists.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(pub i32);

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", self.0)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i32> for ObjectId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_serializes_as_plain_number() {
        let json = serde_json::to_string(&ObjectId(268_435_456)).unwrap();
        assert_eq!(json, "268435456");
        let back: ObjectId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ObjectId(268_435_456));
    }
}
