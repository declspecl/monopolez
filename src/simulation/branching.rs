use anyhow::{
    Result,
    bail,
    ensure,
};
use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;

use super::trace::snapshot;
use crate::game::board::model::PlayerId;
use crate::game::engine::action::{
    ManagementAction,
    ManagementPhase,
    legal_management_actions,
};
use crate::game::engine::turn::{
    TurnPhase,
    advance_turn,
    apply_management_decision,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::strategy::model::PlayerStrategy;

#[derive(Debug, PartialEq, Serialize)]
pub struct BranchOutcome {
    pub action: Option<ManagementAction>,
    pub state_after_action: Value,
    pub final_state: Value,
    pub winner: Option<PlayerId>,
    pub completed_turn_count: u32,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct BranchReport {
    pub decision_index: u64,
    pub completed_turns_before_fork: u32,
    pub phase: &'static str,
    pub player_id: PlayerId,
    pub fork_state: Value,
    pub baseline_action: Option<ManagementAction>,
    pub branches: Vec<BranchOutcome>,
}

pub fn branch_management(
    rules: &Ruleset,
    policies: &[ConfigurableStrategy],
    seed: u64,
    max_turn_count: u32,
    decision_index: u64,
) -> Result<BranchReport> {
    ensure!(max_turn_count > 0, "branching requires a positive turn limit");
    match policies.len() {
        2 => seek::<2>(rules, policies, seed, max_turn_count, decision_index),
        3 => seek::<3>(rules, policies, seed, max_turn_count, decision_index),
        4 => seek::<4>(rules, policies, seed, max_turn_count, decision_index),
        5 => seek::<5>(rules, policies, seed, max_turn_count, decision_index),
        6 => seek::<6>(rules, policies, seed, max_turn_count, decision_index),
        7 => seek::<7>(rules, policies, seed, max_turn_count, decision_index),
        8 => seek::<8>(rules, policies, seed, max_turn_count, decision_index),
        _ => bail!("branching requires 2 to 8 players"),
    }
}

fn winner<const N: usize>(state: &GameState<N>) -> Option<PlayerId> {
    let active = ((1u16 << N) - 1) & !(state.bankrupt_players as u16);
    (active.count_ones() == 1).then(|| active.trailing_zeros() as PlayerId)
}

fn seek<const N: usize>(
    rules: &Ruleset,
    policies: &[ConfigurableStrategy],
    seed: u64,
    limit: u32,
    target: u64,
) -> Result<BranchReport> {
    let mut state = GameState::<N>::create_starting_state(rules, seed);
    let mut strategies: [ConfigurableStrategy; N] = core::array::from_fn(|index| policies[index]);
    let mut phase = TurnPhase::Start;
    let mut completed_turns = 0;
    let mut index = 0;
    while completed_turns < limit {
        let management_phase = match phase {
            TurnPhase::Building => Some(ManagementPhase::Building),
            TurnPhase::Unmortgaging => Some(ManagementPhase::Unmortgaging),
            _ => None,
        };
        if let Some(management_phase) = management_phase {
            let legal = legal_management_actions(&state, rules, state.current_player_id, management_phase);
            if legal.iter().next().is_some() {
                if index == target {
                    let baseline_action = strategies[state.current_player_id as usize].choose_management_action(&state, rules, state.current_player_id, &legal);
                    let actions: Vec<_> = std::iter::once(None).chain(legal.iter().map(Some)).collect();
                    let branches = actions
                        .into_par_iter()
                        .map(|action| rollout(state, strategies, rules, phase, action, completed_turns, limit))
                        .collect::<Result<Vec<_>>>()?;
                    return Ok(BranchReport {
                        decision_index: target,
                        completed_turns_before_fork: completed_turns,
                        phase: if phase == TurnPhase::Building { "Building" } else { "Unmortgaging" },
                        player_id: state.current_player_id,
                        fork_state: snapshot(&state),
                        baseline_action,
                        branches,
                    });
                }
                index += 1;
            }
        }
        phase = advance_turn(&mut state, rules, &mut strategies, phase);
        if phase == TurnPhase::Finished {
            completed_turns += 1;
            if winner(&state).is_some() {
                break;
            }
            phase = TurnPhase::Start;
        }
    }
    bail!("management decision {target} was not reached; found {index} eligible decisions")
}

fn rollout<const N: usize>(
    mut state: GameState<N>,
    mut strategies: [ConfigurableStrategy; N],
    rules: &Ruleset,
    phase: TurnPhase,
    action: Option<ManagementAction>,
    mut completed_turn_count: u32,
    limit: u32,
) -> Result<BranchOutcome> {
    let mut phase = apply_management_decision(&mut state, rules, &mut strategies, phase, action).map_err(|error| anyhow::anyhow!("illegal branch action: {error:?}"))?;
    let state_after_action = snapshot(&state);
    while completed_turn_count < limit {
        phase = advance_turn(&mut state, rules, &mut strategies, phase);
        if phase == TurnPhase::Finished {
            completed_turn_count += 1;
            if winner(&state).is_some() {
                break;
            }
            phase = TurnPhase::Start;
        }
    }
    Ok(BranchOutcome {
        action,
        state_after_action,
        final_state: snapshot(&state),
        winner: winner(&state),
        completed_turn_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::engine::turn::play_game;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::data::DEX_OPTIMAL_STRATEGY;

    #[test]
    fn baseline_branch_matches_unbranched_game_and_parallelism_is_deterministic() {
        let policies = [DEX_OPTIMAL_STRATEGY; 4];
        let run = || branch_management(&DEX_RULESET, &policies, 3, 1000, 0).unwrap();
        let serial = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(run);
        let parallel = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(run);
        assert_eq!(serial, parallel);
        assert!(serial.branches.len() >= 2);
        assert_eq!(serial.branches[0].action, None);
        let baseline = serial.branches.iter().find(|branch| branch.action == serial.baseline_action).unwrap();
        let mut state = GameState::<4>::create_starting_state(&DEX_RULESET, 3);
        let summary = play_game(&mut state, &DEX_RULESET, &mut policies.clone(), 1000);
        assert_eq!(baseline.final_state, snapshot(&state));
        assert_eq!(baseline.completed_turn_count, summary.turn_count);
    }

    #[test]
    fn alternatives_change_only_the_selected_action_before_resuming() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let policies = [ConfigurableStrategy::new(); 2];
        let original = snapshot(&state);
        let stopped = rollout(state, policies, &rules, TurnPhase::Building, None, 10, 11).unwrap();
        assert_eq!(stopped.state_after_action, original);
        for property in [0, 1] {
            let action = Some(ManagementAction::Build(property));
            let branch = rollout(state, policies, &rules, TurnPhase::Building, action, 10, 11).unwrap();
            let mut expected = state;
            apply_management_decision(&mut expected, &rules, &mut policies.clone(), TurnPhase::Building, action).unwrap();
            assert_eq!(branch.state_after_action, snapshot(&expected));
            assert_eq!(branch.completed_turn_count, 11);
        }
        assert_eq!(snapshot(&state), original);
    }

    #[test]
    fn unmortgage_branches_apply_one_asset_action_and_reject_illegal_choices() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.mortgaged_tiles = state.board.owned_tiles_by_player_id[0];
        let policies = [ConfigurableStrategy::new(); 2];
        for tile in [1, 3] {
            let action = Some(ManagementAction::Unmortgage(tile));
            let branch = rollout(state, policies, &rules, TurnPhase::Unmortgaging, action, 0, 1).unwrap();
            let mut expected = state;
            apply_management_decision(&mut expected, &rules, &mut policies.clone(), TurnPhase::Unmortgaging, action).unwrap();
            assert_eq!(branch.state_after_action, snapshot(&expected));
        }
        assert!(rollout(state, policies, &rules, TurnPhase::Unmortgaging, Some(ManagementAction::Build(0)), 0, 1).is_err());
    }

    #[test]
    fn rejects_invalid_or_unreachable_branch_requests() {
        let policies = [ConfigurableStrategy::new(); 2];
        assert!(branch_management(&Ruleset::default(), &[], 0, 10, 0).is_err());
        assert!(branch_management(&Ruleset::default(), &policies, 0, 0, 0).is_err());
        assert!(branch_management(&Ruleset::default(), &policies, 0, 1, 0).is_err());
    }
}
