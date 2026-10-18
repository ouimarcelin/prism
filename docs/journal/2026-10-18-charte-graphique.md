# 2026-10-18 – Ticket 75 : charte graphique

- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Transcription : `2026-10-18-144246-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `536b266d882e7532436c00b8761efd471cd0dea91284bfdae040fe5a96ff60ac`

## Prompt

« analyses et comprends les différentes pages de ce document : https://www.figma.com/design/5tZBgBPJi2K8bBWsabzoL8/… (l'objectif est d'utiliser cette charte graphique dans le logiciel.) », puis « rebrand, modifies l'interface du logiciel pour qu'elle soit cohérente, moderne et qu'elle corresponde à l'ui kit du fichier figma. »

## Plan

- Lire le kit : pages, variables (couleurs clair et sombre, tailles, typographie), styles de texte et d'ombre, dimensions des composants.
- Un module de thème unique pour egui, appliqué à toutes les fenêtres.
- Remplacer les couleurs écrites en dur, redessiner la grille de clips, aligner la page de télécommande.
- Comparer avant et après sur des captures hors écran.

## Résultat

- Kit lu : 29 pages (fondations, une vingtaine de composants, modèles d'écrans), 946 variables de couleur en 8 modes, 12 de taille, 57 de typographie, 10 styles d'ombre. Le quota de l'offre Figma Starter a été atteint avant de lire le texte des pages Vue d'ensemble, Grille et Icônes ; les valeurs utiles étaient déjà relevées.
- `crates/ui/src/theme.rs` : couleurs, Inter 500 et 600 (Google Fonts, OFL), styles de texte, espacements, visuels des widgets, cadres des panneaux ; `apply_theme` appelé par l'application à la création de chaque fenêtre de contrôle.
- Grille de clips en cartes dessinées à la main ; en-têtes de colonnes et « + Ajouter une couche » sans cadre au repos ; couche sélectionnée en bleu ; titres en demi-gras blanc.
- Éditeur de scène, lumière, apprentissage MIDI et inspecteur aux couleurs du thème ; page de télécommande alignée.
- Exemple `capture` (egui_kittest, rendu wgpu) : `controle`, `clip`, `menu`, `scene`, `lumiere` en PNG.
- Vérifié : fmt, clippy (macOS, avec NDI, et Windows en compilation croisée), 320 tests, `cargo deny`, clean room. Prism.app reconstruite.

## Problèmes

- egui applique de nouvelles polices au début de la passe suivante : un contexte qui dessine avant d'avoir reçu le thème s'arrête sur la famille demi-grasse inconnue. L'exemple de capture applique le thème à la première passe ; les tests créent leurs contextes avec le thème.
- Google Fonts ne livre en fichiers statiques que des graisses multiples de 100 : 500 et 600 au lieu de 450 et 550 du kit.
- La poignée des curseurs d'egui prend la couleur de fond des cases à cocher : impossible de la rendre blanche comme dans le kit sans casser les cases ; laissée grise cerclée de blanc.
- Point ouvert : la licence exacte du fichier communautaire Figma n'a pas été vérifiée ; seules des valeurs (couleurs, tailles) sont reprises, aucun élément graphique.
