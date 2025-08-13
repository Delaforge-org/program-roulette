use anchor_lang::prelude::*;
use crate::{
    errors::RouletteError,
    state::*,
};

/// Accounts required for initializing the global `GameSession` account.
#[derive(Accounts)]
pub struct InitializeGameSession<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(init, payer = authority, space = 117, seeds = [b"game_session"], bump)] // 85 + 32 = 117
    pub game_session: Account<'info, GameSession>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to start a new roulette round.
#[derive(Accounts)]
pub struct StartNewRound<'info> {
    /// The global `GameSession` account. Mutable to update round status, number, times, etc.
    #[account(
        mut, 
        seeds = [b"game_session"], 
        bump = game_session.bump,
        constraint = starter.key() == game_session.authority @ RouletteError::AdminOnly
    )]
    pub game_session: Account<'info, GameSession>,

    /// The admin initiating the new round (signer).
    #[account(mut)]
    pub starter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseBets<'info> {
    /// The global `GameSession` account. Mutable to update status and timestamps.
    #[account(
        mut, 
        seeds = [b"game_session"], 
        bump = game_session.bump,
        constraint = closer.key() == game_session.authority @ RouletteError::AdminOnly
    )]
    pub game_session: Account<'info, GameSession>,

    /// The admin initiating the closing of bets (signer).
    #[account(mut)]
    pub closer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts required to trigger the random number generation for the current round.
#[derive(Accounts)]
pub struct GetRandom<'info> {
    /// The global `GameSession` account. Mutable to store the winning number and update status.
    #[account(
        mut, 
        seeds = [b"game_session"], 
        bump = game_session.bump,
        constraint = random_initiator.key() == game_session.authority @ RouletteError::AdminOnly
    )]
    pub game_session: Account<'info, GameSession>,

    /// The admin initiating the random generation (signer).
    #[account(mut)]
    pub random_initiator: Signer<'info>,
}