use super::configurable::ConfigurableStrategy;
use super::model::{
    JailAction,
    PlayerStrategy,
};
use super::observation::PublicObservation;
use super::rollout::{
    RolloutAction,
    RolloutConfig,
    RolloutDecision,
    evaluate,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::action::{
    LegalManagementActions,
    ManagementAction,
    ManagementPhase,
    legal_jail_actions,
};
use crate::game::engine::event::GameEvent;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RolloutStats {
    pub decisions: u64,
    pub changed_decisions: u64,
    pub simulated_games: u64,
    pub unfinished_games: u64,
    pub failures: u64,
}

impl RolloutStats {
    pub fn combine(
        self,
        other: Self,
    ) -> Self {
        Self {
            decisions: self.decisions + other.decisions,
            changed_decisions: self.changed_decisions + other.changed_decisions,
            simulated_games: self.simulated_games + other.simulated_games,
            unfinished_games: self.unfinished_games + other.unfinished_games,
            failures: self.failures + other.failures,
        }
    }
}

pub struct RolloutStrategy {
    baseline: ConfigurableStrategy,
    config: Option<RolloutConfig>,
    history: Vec<GameEvent>,
    pub stats: RolloutStats,
}

impl RolloutStrategy {
    pub fn new(
        baseline: ConfigurableStrategy,
        config: Option<RolloutConfig>,
    ) -> Self {
        Self {
            baseline,
            config,
            history: Vec::new(),
            stats: RolloutStats::default(),
        }
    }

    fn choose<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        decision: RolloutDecision,
        baseline: RolloutAction,
    ) -> RolloutAction {
        let Some(mut config) = self.config else {
            return baseline;
        };
        config.seed = config.seed.wrapping_add(self.stats.decisions.wrapping_mul(config.sample_count as u64));
        self.stats.decisions += 1;
        let observation = PublicObservation::new(state, rules, player, &self.history);
        let report = match evaluate(observation, decision, &[self.baseline; N], config) {
            Ok(report) => report,
            Err(_) => {
                self.stats.failures += 1;
                return baseline;
            },
        };
        let wins = |alternative: &super::rollout::ActionEvaluation| alternative.outcomes.iter().filter(|outcome| outcome.winner == Some(player)).count();
        let mut choice = baseline;
        let mut best = report.alternatives.iter().find(|alternative| alternative.action == baseline).map(wins).unwrap_or(0);
        for alternative in &report.alternatives {
            self.stats.simulated_games += alternative.outcomes.len() as u64;
            self.stats.unfinished_games += alternative.outcomes.iter().filter(|outcome| outcome.winner.is_none()).count() as u64;
            let score = wins(alternative);
            if score > best {
                best = score;
                choice = alternative.action;
            }
        }
        self.stats.changed_decisions += u64::from(choice != baseline);
        choice
    }
}

macro_rules! delegate {
    ($method:ident, $response:ty $(, $argument:ident: $kind:ty)*) => {
        fn $method<const N: usize>(&mut self, state: &GameState<N>, rules: &Ruleset, player: PlayerId, $($argument: $kind),*) -> $response {
            self.baseline.$method(state, rules, player, $($argument),*)
        }
    };
}

impl PlayerStrategy for RolloutStrategy {
    fn observe_public_event(
        &mut self,
        event: GameEvent,
    ) {
        if self.config.is_some() {
            self.history.push(event);
        }
    }

    fn choose_management_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        legal: &LegalManagementActions<'_, N>,
    ) -> Option<ManagementAction> {
        let baseline = self.baseline.choose_management_action(state, rules, player, legal);
        if self.config.is_none() || legal.iter().next().is_none() {
            return baseline;
        }
        let decision = match legal.phase() {
            ManagementPhase::Building => RolloutDecision::Building,
            ManagementPhase::Unmortgaging => RolloutDecision::Unmortgaging,
        };
        match self.choose(state, rules, player, decision, RolloutAction::Management(baseline)) {
            RolloutAction::Management(action) => action,
            _ => baseline,
        }
    }

    fn choose_jail_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
    ) -> JailAction {
        let baseline = self.baseline.choose_jail_action(state, rules, player);
        if self.config.is_none() || legal_jail_actions(state, rules, player).count() < 2 {
            return baseline;
        }
        match self.choose(state, rules, player, RolloutDecision::Jail, RolloutAction::Jail(baseline)) {
            RolloutAction::Jail(action) => action,
            _ => baseline,
        }
    }
    delegate!(should_purchase_property, bool, tile: TileId);
    delegate!(choose_max_auction_bid, Cash, tile: TileId);
    delegate!(choose_property_to_improve, Option<PropertyId>);
    delegate!(choose_tile_to_unmortgage, Option<TileId>);
    delegate!(propose_trade, Option<TradeOffer>);
    delegate!(should_accept_trade, bool, offer: &TradeOffer);
    delegate!(choose_liquidation_action, Option<crate::game::engine::liquidation::LiquidationAction>, amount: Cash);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::engine::action::legal_management_actions;
    use crate::game::ruleset::data::DEX_RULESET;

    #[test]
    fn tied_unfinished_rollouts_retain_baseline_and_track_cost() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 3);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let baseline = ConfigurableStrategy::new();
        let legal = legal_management_actions(&state, &DEX_RULESET, 0, ManagementPhase::Building);
        let expected = baseline.clone().choose_management_action(&state, &DEX_RULESET, 0, &legal);
        let mut player = RolloutStrategy::new(
            baseline,
            Some(RolloutConfig {
                seed: 17,
                sample_count: 2,
                max_turn_count: 1,
            }),
        );
        assert_eq!(player.choose_management_action(&state, &DEX_RULESET, 0, &legal), expected);
        assert_eq!(
            player.stats,
            RolloutStats {
                decisions: 1,
                simulated_games: 6,
                unfinished_games: 6,
                ..RolloutStats::default()
            }
        );
        let mut disabled = RolloutStrategy::new(baseline, None);
        assert_eq!(disabled.choose_management_action(&state, &DEX_RULESET, 0, &legal), expected);
        assert_eq!(disabled.stats, RolloutStats::default());
    }
}
