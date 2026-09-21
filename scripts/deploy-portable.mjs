// Copies the release build to the portable install folder (never C:).
// Usage: npm run build:portable   — target folder can be overridden with HANGAR_DIR.
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const target = process.env.HANGAR_DIR ?? "O:\\Apps\\Hangar";
const root = join(import.meta.dirname, "..");
const files = [
  [join(root, "src-tauri", "target", "release", "Hangar.exe"), "Hangar.exe"],
  [join(root, "sensors", "target", "release", "hangar-sensors.exe"), "hangar-sensors.exe"],
  [join(root, "sensors", "modules", "IntelMSR.bin"), "IntelMSR.bin"],
  [join(root, "vendor", "sdl3", "SDL3.dll"), "SDL3.dll"],
];

if (/^c:/i.test(target)) {
  console.error(`Refusé : ${target} est sur C:. Hangar ne s'installe pas sur C:.`);
  process.exit(1);
}
const missing = files.filter(([source]) => !existsSync(source));
if (missing.length) {
  console.error(`Fichiers introuvables, lance d'abord le build :\n${missing.map(([s]) => `  ${s}`).join("\n")}`);
  process.exit(1);
}

mkdirSync(target, { recursive: true });
for (const [source, name] of files) {
  const destination = join(target, name);
  // Unchanged files are left alone: hangar-sensors.exe runs all the time and is locked by Windows.
  if (existsSync(destination) && readFileSync(source).equals(readFileSync(destination))) continue;
  try {
    copyFileSync(source, destination);
    console.log(`  ${name} mis à jour`);
  } catch (error) {
    const hint =
      name === "hangar-sensors.exe"
        ? "Le capteur tourne en arrière-plan : arrête la tâche « Hangar - capteurs » dans le Planificateur de tâches, puis relance."
        : "Ferme Hangar s'il est ouvert, puis relance.";
    console.error(`Impossible de copier ${name} : ${error.message}\n${hint}`);
    process.exit(1);
  }
}
console.log(`Hangar copié dans ${target}`);
