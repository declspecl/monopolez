use super::data::{
    MORTGAGE_VALUE_PERCENT_OF_PURCHASE_PRICE,
    RAILROAD_TILE_PURCHASE_PRICE,
    UNMORTGAGE_INTEREST_PERCENT_OF_MORTGAGE_VALUE,
    UTILITY_TILE_PURCHASE_PRICE,
};

// total tiles (40) < 255
pub type TileId = u8;

// total tiles (40) < 64 bits for bitmask
pub type TileSetMask = u64;

// static amounts (prices, rents, taxes) max out at 2000 < 65,535
pub type Money = u16;

// bank never runs out, so balances are unbounded in theory, but u32 max (~4.29B) is the absolute max cash
pub type Cash = u32;

pub type RentLevel = usize;

// total properties (22) < 255
pub type PropertyId = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileDefinition {
    pub name: &'static str,
    pub kind: TileDefinitionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileDefinitionKind {
    Property { color: PropertyColor, purchase_price: Money, rent: PropertyRent },
    Railroad,
    Utility,
    Tax { amount: Money },
    Chance,
    CommunityChest,
    Go,
    Jail,
    FreeParking,
    GoToJail,
}

impl TileDefinitionKind {
    pub const fn tile_kind(&self) -> TileKind {
        match self {
            Self::Property { .. } => TileKind::Property,
            Self::Railroad => TileKind::Railroad,
            Self::Utility => TileKind::Utility,
            Self::Tax { .. } => TileKind::Tax,
            Self::Chance => TileKind::Chance,
            Self::CommunityChest => TileKind::CommunityChest,
            Self::Go => TileKind::Go,
            Self::Jail => TileKind::Jail,
            Self::FreeParking => TileKind::FreeParking,
            Self::GoToJail => TileKind::GoToJail,
        }
    }

    pub const fn ownership_group(&self) -> Option<OwnershipGroup> {
        match *self {
            Self::Property { color, .. } => Some(color.ownership_group()),
            Self::Railroad => Some(OwnershipGroup::Railroad),
            Self::Utility => Some(OwnershipGroup::Utility),
            _ => None,
        }
    }

    pub const fn purchase_price(&self) -> Money {
        match *self {
            Self::Property { purchase_price, .. } => purchase_price,
            Self::Railroad => RAILROAD_TILE_PURCHASE_PRICE,
            Self::Utility => UTILITY_TILE_PURCHASE_PRICE,
            _ => 0,
        }
    }

    pub const fn calculate_mortgage_value(&self) -> Money {
        (self.purchase_price() * MORTGAGE_VALUE_PERCENT_OF_PURCHASE_PRICE) / 100
    }

    pub const fn calculate_unmortgage_price(&self) -> Money {
        let mortgage_value = self.calculate_mortgage_value();
        let unmortgage_interest = (mortgage_value * UNMORTGAGE_INTEREST_PERCENT_OF_MORTGAGE_VALUE).div_ceil(100);

        mortgage_value + unmortgage_interest
    }

    pub const fn house_purchase_price(&self) -> Money {
        match *self {
            Self::Property { color, .. } => color.house_purchase_price(),
            _ => 0,
        }
    }

    pub const fn tax_amount(&self) -> Money {
        match *self {
            Self::Tax { amount } => amount,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyRent {
    pub unimproved: Money,
    pub one_house: Money,
    pub two_houses: Money,
    pub three_houses: Money,
    pub four_houses: Money,
    pub hotel: Money,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileKind {
    Property,
    Railroad,
    Utility,
    Tax,
    Chance,
    CommunityChest,
    Go,
    Jail,
    FreeParking,
    GoToJail,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyColor {
    Brown,
    LightBlue,
    Pink,
    Orange,
    Red,
    Yellow,
    Green,
    DarkBlue,
}

impl PropertyColor {
    pub const fn ownership_group(self) -> OwnershipGroup {
        match self {
            Self::Brown => OwnershipGroup::Brown,
            Self::LightBlue => OwnershipGroup::LightBlue,
            Self::Pink => OwnershipGroup::Pink,
            Self::Orange => OwnershipGroup::Orange,
            Self::Red => OwnershipGroup::Red,
            Self::Yellow => OwnershipGroup::Yellow,
            Self::Green => OwnershipGroup::Green,
            Self::DarkBlue => OwnershipGroup::DarkBlue,
        }
    }

    pub const fn house_purchase_price(self) -> Money {
        match self {
            Self::Brown => 50,
            Self::LightBlue => 50,
            Self::Pink => 100,
            Self::Orange => 100,
            Self::Red => 150,
            Self::Yellow => 150,
            Self::Green => 200,
            Self::DarkBlue => 200,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnershipGroup {
    Brown,
    LightBlue,
    Pink,
    Orange,
    Red,
    Yellow,
    Green,
    DarkBlue,
    Railroad,
    Utility,
}

impl OwnershipGroup {
    pub const COUNT: usize = 10;
}
