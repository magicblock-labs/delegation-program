import { web3 } from "@coral-xyz/anchor";

export const ON_CURVE_ACCOUNT = web3.Keypair.fromSecretKey(
  Uint8Array.from([
    74, 198, 48, 104, 119, 57, 255, 80, 67, 181, 191, 189, 85, 21, 235, 45, 185,
    175, 48, 143, 13, 202, 92, 81, 211, 108, 61, 237, 183, 116, 207, 45, 170,
    118, 238, 247, 128, 91, 3, 41, 33, 10, 241, 163, 185, 198, 228, 172, 200,
    220, 225, 192, 149, 94, 106, 209, 65, 79, 210, 54, 191, 49, 115, 159,
  ])
); // CURVek2Zcmv5HUt34CVDnWMeSLAJfrXSrU2mYBuiZvvS
