use crate::game::board::model::PlayerId;
use crate::game::state::model::GameState;
use crate::game::tile::model::{
    Cash,
    TileId,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JailAction {
    RollForDoubles,
    PayBail,
    UseGetOutOfJailFreeCard,
}

pub trait PlayerStrategy {
    fn should_purchase_property<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> bool;

    fn choose_max_auction_bid<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        player_id: PlayerId,
        tile_id: TileId,
    ) -> Cash;

    fn choose_jail_action<const PLAYER_COUNT: usize>(
        &mut self,
        game_state: &GameState<PLAYER_COUNT>,
        player_id: PlayerId,
    ) -> JailAction;
}
