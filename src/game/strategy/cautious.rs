use super::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::improvement::can_improve_property;
use crate::game::engine::trade::calculate_tile_set_purchase_value;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    HOUSE_PURCHASE_PRICE_BY_TILE_ID,
    PURCHASE_PRICE_BY_TILE_ID,
    TILE_ID_BY_PROPERTY_ID,
    UNMORTGAGE_PRICE_BY_TILE_ID,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

const PURCHASE_CASH_MULTIPLIER: Cash = 2;
const ACCEPTED_TRADE_VALUE_MULTIPLIER: Cash = 5;
const ACCEPTED_TRADE_VALUE_DIVISOR: Cash = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CautiousStrategy {
    pub cash_reserve: Cash,
}

impl PlayerStrategy for CautiousStrategy {
    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;

        game_state.cash_by_player_id[player_id as usize] >= purchase_price * PURCHASE_CASH_MULTIPLIER + self.cash_reserve
    }

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash {
        let half_purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash / 2;
        let spendable_cash = game_state.cash_by_player_id[player_id as usize].saturating_sub(self.cash_reserve);

        half_purchase_price.min(spendable_cash)
    }

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> JailAction {
        if game_state.get_out_of_jail_free_card_holder_by_deck_kind.contains(&Some(player_id)) {
            return JailAction::UseGetOutOfJailFreeCard;
        }

        if game_state.cash_by_player_id[player_id as usize] >= ruleset.jail_bail_amount as Cash + self.cash_reserve {
            return JailAction::PayBail;
        }

        JailAction::RollForDoubles
    }

    fn choose_property_to_improve<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<PropertyId> {
        for property_id in 0..PROPERTY_COUNT {
            let tile_id = TILE_ID_BY_PROPERTY_ID[property_id];
            let house_purchase_price = HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] < house_purchase_price * PURCHASE_CASH_MULTIPLIER + self.cash_reserve {
                continue;
            }

            if can_improve_property(game_state, ruleset, player_id, property_id as PropertyId) {
                return Some(property_id as PropertyId);
            }
        }

        None
    }

    fn choose_tile_to_unmortgage<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TileId> {
        let mut mortgaged_owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize] & game_state.board.mortgaged_tiles;

        while mortgaged_owned_tiles != 0 {
            let tile_id = mortgaged_owned_tiles.trailing_zeros() as TileId;
            mortgaged_owned_tiles &= mortgaged_owned_tiles - 1;

            let unmortgage_price = UNMORTGAGE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] >= unmortgage_price * PURCHASE_CASH_MULTIPLIER + self.cash_reserve {
                return Some(tile_id);
            }
        }

        None
    }

    fn propose_trade<const PLAYER_COUNT: usize>(
        &mut self,
        _game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        _player_id: PlayerId,
    ) -> Option<TradeOffer> {
        None
    }

    fn should_accept_trade<const PLAYER_COUNT: usize>(
        &mut self,
        _game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        _player_id: PlayerId,
        trade_offer: &TradeOffer,
    ) -> bool {
        let received_value = trade_offer.offered_cash + calculate_tile_set_purchase_value(trade_offer.offered_tiles);
        let given_value = trade_offer.requested_cash + calculate_tile_set_purchase_value(trade_offer.requested_tiles);

        received_value * ACCEPTED_TRADE_VALUE_DIVISOR > given_value * ACCEPTED_TRADE_VALUE_MULTIPLIER
    }
}
