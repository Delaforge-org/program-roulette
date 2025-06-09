# On-Chain Roulette Game

This is a smart contract for a decentralized roulette game on the Solana blockchain, developed using the Anchor framework. The project allows users to bet on the outcome of a roulette spin using various SPL tokens and also enables other users to become liquidity providers and earn income from the gameplay.

## ⚙️ Core Concepts

### 1. Liquidity Vaults

-   Each SPL token used in the game has its own vault (`VaultAccount`).
-   Users (liquidity providers) can deposit their tokens into these vaults to provide liquidity for paying out winnings.
-   In return for providing liquidity, they receive a share of the players' losses.

### 2. Gameplay

-   **Game Session (`GameSession`)**: A global account that manages the state of the game: current round number, round status, start time, etc.
-   **Rounds**: The game is divided into rounds with the following statuses:
    1.  `AcceptingBets`: Players can place bets.
    2.  `BetsClosed`: Betting is closed for the round.
    3.  `Completed`: A winning number is generated, and the round is considered complete.
-   **Bets (`Bet`)**: Players can place various types of bets similar to classic roulette (on a number, color, dozen, etc.). To do this, they use their `PlayerBets` account.

### 3. Revenue Distribution

-   The contract automatically takes a commission from each winning payout.
-   This commission is distributed between:
    -   **Liquidity Providers**: as a reward for the funds provided.
    -   **Program Owner**: as income from the use of the contract.

### 4. Random Number Generation

The winning number (from 0 to 36) is determined randomly on the blockchain. The generation mechanism is as follows:

1.  After bets are closed for a round, the `get_random` instruction is called.
2.  The contract takes the **current slot number** (`slot`) and the **public key of the last player who placed a bet** (`last_bettor`).
3.  These two values are hashed using `sha256`.
4.  Based on the resulting hash, a number in the range of 0 to 36 is calculated.


## 🗂️ Key Accounts

-   `VaultAccount`: Stores liquidity for a specific SPL token, information about providers, and their rewards.
-   `GameSession`: A global account that manages the state and lifecycle of game rounds.
-   `PlayerBets`: An account created for each player to store their bets for the current round.

## 📜 Contract Instructions

### Vault and Liquidity Management

-   `initialize_vault`: Creates a new vault for an SPL token.
-   `provide_liquidity`: Allows a user to deposit tokens into a vault.
-   `withdraw_liquidity`: Allows a user to withdraw their tokens from a vault.
-   `initialize_and_provide_liquidity`: Combines vault creation and initial liquidity provision.
-   `withdraw_provider_revenue`: Allows a liquidity provider to claim their earned rewards.
-   `withdraw_owner_revenue`: Allows the program owner to claim their share of the revenue.

### Gameplay

-   `initialize_game_session`: Initializes the global game session.
-   `initialize_player_bets`: Creates a betting account for a new player.
-   `start_new_round`: Starts a new round of the game.
-   `place_bet`: Allows a player to place a bet.
-   `close_bets`: Closes betting for the current round.
-   `get_random`: Triggers the generation of the winning number.
-   `claim_my_winnings`: Allows a player to claim their winnings.
-   `close_player_bets_account`: Closes a player's betting account and returns the rent SOL.

## 🚀 Getting Started

### Prerequisites

-   [Rust](https://www.rust-lang.org/tools/install)
-   [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
-   [Anchor](https://www.anchor-lang.com/docs/installation)

### Build

```bash
anchor build
```

### Test

```bash
anchor test
```

### Deploy

```bash
anchor deploy
```

## ⚠️ Disclaimer

This project is intended for educational and demonstration purposes.
