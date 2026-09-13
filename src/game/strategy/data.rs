use super::configurable::ConfigurableStrategy;

pub const BASELINE_STRATEGY: ConfigurableStrategy = ConfigurableStrategy::new();

// tuned against mixed fields on the dex ruleset: wins 1.5x its fair share at 4 players and 1.7x at 6
pub const DEX_OPTIMAL_STRATEGY: ConfigurableStrategy = ConfigurableStrategy {
    cash_reserve: 100,
    purchase_cash_percent: 100,
    auction_bid_percent: 200,
    improvement_cash_percent: 100,
    unmortgage_cash_percent: 1000,
    trade_offer_percent: 400,
    trade_accept_percent: 300,
    pays_bail_when_affordable: true,
    mortgages_before_selling_buildings: false,
    jail_camping_unowned_tile_threshold: None,
};

// heads up there is no third party to outbid, so paying monopoly premiums just funds the opponent
pub const DEX_DUEL_STRATEGY: ConfigurableStrategy = ConfigurableStrategy {
    cash_reserve: 100,
    purchase_cash_percent: 100,
    auction_bid_percent: 100,
    improvement_cash_percent: 100,
    unmortgage_cash_percent: 100,
    trade_offer_percent: 150,
    trade_accept_percent: 100,
    pays_bail_when_affordable: false,
    mortgages_before_selling_buildings: false,
    jail_camping_unowned_tile_threshold: None,
};

pub const NEVER_TRADING_STRATEGY: ConfigurableStrategy = ConfigurableStrategy {
    cash_reserve: 100,
    purchase_cash_percent: 100,
    auction_bid_percent: 100,
    improvement_cash_percent: 100,
    unmortgage_cash_percent: 100,
    trade_offer_percent: 0,
    trade_accept_percent: 1000,
    pays_bail_when_affordable: false,
    mortgages_before_selling_buildings: false,
    jail_camping_unowned_tile_threshold: None,
};
