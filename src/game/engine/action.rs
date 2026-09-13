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
use crate::game::strategy::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::tile::data::{
    PROPERTY_COUNT,
    TILE_COUNT,
};
use crate::game::tile::model::{
    Cash,
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

pub struct LegalManagementActions<'a, const N: usize> {
    phase: ManagementPhase,
    state: &'a GameState<N>,
    rules: &'a Ruleset,
    player: PlayerId,
}

impl<const N: usize> LegalManagementActions<'_, N> {
    pub const fn phase(&self) -> ManagementPhase {
        self.phase
    }

    pub fn iter(&self) -> impl Iterator<Item = ManagementAction> + '_ {
        let count = match self.phase {
            ManagementPhase::Building => PROPERTY_COUNT,
            ManagementPhase::Unmortgaging => TILE_COUNT,
        };
        (0..count as u8)
            .map(|id| match self.phase {
                ManagementPhase::Building => ManagementAction::Build(id),
                ManagementPhase::Unmortgaging => ManagementAction::Unmortgage(id),
            })
            .filter(|action| self.contains(*action))
    }

    pub fn contains(
        &self,
        action: ManagementAction,
    ) -> bool {
        if self.player as usize >= N || self.state.bankrupt_players & (1 << self.player) != 0 || self.state.current_player_id != self.player {
            return false;
        }
        match (self.phase, action) {
            (ManagementPhase::Building, ManagementAction::Build(property)) => can_improve_property(self.state, self.rules, self.player, property),
            (ManagementPhase::Unmortgaging, ManagementAction::Unmortgage(tile)) => can_unmortgage_tile(self.state, self.player, tile),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    InvalidPlayer,
    WrongPhase,
    IllegalAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JailResolution {
    Released,
    AttemptDoubles,
}

pub fn validate_jail_action<const N: usize>(
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    action: JailAction,
) -> Result<(), ActionError> {
    if player as usize >= N || state.bankrupt_players & (1 << player) != 0 || state.current_player_id != player {
        return Err(ActionError::InvalidPlayer);
    }
    if state.jailed_players & (1 << player) == 0 {
        return Err(ActionError::WrongPhase);
    }
    match action {
        JailAction::RollForDoubles => Ok(()),
        JailAction::UseGetOutOfJailFreeCard if state.get_out_of_jail_free_card_holder_by_deck_kind.contains(&Some(player)) => Ok(()),
        JailAction::PayBail if state.cash_by_player_id[player as usize] >= rules.jail_bail_amount as Cash => {
            if rules.free_parking_jackpot_mode == crate::game::ruleset::model::FreeParkingJackpotMode::TaxesAndFees && state.free_parking_jackpot.checked_add(rules.jail_bail_amount as Cash).is_none()
            {
                return Err(ActionError::IllegalAction);
            }
            Ok(())
        },
        _ => Err(ActionError::IllegalAction),
    }
}

pub fn legal_jail_actions<const N: usize>(
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
) -> impl Iterator<Item = JailAction> {
    let valid = [JailAction::RollForDoubles, JailAction::PayBail, JailAction::UseGetOutOfJailFreeCard].map(|action| validate_jail_action(state, rules, player, action).ok().map(|()| action));
    valid.into_iter().flatten()
}

pub fn execute_jail_action<const N: usize, S: PlayerStrategy>(
    state: &mut GameState<N>,
    rules: &Ruleset,
    strategies: &mut [S; N],
    player: PlayerId,
    action: JailAction,
) -> Result<JailResolution, ActionError> {
    validate_jail_action(state, rules, player, action)?;
    match action {
        JailAction::RollForDoubles => return Ok(JailResolution::AttemptDoubles),
        JailAction::UseGetOutOfJailFreeCard => {
            let holder = state
                .get_out_of_jail_free_card_holder_by_deck_kind
                .iter_mut()
                .find(|holder| **holder == Some(player))
                .ok_or(ActionError::IllegalAction)?;
            *holder = None;
        },
        JailAction::PayBail => {
            super::payment::charge_player(state, rules, strategies, player, rules.jail_bail_amount as Cash, super::payment::select_fee_creditor(rules));
        },
    }
    super::movement::release_player_from_jail(state, player);
    Ok(JailResolution::Released)
}

pub fn validate_purchase<const N: usize>(
    state: &GameState<N>,
    player: PlayerId,
    tile: TileId,
) -> Result<Cash, ActionError> {
    use crate::game::tile::lut::{
        OWNABLE_TILE_SET_MASK,
        PURCHASE_PRICE_BY_TILE_ID,
    };
    if player as usize >= N || state.bankrupt_players & (1 << player) != 0 || state.current_player_id != player {
        return Err(ActionError::InvalidPlayer);
    }
    if tile as usize >= TILE_COUNT || OWNABLE_TILE_SET_MASK & (1 << tile) == 0 || state.position_by_player_id[player as usize] != tile || state.board.get_tile_owner(tile).is_some() {
        return Err(ActionError::IllegalAction);
    }
    let price = PURCHASE_PRICE_BY_TILE_ID[tile as usize] as Cash;
    if state.cash_by_player_id[player as usize] < price {
        return Err(ActionError::IllegalAction);
    }
    Ok(price)
}

pub fn execute_purchase<const N: usize>(
    state: &mut GameState<N>,
    player: PlayerId,
    tile: TileId,
) -> Result<GameEvent, ActionError> {
    let price = validate_purchase(state, player, tile)?;
    state.cash_by_player_id[player as usize] -= price;
    state.board.owned_tiles_by_player_id[player as usize] |= 1 << tile;
    Ok(GameEvent::PropertyPurchased {
        player_id: player,
        tile_id: tile,
        price,
        auction: false,
    })
}

pub fn legal_management_actions<'a, const N: usize>(
    state: &'a GameState<N>,
    rules: &'a Ruleset,
    player: PlayerId,
    phase: ManagementPhase,
) -> LegalManagementActions<'a, N> {
    LegalManagementActions { phase, state, rules, player }
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
    fn jail_choices_are_legal_and_rechecked_on_execution() {
        use crate::game::strategy::configurable::ConfigurableStrategy;
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        let mut strategies = [ConfigurableStrategy::new(); 2];
        assert_eq!(legal_jail_actions(&state, &rules, 0).count(), 0);
        super::super::movement::send_player_to_jail(&mut state, 0);
        assert_eq!(legal_jail_actions(&state, &rules, 0).collect::<Vec<_>>(), vec![JailAction::RollForDoubles, JailAction::PayBail]);
        state.cash_by_player_id[0] = 0;
        let original = state;
        for action in [JailAction::PayBail, JailAction::UseGetOutOfJailFreeCard] {
            assert_eq!(execute_jail_action(&mut state, &rules, &mut strategies, 0, action), Err(ActionError::IllegalAction));
            assert_eq!(state, original);
        }
        assert_eq!(
            execute_jail_action(&mut state, &rules, &mut strategies, 255, JailAction::RollForDoubles),
            Err(ActionError::InvalidPlayer)
        );
        assert_eq!(state, original);
        state.get_out_of_jail_free_card_holder_by_deck_kind = [Some(0), Some(0)];
        assert_eq!(
            execute_jail_action(&mut state, &rules, &mut strategies, 0, JailAction::UseGetOutOfJailFreeCard),
            Ok(JailResolution::Released)
        );
        assert_eq!(state.get_out_of_jail_free_card_holder_by_deck_kind, [None, Some(0)]);
        assert_eq!(state.jailed_players, 0);
    }

    #[test]
    fn bail_is_charged_once_before_release() {
        use crate::game::strategy::configurable::ConfigurableStrategy;
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        let mut strategies = [ConfigurableStrategy::new(); 2];
        super::super::movement::send_player_to_jail(&mut state, 0);
        state.cash_by_player_id[0] = rules.jail_bail_amount as Cash;
        assert_eq!(execute_jail_action(&mut state, &rules, &mut strategies, 0, JailAction::PayBail), Ok(JailResolution::Released));
        assert_eq!(state.cash_by_player_id[0], 0);
        assert_eq!(state.free_parking_jackpot, rules.jail_bail_amount as Cash);
        let original = state;
        assert_eq!(execute_jail_action(&mut state, &rules, &mut strategies, 0, JailAction::PayBail), Err(ActionError::WrongPhase));
        assert_eq!(state, original);
    }

    #[test]
    fn purchases_require_current_location_ownership_and_cash() {
        let mut state = GameState::<2>::create_starting_state(&Ruleset::default(), 0);
        state.position_by_player_id[0] = 1;
        let original = state;
        for (player, tile) in [(255, 1), (1, 1), (0, 255), (0, 0), (0, 3)] {
            assert!(execute_purchase(&mut state, player, tile).is_err());
            assert_eq!(state, original);
        }
        state.cash_by_player_id[0] = 59;
        let poor_state = state;
        assert!(execute_purchase(&mut state, 0, 1).is_err());
        assert_eq!(state, poor_state);
        state.cash_by_player_id[0] = 60;
        assert_eq!(
            execute_purchase(&mut state, 0, 1),
            Ok(GameEvent::PropertyPurchased {
                player_id: 0,
                tile_id: 1,
                price: 60,
                auction: false
            })
        );
        assert_eq!(state.cash_by_player_id[0], 0);
        state.cash_by_player_id[0] = 60;
        let bought_state = state;
        assert!(execute_purchase(&mut state, 0, 1).is_err());
        assert_eq!(state, bought_state);
    }

    #[test]
    fn stale_purchase_cannot_take_another_players_property() {
        let mut state = GameState::<2>::create_starting_state(&Ruleset::default(), 0);
        state.position_by_player_id[0] = 1;
        assert!(validate_purchase(&state, 0, 1).is_ok());
        state.board.owned_tiles_by_player_id[1] |= 1 << 1;
        let original = state;
        assert!(execute_purchase(&mut state, 0, 1).is_err());
        assert_eq!(state, original);
    }

    #[test]
    fn stale_actions_are_revalidated_before_execution() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        let legal = legal_management_actions(&state, &rules, 0, ManagementPhase::Building);
        assert_eq!(legal.iter().collect::<Vec<_>>(), vec![ManagementAction::Build(0), ManagementAction::Build(1)]);
        let selected = legal.iter().next().unwrap();
        state.cash_by_player_id[0] = 0;
        let original = state;
        assert_eq!(execute_management_action(&mut state, &rules, 0, ManagementPhase::Building, selected), Err(ActionError::IllegalAction));
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
