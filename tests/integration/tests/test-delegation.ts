import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import * as beet from "@metaplex-foundation/beet";
import { TestDelegation } from "../target/types/test_delegation";
import {
  createDelegateInstruction,
  delegationRecordPdaFromDelegatedAccount,
  delegationMetadataPdaFromDelegatedAccount,
  DELEGATION_PROGRAM_ID,
} from "@magicblock-labs/ephemeral-rollups-sdk-v2";
import { ON_CURVE_ACCOUNT } from "./fixtures/consts";

const SEED_TEST_PDA = "test-pda";

describe("TestDelegation", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const testDelegation = anchor.workspace
    .TestDelegation as Program<TestDelegation>;

  const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_TEST_PDA)],
    testDelegation.programId
  );
  const payer = provider.wallet.publicKey;
  const admin = provider.wallet.publicKey;
  const validator = provider.wallet.publicKey;
  const ownerProgram = testDelegation.programId;
  const reimbursement = provider.wallet.publicKey;

  it("Initialize protocol fees vault", async () => {
    const ix = createInitFeesVaultInstruction(payer);
    const txId = await processInstruction(ix);
    console.log("Initialize protocol fees vault tx:", txId);
  });

  it("Initialize validator fee vault", async () => {
    const ix = createInitValidatorFeesVaultInstruction(payer, admin, validator);
    const txId = await processInstruction(ix);
    console.log("Initialize validator fee vault tx:", txId);
  });

  it("Initializes the counter", async () => {
    // Check if the counter is initialized
    const counterAccountInfo = await provider.connection.getAccountInfo(pda);
    if (counterAccountInfo === null) {
      const tx = await testDelegation.methods
        .initialize()
        .accounts({
          user: provider.wallet.publicKey,
        })
        .rpc({ skipPreflight: true });
      console.log("Init Pda Tx: ", tx);
    }
    const counterAccount = await testDelegation.account.counter.fetch(pda);
    console.log("Counter: ", counterAccount.count.toString());
  });

  it("Initializes another counter", async () => {
    // Check if the counter is initialized
    const counterAccountInfo = await provider.connection.getAccountInfo(pda);
    if (counterAccountInfo === null) {
      const tx = await testDelegation.methods
        .initializeOther()
        .accounts({
          user: provider.wallet.publicKey,
        })
        .rpc({ skipPreflight: true });
      console.log("Init Pda Tx: ", tx);
    }
    const counterAccount = await testDelegation.account.counter.fetch(pda);
    console.log("Counter: ", counterAccount.count.toString());
  });

  it("Increase the counter", async () => {
    const tx = await testDelegation.methods
      .increment()
      .accounts({
        counter: pda,
      })
      .rpc({ skipPreflight: true });
    console.log("Increment Tx: ", tx);
    const counterAccount = await testDelegation.account.counter.fetch(pda);
    console.log("Counter: ", counterAccount.count.toString());
  });

  it("Delegate two PDAs", async () => {
    // Delegate, Close PDA, and Lock PDA in a single instruction
    const tx = await testDelegation.methods
      .delegateTwo()
      .accounts({
        payer: provider.wallet.publicKey,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });

  it("Delegate an on-curve account", async () => {
    const delegateOnCurve = ON_CURVE_ACCOUNT;

    // Airdrop SOL to create the account
    const airdropSignature = await provider.connection.requestAirdrop(
      delegateOnCurve.publicKey,
      web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSignature);

    let tx = new web3.Transaction()
      .add(
        web3.SystemProgram.assign({
          accountPubkey: delegateOnCurve.publicKey,
          programId: DELEGATION_PROGRAM_ID,
        })
      )
      .add(
        createDelegateInstruction({
          payer: provider.wallet.publicKey,
          delegateAccount: delegateOnCurve.publicKey,
          ownerProgram: web3.SystemProgram.programId,
        })
      );
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    tx.partialSign(delegateOnCurve);
    tx = await provider.wallet.signTransaction(tx);
    const txSign = await provider.sendAndConfirm(tx, [], {
      skipPreflight: true,
    });

    console.log("Your transaction signature", txSign);
  });

  it("Commit a new state to the PDA", async () => {
    let account = await provider.connection.getAccountInfo(pda);
    let new_data = account.data;
    new_data[-1] = (new_data[-1] + 1) % 256;

    const args: CommitAccountInstructionArgs = {
      slot: new anchor.BN(40),
      lamports: new anchor.BN(1000000000),
      allow_undelegation: false,
      data: new_data,
    };
    const ix = createCommitAccountInstruction(
      validator,
      pda,
      ownerProgram,
      args
    );
    const txId = await processInstruction(ix);
    console.log("Commit state signature", txId);
  });

  it("Finalize account state", async () => {
    const ix = createFinalizeInstruction(validator, pda);
    const txId = await processInstruction(ix);
    console.log("Finalize signature", txId);
  });

  it("Commit a new state to the PDA", async () => {
    let account = await provider.connection.getAccountInfo(pda);
    let new_data = account.data;
    new_data[-1] = (new_data[-1] + 1) % 256;

    const args: CommitAccountInstructionArgs = {
      slot: new anchor.BN(40),
      lamports: new anchor.BN(1000000000),
      allow_undelegation: true,
      data: new_data,
    };
    const ix = createCommitAccountInstruction(
      validator,
      pda,
      ownerProgram,
      args
    );
    const txId = await processInstruction(ix);
    console.log("Commit state signature", txId);
  });

  it("Finalize account state again", async () => {
    const ix = createFinalizeInstruction(validator, pda);
    const txId = await processInstruction(ix);
    console.log("Finalize signature", txId);
  });

  it("Undelegate account", async () => {
    const ix = createUndelegateInstruction(
      validator,
      pda,
      ownerProgram,
      reimbursement
    );
    const txId = await processInstruction(ix);
    console.log("Undelegate signature", txId);
  });

  it("Whitelist a validator for a program", async () => {
    const ix = createWhitelistValidatorForProgramInstruction(
      admin,
      validator,
      testDelegation.programId,
      true
    );
    const txId = await processInstruction(ix);
    console.log("Whitelist a validator for a program:", txId);
  });

  async function processInstruction(ix: web3.TransactionInstruction) {
    const tx = new web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    const txId = await provider.sendAndConfirm(tx, [], { skipPreflight: true });
    return txId;
  }

  /// Instruction to commit a new state to the PDA

  interface CommitStateAccounts {
    authority: web3.PublicKey;
    delegatedAccount: web3.PublicKey;
    delegatedAccountOwner: web3.PublicKey;
    commitState: web3.PublicKey;
    commitRecord: web3.PublicKey;
    delegationRecord: web3.PublicKey;
    delegationMetadata: web3.PublicKey;
  }

  interface CommitAccountInstructionArgs {
    slot: beet.bignum;
    lamports: beet.bignum;
    allow_undelegation: boolean;
    data: Uint8Array;
  }

  const commitAccountStruct = new beet.FixableBeetArgsStruct<
    CommitAccountInstructionArgs & {
      instructionDiscriminator: number[] /* size: 8 */;
    }
  >(
    [
      ["instructionDiscriminator", beet.uniformFixedSizeArray(beet.u8, 8)],
      ["slot", beet.u64],
      ["lamports", beet.u64],
      ["allow_undelegation", beet.bool],
      ["data", beet.bytes],
    ],
    "CommitStateAccountArgs"
  );

  function createCommitAccountInstruction(
    validator: web3.PublicKey,
    delegatedAccount: web3.PublicKey,
    ownerProgramId: web3.PublicKey,
    args: CommitAccountInstructionArgs
  ) {
    const commitState = commitStatePdaFromDelegatedAccount(pda);
    const commitRecord = commitRecordPdaFromDelegatedAccount(pda);
    const delegationRecord = delegationRecordPdaFromDelegatedAccount(pda);
    const delegationMetadata = delegationMetadataPdaFromDelegatedAccount(pda);
    const validatorFeesVault = validatorFeesVaultPdaFromValidator(validator);
    const programConfig = programConfigPdaFromProgramId(ownerProgramId);
    const keys = [
      { pubkey: validator, isSigner: true, isWritable: false },
      { pubkey: delegatedAccount, isSigner: false, isWritable: false },
      { pubkey: commitState, isSigner: false, isWritable: true },
      { pubkey: commitRecord, isSigner: false, isWritable: true },
      { pubkey: delegationRecord, isSigner: false, isWritable: true },
      { pubkey: delegationMetadata, isSigner: false, isWritable: true },
      { pubkey: validatorFeesVault, isSigner: false, isWritable: true },
      { pubkey: programConfig, isSigner: false, isWritable: false },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const [data] = commitAccountStruct.serialize({
      instructionDiscriminator: [1, 0, 0, 0, 0, 0, 0, 0],
      ...args,
    });
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }

  function createFinalizeInstruction(
    validator: web3.PublicKey,
    delegatedAccount: web3.PublicKey
  ) {
    const commitState = commitStatePdaFromDelegatedAccount(pda);
    const commitRecord = commitRecordPdaFromDelegatedAccount(pda);
    const delegationRecord = delegationRecordPdaFromDelegatedAccount(pda);
    const delegationMetadata = delegationMetadataPdaFromDelegatedAccount(pda);
    const validatorFeesVault = validatorFeesVaultPdaFromValidator(validator);
    const keys = [
      { pubkey: validator, isSigner: true, isWritable: false },
      { pubkey: delegatedAccount, isSigner: false, isWritable: true },
      { pubkey: commitState, isSigner: false, isWritable: true },
      { pubkey: commitRecord, isSigner: false, isWritable: true },
      { pubkey: delegationRecord, isSigner: false, isWritable: true },
      { pubkey: delegationMetadata, isSigner: false, isWritable: true },
      { pubkey: validatorFeesVault, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const data = Buffer.from([2, 0, 0, 0, 0, 0, 0, 0]);
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }

  function createUndelegateInstruction(
    validator: web3.PublicKey,
    delegatedAccount: web3.PublicKey,
    ownerProgramId: web3.PublicKey,
    reimbursement: web3.PublicKey
  ) {
    const buffer = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("buffer"), pda.toBytes()],
      DELEGATION_PROGRAM_ID
    )[0];
    const commitState = commitStatePdaFromDelegatedAccount(pda);
    const commitRecord = commitRecordPdaFromDelegatedAccount(pda);
    const delegationRecord = delegationRecordPdaFromDelegatedAccount(pda);
    const delegationMetadata = delegationMetadataPdaFromDelegatedAccount(pda);
    const feesVault = feesVaultPda();
    const validatorFeesVault = validatorFeesVaultPdaFromValidator(validator);
    const keys = [
      { pubkey: validator, isSigner: true, isWritable: false },
      { pubkey: delegatedAccount, isSigner: false, isWritable: true },
      { pubkey: ownerProgramId, isSigner: false, isWritable: false },
      { pubkey: buffer, isSigner: false, isWritable: true },
      { pubkey: commitState, isSigner: false, isWritable: true },
      { pubkey: commitRecord, isSigner: false, isWritable: true },
      { pubkey: delegationRecord, isSigner: false, isWritable: true },
      { pubkey: delegationMetadata, isSigner: false, isWritable: true },
      { pubkey: reimbursement, isSigner: false, isWritable: true },
      { pubkey: feesVault, isSigner: false, isWritable: true },
      { pubkey: validatorFeesVault, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const data = Buffer.from([3, 0, 0, 0, 0, 0, 0, 0]);
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }

  /// Instruction to initialize protocol fees vault
  function createInitFeesVaultInstruction(payer: web3.PublicKey) {
    const feesVault = feesVaultPda();
    const keys = [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: feesVault, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const data = Buffer.from([5, 0, 0, 0, 0, 0, 0, 0]);
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }

  /// Instruction to initialize a fees vault for a validator authority
  function createInitValidatorFeesVaultInstruction(
    payer: web3.PublicKey,
    admin: web3.PublicKey,
    validator: web3.PublicKey
  ) {
    const validatorFeesVault = validatorFeesVaultPdaFromValidator(validator);
    const keys = [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: validator, isSigner: false, isWritable: false },
      { pubkey: validatorFeesVault, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const data = Buffer.from([6, 0, 0, 0, 0, 0, 0, 0]);
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }

  function createWhitelistValidatorForProgramInstruction(
    admin: web3.PublicKey,
    validator: web3.PublicKey,
    program: web3.PublicKey,
    insert: boolean
  ) {
    const programData = web3.PublicKey.findProgramAddressSync(
      [program.toBuffer()],
      new web3.PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
    )[0];
    const programConfig = programConfigPdaFromProgramId(program);
    const keys = [
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: validator, isSigner: false, isWritable: false },
      { pubkey: program, isSigner: false, isWritable: false },
      { pubkey: programData, isSigner: false, isWritable: false },
      { pubkey: programConfig, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];
    const data = Buffer.from([8, 0, 0, 0, 0, 0, 0, 0, insert ? 1 : 0]);
    const ix = new web3.TransactionInstruction({
      programId: DELEGATION_PROGRAM_ID,
      keys,
      data,
    });
    return ix;
  }
});

function commitStatePdaFromDelegatedAccount(delegatedAccount: web3.PublicKey) {
  return web3.PublicKey.findProgramAddressSync(
    [Buffer.from("state-diff"), delegatedAccount.toBytes()],
    DELEGATION_PROGRAM_ID
  )[0];
}

function commitRecordPdaFromDelegatedAccount(delegatedAccount: web3.PublicKey) {
  return web3.PublicKey.findProgramAddressSync(
    [Buffer.from("commit-state-record"), delegatedAccount.toBytes()],
    DELEGATION_PROGRAM_ID
  )[0];
}

function feesVaultPda() {
  return web3.PublicKey.findProgramAddressSync(
    [Buffer.from("fees-vault")],
    DELEGATION_PROGRAM_ID
  )[0];
}

function validatorFeesVaultPdaFromValidator(validator: web3.PublicKey) {
  return web3.PublicKey.findProgramAddressSync(
    [Buffer.from("v-fees-vault"), validator.toBuffer()],
    DELEGATION_PROGRAM_ID
  )[0];
}

function programConfigPdaFromProgramId(programId: web3.PublicKey) {
  return web3.PublicKey.findProgramAddressSync(
    [Buffer.from("p-conf"), programId.toBuffer()],
    DELEGATION_PROGRAM_ID
  )[0];
}
