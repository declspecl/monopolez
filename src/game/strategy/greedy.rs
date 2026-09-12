use super::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::board::model::{
    PlayerId,
    PropertyImprovementLevel,
};
use crate::game::engine::improvement::can_improve_property;
use crate::game::engine::trade::calculate_tile_set_purchase_value;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    HOUSE_PURCHASE_PRICE_BY_TILE_ID,
    PURCHASE_PRICE_BY_TILE_ID,
    TILE_ID_BY_PROPERTY_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
    UNMORTGAGE_PRICE_BY_TILE_ID,
};
use crate::game::tile::model::{
    Cash,
    OwnershipGroup,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GreedyStrategy {
    pub cash_reserve: Cash,
}

impl PlayerStrategy for GreedyStrategy {
    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;

        game_state.cash_by_player_id[player_id as usize] >= purchase_price + self.cash_reserve
    }

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
        let spendable_cash = game_state.cash_by_player_id[player_id as usize].saturating_sub(self.cash_reserve);

        purchase_price.min(spendable_cash)
    }

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> JailAction {
        let holds_get_out_of_jail_free_card = game_state.get_out_of_jail_free_card_holder_by_deck_kind.contains(&Some(player_id));

        if holds_get_out_of_jail_free_card {
            JailAction::UseGetOutOfJailFreeCard
        } else {
            JailAction::RollForDoubles
        }
    }

    fn choose_property_to_improve<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<PropertyId> {
        let mut least_improved_property: Option<(PropertyImprovementLevel, PropertyId)> = None;

        for property_id in 0..PROPERTY_COUNT {
            let tile_id = TILE_ID_BY_PROPERTY_ID[property_id];
            let house_purchase_price = HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] < house_purchase_price + self.cash_reserve {
                continue;
            }

            if !can_improve_property(game_state, ruleset, player_id, property_id as PropertyId) {
                continue;
            }

            let improvement_level = game_state.board.improvement_level_by_property_id[property_id];
            if least_improved_property.is_none_or(|(lowest_improvement_level, _)| improvement_level < lowest_improvement_level) {
                least_improved_property = Some((improvement_level, property_id as PropertyId));
            }
        }

        least_improved_property.map(|(_, property_id)| property_id)
    }

    fn choose_tile_to_unmortgage<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TileId> {
        let mut mortgaged_owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize] & game_state.board.mortgaged_tiles;
        let mut cheapest_unmortgage: Option<(Cash, TileId)> = None;

        while mortgaged_owned_tiles != 0 {
            let tile_id = mortgaged_owned_tiles.trailing_zeros() as TileId;
            mortgaged_owned_tiles &= mortgaged_owned_tiles - 1;

            let unmortgage_price = UNMORTGAGE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] < unmortgage_price + self.cash_reserve {
                continue;
            }

            if cheapest_unmortgage.is_none_or(|(cheapest_unmortgage_price, _)| unmortgage_price < cheapest_unmortgage_price) {
                cheapest_unmortgage = Some((unmortgage_price, tile_id));
            }
        }

        cheapest_unmortgage.map(|(_, tile_id)| tile_id)
    }

    fn propose_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TradeOffer> {
        let owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize];

        for ownership_group_index in 0..OwnershipGroup::COUNT {
            let group_tiles = TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group_index];
            let owned_group_tiles = owned_tiles & group_tiles;
            let missing_group_tiles = group_tiles & !owned_group_tiles;
            if owned_group_tiles == 0 || missing_group_tiles.count_ones() != 1 {
                continue;
            }

            let missing_tile_id = missing_group_tiles.trailing_zeros() as TileId;
            let Some(owner_player_id) = game_state.board.get_tile_owner(missing_tile_id) else {
                continue;
            };

            let offered_cash = PURCHASE_PRICE_BY_TILE_ID[missing_tile_id as usize] as Cash * 3 / 2;
            if game_state.cash_by_player_id[player_id as usize] < offered_cash + self.cash_reserve {
                continue;
            }

            return Some(TradeOffer {
                proposer_player_id: player_id,
                recipient_player_id: owner_player_id,
                offered_cash,
                offered_tiles: 0,
                offered_get_out_of_jail_free_cards: 0,
                requested_cash: 0,
                requested_tiles: 1 << missing_tile_id,
                requested_get_out_of_jail_free_cards: 0,
            });
        }

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

        received_value > given_value
    }
}
