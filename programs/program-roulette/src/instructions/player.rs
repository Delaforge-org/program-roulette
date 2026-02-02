use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface, TransferChecked, Mint};
use crate::{
    constants::*,
    errors::RouletteError,
    events::*,
    state::*,
};

// =================================================================================================
// Player Initialization
// =================================================================================================

pub fn initialize_player_bets(ctx: Context<InitializePlayerBets>) -> Result<()> {
    let player_bets = &mut ctx.accounts.player_bets;
    player_bets.player = ctx.accounts.player.key();
    player_bets.round = 0; // Initial round is 0
    player_bets.vault = Pubkey::default(); // Will be set on first bet
    player_bets.token_mint = Pubkey::default(); // Will be set on first bet
    player_bets.bets = Vec::with_capacity(MAX_BETS_PER_ROUND);
    player_bets.bump = ctx.bumps.player_bets;
    Ok(())
}

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

// =================================================================================================
// Player Close Account
// =================================================================================================

pub fn close_player_bets_account(ctx: Context<ClosePlayerBetsAccount>) -> Result<()> {
    let _player_key = ctx.accounts.player.key();
    let _player_bets_key = ctx.accounts.player_bets.key();

    Ok(())
}

#[derive(Accounts)]
pub struct ClosePlayerBetsAccount<'info> {
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

// =================================================================================================
// Player Place Bet
// =================================================================================================

pub fn place_bet(ctx: Context<PlaceBets>, bet: Bet) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let player_bets = &mut ctx.accounts.player_bets;
    let player = &ctx.accounts.player;
    let vault_key = ctx.accounts.vault.key();
    let vault = &mut ctx.accounts.vault;

    require!(
        game_session.round_status == RoundStatus::AcceptingBets,
        RouletteError::BetsNotAccepted
    );
    require!(bet.bet_type <= BET_TYPE_MAX, RouletteError::InvalidBet);

    // Check that the bet amount does not exceed 3% of the vault's total liquidity.
    let max_bet_amount = (vault.total_liquidity as u128)
        .checked_mul(MAX_BET_PERCENTAGE as u128)
        .ok_or(RouletteError::ArithmeticOverflow)?
        .checked_div(MAX_BET_PERCENTAGE_DIVISOR as u128)
        .ok_or(RouletteError::ArithmeticOverflow)? as u64;

    require!(
        bet.amount <= max_bet_amount,
        RouletteError::BetAmountExceedsLimit
    );

    // Handle first bet in round / round switch
    if player_bets.round != game_session.current_round {
        player_bets.bets.clear(); // Clear previous round's bets
        player_bets.round = game_session.current_round;
        player_bets.vault = vault_key; // Set vault for this round
        player_bets.token_mint = vault.token_mint; // Set mint for this round
        if player_bets.player == Pubkey::default() {
            // Ensure player is set (first ever call)
            player_bets.player = *player.key;
        }
    } else {
        // Subsequent bet, ensure vault hasn't changed
        require_keys_eq!(vault_key, player_bets.vault, RouletteError::VaultMismatch);
    }

    // Check bet vector capacity
    if player_bets.bets.len() >= MAX_BETS_PER_ROUND {
        return err!(RouletteError::InvalidNumberOfBets); // Or MaxBetsInAccountReached
    }

    // Transfer bet amount
    let bet_amount = bet.amount;
    require!(bet_amount > 0, RouletteError::InvalidBet); // Bet amount cannot be zero
    token_interface::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), TransferChecked {
            from: ctx.accounts.player_token_account.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: player.to_account_info(),
        }),
        bet_amount,
        ctx.accounts.token_mint.decimals,
    )?;

    // Update vault liquidity
    vault.total_liquidity = vault.total_liquidity
        .checked_add(bet_amount)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Distribute rewards
    let provider_revenue = bet_amount / PROVIDER_DIVISOR;
    let owner_revenue = bet_amount / OWNER_DIVISOR;
    vault.owner_reward = vault.owner_reward
        .checked_add(owner_revenue)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    // Update reward index
    if vault.total_provider_capital > 0 {
        let provider_revenue_u128 = provider_revenue as u128;
        let increment = provider_revenue_u128
            .checked_mul(REWARD_PRECISION)
            .ok_or(RouletteError::ArithmeticOverflow)?
            .checked_div(vault.total_provider_capital as u128)
            .ok_or(RouletteError::ArithmeticOverflow)?;
        vault.reward_per_share_index = vault.reward_per_share_index
            .checked_add(increment)
            .ok_or(RouletteError::ArithmeticOverflow)?;
    }

    // Add bet to player's account
    player_bets.bets.push(bet.clone());

    // Record the last bettor
    game_session.last_bettor = Some(*player.key);

    emit!(BetPlaced {
        player: *player.key,
        token_mint: vault.token_mint,
        round: game_session.current_round,
        bet,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct PlaceBets<'info> {
    #[account(mut)]
    pub vault: Account<'info, VaultAccount>,

    #[account(mut, seeds = [b"game_session"], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// CHECK: Validated in instruction logic (is TokenAccount).
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    /// CHECK: Validated by the constraint `vault_token_account.key() == vault.token_account`.
    #[account(
        mut,
        constraint = vault_token_account.key() == vault.token_account @ RouletteError::InvalidTokenAccount,
    )]
    pub vault_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"player_bets", game_session.key().as_ref(), player.key().as_ref()],
        bump = player_bets.bump // Verify bump of existing account
    )]
    pub player_bets: Account<'info, PlayerBets>,

    /// The mint of the token. Needed for transfer_checked and decimals.
    #[account(address = vault.token_mint @ RouletteError::InvalidTokenAccount)]
    pub token_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

// =================================================================================================
// Player Claim Winnings
// =================================================================================================

pub fn claim_my_winnings(ctx: Context<ClaimMyWinnings>, round_to_claim: u64) -> Result<()> {
    let game_session = &ctx.accounts.game_session;
    let player_bets_account = &mut ctx.accounts.player_bets;
    let vault = &mut ctx.accounts.vault;
    let player_token_account_info = &ctx.accounts.player_token_account;
    let vault_token_account_info = &ctx.accounts.vault_token_account;
    let player_key = ctx.accounts.player.key();

    let round_claimed = round_to_claim;

    require!(
        round_claimed <= game_session.last_completed_round,
        RouletteError::ClaimRoundMismatchOrNotCompleted
    );

    require!(
        round_claimed == game_session.last_completed_round && game_session.winning_number.is_some(),
        RouletteError::ClaimRoundMismatchOrNotCompleted
    );

    require!(
        player_bets_account.round == round_claimed,
        RouletteError::BetsRoundMismatch
    );

    let winning_number = game_session.winning_number.unwrap();

    //New check: 
    require!(
        player_bets_account.claimed_round < round_to_claim,
        RouletteError::Unauthorized
    );

    let player_token_account: TokenAccount = TokenAccount::try_deserialize(
        &mut &player_token_account_info.data.borrow()[..]
    )?;
    let vault_token_account: TokenAccount = TokenAccount::try_deserialize(
        &mut &vault_token_account_info.data.borrow()[..]
    )?;
    require_keys_eq!(
        vault_token_account_info.key(),
        vault.token_account,
        RouletteError::InvalidTokenAccount
    );
    require_eq!(vault_token_account.mint, vault.token_mint, RouletteError::InvalidTokenAccount);
    require_eq!(player_token_account.mint, vault.token_mint, RouletteError::InvalidTokenAccount);
    require_keys_eq!(
        player_token_account.owner,
        player_key,
        RouletteError::InvalidTokenAccount
    );

    let mut total_payout: u64 = 0;
    for bet in player_bets_account.bets.iter() {
        if PlayerBets::is_bet_winner(bet.bet_type, &bet.numbers, winning_number) {
            let payout_multiplier = PlayerBets::calculate_payout_multiplier(bet.bet_type);
            let payout_for_bet = bet.amount
                .checked_mul(payout_multiplier)
                .ok_or(RouletteError::ArithmeticOverflow)?;
            total_payout = total_payout
                .checked_add(payout_for_bet)
                .ok_or(RouletteError::ArithmeticOverflow)?;
        }
    }

    let actual_payout = total_payout.min(vault.total_liquidity);

    if total_payout == 0 {
         player_bets_account.claimed_round = round_to_claim;
         return err!(RouletteError::NoWinningsFound);
    }

    require!(actual_payout > 0, RouletteError::InsufficientLiquidity);

    let seeds = &[b"vault".as_ref(), vault.token_mint.as_ref(), &[vault.bump]];
    let signer_seeds = &[&seeds[..]];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: vault_token_account_info.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: player_token_account_info.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        actual_payout,
        ctx.accounts.token_mint.decimals,
    )?;

    vault.total_liquidity = vault.total_liquidity
        .checked_sub(actual_payout)
        .ok_or(RouletteError::ArithmeticOverflow)?;

    if total_payout > actual_payout && vault.total_liquidity == 0 {
        // Consider if this specific alert should be an event if it's critical for off-chain monitoring
    }

    player_bets_account.claimed_round = round_to_claim;

    emit!(WinningsClaimed {
        round: round_claimed,
        player: player_key,
        token_mint: vault.token_mint,
        amount: actual_payout,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

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

    /// The mint of the token. Needed for transfer_checked and decimals.
    #[account(address = vault.token_mint @ RouletteError::InvalidTokenAccount)]
    pub token_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}
