//! Interior-mutable owner for the live monster list (`mlist`).
//!
//! The original C engine kept the monster linked list in a raw `mlist` global.
//! This wrapper replaces that global with an `RwLock`-backed cell holding the
//! raw head pointer, so the static is `Sync` while the gameplay code still
//! traverses the list through raw pointers on the single game thread.

use crate::entity::player::CThing;
use crate::item::thing_list::{
    attach as attach_thing, detach as detach_thing, free_list as free_thing_list,
};
use std::sync::RwLock;

/// Head of the monster linked list.
pub struct MonsterList(RwLock<*mut CThing>);

// The raw pointer is neither Send nor Sync; access is always guarded by the
// inner RwLock and dereferenced only by the single-threaded gameplay loop.
unsafe impl Send for MonsterList {}
unsafe impl Sync for MonsterList {}

impl MonsterList {
    const fn new() -> Self {
        Self(RwLock::new(std::ptr::null_mut()))
    }

    /// Read the current list head.
    #[inline]
    pub fn head(&self) -> *mut CThing {
        *self.0.read().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Replace the list head.
    #[inline]
    pub fn set_head(&self, ptr: *mut CThing) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = ptr;
    }

    /// Prepend `item` to the list.
    #[inline]
    pub unsafe fn attach(&self, item: *mut CThing) {
        let mut guard = self
            .0
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        attach_thing(&raw mut *guard, item);
    }

    /// Unlink `item` from the list.
    #[inline]
    pub unsafe fn detach(&self, item: *mut CThing) {
        let mut guard = self
            .0
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        detach_thing(&raw mut *guard, item);
    }

    /// Drop every item in the list.
    #[inline]
    pub unsafe fn free_list(&self) {
        let mut guard = self
            .0
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        free_thing_list(&raw mut *guard);
    }
}

/// The monster list for the live level.
pub static MLIST: MonsterList = MonsterList::new();