use crate::game::board::model::PlayerId;
use crate::game::card::model::DeckKind;
use crate::game::tile::model::{
    Cash,
    TileSetMask,
};

// one bit per deck kind (2) < 8 bits
pub type DeckKindSetMask = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TradeOffer {
    pub proposer_player_id: PlayerId,
    pub recipient_player_id: PlayerId,
    pub offered_cash: Cash,
    pub offered_tiles: TileSetMask,
    pub offered_get_out_of_jail_free_cards: DeckKindSetMask,
    pub requested_cash: Cash,
    pub requested_tiles: TileSetMask,
    pub requested_get_out_of_jail_free_cards: DeckKindSetMask,
}

impl TradeOffer {
    pub const fn traded_tiles(&self) -> TileSetMask {
        self.offered_tiles | self.requested_tiles
    }

    pub const fn traded_get_out_of_jail_free_cards(&self) -> DeckKindSetMask {
        self.offered_get_out_of_jail_free_cards | self.requested_get_out_of_jail_free_cards
    }
}

pub const fn deck_kind_bit(deck_kind: DeckKind) -> DeckKindSetMask {
    1 << deck_kind as u8
}
