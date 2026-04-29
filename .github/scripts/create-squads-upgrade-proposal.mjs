import fs from "node:fs";
import { execFileSync } from "node:child_process";
import * as multisig from "@sqds/multisig";
import {
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
} from "@solana/web3.js";

const BPF_UPGRADEABLE_LOADER_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111",
);
const COMPUTE_BUDGET_PROGRAM_ID =
  "ComputeBudget111111111111111111111111111111";

function env(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function keypairFromFile(path) {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path, "utf8"))),
  );
}

async function confirmOrThrow(connection, signature, label) {
  const result = await connection.confirmTransaction(signature, "confirmed");
  if (result.value.err) {
    throw new Error(
      `${label} failed on-chain (signature ${signature}): ${JSON.stringify(result.value.err)}`,
    );
  }
}

const connection = new Connection(env("MAINNET_RPC_URL"), "confirmed");
const proposer = keypairFromFile(env("PROPOSER_KEYPAIR_PATH"));
const multisigPda = new PublicKey(env("SQUADS_MULTISIG_PDA"));
const vaultIndex = Number(env("SQUADS_VAULT_INDEX"));
const programId = new PublicKey(env("PROGRAM_ID"));
const bufferAddress = new PublicKey(env("PROGRAM_BUFFER_ADDRESS"));
const spillAddress = new PublicKey(env("SPILL_ADDRESS"));
const programDataAddress = new PublicKey(env("PROGRAMDATA_ADDRESS"));
const proposalName = env("PROPOSAL_NAME");

const [vaultPda] = multisig.getVaultPda({
  multisigPda,
  index: vaultIndex,
});

const upgradeInstruction = new TransactionInstruction({
  programId: BPF_UPGRADEABLE_LOADER_ID,
  data: Buffer.from([3, 0, 0, 0]),
  keys: [
    { pubkey: programDataAddress, isWritable: true, isSigner: false },
    { pubkey: programId, isWritable: true, isSigner: false },
    { pubkey: bufferAddress, isWritable: true, isSigner: false },
    { pubkey: spillAddress, isWritable: true, isSigner: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isWritable: false, isSigner: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isWritable: false, isSigner: false },
    { pubkey: vaultPda, isWritable: false, isSigner: true },
  ],
});

// Reuse `solana-verify` to construct the otter-verify PDA write so the upgrade
// and the verify-PDA land in a single multisig execution. Compute budget ixs
// in the exported tx are dropped — Squads adds its own at execution time.
const verifyExportRaw = execFileSync(
  "solana-verify",
  [
    "export-pda-tx",
    env("VERIFY_REPOSITORY_URL"),
    "--program-id", programId.toBase58(),
    "--uploader", vaultPda.toBase58(),
    "--library-name", env("PROGRAM_LIB_NAME"),
    "--commit-hash", env("GITHUB_SHA"),
    "--encoding", "base64",
    "--compute-unit-price", "0",
  ],
  { encoding: "utf8" },
);
const verifyTxB64 = verifyExportRaw
  .split(/\r?\n/)
  .map((l) => l.trim())
  .filter((l) => l.length > 0)
  .pop();
if (!verifyTxB64) {
  throw new Error("solana-verify export-pda-tx produced no output");
}
const verifyInstructions = Transaction.from(
  Buffer.from(verifyTxB64, "base64"),
).instructions.filter(
  (ix) => ix.programId.toBase58() !== COMPUTE_BUDGET_PROGRAM_ID,
);
if (verifyInstructions.length === 0) {
  throw new Error(
    "solana-verify export-pda-tx returned no non-ComputeBudget instructions",
  );
}

const multisigInfo =
  await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    multisigPda,
  );
const transactionIndex = BigInt(Number(multisigInfo.transactionIndex) + 1);
const blockhash = (await connection.getLatestBlockhash()).blockhash;
const transactionMessage = new TransactionMessage({
  payerKey: vaultPda,
  recentBlockhash: blockhash,
  instructions: [upgradeInstruction, ...verifyInstructions],
});

// Squads wraps this message; the wrapped tx must still fit in a 1232-byte packet.
const compiledSize = transactionMessage.compileToV0Message().serialize().length;
console.log(`Bundled message size: ${compiledSize} bytes`);
if (compiledSize > 1100) {
  console.warn(
    `Warning: bundled message is ${compiledSize} bytes; close to the 1232-byte packet limit after Squads wrapping.`,
  );
}

const vaultTransactionSignature = await multisig.rpc.vaultTransactionCreate({
  connection,
  feePayer: proposer,
  multisigPda,
  transactionIndex,
  creator: proposer.publicKey,
  vaultIndex,
  ephemeralSigners: 0,
  transactionMessage,
  memo: proposalName,
});
await confirmOrThrow(connection, vaultTransactionSignature, "vaultTransactionCreate");

const proposalSignature = await multisig.rpc.proposalCreate({
  connection,
  feePayer: proposer,
  multisigPda,
  transactionIndex,
  creator: proposer,
});
await confirmOrThrow(connection, proposalSignature, "proposalCreate");

fs.appendFileSync(
  env("GITHUB_ENV"),
  `SQUADS_TRANSACTION_INDEX=${transactionIndex.toString()}\n`,
);

console.log(`Squads vault transaction signature: ${vaultTransactionSignature}`);
console.log(`Squads proposal signature: ${proposalSignature}`);
console.log(`Squads transaction index: ${transactionIndex.toString()}`);
