use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Transfer};
use crate::{
    constants::*,
    contexts::*,
    errors::RouletteError,
    events::*,
    state::*,
};

pub fn initialize_player_bets(ctx: Context<InitializePlayerBets>) -> Result<()> {
    msg!("Initializing PlayerBets. Current GameSession status: {:?}", ctx.accounts.game_session.round_status);
    let player_bets = &mut ctx.accounts.player_bets;
    player_bets.player = ctx.accounts.player.key();
    player_bets.round = 0; // Initial round is 0
    player_bets.vault = Pubkey::default(); // Will be set on first bet
    player_bets.token_mint = Pubkey::default(); // Will be set on first bet
    player_bets.bets = Vec::with_capacity(MAX_BETS_PER_ROUND);
    player_bets.bump = ctx.bumps.player_bets;
    msg!("PlayerBets account fields initialized for player {}", ctx.accounts.player.key());
    Ok(())
}

/// Closes the player's PlayerBets account for the current game session PDA structure
/// and returns the rent exemption SOL back to the player.
/// This should only be called when the player is certain they no longer need
/// the account (e.g., finished playing or wants to reset).
pub fn close_player_bets_account(ctx: Context<ClosePlayerBetsAccount>) -> Result<()> {
    let player_key = ctx.accounts.player.key();
    let player_bets_key = ctx.accounts.player_bets.key();
    msg!(
        "PlayerBets account {} for player {} is being closed. Rent SOL will be refunded.",
        player_bets_key,
        player_key
    );

    Ok(())
}


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

    // A max_bet_amount of 0 means the vault is empty or has very little liquidity.
    // In this case, no bets should be allowed. We also check bet.amount > 0 later.
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
    token::transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.player_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: player.to_account_info(),
        }),
        bet_amount
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


pub fn claim_my_winnings(ctx: Context<ClaimMyWinnings>, round_to_claim: u64) -> Result<()> {
    let game_session = &ctx.accounts.game_session;
    let player_bets_account = &mut ctx.accounts.player_bets;
    let vault = &mut ctx.accounts.vault;
    let player_token_account_info = &ctx.accounts.player_token_account;
    let vault_token_account_info = &ctx.accounts.vault_token_account;
    let player_key = ctx.accounts.player.key();
    let program_id = ctx.program_id;

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
    let (expected_claim_record_pda, _) = Pubkey::find_program_address(
        &[
            b"claim_record",
            player_key.as_ref(),
            &round_claimed.to_le_bytes()
        ],
        program_id
    );

    require_keys_eq!(
        player_bets_account.key(),
        expected_claim_record_pda,
        RouletteError::InvalidPlayerBetsAccount
    );

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
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: vault_token_account_info.to_account_info(),
                to: player_token_account_info.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds
        ),
        actual_payout
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
