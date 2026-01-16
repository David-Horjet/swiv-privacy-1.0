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

describe("Pool & Betting Tests (Parimutuel) - Single User with Privacy", () => {
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
  let poolPda: PublicKey;
  let treasuryUsdcAta: PublicKey;
  let userATAs: PublicKey[] = [];
  let admin_USDC: PublicKey;

  // Constants
  const SEED_GLOBAL_CONFIG = Buffer.from("global_config_v1");
  const SEED_POOL = Buffer.from("pool");
  const SEED_BET = Buffer.from("user_bet");

  const POOL_NAME = `SOL-Pool-${Math.floor(Math.random() * 1000)}`;

  const PROTOCOL_FEE_BPS = 300;

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
        .updateConfig(null, new anchor.BN(PROTOCOL_FEE_BPS))
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    } catch (e) {
      const treasuryKey = Keypair.generate();
      await program.methods
        .initializeProtocol(new anchor.BN(PROTOCOL_FEE_BPS))
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
  });

  describe("--- Pool Round (1 User with Privacy) ---", () => {
    let initialVaultBalance: number;
    let netUserDeposit = 0;
    
    // Increased duration to ensure test actions complete before expiry
    const DURATION_SECONDS = 60;

    it("Create Pool", async () => {
      const now = Math.floor(Date.now() / 1000);
      const startTime = new anchor.BN(now);
      const endTime = new anchor.BN(now + DURATION_SECONDS);

      [poolPda] = PublicKey.findProgramAddressSync(
        [SEED_POOL, Buffer.from(POOL_NAME)],
        program.programId
      );

      await program.methods
        .createPool(
          POOL_NAME,
          startTime,
          endTime,
          new anchor.BN(500), // max_accuracy_buffer
          new anchor.BN(1000) // conviction_bonus_bps
        )
        .accounts({
          globalConfig: globalConfigPda,
          pool: poolPda,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const poolAccount = await program.account.pool.fetch(poolPda);
      assert.equal(poolAccount.name, POOL_NAME);
      assert.equal(poolAccount.isResolved, false);
    });

    // 1 User Data
    const requestId = "req_h_1";
    const userSalt = Keypair.generate().publicKey.toBuffer();
    const userPrediction = new anchor.BN(150_000_000); // Winning Prediction

    it("User Places Bet", async () => {
      const betAmount = new anchor.BN(50_000_000);
      const rawAmount = 50.0;
      const fee = rawAmount * (PROTOCOL_FEE_BPS / 10000);
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
          poolPda.toBuffer(),
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
          .placeBet(
            betAmount,
            Array.from(commitment),
            requestId
          )
          .accounts({
            user: user.publicKey,
            globalConfig: globalConfigPda,
            pool: poolPda,
            userTokenAccount: userATAs[0],
            treasuryWallet: treasuryUsdcAta,
            userBet: betPda,
            group: groupPda,
            permission: permissionPda,
            permissionProgram: ACCESS_CONTROL_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }, "Place Bet");

      const betAccount = await program.account.userBet.fetch(betPda);
      assert.equal(betAccount.deposit.toNumber(), 50000000);
      assert.deepEqual(betAccount.commitment, Array.from(commitment));
    });

    it("Delegate, Reveal (ER), Undelegate", async () => {
      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          poolPda.toBuffer(),
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
            pool: poolPda,
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
              pool: poolPda,
              userBet: betPda,
              magicProgram: MAGIC_PROGRAM_ID,
          })
          .rpc();
      }, "Undelegate");
    });

    it("Resolve, Calculate & Claim", async () => {
      console.log("    ⏳ Waiting 65s for pool expiry...");
      await sleep(65000); 

      // Resolve
      await program.methods
        .resolvePool(new anchor.BN(150_000_000))
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          pool: poolPda,
        })
        .rpc();

      const user = users[0];
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          SEED_BET,
          poolPda.toBuffer(),
          user.publicKey.toBuffer(),
          Buffer.from(requestId),
        ],
        program.programId
      );

      // Calculate Pool Outcome
      await program.methods
        .calculatePoolOutcome()
        .accounts({
          payer: user.publicKey,
          betOwner: user.publicKey,
          pool: poolPda,
          userBet: betPda,
        })
        .signers([user])
        .rpc();

      // Finalize Weights
      const [poolVaultPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("pool_vault"), poolPda.toBuffer()],
        program.programId
      );

      await program.methods
        .finalizeWeights()
        .accounts({
          admin: admin.publicKey,
          globalConfig: globalConfigPda,
          pool: poolPda,
          poolVault: poolVaultPda,
          treasuryWallet: treasuryUsdcAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      // Claim Pool Reward
      const preBal = (
        await provider.connection.getTokenAccountBalance(userATAs[0])
      ).value.uiAmount!;
      
      // const [poolVaultPda] = PublicKey.findProgramAddressSync(
      //   [Buffer.from("pool_vault"), poolPda.toBuffer()],
      //   program.programId
      // );
      
      await program.methods
        .claimPoolReward()
        .accounts({
          user: user.publicKey,
          pool: poolPda,
          poolVault: poolVaultPda,
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
});