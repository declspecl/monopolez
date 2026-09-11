use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::model::TileSetMask;

// max player count (8) < 255
pub type PlayerId = u8;

// 0 = unimproved, 1-4 = houses, 5 = hotel
pub type PropertyImprovementLevel = u8;

// max hotel supply (12) < max bank supply (32 houses) < 255
pub type BuildingCount = u8;

#[repr(align(64))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoardState<const PLAYER_COUNT: usize> {
    pub owned_tiles_by_player_id: [TileSetMask; PLAYER_COUNT],
    pub mortgaged_tiles: TileSetMask,
    pub improvement_level_by_property_id: [PropertyImprovementLevel; PROPERTY_COUNT],
    pub bank_house_count: BuildingCount,
    pub bank_hotel_count: BuildingCount,
}

const _: () = {
    assert!(size_of::<BoardState<2>>() == 64);
    assert!(size_of::<BoardState<3>>() == 64);
    assert!(size_of::<BoardState<4>>() == 64);
    assert!(size_of::<BoardState<8>>() == 128);
};
