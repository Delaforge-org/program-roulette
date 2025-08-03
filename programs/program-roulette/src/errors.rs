use anchor_lang::prelude::*;

#[error_code]
pub enum RouletteError {
    #[msg("Arithmetic overflow error during calculation.")]
    ArithmeticOverflow,
    #[msg("Maximum number of bets per round per player reached.")]
    InvalidNumberOfBets,
    #[msg("Insufficient funds in the player's token account for the bet.")]
    InsufficientFunds,
    #[msg("Insufficient liquidity in the vault to cover payout or withdrawal.")]
    InsufficientLiquidity,
    #[msg("Unauthorized: Signer does not have the required permissions.")]
    Unauthorized,
    #[msg("No reward available for withdrawal (for LPs or owner).")]
    NoReward,
    #[msg("Liquidity withdrawal must match the exact total amount provided and not yet withdrawn.")]
    MustWithdrawExactAmount,
    #[msg("Invalid bet type or numbers provided.")]
    InvalidBet,
    #[msg("The bet amount exceeds the maximum limit allowed.")]
    BetAmountExceedsLimit,
    #[msg("Cannot start a new round while one is already in progress.")]
    RoundInProgress,
    #[msg("Bets cannot be placed as the round is not in the 'AcceptingBets' status.")]
    BetsNotAccepted,
    #[msg("The current round status does not allow this operation.")]
    InvalidRoundStatus,
    #[msg("Player has no bets recorded for this round.")]
    NoBetsInRound,
    #[msg("The global GameSession account was not found or is not initialized.")]
    GameSessionNotFound,
    #[msg("The provided reward token mint or account does not match the configured reward mint.")]
    InvalidRewardToken,
    #[msg("The vault specified does not match the vault associated with the PlayerBets account or expected PDA.")]
    VaultMismatch,
    #[msg("Cannot generate the random number before the betting phase is closed.")]
    RandomBeforeClosing,
    #[msg("The random number for this round has already been generated.")]
    RandomAlreadyGenerated,
    #[msg("The provided PlayerBets account is invalid or does not match expectations.")]
    InvalidPlayerBetsAccount,
    #[msg("Game session account is already initialized.")]
    AlreadyInitialized,
    #[msg("Cannot generate random number because no bets were placed in this round.")]
    NoBetsPlacedInRound,
    #[msg("Cannot close bets if no bets were placed in the round.")]
    CannotCloseBetsWithoutBets,
    #[msg("The vault's token account is not owned by the vault PDA.")]
    InvalidTokenAccountOwner,
    #[msg("Derived vault PDA does not match the provided account.")]
    VaultPDAMismatch,
    #[msg("Invalid SPL token account provided (e.g., wrong mint, owner, or not initialized).")]
    InvalidTokenAccount,
    #[msg("Attempting to claim winnings for a round where the winning number is not available.")]
    ClaimRoundMismatchOrNotCompleted,
    #[msg("No winnings found for the player in the specified round (claim attempted).")]
    NoWinningsFound,
    #[msg("Owner of the provided treasury token account is invalid.")]
    InvalidTreasuryAccountOwner,
    #[msg("Mint of the provided treasury token account does not match the vault's token mint.")]
    TreasuryAccountMintMismatch,
    #[msg("Player bets are from a different round than the one being claimed.")]
    BetsRoundMismatch,
    #[msg("Maximum number of liquidity providers for this vault has been reached.")]
    ProviderLimitReached,
    #[msg("Only the game authority can perform this operation.")]
    AdminOnly,
}