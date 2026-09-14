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

pub fn publish_event<const N: usize, S: crate::game::strategy::model::PlayerStrategy>(
    strategies: &mut [S; N],
    recorder: usize,
    event: GameEvent,
) {
    strategies[recorder].record_event(event);
    for strategy in strategies {
        strategy.observe_public_event(event);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GameEvent {
    TurnStarted {
        player_id: PlayerId,
    },
    DiceRolled {
        player_id: PlayerId,
        first: u8,
        second: u8,
    },
    Landed {
        player_id: PlayerId,
        tile_id: TileId,
    },
    CardDrawn {
        player_id: PlayerId,
        deck: DeckKind,
        effect: CardEffect,
    },
    PropertyPurchased {
        player_id: PlayerId,
        tile_id: TileId,
        price: Cash,
        auction: bool,
    },
    PaymentDue {
        player_id: PlayerId,
        creditor: Creditor,
        amount: Cash,
    },
    PaymentCompleted {
        player_id: PlayerId,
        creditor: Creditor,
        amount: Cash,
    },
    Bankrupt {
        player_id: PlayerId,
        creditor: Creditor,
        remaining_cash: Cash,
    },
    TradeExecuted {
        offer: TradeOffer,
    },
    BuildingPurchased {
        player_id: PlayerId,
        property_id: PropertyId,
        level: u8,
    },
    TileUnmortgaged {
        player_id: PlayerId,
        tile_id: TileId,
    },
    BuildingSold {
        player_id: PlayerId,
        property_id: PropertyId,
        previous_level: u8,
        level: u8,
        proceeds: Cash,
    },
    TileMortgaged {
        player_id: PlayerId,
        tile_id: TileId,
        proceeds: Cash,
    },
}

impl GameEvent {
    pub const fn trace_version(self) -> u32 {
        match self {
            Self::BuildingSold { .. } | Self::TileMortgaged { .. } => 3,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::model::Ruleset;
    use crate::game::state::model::GameState;
    use crate::game::strategy::configurable::ConfigurableStrategy;
    use crate::game::strategy::model::{
        JailAction,
        PlayerStrategy,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Observer {
        public: Vec<GameEvent>,
        diagnostic: Rc<RefCell<Vec<GameEvent>>>,
    }

    macro_rules! delegate {
        ($method:ident, $response:ty $(, $argument:ident: $kind:ty)*) => {
            fn $method<const N: usize>(&mut self, state: &GameState<N>, rules: &Ruleset, player: PlayerId, $($argument: $kind),*) -> $response {
                ConfigurableStrategy::new().$method(state, rules, player, $($argument),*)
            }
        };
    }

    impl PlayerStrategy for Observer {
        fn observe_public_event(
            &mut self,
            event: GameEvent,
        ) {
            self.public.push(event);
        }
        fn record_event(
            &mut self,
            event: GameEvent,
        ) {
            self.diagnostic.borrow_mut().push(event);
        }
        delegate!(should_purchase_property, bool, tile: TileId);
        delegate!(choose_max_auction_bid, Cash, tile: TileId);
        delegate!(choose_jail_action, JailAction);
        delegate!(choose_property_to_improve, Option<PropertyId>);
        delegate!(choose_tile_to_unmortgage, Option<TileId>);
        delegate!(propose_trade, Option<TradeOffer>);
        delegate!(should_accept_trade, bool, offer: &TradeOffer);
    }

    fn verify_observers<const N: usize>() {
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let diagnostic = Rc::new(RefCell::new(Vec::new()));
        let mut strategies: [Observer; N] = core::array::from_fn(|_| Observer {
            public: Vec::new(),
            diagnostic: diagnostic.clone(),
        });
        let mut state = GameState::<N>::create_starting_state(&rules, 3);
        super::super::turn::play_game(&mut state, &rules, &mut strategies, 1000);
        let events = diagnostic.borrow();
        assert!(!events.is_empty());
        assert!(events.iter().any(|event| matches!(event, GameEvent::CardDrawn { .. })));
        for strategy in strategies {
            assert_eq!(strategy.public, *events);
        }
        let mut expected = GameState::<N>::create_starting_state(&rules, 3);
        super::super::turn::play_game(&mut expected, &rules, &mut [ConfigurableStrategy::new(); N], 1000);
        assert_eq!(state, expected);
    }

    #[test]
    fn public_events_reach_every_player_without_duplicating_trace_records() {
        verify_observers::<2>();
        verify_observers::<4>();
        verify_observers::<8>();
    }

    #[test]
    fn liquidation_is_observed_by_creditors_and_other_players() {
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let diagnostic = Rc::new(RefCell::new(Vec::new()));
        let mut strategies: [Observer; 4] = core::array::from_fn(|_| Observer {
            public: Vec::new(),
            diagnostic: diagnostic.clone(),
        });
        let mut state = GameState::<4>::create_starting_state(&rules, 3);
        state.cash_by_player_id[0] = 0;
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.improvement_level_by_property_id[0] = 1;
        state.board.bank_house_count -= 1;
        super::super::payment::charge_player(&mut state, &rules, &mut strategies, 0, 50, Creditor::Player(1));
        let events = diagnostic.borrow();
        assert!(events.iter().any(|event| matches!(event, GameEvent::BuildingSold { .. })));
        assert!(events.iter().any(|event| matches!(event, GameEvent::TileMortgaged { .. })));
        for strategy in strategies {
            assert_eq!(strategy.public, *events);
        }
    }
}
