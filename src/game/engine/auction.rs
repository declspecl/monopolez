use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::{
    AuctionEligibility,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::model::{
    Cash,
    TileId,
};

pub fn run_auction<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    declining_player_id: PlayerId,
    tile_id: TileId,
) {
    let mut winning_player_id = None;
    let mut winning_bid: Cash = 0;
    let mut runner_up_bid: Cash = 0;

    for turn_order_offset in 0..PLAYER_COUNT {
        let bidder_player_id = ((declining_player_id as usize + turn_order_offset) % PLAYER_COUNT) as PlayerId;
        let bidder_index = bidder_player_id as usize;

        let is_bankrupt = game_state.bankrupt_players & (1 << bidder_player_id) != 0;
        let is_excluded = bidder_player_id == declining_player_id && ruleset.auction_eligibility == AuctionEligibility::ExcludingDecliningPlayer;
        if is_bankrupt || is_excluded {
            continue;
        }

        let max_bid = strategies[bidder_index]
            .choose_max_auction_bid(game_state, bidder_player_id, tile_id)
            .min(game_state.cash_by_player_id[bidder_index]);

        if max_bid > winning_bid {
            runner_up_bid = winning_bid;
            winning_bid = max_bid;
            winning_player_id = Some(bidder_player_id);
        } else if max_bid > runner_up_bid {
            runner_up_bid = max_bid;
        }
    }

    if let Some(winning_player_id) = winning_player_id {
        let winning_index = winning_player_id as usize;
        let sale_price = (runner_up_bid + 1).min(winning_bid);

        game_state.cash_by_player_id[winning_index] -= sale_price;
        game_state.board.owned_tiles_by_player_id[winning_index] |= 1 << tile_id;
    }
}
