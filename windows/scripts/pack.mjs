// Copies what Tauri buries in target/release/bundle/ into windows/release/, with
// the names it ships under. Used by `npm run pack` and by the release workflows,
// so both produce exactly the same file names.
//
//   Windows  bundle/nsis/*-setup.exe   → Coucou-Windows-X.Y.Z-setup.exe (+ rolling name)
//   Linux    bundle/deb/*.deb          → Coucou-Linux-X.Y.Z-amd64.deb   (+ rolling name)
//            bundle/appimage/*.AppImage → Coucou-Linux-X.Y.Z-x86_64.AppImage (+ rolling name)

import { readFileSync, mkdirSync, copyFileSync, readdirSync, statSync, chmodSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const bundleRoot = join(root, "target", "release", "bundle");
const outDir = join(root, "release");

const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));

const targets =
  process.platform === "win32"
    ? [{ dir: "nsis", suffix: "-setup.exe", names: [`Coucou-Windows-${version}-setup.exe`, "Coucou-Windows-setup.exe"] }]
    : [
        { dir: "deb", suffix: ".deb", names: [`Coucou-Linux-${version}-amd64.deb`, "Coucou-Linux-amd64.deb"] },
        {
          dir: "appimage",
          suffix: ".AppImage",
          names: [`Coucou-Linux-${version}-x86_64.AppImage`, "Coucou-Linux-x86_64.AppImage"],
        },
      ];

/** Newest file in bundle/<dir> ending in `suffix`, in case an older build is still lying around. */
function newest(dir, suffix) {
  const bundleDir = join(bundleRoot, dir);
  let files = [];
  try {
    files = readdirSync(bundleDir).filter((f) => f.endsWith(suffix));
  } catch {
    // handled below
  }
  if (files.length === 0) {
    console.error(`Nothing in ${bundleDir} — run \`npm run tauri build\` first.`);
    process.exit(1);
  }
  return files
    .map((f) => join(bundleDir, f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
}

mkdirSync(outDir, { recursive: true });
console.log("");
for (const t of targets) {
  const built = newest(t.dir, t.suffix);
  for (const name of t.names) {
    const dest = join(outDir, name);
    copyFileSync(built, dest);
    if (t.suffix === ".AppImage") chmodSync(dest, 0o755);
  }
  const mb = (statSync(built).size / 1024 / 1024).toFixed(2);
  console.log(`  ${t.names[0]} — ${mb} MB`);
}
console.log(`\n  Ready in ${outDir}\n`);
