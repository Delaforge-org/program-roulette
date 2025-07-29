use anchor_lang::prelude::*;

pub const TREASURY_PUBKEY: Pubkey = pubkey!("DRqMriKY4X3ggiFdx27Fotu5HebQFyRZhNasWFTzaQ78");
pub const CREATE_VAULT_FEE_SOL_LAMPORTS: u64 = 737_000_000;


pub const MAX_BETS_PER_ROUND: usize = 10; // Example limit for space calculation


/// Divisor for calculating liquidity provider rewards (~1.1%).
pub const PROVIDER_DIVISOR: u64 = 91;
/// Divisor for calculating program owner revenue (~1.6%).
pub const OWNER_DIVISOR: u64 = 62;
/// Precision for calculating provider rewards index.
pub const REWARD_PRECISION: u128 = 1_000_000_000_000;


/// Minimum duration (in seconds) a round must be open for betting.
pub const MIN_ROUND_DURATION: i64 = 180; // 3 minutes
pub const MIN_BETS_CLOSED_DURATION: i64 = 15; // 15 seconds
pub const MIN_START_NEW_ROUND_DURATION: i64 = 45; // 45 seconds


/// Constant for 'Straight' bet type.
pub const BET_TYPE_STRAIGHT: u8 = 0;
/// Constant for 'Split' bet type.
pub const BET_TYPE_SPLIT: u8 = 1;
/// Maximum valid numerical value for a bet type enum.
pub const BET_TYPE_MAX: u8 = 15;