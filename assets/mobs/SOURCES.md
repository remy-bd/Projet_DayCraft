# Créatures importées dans VOXEL 1.9

Les sept modèles sont des recréations communautaires des créatures de Minecraft par **22i**, téléchargées depuis [minecraft-voxel-blender-models](https://github.com/22i/minecraft-voxel-blender-models). Les fichiers Blender originaux, sous **GPL-3.0**, sont conservés dans ce dossier avec le texte complet de la licence. Ce ne sont pas des modèles extraits du jeu Mojang.

| Créature | Source Blender | Adaptation |
|---|---|---|
| Mouton | sheep.blend | Corps et laine, atlas de deux textures |
| Zombie | zombie.blend | Marche à partir du squelette original |
| Villageois | villager-rig.blend | Variante villageois du modèle combiné, sans armes ni chapeau |
| Golem de fer | golem.blend | Modèle iron_golem, hauteur 2,9 blocs |
| Creeper | creeper.blend | Variante normale, sans couche chargée |
| Vache | cow.blend | Variante vache, sans champignons |
| Cochon | pig.blend | Adulte, sans selle ni doublons du bébé |

Les modèles ont été triangulés, orientés avec Y vertical et ramenés aux dimensions du jeu. Huit poses de marche issues des squelettes Blender sont exportées en OBJ pour chaque créature. Les normales et la disposition des UV sont conservées ; les UV du mouton sont adaptés à l'atlas de remplacement. La version 1.9 corrige leur décalage horizontal de trois pixels et sélectionne la variante supérieure de 32 pixels : les faces de peau ne prennent plus des pixels transparents de la mauvaise zone. Le jeu choisit la pose, l'orientation et la taille des petits ; leurs comportements sont écrits dans le moteur VOXEL.

Les textures de remplacement viennent du dépôt [mobs_mc](https://github.com/maikerumine/mobs_mc), basé sur **Pixel Perfection** de **XSSheep**, avec les contributions de **MysticTempest**. La [notice originale de crédits](UPSTREAM-CREDITS.md) détaille leurs licences : zombie et villageois sous **CC BY-SA 4.0**, autres textures utilisées sous **MIT**, selon la notice du dépôt. La combinaison de la peau et de la laine du mouton est une modification pour cet import. Les autres skins gardent les pixels d'origine.

Les révisions, URL de téléchargement et empreintes sont dans [SOURCES.json](SOURCES.json). Les fichiers OBJ adaptés gardent la licence GPL-3.0 de leurs modèles ; le code original du jeu garde sa licence MIT. Minecraft et ses créatures appartiennent à leurs titulaires respectifs ; ce projet communautaire n'est pas affilié à Mojang.

## Reproduire la conversion

Le script `export-mobs.py` est fourni avec les sept sources Blender et les textures. Avec Python 3.11, Blender/bpy 4.2 et Pillow, lancez `python assets/mobs/export-mobs.py` depuis le dossier du jeu. Il recrée les 56 poses OBJ et les sept atlas ; si le dossier de développement `src` est présent, il met aussi à jour le tableau Rust des modèles. Il réutilise les fichiers locaux. Le script original de conversion est sous MIT ; les modèles convertis gardent GPL-3.0 et les textures leurs licences indiquées ci-dessus.
