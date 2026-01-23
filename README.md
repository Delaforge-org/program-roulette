# On-Chain Roulette Game

This is a smart contract for a decentralized roulette game on the Solana blockchain, developed using the Anchor framework. The project allows users to bet on the outcome of a roulette spin using various SPL tokens and also enables other users to become liquidity providers and earn income from the gameplay.

## 📐 Architecture Diagram

A visual representation of the contract's architecture and account interactions can be found on our Miro board:

[View Architecture Diagram on Miro](https://miro.com/app/board/uXjVJbyCEN8=/?share_link_id=978905070684)

## ⚙️ Core Concepts

### 1. Liquidity Management Architecture

The contract uses a scalable, two-tier account system for managing liquidity, ensuring it can support a large number of providers without running into memory limitations.

-   **Global Vault (`VaultAccount`)**: Each SPL token used in the game has its own global vault. This account holds the total pooled liquidity and tracks global reward calculation parameters. It does **not** store individual provider data.
-   **Provider-Specific State (`ProviderState`)**: For each user providing liquidity to a vault, a separate, dedicated `ProviderState` account is created. This account tracks that specific user's capital contribution and their unclaimed rewards. The user pays the rent for their own state account and is refunded when they fully withdraw, making the system horizontally scalable.

### 2. Gameplay

-   **Game Session (`GameSession`)**: A global account that manages the state of the game: current round number, round status, start time, etc.
-   **Rounds**: The game is divided into rounds with the following statuses:
    1.  `AcceptingBets`: Players can place bets.
    2.  `BetsClosed`: Betting is closed for the round.
    3.  `Completed`: A winning number is generated, and the round is considered complete.
-   **Bets (`Bet`)**: Players can place various types of bets similar to classic roulette (on a number, color, dozen, etc.). To do this, they use their `PlayerBets` account.

### 3. Revenue Distribution

-   The contract automatically takes a commission from each bet.
-   This commission is distributed between:
    -   **Liquidity Providers**: ~1.4% of each bet (1/71) as rewards for provided capital.
    -   **Program Owner**: ~0.8% of each bet (1/125) as protocol revenue.
-   The remaining amount (~97.8%) forms a **payout reserve** used to pay winners.
-   The program owner can periodically call `distribute_payout_reserve` to distribute 50% of accumulated reserves equally between providers (25%) and owner (25%).

### 4. Random Number Generation

The winning number (from 0 to 36) is determined randomly on the blockchain. The generation mechanism is as follows:

1.  After bets are closed for a round, the `get_random` instruction is called.
2.  The contract takes the **current slot number** (`slot`), the **timestamp**, and the **public key of the last player who placed a bet** (`last_bettor`).
3.  These values are hashed together using `sha256`.
4.  Based on the resulting hash, a number in the range of 0 to 36 is calculated.


## 🗂️ Key Accounts

-   `VaultAccount`: Stores global data for a liquidity pool of a specific SPL token, such as total liquidity and reward calculation indexes.
-   `ProviderState`: A dedicated account for each liquidity provider within a specific vault. It tracks the amount of capital provided by that user and their unclaimed rewards. It's created on the first deposit and closed on full withdrawal.
-   `GameSession`: A global account that manages the state and lifecycle of game rounds.
-   `PlayerBets`: An account created for each player to store their bets for the current round. It also tracks the `claimed_round` to prevent double-claiming of winnings.

## 📜 Contract Instructions

### Vault and Liquidity Management

-   `initialize_and_provide_liquidity`: Creates a new vault and provides initial liquidity, creating both the `VaultAccount` and the first `ProviderState` account in a single transaction.
-   `provide_liquidity`: Allows a user to deposit tokens into a vault. Creates a personal `ProviderState` account for the user on their first deposit.
-   `withdraw_liquidity`: Allows a user to withdraw their **entire** provided capital and all accumulated rewards. This action closes the user's `ProviderState` account and refunds the associated rent.
-   `withdraw_provider_revenue`: Allows a liquidity provider to claim only their earned rewards without withdrawing their capital.
-   `withdraw_owner_revenue`: Allows the program owner to claim their share of the revenue.
-   `distribute_payout_reserve`: Allows the program owner to distribute 50% of the accumulated payout reserve. Half goes to liquidity providers (proportionally) and half to the program owner.
-   `get_unclaimed_rewards`: A read-only instruction that allows liquidity providers to query their unclaimed rewards without making a transaction (via simulation).

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
