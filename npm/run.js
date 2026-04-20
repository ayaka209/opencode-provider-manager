#!/usr/bin/env node

/**
 * Proxy script — spawns the native `opm` binary downloaded by install.js.
 */

const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

const binName = process.platform === "win32" ? "opm.exe" : "opm";
const binPath = path.join(__dirname, "bin", binName);

if (!fs.existsSync(binPath)) {
  console.error(
    "opm-cli: binary not found. Try running `npm rebuild opm-cli` or install manually from\n" +
      "https://github.com/ayaka209/opencode-provider-manager/releases"
  );
  process.exit(1);
}

const child = spawn(binPath, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});

child.on("exit", (code) => process.exit(code ?? 0));
