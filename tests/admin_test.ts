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
import * as fs from "fs";
import * as path from "path";

describe("1. Admin & Config Tests", () => {
  // Configure the client to use the local cluster (or devnet from provider)
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SwivPrivacy as Program<SwivPrivacy>;

  // --- Constants & Keypairs ---
  const admin = provider.wallet as anchor.Wallet;
  // We generate a random key for treasury initially
  const treasury = Keypair.generate(); 
  
  // Real Devnet Pyth Feed for SOL/USD
  const SOL_USD_FEED = new PublicKey("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");

  // PDAs
  let globalConfigPda: PublicKey;
  let assetConfigPda: PublicKey;

  // Mock Token Info
  let usdcMint: PublicKey;
  let adminUsdcAccount: PublicKey;

  // Constants
  const SEED_GLOBAL_CONFIG = Buffer.from("global_config_v1");
  const SEED_ASSET_CONFIG = Buffer.from("asset_config");
  const ASSET_SYMBOL = "SOL";

  it("Setup: Create Mock USDC Mint and Mint to Admin", async () => {
    // 1. Create a new Mint (acting as USDC)
    usdcMint = await createMint(
      provider.connection,
      admin.payer,
      admin.publicKey,
      null,
      6 
    );
    console.log("Mock USDC Mint:", usdcMint.toBase58());

    // 2. Get ATA for Admin
    const adminAtaInfo = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin.payer,
      usdcMint,
      admin.publicKey
    );
    adminUsdcAccount = adminAtaInfo.address;

    // 3. Mint 1,000,000 USDC to Admin
    await mintTo(
      provider.connection,
      admin.payer,
      usdcMint,
      adminUsdcAccount,
      admin.payer,
      1_000_000 * 1_000_000 
    );

    const balance = await provider.connection.getTokenAccountBalance(adminUsdcAccount);
    assert.equal(balance.value.uiAmount, 1_000_000);
    console.log("Minted 1M Mock USDC to Admin");
  });

  it("Initialize Protocol", async () => {
    // Derive Global Config PDA
    [globalConfigPda] = PublicKey.findProgramAddressSync(
      [SEED_GLOBAL_CONFIG],
      program.programId
    );

    const houseFee = new anchor.BN(100); // 1.00%
    const pariFee = new anchor.BN(250);  // 2.50%
    const allowedAssets = [usdcMint];

    try {
      // 1. Try to fetch to see if it exists
      const configAccount = await program.account.globalConfig.fetch(globalConfigPda);
      console.log("✅ Protocol Initialized (Skipping init step)");

      // Safety Check: Ensure we own it
      if (!configAccount.admin.equals(admin.publicKey)) {
        throw new Error(`Admin mismatch! On-chain: ${configAccount.admin.toBase58()}, Local: ${admin.publicKey.toBase58()}`);
      }

    } catch (e: any) {
      // 2. Only Initialize if account is missing
      if (e.message.includes("Account does not exist") || e.message.includes("not found")) {
        await program.methods
          .initializeProtocol(houseFee, pariFee, allowedAssets)
          .accounts({
            admin: admin.publicKey,
            treasuryWallet: treasury.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        console.log("✅ Protocol Initialized");
      } else {
        throw e;
      }
    }

    // Verification
    const configAccount = await program.account.globalConfig.fetch(globalConfigPda);
    assert.ok(configAccount.admin.equals(admin.publicKey));
    assert.equal(configAccount.paused, false);
  });

  it("Config Asset (SOL)", async () => {
    // Derive Asset Config PDA
    [assetConfigPda] = PublicKey.findProgramAddressSync(
      [SEED_ASSET_CONFIG, Buffer.from(ASSET_SYMBOL)],
      program.programId
    );

    const volatility = new anchor.BN(1500); // 1.5x
    const usePythVol = false;

    try {
      await program.account.assetConfig.fetch(assetConfigPda);
      console.log(`✅ Asset ${ASSET_SYMBOL} Configured (Skipping)`);
    } catch (e: any) {
      if (e.message.includes("Account does not exist") || e.message.includes("not found")) {
        await program.methods
          .configAsset(ASSET_SYMBOL, SOL_USD_FEED, volatility, usePythVol)
          .accounts({
            admin: admin.publicKey,
            pythFeed: SOL_USD_FEED,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        console.log("✅ Asset Configured: SOL");
      } else {
        throw e;
      }
    }

    // Verification
    const assetAccount = await program.account.assetConfig.fetch(assetConfigPda);
    assert.equal(assetAccount.symbol, ASSET_SYMBOL);
    assert.ok(assetAccount.pythFeed.equals(SOL_USD_FEED));
  });

  it("Admin Action: Set Pause", async () => {
    // Pause
    await program.methods
      .setPause(true)
      .accounts({
        admin: admin.publicKey,
      })
      .rpc();

    let config = await program.account.globalConfig.fetch(globalConfigPda);
    assert.equal(config.paused, true);

    // Unpause
    await program.methods
      .setPause(false)
      .accounts({
        admin: admin.publicKey,
      })
      .rpc();

    config = await program.account.globalConfig.fetch(globalConfigPda);
    assert.equal(config.paused, false);
    console.log("Pause/Unpause Verified");
  });

  // --- NEW TEST START ---
  it("Admin Action: Update Global Config", async () => {
    // 1. Prepare new values
    const newTreasury = Keypair.generate(); // New random wallet
    const newHouseFee = new anchor.BN(150); // Change to 1.5%
    const newPariFee = new anchor.BN(300);  // Change to 3.0%
    
    // Create a random public key to act as a second asset (e.g., Mock BTC)
    // This tests the vector resizing (realloc)
    const secondAsset = Keypair.generate().publicKey; 
    const newAllowedAssets = [usdcMint, secondAsset];

    // 2. Call update_config
    await program.methods
      .updateConfig(
        newTreasury.publicKey, // new_treasury
        newPariFee,            // new_parimutuel_fee_bps
        newHouseFee,           // new_house_fee_bps
        newAllowedAssets       // new_allowed_assets
      )
      .accounts({
        admin: admin.publicKey,
        globalConfig: globalConfigPda, 
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // 3. Verification
    const config = await program.account.globalConfig.fetch(globalConfigPda);
    
    assert.ok(config.treasuryWallet.equals(newTreasury.publicKey), "Treasury wallet was not updated");
    assert.equal(config.houseFeeBps.toNumber(), 150, "House fee was not updated");
    assert.equal(config.parimutuelFeeBps.toNumber(), 300, "Parimutuel fee was not updated");
    assert.equal(config.allowedAssets.length, 2, "Allowed assets list size mismatch");
    assert.ok(config.allowedAssets[1].equals(secondAsset), "Second asset not found in list");

    console.log("Global Config Updated: Fees, Treasury, and Asset List modified");
  });
  // --- NEW TEST END ---

  it("Admin Action: Transfer Admin (Safe Mode)", async () => {
    const newAdmin = Keypair.generate();
    // Save backup IMMEDIATELY
    const backupPath = path.join(__dirname, "temp_admin.json");
    fs.writeFileSync(backupPath, JSON.stringify(Array.from(newAdmin.secretKey)));
    
    // Fund temp admin
    const tx = new anchor.web3.Transaction().add(
      SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: newAdmin.publicKey,
        lamports: 0.05 * LAMPORTS_PER_SOL, 
      })
    );
    await provider.sendAndConfirm(tx);

    // Transfer TO temp admin
    await program.methods
      .transferAdmin(newAdmin.publicKey)
      .accounts({ currentAdmin: admin.publicKey })
      .rpc();

    // --- SAFETY BLOCK START ---
    try {
        // Perform your checks here
        let config = await program.account.globalConfig.fetch(globalConfigPda);
        assert.ok(config.admin.equals(newAdmin.publicKey));
        console.log("    ✔ Ownership transferred to Temp Admin");

    } finally {
        // THIS RUNS NO MATTER WHAT HAPPENS ABOVE
        console.log("    ⚠️ Attempting to restore original admin...");
        try {
            await program.methods
                .transferAdmin(admin.publicKey)
                .accounts({ currentAdmin: newAdmin.publicKey })
                .signers([newAdmin])
                .rpc();
            
            console.log("    ✔ Admin restored successfully");
            
            // Only delete backup if restoration succeeded
            if (fs.existsSync(backupPath)) fs.unlinkSync(backupPath);
            
        } catch (cleanupError) {
            console.error("    ❌ CRITICAL: FAILED TO RESTORE ADMIN. USE BACKUP FILE.");
            console.error(cleanupError);
        }
    }
    // --- SAFETY BLOCK END ---

    // Final check to ensure test passes only if admin is back
    const finalConfig = await program.account.globalConfig.fetch(globalConfigPda);
    assert.ok(finalConfig.admin.equals(admin.publicKey));
  });
});