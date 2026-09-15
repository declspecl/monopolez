use super::configurable::{
    ConfigurableStrategy,
    PERCENT_DIVISOR,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::trade::{
    calculate_tile_set_purchase_value,
    execute_trade,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::tile::lut::{
    PURCHASE_PRICE_BY_TILE_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
};
use crate::game::tile::model::Cash;
use crate::game::trade::model::TradeOffer;

pub fn propose_monopoly_swap<const N: usize>(
    strategy: &ConfigurableStrategy,
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
) -> Option<TradeOffer> {
    let owned = state.board.owned_tiles_by_player_id[player as usize];
    for target_group in TILE_SET_MASK_BY_OWNERSHIP_GROUP {
        let missing = target_group & !owned;
        if owned & target_group == 0 || missing.count_ones() != 1 {
            continue;
        }
        let target = missing.trailing_zeros() as u8;
        let Some(recipient) = state.board.get_tile_owner(target) else {
            continue;
        };
        let recipient_owned = state.board.owned_tiles_by_player_id[recipient as usize];
        for offered_group in TILE_SET_MASK_BY_OWNERSHIP_GROUP {
            if target_group == offered_group || owned & offered_group == offered_group {
                continue;
            }
            let needed = offered_group & !recipient_owned;
            if recipient_owned & offered_group == 0 || needed.count_ones() != 1 || owned & needed == 0 {
                continue;
            }
            let target_group_value = calculate_tile_set_purchase_value(target_group) as u128;
            let offered_group_value = calculate_tile_set_purchase_value(offered_group) as u128;
            if target_group_value < offered_group_value {
                continue;
            }
            let offered = needed.trailing_zeros() as usize;
            let offered_value = PURCHASE_PRICE_BY_TILE_ID[offered] as u64;
            let offer_percent = strategy.property_swap_offer_percent.unwrap_or(strategy.trade_offer_percent);
            let budget = PURCHASE_PRICE_BY_TILE_ID[target as usize] as u64 * offer_percent as u64 / PERCENT_DIVISOR as u64;
            if offered_value > budget {
                continue;
            }
            let cash = budget - offered_value;
            if cash > Cash::MAX as u64 {
                continue;
            }
            let offer = TradeOffer {
                proposer_player_id: player,
                recipient_player_id: recipient,
                offered_cash: cash as Cash,
                offered_tiles: needed,
                offered_get_out_of_jail_free_cards: 0,
                requested_cash: 0,
                requested_tiles: missing,
                requested_get_out_of_jail_free_cards: 0,
            };
            let mut projected = *state;
            if execute_trade(&mut projected, rules, &offer) && projected.cash_by_player_id[player as usize] >= strategy.cash_reserve {
                return Some(offer);
            }
        }
    }
    None
}

pub(super) fn trade_values<const N: usize>(
    strategy: &ConfigurableStrategy,
    state: &GameState<N>,
    player: PlayerId,
    offer: &TradeOffer,
) -> (u128, u128) {
    let mut received_value = offer.offered_cash as u128 + calculate_tile_set_purchase_value(offer.offered_tiles) as u128;
    let mut given_value = offer.requested_cash as u128 + calculate_tile_set_purchase_value(offer.requested_tiles) as u128;
    if strategy.monopoly_trade_premium_percent > 0 {
        let owned = state.board.owned_tiles_by_player_id[player as usize];
        let after = (owned & !offer.requested_tiles) | offer.offered_tiles;
        for group in TILE_SET_MASK_BY_OWNERSHIP_GROUP {
            let premium = calculate_tile_set_purchase_value(group) as u128 * strategy.monopoly_trade_premium_percent as u128 / PERCENT_DIVISOR as u128;
            if after & group == group && owned & group != group {
                received_value += premium;
            }
            if owned & group == group && after & group != group {
                given_value += premium;
            }
        }
    }
    (received_value, given_value)
}

pub fn counter_trade_offer<const N: usize>(
    strategy: &ConfigurableStrategy,
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    rejected_offer: &TradeOffer,
) -> Option<TradeOffer> {
    if rejected_offer.recipient_player_id != player {
        return None;
    }
    let (received_value, given_value) = trade_values(strategy, state, player, rejected_offer);
    let required_value = given_value.checked_mul(strategy.trade_accept_percent as u128)?.checked_div(PERCENT_DIVISOR as u128)?.checked_add(1)?;
    let requested_cash = rejected_offer.offered_cash as u128 + required_value.saturating_sub(received_value);
    if requested_cash > Cash::MAX as u128 {
        return None;
    }
    let counteroffer = TradeOffer {
        proposer_player_id: player,
        recipient_player_id: rejected_offer.proposer_player_id,
        offered_cash: rejected_offer.requested_cash,
        offered_tiles: rejected_offer.requested_tiles,
        offered_get_out_of_jail_free_cards: rejected_offer.requested_get_out_of_jail_free_cards,
        requested_cash: requested_cash as Cash,
        requested_tiles: rejected_offer.offered_tiles,
        requested_get_out_of_jail_free_cards: rejected_offer.offered_get_out_of_jail_free_cards,
    };
    let mut projected = *state;
    (execute_trade(&mut projected, rules, &counteroffer) && projected.cash_by_player_id[player as usize] >= strategy.cash_reserve).then_some(counteroffer)
}

pub fn should_accept_counteroffer<const N: usize>(
    strategy: &ConfigurableStrategy,
    state: &GameState<N>,
    rules: &Ruleset,
    player: PlayerId,
    original_offer: &TradeOffer,
    counteroffer: &TradeOffer,
) -> bool {
    if original_offer.proposer_player_id != player
        || counteroffer.proposer_player_id != original_offer.recipient_player_id
        || counteroffer.recipient_player_id != player
        || counteroffer.offered_tiles != original_offer.requested_tiles
        || counteroffer.requested_tiles != original_offer.offered_tiles
        || counteroffer.offered_get_out_of_jail_free_cards != original_offer.requested_get_out_of_jail_free_cards
        || counteroffer.requested_get_out_of_jail_free_cards != original_offer.offered_get_out_of_jail_free_cards
    {
        return false;
    }
    let received_value = counteroffer.offered_cash as u128 + calculate_tile_set_purchase_value(counteroffer.offered_tiles) as u128;
    let given_value = counteroffer.requested_cash as u128 + calculate_tile_set_purchase_value(counteroffer.requested_tiles) as u128;
    let offer_percent = strategy.trade_counteroffer_limit_percent.unwrap_or_else(|| {
        strategy
            .property_swap_offer_percent
            .filter(|_| counteroffer.requested_tiles != 0)
            .unwrap_or(strategy.trade_offer_percent)
    });
    if given_value * PERCENT_DIVISOR as u128 > received_value * offer_percent as u128 {
        return false;
    }
    let mut projected = *state;
    execute_trade(&mut projected, rules, counteroffer) && projected.cash_by_player_id[player as usize] >= strategy.cash_reserve
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::ruleset::model::PermittedBarterTacticsMask;
    use crate::game::strategy::model::PlayerStrategy;

    fn fixture() -> (GameState<2>, ConfigurableStrategy) {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        state.board.owned_tiles_by_player_id = [(1 << 3) | (1 << 5) | (1 << 15) | (1 << 25), (1 << 1) | (1 << 35)];
        let strategy = ConfigurableStrategy {
            offers_property_swaps: true,
            trade_offer_percent: 400,
            cash_reserve: 50,
            ..ConfigurableStrategy::new()
        };
        (state, strategy)
    }

    #[test]
    fn swaps_complete_both_groups_without_changing_the_source_state() {
        let (mut state, mut strategy) = fixture();
        let before = state;
        let offer = strategy.propose_trade(&state, &DEX_RULESET, 0).unwrap();
        assert_eq!(state, before);
        assert_eq!(offer.offered_tiles, 1 << 3);
        assert_eq!(offer.requested_tiles, 1 << 35);
        assert_eq!(offer.offered_cash, 740);
        assert!(ConfigurableStrategy::new().should_accept_trade(&state, &DEX_RULESET, 1, &offer));
        assert!(execute_trade(&mut state, &DEX_RULESET, &offer));
        assert_eq!(state.board.owned_tiles_by_player_id[0], (1 << 5) | (1 << 15) | (1 << 25) | (1 << 35));
        assert_eq!(state.board.owned_tiles_by_player_id[1], (1 << 1) | (1 << 3));
        strategy.offers_property_swaps = false;
        let cash_offer = strategy.propose_trade(&before, &DEX_RULESET, 0).unwrap();
        assert_eq!(cash_offer.offered_tiles, 0);
        assert_eq!(cash_offer.offered_cash, 240);
    }

    #[test]
    fn swaps_obey_reserves_interest_and_engine_restrictions() {
        let (mut state, strategy) = fixture();
        state.cash_by_player_id[0] = 790;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_some());
        state.cash_by_player_id[0] = 789;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
        state.cash_by_player_id[0] = 790;
        state.board.mortgaged_tiles = 1 << 35;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
        state.cash_by_player_id[0] = 800;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_some());
        let mut rules = DEX_RULESET;
        rules.permitted_barter_tactics = PermittedBarterTacticsMask::MONEY;
        assert!(propose_monopoly_swap(&strategy, &state, &rules, 0).is_none());
        state.board.improvement_level_by_property_id[1] = 1;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
    }

    #[test]
    fn monopoly_premium_values_completion_and_protects_existing_groups() {
        let (mut state, mut strategy) = fixture();
        strategy.trade_accept_percent = 100;
        let offer = TradeOffer {
            proposer_player_id: 1,
            recipient_player_id: 0,
            offered_cash: 0,
            offered_tiles: 1 << 1,
            offered_get_out_of_jail_free_cards: 0,
            requested_cash: 100,
            requested_tiles: 0,
            requested_get_out_of_jail_free_cards: 0,
        };
        assert!(!strategy.should_accept_trade(&state, &DEX_RULESET, 0, &offer));
        strategy.monopoly_trade_premium_percent = 100;
        assert!(strategy.should_accept_trade(&state, &DEX_RULESET, 0, &offer));
        state.board.owned_tiles_by_player_id[0] |= 1 << 1;
        let sale = TradeOffer {
            offered_cash: 100,
            offered_tiles: 0,
            requested_cash: 0,
            requested_tiles: 1 << 3,
            ..offer
        };
        assert!(!strategy.should_accept_trade(&state, &DEX_RULESET, 0, &sale));
        strategy.monopoly_trade_premium_percent = 0;
        assert!(strategy.should_accept_trade(&state, &DEX_RULESET, 0, &sale));
        strategy.monopoly_trade_premium_percent = Cash::MAX;
        strategy.trade_accept_percent = Cash::MAX;
        assert!(!strategy.should_accept_trade(&state, &DEX_RULESET, 0, &sale));
    }

    #[test]
    fn swaps_skip_unaffordable_extreme_prices_and_absent_mutual_completions() {
        let (mut state, mut strategy) = fixture();
        strategy.trade_offer_percent = Cash::MAX;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
        strategy.trade_offer_percent = 400;
        state.board.owned_tiles_by_player_id[1] &= !(1 << 1);
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
    }

    #[test]
    fn counteroffers_raise_the_price_to_the_recipients_threshold() {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        state.board.owned_tiles_by_player_id = [1 << 37, 1 << 39];
        let offer = TradeOffer {
            proposer_player_id: 0,
            recipient_player_id: 1,
            offered_cash: 600,
            offered_tiles: 0,
            offered_get_out_of_jail_free_cards: 0,
            requested_cash: 0,
            requested_tiles: 1 << 39,
            requested_get_out_of_jail_free_cards: 0,
        };
        let recipient = ConfigurableStrategy {
            trade_accept_percent: 200,
            ..ConfigurableStrategy::new()
        };
        let counteroffer = counter_trade_offer(&recipient, &state, &DEX_RULESET, 1, &offer).unwrap();
        assert_eq!(counteroffer.offered_tiles, 1 << 39);
        assert_eq!(counteroffer.requested_cash, 801);

        let accepts = ConfigurableStrategy {
            trade_offer_percent: 150,
            trade_counteroffer_limit_percent: Some(250),
            ..ConfigurableStrategy::new()
        };
        assert!(should_accept_counteroffer(&accepts, &state, &DEX_RULESET, 0, &offer, &counteroffer));
        let declines = ConfigurableStrategy {
            trade_counteroffer_limit_percent: Some(200),
            ..accepts
        };
        assert!(!should_accept_counteroffer(&declines, &state, &DEX_RULESET, 0, &offer, &counteroffer));
    }
}
