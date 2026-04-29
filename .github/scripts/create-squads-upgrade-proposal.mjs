import fs from "node:fs";
import * as multisig from "@sqds/multisig";
import {
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
} from "@solana/web3.js";

const BPF_UPGRADEABLE_LOADER_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111",
);

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
  instructions: [upgradeInstruction],
});

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
await connection.confirmTransaction(vaultTransactionSignature, "confirmed");

const proposalSignature = await multisig.rpc.proposalCreate({
  connection,
  feePayer: proposer,
  multisigPda,
  transactionIndex,
  creator: proposer,
});
await connection.confirmTransaction(proposalSignature, "confirmed");

fs.appendFileSync(
  env("GITHUB_ENV"),
  `SQUADS_TRANSACTION_INDEX=${transactionIndex.toString()}\n`,
);

console.log(`Squads vault transaction signature: ${vaultTransactionSignature}`);
console.log(`Squads proposal signature: ${proposalSignature}`);
console.log(`Squads transaction index: ${transactionIndex.toString()}`);
