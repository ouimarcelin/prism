# 0036 – Plugins

- Date : 2026-10-18
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

La phase 6 demande un « format de plugin ouvert (FFGL ou interface Rust) pour ajouter des effets » (`docs/SPEC.md`). Les effets sont rendus par wgpu (Metal, DirectX 12, Vulkan) ; les shaders ISF sont déjà lus fichier par fichier (ADR 0018).

## Options

- **FFGL** : plugins compilés qui dessinent en OpenGL. wgpu n'expose pas de contexte OpenGL partagé sous Metal ; il faudrait un second moteur de rendu, des copies d'images entre les deux et des binaires par plateforme. Écarté.
- **Interface Rust (bibliothèques dynamiques)** : Rust n'a pas d'ABI stable ; chaque plugin devrait être compilé avec la même version du compilateur et des crates, et le chargement demande du code `unsafe`, interdit dans l'espace de travail. Écarté.
- **ISF dans un dossier de plugins** : format ouvert et documenté, des centaines de shaders existants, multiplateforme, sans code natif à charger. Retenu.

## Décision

- **Dossier** : `Plugins`, à côté des préférences (`~/Library/Application Support/Prism/Plugins` sur macOS, `%APPDATA%\Prism\Plugins` sur Windows). Les fichiers `.fs`, `.isf` et `.frag` y sont lus, ainsi que dans deux niveaux de sous-dossiers ; le nom affiché suit les sous-dossiers (« Glitch / Décalage »). Fichiers et dossiers cachés ignorés.
- **Menus** : « Ajouter un effet… » propose les plugins qui lisent une image (`inputImage`), « Générateur… » ceux qui dessinent leur propre image. Un plugin à l'en-tête illisible reste listé, grisé, avec la raison au survol. « Ouvrir le dossier des plugins » crée le dossier et l'ouvre dans le Finder ou l'Explorateur.
- **Lecture** : seuls les en-têtes sont lus pour les menus, au plus toutes les 2 s pendant qu'un menu est ouvert ; le code est traduit au premier usage, comme un shader ISF choisi à la main, et rechargé quand le fichier change.
- Un effet de plugin est un effet ISF ordinaire : le projet garde le chemin du fichier.

## Conséquences

- Un projet qui utilise un plugin, ouvert sur un autre ordinateur, demande le même fichier : il apparaît comme un fichier manquant, à copier dans le dossier des plugins ou à côté du projet.
- Les limites d'ISF de l'ADR 0018 s'appliquent (une passe, pas d'images importées).
