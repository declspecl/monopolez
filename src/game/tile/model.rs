#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileDefinition {
    pub name: &'static str,
    pub kind: TileDefinitionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileDefinitionKind {
    Property { color: PropertyColor, purchase_price: u16, rent: PropertyRent },
    Railroad,
    Utility,
    Tax { amount: u16 },
    Chance,
    CommunityChest,
    Go,
    Jail,
    FreeParking,
    GoToJail,
}

impl TileDefinitionKind {
    pub const fn to_tile_kind(&self) -> TileKind {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyRent {
    pub unimproved: u16,
    pub one_house: u16,
    pub two_houses: u16,
    pub three_houses: u16,
    pub four_houses: u16,
    pub hotel: u16,
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
    pub const fn to_ownership_group(self) -> OwnershipGroup {
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

    pub const fn get_house_purchase_price(self) -> u16 {
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
