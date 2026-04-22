#!/usr/bin/env node

/**
 * postinstall: download the platform-appropriate `opm` binary
 * from the latest GitHub Release with SHA256 checksum verification.
 */

const crypto = require("crypto");
const https = require("https");
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const REPO = "ayaka209/opencode-provider-manager";
const VERSION = require("./package.json").version;
const BIN_DIR = path.join(__dirname, "bin");

// ── Platform mapping ────────────────────────────────────────────────
function getTarget() {
  const platform = process.platform; // win32 | darwin | linux
  const arch = process.arch; // x64 | arm64

  const map = {
    "win32-x64": "opm-x86_64-pc-windows-msvc.exe",
    "win32-arm64": "opm-aarch64-pc-windows-msvc.exe",
    "darwin-x64": "opm-x86_64-apple-darwin",
    "darwin-arm64": "opm-aarch64-apple-darwin",
    "linux-x64": "opm-x86_64-unknown-linux-gnu",
    "linux-arm64": "opm-aarch64-unknown-linux-gnu",
  };

  const key = `${platform}-${arch}`;
  const filename = map[key];
  if (!filename) {
    throw new Error(`Unsupported platform: ${key}`);
  }
  return { filename, platform };
}

// ── HTTP GET with redirect following ────────────────────────────────
function httpGet(url, opts = {}) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith("https") ? https : require("http");
    mod.get(url, opts, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return httpGet(res.headers.location, opts).then(resolve, reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      resolve(res);
    }).on("error", reject);
  });
}

// ── Download to buffer ──────────────────────────────────────────────
async function downloadToBuffer(url) {
  const res = await httpGet(url);
  const chunks = [];
  return new Promise((resolve, reject) => {
    res.on("data", (chunk) => chunks.push(chunk));
    res.on("end", () => resolve(Buffer.concat(chunks)));
    res.on("error", reject);
  });
}

// ── Fetch and parse checksums.txt ───────────────────────────────────
async function fetchChecksums() {
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/checksums.txt`;
  const buf = await downloadToBuffer(url);
  const text = buf.toString("utf-8");
  // Format: "<hash>  <filename>" per line (sha256sum output)
  const map = new Map();
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const [hash, filename] = trimmed.split(/\s+/);
    if (hash && filename) {
      map.set(filename, hash);
    }
  }
  return map;
}

// ── Main ────────────────────────────────────────────────────────────
async function main() {
  const { filename, platform: _pf } = getTarget();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${filename}`;
  const localBin = path.join(BIN_DIR, process.platform === "win32" ? "opm.exe" : "opm");

  console.log(`opm-cli: downloading v${VERSION} for ${process.platform}-${process.arch}...`);

  fs.mkdirSync(BIN_DIR, { recursive: true });

  // Download binary and checksums in parallel
  const [binaryBuf, checksums] = await Promise.all([
    downloadToBuffer(url),
    fetchChecksums().catch(() => null),
  ]);

  // Verify checksum
  if (checksums) {
    const expected = checksums.get(filename);
    if (expected) {
      const actual = crypto.createHash("sha256").update(binaryBuf).digest("hex");
      if (actual !== expected) {
        throw new Error(
          `SHA256 checksum mismatch!\n  expected: ${expected}\n  actual:   ${actual}\n` +
            "The binary may have been tampered with. Aborting."
        );
      }
      console.log("opm-cli: checksum verified ✓");
    } else {
      console.warn(`opm-cli: warning — no checksum found for ${filename} in checksums.txt`);
    }
  } else {
    console.warn("opm-cli: warning — could not fetch checksums.txt, skipping verification");
  }

  // Write binary
  fs.writeFileSync(localBin, binaryBuf, { mode: 0o755 });

  // Ensure executable on non-Windows
  if (process.platform !== "win32") {
    fs.chmodSync(localBin, 0o755);
  }

  console.log(`opm-cli: installed to ${localBin}`);
}

main().catch((err) => {
  console.error(`opm-cli: install failed — ${err.message}`);
  console.error("You can download manually from https://github.com/ayaka209/opencode-provider-manager/releases");
  // Don't fail `npm install` — the user might be on an unsupported platform
  // and just wants to install other dependencies.
  process.exit(0);
});
