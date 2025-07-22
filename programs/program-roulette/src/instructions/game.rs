use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash;
use crate::{
    constants::*,
    contexts::*,
    errors::RouletteError,
    events::*,
    state::*,
};


pub fn initialize_game_session(ctx: Context<InitializeGameSession>) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    game_session.current_round = 0;
    game_session.round_start_time = 0;
    game_session.round_status = RoundStatus::NotStarted;
    game_session.winning_number = None;
    game_session.bets_closed_timestamp = 0;
    game_session.get_random_timestamp = 0;
    game_session.bump = ctx.bumps.game_session;
    game_session.last_bettor = None; // Initialize last_bettor
    game_session.last_completed_round = 0;
    Ok(())
}


pub fn start_new_round(ctx: Context<StartNewRound>) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        game_session.round_status == RoundStatus::NotStarted ||
            game_session.round_status == RoundStatus::Completed,
        RouletteError::RoundInProgress
    );

    if game_session.round_status == RoundStatus::Completed {
        require!(
            current_time >= game_session.get_random_timestamp.checked_add(MIN_START_NEW_ROUND_DURATION).unwrap_or(i64::MAX),
            RouletteError::TooEarlyToStartNewRound
        );
    }

    game_session.current_round = game_session.current_round
        .checked_add(1)
        .ok_or(RouletteError::ArithmeticOverflow)?;
    game_session.round_start_time = current_time;
    game_session.round_status = RoundStatus::AcceptingBets;
    game_session.bets_closed_timestamp = 0;
    game_session.get_random_timestamp = 0;
    game_session.last_bettor = None; // Reset last bettor for the new round

    emit!(RoundStarted {
        round: game_session.current_round,
        starter: *ctx.accounts.starter.key,
        start_time: current_time,
    });
    Ok(())
}


pub fn close_bets(ctx: Context<CloseBets>) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    require!(
        game_session.round_status == RoundStatus::AcceptingBets,
        RouletteError::BetsNotAccepted
    );
    require!(
        game_session.last_bettor.is_some(),
        RouletteError::CannotCloseBetsWithoutBets
    );
    require!(
        current_time >=
            game_session.round_start_time.checked_add(MIN_ROUND_DURATION).unwrap_or(i64::MAX),
        RouletteError::TooEarlyToClose
    );

    game_session.round_status = RoundStatus::BetsClosed;
    game_session.bets_closed_timestamp = current_time;

    emit!(BetsClosed {
        round: game_session.current_round,
        closer: *ctx.accounts.closer.key,
        close_time: current_time,
    });
    Ok(())
}


pub fn get_random(ctx: Context<GetRandom>) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    let current_slot = clock.slot;

    require!(
        game_session.round_status == RoundStatus::BetsClosed,
        RouletteError::RandomBeforeClosing
    );

    require!(
        current_time >= game_session.bets_closed_timestamp.checked_add(MIN_BETS_CLOSED_DURATION).unwrap_or(i64::MAX),
        RouletteError::TooEarlyToGetRandom
    );

    require!(game_session.last_bettor.is_some(), RouletteError::NoBetsPlacedInRound);
    let last_bettor_key = game_session.last_bettor.unwrap();

    // Generate random number using SHA256
    let hash_input_bytes: &[&[u8]] = &[
        &last_bettor_key.to_bytes()[..],
        &current_time.to_le_bytes()[..],
        &current_slot.to_le_bytes()[..],
    ];
    let hash_result_obj = hash::hashv(hash_input_bytes);
    let hash_bytes = hash_result_obj.to_bytes();
    let hash_prefix_u64 = u64::from_le_bytes(hash_bytes[0..8].try_into().unwrap());
    let winning_number = (hash_prefix_u64 % 37) as u8; // Modulo 37 for 0-36

    msg!(
        "Round {} | Hash {:?} | Winning Number {}",
        game_session.current_round,
        hash_bytes,
        winning_number
    );

    // Update game session
    game_session.winning_number = Some(winning_number);
    game_session.round_status = RoundStatus::Completed;
    game_session.last_completed_round = game_session.current_round;
    game_session.get_random_timestamp = current_time;

    emit!(RandomGenerated {
        round: game_session.current_round,
        initiator: *ctx.accounts.random_initiator.key,
        winning_number: winning_number,
        generation_time: current_time,
        slot: current_slot,
        last_bettor: last_bettor_key,
        hash_result: hash_bytes,
        hash_prefix_u64: hash_prefix_u64,
    });

    Ok(())
}