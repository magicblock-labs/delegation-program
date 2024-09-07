import * as anchor from "@coral-xyz/anchor";
import {Program, web3} from "@coral-xyz/anchor";

const FEES_VAULT = "fees-vault";
const EPHEMERAL_BALANCE = "ephemeral-balance";

describe("TestFees", () => {

    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const delegationProgramIdlPath = "./idls/delegation.json";
    const idl = JSON.parse(require('fs').readFileSync(delegationProgramIdlPath, 'utf8'));
    const dlpProgram = new Program(idl, provider);

    const [feesVaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(FEES_VAULT)],
        dlpProgram.programId
    );

    it('Initializes the fees vault', async () => {

        // Check if the fee vault is initialized
        let feesVaultAccountInfo = await provider.connection.getAccountInfo(feesVaultPda);

        if(feesVaultAccountInfo === null) {
            const ix = createInitializeFeesVaultInstruction(provider.wallet.publicKey);
            let tx = new anchor.web3.Transaction().add(ix);
            tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
            tx.lastValidBlockHeight = (await provider.connection.getLatestBlockhash()).lastValidBlockHeight;
            tx.feePayer = provider.wallet.publicKey;
            tx = await provider.wallet.signTransaction(tx);
            const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});
            console.log('Init Fees Vault Tx: ', txSign);
        }
    });

    it('TopUp ephemeral balance', async () => {
        const ix = createTopUpInstruction(provider.wallet.publicKey, 1000000);
        let tx = new anchor.web3.Transaction().add(ix);
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.lastValidBlockHeight = (await provider.connection.getLatestBlockhash()).lastValidBlockHeight;
        tx.feePayer = provider.wallet.publicKey;
        tx = await provider.wallet.signTransaction(tx);
        const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});
        console.log('TopUp Tx: ', txSign);

        // Check if the ephemeral balance is updated
        const [ephemeralBalancePda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from(EPHEMERAL_BALANCE), provider.wallet.publicKey.toBuffer()],
            dlpProgram.programId
        );
        let ephemeralBalanceAccountInfo = await provider.connection.getAccountInfo(ephemeralBalancePda);
        console.log('Ephemeral Balance Account Info: ', ephemeralBalanceAccountInfo.data.toJSON());
        console.log('Ephemeral Balance Account Info: ', ephemeralBalanceAccountInfo.data.toJSON());
    });

    // Transaction building functions

    function createInitializeFeesVaultInstruction(
        payer: anchor.web3.PublicKey,
        programId = dlpProgram.programId
    ) {
        const data = Buffer.from([6, 0, 0, 0, 0, 0, 0, 0]);
        const keys = [
            { pubkey: payer, isSigner: true, isWritable: true },
            { pubkey: feesVaultPda, isSigner: false, isWritable: true },
            { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: false },
        ];

        const ix = new web3.TransactionInstruction({
            programId,
            keys,
            data,
        });
        return ix;
    }

    function createTopUpInstruction(
        payer: anchor.web3.PublicKey,
        amount: number,
        programId = dlpProgram.programId
    ) {
        const data = Buffer.from([5, 0, 0, 0, 0, 0, 0, 0].concat(new anchor.BN(amount).toArray("le", 8)));
        const [ephemeralBalancePda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from(EPHEMERAL_BALANCE), payer.toBuffer()],
            dlpProgram.programId
        );
        const keys = [
            { pubkey: payer, isSigner: true, isWritable: true },
            { pubkey: ephemeralBalancePda, isSigner: false, isWritable: true },
            { pubkey: feesVaultPda, isSigner: false, isWritable: true },
            { pubkey: web3.SystemProgram.programId, isSigner: false, isWritable: false },
        ];

        const ix = new web3.TransactionInstruction({
            programId,
            keys,
            data,
        });
        return ix;
    }

});