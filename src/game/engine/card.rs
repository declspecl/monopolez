use super::landing::{
    RentModifier,
    resolve_landing,
};
use super::movement::{
    advance_player_to_tile,
    move_player_backward,
    send_player_to_jail,
};
use super::payment::{
    Creditor,
    charge_player,
    select_fee_creditor,
};
use crate::game::board::data::HOTEL_IMPROVEMENT_LEVEL;
use crate::game::board::model::PlayerId;
use crate::game::card::data::{
    CARD_COUNT_PER_DECK,
    CHANCE_CARD_DEFINITIONS,
    COMMUNITY_CHEST_CARD_DEFINITIONS,
};
use crate::game::card::model::{
    CardDefinition,
    CardEffect,
    DeckKind,
};
use crate::game::rng::DiceRoll;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    RAILROAD_TILE_SET_MASK,
    TILE_ID_BY_PROPERTY_ID,
    UTILITY_TILE_SET_MASK,
};
use crate::game::tile::model::{
    Cash,
    TileId,
    TileSetMask,
};

const CARD_DEFINITIONS_BY_DECK_KIND: [[CardDefinition; CARD_COUNT_PER_DECK]; DeckKind::COUNT] = [CHANCE_CARD_DEFINITIONS, COMMUNITY_CHEST_CARD_DEFINITIONS];

pub fn draw_and_apply_card<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    deck_kind: DeckKind,
    dice_roll: DiceRoll,
) {
    let card = draw_card(game_state, deck_kind);
    super::event::publish_event(
        strategies,
        player_id as usize,
        super::event::GameEvent::CardDrawn {
            player_id,
            deck: deck_kind,
            effect: card.effect,
        },
    );
    apply_card_effect(game_state, ruleset, strategies, player_id, deck_kind, card.effect, dice_roll);
}

fn draw_card<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    deck_kind: DeckKind,
) -> CardDefinition {
    let deck_index = deck_kind as usize;
    let is_get_out_of_jail_free_card_held = game_state.get_out_of_jail_free_card_holder_by_deck_kind[deck_index].is_some();

    loop {
        let deck_state = &mut game_state.deck_state_by_deck_kind[deck_index];
        let card_id = deck_state.card_id_by_draw_position[deck_state.next_draw_position as usize];

        deck_state.next_draw_position = (deck_state.next_draw_position + 1) % CARD_COUNT_PER_DECK as u8;

        let card_definition = CARD_DEFINITIONS_BY_DECK_KIND[deck_index][card_id as usize];
        if !(is_get_out_of_jail_free_card_held && card_definition.effect == CardEffect::GetOutOfJailFree) {
            return card_definition;
        }
    }
}

fn apply_card_effect<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    deck_kind: DeckKind,
    card_effect: CardEffect,
    dice_roll: DiceRoll,
) {
    let player_index = player_id as usize;

    match card_effect {
        CardEffect::AdvanceToTile { tile_id } => {
            if let Some(event) = advance_player_to_tile(game_state, ruleset, player_id, tile_id) {
                super::event::publish_event(strategies, player_index, event);
            }
            resolve_landing(game_state, ruleset, strategies, player_id, dice_roll, RentModifier::Standard);
        },
        CardEffect::AdvanceToNearestRailroad => {
            let railroad_tile_id = find_next_tile_in_set(game_state.position_by_player_id[player_index], RAILROAD_TILE_SET_MASK);
            if let Some(event) = advance_player_to_tile(game_state, ruleset, player_id, railroad_tile_id) {
                super::event::publish_event(strategies, player_index, event);
            }
            resolve_landing(game_state, ruleset, strategies, player_id, dice_roll, RentModifier::NearestRailroadCard);
        },
        CardEffect::AdvanceToNearestUtility => {
            let utility_tile_id = find_next_tile_in_set(game_state.position_by_player_id[player_index], UTILITY_TILE_SET_MASK);
            if let Some(event) = advance_player_to_tile(game_state, ruleset, player_id, utility_tile_id) {
                super::event::publish_event(strategies, player_index, event);
            }

            let fresh_dice_roll = game_state.rng.roll_dice();
            super::event::publish_event(
                strategies,
                player_index,
                super::event::GameEvent::DiceRolled {
                    player_id,
                    first: fresh_dice_roll.first_die,
                    second: fresh_dice_roll.second_die,
                },
            );
            resolve_landing(game_state, ruleset, strategies, player_id, fresh_dice_roll, RentModifier::NearestUtilityCard);
        },
        CardEffect::MoveBackward { tile_count } => {
            move_player_backward(game_state, player_id, tile_count);
            resolve_landing(game_state, ruleset, strategies, player_id, dice_roll, RentModifier::Standard);
        },
        CardEffect::GoToJail => {
            let event = send_player_to_jail(game_state, player_id);
            super::event::publish_event(strategies, player_index, event);
        },
        CardEffect::GetOutOfJailFree => {
            game_state.get_out_of_jail_free_card_holder_by_deck_kind[deck_kind as usize] = Some(player_id);
        },
        CardEffect::CollectFromBank { amount } => {
            game_state.cash_by_player_id[player_index] += amount as Cash;
            if amount > 0 {
                super::event::publish_event(
                    strategies,
                    player_index,
                    super::event::GameEvent::BankRewardCollected {
                        player_id,
                        amount: amount as Cash,
                        deck: deck_kind,
                    },
                );
            }
        },
        CardEffect::PayBank { amount } => charge_player(game_state, ruleset, strategies, player_id, amount as Cash, select_fee_creditor(ruleset)),
        CardEffect::CollectFromEachPlayer { amount } => {
            for other_player_id in 0..PLAYER_COUNT as PlayerId {
                if other_player_id != player_id && game_state.bankrupt_players & (1 << other_player_id) == 0 {
                    charge_player(game_state, ruleset, strategies, other_player_id, amount as Cash, Creditor::Player(player_id));
                }
            }
        },
        CardEffect::PayEachPlayer { amount } => {
            for other_player_id in 0..PLAYER_COUNT as PlayerId {
                if game_state.bankrupt_players & (1 << player_id) != 0 {
                    break;
                }

                if other_player_id != player_id && game_state.bankrupt_players & (1 << other_player_id) == 0 {
                    charge_player(game_state, ruleset, strategies, player_id, amount as Cash, Creditor::Player(other_player_id));
                }
            }
        },
        CardEffect::PayForRepairs { amount_per_house, amount_per_hotel } => {
            let (house_count, hotel_count) = count_buildings(game_state, player_id);
            let repair_cost = house_count * amount_per_house as Cash + hotel_count * amount_per_hotel as Cash;

            if repair_cost > 0 {
                charge_player(game_state, ruleset, strategies, player_id, repair_cost, select_fee_creditor(ruleset));
            }
        },
    }
}

fn find_next_tile_in_set(
    current_tile_id: TileId,
    tiles: TileSetMask,
) -> TileId {
    let tiles_ahead = tiles & !((1 << (current_tile_id + 1)) - 1);
    let next_tile_id = if tiles_ahead != 0 { tiles_ahead.trailing_zeros() } else { tiles.trailing_zeros() };

    next_tile_id as TileId
}

fn count_buildings<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    player_id: PlayerId,
) -> (Cash, Cash) {
    let owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize];

    let mut house_count = 0;
    let mut hotel_count = 0;

    for property_id in 0..PROPERTY_COUNT {
        if owned_tiles & (1 << TILE_ID_BY_PROPERTY_ID[property_id]) == 0 {
            continue;
        }

        let improvement_level = game_state.board.improvement_level_by_property_id[property_id];
        if improvement_level == HOTEL_IMPROVEMENT_LEVEL {
            hotel_count += 1;
        } else {
            house_count += improvement_level as Cash;
        }
    }

    (house_count, hotel_count)
}
