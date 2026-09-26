//! Floor-item list for the current level.
//!
//! Replaces the legacy C `lvl_obj` global (a raw `THING *` list head) with a
//! small owned handle stored on [`crate::level::Level`].

use crate::entity::player::Thing;
use crate::entity::player::{attach, detach, free_list};
use std::sync::atomic::{AtomicPtr, Ordering};

/// Intrusive linked list of the floor items (objects) for the current level.
///
/// Holds the head of the `l_next`/`l_prev` chain of [`Thing`] objects resting
/// on the level floor. The allocations themselves are owned by
/// [`crate::item::arena::OBJECTS`]; this type is only a handle to the head
/// pointer, so cloning shares the same list and `Debug`/`Eq` compare the head
/// address, matching `Level`'s derives.
///
/// The head is stored in an [`AtomicPtr`] rather than a bare `*mut Thing`. The
/// game is single-threaded, but `Level` (which owns this list) is reachable
/// through `CURRENT_LEVEL`, so the list must be `Send`/`Sync`. `AtomicPtr<T>`
/// is unconditionally `Send + Sync`, which removes the need for an
/// `unsafe impl` on this type while keeping the same pointer-as-handle model.
#[derive(Debug)]
pub struct ItemList {
    head: AtomicPtr<Thing>,
}

impl ItemList {
    /// An empty list.
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Head of the floor-item list.
    pub fn head(&self) -> *mut Thing {
        self.head.load(Ordering::Relaxed)
    }

    /// Prepend `item` to the floor-item list.
    pub unsafe fn attach(&mut self, item: *mut Thing) {
        attach(self.head.get_mut(), item);
    }

    /// Unlink `item` from the floor-item list.
    pub unsafe fn detach(&mut self, item: *mut Thing) {
        detach(self.head.get_mut(), item);
    }

    /// Drop every item in the list.
    pub unsafe fn clear(&mut self) {
        free_list(self.head.get_mut());
    }

    /// Replace the list head (used by save/restore).
    pub fn set_head(&mut self, head: *mut Thing) {
        *self.head.get_mut() = head;
    }
}

impl Default for ItemList {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ItemList {
    fn clone(&self) -> Self {
        // A copy shares the same underlying list via the head address, matching
        // the previous pointer-copy semantics.
        Self {
            head: AtomicPtr::new(self.head()),
        }
    }
}

impl PartialEq for ItemList {
    fn eq(&self, other: &Self) -> bool {
        self.head() == other.head()
    }
}

impl Eq for ItemList {}
