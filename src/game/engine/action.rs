use serde::{
    Deserialize,
    Serialize,
};

use super::event::GameEvent;
use super::improvement::{
    can_improve_property,
    improve_property,
};
use super::mortgage::{
    can_unmortgage_tile,
    unmortgage_tile,
};
use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::{
    PROPERTY_COUNT,
    TILE_COUNT,
};
use crate::game::tile::model::{
    PropertyId,
    TileId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagementAction {
    Build(PropertyId),
    Unmortgage(TileId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagementPhase {
    Building,
    Unmortgaging,
}

pub struct LegalManagementActions {
    phase: ManagementPhase,
    mask: u64,
}

impl LegalManagementActions {
    pub const fn phase(&self) -> ManagementPhase {
        self.phase
    }

    pub fn iter(&self) -> impl Iterator<Item = ManagementAction> + '_ {
        (0..TILE_COUNT as u8).filter(|id| self.mask & (1u64 << id) != 0).map(|id| match self.phase {
            ManagementPhase::Building => ManagementAction::Build(id),
            ManagementPhase::Unmortgaging => ManagementAction::Unmortgage(id),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    InvalidPlayer,
    WrongPhase,
    IllegalAction,
}

pub fn legal_management_actions<const N: usize>(
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    phase: ManagementPhase,
) -> LegalManagementActions {
    let mut mask = 0;
    if player as usize >= N || state.bankrupt_players & (1 << player) != 0 || state.current_player_id != player {
        return LegalManagementActions { phase, mask };
    }
    let count = match phase {
        ManagementPhase::Building => PROPERTY_COUNT,
        ManagementPhase::Unmortgaging => TILE_COUNT,
    };
    for id in 0..count as u8 {
        let legal = match phase {
            ManagementPhase::Building => can_improve_property(state, rules, player, id),
            ManagementPhase::Unmortgaging => can_unmortgage_tile(state, player, id),
        };
        if legal {
            mask |= 1 << id;
        }
    }
    LegalManagementActions { phase, mask }
}

pub fn execute_management_action<const N: usize>(
    state: &mut GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    phase: ManagementPhase,
    action: ManagementAction,
) -> Result<GameEvent, ActionError> {
    if player as usize >= N || state.bankrupt_players & (1 << player) != 0 || state.current_player_id != player {
        return Err(ActionError::InvalidPlayer);
    }
    match (phase, action) {
        (ManagementPhase::Building, ManagementAction::Build(property_id)) => {
            if !improve_property(state, rules, player, property_id) {
                return Err(ActionError::IllegalAction);
            }
            Ok(GameEvent::BuildingPurchased {
                player_id: player,
                property_id,
                level: state.board.improvement_level_by_property_id[property_id as usize],
            })
        },
        (ManagementPhase::Unmortgaging, ManagementAction::Unmortgage(tile_id)) => {
            if !unmortgage_tile(state, player, tile_id) {
                return Err(ActionError::IllegalAction);
            }
            Ok(GameEvent::TileUnmortgaged { player_id: player, tile_id })
        },
        _ => Err(ActionError::WrongPhase),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_actions_are_revalidated_before_execution() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let legal = legal_management_actions(&state, &rules, 0, ManagementPhase::Building);
        assert_eq!(legal.iter().collect::<Vec<_>>(), vec![ManagementAction::Build(0), ManagementAction::Build(1)]);
        state.cash_by_player_id[0] = 0;
        let original = state;
        assert_eq!(
            execute_management_action(&mut state, &rules, 0, ManagementPhase::Building, legal.iter().next().unwrap()),
            Err(ActionError::IllegalAction)
        );
        assert_eq!(state, original);
    }

    #[test]
    fn phase_and_actor_cannot_be_bypassed() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[0] = 1 << 1;
        state.board.mortgaged_tiles = 1 << 1;
        let original = state;
        assert_eq!(
            execute_management_action(&mut state, &rules, 0, ManagementPhase::Building, ManagementAction::Unmortgage(1)),
            Err(ActionError::WrongPhase)
        );
        assert_eq!(
            execute_management_action(&mut state, &rules, 1, ManagementPhase::Unmortgaging, ManagementAction::Unmortgage(1)),
            Err(ActionError::InvalidPlayer)
        );
        assert_eq!(state, original);
        assert!(execute_management_action(&mut state, &rules, 0, ManagementPhase::Unmortgaging, ManagementAction::Unmortgage(1)).is_ok());
        assert_eq!(state.board.mortgaged_tiles, 0);
    }
}
