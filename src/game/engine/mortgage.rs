use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::lut::{
    MORTGAGE_VALUE_BY_TILE_ID,
    OWNERSHIP_GROUP_BY_TILE_ID,
    PROPERTY_ID_BY_TILE_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
    UNMORTGAGE_PRICE_BY_TILE_ID,
};
use crate::game::tile::model::{
    Cash,
    TileId,
    TileSetMask,
};

pub fn run_mortgage_phase<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
) {
    while let Some(tile_id) = strategies[player_id as usize].choose_tile_to_unmortgage(game_state, ruleset, player_id) {
        if !unmortgage_tile(game_state, player_id, tile_id) {
            return;
        }
        strategies[player_id as usize].record_event(super::event::GameEvent::TileUnmortgaged { player_id, tile_id });
    }
}

pub fn can_mortgage_tile<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    tile_id: TileId,
) -> bool {
    if player_id as usize >= PLAYER_COUNT || tile_id as usize >= crate::game::tile::data::TILE_COUNT || game_state.bankrupt_players & (1 << player_id) != 0 {
        return false;
    }
    if game_state.board.get_tile_owner(tile_id) != Some(player_id) || game_state.board.is_tile_mortgaged(tile_id) {
        return false;
    }

    !has_group_improvements(game_state, tile_id)
        && game_state.cash_by_player_id[player_id as usize]
            .checked_add(MORTGAGE_VALUE_BY_TILE_ID[tile_id as usize] as Cash)
            .is_some()
}

pub fn mortgage_tile<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    tile_id: TileId,
) -> bool {
    if !can_mortgage_tile(game_state, player_id, tile_id) {
        return false;
    }

    game_state.board.mortgaged_tiles |= 1 << tile_id;
    game_state.cash_by_player_id[player_id as usize] += MORTGAGE_VALUE_BY_TILE_ID[tile_id as usize] as Cash;

    true
}

pub fn can_unmortgage_tile<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    tile_id: TileId,
) -> bool {
    if player_id as usize >= PLAYER_COUNT || tile_id as usize >= crate::game::tile::data::TILE_COUNT || game_state.bankrupt_players & (1 << player_id) != 0 {
        return false;
    }
    if game_state.board.get_tile_owner(tile_id) != Some(player_id) || !game_state.board.is_tile_mortgaged(tile_id) {
        return false;
    }

    game_state.cash_by_player_id[player_id as usize] >= UNMORTGAGE_PRICE_BY_TILE_ID[tile_id as usize] as Cash
}

pub fn unmortgage_tile<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    tile_id: TileId,
) -> bool {
    if !can_unmortgage_tile(game_state, player_id, tile_id) {
        return false;
    }

    game_state.board.mortgaged_tiles &= !(1 << tile_id);
    game_state.cash_by_player_id[player_id as usize] -= UNMORTGAGE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;

    true
}

pub fn calculate_mortgage_transfer_interest<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    transferred_tiles: TileSetMask,
) -> Cash {
    let mut mortgaged_transferred_tiles = transferred_tiles & game_state.board.mortgaged_tiles;
    let mut transfer_interest = 0;

    while mortgaged_transferred_tiles != 0 {
        let tile_index = mortgaged_transferred_tiles.trailing_zeros() as usize;
        mortgaged_transferred_tiles &= mortgaged_transferred_tiles - 1;

        transfer_interest += (UNMORTGAGE_PRICE_BY_TILE_ID[tile_index] - MORTGAGE_VALUE_BY_TILE_ID[tile_index]) as Cash;
    }

    transfer_interest
}

pub fn has_group_improvements<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    tile_id: TileId,
) -> bool {
    let Some(ownership_group) = OWNERSHIP_GROUP_BY_TILE_ID[tile_id as usize] else {
        return false;
    };

    let mut group_tiles: TileSetMask = TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group as usize];
    while group_tiles != 0 {
        let tile_index = group_tiles.trailing_zeros() as usize;
        group_tiles &= group_tiles - 1;

        if let Some(property_id) = PROPERTY_ID_BY_TILE_ID[tile_index]
            && game_state.board.improvement_level_by_property_id[property_id as usize] > 0
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARK_PLACE_TILE_ID: TileId = 37;
    const BOARDWALK_TILE_ID: TileId = 39;

    #[test]
    fn invalid_mortgage_actions_leave_state_unchanged() {
        let mut state = create_dark_blue_owner_state(&Ruleset::default());
        let original = state;
        for (player, tile) in [(255, 39), (0, 255), (1, 39), (0, 0)] {
            assert!(!mortgage_tile(&mut state, player, tile));
            assert!(!unmortgage_tile(&mut state, player, tile));
            assert_eq!(state, original);
        }
        state.cash_by_player_id[0] = Cash::MAX;
        let original = state;
        assert!(!mortgage_tile(&mut state, 0, 39));
        assert_eq!(state, original);
    }

    fn create_dark_blue_owner_state(ruleset: &Ruleset) -> GameState<2> {
        let mut game_state = GameState::<2>::create_starting_state(ruleset, 1);
        game_state.board.owned_tiles_by_player_id[0] = 1 << PARK_PLACE_TILE_ID | 1 << BOARDWALK_TILE_ID;

        game_state
    }

    #[test]
    fn mortgages_and_unmortgages_for_the_listed_prices() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);

        assert!(mortgage_tile(&mut game_state, 0, BOARDWALK_TILE_ID));
        assert_eq!(game_state.cash_by_player_id[0], 1700);
        assert!(game_state.board.is_tile_mortgaged(BOARDWALK_TILE_ID));

        assert!(unmortgage_tile(&mut game_state, 0, BOARDWALK_TILE_ID));
        assert_eq!(game_state.cash_by_player_id[0], 1480);
        assert!(!game_state.board.is_tile_mortgaged(BOARDWALK_TILE_ID));
    }

    #[test]
    fn refuses_to_mortgage_improved_ownership_group() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);
        game_state.board.improvement_level_by_property_id[20] = 1;

        assert!(!can_mortgage_tile(&game_state, 0, BOARDWALK_TILE_ID));
    }

    #[test]
    fn charges_interest_on_transferred_mortgages() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);
        mortgage_tile(&mut game_state, 0, BOARDWALK_TILE_ID);

        let transfer_interest = calculate_mortgage_transfer_interest(&game_state, game_state.board.owned_tiles_by_player_id[0]);

        assert_eq!(transfer_interest, 20);
    }
}
