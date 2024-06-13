import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TestDelegation } from "../target/types/test_delegation";
import {
    createUndelegateInstruction,
    DelegateAccounts, DELEGATION_PROGRAM_ID, UndelegateAccounts
} from "delegation-program";

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

        const { delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        // Delegate, Close PDA, and Lock PDA in a single instruction
        const tx = await testDelegation.methods
            .delegate()
            .accounts({
                payer: provider.wallet.publicKey,
                pda: pda,
                ownerProgram: testDelegation.programId,
                delegateAccountSeeds: delegatedAccountSeedsPda,
                buffer: bufferPda,
                delegationRecord: delegationPda,
                delegationProgram: DELEGATION_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Your transaction signature", tx);

        // Print delegationPda account bytes
        // const account = await provider.connection.getAccountInfo(delegationPda);
        // console.log("Delegation record PDA", account.data.toJSON());

        // Print delegateAccountSeeds account bytes
        // const account = await provider.connection.getAccountInfo(delegatedAccountSeedsPda);
        // console.log("Delegation account seeds PDA", account.data.toJSON());
        // console.log("Delegation account seeds PDA: ", delegatedAccountSeedsPda.toBase58());
    });

    it("Commit a new state to the PDA", async () => {

        const { delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        // @ts-ignore
        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(5))
            .accounts({
                authority: provider.wallet.publicKey,
                delegatedAccount: pda,
                commitStateAccount: commitStatePda,
                commitStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});

        console.log("Commit state signature", tx);

        // Print commit state record bytes
        // const account = await provider.connection.getAccountInfo(commitStateRecordPda);
        // console.log("Committed state record PDA", account.data.toJSON());
    });

    it("Finalize account state", async () => {

        const { delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        var tx = await dlpProgram.methods
            .finalize()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                committedStateAccount: commitStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                reimbursement: provider.wallet.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Finalize signature", tx);
    });

    it("Commit a new state to the PDA", async () => {

        const { delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, commitStatePda} = DelegateAccounts(pda, testDelegation.programId);

        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(7))
            .accounts({
                authority: provider.wallet.publicKey,
                delegatedAccount: pda,
                commitStateAccount: commitStatePda,
                commitStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});

        console.log("Commit state signature", tx);
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

});
