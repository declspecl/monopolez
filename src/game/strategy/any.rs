use super::cautious::CautiousStrategy;
use super::greedy::GreedyStrategy;
use super::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

macro_rules! delegate_to_strategy {
    ($strategies:ident, $method:ident, $($argument:expr),*) => {
        match $strategies {
            Self::Greedy(strategy) => strategy.$method($($argument),*),
            Self::Cautious(strategy) => strategy.$method($($argument),*),
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyStrategy {
    Greedy(GreedyStrategy),
    Cautious(CautiousStrategy),
}

impl PlayerStrategy for AnyStrategy {
    fn choose_liquidation_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        required_amount: Cash,
    ) -> Option<crate::game::engine::liquidation::LiquidationAction> {
        delegate_to_strategy!(self, choose_liquidation_action, state, rules, player, required_amount)
    }

    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool {
        delegate_to_strategy!(self, should_purchase_property, game_state, ruleset, player_id, tile_id)
    }

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash {
        delegate_to_strategy!(self, choose_max_auction_bid, game_state, ruleset, player_id, tile_id)
    }

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> JailAction {
        delegate_to_strategy!(self, choose_jail_action, game_state, ruleset, player_id)
    }

    fn choose_property_to_improve<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<PropertyId> {
        delegate_to_strategy!(self, choose_property_to_improve, game_state, ruleset, player_id)
    }

    fn choose_tile_to_unmortgage<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TileId> {
        delegate_to_strategy!(self, choose_tile_to_unmortgage, game_state, ruleset, player_id)
    }

    fn propose_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TradeOffer> {
        delegate_to_strategy!(self, propose_trade, game_state, ruleset, player_id)
    }

    fn should_accept_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        trade_offer: &TradeOffer,
    ) -> bool {
        delegate_to_strategy!(self, should_accept_trade, game_state, ruleset, player_id, trade_offer)
    }
}
