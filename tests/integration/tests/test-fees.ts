import * as anchor from "@coral-xyz/anchor";
import {Program, web3} from "@coral-xyz/anchor";
import * as beet from "@metaplex-foundation/beet";

const FEES_VAULT = "fees-vault";

describe("TestFees", () => {
    const connection = new anchor.web3.Connection(anchor.web3.clusterApiUrl('devnet'));

    const provider = new anchor.AnchorProvider(
        connection,
        anchor.Wallet.local(),
        { preflightCommitment: 'confirmed' }
    );
    anchor.setProvider(provider);

    const delegationProgramIdlPath = "./idls/delegation.json";
    const idl = JSON.parse(require('fs').readFileSync(delegationProgramIdlPath, 'utf8'));
    const dlpProgram = new Program(idl, provider);

    const [feesVaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from(FEES_VAULT)],
        dlpProgram.programId
    );

    it.only('Initializes the fees vault', async () => {

        // Check if the fee vault is initialized
        let feesVaultAccountInfo = await provider.connection.getAccountInfo(feesVaultPda);

        if(feesVaultAccountInfo === null) {
            const ix = createInitializeFeesVaultInstruction(provider.wallet.publicKey);
            let tx = new anchor.web3.Transaction().add(ix);
            tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
            tx.lastValidBlockHeight = (await provider.connection.getLatestBlockhash()).lastValidBlockHeight;
            tx.feePayer = provider.wallet.publicKey;
            tx = await provider.wallet.signTransaction(tx);
            console.log(tx.serializeMessage().toString("base64"));
            const txSign = await provider.sendAndConfirm(tx, [], {skipPreflight: true});
            console.log('Init Fees Vault Tx: ', txSign);
        }

        feesVaultAccountInfo = await provider.connection.getAccountInfo(feesVaultPda);
        console.log('Fees Vault Data: ', feesVaultAccountInfo.data);
    });

    const initializeFeesVaultStruct = new beet.FixableBeetArgsStruct<
    {
        instructionDiscriminator: number[] /* size: 8 */;
    }
    >(
        [
            ["instructionDiscriminator", beet.uniformFixedSizeArray(beet.u8, 8)],
        ],
        "InitializeFeesVaultInstructionArgs"
    );

    function createInitializeFeesVaultInstruction(
        payer: anchor.web3.PublicKey,
        programId = dlpProgram.programId
    ) {
        const [data] = initializeFeesVaultStruct.serialize({
            instructionDiscriminator: [6, 0, 0, 0, 0, 0, 0, 0],
        });
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

});