import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TestDelegation } from "../target/types/test_delegation";

// Define constants seeds for PDAs
const SEED_TEST_PDA = "test-pda";
const SEED_BUFFER_PDA = "buffer";
const SEED_DELEGATION_PDA = "delegation";
const SEED_COMMIT_STATE_RECORD_PDA = "commit-state-record";
const SEED_STATE_DIFF_PDA = "state-diff";

const delegationProgram = new anchor.web3.PublicKey("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");
const delegationProgramIdlPath = "./idls/delegation.json";

describe("TestDelegation", () => {
    // Configure the client to use the local cluster.
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const testDelegation = anchor.workspace.TestDelegation as Program<TestDelegation>;
    const idl = JSON.parse(require('fs').readFileSync(delegationProgramIdlPath, 'utf8'));
    const dlpProgram = new Program(idl, provider);

    it("Delegate a PDA", async () => {

        var {pda, delegationPda, ...newStatePda} = getAccounts(testDelegation);

        const pdaBytes = pda.toBytes();

        // Find program-derived address (PDA) for buffer
        var [bufferPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from(SEED_BUFFER_PDA), pdaBytes],
            testDelegation.programId
        );

        // Delegate, Close PDA, and Lock PDA in a single instruction
        const tx = await testDelegation.methods
            .delegate()
            .accounts({
                payer: provider.wallet.publicKey,
                pda: pda,
                ownerProgram: testDelegation.programId,
                buffer: bufferPda,
                delegationRecord: delegationPda,
                delegationProgram: delegationProgram,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Your transaction signature", tx);

        // Print delegationPda account bytes
        // const account = await provider.connection.getAccountInfo(delegationPda);
        // console.log("Delegation record PDA", account.data.toJSON());
    });

    it("Commit a new state to the PDA", async () => {

        const {pda, delegationPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(5))
            .accounts({
                authority: provider.wallet.publicKey,
                originAccount: pda,
                newStatePda: newStatePda,
                commitStateRecordPda: commitStateRecordPda,
                delegationPda: delegationPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});

        console.log("Commit state signature", tx);
    });

    it("Finalize account state", async () => {

        const {pda, delegationPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .finalize()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                stateDiff: newStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                reimbursement: provider.wallet.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Finalize signature", tx);
    });

    it("Commit a new state to the PDA", async () => {

        const {pda, delegationPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(7))
            .accounts({
                authority: provider.wallet.publicKey,
                originAccount: pda,
                newStatePda: newStatePda,
                commitStateRecordPda: commitStateRecordPda,
                delegationPda: delegationPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});

        console.log("Commit state signature", tx);
    });

    it("Undelegate account", async () => {

        const {pda, delegationPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .undelegate()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                ownerProgram: testDelegation.programId,
                buffer: bufferPda,
                stateDiff: newStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                reimbursement: provider.wallet.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Undelegate signature", tx);
    });

});

function getAccounts(testDelegation: Program<TestDelegation>) {
    const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_TEST_PDA)],
        testDelegation.programId
    );
    const pdaBytes = pda.toBytes();

    const [delegationPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_DELEGATION_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [bufferPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_BUFFER_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [commitStateRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_COMMIT_STATE_RECORD_PDA), new Uint8Array(8).fill(0), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [newStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_STATE_DIFF_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );
    return {pda, delegationPda, bufferPda, commitStateRecordPda, newStatePda};
}
