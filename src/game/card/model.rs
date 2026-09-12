use crate::game::tile::model::{
    Money,
    TileId,
};

// cards per deck (16) < 255
pub type CardId = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardDefinition {
    pub text: &'static str,
    pub effect: CardEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardEffect {
    AdvanceToTile { tile_id: TileId },
    AdvanceToNearestRailroad,
    AdvanceToNearestUtility,
    MoveBackward { tile_count: u8 },
    GoToJail,
    GetOutOfJailFree,
    CollectFromBank { amount: Money },
    PayBank { amount: Money },
    CollectFromEachPlayer { amount: Money },
    PayEachPlayer { amount: Money },
    PayForRepairs { amount_per_house: Money, amount_per_hotel: Money },
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckKind {
    Chance,
    CommunityChest,
}

impl DeckKind {
    pub const COUNT: usize = 2;
}
