use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use anchor_lang::system_program;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, SetAuthority, TransferChecked};
use anchor_spl::token_2022::spl_token_2022::instruction::AuthorityType;
use crate::{
    constants::*,
    errors::RouletteError,
    events::*,
    state::*,
};

// =================================================================================================
// Vault Initialization and Provide Liquidity
// =================================================================================================

pub fn initialize_and_provide_liquidity(
    ctx: Context<InitializeAndProvideLiquidity>,
    amount: u64
) -> Result<()> {
    // Anchor's constraints now handle deserialization and validation automatically.

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.liquidity_provider.to_account_info(),
                to: ctx.accounts.treasury_account.to_account_info(),
            },
        ),
        CREATE_VAULT_FEE_SOL_LAMPORTS
    )?;

    // Initialize vault state (simplified, no vectors)
    let vault = &mut ctx.accounts.vault;
    vault.token_mint = ctx.accounts.token_mint.key();
    vault.token_account = ctx.accounts.vault_token_account.key();
    vault.bump = ctx.bumps.vault;
    vault.owner_reward = 0;
    vault.reward_per_share_index = 0;
    
    // Initialize the first provider's state
    let provider_state = &mut ctx.accounts.provider_state;
    provider_state.vault = vault.key();
    provider_state.provider = ctx.accounts.liquidity_provider.key();
    provider_state.unclaimed_rewards = 0;
    provider_state.reward_per_share_index_last_claimed = 0; // Starts at 0
    provider_state.bump = ctx.bumps.provider_state;

    // Transfer initial liquidity
    token_interface::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), TransferChecked {
            from: ctx.accounts.provider_token_account.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.liquidity_provider.to_account_info(),
        }),
        amount,
        ctx.accounts.token_mint.decimals,
    )?;

    // Transfer ownership of the vault token account to the vault PDA
    token_interface::set_authority(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SetAuthority {
                current_authority: ctx.accounts.liquidity_provider.to_account_info(),
                account_or_mint: ctx.accounts.vault_token_account.to_account_info(),
            },
        ),
        AuthorityType::AccountOwner,
        Some(vault.key()),
    )?;

    // Update vault and provider state with the amount
    vault.total_liquidity = amount;
    vault.total_provider_capital = amount;
    provider_state.amount = amount;

    emit!(LiquidityProvided {
        provider: *ctx.accounts.liquidity_provider.key,
        token_mint: vault.token_mint,
        amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeAndProvideLiquidity<'info> {
    /// The mint account of the SPL token for the new vault.
    pub token_mint: InterfaceAccount<'info, Mint>,

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

    /// The provider's token account. It must be for the same mint as `token_mint`.
    #[account(
        mut,
        constraint = provider_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount
    )]
    pub provider_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The token account that will become the vault's token account.
    /// It must also be for the same mint.
    #[account(
        mut,
        constraint = vault_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount,
        constraint = vault_token_account.key() != provider_token_account.key() @ RouletteError::DuplicateTokenAccount
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The initial liquidity provider (signer). Pays for account creation.
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The treasury account that receives the vault creation fee.
    #[account(
        mut,
        address = TREASURY_PUBKEY
    )]
    pub treasury_account: SystemAccount<'info>,

    /// The Solana System Program.
    pub system_program: Program<'info, System>,
    /// The SPL Token Program.
    pub token_program: Interface<'info, TokenInterface>,
    /// The Rent Sysvar.
    pub rent: Sysvar<'info, Rent>,
}

// =================================================================================================
// Provide Liquidity (In already existing vault)
// =================================================================================================

pub fn provide_liquidity(ctx: Context<ProvideLiquidity>, amount: u64) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.token_mint.key(),
        ctx.accounts.vault.token_mint,
        RouletteError::InvalidTokenAccount
    );
    require!(amount > 0, RouletteError::AmountMustBeGreaterThanZero); // Can't provide 0 liquidity

    let vault = &mut ctx.accounts.vault;
    let provider_state = &mut ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward update logic ---
    let newly_earned_reward = calculate_newly_earned_rewards(provider_state, current_reward_index)?;
    provider_state.unclaimed_rewards = provider_state.unclaimed_rewards
        .checked_add(newly_earned_reward)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    // --- End of reward update logic ---

    // Transfer liquidity
    token_interface::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), TransferChecked {
            from: ctx.accounts.provider_token_account.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.liquidity_provider.to_account_info(),
        }),
        amount,
        ctx.accounts.token_mint.decimals,
    )?;

    // If the provider state account is being initialized, set its fixed data.
    if provider_state.vault == Pubkey::default() {
        provider_state.vault = vault.key();
        provider_state.provider = ctx.accounts.liquidity_provider.key();
        provider_state.bump = ctx.bumps.provider_state;
    }

    // Update vault state
    vault.total_liquidity = vault.total_liquidity
        .checked_add(amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    vault.total_provider_capital = vault.total_provider_capital
        .checked_add(amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Update provider state
    provider_state.amount = provider_state.amount
        .checked_add(amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    
    // Set the checkpoint to the current index for the next calculation.
    provider_state.reward_per_share_index_last_claimed = current_reward_index;

    emit!(LiquidityProvided {
        provider: ctx.accounts.liquidity_provider.key(),
        token_mint: vault.token_mint,
        amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

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
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// The user's state account for this vault. Created if it doesn't exist.
    #[account(
        init_if_needed,
        payer = liquidity_provider,
        space = 8 + std::mem::size_of::<ProviderState>(),
        seeds = [b"provider_state", vault.key().as_ref(), liquidity_provider.key().as_ref()],
        bump
    )]
    pub provider_state: Account<'info, ProviderState>,

    /// The provider's token account, constrained to the correct mint.
    #[account(
        mut,
        constraint = provider_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount
    )]
    pub provider_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The vault's token account. Constraint ensures it matches the vault's stored `token_account`.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::VaultMismatch,
        constraint = vault_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount,
        constraint = vault_token_account.key() != provider_token_account.key() @ RouletteError::DuplicateTokenAccount
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The liquidity provider (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Interface<'info, TokenInterface>,
    /// The Solana System Program.
    pub system_program: Program<'info, System>,
}

// =================================================================================================
// Withdraw Liquidity
// =================================================================================================

pub fn withdraw_liquidity(ctx: Context<WithdrawLiquidity>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let provider_state = &ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward calculation ---
    let newly_earned_reward = calculate_newly_earned_rewards(provider_state, current_reward_index)?;
    let final_unclaimed_rewards = provider_state.unclaimed_rewards
        .checked_add(newly_earned_reward)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    // --- End of reward calculation ---

    // Determine the total amount to withdraw: all capital + all rewards.
    let total_capital_to_withdraw = provider_state.amount;
    let total_withdrawal_amount = total_capital_to_withdraw
        .checked_add(final_unclaimed_rewards)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    if total_withdrawal_amount > 0 {
        require!(
            vault.total_liquidity >= total_withdrawal_amount,
            RouletteError::InsufficientLiquidity
        );

        // Transfer tokens back to provider
        let seeds = &[b"vault".as_ref(), vault.token_mint.as_ref(), &[vault.bump]];
        let signer_seeds = &[&seeds[..]];
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.provider_token_account.to_account_info(),
                    authority: vault.to_account_info(),
                },
                signer_seeds
            ),
            total_withdrawal_amount,
            ctx.accounts.token_mint.decimals,
        )?;

        // Update vault global state
        vault.total_liquidity = vault.total_liquidity
            .checked_sub(total_withdrawal_amount)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }
    
    vault.total_provider_capital = vault.total_provider_capital
        .checked_sub(total_capital_to_withdraw) // Only subtract the capital part
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // provider_state account is automatically closed by Anchor via the `close` constraint.

    emit!(LiquidityWithdrawn {
        provider: ctx.accounts.liquidity_provider.key(),
        token_mint: vault.token_mint,
        amount: total_capital_to_withdraw, // Emitting the capital amount withdrawn
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
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

    /// The mint account for the token.
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// The provider's token account to receive the funds.
    #[account(
        mut,
        constraint = provider_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount,
        constraint = provider_token_account.key() != vault_token_account.key() @ RouletteError::DuplicateTokenAccount
    )]
    pub provider_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The vault's token account.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::VaultMismatch,
        constraint = vault_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The liquidity provider requesting the withdrawal (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Interface<'info, TokenInterface>,
}

// =================================================================================================
// Withdraw Provider Revenue
// =================================================================================================

pub fn withdraw_provider_revenue(ctx: Context<WithdrawProviderRevenue>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let provider_state = &mut ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward calculation ---
    let newly_earned_reward = calculate_newly_earned_rewards(provider_state, current_reward_index)?;
    provider_state.unclaimed_rewards = provider_state.unclaimed_rewards
        .checked_add(newly_earned_reward)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    // --- End of reward calculation ---

    let total_rewards_to_claim = provider_state.unclaimed_rewards;

    require!(total_rewards_to_claim > 0, RouletteError::NoReward);
    require!(
        vault.total_liquidity >= total_rewards_to_claim,
        RouletteError::InsufficientLiquidity
    );

    // Transfer rewards to the provider
    let seeds = &[b"vault".as_ref(), vault.token_mint.as_ref(), &[vault.bump]];
    let signer_seeds = &[&seeds[..]];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.provider_token_account.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        total_rewards_to_claim,
        ctx.accounts.token_mint.decimals,
    )?;

    // Update vault global state
    vault.total_liquidity = vault.total_liquidity
        .checked_sub(total_rewards_to_claim)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    
    // Reset provider's claimed rewards and update checkpoint
    provider_state.unclaimed_rewards = 0;
    provider_state.reward_per_share_index_last_claimed = current_reward_index;

    emit!(ProviderRevenueWithdrawn {
        provider: ctx.accounts.liquidity_provider.key(),
        token_mint: vault.token_mint,
        amount: total_rewards_to_claim,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

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
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// The provider's token account to receive rewards.
    #[account(
        mut,
        constraint = provider_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount,
        constraint = provider_token_account.key() != vault_token_account.key() @ RouletteError::DuplicateTokenAccount
    )]
    pub provider_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The vault's token account.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::VaultMismatch,
        constraint = vault_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The liquidity provider requesting the withdrawal (signer).
    #[account(mut)]
    pub liquidity_provider: Signer<'info>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Interface<'info, TokenInterface>,
}

// =================================================================================================
// Withdraw Owner Revenue
// =================================================================================================

pub fn withdraw_owner_revenue(ctx: Context<WithdrawOwnerRevenue>) -> Result<()> {
    // Anchor's constraints now handle token_mint and treasury account validation.
    let vault = &mut ctx.accounts.vault;
    let reward_amount = vault.owner_reward;

    require!(reward_amount > 0, RouletteError::NoReward);
    require!(vault.total_liquidity >= reward_amount, RouletteError::InsufficientLiquidity);

    let seeds = &[b"vault".as_ref(), vault.token_mint.as_ref(), &[vault.bump]];
    let signer_seeds = &[&seeds[..]];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.owner_treasury_token_account.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        reward_amount,
        ctx.accounts.token_mint.decimals,
    )?;

    vault.total_liquidity = vault.total_liquidity
        .checked_sub(reward_amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    
    vault.owner_reward = 0;

    Ok(())
}

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
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// The treasury's token account to receive the funds.
    #[account(
        mut,
        constraint = owner_treasury_token_account.mint == token_mint.key() @ RouletteError::TreasuryAccountMintMismatch,
        constraint = owner_treasury_token_account.owner == TREASURY_PUBKEY @ RouletteError::InvalidTreasuryAccountOwner
    )]
    pub owner_treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The vault's token account.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::VaultMismatch,
        constraint = vault_token_account.mint == token_mint.key() @ RouletteError::InvalidTokenAccount
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The SPL Token Program, needed for the token transfer CPI.
    pub token_program: Interface<'info, TokenInterface>,
}

// =================================================================================================
// Distribute Payout Reserve
// =================================================================================================

pub fn distribute_payout_reserve(ctx: Context<DistributePayoutReserve>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    // 1. Calculate the payout reserve.
    let payout_reserve = vault.total_liquidity
        .checked_sub(vault.total_provider_capital)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Ensure there's a reserve to distribute.
    require!(payout_reserve > 0, RouletteError::NoReward);

    // 2. Determine the amount to distribute (50% of the reserve).
    let amount_to_distribute = payout_reserve
        .checked_div(2)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    require!(amount_to_distribute > 0, RouletteError::NoReward);

    // 3. Split the amount 50/50.
    let owner_share = amount_to_distribute
        .checked_div(2)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    let providers_share = amount_to_distribute
        .checked_sub(owner_share)
        .ok_or(RouletteError::ArithmeticOverflow)?; // To avoid dust loss from integer division

    // 4. Distribute the shares.
    // Add to owner's rewards.
    vault.owner_reward = vault.owner_reward
        .checked_add(owner_share)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Distribute to providers via the reward index.
    if vault.total_provider_capital > 0 {
        let reward_index_increase = (providers_share as u128)
            .checked_mul(REWARD_PRECISION)
            .ok_or(RouletteError::ArithmeticOverflow)?
            .checked_div(vault.total_provider_capital as u128)
            .ok_or(RouletteError::ArithmeticOverflow)?;

        vault.reward_per_share_index = vault.reward_per_share_index
            .checked_add(reward_index_increase)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }

    emit!(PayoutReserveDistributed {
        token_mint: vault.token_mint,
        amount_distributed: amount_to_distribute,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

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
    pub token_mint: InterfaceAccount<'info, Mint>,
}

// =================================================================================================
// Get Unclaimed Rewards (Read-Only via Simulation)
// =================================================================================================

pub fn get_unclaimed_rewards(ctx: Context<GetUnclaimedRewards>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let provider_state = &ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // Use the helper to calculate rewards earned since the last action.
    let newly_earned_reward = calculate_newly_earned_rewards(provider_state, current_reward_index)?;
    
    // Add them to the already accumulated (but not yet claimed) rewards.
    let total_unclaimed_rewards = provider_state.unclaimed_rewards
        .checked_add(newly_earned_reward)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Set the return data so the client can read it from the simulation result.
    set_return_data(&total_unclaimed_rewards.to_le_bytes());

    Ok(())
}

#[derive(Accounts)]
pub struct GetUnclaimedRewards<'info> {
    /// The vault account.
    #[account(
        seeds = [b"vault", token_mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultAccount>,

    /// The provider's state account.
    #[account(
        constraint = provider_state.vault == vault.key() @ RouletteError::VaultMismatch,
        seeds = [b"provider_state", vault.key().as_ref(), provider.key().as_ref()],
        bump = provider_state.bump
    )]
    pub provider_state: Account<'info, ProviderState>,
    
    /// The mint account for the token.
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: The provider's wallet account. No signature is required as this is a read-only function.
    /// It's used solely for deriving the `provider_state` PDA and no data is read from it.
    pub provider: UncheckedAccount<'info>,
}

// A private helper function to calculate rewards without modifying state.
fn calculate_newly_earned_rewards(
    provider_state: &ProviderState,
    current_reward_index: u128
) -> Result<u64> {
    let last_claimed_index = provider_state.reward_per_share_index_last_claimed;
    let provider_capital = provider_state.amount;

    if last_claimed_index < current_reward_index && provider_capital > 0 {
        let index_delta = current_reward_index
            .checked_sub(last_claimed_index)
            .ok_or(RouletteError::ArithmeticOverflow)?;

        let newly_earned_reward = (index_delta)
            .checked_mul(provider_capital as u128)
            .ok_or(RouletteError::ArithmeticOverflow)?
            .checked_div(REWARD_PRECISION)
            .ok_or(RouletteError::ArithmeticOverflow)?;

        // Ensure the cast is safe, then convert the error type to what Anchor expects.
        u64::try_from(newly_earned_reward).map_err(|_| RouletteError::ArithmeticOverflow.into())
    } else {
        Ok(0)
    }
}