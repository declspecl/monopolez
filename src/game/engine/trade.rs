use super::mortgage::{
    calculate_mortgage_transfer_interest,
    has_group_improvements,
};
use crate::game::board::model::PlayerId;
use crate::game::card::model::DeckKind;
use crate::game::ruleset::model::{
    PermittedBarterTacticsMask,
    PermittedBarterTimesMask,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::model::{
    Cash,
    TileSetMask,
};
use crate::game::trade::model::{
    TradeOffer,
    deck_kind_bit,
};

pub fn run_trade_phase<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    barter_time: PermittedBarterTimesMask,
) -> bool {
    if !ruleset.permitted_barter_times.contains(barter_time) {
        return false;
    }

    let Some(trade_offer) = strategies[player_id as usize].propose_trade(game_state, ruleset, player_id) else {
        return false;
    };

    if trade_offer.proposer_player_id != player_id || !is_trade_permitted(game_state, ruleset, &trade_offer) {
        return false;
    }
    super::event::publish_event(strategies, player_id as usize, super::event::GameEvent::TradeProposed { offer: trade_offer });

    let recipient_index = trade_offer.recipient_player_id as usize;
    if !strategies[recipient_index].should_accept_trade(game_state, ruleset, trade_offer.recipient_player_id, &trade_offer) {
        super::event::publish_event(strategies, player_id as usize, super::event::GameEvent::TradeRejected { offer: trade_offer });
        let Some(counteroffer) = strategies[recipient_index].counter_trade_offer(game_state, ruleset, trade_offer.recipient_player_id, &trade_offer) else {
            return false;
        };
        if counteroffer.proposer_player_id != trade_offer.recipient_player_id
            || counteroffer.recipient_player_id != trade_offer.proposer_player_id
            || !is_trade_permitted(game_state, ruleset, &counteroffer)
        {
            return false;
        }
        super::event::publish_event(strategies, recipient_index, super::event::GameEvent::TradeProposed { offer: counteroffer });
        if !strategies[player_id as usize].should_accept_counteroffer(game_state, ruleset, player_id, &trade_offer, &counteroffer) {
            super::event::publish_event(strategies, recipient_index, super::event::GameEvent::TradeRejected { offer: counteroffer });
            return false;
        }
        if !execute_trade(game_state, ruleset, &counteroffer) {
            return false;
        }
        super::event::publish_event(strategies, recipient_index, super::event::GameEvent::TradeExecuted { offer: counteroffer });
        return true;
    }

    if !execute_trade(game_state, ruleset, &trade_offer) {
        return false;
    }
    super::event::publish_event(strategies, player_id as usize, super::event::GameEvent::TradeExecuted { offer: trade_offer });

    true
}

pub fn is_trade_permitted<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    trade_offer: &TradeOffer,
) -> bool {
    let proposer_index = trade_offer.proposer_player_id as usize;
    let recipient_index = trade_offer.recipient_player_id as usize;

    if proposer_index == recipient_index || proposer_index >= PLAYER_COUNT || recipient_index >= PLAYER_COUNT {
        return false;
    }
    if trade_offer.traded_tiles() & !crate::game::tile::lut::OWNABLE_TILE_SET_MASK != 0 || trade_offer.traded_get_out_of_jail_free_cards() & !((1 << DeckKind::COUNT) - 1) != 0 {
        return false;
    }

    let traded_players = 1 << trade_offer.proposer_player_id | 1 << trade_offer.recipient_player_id;
    if game_state.bankrupt_players & traded_players != 0 {
        return false;
    }

    let owns_offered_tiles = game_state.board.owned_tiles_by_player_id[proposer_index] & trade_offer.offered_tiles == trade_offer.offered_tiles;
    let owns_requested_tiles = game_state.board.owned_tiles_by_player_id[recipient_index] & trade_offer.requested_tiles == trade_offer.requested_tiles;
    if !owns_offered_tiles || !owns_requested_tiles {
        return false;
    }

    if game_state.cash_by_player_id[proposer_index] < trade_offer.offered_cash || game_state.cash_by_player_id[recipient_index] < trade_offer.requested_cash {
        return false;
    }
    if trade_balances(game_state, trade_offer).is_none() {
        return false;
    }

    if !holds_get_out_of_jail_free_cards(game_state, trade_offer.proposer_player_id, trade_offer.offered_get_out_of_jail_free_cards)
        || !holds_get_out_of_jail_free_cards(game_state, trade_offer.recipient_player_id, trade_offer.requested_get_out_of_jail_free_cards)
    {
        return false;
    }

    let mut traded_tiles = trade_offer.traded_tiles();
    while traded_tiles != 0 {
        let tile_id = traded_tiles.trailing_zeros() as u8;
        traded_tiles &= traded_tiles - 1;

        if has_group_improvements(game_state, tile_id) {
            return false;
        }
    }

    are_tactics_permitted(game_state, ruleset, trade_offer)
}

pub fn execute_trade<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    trade_offer: &TradeOffer,
) -> bool {
    if !is_trade_permitted(game_state, ruleset, trade_offer) {
        return false;
    }
    let proposer_index = trade_offer.proposer_player_id as usize;
    let recipient_index = trade_offer.recipient_player_id as usize;

    let Some((proposer_cash, recipient_cash)) = trade_balances(game_state, trade_offer) else {
        return false;
    };

    game_state.board.owned_tiles_by_player_id[proposer_index] &= !trade_offer.offered_tiles;
    game_state.board.owned_tiles_by_player_id[proposer_index] |= trade_offer.requested_tiles;
    game_state.board.owned_tiles_by_player_id[recipient_index] &= !trade_offer.requested_tiles;
    game_state.board.owned_tiles_by_player_id[recipient_index] |= trade_offer.offered_tiles;

    game_state.cash_by_player_id[proposer_index] = proposer_cash;
    game_state.cash_by_player_id[recipient_index] = recipient_cash;

    transfer_get_out_of_jail_free_cards(game_state, trade_offer.offered_get_out_of_jail_free_cards, trade_offer.recipient_player_id);
    transfer_get_out_of_jail_free_cards(game_state, trade_offer.requested_get_out_of_jail_free_cards, trade_offer.proposer_player_id);
    true
}

fn trade_balances<const N: usize>(
    state: &GameState<N>,
    offer: &TradeOffer,
) -> Option<(Cash, Cash)> {
    let proposer_cash = state.cash_by_player_id[offer.proposer_player_id as usize]
        .checked_sub(offer.offered_cash)?
        .checked_add(offer.requested_cash)?
        .checked_sub(calculate_mortgage_transfer_interest(state, offer.requested_tiles))?;
    let recipient_cash = state.cash_by_player_id[offer.recipient_player_id as usize]
        .checked_sub(offer.requested_cash)?
        .checked_add(offer.offered_cash)?
        .checked_sub(calculate_mortgage_transfer_interest(state, offer.offered_tiles))?;
    Some((proposer_cash, recipient_cash))
}

fn are_tactics_permitted<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    trade_offer: &TradeOffer,
) -> bool {
    let permitted_tactics = ruleset.permitted_barter_tactics;

    if (trade_offer.offered_cash > 0 || trade_offer.requested_cash > 0) && !permitted_tactics.contains(PermittedBarterTacticsMask::MONEY) {
        return false;
    }

    let traded_tiles = trade_offer.traded_tiles();
    let mortgaged_traded_tiles = traded_tiles & game_state.board.mortgaged_tiles;
    let unmortgaged_traded_tiles = traded_tiles & !game_state.board.mortgaged_tiles;

    if unmortgaged_traded_tiles != 0 && !permitted_tactics.contains(PermittedBarterTacticsMask::UNMORTGAGED_PROPERTIES) {
        return false;
    }

    if mortgaged_traded_tiles != 0 && !permitted_tactics.contains(PermittedBarterTacticsMask::MORTGAGED_PROPERTIES) {
        return false;
    }

    trade_offer.traded_get_out_of_jail_free_cards() == 0 || permitted_tactics.contains(PermittedBarterTacticsMask::GET_OUT_OF_JAIL_FREE_CARDS)
}

fn holds_get_out_of_jail_free_cards<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    traded_cards: u8,
) -> bool {
    for deck_kind in [DeckKind::Chance, DeckKind::CommunityChest] {
        let is_traded = traded_cards & deck_kind_bit(deck_kind) != 0;
        if is_traded && game_state.get_out_of_jail_free_card_holder_by_deck_kind[deck_kind as usize] != Some(player_id) {
            return false;
        }
    }

    true
}

fn transfer_get_out_of_jail_free_cards<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    traded_cards: u8,
    new_holder_player_id: PlayerId,
) {
    for deck_kind in [DeckKind::Chance, DeckKind::CommunityChest] {
        if traded_cards & deck_kind_bit(deck_kind) != 0 {
            game_state.get_out_of_jail_free_card_holder_by_deck_kind[deck_kind as usize] = Some(new_holder_player_id);
        }
    }
}

pub fn calculate_tile_set_purchase_value(tiles: TileSetMask) -> Cash {
    use crate::game::tile::lut::PURCHASE_PRICE_BY_TILE_ID;

    let mut remaining_tiles = tiles;
    let mut purchase_value = 0;

    while remaining_tiles != 0 {
        let tile_index = remaining_tiles.trailing_zeros() as usize;
        remaining_tiles &= remaining_tiles - 1;

        purchase_value += PURCHASE_PRICE_BY_TILE_ID[tile_index] as Cash;
    }

    purchase_value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::strategy::configurable::ConfigurableStrategy;

    const PARK_PLACE_TILE_ID: u8 = 37;
    const BOARDWALK_TILE_ID: u8 = 39;
    const MEDITERRANEAN_AVENUE_TILE_ID: u8 = 1;

    fn create_trade_state(ruleset: &Ruleset) -> GameState<2> {
        let mut game_state = GameState::<2>::create_starting_state(ruleset, 1);
        game_state.board.owned_tiles_by_player_id[0] = 1 << PARK_PLACE_TILE_ID;
        game_state.board.owned_tiles_by_player_id[1] = 1 << BOARDWALK_TILE_ID;

        game_state
    }

    fn create_trade_offer() -> TradeOffer {
        TradeOffer {
            proposer_player_id: 0,
            recipient_player_id: 1,
            offered_cash: 100,
            offered_tiles: 1 << PARK_PLACE_TILE_ID,
            offered_get_out_of_jail_free_cards: 0,
            requested_cash: 0,
            requested_tiles: 1 << BOARDWALK_TILE_ID,
            requested_get_out_of_jail_free_cards: 0,
        }
    }

    #[test]
    fn executes_a_permitted_trade() {
        let ruleset = Ruleset::default();
        let mut game_state = create_trade_state(&ruleset);
        let trade_offer = create_trade_offer();

        assert!(is_trade_permitted(&game_state, &ruleset, &trade_offer));
        assert!(execute_trade(&mut game_state, &ruleset, &trade_offer));

        assert_eq!(game_state.board.owned_tiles_by_player_id[0], 1 << BOARDWALK_TILE_ID);
        assert_eq!(game_state.board.owned_tiles_by_player_id[1], 1 << PARK_PLACE_TILE_ID);
        assert_eq!(game_state.cash_by_player_id[0], 1400);
        assert_eq!(game_state.cash_by_player_id[1], 1600);
    }

    #[test]
    fn refuses_trade_of_tiles_the_players_do_not_own() {
        let ruleset = Ruleset::default();
        let game_state = create_trade_state(&ruleset);
        let mut trade_offer = create_trade_offer();
        trade_offer.offered_tiles = 1 << MEDITERRANEAN_AVENUE_TILE_ID;

        assert!(!is_trade_permitted(&game_state, &ruleset, &trade_offer));
    }

    #[test]
    fn refuses_trade_of_improved_ownership_group() {
        let ruleset = Ruleset::default();
        let mut game_state = create_trade_state(&ruleset);
        game_state.board.improvement_level_by_property_id[20] = 1;

        assert!(!is_trade_permitted(&game_state, &ruleset, &create_trade_offer()));
    }

    #[test]
    fn refuses_tactics_the_ruleset_forbids() {
        let ruleset = Ruleset::builder().with_permitted_barter_tactics(PermittedBarterTacticsMask::MONEY).build();
        let game_state = create_trade_state(&ruleset);

        assert!(!is_trade_permitted(&game_state, &ruleset, &create_trade_offer()));
    }

    #[test]
    fn charges_interest_on_traded_mortgages() {
        let ruleset = Ruleset::default();
        let mut game_state = create_trade_state(&ruleset);
        game_state.board.mortgaged_tiles = 1 << PARK_PLACE_TILE_ID;

        let mut trade_offer = create_trade_offer();
        trade_offer.offered_cash = 0;
        assert!(execute_trade(&mut game_state, &ruleset, &trade_offer));

        assert_eq!(game_state.cash_by_player_id[1], 1500 - 18);
    }

    #[test]
    fn rejects_invalid_trade_execution_without_mutation() {
        let ruleset = Ruleset::default();
        let original = create_trade_state(&ruleset);
        let offer = create_trade_offer();
        let invalid = [
            TradeOffer { proposer_player_id: 255, ..offer },
            TradeOffer { recipient_player_id: 255, ..offer },
            TradeOffer { offered_tiles: 1 << 63, ..offer },
            TradeOffer {
                offered_get_out_of_jail_free_cards: 128,
                ..offer
            },
            TradeOffer { offered_cash: Cash::MAX, ..offer },
        ];
        for offer in invalid {
            let mut state = original;
            assert!(!execute_trade(&mut state, &ruleset, &offer));
            assert_eq!(state, original);
        }
    }

    #[test]
    fn rejects_unaffordable_transfer_interest_and_cash_overflow() {
        let ruleset = Ruleset::default();
        let mut state = create_trade_state(&ruleset);
        state.board.mortgaged_tiles = 1 << PARK_PLACE_TILE_ID;
        state.cash_by_player_id[1] = 0;
        let offer = TradeOffer {
            offered_cash: 0,
            ..create_trade_offer()
        };
        let original = state;
        assert!(!execute_trade(&mut state, &ruleset, &offer));
        assert_eq!(state, original);

        state.board.mortgaged_tiles = 0;
        state.cash_by_player_id[1] = Cash::MAX;
        let original = state;
        assert!(!execute_trade(&mut state, &ruleset, &create_trade_offer()));
        assert_eq!(state, original);
    }

    #[test]
    fn executes_one_counteroffer_when_the_original_proposer_accepts() {
        let rules = Ruleset::default();
        let mut state = create_trade_state(&rules);
        let mut strategies = [
            ConfigurableStrategy {
                trade_offer_percent: 150,
                trade_counteroffer_limit_percent: Some(250),
                ..ConfigurableStrategy::new()
            },
            ConfigurableStrategy {
                trade_accept_percent: 200,
                makes_trade_counteroffers: true,
                ..ConfigurableStrategy::new()
            },
        ];

        assert!(run_trade_phase(&mut state, &rules, &mut strategies, 0, PermittedBarterTimesMask::START_OF_TURN));
        assert_eq!(state.board.owned_tiles_by_player_id, [(1 << PARK_PLACE_TILE_ID) | (1 << BOARDWALK_TILE_ID), 0]);
        assert_eq!(state.cash_by_player_id, [699, 2301]);
    }

    #[test]
    fn declining_the_only_counteroffer_ends_the_trade() {
        let rules = Ruleset::default();
        let original = create_trade_state(&rules);
        let mut state = original;
        let mut strategies = [
            ConfigurableStrategy {
                trade_offer_percent: 150,
                trade_counteroffer_limit_percent: Some(200),
                ..ConfigurableStrategy::new()
            },
            ConfigurableStrategy {
                trade_accept_percent: 200,
                makes_trade_counteroffers: true,
                ..ConfigurableStrategy::new()
            },
        ];

        assert!(!run_trade_phase(&mut state, &rules, &mut strategies, 0, PermittedBarterTimesMask::START_OF_TURN));
        assert_eq!(state, original);
    }
}
