use anchor_lang::prelude::*;

/// Represents a single bet placed by a player.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Bet {
    pub amount: u64,
    pub bet_type: u8,
    pub numbers: [u8; 4],
}

/// Defines the possible states of a roulette game round.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Default)]
pub enum RoundStatus {
    #[default]
    NotStarted,
    AcceptingBets,
    BetsClosed,
    Completed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum BetType {
    Straight {
        number: u8,
    },
    Split {
        first: u8,
        second: u8,
    },
    Corner {
        top_left: u8,
    },
    Street {
        street: u8,
    },
    SixLine {
        six_line: u8,
    },
    FirstFour,
    Red,
    Black,
    Even,
    Odd,
    Manque, // 1-18
    Passe, // 19-36
    Column {
        column: u8,
    },
    P12, // 1-12
    M12, // 13-24
    D12, // 25-36
}

#[account]
pub struct VaultAccount {
    pub token_mint: Pubkey,
    pub token_account: Pubkey,
    pub total_liquidity: u64,
    pub total_provider_capital: u64,
    pub bump: u8,
    pub owner_reward: u64,
    pub reward_per_share_index: u128,
}

#[account]
#[derive(Default)]
pub struct GameSession {
    pub current_round: u64,
    pub round_start_time: i64,
    pub round_status: RoundStatus,
    pub winning_number: Option<u8>,
    pub bets_closed_timestamp: i64,
    pub get_random_timestamp: i64,
    pub bump: u8,
    pub last_bettor: Option<Pubkey>,
    pub last_completed_round: u64,
}

#[account]
pub struct PlayerBets {
    pub player: Pubkey,
    pub round: u64,
    pub vault: Pubkey,
    pub token_mint: Pubkey,
    pub bets: Vec<Bet>,
    pub bump: u8,
}

/// Record to prevent double-claiming winnings for a specific player and round.
#[account]
#[derive(Default)]
pub struct ClaimRecord {
    pub claimed: bool,
    pub bump: u8,
}

/// Stores the state for a single liquidity provider in a specific vault.
#[account]
pub struct ProviderState {
    pub vault: Pubkey,    // The vault this state belongs to
    pub provider: Pubkey, // The owner of this state account
    pub amount: u64,      // The total amount of capital provided
    pub unclaimed_rewards: u64,
    pub reward_per_share_index_last_claimed: u128,
    pub bump: u8,
}

impl PlayerBets {
    pub fn calculate_payout_multiplier(bet_type: u8) -> u64 {
        match bet_type {
            0 => 36, // Straight
            1 => 18, // Split
            2 => 9, // Corner
            3 => 12, // Street
            4 => 6, // SixLine
            5 => 9, // FirstFour
            6 | 7 | 8 | 9 | 10 | 11 => 2, // Red/Black/Even/Odd/Manque/Passe
            12 | 13 | 14 | 15 => 3, // Column/Dozens
            _ => 0, // Unknown
        }
    }

    pub fn is_bet_winner(bet_type: u8, numbers: &[u8; 4], winning_number: u8) -> bool {
        const RED_NUMBERS: [u8; 18] = [
            1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36,
        ];

        match bet_type {
            0 => numbers[0] == winning_number, // Straight
            1 => numbers[0] == winning_number || numbers[1] == winning_number, // Split
            2 => {
                // Corner
                let top_left = numbers[0];
                if top_left == 0 || top_left > 34 || top_left % 3 == 0 {
                    return false;
                }
                let corner_numbers = [top_left, top_left + 1, top_left + 3, top_left + 4];
                corner_numbers.contains(&winning_number)
            }
            3 => {
                // Street
                let start_street = numbers[0];
                if
                    start_street == 0 ||
                    start_street > 34 ||
                    (start_street > 0 && (start_street - 1) % 3 != 0)
                {
                    return false;
                }
                winning_number > 0 &&
                    winning_number >= start_street &&
                    winning_number < start_street + 3
            }
            4 => {
                // Six Line
                let start_six_line = numbers[0];
                if
                    start_six_line == 0 ||
                    start_six_line > 31 ||
                    (start_six_line > 0 && (start_six_line - 1) % 3 != 0)
                {
                    return false;
                }
                winning_number > 0 &&
                    winning_number >= start_six_line &&
                    winning_number < start_six_line + 6
            }
            5 => [0, 1, 2, 3].contains(&winning_number), // First Four
            6 => RED_NUMBERS.contains(&winning_number), // Red
            7 => winning_number != 0 && !RED_NUMBERS.contains(&winning_number), // Black
            8 => winning_number != 0 && winning_number % 2 == 0, // Even
            9 => winning_number != 0 && winning_number % 2 == 1, // Odd
            10 => winning_number >= 1 && winning_number <= 18, // Manque (1-18)
            11 => winning_number >= 19 && winning_number <= 36, // Passe (19-36)
            12 => {
                // Column
                let column = numbers[0];
                if column < 1 || column > 3 {
                    return false;
                }
                winning_number != 0 && winning_number % 3 == column % 3
            }
            13 => winning_number >= 1 && winning_number <= 12, // P12 (Dozen 1)
            14 => winning_number >= 13 && winning_number <= 24, // M12 (Dozen 2)
            15 => winning_number >= 25 && winning_number <= 36, // D12 (Dozen 3)
            _ => false, // Unknown
        }
    }
}
