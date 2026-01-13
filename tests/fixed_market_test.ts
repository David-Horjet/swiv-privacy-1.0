import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SwivPrivacy } from "../target/types/swiv_privacy";
import {
  PublicKey,
  SystemProgram,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import { keccak256 } from "js-sha3";
import * as fs from "fs";
import * as path from "path";

// --- MAGIC BLOCK CONFIGURATION ---
import {
  MAGIC_PROGRAM_ID,
} from "@magicblock-labs/ephemeral-rollups-sdk";

// FIX: Hardcode the ID found in your error logs to ensure PDA derivation matches on-chain
const ACCESS_CONTROL_PROGRAM_ID = new PublicKey("BTWAqWNBmF2TboMh3fxMJfgR16xGHYD7Kgr2dPwbRPBi");

// --- Helper: Retry Mechanism ---
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function retryOp<T>(
  operation: () => Promise<T>,
  description: string,
  maxRetries = 5,
  delayMs = 2000
): Promise<T> {
  let lastError: any;
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await operation();
    } catch (e: any) {
      console.log(
        `    ⚠️ ${description} failed (Attempt ${
          i + 1
        }/${maxRetries}). Retrying in ${delayMs}ms...`
      );
      lastError = e;
      await sleep(delayMs);
    }
  }
  console.error(
    `    ❌ ${description} failed permanently after ${maxRetries} attempts.`
  );
  throw lastError;
}

// --- Helper: Load or Generate Keypairs ---
const KEYS_DIR = path.join(__dirname, "keys");
if (!fs.existsSync(KEYS_DIR)) {
  fs.mkdirSync(KEYS_DIR);
}

function loadOrGenerateKeypair(name: string): Keypair {
  const filePath = path.join(KEYS_DIR, `${name}.json`);
  if (fs.existsSync(filePath)) {
    const secretKey = JSON.parse(fs.readFileSync(filePath, "utf-8"));
    return Keypair.fromSecretKey(new Uint8Array(secretKey));
  } else {
    const kp = Keypair.generate();
    fs.writeFileSync(filePath, JSON.stringify(Array.from(kp.secretKey)));
    return kp;
  }
}

describe("2. Fixed Market & Betting Tests (House & Parimutuel) - Single User", () => {
  // 1. BASE LAYER PROVIDER
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // 2. EPHEMERAL ROLLUP PROVIDER (MagicBlock)
  const providerEphemeralRollup = new anchor.AnchorProvider(
    new anchor.web3.Connection(
      process.env.EPHEMERAL_PROVIDER_ENDPOINT ||
        "https://devnet.magicblock.app",
      {
        wsEndpoint:
          process.env.EPHEMERAL_WS_ENDPOINT || "wss://devnet.magicblock.app",
      }
    ),
    anchor.Wallet.local(),
    { commitment: "confirmed" }
  );

  console.log("Base Layer Connection: ", provider.connection.rpcEndpoint);
  console.log(
    "Ephemeral Rollup Connection: ",
    providerEphemeralRollup.connection.rpcEndpoint
  );

  const program = anchor.workspace.SwivPrivacy as Program<SwivPrivacy>;
  const admin = provider.wallet as anchor.Wallet;

  // --- REDUCED TO 1 USER ---
  const users = [loadOrGenerateKeypair("userA")];

  // PDAs & Accounts
  let usdcMint: PublicKey;
  let globalConfigPda: PublicKey;
  let assetConfigPda: PublicKey;
  let houseMarketPda: PublicKey;
  let houseVaultPda: PublicKey;
  let pariMarketPda: PublicKey;
  let pariVaultPda: PublicKey;
  let treasuryUsdcAta: PublicKey;
  let userATAs: PublicKey[] = [];
  let admin_USDC: PublicKey;

  // Constants
  const SEED_GLOBAL_CONFIG = Buffer.from("global_config_v1");
  const SEED_ASSET_CONFIG = Buffer.from("asset_config");
  const SEED_FIXED_MARKET = Buffer.from("fixed_market");
  const SEED_BET = Buffer.from("user_bet");

  const ASSET_SYMBOL = "SOL";
  const SOL_USD_FEED = new PublicKey(
    "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
  );

  const HOUSE_MARKET_NAME = `SOL-House-${Math.floor(Math.random() * 1000)}`;
  const PARI_MARKET_NAME = `SOL-Pari-${Math.floor(Math.random() * 1000)}`;

  const HOUSE_FEE_BPS = 150;
  const PARI_FEE_BPS = 300;

  function createCommitment(
    low: anchor.BN,
    high: anchor.BN,
    target: anchor.BN,
    salt: Buffer
  ) {
    const buf = Buffer.concat([
      low.toArrayLike(Buffer, "le", 8),
      high.toArrayLike(Buffer, "le", 8),
      target.toArrayLike(Buffer, "le", 8),
      salt,
    ]);
    return Buffer.from(keccak256.create().update(buf).arrayBuffer());
  }

  it("Setup: Prepare Protocol, Assets & Fund User", async () => {
    // 1. FUND USER
    for (const user of users) {
      const balance = await provider.connection.getBalance(user.publicKey);
      if (balance < 0.05 * LAMPORTS_PER_SOL) {
        await retryOp(async () => {
          const tx = new anchor.web3.Transaction().add(
            SystemProgram.transfer({
              fromPubkey: admin.publicKey,
              toPubkey: user.publicKey,
              lamports: 0.05 * LAMPORTS_PER_SOL,
            })
          );
          await provider.sendAndConfirm(tx);
        }, "Fund User SOL");
      }
    }

    // 2. Create Mint & ATAs
    usdcMint = await createMint(
      provider.connection,
      admin.payer,
      admin.publicKey,
      null,
      6
    );

    admin_USDC = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        admin.payer,
        usdcMint,
        admin.publicKey
      )
    ).address;
    await mintTo(
      provider.connection,
      admin.payer,
      usdcMint,
      admin_USDC,
      admin.payer,
      100_000_000_000
    );

    userATAs = [];
    for (const user of users) {
      const ata = (
        await getOrCreateAssociatedTokenAccount(
          provider.connection,
          admin.payer,
          usdcMint,
          user.publicKey
        )
      ).address;
      userATAs.push(ata);
      await mintTo(
        provider.connection,
        admin.payer,
        usdcMint,
        ata,
        admin.payer,
        10_000_000_000
      );
    }

    // 3. Init or Fetch Global Config
    [globalConfigPda] = PublicKey.findProgramAddressSync(
      [SEED_GLOBAL_CONFIG],
      program.programId
    );
    let currentTreasuryWallet = admin.publicKey;

    try {
      const configAccount = await program.account.globalConfig.fetch(
        globalConfigPda
      );
      currentTreasuryWallet = configAccount.treasuryWallet;
      
      await program.methods
        .updateConfig(null, null, null, [usdcMint])
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    } catch (e) {
      const treasuryKey = Keypair.generate();
      await program.methods
        .initializeProtocol(
          new anchor.BN(HOUSE_FEE_BPS),
          new anchor.BN(PARI_FEE_BPS),
          [usdcMint]
        )
        .accounts({
          admin: admin.publicKey,
          treasuryWallet: treasuryKey.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      currentTreasuryWallet = treasuryKey.publicKey;
    }

    treasuryUsdcAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        admin.payer,
        usdcMint,
        currentTreasuryWallet,
        true
      )
    ).address;

    [assetConfigPda] = PublicKey.findProgramAddressSync(
      [SEED_ASSET_CONFIG, Buffer.from(ASSET_SYMBOL)],
      program.programId
    );
    try {
      await program.methods
        .configAsset(ASSET_SYMBOL, SOL_USD_FEED, new anchor.BN(1500), false)
        .accounts({
          admin: admin.publicKey,
          pythFeed: SOL_USD_FEED,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    } catch (e: any) {}
  });

  describe("--- House Mode Round (1 User) ---", () => {
    let initialVaultBalance: number;
    let netUserDeposit = 0;
    
    // Increased duration to ensure test actions complete before expiry
    const DURATION_SECONDS = 60;

    it("Create Fixed Market & Admin Funds Vault", async () => {
      const now = Math.floor(Date.now() / 1000);
      const startTime = new anchor.BN(now);
      const endTime = new anchor.BN(now + DURATION_SECONDS);

      const initialLiquidity = new anchor.BN(1_000_000_000);

      [houseMarketPda] = PublicKey.findProgramAddressSync(
        [SEED_FIXED_MARKET, Buffer.from(HOUSE_MARKET_NAME)],
        program.programId
      );

      [houseVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("fixed_vault"), houseMarketPda.toBuffer()],
        program.programId
      );

      await program.methods
        .createFixedMarket(
          HOUSE_MARKET_NAME,
          "House PnL Test",
          startTime,
          endTime,
          initialLiquidity,
          { house: {} },
          { targetOnly: {} },
          new anchor.BN(500000),
          new anchor.BN(1000)
        )
        .accounts({
          globalConfig: globalConfigPda,
          fixedMarket: houseMarketPda,
          marketVault: houseVaultPda,
          tokenMint: usdcMint,
          admin: admin.publicKey,
          adminTokenAccount: admin_USDC,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      const vaultAccount = await provider.connection.getTokenAccountBalance(
        houseVaultPda
      );
      initialVaultBalance = vaultAccount.value.uiAmount!;
      assert.equal(initialVaultBalance, 1000);
    });

    // 1 User Data
    const requestId = "req_h_1";
    const userSalt = Keypair.generate().publicKey.toBuffer();
    const userPrediction = new anchor.BN(150_000_000); // Winning Prediction

    it("User Places Bet", async () => {
      const betAmount = new anchor.BN(50_000_000);
      const rawAmount = 50.0;
      const fee = rawAmount * (HOUSE_FEE_BPS / 10000);
      netUserDeposit = rawAmount - fee;

      const user = users[0];
      const commitment = createCommitment(
        new anchor.BN(0),
        new anchor.BN(0),
        userPrediction,
        userSalt
      );

      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          houseMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      const [groupPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("group"), betPda.toBuffer()],
        ACCESS_CONTROL_PROGRAM_ID
      );

      const [permissionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("permission"),
          groupPda.toBuffer(),
          user.publicKey.toBuffer(),
        ],
        ACCESS_CONTROL_PROGRAM_ID
      );

      await retryOp(async () => {
        await program.methods
          .placeBetFixed(
            betAmount,
            new anchor.BN(20000),
            Array.from(commitment),
            requestId
          )
          .accounts({
            user: user.publicKey,
            globalConfig: globalConfigPda,
            fixedMarket: houseMarketPda,
            assetConfig: assetConfigPda,
            marketVault: houseVaultPda,
            userTokenAccount: userATAs[0],
            treasuryWallet: treasuryUsdcAta,
            userBet: betPda,
            group: groupPda,
            permission: permissionPda,
            permissionProgram: ACCESS_CONTROL_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .signers([user])
          .rpc({ skipPreflight: true });
      }, `User 0 Bet`);
    });

    it("Delegate, Reveal (ER), Undelegate", async () => {
      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          houseMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      // 1. DELEGATE (L1 -> ER)
      console.log(`    🔒 Delegating Bet to TEE...`);
      await retryOp(async () => {
        await program.methods
          .delegateBet(requestId)
          .accounts({
            user: user.publicKey,
            fixedMarket: houseMarketPda,
            userBet: betPda,
          })
          .signers([user])
          .rpc({ skipPreflight: true });
      }, "Delegation");
      
      await sleep(3000); // Wait for sync

      // 2. REVEAL (ER)
      console.log(`    🕵️  Revealing Bet inside TEE...`);
      
      const erProvider = new anchor.AnchorProvider(
         providerEphemeralRollup.connection,
         new anchor.Wallet(user),
         { commitment: "confirmed", skipPreflight: true }
      );
      const erProgram = new anchor.Program(program.idl, erProvider);

      await retryOp(async () => {
        const txHash = await erProgram.methods
          .revealBet(
            new anchor.BN(0),
            new anchor.BN(0),
            userPrediction,
            Array.from(userSalt)
          )
          .accounts({ 
              user: user.publicKey, 
              userBet: betPda,
              permissionProgram: ACCESS_CONTROL_PROGRAM_ID
           })
          .rpc();
        console.log(`    ⚡ Reveal Tx: ${txHash}`);
      }, "ER Reveal");

      await sleep(1000);

      // 3. UNDELEGATE (ER -> L1 Commit)
      console.log(`    🔓 Undelegating (Committing)...`);
      await retryOp(async () => {
         await erProgram.methods
          .undelegateBet(requestId)
          .accounts({
              user: user.publicKey,
              fixedMarket: houseMarketPda,
              userBet: betPda,
              magicProgram: MAGIC_PROGRAM_ID,
          })
          .rpc();
      }, "Undelegate");
    });

    it("Resolve, Calculate & Claim", async () => {
      console.log("    ⏳ Waiting 65s for market expiry...");
      await sleep(65000); 

      // Resolve
      await program.methods
        .resolveFixedMarket(new anchor.BN(150_000_000))
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          fixedMarket: houseMarketPda,
        })
        .rpc();

      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          houseMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      // Calculate (FIX: Added betOwner)
      await program.methods
        .calculateFixedOutcome()
        .accounts({
          payer: user.publicKey,
          betOwner: user.publicKey,
          fixedMarket: houseMarketPda,
          userBet: betPda,
        })
        .signers([user])
        .rpc();

      // Claim
      const preBal = (
        await provider.connection.getTokenAccountBalance(userATAs[0])
      ).value.uiAmount!;
      
      await program.methods
        .claimFixedReward()
        .accounts({
          user: user.publicKey,
          fixedMarket: houseMarketPda,
          marketVault: houseVaultPda,
          userBet: betPda,
          userTokenAccount: userATAs[0],
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();

      const postBal = (
        await provider.connection.getTokenAccountBalance(userATAs[0])
      ).value.uiAmount!;
      
      const payout = postBal - preBal;
      console.log(`    🎉 User Claimed: ${payout.toFixed(2)} USDC`);
      assert.isAbove(payout, 0, "User should have won");
    });
  });

  describe("--- Parimutuel Mode Round (1 User) ---", () => {
    it("Create Fixed Market (Parimutuel Mode)", async () => {
      const now = Math.floor(Date.now() / 1000);
      const startTime = new anchor.BN(now);
      const endTime = new anchor.BN(now + 60);

      [pariMarketPda] = PublicKey.findProgramAddressSync(
        [SEED_FIXED_MARKET, Buffer.from(PARI_MARKET_NAME)],
        program.programId
      );
      [pariVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("fixed_vault"), pariMarketPda.toBuffer()],
        program.programId
      );

      await program.methods
        .createFixedMarket(
          PARI_MARKET_NAME,
          "Pari Privacy Test",
          startTime,
          endTime,
          new anchor.BN(0),
          { parimutuel: {} },
          { targetOnly: {} },
          new anchor.BN(10),
          new anchor.BN(1000)
        )
        .accounts({
          globalConfig: globalConfigPda,
          fixedMarket: pariMarketPda,
          marketVault: pariVaultPda,
          tokenMint: usdcMint,
          admin: admin.publicKey,
          adminTokenAccount: admin_USDC,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();
    });

    const requestId = "req_p_1";
    const userSalt = Keypair.generate().publicKey.toBuffer();
    const prediction = new anchor.BN(200_000_000);

    it("User Places Bet", async () => {
      const betAmount = new anchor.BN(100_000_000);
      const user = users[0];
      const commitment = createCommitment(
        new anchor.BN(0),
        new anchor.BN(0),
        prediction,
        userSalt
      );
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          pariMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );
      const [groupPda] = PublicKey.findProgramAddressSync(
          [Buffer.from("group"), betPda.toBuffer()],
          ACCESS_CONTROL_PROGRAM_ID
      );
      const [permissionPda] = PublicKey.findProgramAddressSync(
          [Buffer.from("permission"), groupPda.toBuffer(), user.publicKey.toBuffer()],
          ACCESS_CONTROL_PROGRAM_ID
      );

      await program.methods
        .placeBetFixed(
          betAmount,
          new anchor.BN(0),
          Array.from(commitment),
          requestId
        )
        .accounts({
          user: user.publicKey,
          globalConfig: globalConfigPda,
          fixedMarket: pariMarketPda,
          assetConfig: assetConfigPda,
          marketVault: pariVaultPda,
          userTokenAccount: userATAs[0],
          treasuryWallet: treasuryUsdcAta,
          userBet: betPda,
          group: groupPda,
          permission: permissionPda,
          permissionProgram: ACCESS_CONTROL_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([user])
        .rpc();
    });

    it("Delegate, Reveal (ER), Undelegate", async () => {
      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          pariMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      // 1. DELEGATE
      await program.methods
        .delegateBet(requestId)
        .accounts({
          user: user.publicKey,
          fixedMarket: pariMarketPda,
          userBet: betPda,
        })
        .signers([user])
        .rpc({skipPreflight: true});
      await sleep(2000);

      // 2. REVEAL
      const erProvider = new anchor.AnchorProvider(
        providerEphemeralRollup.connection,
        new anchor.Wallet(user),
        { commitment: "confirmed", skipPreflight: true }
      );
      const erProgram = new anchor.Program(program.idl, erProvider);
      
      await erProgram.methods
        .revealBet(
          new anchor.BN(0),
          new anchor.BN(0),
          prediction,
          Array.from(userSalt)
        )
        .accounts({ 
            user: user.publicKey, 
            userBet: betPda,
            permissionProgram: ACCESS_CONTROL_PROGRAM_ID
         })
        .rpc();
      await sleep(1000);

      // 3. UNDELEGATE
      await erProgram.methods
        .undelegateBet(requestId)
        .accounts({
          user: user.publicKey,
          fixedMarket: pariMarketPda,
          userBet: betPda,
          magicProgram: MAGIC_PROGRAM_ID,
        })
        .rpc();
    });

    it("Resolve, Finalize & Claim", async () => {
      console.log("    ⏳ Waiting 65s for market expiry...");
      await sleep(65000);

      await program.methods
        .resolveFixedMarket(new anchor.BN(200_000_000))
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          fixedMarket: pariMarketPda,
        })
        .rpc();

      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          pariMarketPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      // FIX: Added betOwner
      await program.methods
        .calculateFixedOutcome()
        .accounts({
          payer: user.publicKey,
          betOwner: user.publicKey,
          fixedMarket: pariMarketPda,
          userBet: betPda,
        })
        .signers([user])
        .rpc();

      await program.methods
        .finalizeWeights()
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          fixedMarket: pariMarketPda,
          marketVault: pariVaultPda,
          treasuryWallet: treasuryUsdcAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      await program.methods
        .claimFixedReward()
        .accounts({
          user: user.publicKey,
          fixedMarket: pariMarketPda,
          marketVault: pariVaultPda,
          userBet: betPda,
          userTokenAccount: userATAs[0],
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
    });
  });
});