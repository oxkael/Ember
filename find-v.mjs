import { ethers } from "ethers";

const hash = "0x8e3ef335eb5c4d774b1cc7fdf25c8c6c9915dd1fda84fd03d0b4ece5c58df475";
const r = "0x105e2f4a0ddcd57d4f384b30a7b581f061643ea1d3780d65a221e0bff146d123";
const s = "0x474948668c93ee798ee6209ee79add84c8e357e37a23d8421799837b0e8b0a44";
const target = "0xDAe49575b025e1f21b3238605fE1887c97da210a".toLowerCase();

const secp256k1_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141n;
const sBig = BigInt(s);
const sInv = "0x" + (secp256k1_N - sBig).toString(16).padStart(64, "0");

let match = null;

for (const currS of [s, sInv]) {
  for (const v of [27, 28]) {
    try {
      const recovered = ethers.recoverAddress(hash, { r, s: currS, v });
      console.log(`v=${v}, s=${currS.slice(0, 10)}... -> ${recovered}`);
      if (recovered.toLowerCase() === target) {
        console.log(`\n🎉 MATCH FOUND! v=${v}, s=${currS}`);
        match = { v, s: currS };
      }
    } catch (e) {
      console.log("err:", e.message);
    }
  }
}

if (!match) {
  console.log("No standard match directly against hash. Checking other hashes or signer address derivation.");
}
