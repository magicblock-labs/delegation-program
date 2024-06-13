import { PublicKey } from "@solana/web3.js";
import {
  DELEGATED_ACCOUNT_SEEDS,
  SEED_BUFFER_PDA,
  SEED_COMMIT_STATE_RECORD_PDA,
  SEED_DELEGATION_PDA,
  SEED_STATE_DIFF_PDA,
} from "./seeds";

import { DELEGATION_PROGRAM_ID } from "./constants";

/**
 * Get delegation accounts
 * @param accountToDelegate
 * @param ownerProgram
 * @constructor
 */
export function DelegateAccounts(
  accountToDelegate: PublicKey,
  ownerProgram: PublicKey,
) {
  return getAccounts(accountToDelegate, ownerProgram, true);
}

/**
 * Get undelegation accounts
 * @param accountToDelegate
 * @param ownerProgram
 * @constructor
 */
export function UndelegateAccounts(
  accountToDelegate: PublicKey,
  ownerProgram: PublicKey,
) {
  return getAccounts(accountToDelegate, ownerProgram, false);
}

function getAccounts(
  accountToDelegate: PublicKey,
  ownerProgram: PublicKey,
  ownedBuffer: boolean = true,
) {
  const pdaBytes = accountToDelegate.toBytes();

  const [delegationPda] = PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_DELEGATION_PDA), pdaBytes],
    new PublicKey(DELEGATION_PROGRAM_ID),
  );

  const [delegatedAccountSeedsPda] = PublicKey.findProgramAddressSync(
    [Buffer.from(DELEGATED_ACCOUNT_SEEDS), pdaBytes],
    new PublicKey(DELEGATION_PROGRAM_ID),
  );

  const [bufferPda] = PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_BUFFER_PDA), pdaBytes],
    ownedBuffer
      ? new PublicKey(ownerProgram)
      : new PublicKey(DELEGATION_PROGRAM_ID),
  );

  const [commitStateRecordPda] = PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_COMMIT_STATE_RECORD_PDA), pdaBytes],
    new PublicKey(DELEGATION_PROGRAM_ID),
  );

  const [commitStatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_STATE_DIFF_PDA), pdaBytes],
    new PublicKey(DELEGATION_PROGRAM_ID),
  );
  return {
    delegationPda,
    delegatedAccountSeedsPda,
    bufferPda,
    commitStateRecordPda,
    commitStatePda,
  };
}
