use glam::IVec2;
use rogue_rust::dungeon::DUNGEON;
use rogue_rust::entity::chase::roomin;
use rogue_rust::ui::{huh_string, set_huh_string, set_mpos};
use rogue_rust::game::with_current_level_mut;
use rogue_rust::level::Level;
use rogue_rust::structure::Room;
use rogue_rust::tile::Tile;

#[test]
fn passages_are_valid_locations_without_a_room() {
    with_current_level_mut(|level| {
        *level = Level::new();
        level.rooms = vec![Room::new(IVec2::new(2, 2), IVec2::new(5, 5))];
        assert!(level.map.set(IVec2::new(18, 14), Tile::Passage));
    });
    set_huh_string("previous message");
    set_mpos(0);

    unsafe {
        assert_eq!(roomin(IVec2::new(3, 3), &DUNGEON), Some(0));
        assert_eq!(huh_string(), "previous message");

        for _ in 0..3 {
            assert_eq!(roomin(IVec2::new(18, 14), &DUNGEON), None);
            assert_eq!(huh_string(), "previous message");
        }

        assert_eq!(roomin(IVec2::new(-1, -1), &DUNGEON), None);
        assert_eq!(huh_string(), "in some bizarre place (-1, -1)");
    }
}