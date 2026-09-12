use serde::{
    Deserialize,
    Serialize,
};

use super::payment::Creditor;
use crate::game::board::model::PlayerId;
use crate::game::card::model::{
    CardEffect,
    DeckKind,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GameEvent {
    TurnStarted { player_id: PlayerId },
    DiceRolled { player_id: PlayerId, first: u8, second: u8 },
    Landed { player_id: PlayerId, tile_id: TileId },
    CardDrawn { player_id: PlayerId, deck: DeckKind, effect: CardEffect },
    PropertyPurchased { player_id: PlayerId, tile_id: TileId, price: Cash, auction: bool },
    PaymentDue { player_id: PlayerId, creditor: Creditor, amount: Cash },
    PaymentCompleted { player_id: PlayerId, creditor: Creditor, amount: Cash },
    Bankrupt { player_id: PlayerId, creditor: Creditor, remaining_cash: Cash },
    TradeExecuted { offer: TradeOffer },
    BuildingPurchased { player_id: PlayerId, property_id: PropertyId, level: u8 },
    TileUnmortgaged { player_id: PlayerId, tile_id: TileId },
}
