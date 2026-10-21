import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TestDelegation } from "../target/types/test_delegation";
import { assert } from "chai";

describe("TestDelegation", () => {
  const connection = new anchor.web3.Connection(
    anchor.web3.clusterApiUrl("devnet")
  );

  const provider = new anchor.AnchorProvider(
    connection,
    anchor.Wallet.local(),
    { preflightCommitment: "processed" }
  );
  anchor.setProvider(provider);

  const testDelegation = anchor.workspace
    .TestDelegation as Program<TestDelegation>;

  it("Increase the counter after undelegation", async () => {
    const counter = new anchor.web3.PublicKey(
      "C89kNYAztTjg3qiPeztai2Ua9ucES41MLTuQS44rYqTP"
    );
    let failed = false;
    try {
      const tx = await testDelegation.methods
        .increment()
        .accounts({
          counter: counter,
        })
        .rpc();
      console.log("Increment Tx: ", tx);
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed);
  });
});
