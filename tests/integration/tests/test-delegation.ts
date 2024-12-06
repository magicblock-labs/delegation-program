import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import * as beet from "@metaplex-foundation/beet";
import { TestDelegation } from "../target/types/test_delegation";
import {
  createDelegateInstruction, createUndelegateInstruction,
  DelegateAccounts,
  DELEGATION_PROGRAM_ID,
} from "@magicblock-labs/ephemeral-rollups-sdk-v2";
import { ON_CURVE_ACCOUNT } from "./fixtures/consts";

const SEED_TEST_PDA = "test-pda";

const delegationProgramIdlPath = "./idls/delegation.json";

describe("TestDelegation", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const testDelegation = anchor.workspace
    .TestDelegation as Program<TestDelegation>;
  const idl = JSON.parse(
    require("fs").readFileSync(delegationProgramIdlPath, "utf8")
  );
  const dlpProgram = new Program(idl, provider);

  const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_TEST_PDA)],
    testDelegation.programId
  );

  const validatorFeeVaultPda = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("v-fees-vault"), provider.wallet.publicKey.toBuffer()],
    dlpProgram.programId
  )[0];

  it("Initialize protocol fees vault", async () => {
    const ix = createInitFeesVaultInstruction(provider.wallet.publicKey);
    const tx = new web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    const txId = await provider.sendAndConfirm(tx, [], { skipPreflight: true });
    console.log("Initialize protocol fees vault tx:", txId);
  });

  it("Initialize validator fee vault", async () => {
    const ix = createInitValidatorFeesVaultInstruction(
      provider.wallet.publicKey,
      provider.wallet.publicKey,
      provider.wallet.publicKey
    );
    const tx = new web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    const txId = await provider.sendAndConfirm(tx);
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

  it("Delegate a PDA", async () => {
    // Delegate, Close PDA, and Lock PDA in a single instruction
    const tx = await testDelegation.methods
      .delegate()
      .accounts({
        payer: provider.wallet.publicKey,
      })
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });

  it("Allow Undelegation and Undelegate", async () => {
    await allow_undelegation(pda);
    await undelegate(pda);
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
          programId: new web3.PublicKey(DELEGATION_PROGRAM_ID),
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
    const {
      delegationRecord,
      delegationMetadata,
      bufferPda,
      commitStateRecordPda,
      commitStatePda,
    } = DelegateAccounts(pda, testDelegation.programId);

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
      {
        authority: provider.wallet.publicKey,
        delegatedAccount: pda,
        delegatedAccountOwner: testDelegation.programId,
        commitStatePda: commitStatePda,
        commitStateRecordPda: commitStateRecordPda,
        delegationRecordPda: delegationRecord,
        delegationMetadataPda: delegationMetadata,
      },
      args
    );

    let tx = new anchor.web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    tx = await provider.wallet.signTransaction(tx);
    const txSign = await provider.sendAndConfirm(tx, [], {
      skipPreflight: true,
    });

    console.log("Commit state signature", txSign);
  });

  it("Finalize account state", async () => {
    const {
      delegationRecord,
      delegationMetadata,
      bufferPda,
      commitStateRecordPda,
      commitStatePda,
    } = DelegateAccounts(pda, testDelegation.programId);

    // @ts-ignore
    const tx = await dlpProgram.methods
      .finalize()
      .accounts({
        validator: provider.wallet.publicKey,
        delegatedAccount: pda,
        committedStateAccount: commitStatePda,
        committedStateRecord: commitStateRecordPda,
        delegationRecord: delegationRecord,
        delegationMetadata: delegationMetadata,
        validatorFeesVault: validatorFeeVaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc({ skipPreflight: true });
    console.log("Finalize signature", tx);
  });

  it("Commit a new state to the PDA", async () => {
    const {
      delegationRecord,
      delegationMetadata,
      bufferPda,
      commitStateRecordPda,
      commitStatePda,
    } = DelegateAccounts(pda, testDelegation.programId);

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
      {
        authority: provider.wallet.publicKey,
        delegatedAccount: pda,
        delegatedAccountOwner: testDelegation.programId,
        commitStatePda: commitStatePda,
        commitStateRecordPda: commitStateRecordPda,
        delegationRecordPda: delegationRecord,
        delegationMetadataPda: delegationMetadata,
      },
      args
    );

    let tx = new anchor.web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    tx = await provider.wallet.signTransaction(tx);
    const txSign = await provider.sendAndConfirm(tx);

    console.log("Commit state signature", txSign);
  });

  async function allow_undelegation(pda: web3.PublicKey) {
    const {
      delegationRecord,
      delegationMetadata,
      bufferPda,
    } = DelegateAccounts(pda, testDelegation.programId);
    const txSign = await testDelegation.methods
        .allowUndelegation()
        .accounts({
          delegationRecord: delegationRecord,
          delegationMetadata: delegationMetadata,
          buffer: bufferPda,
          delegationProgram: DELEGATION_PROGRAM_ID,
        })
        .rpc({skipPreflight: true});
    console.log("Allow Undelegation signature", txSign);
  }

  it("Allow Undelegation", async () => {
    await allow_undelegation(pda);
  });

  async function undelegate(pda: web3.PublicKey) {
    const ix = createUndelegateInstruction({
      validator: provider.wallet.publicKey,
      delegatedAccount: pda,
      ownerProgram: testDelegation.programId,
      reimbursement: provider.wallet.publicKey,
    });

    let tx = new anchor.web3.Transaction().add(ix);
    tx.recentBlockhash = (
        await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    tx = await provider.wallet.signTransaction(tx);
    const txSign = await provider.sendAndConfirm(tx, [], {
      skipPreflight: true,
    });
    console.log("Undelegate signature", txSign);
  }

  it("Undelegate account", async () => {
    await undelegate(pda);
  });

  it("Whitelist a validator for a program", async () => {
    const ix = createWhitelistValidatorForProgramInstruction(
      provider.wallet.publicKey,
      provider.wallet.publicKey,
      testDelegation.programId,
      true
    );
    const tx = new web3.Transaction().add(ix);
    tx.recentBlockhash = (
      await provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = provider.wallet.publicKey;
    const txId = await provider.sendAndConfirm(tx, [], { skipPreflight: true });
    console.log("Whitelist a validator for a program:", txId);
  });

  /// Instruction to commit a new state to the PDA

  interface CommitStateAccounts {
    authority: web3.PublicKey;
    delegatedAccount: web3.PublicKey;
    delegatedAccountOwner: web3.PublicKey;
    commitStatePda: web3.PublicKey;
    commitStateRecordPda: web3.PublicKey;
    delegationRecordPda: web3.PublicKey;
    delegationMetadataPda: web3.PublicKey;
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
    accounts: CommitStateAccounts,
    args: CommitAccountInstructionArgs,
    programId = dlpProgram.programId
  ) {
    const [data] = commitAccountStruct.serialize({
      instructionDiscriminator: [1, 0, 0, 0, 0, 0, 0, 0],
      ...args,
    });
    const validatorFeesVaultPda = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("v-fees-vault"), accounts.authority.toBuffer()],
      new anchor.web3.PublicKey(DELEGATION_PROGRAM_ID)
    )[0];
    const programConfig = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("p-conf"), accounts.delegatedAccountOwner.toBuffer()],
      programId
    )[0];
    const keys = [
      { pubkey: accounts.authority, isSigner: true, isWritable: false },
      { pubkey: accounts.delegatedAccount, isSigner: false, isWritable: false },
      { pubkey: accounts.commitStatePda, isSigner: false, isWritable: true },
      {
        pubkey: accounts.commitStateRecordPda,
        isSigner: false,
        isWritable: true,
      },
      {
        pubkey: accounts.delegationRecordPda,
        isSigner: false,
        isWritable: true,
      },
      {
        pubkey: accounts.delegationMetadataPda,
        isSigner: false,
        isWritable: true,
      },
      { pubkey: validatorFeesVaultPda, isSigner: false, isWritable: true },
      { pubkey: programConfig, isSigner: false, isWritable: false },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];

    const ix = new web3.TransactionInstruction({
      programId,
      keys,
      data,
    });
    return ix;
  }

  /// Instruction to initialize protocol fees vault
  function createInitFeesVaultInstruction(
      payer: web3.PublicKey,
      programId = new web3.PublicKey(DELEGATION_PROGRAM_ID)
  ) {
    const feesVaultPda = web3.PublicKey.findProgramAddressSync(
        [Buffer.from("fees-vault")],
        programId
    )[0];

    const keys = [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: feesVaultPda, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];

    const data = Buffer.from([5, 0, 0, 0, 0, 0, 0, 0]);

    const ix = new web3.TransactionInstruction({
      programId,
      keys,
      data,
    });
    return ix;
  }

  /// Instruction to initialize a fees vault for a validator authority
  function createInitValidatorFeesVaultInstruction(
    payer: web3.PublicKey,
    admin: web3.PublicKey,
    validatorIdentity: web3.PublicKey,
    programId = new web3.PublicKey(DELEGATION_PROGRAM_ID)
  ) {
    const validatorFeesVaultPda = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("v-fees-vault"), validatorIdentity.toBuffer()],
      programId
    )[0];

    const keys = [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: validatorIdentity, isSigner: false, isWritable: false },
      { pubkey: validatorFeesVaultPda, isSigner: false, isWritable: true },
      {
        pubkey: web3.SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
    ];

    const data = Buffer.from([6, 0, 0, 0, 0, 0, 0, 0]);

    const ix = new web3.TransactionInstruction({
      programId,
      keys,
      data,
    });
    return ix;
  }

  function createWhitelistValidatorForProgramInstruction(
    authority: web3.PublicKey,
    validatorIdentity: web3.PublicKey,
    program: web3.PublicKey,
    insert: boolean,
    programId = new web3.PublicKey(DELEGATION_PROGRAM_ID)
  ) {
    const programData = web3.PublicKey.findProgramAddressSync(
      [program.toBuffer()],
      new web3.PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
    )[0];

    const programConfig = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("p-conf"), program.toBuffer()],
      programId
    )[0];

    const keys = [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: validatorIdentity, isSigner: false, isWritable: false },
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
      programId,
      keys,
      data,
    });
    return ix;
  }
});
