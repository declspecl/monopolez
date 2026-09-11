use super::model::{
    BoardState,
    BuildingCount,
    PropertyImprovementLevel,
};
use crate::game::tile::data::TILE_COUNT;

pub const MIN_PLAYER_COUNT: usize = 2;
pub const MAX_PLAYER_COUNT: usize = 8;

pub const BANK_STARTING_HOUSE_COUNT: BuildingCount = 32;
pub const BANK_STARTING_HOTEL_COUNT: BuildingCount = 12;

pub const MAX_HOUSE_IMPROVEMENT_LEVEL: PropertyImprovementLevel = 4;
pub const HOTEL_IMPROVEMENT_LEVEL: PropertyImprovementLevel = 5;

pub const STARTING_BOARD_STATE: BoardState = BoardState {
    owned_tiles_by_player_id: [0; MAX_PLAYER_COUNT],
    mortgaged_tiles: 0,
    improvement_level_by_tile_id: [0; TILE_COUNT],
    bank_house_count: BANK_STARTING_HOUSE_COUNT,
    bank_hotel_count: BANK_STARTING_HOTEL_COUNT,
};
