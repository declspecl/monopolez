use rayon::prelude::*;
use serde::Serialize;

use super::belief::RolloutBelief;
use super::configurable::ConfigurableStrategy;
use super::model::JailAction;
use super::observation::PublicObservation;
use crate::game::board::model::PlayerId;
use crate::game::engine::action::{
    ManagementAction,
    ManagementPhase,
    legal_jail_actions,
    legal_management_actions,
};
use crate::game::engine::turn::{
    TurnPhase,
    advance_turn,
    apply_jail_decision,
    apply_management_decision,
};
use crate::game::rng::WyRand;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::{
    GameState,
    PlayerSetMask,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
pub enum RolloutDecision {
    Building,
    Unmortgaging,
    Jail,
}

impl RolloutDecision {
    fn phase(self) -> TurnPhase {
        match self {
            Self::Building => TurnPhase::Building,
            Self::Unmortgaging => TurnPhase::Unmortgaging,
            Self::Jail => TurnPhase::Jail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RolloutAction {
    Management(Option<ManagementAction>),
    Jail(JailAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RolloutConfig {
    pub decisions: RolloutDecisions,
    pub seed: u64,
    pub sample_count: u32,
    pub max_turn_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RolloutDecisions {
    pub building: bool,
    pub unmortgaging: bool,
    pub jail: bool,
}

impl Default for RolloutDecisions {
    fn default() -> Self {
        Self {
            building: true,
            unmortgaging: true,
            jail: true,
        }
    }
}

impl RolloutDecisions {
    pub fn enabled(
        self,
        decision: RolloutDecision,
    ) -> bool {
        match decision {
            RolloutDecision::Building => self.building,
            RolloutDecision::Unmortgaging => self.unmortgaging,
            RolloutDecision::Jail => self.jail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RolloutOutcome {
    pub winner: Option<PlayerId>,
    pub bankrupt_players: PlayerSetMask,
    pub completed_turn_count: u32,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ActionEvaluation {
    pub action: RolloutAction,
    pub outcomes: Vec<RolloutOutcome>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct RolloutReport {
    pub player_id: PlayerId,
    pub decision: RolloutDecision,
    pub config: RolloutConfig,
    pub continuation_policies: Vec<ConfigurableStrategy>,
    pub alternatives: Vec<ActionEvaluation>,
}

pub fn evaluate<const N: usize>(
    observation: PublicObservation<'_>,
    decision: RolloutDecision,
    policies: &[ConfigurableStrategy; N],
    config: RolloutConfig,
) -> Result<RolloutReport, &'static str> {
    if config.sample_count == 0 || config.max_turn_count == 0 {
        return Err("rollouts require positive sample and turn counts");
    }
    if !(2..=8).contains(&N) || observation.current_player_id as usize >= N || observation.player_id != observation.current_player_id {
        return Err("rollouts require the current player and 2 to 8 players");
    }
    let active = ((1u16 << N) - 1) & !(observation.bankrupt_players as u16);
    if active.count_ones() < 2 || active & (1 << observation.player_id) == 0 {
        return Err("rollouts require an active player in an unfinished game");
    }
    let belief = RolloutBelief::new(observation)?;
    let state = belief.sample::<N>(&mut WyRand::new(config.seed))?;
    let actions: Vec<_> = match decision {
        RolloutDecision::Jail => legal_jail_actions(&state, observation.rules, observation.player_id).map(RolloutAction::Jail).collect(),
        RolloutDecision::Building | RolloutDecision::Unmortgaging => {
            let phase = if decision == RolloutDecision::Building {
                ManagementPhase::Building
            } else {
                ManagementPhase::Unmortgaging
            };
            std::iter::once(RolloutAction::Management(None))
                .chain(
                    legal_management_actions(&state, observation.rules, observation.player_id, phase)
                        .iter()
                        .map(|action| RolloutAction::Management(Some(action))),
                )
                .collect()
        },
    };
    if actions.is_empty() {
        return Err("no legal actions at the requested decision");
    }
    let samples = (0..config.sample_count)
        .into_par_iter()
        .map(|index| {
            let state = belief.sample::<N>(&mut WyRand::new(config.seed.wrapping_add(index as u64)))?;
            actions
                .iter()
                .map(|action| run(state, *policies, observation.rules, decision, *action, config.max_turn_count))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let alternatives = actions
        .into_iter()
        .enumerate()
        .map(|(index, action)| ActionEvaluation {
            action,
            outcomes: samples.iter().map(|sample| sample[index]).collect(),
        })
        .collect();
    Ok(RolloutReport {
        player_id: observation.player_id,
        decision,
        config,
        continuation_policies: policies.to_vec(),
        alternatives,
    })
}

fn run<const N: usize>(
    mut state: GameState<N>,
    mut strategies: [ConfigurableStrategy; N],
    rules: &Ruleset,
    decision: RolloutDecision,
    action: RolloutAction,
    limit: u32,
) -> Result<RolloutOutcome, &'static str> {
    let mut phase = match action {
        RolloutAction::Management(action) => apply_management_decision(&mut state, rules, &mut strategies, decision.phase(), action),
        RolloutAction::Jail(action) if decision == RolloutDecision::Jail => apply_jail_decision(&mut state, rules, &mut strategies, action),
        _ => return Err("rollout action does not match decision"),
    }
    .map_err(|_| "engine rejected rollout action")?;
    let mut completed_turn_count = 0;
    let mut winner = None;
    while completed_turn_count < limit {
        phase = advance_turn(&mut state, rules, &mut strategies, phase);
        if phase == TurnPhase::Finished {
            completed_turn_count += 1;
            let active = ((1u16 << N) - 1) & !(state.bankrupt_players as u16);
            if active.count_ones() == 1 {
                winner = Some(active.trailing_zeros() as PlayerId);
                break;
            }
            phase = TurnPhase::Start;
        }
    }
    Ok(RolloutOutcome {
        winner,
        bankrupt_players: state.bankrupt_players,
        completed_turn_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;

    #[test]
    fn alternatives_are_paired_reproducible_and_engine_validated() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 3);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let observation = PublicObservation::new(&state, &DEX_RULESET, 0, &[]);
        let config = RolloutConfig {
            decisions: Default::default(),
            seed: u64::MAX - 2,
            sample_count: 4,
            max_turn_count: 20,
        };
        let policies = [ConfigurableStrategy::new(); 2];
        let evaluate = || super::evaluate(observation, RolloutDecision::Building, &policies, config).unwrap();
        let serial = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(evaluate);
        let parallel = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(evaluate);
        assert_eq!(serial, parallel);
        assert_eq!(serial.alternatives.len(), 3);
        let belief = RolloutBelief::new(observation).unwrap();
        for index in 0..config.sample_count {
            let sample = belief.sample::<2>(&mut WyRand::new(config.seed.wrapping_add(index as u64))).unwrap();
            for alternative in &serial.alternatives {
                assert_eq!(
                    alternative.outcomes[index as usize],
                    run(sample, policies, &DEX_RULESET, RolloutDecision::Building, alternative.action, config.max_turn_count).unwrap()
                );
                assert!(alternative.outcomes[index as usize].completed_turn_count <= config.max_turn_count);
            }
        }
        assert!(
            run(
                state,
                policies,
                &DEX_RULESET,
                RolloutDecision::Building,
                RolloutAction::Management(Some(ManagementAction::Build(255))),
                1
            )
            .is_err()
        );
        assert!(super::evaluate(observation, RolloutDecision::Jail, &policies, config).is_err());
        assert!(super::evaluate(observation, RolloutDecision::Building, &policies, RolloutConfig { sample_count: 0, ..config }).is_err());
    }

    #[test]
    fn management_and_jail_preserve_unfinished_outcomes_at_one_turn_horizon() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        state.board.owned_tiles_by_player_id[0] = 1 << 5;
        state.board.mortgaged_tiles = 1 << 5;
        let config = RolloutConfig {
            decisions: Default::default(),
            seed: 4,
            sample_count: 3,
            max_turn_count: 1,
        };
        for decision in [RolloutDecision::Unmortgaging, RolloutDecision::Jail] {
            if decision == RolloutDecision::Jail {
                crate::game::engine::movement::send_player_to_jail(&mut state, 0);
            }
            let report = evaluate(PublicObservation::new(&state, &DEX_RULESET, 0, &[]), decision, &[ConfigurableStrategy::new(); 2], config).unwrap();
            assert_eq!(report.alternatives.len(), 2);
            for alternative in report.alternatives {
                assert!(alternative.outcomes.iter().all(|outcome| outcome.winner.is_none() && outcome.completed_turn_count == 1));
            }
        }
    }
}
