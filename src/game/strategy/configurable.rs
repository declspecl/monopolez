use serde::{
    Deserialize,
    Serialize,
};

use super::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::board::data::{
    HOTEL_IMPROVEMENT_LEVEL,
    MAX_HOUSE_IMPROVEMENT_LEVEL,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::improvement::can_improve_property;
use crate::game::engine::trade::calculate_tile_set_purchase_value;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    HOUSE_PURCHASE_PRICE_BY_TILE_ID,
    OWNABLE_TILE_SET_MASK,
    OWNERSHIP_GROUP_BY_TILE_ID,
    PURCHASE_PRICE_BY_TILE_ID,
    TILE_ID_BY_PROPERTY_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
    UNMORTGAGE_PRICE_BY_TILE_ID,
};
use crate::game::tile::model::{
    Cash,
    OwnershipGroup,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

const PERCENT_DIVISOR: Cash = 100;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingAllocation {
    #[default]
    Spread,
    Concentrate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DevelopmentCeiling {
    ThreeHouses,
    FourHouses,
    #[default]
    Hotel,
}

impl DevelopmentCeiling {
    pub const fn improvement_level(self) -> u8 {
        match self {
            Self::ThreeHouses => 3,
            Self::FourHouses => MAX_HOUSE_IMPROVEMENT_LEVEL,
            Self::Hotel => HOTEL_IMPROVEMENT_LEVEL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfigurableStrategy {
    pub cash_reserve: Cash,
    pub purchase_cash_percent: Cash,
    pub auction_bid_percent: Cash,
    pub improvement_cash_percent: Cash,
    pub unmortgage_cash_percent: Cash,
    pub trade_offer_percent: Cash,
    pub trade_accept_percent: Cash,
    pub pays_bail_when_affordable: bool,
    #[serde(default)]
    pub mortgages_before_selling_buildings: bool,
    #[serde(default, alias = "camps_in_jail_below_unowned_tile_count")]
    pub jail_camping_unowned_tile_threshold: Option<u8>,
    #[serde(default)]
    pub building_allocation: BuildingAllocation,
    #[serde(default)]
    pub development_ceiling: DevelopmentCeiling,
}

impl ConfigurableStrategy {
    pub const fn new() -> Self {
        Self {
            cash_reserve: 100,
            purchase_cash_percent: 100,
            auction_bid_percent: 100,
            improvement_cash_percent: 100,
            unmortgage_cash_percent: 100,
            trade_offer_percent: 150,
            trade_accept_percent: 100,
            pays_bail_when_affordable: false,
            mortgages_before_selling_buildings: false,
            jail_camping_unowned_tile_threshold: None,
            building_allocation: BuildingAllocation::Spread,
            development_ceiling: DevelopmentCeiling::Hotel,
        }
    }

    const fn calculate_required_cash(
        &self,
        price: Cash,
        cash_percent: Cash,
    ) -> Cash {
        price * cash_percent / PERCENT_DIVISOR + self.cash_reserve
    }
}

impl Default for ConfigurableStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerStrategy for ConfigurableStrategy {
    fn choose_liquidation_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        _required_amount: Cash,
    ) -> Option<crate::game::engine::liquidation::LiquidationAction> {
        use crate::game::engine::liquidation::{
            LiquidationAction,
            default_liquidation_action,
            legal_liquidation_actions,
        };
        if self.mortgages_before_selling_buildings
            && let Some(action) = legal_liquidation_actions(state, rules, player).find(|action| matches!(action, LiquidationAction::Mortgage(_)))
        {
            return Some(action);
        }
        default_liquidation_action(state, rules, player)
    }

    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;

        game_state.cash_by_player_id[player_id as usize] >= self.calculate_required_cash(purchase_price, self.purchase_cash_percent)
    }

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash {
        let purchase_price = PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
        let max_bid = purchase_price * self.auction_bid_percent / PERCENT_DIVISOR;
        let spendable_cash = game_state.cash_by_player_id[player_id as usize].saturating_sub(self.cash_reserve);

        max_bid.min(spendable_cash)
    }

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> JailAction {
        if let Some(threshold) = self.jail_camping_unowned_tile_threshold {
            let owned_tiles = game_state.board.owned_tiles_by_player_id.iter().fold(0, |owned_tiles, player_tiles| owned_tiles | player_tiles);
            let unowned_tile_count = (OWNABLE_TILE_SET_MASK & !owned_tiles).count_ones();
            if unowned_tile_count <= u32::from(threshold) {
                return JailAction::RollForDoubles;
            }
        }

        if game_state.get_out_of_jail_free_card_holder_by_deck_kind.contains(&Some(player_id)) {
            return JailAction::UseGetOutOfJailFreeCard;
        }

        let should_pay_bail = self.jail_camping_unowned_tile_threshold.is_some() || self.pays_bail_when_affordable;
        let jail_bail_amount = ruleset.jail_bail_amount as Cash;
        let available_cash = game_state.cash_by_player_id[player_id as usize].checked_sub(self.cash_reserve);
        if should_pay_bail && available_cash.is_some_and(|cash| cash >= jail_bail_amount) {
            return JailAction::PayBail;
        }

        JailAction::RollForDoubles
    }

    fn choose_property_to_improve<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<PropertyId> {
        let mut group_development = [0u8; OwnershipGroup::COUNT];
        if self.building_allocation == BuildingAllocation::Concentrate {
            for (property_id, tile_id) in TILE_ID_BY_PROPERTY_ID.iter().enumerate() {
                let group = OWNERSHIP_GROUP_BY_TILE_ID[*tile_id as usize].expect("property tiles should have an ownership group");
                group_development[group as usize] += game_state.board.improvement_level_by_property_id[property_id];
            }
        }
        let mut selected_property = None;
        let mut selected_priority = None;

        for property_id in 0..PROPERTY_COUNT {
            let tile_id = TILE_ID_BY_PROPERTY_ID[property_id];
            let improvement_level = game_state.board.improvement_level_by_property_id[property_id];
            if improvement_level >= self.development_ceiling.improvement_level() {
                continue;
            }
            let house_purchase_price = HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] < self.calculate_required_cash(house_purchase_price, self.improvement_cash_percent) {
                continue;
            }

            if !can_improve_property(game_state, ruleset, player_id, property_id as PropertyId) {
                continue;
            }

            let (development, group_index) = match self.building_allocation {
                BuildingAllocation::Spread => (0, 0),
                BuildingAllocation::Concentrate => {
                    let group = OWNERSHIP_GROUP_BY_TILE_ID[tile_id as usize].expect("property tiles should have an ownership group") as usize;
                    (group_development[group], group)
                },
            };
            let priority = (std::cmp::Reverse(development), group_index, improvement_level);
            if selected_priority.is_none_or(|previous| priority < previous) {
                selected_priority = Some(priority);
                selected_property = Some(property_id as PropertyId);
            }
        }

        selected_property
    }

    fn choose_tile_to_unmortgage<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TileId> {
        let mut mortgaged_owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize] & game_state.board.mortgaged_tiles;
        let mut cheapest_unmortgage: Option<(Cash, TileId)> = None;

        while mortgaged_owned_tiles != 0 {
            let tile_id = mortgaged_owned_tiles.trailing_zeros() as TileId;
            mortgaged_owned_tiles &= mortgaged_owned_tiles - 1;

            let unmortgage_price = UNMORTGAGE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
            if game_state.cash_by_player_id[player_id as usize] < self.calculate_required_cash(unmortgage_price, self.unmortgage_cash_percent) {
                continue;
            }

            if cheapest_unmortgage.is_none_or(|(cheapest_unmortgage_price, _)| unmortgage_price < cheapest_unmortgage_price) {
                cheapest_unmortgage = Some((unmortgage_price, tile_id));
            }
        }

        cheapest_unmortgage.map(|(_, tile_id)| tile_id)
    }

    fn propose_trade<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        player_id: PlayerId,
    ) -> Option<TradeOffer> {
        if self.trade_offer_percent == 0 {
            return None;
        }

        let owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize];

        for ownership_group_index in 0..OwnershipGroup::COUNT {
            let group_tiles = TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group_index];
            let owned_group_tiles = owned_tiles & group_tiles;
            let missing_group_tiles = group_tiles & !owned_group_tiles;
            if owned_group_tiles == 0 || missing_group_tiles.count_ones() != 1 {
                continue;
            }

            let missing_tile_id = missing_group_tiles.trailing_zeros() as TileId;
            let Some(owner_player_id) = game_state.board.get_tile_owner(missing_tile_id) else {
                continue;
            };

            let offered_cash = PURCHASE_PRICE_BY_TILE_ID[missing_tile_id as usize] as Cash * self.trade_offer_percent / PERCENT_DIVISOR;
            if game_state.cash_by_player_id[player_id as usize] < offered_cash + self.cash_reserve {
                continue;
            }

            return Some(TradeOffer {
                proposer_player_id: player_id,
                recipient_player_id: owner_player_id,
                offered_cash,
                offered_tiles: 0,
                offered_get_out_of_jail_free_cards: 0,
                requested_cash: 0,
                requested_tiles: 1 << missing_tile_id,
                requested_get_out_of_jail_free_cards: 0,
            });
        }

        None
    }

    fn should_accept_trade<const PLAYER_COUNT: usize>(
        &mut self,
        _game_state: &GameState<PLAYER_COUNT>,
        _ruleset: &Ruleset,
        _player_id: PlayerId,
        trade_offer: &TradeOffer,
    ) -> bool {
        let received_value = trade_offer.offered_cash + calculate_tile_set_purchase_value(trade_offer.offered_tiles);
        let given_value = trade_offer.requested_cash + calculate_tile_set_purchase_value(trade_offer.requested_tiles);

        received_value * PERCENT_DIVISOR > given_value * self.trade_accept_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::engine::action::validate_jail_action;
    use crate::game::engine::movement::send_player_to_jail;
    use crate::game::ruleset::data::DEX_RULESET;

    fn building_state(rules: &Ruleset) -> GameState<2> {
        let mut state = GameState::<2>::create_starting_state(rules, 0);
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3) | (1 << 6) | (1 << 8) | (1 << 9);
        state.cash_by_player_id[0] = 10_000;
        state
    }

    #[test]
    fn building_allocation_changes_group_priority() {
        for rules in [Ruleset::default(), DEX_RULESET] {
            let mut state = building_state(&rules);
            state.board.improvement_level_by_property_id[2..5].fill(2);
            state.board.bank_house_count -= 6;
            let mut strategy = ConfigurableStrategy::new();
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(0));
            strategy.building_allocation = BuildingAllocation::Concentrate;
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(2));
            state.board.mortgaged_tiles = 1 << 6;
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(0));
        }
    }

    #[test]
    fn all_building_policies_obey_engine_rules_and_their_ceiling() {
        use crate::game::engine::action::{
            ManagementAction,
            ManagementPhase,
            execute_management_action,
        };
        for rules in [Ruleset::default(), DEX_RULESET] {
            for building_allocation in [BuildingAllocation::Spread, BuildingAllocation::Concentrate] {
                for development_ceiling in [DevelopmentCeiling::ThreeHouses, DevelopmentCeiling::FourHouses, DevelopmentCeiling::Hotel] {
                    let mut strategy = ConfigurableStrategy {
                        building_allocation,
                        development_ceiling,
                        ..ConfigurableStrategy::new()
                    };
                    let mut state = building_state(&rules);
                    let ceiling = development_ceiling.improvement_level();
                    for _ in 0..5 * ceiling {
                        let property = strategy.choose_property_to_improve(&state, &rules, 0).unwrap();
                        assert!(state.board.improvement_level_by_property_id[property as usize] < ceiling);
                        execute_management_action(&mut state, &rules, 0, ManagementPhase::Building, ManagementAction::Build(property)).unwrap();
                    }
                    assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), None);
                    assert_eq!(&state.board.improvement_level_by_property_id[..5], &[ceiling; 5]);
                    assert_eq!(state.cash_by_player_id[0], 10_000 - 250 * ceiling as Cash);
                    assert_eq!(state.board.bank_house_count, if ceiling == HOTEL_IMPROVEMENT_LEVEL { 32 } else { 32 - 5 * ceiling });
                    assert_eq!(state.board.bank_hotel_count, if ceiling == HOTEL_IMPROVEMENT_LEVEL { 7 } else { 12 });
                }
            }
        }
    }

    #[test]
    fn concentration_moves_on_when_preferred_group_reaches_ceiling() {
        let rules = Ruleset::default();
        let mut state = building_state(&rules);
        state.board.improvement_level_by_property_id[2..5].fill(3);
        state.board.bank_house_count -= 9;
        let mut strategy = ConfigurableStrategy {
            building_allocation: BuildingAllocation::Concentrate,
            development_ceiling: DevelopmentCeiling::ThreeHouses,
            ..ConfigurableStrategy::new()
        };
        assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(0));
        strategy.development_ceiling = DevelopmentCeiling::FourHouses;
        assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(2));
    }

    #[test]
    fn building_policies_respect_cash_and_bank_supply() {
        let rules = Ruleset::default();
        for building_allocation in [BuildingAllocation::Spread, BuildingAllocation::Concentrate] {
            let mut strategy = ConfigurableStrategy {
                building_allocation,
                ..ConfigurableStrategy::new()
            };
            let mut state = building_state(&rules);
            state.cash_by_player_id[0] = strategy.cash_reserve + 49;
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), None);
            state.cash_by_player_id[0] += 1;
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), Some(0));
            state.board.bank_house_count = 0;
            assert_eq!(strategy.choose_property_to_improve(&state, &rules, 0), None);
        }
    }

    #[test]
    fn building_settings_default_for_old_json_and_round_trip_all_combinations() {
        let mut value = serde_json::to_value(ConfigurableStrategy::new()).unwrap();
        value.as_object_mut().unwrap().remove("building_allocation");
        value.as_object_mut().unwrap().remove("development_ceiling");
        assert_eq!(serde_json::from_value::<ConfigurableStrategy>(value.clone()).unwrap(), ConfigurableStrategy::new());
        for building_allocation in [BuildingAllocation::Spread, BuildingAllocation::Concentrate] {
            for development_ceiling in [DevelopmentCeiling::ThreeHouses, DevelopmentCeiling::FourHouses, DevelopmentCeiling::Hotel] {
                let strategy = ConfigurableStrategy {
                    building_allocation,
                    development_ceiling,
                    ..ConfigurableStrategy::new()
                };
                assert_eq!(serde_json::from_value::<ConfigurableStrategy>(serde_json::to_value(strategy).unwrap()).unwrap(), strategy);
            }
        }
        value["development_ceiling"] = serde_json::json!("SixHouses");
        assert!(serde_json::from_value::<ConfigurableStrategy>(value).is_err());
    }

    #[test]
    fn adaptive_jail_threshold_is_inclusive_and_counts_all_players() {
        for rules in [Ruleset::default(), DEX_RULESET] {
            for threshold in [0, 4, OWNABLE_TILE_SET_MASK.count_ones() as u8, u8::MAX] {
                let mut strategy = ConfigurableStrategy {
                    jail_camping_unowned_tile_threshold: Some(threshold),
                    ..ConfigurableStrategy::new()
                };
                let mut state = GameState::<2>::create_starting_state(&rules, 0);
                send_player_to_jail(&mut state, 0);
                for unowned_tile_count in 0..=OWNABLE_TILE_SET_MASK.count_ones() {
                    let mut owned_tiles = OWNABLE_TILE_SET_MASK;
                    for _ in 0..unowned_tile_count {
                        owned_tiles &= owned_tiles - 1;
                    }
                    state.board.owned_tiles_by_player_id = [owned_tiles & 0x5555555555, owned_tiles & 0xaaaaaaaaaa];
                    let expected = if unowned_tile_count <= u32::from(threshold) {
                        JailAction::RollForDoubles
                    } else {
                        JailAction::PayBail
                    };
                    let action = strategy.choose_jail_action(&state, &rules, 0);
                    assert_eq!(action, expected);
                    assert_eq!(validate_jail_action(&state, &rules, 0, action), Ok(()));
                }
            }
        }
    }

    #[test]
    fn adaptive_jail_policy_preserves_cards_while_camping() {
        let rules = Ruleset::default();
        let mut strategy = ConfigurableStrategy {
            jail_camping_unowned_tile_threshold: Some(4),
            ..ConfigurableStrategy::new()
        };
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        send_player_to_jail(&mut state, 0);
        for deck in 0..state.get_out_of_jail_free_card_holder_by_deck_kind.len() {
            state.get_out_of_jail_free_card_holder_by_deck_kind = [None; 2];
            state.get_out_of_jail_free_card_holder_by_deck_kind[deck] = Some(0);
            state.board.owned_tiles_by_player_id = [0; 2];
            assert_eq!(strategy.choose_jail_action(&state, &rules, 0), JailAction::UseGetOutOfJailFreeCard);
            state.board.owned_tiles_by_player_id[1] = OWNABLE_TILE_SET_MASK;
            let original = state;
            let action = strategy.choose_jail_action(&state, &rules, 0);
            assert_eq!(action, JailAction::RollForDoubles);
            assert_eq!(validate_jail_action(&state, &rules, 0, action), Ok(()));
            assert_eq!(state, original);
        }
    }

    #[test]
    fn jail_bail_respects_reserves_without_overflow() {
        for rules in [Ruleset::default(), DEX_RULESET] {
            for threshold in [None, Some(4)] {
                let mut strategy = ConfigurableStrategy {
                    pays_bail_when_affordable: true,
                    jail_camping_unowned_tile_threshold: threshold,
                    ..ConfigurableStrategy::new()
                };
                let mut state = GameState::<2>::create_starting_state(&rules, 0);
                send_player_to_jail(&mut state, 0);
                state.cash_by_player_id[0] = strategy.cash_reserve + rules.jail_bail_amount as Cash;
                assert_eq!(strategy.choose_jail_action(&state, &rules, 0), JailAction::PayBail);
                state.cash_by_player_id[0] -= 1;
                assert_eq!(strategy.choose_jail_action(&state, &rules, 0), JailAction::RollForDoubles);
                strategy.cash_reserve = Cash::MAX;
                state.cash_by_player_id[0] = Cash::MAX;
                assert_eq!(strategy.choose_jail_action(&state, &rules, 0), JailAction::RollForDoubles);
            }
        }
    }

    #[test]
    fn disabled_adaptive_policy_preserves_existing_jail_choices() {
        let rules = Ruleset::default();
        let mut state = GameState::<2>::create_starting_state(&rules, 0);
        send_player_to_jail(&mut state, 0);
        state.board.owned_tiles_by_player_id[1] = OWNABLE_TILE_SET_MASK;
        for pays_bail_when_affordable in [false, true] {
            let mut strategy = ConfigurableStrategy {
                pays_bail_when_affordable,
                ..ConfigurableStrategy::new()
            };
            state.get_out_of_jail_free_card_holder_by_deck_kind = [None; 2];
            let expected = if pays_bail_when_affordable { JailAction::PayBail } else { JailAction::RollForDoubles };
            assert_eq!(strategy.choose_jail_action(&state, &rules, 0), expected);
            state.get_out_of_jail_free_card_holder_by_deck_kind[0] = Some(0);
            assert_eq!(strategy.choose_jail_action(&state, &rules, 0), JailAction::UseGetOutOfJailFreeCard);
        }
    }

    #[test]
    fn jail_policy_json_preserves_old_inputs_and_round_trips() {
        let mut value = serde_json::to_value(ConfigurableStrategy::new()).unwrap();
        value.as_object_mut().unwrap().remove("jail_camping_unowned_tile_threshold");
        let strategy: ConfigurableStrategy = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(strategy.jail_camping_unowned_tile_threshold, None);
        value["camps_in_jail_below_unowned_tile_count"] = serde_json::json!(4);
        let strategy: ConfigurableStrategy = serde_json::from_value(value).unwrap();
        assert_eq!(strategy.jail_camping_unowned_tile_threshold, Some(4));
        let serialized = serde_json::to_value(strategy).unwrap();
        assert!(serialized.get("camps_in_jail_below_unowned_tile_count").is_none());
        assert_eq!(serde_json::from_value::<ConfigurableStrategy>(serialized).unwrap(), strategy);
    }
}
