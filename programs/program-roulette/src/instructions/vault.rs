use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, TokenAccount, Transfer, SetAuthority};
use anchor_spl::token::spl_token::instruction::AuthorityType;
use crate::{
    constants::*,
    contexts::*,
    errors::RouletteError,
    events::*,
};

pub fn initialize_and_provide_liquidity(
    ctx: Context<InitializeAndProvideLiquidity>,
    amount: u64
) -> Result<()> {
    // Manual deserialization and validation
    let provider_token_info = &ctx.accounts.provider_token_account;
    let vault_token_info = &ctx.accounts.vault_token_account;
    let _provider_token_account: TokenAccount = TokenAccount::try_deserialize(
        &mut &provider_token_info.data.borrow()[..]
    )?;
    let _vault_token_account: TokenAccount = TokenAccount::try_deserialize(
        &mut &vault_token_info.data.borrow()[..]
    )?;
    let mint_info = &ctx.accounts.token_mint;
    let _mint: Mint = Mint::try_deserialize(&mut &mint_info.data.borrow()[..])?;
    require_eq!(
        _provider_token_account.mint,
        mint_info.key(),
        RouletteError::InvalidTokenAccount
    );
    require_eq!(_vault_token_account.mint, mint_info.key(), RouletteError::InvalidTokenAccount);

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
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
    provider_state.amount = 0;
    provider_state.unclaimed_rewards = 0;
    provider_state.reward_per_share_index_last_claimed = 0; // Starts at 0
    provider_state.bump = ctx.bumps.provider_state;

    // Transfer initial liquidity
    token::transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.provider_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        }),
        amount
    )?;

    // Transfer ownership of the vault token account to the vault PDA
    token::set_authority(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SetAuthority {
                current_authority: ctx.accounts.authority.to_account_info(),
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

pub fn provide_liquidity(ctx: Context<ProvideLiquidity>, amount: u64) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.token_mint.key(),
        ctx.accounts.vault.token_mint,
        RouletteError::InvalidTokenAccount
    );
    require!(amount > 0, RouletteError::InvalidBet); // Can't provide 0 liquidity

    let vault = &mut ctx.accounts.vault;
    let provider_state = &mut ctx.accounts.provider_state;
    let liquidity_provider = &ctx.accounts.liquidity_provider;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward update logic ---
    // Update rewards based on capital *before* adding the new amount.
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

        provider_state.unclaimed_rewards = provider_state.unclaimed_rewards
            .checked_add(newly_earned_reward as u64)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }
    // --- End of reward update logic ---

    // Transfer liquidity
    token::transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.provider_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: liquidity_provider.to_account_info(),
        }),
        amount
    )?;

    // If the provider state account is being initialized, set its fixed data.
    if provider_state.vault == Pubkey::default() {
        provider_state.vault = vault.key();
        provider_state.provider = liquidity_provider.key();
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
        provider: liquidity_provider.key(),
        token_mint: vault.token_mint,
        amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub fn withdraw_liquidity(ctx: Context<WithdrawLiquidity>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let provider_state = &ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward calculation ---
    // Calculate any final rewards earned since the last action.
    let last_claimed_index = provider_state.reward_per_share_index_last_claimed;
    let provider_capital = provider_state.amount;
    let mut final_unclaimed_rewards = provider_state.unclaimed_rewards;

    if last_claimed_index < current_reward_index && provider_capital > 0 {
        let index_delta = current_reward_index
            .checked_sub(last_claimed_index)
            .ok_or(RouletteError::ArithmeticOverflow)?;

        let newly_earned_reward = (index_delta)
            .checked_mul(provider_capital as u128)
            .ok_or(RouletteError::ArithmeticOverflow)?
            .checked_div(REWARD_PRECISION)
            .ok_or(RouletteError::ArithmeticOverflow)?;

        final_unclaimed_rewards = final_unclaimed_rewards
            .checked_add(newly_earned_reward as u64)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }
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
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.provider_token_account.to_account_info(),
                    authority: vault.to_account_info(),
                },
                signer_seeds
            ),
            total_withdrawal_amount
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

pub fn withdraw_provider_revenue(ctx: Context<WithdrawProviderRevenue>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let provider_state = &mut ctx.accounts.provider_state;
    let current_reward_index = vault.reward_per_share_index;

    // --- Start of reward calculation ---
    // Calculate any final rewards earned since the last action.
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

        provider_state.unclaimed_rewards = provider_state.unclaimed_rewards
            .checked_add(newly_earned_reward as u64)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }
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
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.provider_token_account.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        total_rewards_to_claim
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

pub fn withdraw_owner_revenue(ctx: Context<WithdrawOwnerRevenue>) -> Result<()> {
    // Verify that token_mint matches vault.token_mint
    require_keys_eq!(
        ctx.accounts.token_mint.key(),
        ctx.accounts.vault.token_mint,
        RouletteError::InvalidTokenAccount
    );

    let vault = &mut ctx.accounts.vault;
    let treasury_token_account_info = &ctx.accounts.owner_treasury_token_account;
    let treasury_spl_token_account = TokenAccount::try_deserialize(
        &mut &treasury_token_account_info.data.borrow()[..]
    )?;

    require_keys_eq!(
        treasury_spl_token_account.owner,
        TREASURY_PUBKEY,
        RouletteError::InvalidTreasuryAccountOwner
    );
    require_eq!(
        treasury_spl_token_account.mint,
        vault.token_mint,
        RouletteError::TreasuryAccountMintMismatch
    );

    let reward_amount = vault.owner_reward;
    require!(reward_amount > 0, RouletteError::NoReward);
    require!(vault.total_liquidity >= reward_amount, RouletteError::InsufficientLiquidity);

    let seeds = &[b"vault".as_ref(), vault.token_mint.as_ref(), &[vault.bump]];
    let signer_seeds = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: treasury_token_account_info.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        reward_amount
    )?;

    vault.total_liquidity = vault.total_liquidity
        .checked_sub(reward_amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    
    vault.owner_reward = 0;

    Ok(())
}