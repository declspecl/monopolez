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
    legal_jail_actions,
    legal_management_actions,
};
use crate::game::engine::turn::{
    TurnPhase,
    advance_turn,
    apply_jail_decision,
    apply_management_decision,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::strategy::model::{
    JailAction,
    PlayerStrategy,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BranchAction {
    Management(Option<ManagementAction>),
    Jail(JailAction),
}

impl From<Option<ManagementAction>> for BranchAction {
    fn from(action: Option<ManagementAction>) -> Self {
        Self::Management(action)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchKind {
    Management,
    Jail,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct BranchOutcome {
    pub action: BranchAction,
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
    pub baseline_action: BranchAction,
    pub branches: Vec<BranchOutcome>,
}

pub fn branch_management(
    rules: &Ruleset,
    policies: &[ConfigurableStrategy],
    seed: u64,
    max_turn_count: u32,
    decision_index: u64,
) -> Result<BranchReport> {
    branch_decision(rules, policies, seed, max_turn_count, decision_index, BranchKind::Management)
}

pub fn branch_decision(
    rules: &Ruleset,
    policies: &[ConfigurableStrategy],
    seed: u64,
    max_turn_count: u32,
    decision_index: u64,
    kind: BranchKind,
) -> Result<BranchReport> {
    ensure!(max_turn_count > 0, "branching requires a positive turn limit");
    match policies.len() {
        2 => seek::<2>(rules, policies, seed, max_turn_count, decision_index, kind),
        3 => seek::<3>(rules, policies, seed, max_turn_count, decision_index, kind),
        4 => seek::<4>(rules, policies, seed, max_turn_count, decision_index, kind),
        5 => seek::<5>(rules, policies, seed, max_turn_count, decision_index, kind),
        6 => seek::<6>(rules, policies, seed, max_turn_count, decision_index, kind),
        7 => seek::<7>(rules, policies, seed, max_turn_count, decision_index, kind),
        8 => seek::<8>(rules, policies, seed, max_turn_count, decision_index, kind),
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
    kind: BranchKind,
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
        if let Some(management_phase) = management_phase.filter(|_| kind == BranchKind::Management) {
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
                        baseline_action: baseline_action.into(),
                        branches,
                    });
                }
                index += 1;
            }
        }
        if kind == BranchKind::Jail && phase == TurnPhase::Jail && state.jailed_players & (1 << state.current_player_id) != 0 {
            if index == target {
                let baseline_action = strategies[state.current_player_id as usize].choose_jail_action(&state, rules, state.current_player_id);
                let actions: Vec<_> = legal_jail_actions(&state, rules, state.current_player_id).map(BranchAction::Jail).collect();
                let branches = actions
                    .into_par_iter()
                    .map(|action| rollout(state, strategies, rules, phase, action, completed_turns, limit))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(BranchReport {
                    decision_index: target,
                    completed_turns_before_fork: completed_turns,
                    phase: "Jail",
                    player_id: state.current_player_id,
                    fork_state: snapshot(&state),
                    baseline_action: BranchAction::Jail(baseline_action),
                    branches,
                });
            }
            index += 1;
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
    bail!("{kind:?} decision {target} was not reached; found {index} eligible decisions")
}

fn rollout<const N: usize>(
    mut state: GameState<N>,
    mut strategies: [ConfigurableStrategy; N],
    rules: &Ruleset,
    phase: TurnPhase,
    action: impl Into<BranchAction>,
    mut completed_turn_count: u32,
    limit: u32,
) -> Result<BranchOutcome> {
    let action = action.into();
    let mut phase = match action {
        BranchAction::Management(action) => apply_management_decision(&mut state, rules, &mut strategies, phase, action),
        BranchAction::Jail(action) if phase == TurnPhase::Jail => apply_jail_decision(&mut state, rules, &mut strategies, action),
        _ => Err(crate::game::engine::action::ActionError::WrongPhase),
    }
    .map_err(|error| anyhow::anyhow!("illegal branch action: {error:?}"))?;
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
        assert_eq!(serial.branches[0].action, BranchAction::Management(None));
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
    fn jail_baseline_branch_matches_normal_play() {
        let policies = [DEX_OPTIMAL_STRATEGY; 4];
        for rules in [Ruleset::default(), DEX_RULESET] {
            let report = branch_decision(&rules, &policies, 3, 1000, 0, BranchKind::Jail).unwrap();
            assert_eq!(report.phase, "Jail");
            assert!(report.branches.iter().any(|branch| branch.action == BranchAction::Jail(JailAction::RollForDoubles)));
            let branch = report.branches.iter().find(|branch| branch.action == report.baseline_action).unwrap();
            let mut state = GameState::<4>::create_starting_state(&rules, 3);
            let summary = play_game(&mut state, &rules, &mut policies.clone(), 1000);
            assert_eq!(branch.final_state, snapshot(&state));
            assert_eq!(branch.completed_turn_count, summary.turn_count);
        }
    }

    #[test]
    fn jail_branches_charge_bail_or_consume_a_card_once() {
        use crate::game::engine::movement::send_player_to_jail;
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        send_player_to_jail(&mut state, 0);
        let policies = [ConfigurableStrategy::new(); 2];
        assert!(rollout(state, policies, &rules, TurnPhase::Jail, BranchAction::Jail(JailAction::UseGetOutOfJailFreeCard), 0, 1).is_err());
        state.get_out_of_jail_free_card_holder_by_deck_kind[0] = Some(0);
        for action in [JailAction::PayBail, JailAction::UseGetOutOfJailFreeCard] {
            let branch = rollout(state, policies, &rules, TurnPhase::Jail, BranchAction::Jail(action), 0, 1).unwrap();
            let mut expected = state;
            expected.jailed_players = 0;
            if action == JailAction::PayBail {
                expected.cash_by_player_id[0] -= rules.jail_bail_amount as crate::game::tile::model::Cash;
            } else {
                expected.get_out_of_jail_free_card_holder_by_deck_kind[0] = None;
            }
            assert_eq!(branch.state_after_action, snapshot(&expected));
            assert_eq!(branch.completed_turn_count, 1);
        }
        state.cash_by_player_id[0] = 0;
        assert!(rollout(state, policies, &rules, TurnPhase::Jail, BranchAction::Jail(JailAction::PayBail), 0, 1).is_err());
    }

    #[test]
    fn rejects_invalid_or_unreachable_branch_requests() {
        let policies = [ConfigurableStrategy::new(); 2];
        assert!(branch_management(&Ruleset::default(), &[], 0, 10, 0).is_err());
        assert!(branch_management(&Ruleset::default(), &policies, 0, 0, 0).is_err());
        assert!(branch_management(&Ruleset::default(), &policies, 0, 1, 0).is_err());
    }
}
