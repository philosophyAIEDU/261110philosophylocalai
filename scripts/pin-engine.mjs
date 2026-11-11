#!/usr/bin/env node
// 공식 Ollama 릴리스의 sha256sum.txt를 받아 engine-manifest.json의 sha256 값을 채운다.
// 사용: node scripts/pin-engine.mjs [버전]   (버전 생략 시 매니페스트의 현재 버전)
// 매니페스트의 자산 중 하나라도 체크섬 파일에 없으면 실패한다 (fail-closed).
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(root, "crates/core/src/runtime/engine-manifest.json");
const m = JSON.parse(readFileSync(manifestPath, "utf8"));
const version = (process.argv[2] ?? m.version).replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`잘못된 버전: ${version}`);

const base = `https://github.com/ollama/ollama/releases/download/v${version}/`;
const res = await fetch(base + "sha256sum.txt", { redirect: "follow" });
if (!res.ok) throw new Error(`sha256sum.txt 내려받기 실패: HTTP ${res.status}`);
const sums = new Map();
for (const line of (await res.text()).split("\n")) {
  const mm = line.trim().match(/^([0-9a-f]{64})\s+\*?\.?\/?(\S+)$/);
  if (mm) sums.set(mm[2], mm[1]);
}

m.version = version;
m.source = `https://github.com/ollama/ollama/releases/tag/v${version}`;
m.base_url = base;
const missing = [];
for (const [key, asset] of Object.entries(m.assets)) {
  const h = sums.get(asset.file);
  if (!h) missing.push(`${key} (${asset.file})`);
  asset.sha256 = h ?? null;
}
if (missing.length) {
  writeFileSync(manifestPath, JSON.stringify(m, null, 2) + "\n");
  console.error(`체크섬이 없는 자산: ${missing.join(", ")}`);
  process.exit(1);
}
writeFileSync(manifestPath, JSON.stringify(m, null, 2) + "\n");
console.log(`v${version}: ${Object.keys(m.assets).length}개 자산의 SHA-256을 고정했습니다.`);
