# Projet DayCraft (voxel-rust)

Moteur et jeu de voxels 3D en Rust développé avec la bibliothèque Macroquad.
Le projet combine survie, mode créatif, exploration, une map urbaine précalculée ("Apocalypse City"), un arsenal d'armes modernes et des PNJ.

## Commandes utiles

- **Lancer le jeu** : `cargo run --release` (ou double-clic sur `JOUER.cmd`)
- **Lancer en mode debug** : `cargo run`
- **Compiler en release** : `cargo build --release`
- **Lancer les tests** : `cargo test` (117 tests automatisés)
- **Tester un module précis** : `cargo test <nom>` (ex: `cargo test equipment_tests`)
- **Vérifier les types / linter** : `cargo check` ou `cargo clippy`
- **Formater le code** : `cargo fmt`

## Gameplay et Contrôles

- **Déplacement** : ZQSD / WASD pour bouger, Espace pour sauter (double-saut pour voler en créatif), Shift pour s'accroupir.
- **Inventaire & Fabrication** : `E` pour le sac, `C` pour le catalogue créatif, `1-9` pour la barre rapide.
- **Interactions** : clic gauche pour miner ou attaquer, clic droit pour poser un bloc ou ouvrir une station (établi, four, coffre).
- **Combat & Armes** : clic gauche pour tirer, clic droit pour viser (ADS), `R` pour recharger, `V` pour le sélecteur de tir, `G` pour jeter.
- **Modes de jeu** :
  - *Survie* : gestion de la santé, de la faim, dégâts de chute et craft via stations (établi, four).
  - *Créatif* : catalogue complet de blocs et armes, vol libre et ressources infinies.
  - *Apocalypse City* : exploration urbaine avec 6 points de spawn au choix (Entrée est, Centre-ville, etc.).
- **Cycle temporel** : jour d'environ 11 min 12 s et nuit de 4 min 24 s.

## Structure du code (`src/`)

- `src/main.rs` : boucle principale Macroquad, gestion des états (menus, jeu, pause, mort), cycle jour/nuit.
- `src/game.rs` : joueur (`Player`), état global (`GameState`), physique AABB, inventaire et fabrication.
- `src/world.rs` : système de chunks (16×16×256), génération procédurale, propagation des fluides (eau, lave).
- `src/render.rs` : pipeline de rendu 3D, chunk meshing, atlas de textures de blocs, reflets d'eau.
- `src/viewmodel.rs` : vue subjective (FPS), animations de recul, visée ADS, éjection de chargeur et mains articulées.
- `src/weapons.rs` & `combat.rs` : balistique, calibres, tirs et dégâts des armes à feu (AK, M4, FAMAS, snipers...).
- `src/explosives.rs` : armes explosives (grenades à fragmentation, charges C4, lance-grenades M79).
- `src/city.rs` : chargement et streaming binaire de la map Apocalypse City (`assets/maps/city.bin`, `city.json`).
- `src/villages.rs` & `settlements.rs` : génération des structures de village, PNJ, marchands et golems protecteurs.
- `src/mob_models.rs` & `mob_assets.rs` : modèles 3D (.obj) et animations des créatures (animaux, zombies, creepers).
- `src/graphics.rs`, `shadows.rs` & `sky.rs` : ombres portées, shaders GLSL et rendu du ciel dynamique.
- `src/audio.rs` & `weapon_audio.rs` : sons spatialisés des armes, effets de pas et ambiances sonores.
- `src/drops.rs` & `item_icons.rs` : physique des objets au sol et génération des icônes d'inventaire.
- `src/controls.rs` : capture souris, sensibilité et gestion des touches.

## Données & Sauvegardes

- `saves/world.json` : sauvegarde de la carte procédurale classique.
- `saves/apocalypse-city.json` : delta des modifications de blocs et inventaire sur Apocalypse City.
- `assets/maps/` : carte urbaine (`city.bin` de 200 Mo sous Git LFS, `city.json`, atlas de textures).
- `assets/weapons/` & `assets/mobs/` : modèles 3D (.obj/.mtl), textures et animations.
- `assets/audio/` : bruitages et ambiances au format WAV.
- `saves/` est ignoré par Git pour ne pas synchroniser les parties en cours.

## Bonnes pratiques de dev

- **Git LFS** : `assets/maps/city.bin` (200 Mo) et `VOXEL.exe` (69 Mo) sont sous Git LFS. Ne jamais committer de gros binaires sans LFS.
- **Tests obligatoires** : lancer systématiquement `cargo test` avant de push (tous les 117 tests doivent passer).
- **Performances** : zéro allocation inutile dans `update()` et `draw()` pour garantir 60 FPS constants.
- **Rétrocompatibilité** : préserver scrupuleusement la compatibilité des sauvegardes JSON existantes.
