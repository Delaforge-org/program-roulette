use anchor_lang::prelude::*;

// 1. Declare all our modules
pub mod constants;
pub mod contexts;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

// 2. Make everything from them accessible
use contexts::*;
use state::Bet; // Needed for the place_bet function signature

declare_id!("3qsuZnHizu23tsvx5mc6ZP1tSrEmfNvHp4kMFnWRsBKy");

#[program]
pub mod roulette_game {
    use super::*;

    // ========== VAULT INSTRUCTIONS ==========
    pub fn initialize_and_provide_liquidity(ctx: Context<InitializeAndProvideLiquidity>, amount: u64) -> Result<()> {
        instructions::vault::initialize_and_provide_liquidity(ctx, amount)
    }

    pub fn provide_liquidity(ctx: Context<ProvideLiquidity>, amount: u64) -> Result<()> {
        instructions::vault::provide_liquidity(ctx, amount)
    }

    pub fn withdraw_liquidity(ctx: Context<WithdrawLiquidity>) -> Result<()> {
        instructions::vault::withdraw_liquidity(ctx)
    }

    pub fn withdraw_provider_revenue(ctx: Context<WithdrawProviderRevenue>) -> Result<()> {
        instructions::vault::withdraw_provider_revenue(ctx)
    }

    pub fn withdraw_owner_revenue(ctx: Context<WithdrawOwnerRevenue>) -> Result<()> {
        instructions::vault::withdraw_owner_revenue(ctx)
    }

    // ========== GAME INSTRUCTIONS ==========
    pub fn initialize_game_session(ctx: Context<InitializeGameSession>) -> Result<()> {
        instructions::game::initialize_game_session(ctx)
    }

    pub fn start_new_round(ctx: Context<StartNewRound>) -> Result<()> {
        instructions::game::start_new_round(ctx)
    }

    pub fn close_bets(ctx: Context<CloseBets>) -> Result<()> {
        instructions::game::close_bets(ctx)
    }

    pub fn get_random(ctx: Context<GetRandom>) -> Result<()> {
        instructions::game::get_random(ctx)
    }

    // ========== PLAYER INSTRUCTIONS ==========
    pub fn initialize_player_bets(ctx: Context<InitializePlayerBets>) -> Result<()> {
        instructions::player::initialize_player_bets(ctx)
    }

    pub fn close_player_bets_account(ctx: Context<ClosePlayerBetsAccount>) -> Result<()> {
        instructions::player::close_player_bets_account(ctx)
    }

    pub fn place_bet(ctx: Context<PlaceBets>, bet: Bet) -> Result<()> {
        instructions::player::place_bet(ctx, bet)
    }

    pub fn claim_my_winnings(ctx: Context<ClaimMyWinnings>, round_to_claim: u64) -> Result<()> {
        instructions::player::claim_my_winnings(ctx, round_to_claim)
    }
}