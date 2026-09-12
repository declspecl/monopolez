use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::model::{
    TileId,
    TileSetMask,
};

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

impl<const PLAYER_COUNT: usize> BoardState<PLAYER_COUNT> {
    pub fn get_tile_owner(
        &self,
        tile_id: TileId,
    ) -> Option<PlayerId> {
        let tile_bit = 1 << tile_id;

        self.owned_tiles_by_player_id
            .iter()
            .position(|owned_tiles| owned_tiles & tile_bit != 0)
            .map(|player_id| player_id as PlayerId)
    }

    pub fn get_player_owned_tiles_count(
        &self,
        player_id: PlayerId,
        tiles: TileSetMask,
    ) -> u32 {
        (self.owned_tiles_by_player_id[player_id as usize] & tiles).count_ones()
    }

    pub fn does_player_own_all_tiles(
        &self,
        player_id: PlayerId,
        tiles: TileSetMask,
    ) -> bool {
        self.owned_tiles_by_player_id[player_id as usize] & tiles == tiles
    }

    pub fn is_tile_mortgaged(
        &self,
        tile_id: TileId,
    ) -> bool {
        self.mortgaged_tiles & (1 << tile_id) != 0
    }
}

const _: () = {
    assert!(size_of::<BoardState<2>>() == 64);
    assert!(size_of::<BoardState<3>>() == 64);
    assert!(size_of::<BoardState<4>>() == 64);
    assert!(size_of::<BoardState<8>>() == 128);
};
