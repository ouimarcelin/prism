# 2026-10-17 – Ticket 25 : son des clips

- Branche : `feat/25-clip-audio`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : voir `docs/SOURCES.md` (objc2-avf-audio, objc2-core-media, fixture ffmpeg)

## Prompts donnés

1. « continues le projet en autonomie jusqu'à ne être à la limite de crédit » (suite)

## Plan

1. `prism-media` : décodage complet de la piste son (AVFoundation), `ClipAudio`.
2. `prism-audio` : mixeur dans le rappel de sortie, voix calées sur les positions envoyées par le rendu ; sortie `cpal`.
3. `prism-engine` : son par flux, mises à jour à chaque image, volume × opacité, transitions.
4. Interface et application : sortie ouverte au démarrage, menu Audio, volume du clip.

## Résultat

- `prism-media` : `audio.rs`, `decode_audio`, interpolation ; test sur un clip à deux sons purs (fréquences retrouvées).
- `prism-audio` : `Mixer`, `VoiceCommand`, `AudioOutput`, `Volume` ; tests (lecture depuis la position, rattrapage et saut, silence en pause, fondu à l'arrêt).
- `prism-engine` : `sound.rs`, `ClipStream::update_sound`, `Command::SetSound` ; test d'intégration (le son arrive, volume nul muet, arrêt libère la voix).
- `prism-ui`, `prism-app` : préférences « Son des clips » et volume, volume du clip.
- ADR 0020.

## Problèmes et points ouverts

- Un premier test d'intégration échouait à cause du test lui-même (le mixeur n'était jamais rendu tant qu'aucune voix n'existait) : corrigé.
- Pas essayé ici sur un vrai fichier HAP avec son ni à l'oreille : à vérifier à la main.

## Archive

- Transcription : `2026-10-17-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `40fde0c4b388ef6fbfa6e739844447f0d6cb2b1bfc9dda4f97f999cc1e0e58a6` (même session, archivée le 17 octobre après le ticket 25 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
