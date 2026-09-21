# Hangar

Launcher de jeux pour Windows 11 : bibliothèque unifiée (Steam, Epic, jeux ajoutés à la main), temps de jeu, moniteur matériel, réglages des manettes DualShock.

Le détail des fonctionnalités et l'avancement phase par phase sont dans [CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md).

## Portable : rien sur C:

Hangar écrit **uniquement** dans le dossier `data\` à côté de son exe (base SQLite, cache, profil WebView2). Tous les chemins sont définis dans [src-tauri/src/paths.rs](src-tauri/src/paths.rs).

## Développement

```sh
npm install
npm run tauri dev        # appli en mode développement (données dans src-tauri/target/debug/data)
npm run lint             # ESLint
npm run typecheck        # TypeScript
npm run format           # Prettier
cd src-tauri && cargo test
```

## Build portable

```sh
npm run build:portable   # compile puis copie Hangar.exe dans O:\Apps\Hangar
```

Le dossier cible peut être changé avec la variable `HANGAR_DIR`. Le script refuse un dossier sur C:.
