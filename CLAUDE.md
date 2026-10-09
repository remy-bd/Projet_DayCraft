# CLAUDE.md — Guide de développement DayCraft / Voxel-Rust

Guide complet pour le développement, la maintenance et les tests sur **Projet_DayCraft** (`voxel-rust`).

---

## 1. Vue d'ensemble du Projet

- **Nom du projet** : DayCraft (`voxel-rust` v1.13.0)
- **Description** : Moteur et jeu de voxels 3D complet en Rust inspiré de Minecraft, intégrant un mode Survie, un mode Créatif, la map urbaine précalculée **Apocalypse City**, un arsenal d'armes à feu modernes avec balistique et optiques, des PNJ villageois/défenseurs, des créatures animées, et un cycle jour/nuit dynamique.
- **Édition Rust** : 2021
- **Moteur / Framework** : [Macroquad](https://macroquad.rs/) 0.4.16 (`features = ["audio"]`)
- **Dépôt Git** : [remy-bd/Projet_DayCraft](https://github.com/remy-bd/Projet_DayCraft.git)

---

## 2. Commandes Essentielles

### Compilation & Vérification
```bash
# Vérification rapide de la syntaxe et des types
cargo check

# Compilation en mode debug
cargo build

# Compilation optimisée (Release)
cargo build --release

# Analyse statique et linter (recommandé)
cargo clippy --all-targets -- -D warnings

# Formatage automatique du code
cargo fmt
```

### Exécution
```bash
# Lancer le jeu en mode debug
cargo run

# Lancer le jeu en mode release (performances optimales)
cargo run --release

# Sous Windows : double-clic sur JOUER.cmd ou VOXEL.exe
.\JOUER.cmd
```

### Tests
```bash
# Exécuter l'ensemble de la suite de tests (117 tests)
cargo test

# Exécuter un module de tests spécifique
cargo test equipment_tests
cargo test viewmodel::tests
cargo test explosives::tests
cargo test settlements::tests

# Compiler les tests sans les lancer
cargo test --no-run
```

---

## 3. Architecture du Projet (`src/`)

| Fichier / Module | Responsabilité |
|---|---|
| [`src/main.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/main.rs) | Point d'entrée principal, boucle Macroquad, menus (accueil, pause, mort, choix du spawn de ville), gestion des fenêtres et cycle jour/nuit (11 min 12 s jour / 4 min 24 s nuit). |
| [`src/game.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/game.rs) | État global du jeu (`GameState`), joueur (`Player`), physique et collisions AABB, gestion de l'inventaire, fabrication (établi/four), faim, vie et endurance. |
| [`src/world.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/world.rs) | Moteur de chunks (16×16×256), génération procédurale de biomes (plaines, déserts, montagnes, grottes), simulation des fluides (eau, lave) et persistance du monde. |
| [`src/city.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/city.rs) | Chargeur et streaming de la carte urbaine Apocalypse City (`assets/maps/city.bin`, `city.json`). Enregistrement incrémental des modifications dans `saves/apocalypse-city.json`. |
| [`src/render.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/render.rs) | Pipeline de rendu 3D, chunk meshing géométrique, atlas de textures de blocs, réflexions d'eau, occlusion culling et transparence. |
| [`src/graphics.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/graphics.rs) | Configuration de la caméra Macroquad, pipeline GLSL (`assets/shadow.glsl`), ombres portées et éclairage ambiant/soleil. |
| [`src/viewmodel.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/viewmodel.rs) | Rendu de l'arme en vue subjective (vue FPS), animations de recul, visée épaule/ADS, détachement de chargeur et animations des mains. |
| [`src/weapons.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/weapons.rs) | Définition des armes modernes (AK-74, AKM, M4A1, FAMAS, SVD, M200, AW50, MP5K, etc.), calibres, chargeurs et optiques. |
| [`src/combat.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/combat.rs) | Système de tirs balistiques, propagation des projectiles, impact sur les blocs (vitres brisées, perforation du bois), dégâts sur entités. |
| [`src/explosives.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/explosives.rs) | Armes explosives : lance-grenades M79, charges de C4 et grenades à main à fragmentation (physique des rebonds, souffle et absorption par couverture). |
| [`src/settlements.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/settlements.rs) | PNJ villageois, marchands, IA des golems de fer protecteurs, reproduction d'animaux et populating des structures. |
| [`src/villages.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/villages.rs) | Génération procédurale des structures de village (maisons, forges, églises, auberges, coffres de butin). |
| [`src/mob_models.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/mob_models.rs) / [`src/mob_assets.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/mob_assets.rs) | Modèles 3D et rigs d'animation pour les créatures (vaches, cochons, moutons, creepers, zombies, villageois, golems). |
| [`src/controls.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/controls.rs) | Gestion des entrées clavier/souris, capture du curseur, touches rapides et bascules d'état. |
| [`src/audio.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/audio.rs) / [`src/weapon_audio.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/weapon_audio.rs) | Moteur sonore spatialisé, bruitages d'armes (tir, rechargement, culasse), effets de pas, sons d'ambiance et d'eau. |
| [`src/drops.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/drops.rs) / [`src/item_icons.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/item_icons.rs) | Physique des objets au sol (ramassage, gravité), icônes 2D d'inventaire et rendu visuel. |
| [`src/sky.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/sky.rs) / [`src/shadows.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/shadows.rs) | Dôme céleste, soleil, lune, étoiles, couleurs crépusculaires et rayons d'ombre. |
| [`src/equipment_tests.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/equipment_tests.rs) / [`src/proof_v110.rs`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/src/proof_v110.rs) | Tests unitaires d'équipements, de sérialisation et de non-régression. |

---

## 4. Organisation des Ressources (`assets/`)

- `assets/maps/` : Données de la ville (`city.bin` ~200 Mo tracé par Git LFS, `city.json`, `city-atlas.png`).
- `assets/weapons/` : Modèles `.obj` et matériaux `.mtl` des armes modernes et viseurs.
- `assets/mobs/` : Modèles `.obj`, textures et fichiers Blender `.blend` des créatures.
- `assets/blocks/` : Textures 16×16 des blocs et scripts d'assemblage de textures.
- `assets/audio/` : Banques de sons WAV (tirs, culasses, bruits de pas, etc.).

---

## 5. Gestion de Version & Git LFS

Le projet utilise **Git LFS** pour les fichiers binaires volumineux :
- `*.bin` (dont [`assets/maps/city.bin`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/assets/maps/city.bin) ~200 Mo)
- `*.exe` (dont [`VOXEL.exe`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/VOXEL.exe) ~69 Mo)

> **Important** : Ne jamais ajouter de fichiers binaires lourds (> 50 Mo) sans vérifier leur configuration dans [`.gitattributes`](file:///c:/Users/debra/Documents/Codex/2026-10-06/fait-un-minecraft-3d-ultra-complet/outputs/voxel-rust/.gitattributes).

---

## 6. Règles de Code & Bonnes Pratiques

1. **Zéro Régression de Sauvegarde** : Les sauvegardes existantes (`saves/world.json`, `saves/world.bak`, `saves/apocalypse-city.json`) doivent rester rétrocompatibles lors des modifications de formats de données.
2. **Performance du Rendu & de la Physique** :
   - Éviter d'allouer de la mémoire dans la boucle de rendu (`draw()`) ou de mise à jour physique (`update()`).
   - Réutiliser les buffers de vertex et de collision lors du remeshing des chunks.
3. **Tests Obligatoires** : Avant toute validation de changement majeur, exécuter `cargo test` et s'assurer que les 117 tests passent avec succès.
4. **Style Rust** : Suivre les recommandations de `cargo clippy`. Préférer les structures déclaratives et la gestion propre d'erreurs via `Result` et `Option`.
