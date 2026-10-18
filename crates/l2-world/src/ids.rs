//! Object id allocation: the Rust game server's IdFactory (D-39).
// Java: org/l2jmobius/gameserver/managers/IdManager.java (allocation: BitSet, next free bit)
// Java: org/l2jmobius/gameserver/managers/DatabaseIdManager.java (the startup scan lives in `l2-db`)
//
// Why a separate range: the Java GS and ours run at the same time on the same database (D-39).
// Java hands out ids from `FirstObjectId` (0x10000000) upwards, always the lowest free one, so it
// needs more than a billion ids before it could reach 0x60000000. We allocate only from
// 0x60000000..=0x7FFFFFFF. Neither allocator can then hand out an id the other one owns, without
// the two servers talking to each other.
//
// This module has no dependencies and takes no database handle: the game server runs the scan
// (`l2_db::IdRepository::used_object_ids`) and passes the ids in.

use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use l2_util::ObjectId;
use tracing::{debug, warn};

/// First object id the Rust game server may allocate (D-39).
pub const RUST_FIRST_OBJECT_ID: i32 = 0x6000_0000;
/// Last object id the Rust game server may allocate: Java's `LastObjectId` default, the largest
/// positive `int`.
pub const RUST_LAST_OBJECT_ID: i32 = 0x7FFF_FFFF;

/// Java's default `FirstObjectId` (`IdManager.ini`). Ids below [`RUST_FIRST_OBJECT_ID`] belong to
/// the Java GS; this server never allocates, scans for or releases them.
pub const JAVA_FIRST_OBJECT_ID: i32 = 0x1000_0000;

/// Why an [`IdFactory`] could not be built or could not hand out an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// Every id of the range is in use. At startup this means the server must not start.
    // Java: IdManager.getNextId throws NullPointerException("IdManager: Ran out of valid ids.")
    Exhausted,
    /// `first` is negative or greater than `last`.
    InvalidRange,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => f.write_str("ran out of valid object ids"),
            Self::InvalidRange => f.write_str("invalid object id range"),
        }
    }
}

impl std::error::Error for IdError {}

/// Thread-safe object id allocator over one contiguous range.
///
/// Share it as `Arc<IdFactory>`: the world loop allocates ids for spawned objects and dropped
/// items, and the client tasks allocate the id of a new character before the world knows the
/// player. It is infrastructure, not game state: nothing in it refers to a game object.
///
/// Ids are only unique among the ids that were *passed in at startup* and allocated since.
/// Whoever allocates an id and then fails to use it must [`release`](Self::release) it.
///
/// Behaviour copied from Java's `IdManager`: the next id is the first free one after the id
/// handed out last, wrapping to the lowest free one when the end of the range is reached. A
/// released id is therefore not handed out again until the allocator wraps (or the server is
/// restarted and the database scan no longer finds it).
#[derive(Debug)]
pub struct IdFactory {
    first: i32,
    total: u32,
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// Bit `n` is set when id `first + n` is in use. Grows on demand, so a nearly empty range
    /// costs next to nothing; bits past the end are free.
    words: Vec<u64>,
    /// Number of set bits.
    used: u32,
    /// The offset the next allocation returns. `None` only when every id is in use.
    next: Option<u32>,
}

impl Inner {
    fn is_set(&self, offset: u32) -> bool {
        self.words
            .get((offset / 64) as usize)
            .is_some_and(|w| w & (1u64 << (offset % 64)) != 0)
    }

    /// Sets the bit; returns false when it was already set.
    fn set(&mut self, offset: u32) -> bool {
        let word = (offset / 64) as usize;
        if self.words.len() <= word {
            self.words.resize(word + 1, 0);
        }
        let mask = 1u64 << (offset % 64);
        let was_clear = self.words[word] & mask == 0;
        self.words[word] |= mask;
        was_clear
    }

    /// Clears the bit; returns false when it was already clear.
    fn clear(&mut self, offset: u32) -> bool {
        match self.words.get_mut((offset / 64) as usize) {
            Some(w) if *w & (1u64 << (offset % 64)) != 0 => {
                *w &= !(1u64 << (offset % 64));
                true
            }
            _ => false,
        }
    }

    /// The first clear bit at or after `from` and below `total`.
    // Java: BitSet.nextClearBit
    fn next_clear_from(&self, from: u32, total: u32) -> Option<u32> {
        let mut word = (from / 64) as usize;
        let mut shift = from % 64;
        while let Some(&w) = self.words.get(word) {
            let free = !w & (!0u64 << shift);
            if free != 0 {
                // `word` indexes `words`, whose length is bounded by `total / 64 + 1`.
                let found = word as u32 * 64 + free.trailing_zeros();
                return (found < total).then_some(found);
            }
            word += 1;
            shift = 0;
        }
        // Past the end of the stored words every bit is clear.
        let found = (word as u32).saturating_mul(64).max(from);
        (found < total).then_some(found)
    }
}

impl IdFactory {
    /// The factory for the Rust range ([`RUST_FIRST_OBJECT_ID`]..=[`RUST_LAST_OBJECT_ID`]),
    /// with the ids the startup scan found in use.
    ///
    /// Ids outside the range are ignored (they are the Java GS's). Fails with
    /// [`IdError::Exhausted`] when the scan shows the whole range used: the server must then
    /// refuse to start.
    // Java: IdManager constructor
    pub fn new(used_ids: impl IntoIterator<Item = i32>) -> Result<Self, IdError> {
        Self::with_range(RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID, used_ids)
    }

    /// Like [`new`](Self::new) for another range (tests, or a future config key).
    pub fn with_range(
        first: i32,
        last: i32,
        used_ids: impl IntoIterator<Item = i32>,
    ) -> Result<Self, IdError> {
        if first < 0 || first > last {
            return Err(IdError::InvalidRange);
        }
        // Both are non-negative, so the difference fits `u32`; the range holds at most 2^31 ids.
        let total = (last - first) as u32 + 1;
        let mut inner = Inner {
            words: Vec::new(),
            used: 0,
            next: None,
        };
        for id in used_ids {
            if id < first || id > last {
                continue;
            }
            // In range, so the offset is below `total`. Duplicates (an id in two tables) count once.
            if inner.set((id - first) as u32) {
                inner.used += 1;
            }
        }
        if inner.used >= total {
            return Err(IdError::Exhausted);
        }
        inner.next = inner.next_clear_from(0, total);
        Ok(Self {
            first,
            total,
            inner: Mutex::new(inner),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // No code path panics while holding the lock, and every update leaves `Inner` consistent
        // after each statement, so a poisoned lock still holds usable data.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The first id of the range.
    pub fn first(&self) -> ObjectId {
        ObjectId(self.first)
    }

    /// The last id of the range.
    pub fn last(&self) -> ObjectId {
        // `first + total - 1` is the `last` given to the constructor, so it fits `i32`.
        ObjectId(self.first + (self.total - 1) as i32)
    }

    /// Whether `id` lies in this factory's range.
    pub fn contains(&self, id: ObjectId) -> bool {
        self.offset(id).is_some()
    }

    fn offset(&self, id: ObjectId) -> Option<u32> {
        let offset = i64::from(id.0) - i64::from(self.first);
        u32::try_from(offset).ok().filter(|&o| o < self.total)
    }

    /// Allocates the next free id.
    // Java: IdManager.getNextId
    pub fn allocate(&self) -> Result<ObjectId, IdError> {
        let mut inner = self.lock();
        let offset = inner.next.ok_or(IdError::Exhausted)?;
        inner.set(offset);
        inner.used += 1;
        // The first free id after the one just handed out, else the lowest free one. Java's
        // `nextClearBit(newId)` then `nextClearBit(0)`.
        inner.next = if inner.used >= self.total {
            None
        } else {
            let after = offset + 1;
            inner
                .next_clear_from(after, self.total)
                .or_else(|| inner.next_clear_from(0, self.total))
        };
        // `offset < total <= 2^31`, so it fits `i32`, and `first + offset <= last`.
        Ok(ObjectId(self.first + offset as i32))
    }

    /// Gives an id back. Returns whether it was an allocated id of this range.
    ///
    /// Ids outside the range are ignored: they belong to the Java GS (a character it created and
    /// the Rust GS deletes, say), whose allocator reuses them after its own next restart scan.
    // Java: IdManager.releaseId
    // NOTE(java-quirk): Java increments its free counter even for an id that was never allocated
    // or was already released, which makes the counter drift. Here only a set bit is cleared and
    // counted, so a double release is harmless.
    pub fn release(&self, id: ObjectId) -> bool {
        let Some(offset) = self.offset(id) else {
            debug!(
                "IdFactory: release of {id} ignored (outside {}..={})",
                self.first,
                self.last()
            );
            return false;
        };
        let mut inner = self.lock();
        if !inner.clear(offset) {
            warn!("IdFactory: release of {id} ignored (not allocated)");
            return false;
        }
        inner.used -= 1;
        // Java leaves the cursor alone; it only matters once the range was full, when there is
        // no cursor and the freed id is the only candidate.
        if inner.next.is_none() {
            inner.next = Some(offset);
        }
        true
    }

    /// Whether `id` is currently in use (allocated, or found by the startup scan).
    pub fn is_allocated(&self, id: ObjectId) -> bool {
        self.offset(id).is_some_and(|o| self.lock().is_set(o))
    }

    /// How many ids are in use.
    pub fn used_count(&self) -> u32 {
        self.lock().used
    }

    /// How many ids are left.
    // Java: IdManager.getAvailableIdCount
    pub fn available_count(&self) -> u32 {
        self.total - self.lock().used
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    use super::*;

    fn small(first: i32, last: i32, used: &[i32]) -> IdFactory {
        IdFactory::with_range(first, last, used.iter().copied()).unwrap()
    }

    #[test]
    fn rust_range_is_the_high_half_and_does_not_start_at_javas_first_id() {
        assert_eq!(RUST_FIRST_OBJECT_ID, 1_610_612_736);
        assert_eq!(RUST_LAST_OBJECT_ID, i32::MAX);
        const { assert!(RUST_FIRST_OBJECT_ID > JAVA_FIRST_OBJECT_ID) };
        let ids = IdFactory::new([]).unwrap();
        assert_eq!(ids.first(), ObjectId(RUST_FIRST_OBJECT_ID));
        assert_eq!(ids.last(), ObjectId(RUST_LAST_OBJECT_ID));
        assert_eq!(ids.available_count(), 0x2000_0000);
        assert_eq!(ids.allocate().unwrap(), ObjectId(RUST_FIRST_OBJECT_ID));
    }

    #[test]
    fn java_range_ids_in_the_scan_are_ignored() {
        let ids = IdFactory::new([268_435_456, 268_435_457, 1_000_000_000, -5, 0]).unwrap();
        assert_eq!(ids.used_count(), 0);
        assert_eq!(ids.allocate().unwrap(), ObjectId(RUST_FIRST_OBJECT_ID));
    }

    #[test]
    fn allocation_skips_used_ids_and_counts_duplicates_once() {
        let first = RUST_FIRST_OBJECT_ID;
        // The same id in two tables (items and itemsonground, say) is one used id.
        let ids = IdFactory::new([first, first + 1, first + 1, first + 3]).unwrap();
        assert_eq!(ids.used_count(), 3);
        assert_eq!(ids.allocate().unwrap(), ObjectId(first + 2));
        assert_eq!(ids.allocate().unwrap(), ObjectId(first + 4));
        assert_eq!(ids.allocate().unwrap(), ObjectId(first + 5));
        assert_eq!(ids.used_count(), 6);
    }

    #[test]
    fn allocation_works_across_word_boundaries() {
        let ids = small(1000, 1999, &[]);
        let got: Vec<i32> = (0..200).map(|_| ids.allocate().unwrap().0).collect();
        assert_eq!(got, (1000..1200).collect::<Vec<_>>());
        // Fill the bits 63..=65 around a word edge first, then continue.
        let ids = small(0, 1000, &(0..64).collect::<Vec<_>>());
        assert_eq!(ids.allocate().unwrap(), ObjectId(64));
        let ids = small(0, 1000, &(0..128).collect::<Vec<_>>());
        assert_eq!(ids.allocate().unwrap(), ObjectId(128));
    }

    #[test]
    fn released_ids_are_reused_only_after_the_range_wraps() {
        let ids = small(10, 14, &[]);
        let a = ids.allocate().unwrap();
        let b = ids.allocate().unwrap();
        assert_eq!((a, b), (ObjectId(10), ObjectId(11)));
        assert!(ids.release(a));
        // Java keeps going upwards...
        assert_eq!(ids.allocate().unwrap(), ObjectId(12));
        assert_eq!(ids.allocate().unwrap(), ObjectId(13));
        assert_eq!(ids.allocate().unwrap(), ObjectId(14));
        // ...and wraps to the lowest free id at the end of the range.
        assert_eq!(ids.allocate().unwrap(), ObjectId(10));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
    }

    #[test]
    fn exhaustion_is_an_error_and_a_release_recovers() {
        let ids = small(5, 7, &[]);
        for _ in 0..3 {
            ids.allocate().unwrap();
        }
        assert_eq!(ids.available_count(), 0);
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
        assert!(ids.release(ObjectId(6)));
        assert_eq!(ids.available_count(), 1);
        assert_eq!(ids.allocate().unwrap(), ObjectId(6));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
    }

    #[test]
    fn a_full_range_at_startup_is_refused() {
        assert_eq!(
            IdFactory::with_range(5, 7, [5, 6, 7]).unwrap_err(),
            IdError::Exhausted
        );
        assert!(IdFactory::with_range(5, 7, [5, 6]).is_ok());
    }

    #[test]
    fn invalid_ranges_are_refused() {
        assert_eq!(
            IdFactory::with_range(10, 9, []).unwrap_err(),
            IdError::InvalidRange
        );
        assert_eq!(
            IdFactory::with_range(-1, 9, []).unwrap_err(),
            IdError::InvalidRange
        );
        assert!(IdFactory::with_range(7, 7, []).is_ok());
    }

    #[test]
    fn release_ignores_foreign_unallocated_and_double_released_ids() {
        let ids = small(100, 109, &[]);
        let a = ids.allocate().unwrap();
        // Java GS id, below and above the range.
        assert!(!ids.release(ObjectId(268_435_456)));
        assert!(!ids.release(ObjectId(99)));
        assert!(!ids.release(ObjectId(110)));
        // Never allocated.
        assert!(!ids.release(ObjectId(105)));
        assert!(ids.release(a));
        assert!(!ids.release(a));
        assert_eq!(ids.used_count(), 0);
        assert_eq!(ids.available_count(), 10);
    }

    #[test]
    fn is_allocated_and_contains() {
        let ids = small(100, 109, &[103]);
        assert!(ids.is_allocated(ObjectId(103)));
        assert!(!ids.is_allocated(ObjectId(104)));
        assert!(!ids.is_allocated(ObjectId(5)));
        assert!(ids.contains(ObjectId(100)));
        assert!(ids.contains(ObjectId(109)));
        assert!(!ids.contains(ObjectId(110)));
    }

    #[test]
    fn single_id_range() {
        let ids = small(7, 7, &[]);
        assert_eq!(ids.allocate().unwrap(), ObjectId(7));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
        assert!(ids.release(ObjectId(7)));
        assert_eq!(ids.allocate().unwrap(), ObjectId(7));
    }

    #[test]
    fn the_last_id_of_the_default_range_can_be_allocated() {
        // Everything but the last id is used; the scan would never list half a billion ids, so
        // use a small range at the top of the positive ints instead.
        let last = i32::MAX;
        let ids = small(last - 2, last, &[last - 2]);
        assert_eq!(ids.allocate().unwrap(), ObjectId(last - 1));
        assert_eq!(ids.allocate().unwrap(), ObjectId(last));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
    }

    #[test]
    fn threads_never_get_the_same_id() {
        let ids = Arc::new(IdFactory::new([]).unwrap());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let ids = Arc::clone(&ids);
                thread::spawn(move || (0..2000).map(|_| ids.allocate().unwrap().0).collect::<Vec<_>>())
            })
            .collect();
        let mut seen = HashSet::new();
        for handle in handles {
            for id in handle.join().unwrap() {
                assert!(seen.insert(id), "id {id} handed out twice");
                assert!(id >= RUST_FIRST_OBJECT_ID);
            }
        }
        assert_eq!(seen.len(), 16_000);
        assert_eq!(ids.used_count(), 16_000);
    }

    #[test]
    fn threads_can_allocate_and_release_together() {
        let ids = Arc::new(small(0, 99, &[]));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let ids = Arc::clone(&ids);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let id = ids.allocate().unwrap();
                        assert!(ids.release(id));
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(ids.used_count(), 0);
        assert_eq!(ids.available_count(), 100);
    }
}
