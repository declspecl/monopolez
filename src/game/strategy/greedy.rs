use super::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::board::model::PlayerId;
use crate::game::state::model::GameState;
use crate::game::tile::lut::PURCHASE_PRICE_BY_TILE_ID;
use crate::game::tile::model::{
    Cash,
    TileId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GreedyStrategy {
    pub cash_reserve: Cash,
}

impl PlayerStrategy for GreedyStrategy {
    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;

        game_state.cash_by_player_id[player_id as usize] >= purchase_price + self.cash_reserve
    }

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
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
        player_id: PlayerId,
    ) -> JailAction {
        let holds_get_out_of_jail_free_card = game_state.get_out_of_jail_free_card_holder_by_deck_kind.contains(&Some(player_id));

        if holds_get_out_of_jail_free_card {
            JailAction::UseGetOutOfJailFreeCard
        } else {
            JailAction::RollForDoubles
        }
    }
}
