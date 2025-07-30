use anchor_lang::prelude::*;
use anchor_spl::token::{Token};
use crate::{
    constants::*,
    errors::RouletteError,
    state::*,
};

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

// Max bets stored per player per round in their PlayerBets account.

/// Accounts required for initializing the global `GameSession` account.
#[derive(Accounts)]
pub struct InitializeGameSession<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(init, payer = authority, space = 85, seeds = [b"game_session"], bump)]
    pub game_session: Account<'info, GameSession>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required for a user to add liquidity to an existing vault.
#[derive(Accounts)]
pub struct ProvideLiquidity<'info> {
    /// The vault account to which liquidity is being added. Mutable to update `total_liquidity`.
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The mint account for the token being deposited
    /// CHECK: Used for PDA seeds validation
    pub token_mint: AccountInfo<'info>,

    /// The user's state account for this vault. Created if it doesn't exist.
    #[account(
        init_if_needed,
        payer = liquidity_provider,
        space = 8 + std::mem::size_of::<ProviderState>(),
        seeds = [b"provider_state", vault.key().as_ref(), liquidity_provider.key().as_ref()],
        bump
    )]
    pub provider_state: Account<'info, ProviderState>,

    /// CHECK: Validated in instruction logic (is TokenAccount).
    #[account(mut)]
    pub provider_token_account: AccountInfo<'info>,

    /// CHECK: Validated in instruction logic (is TokenAccount). Constraint ensures it matches the vault's stored `token_account`.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// The liquidity provider (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Program<'info, Token>,
    /// The Solana System Program.
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawLiquidity<'info> {
    /// The vault account from which liquidity is being withdrawn.
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The provider's state account, which will be closed.
    #[account(
        mut,
        // The provider's state account must belong to the vault.
        constraint = provider_state.vault == vault.key() @ RouletteError::VaultMismatch,
        // It must also belong to the signer.
        constraint = provider_state.provider == liquidity_provider.key() @ RouletteError::Unauthorized,
        seeds = [b"provider_state", vault.key().as_ref(), liquidity_provider.key().as_ref()],
        bump = provider_state.bump,
        // Close the account and return rent to the provider.
        close = liquidity_provider
    )]
    pub provider_state: Account<'info, ProviderState>,

    /// CHECK: Used for PDA seeds validation
    pub token_mint: AccountInfo<'info>,

    /// CHECK: The provider's token account to receive the funds.
    #[account(mut)]
    pub provider_token_account: AccountInfo<'info>,

    /// CHECK: The vault's token account.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// The liquidity provider requesting the withdrawal (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Program<'info, Token>,
}

/// Accounts required for a liquidity provider to withdraw their accumulated rewards.
#[derive(Accounts)]
pub struct WithdrawProviderRevenue<'info> {
    /// The vault account holding the rewards.
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The provider's state account, which will be updated.
    #[account(
        mut,
        // The provider's state account must belong to the vault.
        constraint = provider_state.vault == vault.key() @ RouletteError::VaultMismatch,
        // It must also belong to the signer.
        constraint = provider_state.provider == liquidity_provider.key() @ RouletteError::Unauthorized,
        seeds = [b"provider_state", vault.key().as_ref(), liquidity_provider.key().as_ref()],
        bump = provider_state.bump
    )]
    pub provider_state: Account<'info, ProviderState>,

    /// The mint account for the token being withdrawn
    /// CHECK: Used for PDA seeds validation
    pub token_mint: AccountInfo<'info>,

    /// CHECK: The provider's token account to receive rewards.
    #[account(mut)]
    pub provider_token_account: AccountInfo<'info>,

    /// CHECK: The vault's token account.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// The liquidity provider requesting the withdrawal (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Program<'info, Token>,
}

/// Accounts required for the program authority to withdraw accumulated owner revenue.
#[derive(Accounts)]
pub struct WithdrawOwnerRevenue<'info> {
    /// The vault account holding the owner revenue. Mutable to update `total_liquidity` and `owner_reward`.
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The mint account for the token being withdrawn
    /// CHECK: Used for PDA seeds validation
    pub token_mint: AccountInfo<'info>,

    /// CHECK: Validated in instruction logic (is TokenAccount).
    #[account(mut)]
    pub owner_treasury_token_account: AccountInfo<'info>,

    /// CHECK: Validated in instruction logic (is TokenAccount). Constraint ensures it matches the vault's stored `token_account`.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Program<'info, Token>,
}

/// Accounts for initializing a vault AND providing initial liquidity in a single transaction.
/// Useful for bootstrapping a new token vault.
#[derive(Accounts)]
pub struct InitializeAndProvideLiquidity<'info> {
    /// The mint account of the SPL token for the new vault.
    /// CHECK: Verified in instruction logic (is Mint).
    pub token_mint: AccountInfo<'info>,

    /// The `VaultAccount` PDA to be initialized.
    /// Seeds: [b"vault", token_mint_key]
    #[account(
        init,
        payer = liquidity_provider,
        space = 8 + std::mem::size_of::<VaultAccount>(), // Becomes fixed size
        seeds = [b"vault", token_mint.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The state account for the initial liquidity provider.
    #[account(
        init, // Always init, since the vault is new
        payer = liquidity_provider, // Provider pays for their own account
        space = 8 + std::mem::size_of::<ProviderState>(),
        seeds = [b"provider_state", vault.key().as_ref(), liquidity_provider.key().as_ref()],
        bump
    )]
    pub provider_state: Account<'info, ProviderState>,

    /// CHECK: Validated in instruction logic (is TokenAccount).
    #[account(mut)]
    pub provider_token_account: AccountInfo<'info>,

    /// CHECK: Verified in instruction logic (is TokenAccount).
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,

    /// The initial liquidity provider (signer). Pays for account creation.
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// CHECK: Address checked in instruction logic, used for SOL transfer. Must be writable.
    #[account(
        mut,
        address = TREASURY_PUBKEY
    )]
    pub treasury_account: AccountInfo<'info>,

    /// The Solana System Program.
    pub system_program: Program<'info, System>,
    /// The SPL Token Program.
    pub token_program: Program<'info, Token>,
    /// The Rent Sysvar.
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts required to start a new roulette round.
#[derive(Accounts)]
pub struct StartNewRound<'info> {
    /// The global `GameSession` account. Mutable to update round status, number, times, etc.
    #[account(mut, seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// The user initiating the new round (signer).
    #[account(mut)]
    pub starter: Signer<'info>,

    pub system_program: Program<'info, System>, // Kept in case needed for future logic
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

#[derive(Accounts)]
pub struct CloseBets<'info> {
    /// The global `GameSession` account. Mutable to update status and timestamps.
    #[account(mut, seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// The user initiating the closing of bets (signer).
    #[account(mut)]
    pub closer: Signer<'info>,

    pub system_program: Program<'info, System>, // Kept in case needed for future logic
}

/// Accounts required to trigger the random number generation for the current round.
#[derive(Accounts)]
pub struct GetRandom<'info> {
    /// The global `GameSession` account. Mutable to store the winning number and update status.
    #[account(mut, seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// The user initiating the random generation (signer).
    #[account(mut)]
    pub random_initiator: Signer<'info>,
}

/// Accounts required for a player to claim their winnings for the MOST RECENTLY completed round.
/// Uses the player's LATEST bets recorded in their PlayerBets account.
#[derive(Accounts)]
pub struct ClaimMyWinnings<'info> {
    /// The player claiming the winnings (signer). Pays for `claim_record` creation if needed.
    #[account(mut)]
    pub player: Signer<'info>,

    /// The global game session account. Checked to ensure a winning number exists.
    #[account(
        seeds = [b"game_session"],
        bump = game_session.bump,
    )]
    pub game_session: Account<'info, GameSession>, // Needed for winning_number and last_completed_round

    /// The player's bets account, containing their LATEST placed bets, vault, and token mint.
    #[account(
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump = player_bets.bump,
        constraint = player_bets.player == player.key() @ RouletteError::Unauthorized,
    )]
    pub player_bets: Account<'info, PlayerBets>,

    /// The vault corresponding to the LATEST token_mint used by the player in `player_bets`.
    #[account(
        mut,
        seeds = [b"vault", player_bets.token_mint.as_ref()],
        bump = vault.bump,
        constraint = vault.key() == player_bets.vault @ RouletteError::VaultMismatch,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// CHECK: Validated manually + via constraint below.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::InvalidTokenAccount,
    )]
    pub vault_token_account: AccountInfo<'info>,

    /// CHECK: Validated manually (mint, owner).
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + 1 + 1, // Discriminator + claimed: bool + bump: u8
        seeds = [
            b"claim_record",
            player.key().as_ref(),
            // The seed here now uses the last_completed_round from game_session.
            // Verification that this matches the *intended* round_to_claim happens inside the function.
            game_session.last_completed_round.to_le_bytes().as_ref(),
        ],
        bump
    )]
    pub claim_record: Account<'info, ClaimRecord>,

    /// SPL Token Program.
    pub token_program: Program<'info, Token>,
    /// System Program (for creating `claim_record`).
    pub system_program: Program<'info, System>,
    /// Rent Sysvar (for creating `claim_record`).
    pub rent: Sysvar<'info, Rent>,
}