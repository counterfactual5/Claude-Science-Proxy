#!/usr/bin/env node
// 测试预言机：读一个 auth_dir 里的 encryption.key + 唯一 .enc，用与 make-virtual-oauth.mjs
// 完全相同的 v2 GCM 解密逻辑解开，打印令牌 blob（JSON）。仅供 node↔rust 双向对拍用，
// 证明 Rust 版 oauth_forge 产出的 .enc 能被 Node 的解密逻辑读回（= 与 Science 侧一致）。
//
// 用法：node test/decrypt-oauth.mjs --auth-dir <目录>
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

function arg(name, def = undefined) {
  const i = process.argv.indexOf(name);
  return i > -1 && i + 1 < process.argv.length ? process.argv[i + 1] : def;
}
const authDir = arg("--auth-dir");
if (!authDir) {
  console.error("必须指定 --auth-dir <目录>");
  process.exit(2);
}

// encryption.key（换行分隔 KEY=VALUE）里取 OAUTH_ENCRYPTION_KEY。
const keyTxt = fs.readFileSync(path.join(authDir, "encryption.key"), "utf-8");
let oauthKey = null;
for (const line of keyTxt.split("\n")) {
  const eq = line.indexOf("=");
  if (eq <= 0) continue;
  if (line.slice(0, eq).trim() === "OAUTH_ENCRYPTION_KEY") oauthKey = line.slice(eq + 1).trim();
}
if (!oauthKey) {
  console.error("encryption.key 里没有 OAUTH_ENCRYPTION_KEY");
  process.exit(3);
}

// .oauth-tokens/ 下唯一的 .enc。
const tokDir = path.join(authDir, ".oauth-tokens");
const encs = fs.readdirSync(tokDir).filter((f) => f.endsWith(".enc"));
if (encs.length !== 1) {
  console.error(`应恰好一个 .enc，实为 ${encs.length}`);
  process.exit(4);
}
const body = fs.readFileSync(path.join(tokDir, encs[0]), "utf-8");

// v2 解密（与 make-virtual-oauth.mjs decryptTokenV2 逐字一致）。
function decryptTokenV2(b, oauthKeyB64) {
  const ikm = Buffer.from(oauthKeyB64, "base64");
  const derived = Buffer.from(
    crypto.hkdfSync("sha256", ikm, Buffer.alloc(0), "operon:aes-256-gcm:oauth", 32)
  );
  const raw = Buffer.from(b.slice("v2:".length), "base64");
  const iv = raw.subarray(0, 12);
  const tag = raw.subarray(raw.length - 16);
  const ct = raw.subarray(12, raw.length - 16);
  const d = crypto.createDecipheriv("aes-256-gcm", derived, iv, { authTagLength: 16 });
  d.setAAD(Buffer.from("v2:oauth"));
  d.setAuthTag(tag);
  return Buffer.concat([d.update(ct), d.final()]).toString("utf-8");
}

process.stdout.write(decryptTokenV2(body, oauthKey));
