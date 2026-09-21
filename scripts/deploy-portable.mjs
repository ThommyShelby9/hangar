// Copies the release executable to the portable install folder (never C:).
// Usage: npm run build:portable   — target folder can be overridden with HANGAR_DIR.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const target = process.env.HANGAR_DIR ?? "O:\\Apps\\Hangar";
const exe = join(import.meta.dirname, "..", "src-tauri", "target", "release", "Hangar.exe");

if (/^c:/i.test(target)) {
  console.error(`Refusé : ${target} est sur C:. Hangar ne s'installe pas sur C:.`);
  process.exit(1);
}
if (!existsSync(exe)) {
  console.error(`Exécutable introuvable : ${exe}. Lance d'abord « npm run tauri build ».`);
  process.exit(1);
}

mkdirSync(target, { recursive: true });
copyFileSync(exe, join(target, "Hangar.exe"));
console.log(`Hangar copié dans ${target}`);
