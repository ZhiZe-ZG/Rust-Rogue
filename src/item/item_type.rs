//! The typed item-kind enum that replaces the legacy per-item ASCII code
//! (`'!'`, `'?'`, `':'`, `')'`, `']'`, `'='`, `'/'`, `'*'`, `','`).
//!
//! Every item category is a distinct variant carrying its own sub-type (for the
//! categories that have one). The legacy ASCII code and the `o_which` sub-type
//! index are still what the save format and the object-info tables key off, so
//! [`ItemType::code`] and [`ItemType::which`] bridge back to them.
//!
//! Rendering an item kind to a screen character is deliberately *not* a method
//! here in the sense of "presentation"; the single mapping from [`ItemType`] to
//! the drawn glyph lives in [`crate::draw::item_glyph`]. The [`ItemType::code`]
//! accessor only exposes the legacy numeric identity used by serialization.

use crate::item::potions::PotionType;
use crate::item::rings::RingType;
use crate::item::scrolls::ScrollType;
use crate::item::sticks::StickType;

const POTION_CODE: i32 = b'!' as i32;
const SCROLL_CODE: i32 = b'?' as i32;
const FOOD_CODE: i32 = b':' as i32;
const WEAPON_CODE: i32 = b')' as i32;
const ARMOR_CODE: i32 = b']' as i32;
const RING_CODE: i32 = b'=' as i32;
const STICK_CODE: i32 = b'/' as i32;
const GOLD_CODE: i32 = b'*' as i32;
const AMULET_CODE: i32 = b',' as i32;

/// The kind of an object (item), replacing the legacy `o_type` ASCII code.
///
/// The `Potion`/`Scroll`/`Ring`/`Stick` variants carry the sub-type; `Weapon`
/// and `Armor` carry the legacy `o_which` index (there is no weapon/armor
/// sub-type enum yet). `None` is the transient "unset" state (a raw `0`), used
/// by freshly allocated objects before their kind is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ItemType {
    #[default]
    None,
    Potion(PotionType),
    Scroll(ScrollType),
    Food,
    Weapon(i32),
    Armor(i32),
    Ring(RingType),
    Stick(StickType),
    Gold,
    Amulet,
}

impl ItemType {
    /// A representative potion kind (used for category filters).
    pub const POTION: ItemType = ItemType::Potion(PotionType::Confuse);
    /// A representative scroll kind (used for category filters).
    pub const SCROLL: ItemType = ItemType::Scroll(ScrollType::Confuse);
    /// The food kind.
    pub const FOOD: ItemType = ItemType::Food;
    /// A representative weapon kind (used for category filters).
    pub const WEAPON: ItemType = ItemType::Weapon(0);
    /// A representative armor kind (used for category filters).
    pub const ARMOR: ItemType = ItemType::Armor(0);
    /// A representative ring kind (used for category filters).
    pub const RING: ItemType = ItemType::Ring(RingType::Protection);
    /// A representative wand/staff kind (used for category filters).
    pub const STICK: ItemType = ItemType::Stick(StickType::Light);
    /// The gold kind.
    pub const GOLD: ItemType = ItemType::Gold;
    /// The Amulet of Yendor kind.
    pub const AMULET: ItemType = ItemType::Amulet;

    /// The legacy ASCII code for this kind (`0` for [`ItemType::None`]).
    ///
    /// Only the serialization layer and [`crate::draw::item_glyph`] should use
    /// this; gameplay code matches on the variants directly.
    pub const fn code(self) -> i32 {
        match self {
            ItemType::None => 0,
            ItemType::Potion(_) => POTION_CODE,
            ItemType::Scroll(_) => SCROLL_CODE,
            ItemType::Food => FOOD_CODE,
            ItemType::Weapon(_) => WEAPON_CODE,
            ItemType::Armor(_) => ARMOR_CODE,
            ItemType::Ring(_) => RING_CODE,
            ItemType::Stick(_) => STICK_CODE,
            ItemType::Gold => GOLD_CODE,
            ItemType::Amulet => AMULET_CODE,
        }
    }

    /// The legacy `o_which` sub-type index this kind corresponds to.
    pub fn which(self) -> i32 {
        match self {
            ItemType::Potion(p) => p.index() as i32,
            ItemType::Scroll(s) => s.index() as i32,
            ItemType::Ring(r) => r.index() as i32,
            ItemType::Stick(s) => s.index() as i32,
            ItemType::Weapon(w) | ItemType::Armor(w) => w,
            _ => 0,
        }
    }

    /// Build a kind from a legacy ASCII code and `o_which` index.
    ///
    /// Unknown codes (and unparsable sub-type indices) collapse to
    /// [`ItemType::None`]; the wizard-mode `create_obj` command is the only
    /// source of arbitrary codes.
    pub fn from_raw(code: i32, which: i32) -> ItemType {
        match code {
            0 => ItemType::None,
            POTION_CODE => match PotionType::try_from_raw(which) {
                Some(p) => ItemType::Potion(p),
                None => ItemType::None,
            },
            SCROLL_CODE => match ScrollType::try_from_raw(which) {
                Some(s) => ItemType::Scroll(s),
                None => ItemType::None,
            },
            FOOD_CODE => ItemType::Food,
            WEAPON_CODE => ItemType::Weapon(which),
            ARMOR_CODE => ItemType::Armor(which),
            RING_CODE => match RingType::from_raw(which) {
                Some(r) => ItemType::Ring(r),
                None => ItemType::None,
            },
            STICK_CODE => match StickType::from_raw(which) {
                Some(s) => ItemType::Stick(s),
                None => ItemType::None,
            },
            GOLD_CODE => ItemType::Gold,
            AMULET_CODE => ItemType::Amulet,
            _ => ItemType::None,
        }
    }

    /// Whether `self` is the same category as `other` (ignoring sub-type).
    ///
    /// Mirrors the legacy `o_type` comparison, where all potions share the same
    /// ASCII code regardless of which potion they are.
    pub fn same_category(self, other: ItemType) -> bool {
        self.code() == other.code()
    }

    /// Build a potion kind from its sub-type index (`None` when invalid).
    pub fn potion(which: i32) -> ItemType {
        PotionType::try_from_raw(which).map_or(ItemType::None, ItemType::Potion)
    }

    /// Build a scroll kind from its sub-type index (`None` when invalid).
    pub fn scroll(which: i32) -> ItemType {
        ScrollType::try_from_raw(which).map_or(ItemType::None, ItemType::Scroll)
    }

    /// Build a ring kind from its sub-type index (`None` when invalid).
    pub fn ring(which: i32) -> ItemType {
        RingType::from_raw(which).map_or(ItemType::None, ItemType::Ring)
    }

    /// Build a wand/staff kind from its sub-type index (`None` when invalid).
    pub fn stick(which: i32) -> ItemType {
        StickType::from_raw(which).map_or(ItemType::None, ItemType::Stick)
    }

    /// Whether dropping this kind drops the whole stack by default.
    ///
    /// Preserves the legacy `(o_type & 0x1) == 0` test, which held for food,
    /// gold and the Amulet.
    pub fn drop_whole_stack_by_default(self) -> bool {
        matches!(self, ItemType::Food | ItemType::Gold | ItemType::Amulet)
    }
}

/// A predicate used to select items by kind, replacing the legacy `int type`
/// argument to `get_item`/`inventory`/`whatis` and its magic sentinels
/// (`0` = any, `-1` = callable, `-2` = ring-or-stick).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemFilter {
    /// Match every item.
    Any,
    /// Match items of the same category as the supplied kind.
    Category(ItemType),
    /// Match anything that can be "called" (i.e. not food or the Amulet).
    Callable,
    /// Match rings and wands/staves.
    RingOrStick,
}

impl ItemFilter {
    /// Whether `ty` satisfies this filter.
    pub fn matches(self, ty: ItemType) -> bool {
        match self {
            ItemFilter::Any => true,
            ItemFilter::Category(want) => ty.same_category(want),
            ItemFilter::Callable => !matches!(ty, ItemType::Food | ItemType::Amulet),
            ItemFilter::RingOrStick => matches!(ty, ItemType::Ring(_) | ItemType::Stick(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_through_from_raw() {
        let kinds = [
            ItemType::Potion(PotionType::Healing),
            ItemType::Scroll(ScrollType::Teleport),
            ItemType::Food,
            ItemType::Weapon(3),
            ItemType::Armor(7),
            ItemType::Ring(RingType::Regeneration),
            ItemType::Stick(StickType::Fire),
            ItemType::Gold,
            ItemType::Amulet,
        ];
        for kind in kinds {
            let rebuilt = ItemType::from_raw(kind.code(), kind.which());
            assert_eq!(rebuilt, kind, "round-trip failed for {kind:?}");
        }
        assert_eq!(ItemType::from_raw(0, 0), ItemType::None);
    }

    #[test]
    fn same_category_ignores_subtype() {
        assert!(ItemType::Potion(PotionType::Healing)
            .same_category(ItemType::Potion(PotionType::Poison)));
        assert!(!ItemType::Potion(PotionType::Healing).same_category(ItemType::Scroll(ScrollType::Map)));
    }

    #[test]
    fn filters_select_expected_kinds() {
        assert!(ItemFilter::Category(ItemType::POTION).matches(ItemType::Potion(PotionType::Poison)));
        assert!(!ItemFilter::Category(ItemType::POTION).matches(ItemType::Scroll(ScrollType::Map)));
        assert!(ItemFilter::Callable.matches(ItemType::Potion(PotionType::Poison)));
        assert!(!ItemFilter::Callable.matches(ItemType::Food));
        assert!(ItemFilter::RingOrStick.matches(ItemType::Stick(StickType::Light)));
        assert!(ItemFilter::Any.matches(ItemType::Gold));
    }
}