use anchor_lang::prelude::*;
use anchor_spl::token::{Token};
use crate::{
    constants::*,
    errors::RouletteError,
    state::*,
};

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
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"game_session"], 
        bump = game_session.bump,
        constraint = authority.key() == game_session.authority @ RouletteError::AdminOnly
    )]
    pub game_session: Account<'info, GameSession>,

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

/// Accounts required for the program authority to distribute the payout reserve.
#[derive(Accounts)]
pub struct DistributePayoutReserve<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"game_session"],
        bump = game_session.bump,
        constraint = authority.key() == game_session.authority @ RouletteError::AdminOnly
    )]
    pub game_session: Account<'info, GameSession>,

    /// The vault account to distribute revenue from.
    #[account(
        mut,
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The mint account for the token.
    /// CHECK: Used for PDA seeds validation.
    pub token_mint: AccountInfo<'info>,
}