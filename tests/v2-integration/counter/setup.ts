import { execSync, spawn } from "child_process";
import path from "path";

import {
  PAYER_FILE,
  COUNTER_PROGRAM_ID,
  DLP_PROGRAM_ID,
  PAYER,
  wait,
} from "./utils";

export const COUNTER_SO = path.resolve("./target/deploy/counter.so");
export const DLP_SO = path.resolve("../../../target/deploy/dlp.so");

let validatorProcess: ReturnType<typeof spawn> | null = null;

async function waitForRPC() {
  const maxRetries = 40; //40;
  for (let i = 0; i < maxRetries; i++) {
    try {
      console.log(`check: solana --url ${process.env.RPC_URL} get-slot`);

      execSync(`solana --url ${process.env.RPC_URL} get-slot`, {
        stdio: "overlapped",
      });
      return;
    } catch (e) {
      console.log("waitForRPC failed with: ", e);
      await wait(500);
    }
  }
  throw new Error("RPC did not come online in time");
}

beforeEach(async () => {
  console.log("🔥 Starting solana-test-validator...");

  const FAUCET_PORT = 9900 + Math.floor(Math.random() * 1000);
  const RPC_PORT = 8899 + Math.floor(Math.random() * 1000);

  console.log("Use random ports: ", { RPC_PORT, FAUCET_PORT });

  process.env.RPC_PORT = `${RPC_PORT}`;
  process.env.FAUCET_PORT = `${FAUCET_PORT}`;
  process.env.RPC_URL = `http://127.0.0.1:${RPC_PORT}`;

  if (process.env.RPC_URL) {
    return;
  }

  try {
    validatorProcess = spawn(
      "solana-test-validator",
      [
        "--reset",

        "--rpc-port",
        process.env.RPC_PORT,
        "--faucet-port",
        process.env.FAUCET_PORT,

        // "--quiet", // less logs

        // "--upgradeable-program",
        // DLP_PROGRAM_ID.toBase58(),
        // DLP_SO,
        // PAYER.publicKey.toBase58(),

        // "--upgradeable-program",
        // COUNTER_PROGRAM_ID.toBase58(),
        // COUNTER_SO,
        // PAYER.publicKey.toBase58(),
      ],
      {
        stdio: "inherit",
        env: process.env,
      }
    );
  } catch (e) {
    console.log("starting solana-test-validator failed with: ", e);
  }

  console.log("⏳ Waiting for RPC...");
  await waitForRPC();
  console.log("✅ RPC online");

  try {
    execSync(
      `solana airdrop --url ${process.env.RPC_URL} --keypair ${PAYER_FILE} 100`
    );
    execSync(
      `solana program --url ${process.env.RPC_URL} show ${DLP_PROGRAM_ID}`
    );
    execSync(
      `solana program --url ${process.env.RPC_URL} show ${COUNTER_PROGRAM_ID}`
    );
    console.log("✅ Program loaded into validator");
  } catch {
    throw new Error("❌ Program failed to load");
  }
});

afterEach(async () => {
  console.log("🧹 Shutting down validator...");

  if (validatorProcess) {
    validatorProcess.kill("SIGINT");
  }

  await wait(3000); // 3 seconds
});
