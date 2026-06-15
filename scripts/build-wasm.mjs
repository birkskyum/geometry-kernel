import { copyFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const pkgDir = join(root, "pkg");
const wasmInput = join(root, "target/wasm32-unknown-unknown/release/geometry_kernel.wasm");
const wasmOutput = join(pkgDir, "geometry_kernel_bg.wasm");
const wasmOptimized = join(pkgDir, "geometry_kernel_bg.opt.wasm");

function run(command, args) {
  execFileSync(command, args, {
    cwd: root,
    stdio: "inherit",
  });
}

rmSync(pkgDir, { recursive: true, force: true });

run("cargo", [
  "build",
  "--release",
  "--target",
  "wasm32-unknown-unknown",
  "--no-default-features",
  "--features",
  "wasm",
]);

run("wasm-bindgen", [
  "--target",
  "web",
  "--out-dir",
  "pkg",
  "--out-name",
  "geometry_kernel",
  wasmInput,
]);

try {
  run("wasm-opt", ["-Oz", wasmOutput, "-o", wasmOptimized]);
  rmSync(wasmOutput);
  copyFileSync(wasmOptimized, wasmOutput);
  rmSync(wasmOptimized);
} catch {
  // wasm-opt is optional for local builds.
}

const rootPackage = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const packageJson = {
  name: rootPackage.name,
  version: rootPackage.version,
  description: rootPackage.description,
  author: rootPackage.author,
  license: rootPackage.license,
  type: "module",
  exports: {
    ".": {
      types: "./geometry_kernel.d.ts",
      import: "./geometry_kernel.js",
    },
  },
  files: [
    "geometry_kernel.js",
    "geometry_kernel.d.ts",
    "geometry_kernel_bg.wasm",
    "geometry_kernel_bg.wasm.d.ts",
  ],
  dependencies: rootPackage.dependencies ?? {},
};

if (existsSync(join(pkgDir, "snippets"))) {
  packageJson.sideEffects = ["./snippets/**/*.js"];
  packageJson.files.push("snippets");
}

writeFileSync(join(pkgDir, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);

if (existsSync(join(root, "README.md"))) {
  copyFileSync(join(root, "README.md"), join(pkgDir, "README.md"));
}
