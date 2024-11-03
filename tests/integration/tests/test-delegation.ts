import * as anchor from "@coral-xyz/anchor";
import {Program, web3} from "@coral-xyz/anchor";
import * as beet from "@metaplex-foundation/beet";
import { TestDelegation } from "../target/types/test_delegation";
import {
    createDelegateInstruction, createUndelegateInstruction,
    DelegateAccounts,
    DELEGATION_PROGRAM_ID
} from "@magicblock-labs/ephemeral-rollups-sdk";
import {ON_CURVE_ACCOUNT} from "./fixtures/consts";

const SEED_TEST_PDA = "test-pda";

const delegationProgramIdlPath = "./idls/delegation.json";

describe("TestDelegation", () => {
    // Configure the client to use the local cluster.
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const testDelegation = anchor.workspace.TestDelegation as Program<TestDelegation>;
    const idl = JSON.parse(require('fs').readFileSync(delegationProgramIdlPath, 'utf8'));
    const dlpProgram = new Program(idl, provider);

    const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_TEST_PDA)],
        testDelegation.programId
    );

    it("Initialize validator fee vault", async () => {
            const ix = createInitializeValidatorFeesVaultInstruction(
                provider.wallet.publicKey,
                provider.wallet.publicKey,
                provider.wallet.publicKey
            );
            const tx = new web3.Transaction().add(ix);
            tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
            tx.feePayer = provider.wallet.publicKey;
            const txId = await provider.sendAndConfirm(tx);
            console.log("Initialize validator fee vault tx:", txId);
        });


    it('Initializes the counter', async () => {
        // Check if the counter is initialized
        const counterAccountInfo = await provider.connection.getAccountInfo(pda);
        if(counterAccountInfo === null) {
            const tx = await testDelegation.methods
                .initialize()
                .accounts({
                    // @ts-ignore
                    counter: pda,
                    user: provider.wallet.publicKey,
                    systemProgram: anchor.web3.SystemProgram.programId,
                }).rpc({skipPreflight: true});
            console.log('Init Pda Tx: ', tx);
        }

        const counterAccount = await testDelegation.account.counter.fetch(pda);
        console.log('Counter: ', counterAccount.count.toString());
    });

    it('Increase the counter', async () => {
        const tx = await testDelegation.methods
            .increment()
            .accounts({
                counter: pda,
            }).rpc({skipPreflight: true});
        console.log('Increment Tx: ', tx);

        const counterAccount = await testDelegation.account.counter.fetch(pda);
        console.log('Counter: ', counterAccount.count.toString());
    });



    it("Delegate a PDA", async () => {

        const { delegationRecord, delegationMetadata, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        // Delegate, Close PDA, and Lock PDA in a single instruction
        const tx = await testDelegation.methods
            .delegate()
            .accounts({
                payer: provider.wallet.publicKey,
                pda: pda,
                ownerProgram: testDelegation.programId,
                delegationMetadata: delegationMetadata,
                buffer: bufferPda,
                delegationRecord: delegationRecord,
                delegationProgram: DELEGATION_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Your transaction signature", tx);

        // Print delegationPda account bytes
        // let account = await provider.connection.getAccountInfo(delegationPda);
        // console.log("Delegation record PDA", account.data.toJSON());

        // Print delegateAccountMetadata account bytes
        // let account = await provider.connection.getAccountInfo(delegationMetadata);
        // console.log("Delegation account metadata", account.data.toJSON());
        // console.log("Delegation account metadata PDA: ", delegationMetadata.toBase58());
    });

    it("Delegate an on-curve account", async () => {
        const delegateOnCurve = ON_CURVE_ACCOUNT;
        const { delegationRecord, delegationMetadata} = DelegateAccounts(delegateOnCurve.publicKey, web3.SystemProgram.programId);

        // Airdrop SOL to create the account
        const airdropSignature = await provider.connection.requestAirdrop(
            delegateOnCurve.publicKey,
            web3.LAMPORTS_PER_SOL
        );
        await provider.connection.confirmTransaction(airdropSignature);

        let tx = new web3.Transaction()
            .add(web3.SystemProgram.assign({
                accountPubkey: delegateOnCurve.publicKey,
                programId: new web3.PublicKey(DELEGATION_PROGRAM_ID)
            }))
            .add(
                createDelegateInstruction({
                    payer: provider.wallet.publicKey,
                    delegateAccount: delegateOnCurve.publicKey,
                    ownerProgram: web3.SystemProgram.programId,
                }
            )
        );
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = provider.wallet.publicKey;
        tx.partialSign(delegateOnCurve);
        tx = await provider.wallet.signTransaction(tx);
        const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});

        console.log("Your transaction signature", txSign);

        // Print delegationPda account bytes
        // let account = await provider.connection.getAccountInfo(delegationRecord);
        // console.log("Delegation record", account.data.toJSON());
        // console.log("Delegation record PDA", delegationRecord.toBase58());

        // Print delegateAccountMetadata account bytes
        // const account = await provider.connection.getAccountInfo(delegationMetadata);
        // console.log("Delegation account metadata", account.data.toJSON());
        // console.log("Delegation account metadata PDA: ", delegationMetadata.toBase58());
    });

    it("Commit a new state to the PDA", async () => {
        const { delegationRecord, delegationMetadata, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        let account = await provider.connection.getAccountInfo(pda);
        let new_data = account.data;
        new_data[-1] = (new_data[-1] + 1) % 256

        const args: CommitAccountInstructionArgs = {
            slot: new anchor.BN(40),
            lamports: new anchor.BN(1000000000),
            allow_undelegation: false,
            data: new_data
        }

        const ix = createCommitAccountInstruction({
            authority: provider.wallet.publicKey,
            delegatedAccount: pda,
            commitStatePda: commitStatePda,
            commitStateRecordPda: commitStateRecordPda,
            delegationRecordPda: delegationRecord,
            delegationMetadataPda: delegationMetadata,
        }, args);

        let tx = new anchor.web3.Transaction().add(ix);
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = provider.wallet.publicKey;
        tx = await provider.wallet.signTransaction(tx);
        const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});

        console.log("Commit state signature", txSign);

        // Print commit state record bytes
        // account = await provider.connection.getAccountInfo(commitStateRecordPda);
        // console.log("Committed state record PDA", account.data.toJSON());
    });

    it("Finalize account state", async () => {

        const { delegationRecord, delegationMetadata, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        // @ts-ignore
        const tx = await dlpProgram.methods
            .finalize()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                committedStateAccount: commitStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationRecord,
                delegationMetadata: delegationMetadata,
                reimbursement: provider.wallet.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Finalize signature", tx);
    });

    it("Commit a new state to the PDA", async () => {
        const { delegationRecord, delegationMetadata, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        let account = await provider.connection.getAccountInfo(pda);
        let new_data = account.data;
        new_data[-1] = (new_data[-1] + 1) % 256

        const args: CommitAccountInstructionArgs = {
            slot: new anchor.BN(40),
            lamports: new anchor.BN(1000000000),
            allow_undelegation: true,
            data: new_data
        }

        const ix = createCommitAccountInstruction({
            authority: provider.wallet.publicKey,
            delegatedAccount: pda,
            commitStatePda: commitStatePda,
            commitStateRecordPda: commitStateRecordPda,
            delegationRecordPda: delegationRecord,
            delegationMetadataPda: delegationMetadata,
        }, args);

        let tx = new anchor.web3.Transaction().add(ix);
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = provider.wallet.publicKey;
        tx = await provider.wallet.signTransaction(tx);
        const txSign = await provider.sendAndConfirm(tx);

        console.log("Commit state signature", txSign);
    });

    it("Allow Undelegation", async () => {
        const { delegationRecord, delegationMetadata, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);
        const txSign = await testDelegation.methods
            .allowUndelegation()
            .accounts({
                delegationRecord: delegationRecord,
                delegationMetadata: delegationMetadata,
                buffer: bufferPda,
                delegationProgram: DELEGATION_PROGRAM_ID,
            }).rpc({skipPreflight: true});
        console.log("Allow Undelegation signature", txSign);

        // Print delegateAccountMetadata account bytes
        // const account = await provider.connection.getAccountInfo(delegationMetadata);
        // console.log("Delegation account metadata", account.data.toJSON());
    });

    it("Undelegate account", async () => {
        const ixUndelegate = createUndelegateInstruction({
            payer: provider.wallet.publicKey,
            delegatedAccount: pda,
            ownerProgram: testDelegation.programId,
            reimbursement: provider.wallet.publicKey,
        });

        const tx = new anchor.web3.Transaction().add(ixUndelegate);
        const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});

        console.log("Undelegate signature", txSign);
    });

    /// Instruction to commit a new state to the PDA

    interface CommitStateAccounts {
        authority: web3.PublicKey;
        delegatedAccount: web3.PublicKey;
        commitStatePda: web3.PublicKey;
        commitStateRecordPda: web3.PublicKey;
        delegationRecordPda: web3.PublicKey;
        delegationMetadataPda: web3.PublicKey;
    }

    interface CommitAccountInstructionArgs {
        slot: beet.bignum,
        lamports: beet.bignum,
        allow_undelegation: boolean,
        data: Uint8Array,
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
            [Buffer.from("v-fees-vault"),  accounts.authority.toBuffer()],
            new anchor.web3.PublicKey(DELEGATION_PROGRAM_ID)
        )[0];
        const keys = [
            { pubkey: accounts.authority, isSigner: true, isWritable: false },
            { pubkey: accounts.delegatedAccount, isSigner: false, isWritable: false },
            { pubkey: accounts.commitStatePda, isSigner: false, isWritable: true },
            { pubkey: accounts.commitStateRecordPda, isSigner: false, isWritable: true },
            { pubkey: accounts.delegationRecordPda, isSigner: false, isWritable: true },
            { pubkey: accounts.delegationMetadataPda, isSigner: false, isWritable: true },
            { pubkey: validatorFeesVaultPda, isSigner: false, isWritable: true },
            { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: false },
        ];

        const ix = new web3.TransactionInstruction({
            programId,
            keys,
            data,
        });
        return ix;
    }

    /// Instruction to initialize a fees vault for a validator authority
    function createInitializeValidatorFeesVaultInstruction(
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
            { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: false },
        ];

        const data = Buffer.from([7, 0, 0, 0, 0, 0, 0, 0]);

        const ix = new web3.TransactionInstruction({
            programId,
            keys,
            data,
        });
        return ix;
    }

});
