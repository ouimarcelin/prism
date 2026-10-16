# 0019 – Entrée audio et bandes de fréquences

- Date : 2026-10-17
- Statut : Acceptée (prise en mode autonome, à revoir sur demande)

## Contexte

Le cahier des charges demande, en phase 2, une entrée audio et une analyse FFT par bande pour piloter les paramètres. La lecture du son des clips est un autre ticket.

## Décisions

- **Crate `prism-audio`** : entrée par `cpal` (Core Audio sur macOS, WASAPI sur Windows), analyse par `rustfft`. Le son est mixé en mono.
- **Quatre bandes** : niveau (20 Hz–20 kHz), graves (20–250 Hz), médiums (250 Hz–2 kHz), aigus (2–16 kHz). Fenêtre de Hann de 2048 échantillons, une mesure tous les 512 (environ 94 par seconde à 48 kHz).
- **Niveau** : énergie de la bande (Parseval) ramenée à l'amplitude d'un sinus, en décibels ; de 0 à 1 sur les 50 dB sous la pleine échelle. Montée immédiate, retombée de toute la plage en 0,25 s. Gain (« sensibilité ») de 0 à 40 dB.
- **Sans verrou** : l'analyse tourne dans le rappel audio, sans allocation ; les niveaux sont publiés dans des atomiques (`AudioLevels`), lus par le thread de rendu à chaque image et par l'interface.
- **Modèle** : `Effect::audio` relie un réglage à une bande, qui parcourt toute sa plage. Ordre de priorité : animation, bande du son, curseur du tableau de bord, valeur réglée. Possible aussi pour les effets de la composition.
- **Préférences** (pas le projet) : entrée activée ou non, appareil, sensibilité. L'entrée est éteinte par défaut, pour ne pas demander l'accès au micro sans raison.

## Options écartées

- **Bandes réglables** (fréquences, nombre) : plus souple, plus long à présenter. Les quatre bandes fixes couvrent l'usage courant ; à revoir sur observation.
- **Analyse sur le thread de rendu** : lier la mesure à la cadence d'affichage, et copier le son entre threads.

## Conséquences

- macOS demande l'accès au micro à la première écoute (au Terminal quand Prism y est lancé). L'application empaquetée devra déclarer `NSMicrophoneUsageDescription`.
- L'opacité des couches ne suit pas encore le son (seulement les réglages d'effets).
