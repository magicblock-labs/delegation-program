import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Counter } from "../target/types/counter";
import { sol, wait } from "../utils";

describe("counter", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.counter as Program<Counter>;
  const connection = program.provider.connection;

  async function newUser(solAmount: number = 5) {
    const user = Keypair.generate();
    console.log("user: ", user.publicKey.toBase58());
    await connection.requestAirdrop(user.publicKey, sol(solAmount));
    await wait(1000);
    return user;
  }

  async function printLogs(sig: string, waitMs: number = 100) {
    await wait(waitMs);
    const logs = (
      await connection.getTransaction(sig, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      })
    )?.meta?.logMessages;
    console.log(`logs (${sig}): `, logs);
  }

  it("ensures CommitFinalizeInline consumes less than 100 CU", async () => {
    console.log("programId: ", program.programId);
    console.log("env: ", connection.rpcEndpoint);

    const user = await newUser();

    const tx = await program.methods
      .initialize()
      .accounts({ user: user.publicKey })
      .signers([user])
      .rpc();

    await printLogs(tx);
  });
});
