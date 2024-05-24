import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TestDelegation } from "../target/types/test_delegation";

// Define constants seeds for PDAs
const SEED_TEST_PDA = "test-pda";
const SEED_BUFFER_PDA = "buffer";
const SEED_DELEGATION_PDA = "delegation";
const DELEGATED_ACCOUNT_SEEDS = "account-seeds";
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

        var {pda, delegationPda, delegatedAccountSeedsPda, ...newStatePda} = getAccounts(testDelegation);

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
                delegateAccountSeeds: delegatedAccountSeedsPda,
                buffer: bufferPda,
                delegationRecord: delegationPda,
                delegationProgram: delegationProgram,
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

        const {pda, delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(5))
            .accounts({
                authority: provider.wallet.publicKey,
                delegatedAccount: pda,
                commitStateAccount: newStatePda,
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

        const {pda, delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .finalize()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                committedStateAccount: newStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                reimbursement: provider.wallet.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});
        console.log("Finalize signature", tx);
    });

    it("Commit a new state to the PDA", async () => {

        const {pda, delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .commitState(Buffer.alloc(15).fill(7))
            .accounts({
                authority: provider.wallet.publicKey,
                delegatedAccount: pda,
                commitStateAccount: newStatePda,
                commitStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                systemProgram: anchor.web3.SystemProgram.programId,
            }).rpc({skipPreflight: true});

        console.log("Commit state signature", tx);
    });

    it("Undelegate account", async () => {

        const {pda, delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, newStatePda} = getAccounts(testDelegation);

        var tx = await dlpProgram.methods
            .undelegate()
            .accounts({
                payer: provider.wallet.publicKey,
                delegatedAccount: pda,
                ownerProgram: testDelegation.programId,
                buffer: bufferPda,
                committedStateAccount: newStatePda,
                committedStateRecord: commitStateRecordPda,
                delegationRecord: delegationPda,
                delegatedAccountSeeds: delegatedAccountSeedsPda,
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

    const [delegatedAccountSeedsPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(DELEGATED_ACCOUNT_SEEDS), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [bufferPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_BUFFER_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [commitStateRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_COMMIT_STATE_RECORD_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );

    const [newStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(SEED_STATE_DIFF_PDA), pdaBytes],
        new anchor.web3.PublicKey(delegationProgram)
    );
    return {pda, delegationPda, delegatedAccountSeedsPda, bufferPda, commitStateRecordPda, newStatePda};
}
