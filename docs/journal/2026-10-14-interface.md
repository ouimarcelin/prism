# 2026-10-14 – Ticket 6 : interface egui

- Branche : `feat/ui`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source d'egui 0.36, egui-winit, egui-wgpu, epaint et de la crate de polices d'egui (licences), dans le registre Cargo local

## Prompts donnés

1. « ok, travailles sur les 3 prochains tickets, les uns après les autres » (troisième des trois)

## Plan

Interface de contrôle egui intégrée à la boucle winit et au GPU partagé : grille, couches, crossfader, propriétés du clip, fichiers, sortie.

## Résultat

- `prism-ui` : `Document` (composition, fichier, modifications, révision, import, relocalisation des médias), `ControlPanel` (menus, grille, inspecteur, crossfader, état du moteur, fenêtres de dialogue), raccourcis.
- `prism-app` : `ControlWindow` (egui sur la surface de la fenêtre de contrôle, aperçu par texture partagée), envoi de la composition au moteur à chaque révision, actions (sortie, commandes de tête de lecture, quitter), glisser-déposer, titre.
- `prism-core` : `same_content` (le déclenchement ne marque pas le projet modifié), noms par défaut en français.
- 13 tests dans `prism-ui`, dont 6 qui dessinent le panneau sans fenêtre (raccourcis, glisser-déposer, confirmation de fermeture, médias manquants).
- Essai de lancement avec 4 mires : l'application démarre et tourne sans erreur (environ 67 % d'un cœur processeur : interface redessinée en continu, moteur et 4 décodages).
- ADR 0008.

## Problèmes et points ouverts

- egui 0.36 a changé d'API (panneaux dans un `Ui` racine, `run_ui`) : vérifiée dans le code source.
- egui exige que les mises à jour de textures soient consommées, même sans rendu : les tests les vident explicitement.
- Les touches envoyées dans la toute première image d'un contexte egui ne sont pas vues ; sans effet dans l'application, qui dessine une image avant toute saisie.
- `cargo-deny` refusait les licences des polices d'egui (OFL 1.1, Ubuntu Font Licence) : exception limitée à ce paquet, citée dans `NOTICE`.
- Non vérifié visuellement par Claude Code : capture d'écran impossible sans autorisation macOS. Test à la main à faire par l'observateur.
- À traiter : Cmd+Q sans confirmation, annulation, consommation de l'interface au repos.

## Archive

- Transcription : à archiver avec `scripts/archive-session.sh` en fin de session.
