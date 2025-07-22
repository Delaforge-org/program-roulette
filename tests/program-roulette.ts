import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { RouletteGame } from "../target/types/roulette_game"; // Path to the IDL types
import { assert } from "chai";
import { PublicKey, SystemProgram, Keypair, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAssociatedTokenAccount, mintTo, getAccount, createAssociatedTokenAccountInstruction } from "@solana/spl-token";
import { BN } from "bn.js";

describe("roulette-game", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.RouletteGame as Program<RouletteGame>;
  const payer = provider.wallet as anchor.Wallet; // Use the wallet from Anchor.toml

  // Static constant from the contract
  const TREASURY_PUBKEY = new PublicKey("DRqMriKY4X3ggiFdx27Fotu5HebQFyRZhNasWFTzaQ78");

  // Test keys
  const mintAuthority = Keypair.generate();
  let tokenMint: PublicKey;

  // Provider One (the payer)
  const providerOne = payer;
  let providerOneTokenAccount: PublicKey;
  let providerOneStatePda: PublicKey;

  // Provider Two (a new keypair)
  const providerTwo = Keypair.generate();
  let providerTwoTokenAccount: PublicKey;
  let providerTwoStatePda: PublicKey;

  // Game-related keys
  let vaultPda: PublicKey;
  let vaultTokenAccount: PublicKey;
  let gameSessionPda: PublicKey;
  let playerBetsPda: PublicKey;
  let claimRecordPda: PublicKey;
  const roundToCheck = new BN(1);

  before(async () => {
    // --- Step 0: Airdrops and Token setup ---
    await provider.connection.requestAirdrop(mintAuthority.publicKey, 1 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(providerTwo.publicKey, 1 * anchor.web3.LAMPORTS_PER_SOL);
    await new Promise(resolve => setTimeout(resolve, 500));

    tokenMint = await createMint(provider.connection, mintAuthority, mintAuthority.publicKey, null, 9);
    console.log("Token Mint:", tokenMint.toBase58());

    // Setup for Provider One
    providerOneTokenAccount = await createAssociatedTokenAccount(provider.connection, payer.payer, tokenMint, providerOne.publicKey);
    await mintTo(provider.connection, payer.payer, tokenMint, providerOneTokenAccount, mintAuthority, 1_000_000_000_000);
    console.log("Provider One ATA:", providerOneTokenAccount.toBase58());

    // Setup for Provider Two
    providerTwoTokenAccount = await createAssociatedTokenAccount(provider.connection, providerTwo, tokenMint, providerTwo.publicKey);
    await mintTo(provider.connection, payer.payer, tokenMint, providerTwoTokenAccount, mintAuthority, 500_000_000_000); // Mint 500 tokens
    console.log("Provider Two ATA:", providerTwoTokenAccount.toBase58());

    // --- Step 1: Find PDAs ---
    [gameSessionPda] = PublicKey.findProgramAddressSync([Buffer.from("game_session")], program.programId);
    [playerBetsPda] = PublicKey.findProgramAddressSync([Buffer.from("player_bets"), gameSessionPda.toBuffer(), providerOne.publicKey.toBuffer()], program.programId);
    [vaultPda] = PublicKey.findProgramAddressSync([Buffer.from("vault"), tokenMint.toBuffer()], program.programId);
    [providerOneStatePda] = PublicKey.findProgramAddressSync([Buffer.from("provider_state"), vaultPda.toBuffer(), providerOne.publicKey.toBuffer()], program.programId);
    [providerTwoStatePda] = PublicKey.findProgramAddressSync([Buffer.from("provider_state"), vaultPda.toBuffer(), providerTwo.publicKey.toBuffer()], program.programId);

    vaultTokenAccount = await anchor.utils.token.associatedAddress({ mint: tokenMint, owner: vaultPda });
    console.log("Vault PDA:", vaultPda.toBase58());
    console.log("Vault ATA:", vaultTokenAccount.toBase58());

    const roundBytes = roundToCheck.toArrayLike(Buffer, 'le', 8);
    [claimRecordPda] = PublicKey.findProgramAddressSync([Buffer.from("claim_record"), providerOne.publicKey.toBuffer(), roundBytes], program.programId);
    console.log("Claim Record PDA (for round 1):", claimRecordPda.toBase58());

    // --- Step 2: Initialization ---
    try {
      await program.methods.initializeGameSession().accounts({
        authority: payer.publicKey,
        gameSession: gameSessionPda,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      }).rpc();
      console.log("Game Session initialized.");
    } catch (e) { if (!e.toString().includes("already in use")) throw e; console.log("Game Session already initialized."); }

    try {
      // Create the vault's associated token account before initializing
      await createAssociatedTokenAccount(provider.connection, payer.payer, tokenMint, vaultPda, true);
    } catch (e) { if (!e.message.includes("TokenAccountAlreadyAssociatedWithMint")) throw e; }

    try {
      await program.methods.initializeAndProvideLiquidity(new BN(1_000_000_000_000))
        .accounts({
          authority: payer.publicKey,
          tokenMint: tokenMint,
          vault: vaultPda,
          providerState: providerOneStatePda,
          providerTokenAccount: providerOneTokenAccount,
          vaultTokenAccount: vaultTokenAccount,
          liquidityProvider: providerOne.publicKey,
          treasuryAccount: TREASURY_PUBKEY,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      console.log("Vault initialized and initial liquidity provided by Provider One.");
    } catch (e) {
      if (!e.toString().includes("already in use")) {
        console.error("Error initializing vault:", e);
        if (e.logs) { console.error("Logs:", e.logs); }
        throw e;
      }
      console.log("Vault likely already initialized.");
    }

    try {
      await program.methods.initializePlayerBets().accounts({
        player: providerOne.publicKey,
        gameSession: gameSessionPda,
        playerBets: playerBetsPda,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      }).rpc();
      console.log("Player Bets initialized for Provider One.");
    } catch (e) { if (!e.toString().includes("already in use")) throw e; console.log("Player Bets already initialized."); }
  });

  it("Allows a second provider to add liquidity", async () => {
    const vaultBefore = await program.account.vaultAccount.fetch(vaultPda);

    await program.methods.provideLiquidity(new BN(500_000_000_000))
      .accounts({
        vault: vaultPda,
        tokenMint: tokenMint,
        providerState: providerTwoStatePda,
        providerTokenAccount: providerTwoTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        liquidityProvider: providerTwo.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([providerTwo])
      .rpc();
    
    console.log("Provider Two added liquidity.");
    
    const vaultAfter = await program.account.vaultAccount.fetch(vaultPda);
    const providerTwoState = await program.account.providerState.fetch(providerTwoStatePda);

    const expectedCapital = vaultBefore.totalProviderCapital.add(new BN(500_000_000_000));
    assert.ok(vaultAfter.totalProviderCapital.eq(expectedCapital), "Vault total capital should increase.");
    assert.ok(providerTwoState.amount.eq(new BN(500_000_000_000)), "Provider Two state should record correct amount.");
  });

  it("Conducts a game round and generates rewards", async () => {
    console.log("Starting round 1...");
    await program.methods.startNewRound().accounts({
      gameSession: gameSessionPda,
      starter: providerOne.publicKey,
      systemProgram: SystemProgram.programId,
    }).rpc();
    let gameSession = await program.account.gameSession.fetch(gameSessionPda);
    assert.ok(gameSession.currentRound.eq(roundToCheck), "Round number should be 1");

    console.log("Placing a bet...");
    const betAmount = new BN(100_000_000);
    const betOnRed = { amount: betAmount, betType: 6, numbers: [0, 0, 0, 0] };
    await program.methods.placeBet(betOnRed).accounts({
      vault: vaultPda,
      gameSession: gameSessionPda,
      playerTokenAccount: providerOneTokenAccount,
      vaultTokenAccount: vaultTokenAccount,
      player: providerOne.publicKey,
      playerBets: playerBetsPda,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).rpc();
    console.log("Bet placed.");
    
    const vaultAfterBet = await program.account.vaultAccount.fetch(vaultPda);
    // After a bet, reward_per_share_index should be > 0
    assert.ok(vaultAfterBet.rewardPerShareIndex.gt(new BN(0)), "Reward index should increase after a bet.");
  });
  
  it("Allows a provider to withdraw only revenue", async () => {
    const providerStateBefore = await program.account.providerState.fetch(providerOneStatePda);
    const providerTokenAccBefore = await getAccount(provider.connection, providerOneTokenAccount);

    // Call withdraw revenue
    await program.methods.withdrawProviderRevenue()
      .accounts({
        vault: vaultPda,
        providerState: providerOneStatePda,
        tokenMint: tokenMint,
        providerTokenAccount: providerOneTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        liquidityProvider: providerOne.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
    
    console.log("Provider One withdrew revenue.");
      
    const providerStateAfter = await program.account.providerState.fetch(providerOneStatePda);
    const providerTokenAccAfter = await getAccount(provider.connection, providerOneTokenAccount);
    
    // Rewards were earned from the bet in the previous test.
    assert.ok(providerStateBefore.unclaimed_rewards.gt(new BN(0)) || providerTokenAccAfter.amount > providerTokenAccBefore.amount, "Provider one should have earned rewards and balance should increase.");
    assert.ok(providerStateAfter.unclaimedRewards.eq(new BN(0)), "Unclaimed rewards should be zero after withdrawal.");
    assert.ok(providerStateAfter.amount.eq(providerStateBefore.amount), "Capital amount should not change.");
  });
  
  it("Allows a provider to withdraw all liquidity and closes their account", async () => {
    const vaultBefore = await program.account.vaultAccount.fetch(vaultPda);
    const providerStateBefore = await program.account.providerState.fetch(providerTwoStatePda);
    
    // Call withdraw all liquidity
    await program.methods.withdrawLiquidity()
      .accounts({
        vault: vaultPda,
        providerState: providerTwoStatePda,
        tokenMint: tokenMint,
        providerTokenAccount: providerTwoTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        liquidityProvider: providerTwo.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([providerTwo])
      .rpc();

    console.log("Provider Two withdrew all liquidity.");

    const vaultAfter = await program.account.vaultAccount.fetch(vaultPda);
    
    // Check that vault capital decreased
    const expectedCapital = vaultBefore.totalProviderCapital.sub(providerStateBefore.amount);
    assert.ok(vaultAfter.totalProviderCapital.eq(expectedCapital), "Vault capital should decrease by the withdrawn amount.");

    // Check that the provider's state account is closed
    const closedAccountInfo = await provider.connection.getAccountInfo(providerTwoStatePda);
    assert.isNull(closedAccountInfo, "Provider Two's state account should be closed.");
  });
});