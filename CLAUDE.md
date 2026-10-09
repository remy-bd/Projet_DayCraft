# Projet DayCraft (voxel-rust)

Clone Minecraft 3D en Rust développé avec Macroquad.
Le jeu propose de la survie, du créatif, une carte de ville (Apocalypse City), des armes à feu et des PNJ.

## Commandes utiles

- **Lancer le jeu** : `cargo run --release` (ou double-clic sur `JOUER.cmd`)
- **Compiler** : `cargo build --release`
- **Lancer les tests** : `cargo test`
- **Vérifier le code** : `cargo check` ou `cargo clippy`
- **Formater** : `cargo fmt`

## Structure du code

- `src/main.rs` : boucle de jeu, menus, cycle jour/nuit
- `src/game.rs` : joueur, physique, inventaire, craft
- `src/world.rs` : chunks, génération procédurale, eau et lave
- `src/render.rs` : moteur de rendu 3D des voxels
- `src/city.rs` : chargement de la map urbaine (`assets/maps/city.bin`)
- `src/weapons.rs` & `combat.rs` : armes modernes, balistique, tirs
- `src/explosives.rs` : grenades, C4, lance-grenades M79
- `src/villages.rs` & `settlements.rs` : villages, PNJ et marchands
- `assets/` : sons, modèles 3D, textures et carte

## Notes

- `city.bin` et `VOXEL.exe` sont trackés par **Git LFS** (ne pas les modifier sans LFS).
- Toujours vérifier que `cargo test` passe avant de push.
