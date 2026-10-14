# Décisions techniques (ADR)

Un fichier court par choix technique : `NNNN-titre.md`. Une décision n'est jamais effacée. Si elle change, une nouvelle ADR la remplace et l'ancienne passe au statut « Remplacée par NNNN ».

## Modèle

```markdown
# NNNN – Titre

- Date : AAAA-MM-JJ
- Statut : Proposée | Acceptée | Remplacée par NNNN

## Contexte

Le problème et les contraintes.

## Options

1. Option A : avantages, inconvénients.
2. Option B : avantages, inconvénients.

## Décision

L'option retenue et pourquoi.

## Conséquences

Ce que cela implique pour la suite.
```

## Index

| N° | Titre | Statut |
| --- | --- | --- |
| [0001](0001-processus-clean-room.md) | Processus clean room | Acceptée |
| [0002](0002-format-de-projet-json.md) | Format de projet JSON | Acceptée |
| [0003](0003-lecture-video-native.md) | Lecture vidéo native, sans FFmpeg | Acceptée |
| [0004](0004-espace-de-mixage.md) | Espace de mixage et pipeline de composition | Acceptée |
| [0005](0005-wgpu-30-et-egui.md) | wgpu 30, egui 0.36 et Rust 1.95 | Acceptée |
| [0006](0006-moteur-de-lecture.md) | Moteur de lecture et thread de rendu | Acceptée |
| [0007](0007-sorties-et-application.md) | Sorties présentées par le thread de rendu | Acceptée |
| [0008](0008-interface-egui.md) | Interface egui et document édité | Acceptée |
| [0009](0009-sources-d-images.md) | Sources d'images communes à tous les formats | Acceptée |
| [0010](0010-h264-prores-avfoundation.md) | H.264 et ProRes par AVFoundation | Acceptée |
| [0011](0011-finitions-phase-1.md) | Finitions de la phase 1 | Acceptée |
