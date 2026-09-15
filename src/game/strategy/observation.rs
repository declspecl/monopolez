use crate::game::board::model::PlayerId;
use crate::game::engine::action::{
    LegalManagementActions,
    ManagementAction,
    ManagementPhase,
    legal_jail_actions,
};
use crate::game::engine::event::GameEvent;
use crate::game::engine::liquidation::{
    LiquidationAction,
    legal_liquidation_actions,
};
use crate::game::ruleset::model::{
    Ruleset,
    TurnCount,
};
use crate::game::state::model::{
    GameState,
    PlayerSetMask,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
    TileSetMask,
};
use crate::game::trade::model::TradeOffer;

use super::model::{
    JailAction,
    PlayerStrategy,
};

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PublicObservation<'a> {
    pub rules: &'a Ruleset,
    pub player_id: PlayerId,
    pub owned_tiles_by_player_id: &'a [TileSetMask],
    pub mortgaged_tiles: TileSetMask,
    pub improvement_level_by_property_id: &'a [u8],
    pub bank_house_count: u8,
    pub bank_hotel_count: u8,
    pub cash_by_player_id: &'a [Cash],
    pub position_by_player_id: &'a [TileId],
    pub jail_turn_count_by_player_id: &'a [TurnCount],
    pub jailed_players: PlayerSetMask,
    pub bankrupt_players: PlayerSetMask,
    pub get_out_of_jail_free_card_holder_by_deck_kind: &'a [Option<PlayerId>],
    pub free_parking_jackpot: Cash,
    pub current_player_id: PlayerId,
    pub consecutive_double_count: u8,
    pub history: &'a [GameEvent],
}

impl<'a> PublicObservation<'a> {
    pub(super) fn new<const N: usize>(
        state: &'a GameState<N>,
        rules: &'a Ruleset,
        player_id: PlayerId,
        history: &'a [GameEvent],
    ) -> Self {
        Self {
            rules,
            player_id,
            owned_tiles_by_player_id: &state.board.owned_tiles_by_player_id,
            mortgaged_tiles: state.board.mortgaged_tiles,
            improvement_level_by_property_id: &state.board.improvement_level_by_property_id,
            bank_house_count: state.board.bank_house_count,
            bank_hotel_count: state.board.bank_hotel_count,
            cash_by_player_id: &state.cash_by_player_id,
            position_by_player_id: &state.position_by_player_id,
            jail_turn_count_by_player_id: &state.jail_turn_count_by_player_id,
            jailed_players: state.jailed_players,
            bankrupt_players: state.bankrupt_players,
            get_out_of_jail_free_card_holder_by_deck_kind: &state.get_out_of_jail_free_card_holder_by_deck_kind,
            free_parking_jackpot: state.free_parking_jackpot,
            current_player_id: state.current_player_id,
            consecutive_double_count: state.consecutive_double_count,
            history,
        }
    }
}

pub trait ObservedPolicy {
    fn should_purchase_property(
        &mut self,
        _observation: PublicObservation<'_>,
        _tile: TileId,
    ) -> bool {
        false
    }
    fn choose_max_auction_bid(
        &mut self,
        _observation: PublicObservation<'_>,
        _tile: TileId,
    ) -> Cash {
        0
    }
    fn propose_trade(
        &mut self,
        _observation: PublicObservation<'_>,
    ) -> Option<TradeOffer> {
        None
    }
    fn should_accept_trade(
        &mut self,
        _observation: PublicObservation<'_>,
        _offer: &TradeOffer,
    ) -> bool {
        false
    }
    fn counter_trade_offer(
        &mut self,
        _observation: PublicObservation<'_>,
        _rejected_offer: &TradeOffer,
    ) -> Option<TradeOffer> {
        None
    }
    fn should_accept_counteroffer(
        &mut self,
        observation: PublicObservation<'_>,
        _original_offer: &TradeOffer,
        counteroffer: &TradeOffer,
    ) -> bool {
        self.should_accept_trade(observation, counteroffer)
    }

    fn choose_jail_action(
        &mut self,
        _observation: PublicObservation<'_>,
        _legal: &[JailAction],
    ) -> JailAction {
        JailAction::RollForDoubles
    }
    fn choose_management_action(
        &mut self,
        _observation: PublicObservation<'_>,
        _phase: ManagementPhase,
        _legal: &[ManagementAction],
    ) -> Option<ManagementAction> {
        None
    }
    fn choose_liquidation_action(
        &mut self,
        _observation: PublicObservation<'_>,
        _required_amount: Cash,
        legal: &[LiquidationAction],
    ) -> Option<LiquidationAction> {
        legal.first().copied()
    }
}

#[derive(Debug, Clone)]
pub struct ObservedStrategy<P> {
    pub policy: P,
    history: Vec<GameEvent>,
}

impl<P> ObservedStrategy<P> {
    pub fn new(policy: P) -> Self {
        Self { policy, history: Vec::new() }
    }

    pub fn history(&self) -> &[GameEvent] {
        &self.history
    }
}

impl<P: ObservedPolicy> PlayerStrategy for ObservedStrategy<P> {
    fn observe_public_event(
        &mut self,
        event: GameEvent,
    ) {
        self.history.push(event);
    }
    fn should_purchase_property<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        tile: TileId,
    ) -> bool {
        self.policy.should_purchase_property(PublicObservation::new(state, rules, player, &self.history), tile)
    }
    fn choose_max_auction_bid<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        tile: TileId,
    ) -> Cash {
        self.policy.choose_max_auction_bid(PublicObservation::new(state, rules, player, &self.history), tile)
    }
    fn propose_trade<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
    ) -> Option<TradeOffer> {
        self.policy.propose_trade(PublicObservation::new(state, rules, player, &self.history))
    }
    fn should_accept_trade<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        offer: &TradeOffer,
    ) -> bool {
        self.policy.should_accept_trade(PublicObservation::new(state, rules, player, &self.history), offer)
    }
    fn counter_trade_offer<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        rejected_offer: &TradeOffer,
    ) -> Option<TradeOffer> {
        self.policy.counter_trade_offer(PublicObservation::new(state, rules, player, &self.history), rejected_offer)
    }
    fn should_accept_counteroffer<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        original_offer: &TradeOffer,
        counteroffer: &TradeOffer,
    ) -> bool {
        self.policy
            .should_accept_counteroffer(PublicObservation::new(state, rules, player, &self.history), original_offer, counteroffer)
    }

    fn choose_jail_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
    ) -> JailAction {
        let legal: Vec<_> = legal_jail_actions(state, rules, player).collect();
        self.policy.choose_jail_action(PublicObservation::new(state, rules, player, &self.history), &legal)
    }

    fn choose_management_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        legal: &LegalManagementActions<'_, N>,
    ) -> Option<ManagementAction> {
        let actions: Vec<_> = legal.iter().collect();
        self.policy
            .choose_management_action(PublicObservation::new(state, rules, player, &self.history), legal.phase(), &actions)
    }

    fn choose_liquidation_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        required_amount: Cash,
    ) -> Option<LiquidationAction> {
        let legal: Vec<_> = legal_liquidation_actions(state, rules, player).collect();
        self.policy
            .choose_liquidation_action(PublicObservation::new(state, rules, player, &self.history), required_amount, &legal)
    }

    fn choose_property_to_improve<const N: usize>(
        &mut self,
        _state: &GameState<N>,
        _rules: &Ruleset,
        _player: PlayerId,
    ) -> Option<PropertyId> {
        None
    }

    fn choose_tile_to_unmortgage<const N: usize>(
        &mut self,
        _state: &GameState<N>,
        _rules: &Ruleset,
        _player: PlayerId,
    ) -> Option<TileId> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::engine::action::legal_management_actions;
    use crate::game::engine::turn::{
        TurnPhase,
        apply_management_decision,
        play_game,
    };
    use crate::game::ruleset::data::DEX_RULESET;

    #[derive(Debug, Clone, Default)]
    struct Buyer {
        decisions: usize,
    }

    impl ObservedPolicy for Buyer {
        fn should_purchase_property(
            &mut self,
            observation: PublicObservation<'_>,
            tile: TileId,
        ) -> bool {
            self.decisions += 1;
            assert!(!observation.history.is_empty());
            observation.cash_by_player_id[observation.player_id as usize] >= crate::game::tile::lut::PURCHASE_PRICE_BY_TILE_ID[tile as usize] as Cash
        }
    }

    fn observation_json<const N: usize>(
        state: &GameState<N>,
        history: &[GameEvent],
    ) -> serde_json::Value {
        serde_json::to_value(PublicObservation::new(state, &DEX_RULESET, 0, history)).unwrap()
    }

    #[test]
    fn observations_exclude_hidden_state_and_preserve_public_history() {
        let state = GameState::<4>::create_starting_state(&DEX_RULESET, 7);
        let mut other = state;
        other.rng = crate::game::rng::WyRand::new(123);
        for deck in &mut other.deck_state_by_deck_kind {
            deck.card_id_by_draw_position.reverse();
            deck.next_draw_position = 9;
        }
        assert_ne!(state, other);
        let history = [GameEvent::TurnStarted { player_id: 0 }];
        let value = observation_json(&state, &history);
        assert_eq!(value, observation_json(&other, &history));
        for field in ["rng", "seed", "deck_state_by_deck_kind", "next_draw_position", "card_id_by_draw_position"] {
            assert!(!value.to_string().contains(field));
        }
        assert_eq!(value["history"], serde_json::to_value(history).unwrap());
        other.cash_by_player_id[0] += 1;
        assert_ne!(value, observation_json(&other, &history));
    }

    fn verify_game<const N: usize>() {
        let mut state = GameState::<N>::create_starting_state(&DEX_RULESET, 7);
        let mut strategies = core::array::from_fn(|_| ObservedStrategy::new(Buyer::default()));
        play_game(&mut state, &DEX_RULESET, &mut strategies, 1000);
        assert!(strategies.iter().any(|strategy| strategy.policy.decisions > 0));
        assert!(strategies[0].history().iter().any(|event| matches!(event, GameEvent::CardDrawn { .. })));
        for strategy in &strategies {
            assert_eq!(strategy.history(), strategies[0].history());
        }
        let mut replay = GameState::<N>::create_starting_state(&DEX_RULESET, 7);
        let mut replay_strategies = core::array::from_fn(|_| ObservedStrategy::new(Buyer::default()));
        play_game(&mut replay, &DEX_RULESET, &mut replay_strategies, 1000);
        assert_eq!(state, replay);
        assert_eq!(strategies[0].history(), replay_strategies[0].history());
    }

    #[test]
    fn observed_policies_play_reproducible_games_with_shared_public_history() {
        verify_game::<2>();
        verify_game::<4>();
        verify_game::<8>();
    }

    #[test]
    fn cloned_strategies_have_independent_history_and_policy_state() {
        let mut strategy = ObservedStrategy::new(Buyer::default());
        strategy.observe_public_event(GameEvent::TurnStarted { player_id: 0 });
        let mut fork = strategy.clone();
        fork.observe_public_event(GameEvent::TurnStarted { player_id: 1 });
        fork.policy.decisions += 1;
        assert_eq!(strategy.history().len(), 1);
        assert_eq!(fork.history().len(), 2);
        assert_eq!(strategy.policy.decisions, 0);
    }

    struct InvalidBuilder;

    impl ObservedPolicy for InvalidBuilder {
        fn choose_management_action(
            &mut self,
            observation: PublicObservation<'_>,
            phase: ManagementPhase,
            legal: &[ManagementAction],
        ) -> Option<ManagementAction> {
            assert_eq!(observation.player_id, 0);
            assert_eq!(phase, ManagementPhase::Building);
            assert_eq!(legal, &[ManagementAction::Build(0), ManagementAction::Build(1)]);
            Some(ManagementAction::Build(u8::MAX))
        }
    }

    #[test]
    fn engine_rejects_illegal_observed_policy_decisions() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let mut strategies = core::array::from_fn(|_| ObservedStrategy::new(InvalidBuilder));
        let legal = legal_management_actions(&state, &DEX_RULESET, 0, ManagementPhase::Building);
        let action = strategies[0].choose_management_action(&state, &DEX_RULESET, 0, &legal);
        let before = state;
        assert!(apply_management_decision(&mut state, &DEX_RULESET, &mut strategies, TurnPhase::Building, action).is_err());
        assert_eq!(state, before);
        assert!(strategies.iter().all(|strategy| strategy.history().is_empty()));
    }
}
