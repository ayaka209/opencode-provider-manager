#!/usr/bin/env node

/**
 * postinstall: download the platform-appropriate `opm` binary
 * from the latest GitHub Release.
 */

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

// ── Main ────────────────────────────────────────────────────────────
async function main() {
  const { filename, platform: _pf } = getTarget();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${filename}`;
  const localBin = path.join(BIN_DIR, process.platform === "win32" ? "opm.exe" : "opm");

  console.log(`opm-cli: downloading v${VERSION} for ${process.platform}-${process.arch}...`);

  fs.mkdirSync(BIN_DIR, { recursive: true });

  const res = await httpGet(url);

  await new Promise((resolve, reject) => {
    const stream = fs.createWriteStream(localBin, { mode: 0o755 });
    // GitHub may send gzip for binary assets
    if (res.headers["content-encoding"] === "gzip") {
      res.pipe(zlib.createGunzip()).pipe(stream);
    } else {
      res.pipe(stream);
    }
    stream.on("finish", resolve);
    stream.on("error", reject);
  });

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
