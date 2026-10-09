# Vérification — VOXEL 1.13

Windows 64 bits, 9 octobre 2026. **117 tests réussis**, Clippy sans avertissement, compilation optimisée.

- Régression reproduite sur la version précédente : la texture de moss_block contenait des pixels transparents. La mousse et son tapis utilisent désormais une texture opaque ; les fleurs sont des plantes traversables qui conservent les faces du terrain. Les autres approximations ajourées ne masquent plus les blocs voisins.
- Tous les pixels des textures de mousse sont contrôlés, ainsi que les fleurs importées et l'absence de trous dans les textures des cubes qui masquent le terrain. Atlas ancien, blocs partiels, collisions, tirs, inventaire, fabrication, fluides, sauvegardes et autres mécaniques restent couverts par les tests.
- Durée du jour doublée : environ 672 secondes ; nuit inchangée à 264 secondes. Vitesse des deux phases, passage minuit et cycle complet contrôlés. Seul le rythme de l'heure change, les animations conservent leur vitesse.
- Trois scènes natives de 180 images : ville avec shaders, centre sans shaders et régressions de gameplay. Les contrôles GPU utilisent le vrai maillage pour comparer la mousse et les fleurs sur un sol, dans les deux passes de rendu ; profondeur des fluides, portes/coffres, ombres, reflets et post-traitement contrôlés également.
- Identité de map, ordre des 2 247 états, palettes, points de départ, coffres et population inchangés. Les sauvegardes personnelles et l'archive v1.12 conservent leurs empreintes SHA-256. La géométrie de la map n'est pas régénérée.

Preuves et captures : screenshots/v1.13. SHA-256 de VOXEL.exe : `fe025bf6e3f8667cc5cbb319817d358f98c07abc150e69a1751744cc3323c221`.

Pas de longue partie manuelle ni de test sur un autre PC/GPU. Les textures restent adaptées du pack communautaire ; provenance dans assets/maps/SOURCES.json. Gardez le dossier assets à côté de l'exécutable.
