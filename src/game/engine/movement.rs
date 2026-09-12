use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::{
    GO_TILE_ID,
    JAIL_TILE_ID,
    TILE_COUNT,
};
use crate::game::tile::model::{
    Cash,
    TileId,
};

pub fn move_player_forward<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    player_id: PlayerId,
    tile_count: u8,
) {
    let current_tile_id = game_state.position_by_player_id[player_id as usize];
    let target_tile_id = ((current_tile_id as usize + tile_count as usize) % TILE_COUNT) as TileId;

    advance_player_to_tile(game_state, ruleset, player_id, target_tile_id);
}

pub fn advance_player_to_tile<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    player_id: PlayerId,
    target_tile_id: TileId,
) {
    let player_index = player_id as usize;
    let current_tile_id = game_state.position_by_player_id[player_index];

    if target_tile_id == GO_TILE_ID {
        game_state.cash_by_player_id[player_index] += ruleset.go_landing_salary as Cash;
    } else if target_tile_id < current_tile_id {
        game_state.cash_by_player_id[player_index] += ruleset.go_passing_salary as Cash;
    }

    game_state.position_by_player_id[player_index] = target_tile_id;
}

pub fn move_player_backward<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    tile_count: u8,
) {
    let player_index = player_id as usize;
    let current_tile_id = game_state.position_by_player_id[player_index] as usize;

    game_state.position_by_player_id[player_index] = ((current_tile_id + TILE_COUNT - tile_count as usize) % TILE_COUNT) as TileId;
}

pub fn send_player_to_jail<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
) {
    let player_index = player_id as usize;

    game_state.position_by_player_id[player_index] = JAIL_TILE_ID;
    game_state.jail_turn_count_by_player_id[player_index] = 0;
    game_state.jailed_players |= 1 << player_id;
    game_state.consecutive_double_count = 0;
}

pub fn release_player_from_jail<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
) {
    game_state.jail_turn_count_by_player_id[player_id as usize] = 0;
    game_state.jailed_players &= !(1 << player_id);
}
