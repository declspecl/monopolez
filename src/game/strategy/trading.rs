use super::configurable::{
    ConfigurableStrategy,
    PERCENT_DIVISOR,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::trade::execute_trade;
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
        let budget = PURCHASE_PRICE_BY_TILE_ID[target as usize] as u64 * strategy.trade_offer_percent as u64 / PERCENT_DIVISOR as u64;
        for offered_group in TILE_SET_MASK_BY_OWNERSHIP_GROUP {
            if target_group == offered_group || owned & offered_group == offered_group {
                continue;
            }
            let needed = offered_group & !recipient_owned;
            if recipient_owned & offered_group == 0 || needed.count_ones() != 1 || owned & needed == 0 {
                continue;
            }
            let offered = needed.trailing_zeros() as usize;
            let offered_value = PURCHASE_PRICE_BY_TILE_ID[offered] as u64;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::ruleset::model::PermittedBarterTacticsMask;
    use crate::game::strategy::model::PlayerStrategy;

    fn fixture() -> (GameState<2>, ConfigurableStrategy) {
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        state.board.owned_tiles_by_player_id = [(1 << 1) | (1 << 5), (1 << 3) | (1 << 15) | (1 << 25) | (1 << 35)];
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
        assert_eq!(offer.offered_tiles, 1 << 5);
        assert_eq!(offer.requested_tiles, 1 << 3);
        assert_eq!(offer.offered_cash, 40);
        assert!(ConfigurableStrategy::new().should_accept_trade(&state, &DEX_RULESET, 1, &offer));
        assert!(execute_trade(&mut state, &DEX_RULESET, &offer));
        assert_eq!(state.board.owned_tiles_by_player_id[0], (1 << 1) | (1 << 3));
        assert_eq!(state.board.owned_tiles_by_player_id[1], (1 << 5) | (1 << 15) | (1 << 25) | (1 << 35));
        strategy.offers_property_swaps = false;
        let cash_offer = strategy.propose_trade(&before, &DEX_RULESET, 0).unwrap();
        assert_eq!(cash_offer.offered_tiles, 0);
        assert_eq!(cash_offer.offered_cash, 240);
    }

    #[test]
    fn swaps_obey_reserves_interest_and_engine_restrictions() {
        let (mut state, strategy) = fixture();
        state.cash_by_player_id[0] = 90;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_some());
        state.cash_by_player_id[0] = 89;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
        state.cash_by_player_id[0] = 90;
        state.board.mortgaged_tiles = 1 << 3;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
        state.cash_by_player_id[0] = 93;
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_some());
        let mut rules = DEX_RULESET;
        rules.permitted_barter_tactics = PermittedBarterTacticsMask::MONEY;
        assert!(propose_monopoly_swap(&strategy, &state, &rules, 0).is_none());
        state.board.improvement_level_by_property_id[0] = 1;
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
            offered_tiles: 1 << 3,
            offered_get_out_of_jail_free_cards: 0,
            requested_cash: 100,
            requested_tiles: 0,
            requested_get_out_of_jail_free_cards: 0,
        };
        assert!(!strategy.should_accept_trade(&state, &DEX_RULESET, 0, &offer));
        strategy.monopoly_trade_premium_percent = 100;
        assert!(strategy.should_accept_trade(&state, &DEX_RULESET, 0, &offer));
        state.board.owned_tiles_by_player_id[0] |= 1 << 3;
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
        state.board.owned_tiles_by_player_id[1] &= !(1 << 15);
        assert!(propose_monopoly_swap(&strategy, &state, &DEX_RULESET, 0).is_none());
    }
}
