use serde::{
    Deserialize,
    Serialize,
};

use super::action::ActionError;
use super::event::GameEvent;
use super::improvement::{
    can_sell_property_building,
    sell_property_building,
};
use super::mortgage::{
    can_mortgage_tile,
    mortgage_tile,
};
use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::{
    PROPERTY_COUNT,
    TILE_COUNT,
};
use crate::game::tile::lut::MORTGAGE_VALUE_BY_TILE_ID;
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidationAction {
    SellBuilding(PropertyId),
    Mortgage(TileId),
}

pub fn default_liquidation_action<const N: usize>(
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
) -> Option<LiquidationAction> {
    if player as usize >= N {
        return None;
    }
    super::improvement::find_most_improved_property(state, player)
        .map(LiquidationAction::SellBuilding)
        .or_else(|| legal_liquidation_actions(state, rules, player).next())
}

pub fn legal_liquidation_actions<'a, const N: usize>(
    state: &'a GameState<N>,
    rules: &'a Ruleset,
    player: PlayerId,
) -> impl Iterator<Item = LiquidationAction> + 'a {
    (0..PROPERTY_COUNT as u8)
        .filter(move |property| can_sell_property_building(state, rules, player, *property))
        .map(LiquidationAction::SellBuilding)
        .chain((0..TILE_COUNT as u8).filter(move |tile| can_mortgage_tile(state, player, *tile)).map(LiquidationAction::Mortgage))
}

pub fn execute_liquidation_action<const N: usize>(
    state: &mut GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    action: LiquidationAction,
) -> Result<GameEvent, ActionError> {
    if player as usize >= N || state.bankrupt_players & (1 << player) != 0 {
        return Err(ActionError::InvalidPlayer);
    }
    match action {
        LiquidationAction::SellBuilding(property_id) => {
            let sale = sell_property_building(state, rules, player, property_id).ok_or(ActionError::IllegalAction)?;
            Ok(GameEvent::BuildingSold {
                player_id: player,
                property_id,
                previous_level: sale.previous_level,
                level: sale.level,
                proceeds: sale.proceeds,
            })
        },
        LiquidationAction::Mortgage(tile_id) => {
            if !mortgage_tile(state, player, tile_id) {
                return Err(ActionError::IllegalAction);
            }
            Ok(GameEvent::TileMortgaged {
                player_id: player,
                tile_id,
                proceeds: MORTGAGE_VALUE_BY_TILE_ID[tile_id as usize] as Cash,
            })
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;

    #[test]
    fn dex_hotel_can_be_liquidated_without_replacement_houses() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.improvement_level_by_property_id[0] = 5;
        state.board.improvement_level_by_property_id[1] = 5;
        state.board.bank_hotel_count -= 2;
        state.board.bank_house_count = 0;
        assert!(legal_liquidation_actions(&state, &DEX_RULESET, 0).any(|action| action == LiquidationAction::SellBuilding(0)));
        let cash = state.cash_by_player_id[0];
        assert_eq!(
            execute_liquidation_action(&mut state, &DEX_RULESET, 0, LiquidationAction::SellBuilding(0)),
            Ok(GameEvent::BuildingSold {
                player_id: 0,
                property_id: 0,
                previous_level: 5,
                level: 0,
                proceeds: 125
            })
        );
        assert_eq!(state.cash_by_player_id[0], cash + 125);
        assert_eq!(state.board.improvement_level_by_property_id[1], 5);
    }

    #[test]
    fn invalid_liquidation_leaves_state_unchanged() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 0);
        let original = state;
        for action in [
            LiquidationAction::SellBuilding(255),
            LiquidationAction::SellBuilding(0),
            LiquidationAction::Mortgage(255),
            LiquidationAction::Mortgage(1),
        ] {
            assert!(execute_liquidation_action(&mut state, &DEX_RULESET, 0, action).is_err());
            assert_eq!(state, original);
        }
    }

    #[test]
    fn mortgage_first_policy_preserves_rent_producing_buildings() {
        use crate::game::engine::payment::{
            Creditor,
            charge_player,
        };
        use crate::game::strategy::configurable::ConfigurableStrategy;
        for mortgage_first in [false, true] {
            let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 0);
            state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3) | (1 << 12);
            state.board.improvement_level_by_property_id[0] = 1;
            state.board.bank_house_count -= 1;
            state.cash_by_player_id[0] = 0;
            let mut strategies = [ConfigurableStrategy {
                mortgages_before_selling_buildings: mortgage_first,
                ..ConfigurableStrategy::new()
            }; 2];
            charge_player(&mut state, &DEX_RULESET, &mut strategies, 0, 50, Creditor::Bank);
            assert_eq!(state.bankrupt_players, 0);
            if mortgage_first {
                assert_eq!(state.board.improvement_level_by_property_id[0], 1);
                assert_eq!(state.board.mortgaged_tiles, 1 << 12);
                assert_eq!(state.cash_by_player_id[0], 25);
            } else {
                assert_eq!(state.board.improvement_level_by_property_id[0], 0);
                assert_eq!(state.board.mortgaged_tiles, 1 << 1);
                assert_eq!(state.cash_by_player_id[0], 5);
            }
        }
    }
}
