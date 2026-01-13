import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SwivPrivacy } from "../target/types/swiv_privacy";
import { Keypair, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

async function restore() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SwivPrivacy as Program<SwivPrivacy>;

  // 1. Load the "Lockout" Keypair
  const keyPath = path.join(__dirname, "temp_admin.json");
  if (!fs.existsSync(keyPath)) {
    console.error("❌ temp_admin.json not found! Cannot restore.");
    return;
  }
  const secretKey = JSON.parse(fs.readFileSync(keyPath, "utf-8"));
  const tempAdmin = Keypair.fromSecretKey(new Uint8Array(secretKey));

  console.log(`Using Temp Admin: ${tempAdmin.publicKey.toBase58()}`);

  // 2. Derive Global Config
  const [globalConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("global_config_v1")],
    program.programId
  );

  // 3. Check Current Admin
  const config = await program.account.globalConfig.fetch(globalConfigPda);
  console.log(`Current On-Chain Admin: ${config.admin.toBase58()}`);

  if (config.admin.equals(provider.wallet.publicKey)) {
    console.log("✅ Admin is ALREADY the provider wallet. No action needed.");
    return;
  }

  // 4. Transfer Back
  console.log("Restoring original admin...");
  try {
    await program.methods
      .transferAdmin(provider.wallet.publicKey)
      .accounts({
        currentAdmin: tempAdmin.publicKey,
        // globalConfig is likely inferred, but if your instruction needs it explicitly, add it:
        // globalConfig: globalConfigPda, 
      })
      .signers([tempAdmin]) // <--- Signing as the temp admin
      .rpc();
    
    console.log("✅ Success! Admin restored to provider wallet.");
    
    // Optional: Delete the file now that we are safe
    // fs.unlinkSync(keyPath);
  } catch (e) {
    console.error("Failed to restore:", e);
  }
}

restore();