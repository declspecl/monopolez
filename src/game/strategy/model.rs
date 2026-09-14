use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum JailAction {
    RollForDoubles,
    PayBail,
    UseGetOutOfJailFreeCard,
}

pub trait PlayerStrategy {
    fn observe_public_event(
        &mut self,
        _event: crate::game::engine::event::GameEvent,
    ) {
    }

    fn choose_liquidation_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        _required_amount: Cash,
    ) -> Option<crate::game::engine::liquidation::LiquidationAction> {
        crate::game::engine::liquidation::default_liquidation_action(state, rules, player)
    }

    fn choose_management_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        legal: &crate::game::engine::action::LegalManagementActions<'_, N>,
    ) -> Option<crate::game::engine::action::ManagementAction> {
        use crate::game::engine::action::{
            ManagementAction,
            ManagementPhase,
        };
        let action = match legal.phase() {
            ManagementPhase::Building => self.choose_property_to_improve(state, rules, player).map(ManagementAction::Build),
            ManagementPhase::Unmortgaging => self.choose_tile_to_unmortgage(state, rules, player).map(ManagementAction::Unmortgage),
        };
        action.filter(|action| legal.contains(*action))
    }

    fn record_event(
        &mut self,
        _event: crate::game::engine::event::GameEvent,
    ) {
    }

    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool;

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash;

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> JailAction;

    fn choose_property_to_improve<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<PropertyId>;

    fn choose_tile_to_unmortgage<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TileId>;

    fn propose_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TradeOffer>;

    fn should_accept_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
        trade_offer: &TradeOffer,
    ) -> bool;
}
