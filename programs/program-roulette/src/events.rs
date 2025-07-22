use anchor_lang::prelude::*;
use crate::state::Bet;

#[event]
pub struct RoundStarted {
    pub round: u64,
    pub starter: Pubkey,
    pub start_time: i64,
}

#[event]
pub struct WinningsClaimed {
    pub round: u64,
    pub player: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct BetsClosed {
    pub round: u64,
    pub closer: Pubkey,
    pub close_time: i64,
}

#[event]
pub struct RandomGenerated {
    pub round: u64,
    pub initiator: Pubkey,
    pub winning_number: u8,
    pub generation_time: i64,
    pub slot: u64,
    pub last_bettor: Pubkey,
    pub hash_result: [u8; 32],
    pub hash_prefix_u64: u64,
}

#[event]
pub struct LiquidityProvided {
    pub provider: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityWithdrawn {
    pub provider: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct BetPlaced {
    pub player: Pubkey,
    pub token_mint: Pubkey,
    pub round: u64,
    pub bet: Bet,
    pub timestamp: i64,
}

#[event]
pub struct ProviderRevenueWithdrawn {
    pub provider: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}