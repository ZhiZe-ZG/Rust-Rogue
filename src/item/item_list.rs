//! Floor-item list for the current level.
//!
//! Replaces the legacy C `lvl_obj` global (a raw `THING *` list head) with a
//! small owned handle stored on [`crate::level::Level`].

use crate::entity::player::CThing;
use crate::item::thing_list::{attach, detach, free_list};

/// Intrusive linked list of the floor items (objects) for the current level.
///
/// Holds the head of the `l_next`/`l_prev` chain of [`CThing`] objects resting
/// on the level floor. The allocations themselves are owned by
/// [`crate::item::thing_list`]; this type is only a handle to the head pointer,
/// so it is `Copy`-like (cloning shares the same list) and its `Debug`/`Eq`
/// implementations compare the head address, matching `Level`'s derives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemList {
    head: *mut CThing,
}

// The raw head pointer is neither `Send` nor `Sync`, but the game is
// single-threaded and the pointer is only dereferenced by the gameplay loop.
// `Level` (which owns this list) is shared through `CURRENT_LEVEL`'s `RwLock`,
// so opting in here matches `MonsterList` and `Equipment`.
unsafe impl Send for ItemList {}
unsafe impl Sync for ItemList {}

impl ItemList {
    /// An empty list.
    pub const fn new() -> Self {
        Self {
            head: std::ptr::null_mut(),
        }
    }

    /// Head of the floor-item list.
    pub fn head(&self) -> *mut CThing {
        self.head
    }

    /// Prepend `item` to the floor-item list.
    pub unsafe fn attach(&mut self, item: *mut CThing) {
        attach(&mut self.head, item);
    }

    /// Unlink `item` from the floor-item list.
    pub unsafe fn detach(&mut self, item: *mut CThing) {
        detach(&mut self.head, item);
    }

    /// Drop every item in the list.
    pub unsafe fn clear(&mut self) {
        free_list(&mut self.head);
    }

    /// Replace the list head (used by save/restore).
    pub fn set_head(&mut self, head: *mut CThing) {
        self.head = head;
    }
}
