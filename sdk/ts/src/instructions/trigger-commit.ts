import * as beet from "@metaplex-foundation/beet";
import * as web3 from "@solana/web3.js";
import { PublicKey } from "@solana/web3.js";
import { MAGIC_PROGRAM_ID } from "../constants";

export const commitStruct = new beet.FixableBeetArgsStruct<{
  instructionDiscriminator: number[];
}>(
  [["instructionDiscriminator", beet.uniformFixedSizeArray(beet.u8, 4)]],
  "CommitInstructionArgs",
);

/**
 * Accounts required by the _undelegate_ instruction
 */

export interface CommitInstructionAccounts {
  payer: web3.PublicKey;
  delegatedAccount: web3.PublicKey;
}

export const commitInstructionDiscriminator = [1, 0, 0, 0];

/**
 * Creates an _undelegate_ instruction.
 *
 */

export function createCommitInstruction(
  accounts: CommitInstructionAccounts,
  programId = new PublicKey(MAGIC_PROGRAM_ID),
) {
  const [data] = commitStruct.serialize({
    instructionDiscriminator: commitInstructionDiscriminator,
  });

  const keys: web3.AccountMeta[] = [
    {
      pubkey: accounts.payer,
      isWritable: false,
      isSigner: true,
    },
    {
      pubkey: accounts.delegatedAccount,
      isWritable: true,
      isSigner: false,
    },
  ];

  return new web3.TransactionInstruction({
    programId,
    keys,
    data,
  });
}
