# Hangar — Cahier des charges

> Nom de l'application : **Hangar**
> Emplacement de l'exe portable : `O:\Apps\Hangar\`
> Plateforme : Windows 11
> Document de référence du projet : on avance **phase par phase**, chaque phase se termine par ses critères de fin validés.

---

## Sommaire

1. [Vision](#1-vision)
2. [Contraintes](#2-contraintes)
3. [Stack technique](#3-stack-technique)
4. [Architecture](#4-architecture)
5. [Direction visuelle](#5-direction-visuelle)
6. [Phases](#6-phases)
   - [Phase 0 — Initialisation](#phase-0--initialisation)
   - [Phase 1 — Bibliothèque de jeux](#phase-1--bibliothèque-de-jeux)
   - [Phase 2 — Temps de jeu](#phase-2--temps-de-jeu)
   - [Phase 3 — Moniteur matériel](#phase-3--moniteur-matériel)
   - [Phase 4 — Manettes : diagnostic et réglages](#phase-4--manettes--diagnostic-et-réglages)
   - [Phase 5 — Finitions visuelles et mode Big Picture](#phase-5--finitions-visuelles-et-mode-big-picture)
   - [Phase 6 — Manettes : émulation Xbox et profils par jeu](#phase-6--manettes--émulation-xbox-et-profils-par-jeu)
   - [Phase 7 — Fonctionnalités bonus](#phase-7--fonctionnalités-bonus)
7. [Clés d'API](#7-clés-dapi)
8. [Risques et points d'attention](#8-risques-et-points-dattention)
9. [Décisions et questions ouvertes](#9-décisions-et-questions-ouvertes)

---

## 1. Vision

Une application desktop **très soignée visuellement** qui :

- regroupe **tous mes jeux** au même endroit : ceux des plateformes (Steam, Epic…) **et** ceux que j'ajoute moi-même (jeux téléchargés, jeux portables, launchers de jeux) ;
- les lance ;
- mesure **le temps que je passe** sur chacun ;
- affiche les **caractéristiques et l'état de mon PC en temps réel** ;
- gère mes **manettes** : DualShock 4 (PS4) et DualShock 3 (PS3).

### Règle d'or : la fiche unique

Chaque jeu est **une fiche**, quelle que soit sa provenance. La provenance (`steam`, `epic`, `manual`…) n'est qu'une étiquette.
**Toutes les fonctionnalités se branchent sur la fiche, jamais sur la plateforme.** Un jeu ajouté à la main a donc exactement les mêmes fonctionnalités qu'un jeu Steam : temps de jeu, stats, jaquettes, profils manette, couleur de la barre lumineuse, mode performance, sauvegarde des saves…

Seules exceptions, liées aux API des plateformes : succès Steam et actus Steam (voir phase 7).

---

## 2. Contraintes

### 2.1 Rien n'est installé sur le disque C:

Le disque C: est presque plein. L'application doit être **entièrement portable** et vivre sur un autre disque : `O:\Apps\Hangar\`.

| Élément | Emplacement par défaut (à éviter) | Emplacement retenu |
|---|---|---|
| Application | Installeur NSIS/MSI → `C:\Program Files` ou `%LOCALAPPDATA%` | **Exe portable**, pas d'installeur. Copié où je veux. |
| Base SQLite, réglages, logs | `%APPDATA%\<identifiant>` | `<dossier de l'exe>\data\` |
| Cache des images (jaquettes, fonds…) | `%LOCALAPPDATA%` | `<dossier de l'exe>\data\cache\` |
| Données du WebView2 (cache, stockage) | `%LOCALAPPDATA%\<identifiant>\EBWebView` | `<dossier de l'exe>\data\webview\` (à forcer via `data_directory` à la création de la fenêtre) |
| Sauvegardes des saves (phase 7) | — | Dossier configurable, sur un disque autre que C: par défaut |

Règles de code :

- **Aucun** appel à `app_data_dir()` / `app_config_dir()` / `app_cache_dir()` de Tauri : un module unique `paths.rs` calcule tous les chemins à partir du dossier de l'exe.
- Un **test** vérifie qu'aucun chemin produit par `paths.rs` ne commence par `C:`.
- Si le dossier de l'exe n'est pas accessible en écriture, l'appli l'affiche clairement au lieu de se replier en silence sur `%APPDATA%`.

Ce qui existe déjà sur C: et **ne compte pas comme une installation** :

- **WebView2** : composant système de Windows 11, déjà présent (version 153).
- **Démarrage avec Windows** (phase 7, optionnel) : une simple entrée de registre `HKCU\...\Run`, aucun fichier écrit.

**Exception validée : les pilotes.** Tout pilote Windows s'installe forcément dans `C:\Windows` (magasin de pilotes, quelques Mo). Leur installation est autorisée ; elle se fait **une fois, à la main**, au début de la phase qui en a besoin — l'application elle-même n'installe jamais rien. Voir [section 9](#9-décisions-et-questions-ouvertes).

### 2.2 Environnement de développement (déjà en place, rien à installer)

| Outil | État |
|---|---|
| Rust 1.98 (toolchain MSVC) | Installé sur **O:** (`RUSTUP_HOME=O:\Rust\rustup`, `CARGO_HOME=O:\Rust\cargo`) |
| Node 22 / npm 10 | Déjà installés ; cache npm sur **O:** (`O:\dev-cache\npm-cache`) |
| Outils de compilation MSVC (Visual Studio 2022) | Déjà installés |
| WebView2 | Déjà installé |
| Git | Déjà installé |

Le dossier `target/` de Rust reste dans le projet (sur O:).

### 2.3 Autres contraintes

- **Léger** : l'appli tourne pendant que je joue → objectif < 150 Mo de RAM au repos, CPU quasi nul en arrière-plan.
- **Aucune injection dans les jeux** (pas d'overlay injecté) : risque de ban par les anti-cheat.
- **Hors ligne** : tout fonctionne sans internet, sauf la récupération des métadonnées et des images.

---

## 3. Stack technique

| Couche | Techno |
|---|---|
| Framework desktop | **Tauri 2** (backend Rust) |
| Interface | **React + TypeScript + Vite** |
| Style | **Tailwind CSS** + **shadcn/ui** (fortement personnalisé) |
| Animations | **Motion** (ex Framer Motion) |
| Graphiques | **Recharts** (ou **visx** si besoin de plus de contrôle) |
| État côté interface | **Zustand** + **TanStack Query** (appels au backend) |
| Base de données | **SQLite** via `rusqlite` (migrations versionnées) |
| Effet de fenêtre | Effets de fenêtre intégrés à Tauri 2 (Mica) + barre de titre personnalisée |
| Recherche | `fuse.js` |
| Couleurs dynamiques | `fast-average-color` (ou extraction côté Rust) |

### Crates Rust par domaine

| Domaine | Crates |
|---|---|
| Processus, CPU, RAM, disques, réseau | `sysinfo` |
| GPU NVIDIA | `nvml-wrapper` |
| Registre Windows, infos de version des exe, icônes | `windows` / `winreg` |
| Fichiers Steam (`.vdf`, `.acf`) | `keyvalues-parser` (ou parseur maison) |
| Surveillance des dossiers | `notify` |
| Manettes | `sdl3` (principal), `hidapi` (fonctions avancées DS4), `vigem-client` (phase 6) |
| HTTP (API de métadonnées) | `reqwest` |
| Discord (phase 7) | `discord-rich-presence` |

---

## 4. Architecture

### 4.1 Organisation du projet

```
launcher/
├─ src/                      # Interface React
│  ├─ app/                   # Routes / pages
│  ├─ components/            # Composants UI
│  ├─ features/              # library, stats, hardware, controllers, settings
│  ├─ lib/                   # Appels Tauri, utilitaires
│  └─ styles/                # Tokens de design
├─ src-tauri/
│  ├─ src/
│  │  ├─ paths.rs            # TOUS les chemins (portable, jamais C:)
│  │  ├─ db/                 # Connexion SQLite + migrations
│  │  ├─ library/            # Fiche jeu, ajout manuel, dossiers surveillés
│  │  ├─ scanners/           # steam.rs, epic.rs, … (un fichier par plateforme)
│  │  ├─ metadata/           # IGDB, SteamGridDB, nettoyage des noms
│  │  ├─ launcher/           # Lancement, options, scripts avant/après
│  │  ├─ tracker/            # Surveillance des processus, sessions
│  │  ├─ hardware/           # Moniteur matériel
│  │  ├─ controllers/        # SDL3, DS4, DS3
│  │  └─ commands.rs         # Commandes exposées à l'interface
│  └─ tauri.conf.json
└─ CAHIER_DES_CHARGES.md
```

### 4.2 Communication interface ↔ backend

- **Commandes Tauri** (`invoke`) pour les actions : lister les jeux, lancer, modifier une fiche…
- **Événements Tauri** pour le temps réel : état du matériel (~1/s), début/fin de session, manette branchée/débranchée, progression d'un scan.

### 4.3 Modèle de données (SQLite)

| Table | Contenu |
|---|---|
| `games` | Fiche unique : id, titre, titre de tri, provenance (`steam`, `epic`, `manual`…), id sur la plateforme, dossier d'installation, statut (`ok` / `introuvable`), favori, masqué, date d'ajout, dernier lancement |
| `game_metadata` | Id IGDB, id SteamGridDB, description, genres, développeur, éditeur, date de sortie, note, durées HowLongToBeat |
| `game_images` | Jaquette, fond, logo, icône : chemin dans le cache, provenance (`steamgriddb`, `igdb`, `exe`, `custom`) |
| `launch_actions` | Plusieurs actions par jeu (« Jouer », « Launcher », « Mods »…) : exe ou URI, arguments, dossier de travail, admin, script avant, script après, action par défaut |
| `tracked_processes` | Exe(s) à surveiller pour chaque jeu, appris automatiquement ou choisis à la main |
| `sessions` | id jeu, début, fin, durée, FPS moyen, températures max (phase 3+) |
| `collections` / `collection_games` | Collections et tags |
| `watched_folders` | Dossiers surveillés pour l'ajout automatique |
| `ignored_executables` | Exe écartés par l'utilisateur, pour ne plus les reproposer |
| `controller_profiles` | Profils manette : zones mortes, courbes, réassignations, couleur de barre lumineuse (phases 4 et 6) |
| `settings` | Clé → valeur (JSON) |

---

## 5. Direction visuelle

Objectif : une appli qui donne envie de l'ouvrir. Pas de rendu « template par défaut ».

- **Fenêtre** : sans cadre, barre de titre personnalisée, effet **Mica/Acrylic** de Windows 11.
- **Couleurs dynamiques** : la couleur dominante de la jaquette du jeu sélectionné teinte l'interface (accents, dégradés, halo).
- **Fonds** : image héro du jeu floutée derrière le contenu, et **trailer** en fond sur la fiche quand il existe.
- **Transitions partagées** : la jaquette « vole » de la grille vers la fiche du jeu (`layoutId` de Motion).
- **Cartes** : inclinaison 3D au survol avec reflet lumineux.
- **Moniteur matériel** : jauges animées style HUD.
- **Typographie** : une vraie identité (police d'affichage + police de texte), pas les polices par défaut.
- **Icônes de manette PlayStation** (✕ ○ □ △) partout dans l'interface.
- **Thème sombre** par défaut.

Les choix précis (palette, polices, tokens) sont fixés en phase 0 et affinés en phase 5.

---

## 6. Phases

Chaque phase contient : **objectif**, **fonctionnalités** (cases à cocher), **détails techniques**, **critères de fin**.

---

### Phase 0 — Initialisation

**Objectif** : un projet qui compile, s'ouvre en fenêtre soignée, et respecte la contrainte « rien sur C: » dès le premier jour.

- [x] Création du projet Tauri 2 « Hangar » + React + TypeScript + Vite
- [x] Tailwind CSS, shadcn/ui (configuré, composants ajoutés au besoin), Motion, ESLint, Prettier
- [x] Module `paths.rs` : tous les chemins calculés depuis le dossier de l'exe + test « aucun chemin sur C: »
- [x] Dossier de données du WebView2 forcé dans `data\webview\`
- [x] Fenêtre sans cadre, barre de titre personnalisée, effet Mica
- [x] Tokens de design : couleurs, espacements, rayons, polices
- [x] Structure de navigation : Bibliothèque, Stats, Matériel, Manettes, Réglages (pages vides)
- [x] SQLite + système de migrations
- [x] Build **portable** (exe seul, sans installeur)
- [x] Initialisation du dépôt Git

**Critères de fin**
- L'exe portable, copié dans un dossier sur O:, s'ouvre et affiche la fenêtre de navigation.
- Après utilisation, **aucun fichier nouveau** dans `%APPDATA%` ni `%LOCALAPPDATA%` (vérifié à la main).

---

### Phase 1 — Bibliothèque de jeux

**Objectif** : tous mes jeux apparaissent avec de belles images et se lancent, qu'ils viennent d'une plateforme ou que je les aie ajoutés moi-même.

#### 1.1 Fiche unique
- [ ] Modèle `games` + tables associées (voir 4.3)
- [ ] La provenance n'est qu'une étiquette, toutes les fonctionnalités passent par la fiche

#### 1.2 Ajout manuel
- [ ] **Glisser-déposer** d'un `.exe`, d'un raccourci `.lnk` / `.url` ou d'un dossier dans la fenêtre
- [ ] Bouton **« Ajouter un jeu »** avec sélecteur de fichier
- [ ] **Dossiers surveillés** (par défaut `H:\Games` et `N:\`) : analyse de chaque sous-dossier, propositions à valider, détection des nouveaux jeux ajoutés plus tard (crate `notify`)
- [ ] Détection du bon exe dans un dossier :
  - écarter : désinstalleurs (`unins*.exe`, `uninstall*`), `UnityCrashHandler*`, `CrashReport*`, redistribuables (`vcredist*`, `DXSETUP`, `dotnet*`, `UE4PrereqSetup*`), outils de mise à jour, `*setup*` ;
  - classer le reste : ressemblance avec le nom du dossier, taille, présence d'infos de version ;
  - **toujours demander confirmation** avant d'ajouter.
- [ ] Exe écartés mémorisés (`ignored_executables`) pour ne plus les reproposer
- [ ] Cas réels observés à gérer :
  - `H:\Games` ne contient que des jeux → scan de chaque sous-dossier ;
  - `N:\` est **mélangé** (jeux, films, archives `.rar`/`.zip`, environnements Python `*-venv`, dossiers système) → ignorer les dossiers sans exe de jeu, les dossiers système (`$RECYCLE.BIN`, `System Volume Information`, `Config.Msi`) et les venv ;
  - dossiers **d'installation de repacks** (`[FitGirl Repack]`, `[DODI Repack]` contenant `setup.exe`) → signalés « installeur, pas un jeu » au lieu d'être proposés ;
  - émulateurs présents (ex. **shadPS4**) → proposer de l'ajouter comme émulateur (phase 1.5)

#### 1.3 Scanners de plateformes
- [ ] **Steam** : `libraryfolders.vdf` + `appmanifest_*.acf` ; lancement via `steam://rungameid/<id>`
- [ ] **Epic** : `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (lecture seule) ; lancement via `com.epicgames.launcher://apps/<id>?action=launch`
- [ ] Architecture « un scanner = un fichier » pour ajouter facilement les autres plateformes (phase 7)
- [ ] Re-scan manuel + au démarrage ; les jeux disparus passent en statut « introuvable » (historique conservé)

#### 1.4 Identification et métadonnées
- [ ] Lecture du **nom du produit** dans les infos de version de l'exe
- [ ] **Nettoyage du nom** : suppression des versions (`v1.2.3`), crochets, suffixes de groupes, mentions d'édition, points/tirets/underscores
- [ ] Recherche **IGDB** + **SteamGridDB** → écran de choix avec les **jaquettes** des meilleurs résultats
- [ ] Repli si rien ne correspond : **icône extraite de l'exe** + fond généré à partir de ses couleurs
- [ ] Modification libre de la fiche : titre, jaquette, fond, logo, description (choix dans SteamGridDB ou **images perso**)
- [ ] Images téléchargées une fois et mises en cache dans `data\cache\`

#### 1.5 Lancement
- [ ] **Plusieurs actions de lancement** par jeu (« Jouer », « Launcher », « Mods », « Configuration »), une par défaut
- [ ] Options : arguments, dossier de travail, exécution en administrateur
- [ ] Scripts **avant** et **après** le jeu
- [ ] Émulateurs : jeu = ROM + modèle de commande de l'émulateur (ex. `retroarch.exe -L <core> "{rom}"`)

#### 1.6 Interface
- [ ] **Grille** de jaquettes (virtualisée pour les grosses bibliothèques)
- [ ] **Fiche du jeu** : fond héro, logo, description, bouton Jouer, actions de lancement, infos
- [ ] Recherche instantanée tolérante aux fautes (`fuse.js`)
- [ ] Filtres : provenance, installé/introuvable, favoris ; tris : nom, dernier lancement, date d'ajout
- [ ] Favoris, collections, tags, « récemment joués »
- [ ] **Relocaliser** un jeu introuvable (disque externe, lettre de lecteur changée)
- [ ] **Désinstaller** : lance le désinstalleur s'il existe, sinon supprime le dossier après double confirmation

**Critères de fin**
- Mes jeux Steam, Epic et mes jeux téléchargés apparaissent avec jaquette et fond.
- Un dossier de jeux entier est analysé et les bons exe sont proposés.
- Chaque jeu se lance depuis la fiche.
- Une fiche peut être entièrement personnalisée à la main.

---

### Phase 2 — Temps de jeu

**Objectif** : un temps de jeu fiable pour **tous** les jeux, même lancés en dehors du launcher.

#### 2.1 Surveillance
- [ ] Surveillance des processus toutes les ~5 s (`sysinfo`), coût CPU négligeable
- [ ] Distinction **« ce qu'on lance »** / **« ce qu'on surveille »** :
  - au premier lancement, repérer les nouveaux processus dont l'exe est **dans le dossier du jeu** ;
  - retenir celui qui reste ouvert le plus longtemps et utilise le GPU ;
  - le mémoriser dans `tracked_processes` ; choix manuel possible.
- [ ] **Surveillance par dossier** : le temps est compté même si le jeu est lancé sans le launcher
- [ ] Jeux Steam/Epic : exe trouvé dans le dossier d'installation (même mécanisme)
- [ ] Robustesse : session fermée proprement si le PC s'éteint ou si l'appli plante (reprise au redémarrage)
- [ ] Sessions de moins de 1 minute ignorées (réglable)

#### 2.2 Affichage
- [ ] Sur la fiche : temps total, dernière session, nombre de sessions
- [ ] **Page Stats** :
  - temps par jour / semaine / mois ;
  - top jeux sur la période ;
  - **calendrier façon GitHub** (heatmap) ;
  - historique des sessions.
- [ ] Tri de la bibliothèque par temps de jeu

**Critères de fin**
- Un jeu lancé via son propre launcher a un temps de jeu correct.
- Un jeu lancé depuis l'explorateur Windows est comptabilisé.
- Les stats sont justes après un redémarrage du PC en pleine session.

---

### Phase 3 — Moniteur matériel

**Objectif** : voir la configuration et l'état du PC en temps réel.

#### 3.1 Configuration fixe
- [ ] CPU (modèle, cœurs/threads), GPU, RAM (quantité), disques (taille, espace libre), carte mère, version de Windows

#### 3.2 Temps réel (~1 fois par seconde)
- [ ] CPU : charge globale et par cœur, fréquence
- [ ] RAM : utilisée / disponible
- [ ] GPU NVIDIA MX250 (`nvml-wrapper`) : charge, VRAM, **température**, fréquence (certaines valeurs peuvent manquer sur les GPU mobiles)
- [ ] GPU Intel Iris Plus : charge via les compteurs de performance Windows
- [ ] **GPU utilisé par le jeu en cours** (utile sur portable : un jeu lancé sur le GPU Intel par erreur tourne beaucoup moins bien)
- [ ] **Batterie du PC** : niveau, sur secteur ou non
- [ ] Disques : lecture/écriture
- [ ] Réseau : débit montant/descendant
- [ ] **Températures CPU** via LibreHardwareMonitor + pilote **PawnIO** (à installer au début de la phase)

#### 3.3 Affichage
- [ ] Page Matériel : jauges animées style HUD + courbes des 60 dernières secondes
- [ ] Mini-widget dans la barre latérale (CPU / GPU / RAM)
- [ ] Mise à jour en direct **uniquement quand la page est visible** (économie de ressources)

#### 3.4 Lien avec les sessions
- [ ] Enregistrement pendant les sessions : températures max, charge moyenne
- [ ] **Résumé de fin de session** : durée, température max, charge moyenne (FPS en phase 7)

**Critères de fin**
- Les valeurs affichées correspondent à celles du Gestionnaire des tâches (±5 %).
- L'appli consomme < 2 % de CPU avec la page Matériel ouverte.

---

### Phase 4 — Manettes : diagnostic et réglages

**Objectif** : tester et régler ma **DualShock 4** et ma **DualShock 3** depuis l'appli.

#### 4.1 Commun
- [ ] Détection des manettes branchées/débranchées (SDL3) : nom, type, USB ou Bluetooth
- [ ] **Page de test interactive** : dessin SVG de la manette, boutons qui s'allument, position des sticks, gâchettes en barres 0–100 %
- [ ] **Détection de la dérive** des sticks (mesure du bruit au repos)
- [ ] **Test des vibrations** (moteurs gauche/droit)
- [ ] Niveau de batterie dans la barre latérale

#### 4.2 DualShock 4
Fonctionne sans pilote supplémentaire.
- [ ] **Barre lumineuse** : couleur et luminosité
- [ ] **Barre lumineuse = batterie** (option) : vert → orange → rouge clignotant
- [ ] **Pavé tactile** : visualisation des 2 doigts et du clic dans la page de test
- [ ] **Gyroscope / accéléromètre** : visualisation
- [ ] Mise en veille automatique après X minutes d'inactivité
- [ ] Vibration de confirmation au lancement d'un jeu

#### 4.3 DualShock 3
Nécessite le pilote **DsHidMini** (et **BthPS3** pour le Bluetooth), installés au début de la phase.
- [ ] Détection de la présence de DsHidMini / BthPS3 ; si absents : écran d'explication avec liens officiels (l'appli n'installe rien elle-même)
- [ ] Aide à l'**appairage Bluetooth** (étape en USB pour écrire l'adresse Bluetooth du PC dans la manette)
- [ ] Page de test : boutons, sticks, **boutons sensibles à la pression**, accéléromètre
- [ ] **LED de numéro de joueur** (ex. joueur 2 quand les deux manettes sont branchées)
- [ ] Batterie (paliers approximatifs)

**Critères de fin**
- Chaque bouton, stick et gâchette de la DS4 est visible en direct dans la page de test.
- La couleur de la barre lumineuse se change depuis l'appli.
- La DS3 est détectée (si le pilote est installé) ou un message clair explique quoi faire.

---

### Phase 5 — Finitions visuelles et mode Big Picture

**Objectif** : l'appli devient vraiment belle, et utilisable entièrement à la manette sur une TV.

#### 5.1 Finitions
- [ ] **Couleurs dynamiques** tirées de la jaquette
- [ ] Fond héro flouté, **trailer** en fond sur la fiche
- [ ] Transitions partagées grille → fiche
- [ ] Inclinaison 3D + reflet des cartes
- [ ] Micro-interactions, états vides, écrans de chargement soignés
- [ ] **Palette de commandes** Ctrl+K (rechercher un jeu, lancer, aller à une page)

#### 5.2 Mode Big Picture
- [ ] Interface plein écran pensée pour la TV (grands éléments, lisible à distance)
- [ ] **Navigation complète à la manette** (focus visible, sons de navigation optionnels)
- [ ] **Icônes PlayStation** (✕ ○ □ △) partout
- [ ] **Pavé tactile DS4 comme souris** (option)
- [ ] Bouton PS → ouvre / ramène le launcher

#### 5.3 DS4 aux couleurs du jeu
- [ ] Au lancement d'un jeu, la **barre lumineuse prend la couleur dominante de sa jaquette**

**Critères de fin**
- Toute l'appli se parcourt et se lance à la manette, sans clavier ni souris.
- Animations fluides (60 i/s) sur la grille avec 200+ jeux.

---

### Phase 6 — Manettes : émulation Xbox et profils par jeu

**Objectif** : que les jeux qui ne reconnaissent que les manettes Xbox fonctionnent avec la DS4, et appliquer mes réglages dans les jeux.

**Pilotes nécessaires** : **ViGEmBus** et **HidHide** → **déjà installés sur ce PC**. Rien de plus à installer pour cette phase.
Pour la DS3, DsHidMini (installé en phase 4) propose déjà son propre mode XInput : l'émulation concerne surtout la DS4.

- [ ] **Émulation Xbox** de la DS4 : la vraie manette est cachée aux jeux (HidHide), une manette Xbox virtuelle est créée (ViGEmBus)
- [ ] **Zones mortes** par stick et par gâchette, dessinées sur le cercle du stick
- [ ] **Courbes de réponse** : linéaire, exponentielle, personnalisée (éditeur visuel)
- [ ] **Réassignation des boutons**
- [ ] **Profils par jeu**, activés automatiquement au lancement du jeu (grâce au suivi de la phase 2)
- [ ] Interrupteur **« désactiver l'émulation pour ce jeu »** (anti-cheat, jeux compatibles PlayStation nativement)
- [ ] Détection de conflit avec **Steam Input** et **DS4Windows** (avertissement)

**Critères de fin**
- Un jeu qui ne reconnaît que les manettes Xbox fonctionne avec la DS4.
- Le profil d'un jeu s'active seul au lancement et se désactive à la fermeture.

---

### Phase 7 — Fonctionnalités bonus

À prioriser au fur et à mesure. Chaque ligne peut devenir une petite phase.

#### Contenu
- [ ] **Durée pour finir** (HowLongToBeat)
- [ ] **Actus et patch notes** (API news Steam, pour les jeux Steam)
- [ ] **Succès Steam** (Steam Web API, pour les jeux Steam)
- [ ] **Galerie de captures d'écran** (surveillance des dossiers Steam, Xbox Game Bar, dossiers perso)

#### Stats
- [ ] **FPS de session** via **PresentMon** (sans injection) → FPS moyen / 1 % low dans le résumé de session
- [ ] **Bilan annuel** façon « Wrapped »
- [ ] **Alertes de temps de jeu** : limite quotidienne, rappel de pause

#### Confort
- [ ] **Mode performance au lancement** : mode d'alimentation (`powercfg`), fermeture d'applis gourmandes, Ne pas déranger — restauré à la fermeture du jeu
- [ ] **Discord Rich Presence**
- [ ] Icône dans la zone de notification, raccourci global, démarrage avec Windows (entrée de registre uniquement)
- [ ] Affichage de l'espace disque par jeu

#### Données
- [ ] **Sauvegarde des saves** avec la base de **Ludusavi** ; dossier des saves réglable à la main pour les jeux inconnus ; destination hors C: ; synchronisation cloud optionnelle
- [ ] Export / import de la bibliothèque (JSON)

#### Plateformes supplémentaires
- [ ] GOG (registre `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`)
- [ ] Ubisoft Connect, EA App, Battle.net, Riot (registre et fichiers de configuration)
- [ ] Xbox / Game Pass (paquets UWP, le plus complexe)
- [ ] Scan de ROMs pour RetroArch

#### Achats
- [ ] **Liste de souhaits** avec suivi des prix (API IsThereAnyDeal)

---

## 7. Clés d'API

Clés **personnelles**, saisies dans les Réglages et stockées localement dans `data\` (jamais dans le code source).

| Service | Utilisation | Phase | Obtention |
|---|---|---|---|
| **SteamGridDB** | Jaquettes, fonds, logos, icônes | 1 | Compte sur steamgriddb.com → Préférences → API |
| **IGDB** (via Twitch) | Titres, descriptions, genres, notes | 1 | Application sur dev.twitch.tv → Client ID + Client Secret |
| **Steam Web API** | Succès, actus | 7 | steamcommunity.com/dev/apikey |
| **IsThereAnyDeal** | Prix | 7 | isthereanydeal.com → Apps |

---

## 8. Risques et points d'attention

| Sujet | Risque | Réponse |
|---|---|---|
| **Pilotes et disque C:** | Tout pilote Windows s'installe dans `C:\Windows` (quelques Mo). Impossible autrement. | Exception **validée** (section 9). ViGEmBus et HidHide sont déjà installés ; PawnIO (phase 3) et DsHidMini/BthPS3 (phase 4) seront installés au début de leur phase. L'appli n'installe jamais de pilote elle-même. |
| **DualShock 3** | Aucun support Windows sans DsHidMini. État actuel du projet à vérifier sur son GitHub avant installation. | Support DS3 conditionné à la présence du pilote, message clair sinon. |
| **ViGEmBus** | Plus maintenu depuis 2023 : fonctionne encore, mais risque à long terme. | L'émulation (phase 6) reste une option qu'on peut désactiver. |
| **Anti-cheat** | Certains anti-cheat refusent les manettes virtuelles. | Interrupteur par jeu. Jamais d'injection dans les jeux. |
| **Températures CPU** | Windows ne les expose pas sans pilote noyau (LibreHardwareMonitor → PawnIO ou WinRing0 ; WinRing0 est signalé par Windows Defender). | **PawnIO** retenu (pilote autorisé, section 9). Les températures GPU NVIDIA fonctionnent sans pilote supplémentaire. |
| **Steam Input / DS4Windows** | Conflits avec l'émulation et la barre lumineuse. | Détection et avertissement. |
| **Identification des jeux téléchargés** | Noms de dossiers peu fiables. | Nettoyage + infos de version de l'exe + **choix toujours confirmé** par l'utilisateur. |
| **Jeux sur disques externes** | Lettre de lecteur qui change. | Statut « introuvable » + « Relocaliser », historique conservé. |
| **Espace disque** | Cache d'images qui grossit. | Taille du cache affichée dans les Réglages + bouton « Vider le cache ». |

---

## 9. Décisions et questions ouvertes

### Questions ouvertes

| # | Question | Phase | Options |
|---|---|---|---|
| 7 | **Plateformes utilisées** en dehors de Steam et Epic | 1 / 7 | Pour prioriser les scanners |

### Décisions prises

| # | Sujet | Décision | Phase |
|---|---|---|---|
| 1 | Nom de l'application | **Hangar** | 0 |
| 2 | Emplacement de l'exe portable | **`O:\Apps\Hangar\`** (données dans `O:\Apps\Hangar\data\`) | 0 |
| 3 | Pilotes DS3 (DsHidMini + BthPS3) | **Autorisés** → DS3 prise en charge. Installés au début de la phase 4. | 4 |
| 4 | Températures CPU | **Pilote autorisé** (PawnIO) → températures CPU affichées. Installé au début de la phase 3. | 3 |
| 5 | Matériel | PC portable, Intel Core i7-1065G7, **NVIDIA GeForce MX250** + **Intel Iris Plus**. NVIDIA via `nvml-wrapper` ; Intel via les compteurs de performance Windows (« GPU Engine »). Afficher les deux GPU, **celui utilisé par le jeu** en cours, et la **batterie du PC**. | 3 |
| 6 | Dossiers des jeux téléchargés | **`H:\Games`** et **`N:\`** → dossiers surveillés par défaut | 1 |
