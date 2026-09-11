use super::model::{
    BoardState,
    BuildingCount,
    PropertyImprovementLevel,
};
use crate::game::tile::data::PROPERTY_COUNT;

pub const MIN_PLAYER_COUNT: usize = 2;
pub const MAX_PLAYER_COUNT: usize = 8;

pub const BANK_STARTING_HOUSE_COUNT: BuildingCount = 32;
pub const BANK_STARTING_HOTEL_COUNT: BuildingCount = 12;

pub const MAX_HOUSE_IMPROVEMENT_LEVEL: PropertyImprovementLevel = 4;
pub const HOTEL_IMPROVEMENT_LEVEL: PropertyImprovementLevel = 5;

impl<const PLAYER_COUNT: usize> BoardState<PLAYER_COUNT> {
    pub const STARTING_STATE: Self = {
        assert!(
            PLAYER_COUNT >= MIN_PLAYER_COUNT && PLAYER_COUNT <= MAX_PLAYER_COUNT,
            "player count should be within the supported range"
        );

        Self {
            owned_tiles_by_player_id: [0; PLAYER_COUNT],
            mortgaged_tiles: 0,
            improvement_level_by_property_id: [0; PROPERTY_COUNT],
            bank_house_count: BANK_STARTING_HOUSE_COUNT,
            bank_hotel_count: BANK_STARTING_HOTEL_COUNT,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_starting_state_for_supported_player_counts() {
        let two_player_board_state = BoardState::<MIN_PLAYER_COUNT>::STARTING_STATE;
        let eight_player_board_state = BoardState::<MAX_PLAYER_COUNT>::STARTING_STATE;

        assert_eq!(two_player_board_state.owned_tiles_by_player_id, [0; MIN_PLAYER_COUNT]);
        assert_eq!(eight_player_board_state.bank_house_count, BANK_STARTING_HOUSE_COUNT);
        assert_eq!(eight_player_board_state.bank_hotel_count, BANK_STARTING_HOTEL_COUNT);
    }
}
