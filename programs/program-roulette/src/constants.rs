use anchor_lang::prelude::*;

pub const TREASURY_PUBKEY: Pubkey = pubkey!("DELAFDnkiiqu9amML7drYNLmcVALXboRQ2yf4DQeDTMr");
pub const GAME_ADMIN_PUBKEY: Pubkey = pubkey!("RBAd8hvSpJMtBu5o2BJytCBvy9wy6UKJvebDf7wRw7A");
pub const CREATE_VAULT_FEE_SOL_LAMPORTS: u64 = 537_000_000;


pub const MAX_BETS_PER_ROUND: usize = 8; // Example limit for space calculation


/// Divisor for calculating liquidity provider rewards (~1.4%).
pub const PROVIDER_DIVISOR: u64 = 71;

/// Divisor for calculating program owner revenue (~0.8%).
pub const OWNER_DIVISOR: u64 = 125;
/// Precision for calculating provider rewards index.
pub const REWARD_PRECISION: u128 = 1_000_000_000_000;

/// Maximum bet allowed as a percentage of the vault's total liquidity.
pub const MAX_BET_PERCENTAGE: u64 = 11;
/// Divisor for calculating the maximum bet percentage.
pub const MAX_BET_PERCENTAGE_DIVISOR: u64 = 100;

/// Maximum valid numerical value for a bet type enum.
pub const BET_TYPE_MAX: u8 = 15;