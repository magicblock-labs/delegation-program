import { readFileSync } from "fs";
import { Keypair, PublicKey } from "@solana/web3.js";
import path from "path";
import util from "node:util";

export function keypairFromFile(path: string): Keypair {
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(readFileSync(path, "utf-8")))
  );
}

export const PAYER_FILE = path.resolve("./secrets/admin.json");

export const PAYER = keypairFromFile(PAYER_FILE);

export const DLP_PROGRAM_ID = new PublicKey(
  "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh"
);

export const COUNTER_PROGRAM_ID = new PublicKey(
  "8Aw8uKuJL2Yhr7nNCYjKAtKAajyoRicCbipR1kT3qEmW"
);

// every value is in seconds
export const MINUTE = 60;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;

export function wait(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function sol(amount: number) {
  return amount * 10 ** 9;
}

// returns epoch seconds.
export function now_since_epoch(): bigint {
  return BigInt(Math.floor(Date.now() / 1000));
}
