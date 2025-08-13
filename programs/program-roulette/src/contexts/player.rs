use anchor_lang::prelude::*;
use crate::{
    constants::*,
    errors::RouletteError,
    state::*,
};
use anchor_spl::token::Token;

/// Accounts required for initializing the player's betting account for the game session.
#[derive(Accounts)]
pub struct InitializePlayerBets<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    #[account(
        init,
        payer = player,
        space = 8 + 32 + 8 + 32 + 32 + (4 + std::mem::size_of::<Bet>() * MAX_BETS_PER_ROUND) + 1,
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump
    )]
    pub player_bets: Account<'info, PlayerBets>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ClosePlayerBetsAccount<'info> {
    /// The player closing their account (signer). Rent SOL will be returned here.
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut, // Account data will be wiped, and lamports transferred.
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump = player_bets.bump, // Make sure we are closing the correct PDA
        close = player // Return lamports to the player signer.
    )]
    pub player_bets: Account<'info, PlayerBets>,

    #[account(seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,
}

/// Accounts required for a player to place bets in the current round.
#[derive(Accounts)]
pub struct PlaceBets<'info> {
    /// The vault corresponding to the token the player is betting with. Mutable to update liquidity and rewards.
    #[account(mut)]
    pub vault: Account<'info, VaultAccount>,

    /// The global `GameSession` account. Mutable to update bet counts.
    #[account(mut, seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// CHECK: Validated in instruction logic (is TokenAccount).
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    /// CHECK: Validated in instruction logic (is TokenAccount). Constraint ensures it matches `vault.token_account`.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::InvalidTokenAccount,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// The player placing the bets (signer).
    #[account(mut)]
    pub player: Signer<'info>,

    /// The account storing the player's bets for the current round. MUST exist (initialized via `initialize_player_bets`).
    /// Seeds: [b"player_bets", game_session_key, player_key]
    #[account(
        mut,
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump = player_bets.bump // Verify bump of existing account
    )]
    pub player_bets: Account<'info, PlayerBets>,

    /// The SPL Token Program, needed for the bet transfer CPI.
    pub token_program: Program<'info, Token>,
}

/// Accounts required for a player to claim their winnings for the MOST RECENTLY completed round.
/// Uses the player's LATEST bets recorded in their PlayerBets account.
#[derive(Accounts)]
pub struct ClaimMyWinnings<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    #[account(
        mut,
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump = player_bets.bump,
        constraint = player_bets.player == player.key() @ RouletteError::Unauthorized,
    )]
    pub player_bets: Account<'info, PlayerBets>,

    #[account(mut, seeds = [b"vault", player_bets.token_mint.as_ref()], bump = vault.bump)]
    pub vault: Account<'info, VaultAccount>,

    /// CHECK: Validated manually + via constraint below.
    #[account(mut, constraint = vault_token_account.key() == vault.token_account)]
    pub vault_token_account: AccountInfo<'info>,

    /// CHECK: Validated manually (mint, owner).
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}