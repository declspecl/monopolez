use anyhow::{
    Result,
    bail,
};
use rayon::prelude::*;
use serde::Serialize;

use super::tournament::TournamentConfig;
use crate::game::board::model::PlayerId;
use crate::game::engine::turn::play_turn;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::{
    GameState,
    PlayerSetMask,
};
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    TILE_ID_BY_PROPERTY_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
};
use crate::game::tile::model::OwnershipGroup;

pub const MONOPOLY_COUNT_BUCKET_COUNT: usize = 5;
pub const DEVELOPMENT_BUCKET_COUNT: usize = 4;

pub const MONOPOLY_SNAPSHOT_TURN: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct GroupEntry {
    pub first_completion_count: u32,
    pub first_completion_win_count: u32,
    pub total_completion_turn: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct MonopolyCountEntry {
    pub player_count: u32,
    pub win_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct MonopolyReport {
    pub game_count: u32,
    pub decisive_game_count: u32,
    pub snapshot_game_count: u32,
    pub group_entries: [GroupEntry; OwnershipGroup::COUNT],
    pub monopoly_count_entries: [MonopolyCountEntry; MONOPOLY_COUNT_BUCKET_COUNT],
    pub sole_monopoly_entries: [MonopolyCountEntry; OwnershipGroup::COUNT],
    pub sole_monopoly_development_entries: [MonopolyCountEntry; DEVELOPMENT_BUCKET_COUNT],
}

impl MonopolyReport {
    pub const fn empty() -> Self {
        Self {
            game_count: 0,
            decisive_game_count: 0,
            snapshot_game_count: 0,
            group_entries: [GroupEntry {
                first_completion_count: 0,
                first_completion_win_count: 0,
                total_completion_turn: 0,
            }; OwnershipGroup::COUNT],
            monopoly_count_entries: [MonopolyCountEntry { player_count: 0, win_count: 0 }; MONOPOLY_COUNT_BUCKET_COUNT],
            sole_monopoly_entries: [MonopolyCountEntry { player_count: 0, win_count: 0 }; OwnershipGroup::COUNT],
            sole_monopoly_development_entries: [MonopolyCountEntry { player_count: 0, win_count: 0 }; DEVELOPMENT_BUCKET_COUNT],
        }
    }

    pub fn combine(
        self,
        other: Self,
    ) -> Self {
        let mut combined = self;
        combined.game_count += other.game_count;
        combined.decisive_game_count += other.decisive_game_count;
        combined.snapshot_game_count += other.snapshot_game_count;

        for group_index in 0..OwnershipGroup::COUNT {
            combined.group_entries[group_index].first_completion_count += other.group_entries[group_index].first_completion_count;
            combined.group_entries[group_index].first_completion_win_count += other.group_entries[group_index].first_completion_win_count;
            combined.group_entries[group_index].total_completion_turn += other.group_entries[group_index].total_completion_turn;
        }

        for bucket_index in 0..MONOPOLY_COUNT_BUCKET_COUNT {
            combined.monopoly_count_entries[bucket_index].player_count += other.monopoly_count_entries[bucket_index].player_count;
            combined.monopoly_count_entries[bucket_index].win_count += other.monopoly_count_entries[bucket_index].win_count;
        }

        for group_index in 0..OwnershipGroup::COUNT {
            combined.sole_monopoly_entries[group_index].player_count += other.sole_monopoly_entries[group_index].player_count;
            combined.sole_monopoly_entries[group_index].win_count += other.sole_monopoly_entries[group_index].win_count;
        }

        for bucket_index in 0..DEVELOPMENT_BUCKET_COUNT {
            combined.sole_monopoly_development_entries[bucket_index].player_count += other.sole_monopoly_development_entries[bucket_index].player_count;
            combined.sole_monopoly_development_entries[bucket_index].win_count += other.sole_monopoly_development_entries[bucket_index].win_count;
        }

        combined
    }
}

pub fn analyze_monopolies(
    ruleset: &Ruleset,
    strategy: ConfigurableStrategy,
    config: &TournamentConfig,
) -> Result<MonopolyReport> {
    if config.game_count == 0 || config.max_turn_count == 0 {
        bail!("analysis requires positive game and turn counts");
    }
    match config.player_count {
        2 => Ok(analyze_games::<2>(ruleset, strategy, config)),
        3 => Ok(analyze_games::<3>(ruleset, strategy, config)),
        4 => Ok(analyze_games::<4>(ruleset, strategy, config)),
        5 => Ok(analyze_games::<5>(ruleset, strategy, config)),
        6 => Ok(analyze_games::<6>(ruleset, strategy, config)),
        7 => Ok(analyze_games::<7>(ruleset, strategy, config)),
        8 => Ok(analyze_games::<8>(ruleset, strategy, config)),
        player_count => bail!("player count {player_count} is outside the supported range of 2 to 8"),
    }
}

fn analyze_games<const PLAYER_COUNT: usize>(
    ruleset: &Ruleset,
    strategy: ConfigurableStrategy,
    config: &TournamentConfig,
) -> MonopolyReport {
    (0..config.game_count)
        .into_par_iter()
        .map(|game_index| analyze_game::<PLAYER_COUNT>(ruleset, strategy, config, game_index))
        .reduce(MonopolyReport::empty, MonopolyReport::combine)
}

fn analyze_game<const PLAYER_COUNT: usize>(
    ruleset: &Ruleset,
    strategy: ConfigurableStrategy,
    config: &TournamentConfig,
    game_index: u32,
) -> MonopolyReport {
    let all_players = ((1u16 << PLAYER_COUNT) - 1) as PlayerSetMask;
    let mut strategies = [strategy; PLAYER_COUNT];
    let mut game_state = GameState::<PLAYER_COUNT>::create_starting_state(ruleset, config.seed.wrapping_add(game_index as u64));

    let mut report = MonopolyReport::empty();
    report.game_count = 1;

    let mut first_holder_by_group: [Option<PlayerId>; OwnershipGroup::COUNT] = [None; OwnershipGroup::COUNT];
    let mut completion_turn_by_group = [0u64; OwnershipGroup::COUNT];
    let mut monopoly_count_by_player = [0usize; PLAYER_COUNT];
    let mut sole_monopoly_group_by_player: [Option<usize>; PLAYER_COUNT] = [None; PLAYER_COUNT];
    let mut building_count_by_player = [0usize; PLAYER_COUNT];
    let mut winner_player_id = None;

    for turn_index in 0..config.max_turn_count {
        play_turn(&mut game_state, ruleset, &mut strategies);
        record_completed_groups(&game_state, &mut first_holder_by_group, &mut completion_turn_by_group, turn_index);

        if turn_index + 1 == MONOPOLY_SNAPSHOT_TURN {
            report.snapshot_game_count = 1;
            monopoly_count_by_player = count_monopolies_by_player(&game_state);
            sole_monopoly_group_by_player = find_sole_monopoly_group_by_player(&game_state);
            building_count_by_player = count_buildings_by_player(&game_state);
        }

        let active_players = all_players & !game_state.bankrupt_players;
        if active_players.count_ones() == 1 {
            winner_player_id = Some(active_players.trailing_zeros() as PlayerId);
            report.decisive_game_count = 1;

            break;
        }
    }

    for group_index in 0..OwnershipGroup::COUNT {
        let Some(first_holder_player_id) = first_holder_by_group[group_index] else {
            continue;
        };

        report.group_entries[group_index].first_completion_count = 1;
        report.group_entries[group_index].total_completion_turn = completion_turn_by_group[group_index];

        if winner_player_id == Some(first_holder_player_id) {
            report.group_entries[group_index].first_completion_win_count = 1;
        }
    }

    if report.snapshot_game_count == 0 {
        return report;
    }

    for (player_index, sole_monopoly_group) in sole_monopoly_group_by_player.iter().enumerate() {
        let Some(group_index) = sole_monopoly_group else {
            continue;
        };

        report.sole_monopoly_entries[*group_index].player_count += 1;
        if winner_player_id == Some(player_index as PlayerId) {
            report.sole_monopoly_entries[*group_index].win_count += 1;
        }

        let development_bucket_index = match building_count_by_player[player_index] {
            0 => 0,
            1..=4 => 1,
            5..=9 => 2,
            _ => 3,
        };

        report.sole_monopoly_development_entries[development_bucket_index].player_count += 1;
        if winner_player_id == Some(player_index as PlayerId) {
            report.sole_monopoly_development_entries[development_bucket_index].win_count += 1;
        }
    }

    for (player_index, monopoly_count) in monopoly_count_by_player.iter().enumerate() {
        let bucket_index = (*monopoly_count).min(MONOPOLY_COUNT_BUCKET_COUNT - 1);

        report.monopoly_count_entries[bucket_index].player_count += 1;
        if winner_player_id == Some(player_index as PlayerId) {
            report.monopoly_count_entries[bucket_index].win_count += 1;
        }
    }

    report
}

fn record_completed_groups<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    first_holder_by_group: &mut [Option<PlayerId>; OwnershipGroup::COUNT],
    completion_turn_by_group: &mut [u64; OwnershipGroup::COUNT],
    turn_index: u32,
) {
    for group_index in 0..OwnershipGroup::COUNT {
        if first_holder_by_group[group_index].is_some() {
            continue;
        }

        let group_tiles = TILE_SET_MASK_BY_OWNERSHIP_GROUP[group_index];
        for player_id in 0..PLAYER_COUNT as PlayerId {
            if game_state.board.does_player_own_all_tiles(player_id, group_tiles) {
                first_holder_by_group[group_index] = Some(player_id);
                completion_turn_by_group[group_index] = turn_index as u64 + 1;

                break;
            }
        }
    }
}

fn find_sole_monopoly_group_by_player<const PLAYER_COUNT: usize>(game_state: &GameState<PLAYER_COUNT>) -> [Option<usize>; PLAYER_COUNT] {
    core::array::from_fn(|player_index| {
        let mut owned_groups = (0..OwnershipGroup::COUNT).filter(|group_index| game_state.board.does_player_own_all_tiles(player_index as PlayerId, TILE_SET_MASK_BY_OWNERSHIP_GROUP[*group_index]));

        let sole_group_index = owned_groups.next()?;
        if owned_groups.next().is_some() {
            return None;
        }

        Some(sole_group_index)
    })
}

fn count_buildings_by_player<const PLAYER_COUNT: usize>(game_state: &GameState<PLAYER_COUNT>) -> [usize; PLAYER_COUNT] {
    core::array::from_fn(|player_index| {
        let owned_tiles = game_state.board.owned_tiles_by_player_id[player_index];

        (0..PROPERTY_COUNT)
            .filter(|property_id| owned_tiles & (1 << TILE_ID_BY_PROPERTY_ID[*property_id]) != 0)
            .map(|property_id| game_state.board.improvement_level_by_property_id[property_id] as usize)
            .sum()
    })
}

fn count_monopolies_by_player<const PLAYER_COUNT: usize>(game_state: &GameState<PLAYER_COUNT>) -> [usize; PLAYER_COUNT] {
    core::array::from_fn(|player_index| {
        (0..OwnershipGroup::COUNT)
            .filter(|group_index| game_state.board.does_player_own_all_tiles(player_index as PlayerId, TILE_SET_MASK_BY_OWNERSHIP_GROUP[*group_index]))
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::data::DEX_OPTIMAL_STRATEGY;

    #[test]
    fn counts_each_game_and_player_once() {
        let config = TournamentConfig {
            game_count: 500,
            max_turn_count: 3_000,
            seed: 3,
            player_count: 4,
        };

        let report = analyze_monopolies(&DEX_RULESET, DEX_OPTIMAL_STRATEGY, &config).expect("4 players should be supported");
        let bucketed_player_count: u32 = report.monopoly_count_entries.iter().map(|entry| entry.player_count).sum();

        assert_eq!(report.game_count, 500);
        assert_eq!(bucketed_player_count, report.snapshot_game_count * 4);
        assert!(report.group_entries.iter().all(|entry| entry.first_completion_count <= report.game_count));
    }

    #[test]
    fn short_runs_do_not_fabricate_snapshots() {
        let config = TournamentConfig {
            game_count: 20,
            max_turn_count: 99,
            seed: 3,
            player_count: 4,
        };
        let report = analyze_monopolies(&DEX_RULESET, DEX_OPTIMAL_STRATEGY, &config).unwrap();
        assert_eq!(report.snapshot_game_count, 0);
        assert!(report.monopoly_count_entries.iter().all(|entry| entry.player_count == 0));
        assert!(report.sole_monopoly_entries.iter().all(|entry| entry.player_count == 0));
        assert!(report.sole_monopoly_development_entries.iter().all(|entry| entry.player_count == 0));
    }

    #[test]
    fn games_ending_before_snapshot_are_excluded() {
        let ruleset = Ruleset::builder().with_starting_player_money(0).with_go_passing_salary(0).with_go_landing_salary(0).build();
        let config = TournamentConfig {
            game_count: 50,
            max_turn_count: 100,
            seed: 0,
            player_count: 2,
        };
        let report = analyze_monopolies(&ruleset, ConfigurableStrategy::new(), &config).unwrap();
        assert!(report.decisive_game_count > 0);
        assert!(report.snapshot_game_count < report.game_count);
        assert_eq!(report.monopoly_count_entries.iter().map(|entry| entry.player_count).sum::<u32>(), report.snapshot_game_count * 2);
    }
}
