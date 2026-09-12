use crate::game::board::model::{
    BoardState,
    PlayerId,
};
use crate::game::card::data::CARD_COUNT_PER_DECK;
use crate::game::card::model::{
    CardId,
    DeckKind,
};
use crate::game::rng::WyRand;
use crate::game::ruleset::model::{
    Ruleset,
    TurnCount,
};
use crate::game::tile::data::GO_TILE_ID;
use crate::game::tile::model::{
    Cash,
    TileId,
};

// max player count (8) = 8 bits
pub type PlayerSetMask = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeckState {
    pub card_id_by_draw_position: [CardId; CARD_COUNT_PER_DECK],
    pub next_draw_position: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameState<const PLAYER_COUNT: usize> {
    pub board: BoardState<PLAYER_COUNT>,
    pub cash_by_player_id: [Cash; PLAYER_COUNT],
    pub position_by_player_id: [TileId; PLAYER_COUNT],
    pub jail_turn_count_by_player_id: [TurnCount; PLAYER_COUNT],
    pub jailed_players: PlayerSetMask,
    pub bankrupt_players: PlayerSetMask,
    pub get_out_of_jail_free_card_holder_by_deck_kind: [Option<PlayerId>; DeckKind::COUNT],
    pub deck_state_by_deck_kind: [DeckState; DeckKind::COUNT],
    pub free_parking_jackpot: Cash,
    pub current_player_id: PlayerId,
    pub consecutive_double_count: u8,
    pub rng: WyRand,
}

impl<const PLAYER_COUNT: usize> GameState<PLAYER_COUNT> {
    pub fn create_starting_state(
        ruleset: &Ruleset,
        seed: u64,
    ) -> Self {
        let mut rng = WyRand::new(seed);

        let mut deck_state_by_deck_kind = [DeckState {
            card_id_by_draw_position: core::array::from_fn(|draw_position| draw_position as CardId),
            next_draw_position: 0,
        }; DeckKind::COUNT];

        for deck_state in &mut deck_state_by_deck_kind {
            rng.shuffle(&mut deck_state.card_id_by_draw_position);
        }

        Self {
            board: BoardState::STARTING_STATE,
            cash_by_player_id: [ruleset.starting_player_money as Cash; PLAYER_COUNT],
            position_by_player_id: [GO_TILE_ID; PLAYER_COUNT],
            jail_turn_count_by_player_id: [0; PLAYER_COUNT],
            jailed_players: 0,
            bankrupt_players: 0,
            get_out_of_jail_free_card_holder_by_deck_kind: [None; DeckKind::COUNT],
            deck_state_by_deck_kind,
            free_parking_jackpot: 0,
            current_player_id: 0,
            consecutive_double_count: 0,
            rng,
        }
    }
}

const _: () = {
    assert!(size_of::<GameState<2>>() == 192);
    assert!(size_of::<GameState<4>>() == 192);
    assert!(size_of::<GameState<8>>() == 256);
};
