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
    if declining_player_id as usize >= PLAYER_COUNT
        || tile_id as usize >= crate::game::tile::data::TILE_COUNT
        || crate::game::tile::lut::OWNABLE_TILE_SET_MASK & (1 << tile_id) == 0
        || game_state.board.get_tile_owner(tile_id).is_some()
    {
        return;
    }
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
            .choose_max_auction_bid(game_state, ruleset, bidder_player_id, tile_id)
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
        let sale_price = auction_sale_price(winning_bid, runner_up_bid);

        game_state.cash_by_player_id[winning_index] -= sale_price;
        game_state.board.owned_tiles_by_player_id[winning_index] |= 1 << tile_id;
        super::event::publish_event(
            strategies,
            winning_index,
            super::event::GameEvent::PropertyPurchased {
                player_id: winning_player_id,
                tile_id,
                price: sale_price,
                auction: true,
            },
        );
    }
}

fn auction_sale_price(
    winning_bid: Cash,
    runner_up_bid: Cash,
) -> Cash {
    runner_up_bid.saturating_add(1).min(winning_bid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::strategy::configurable::ConfigurableStrategy;

    #[test]
    fn invalid_auction_targets_leave_state_unchanged() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[1] = 1 << 1;
        let original = state;
        let mut strategies = [ConfigurableStrategy::new(); 2];
        for (decliner, tile) in [(255, 3), (0, 255), (0, 0), (0, 1)] {
            run_auction(&mut state, &rules, &mut strategies, decliner, tile);
            assert_eq!(state, original);
        }
    }

    #[test]
    fn excluded_and_bankrupt_players_cannot_win() {
        let rules = Ruleset::builder().with_auction_eligibility(AuctionEligibility::ExcludingDecliningPlayer).build();
        let mut state = GameState::<3>::create_starting_state(&rules, 0);
        state.bankrupt_players = 1 << 1;
        let mut strategies = [ConfigurableStrategy::new(); 3];
        run_auction(&mut state, &rules, &mut strategies, 0, 1);
        assert_eq!(state.board.get_tile_owner(1), Some(2));
        assert_eq!(state.cash_by_player_id, [1500, 1500, 1499]);
    }

    #[test]
    fn auction_preserves_tie_order_and_charges_runner_up_price() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        let mut strategies = [ConfigurableStrategy::new(); 2];
        run_auction(&mut state, &rules, &mut strategies, 1, 1);
        assert_eq!(state.board.get_tile_owner(1), Some(1));
        assert_eq!(state.cash_by_player_id, [1500, 1440]);
        assert_eq!(auction_sale_price(100, 60), 61);
        assert_eq!(auction_sale_price(Cash::MAX, Cash::MAX), Cash::MAX);
    }
}
