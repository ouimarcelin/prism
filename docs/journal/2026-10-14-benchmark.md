# 2026-10-14 – Ticket 11 : benchmark et endurance de la phase 1

- Branche : `feat/benchmark`
- Outil : Claude Code (application de bureau), modèle Claude Opus 5.5
- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Sources consultées : code source de `wgpu-core` 30, `wgpu-hal` 30 (backend Metal), `wgpu::util::StagingBelt` et `libc` 0.2.190 (registre Cargo local) ; API Mach d'ordonnancement temps réel, connue de mémoire ; outils macOS `ps`, `vm_stat` et `sysctl` en boîte noire (voir `docs/SOURCES.md`)

## Prompts donnés

1. « parfait, continues, fais les 5 tickets suivants. » (cinquième des cinq ; session interrompue par la limite d'usage pendant l'endurance)
2. « vas y contiues le prochain ticket. » Le ticket 11 restait ouvert, et la phase 2 attend que le critère de la phase 1 passe : la session a donc repris le ticket 11.

## Plan

1. Mesurer ce que le critère de la phase 1 demande : saccades, pire écart entre deux images, pire temps de préparation d'une image, mémoire.
2. Faire d'`engine-run` un test d'endurance, avec un rapport Markdown, sur le contenu HAP le plus lourd (clips de bruit).
3. Lancer 30 minutes, expliquer chaque saccade, corriger.
4. Guide et rapport de benchmark, ADR.

## Résultat

- Mesures dans l'état du moteur : pire écart entre deux images, saccades (plus d'une période et demie), pire temps de préparation, puis durée de chaque étape d'une image (lecture, envoi au GPU, mixage, sorties) et description de la dernière saccade.
- `engine-run` : durée et relevé réglables, mémoire du processus, swap et pagination de la machine, chaque incident (saccade, images en retard) au moment où il arrive, rapport Markdown, verdict FLUIDE ou PAS FLUIDE (code de sortie). `make-test-clip` : clips de bruit HAP et HAP Q.
- Première endurance (02:49–03:19), 4 clips de bruit d'une seconde : **PAS FLUIDE**. 60 images/s tenues (59,8 au pire), mais 5 saccades, dont une de 139,3 ms pendant la préparation d'une image, 4 images en retard, 3 départs en retard ; mémoire stable (73 puis 71 Mo).
- Enquête :
  - Dans wgpu 30, `Queue::write_texture` crée un tampon Metal neuf à chaque appel, soit 6 Mo de mémoire fraîche par image. La machine de mesure (8 Go) avait 2,1 Go de swap et 60 Mo de mémoire libre.
  - Lancé depuis Claude Code, `engine-run` hérite de `nice 5`, et ses threads restent en priorité 31 malgré la classe « interface utilisateur ». Le thread de rendu se réveillait jusqu'à 10 ms trop tard.
- Corrections (ADR 0012) : tampons d'envoi réutilisés (`StagingBelt`, copies enregistrées dans l'encodeur de l'image), paramètres écrits seulement s'ils changent, interrogation du GPU sans attente avant les envois, thread de rendu temps réel (politique « time constraint » de Mach).
- Mesures de 60 s, un essai par configuration, lancées depuis Claude Code :

| Configuration | Travail moyen | Pire envoi au GPU | Pire écart | Saccades | Images en retard |
| --- | --- | --- | --- | --- | --- |
| Avant (classe « interface utilisateur », `write_texture`) | 3,93 ms | 4,9 ms | 23,3 ms | 0 | 0 |
| Tampons réutilisés, sans temps réel | 2,30 ms | 32,9 ms | 38,1 ms | 4 | 0 |
| Temps réel, `write_texture` | 3,48 ms | 15,0 ms | 16,7 ms | 0 | 4 |
| Temps réel et tampons réutilisés | 2,30 ms | 1,4 ms | 16,8 ms | 0 | 0 |

- Seconde endurance (10:25–10:55), avec les deux corrections : **FLUIDE**. 108 001 images, 60,0 images/s à chaque seconde, aucune saccade, aucune image en retard, pire écart 16,8 ms, travail moyen 2,54 ms (4,7 ms au pire).
- Mémoire : la mémoire résidente de `ps` est passée de 82 à 105 Mo pendant cette endurance. Un essai suivant l'a vue monter puis redescendre (92, 129, puis 117 Mo) : sur une machine qui pagine, elle suit la compression des pages par macOS. `engine-run` relève maintenant l'empreinte du processus (outil `footprint`) et le nombre de tampons GPU (compteurs de wgpu). Sur 5 minutes : 252 à 253 Mo et 12 tampons GPU de la 20e à la 300e seconde, sans variation.
- Troisième endurance (11:07–11:37), empreinte relevée chaque minute : FLUIDE, mais 11 puis 13 tampons GPU (+24 Mo) à la 11e minute, sans saccade vue par le processeur. Le GPU avait pris plusieurs images de retard. Correction : au plus 3 images en attente sur le GPU, images sautées comptées et incluses dans le verdict, latence GPU mesurée.
- Quatrième endurance (11:39–12:09), code final : **PAS FLUIDE**. Processeur à l'heure (6,0 ms de travail au pire, aucune image en retard), mémoire stable (11 tampons GPU du début à la fin, 232 puis 239 Mo), mais un blocage du GPU de 80 ms à la 1 204e seconde : 2 images sautées, 2 saccades de 33 ms. Le blocage est venu d'ailleurs, 4 secondes après une réponse de Claude affichée par l'application de bureau sur le même GPU.
- Guide `docs/benchmarks/README.md`, rapport `docs/benchmarks/2026-10-14-phase-1.md`, ADR 0012.
- 154 tests au total, tous verts ; `cargo deny` et le contrôle clean room passent.

## Problèmes et points ouverts

- Les mesures lancées depuis Claude Code tournent avec `nice 5` sur une machine qui pagine : elles servent à comparer, pas à valider. La mesure de référence se lance depuis le Terminal, les autres applications fermées (procédure dans `docs/benchmarks/README.md`).
- Le critère de la phase 1 demande aussi le test à la main : vos fichiers HAP 1080p, sortie plein écran sur le second écran, 30 minutes.
- Images en retard quand la machine pagine beaucoup : les threads de décodage relisent les fichiers sur le disque, et l'avance de décodage est de 6 images (100 ms).
- Blocages du GPU venus d'autres programmes : désormais visibles (images sautées). Si la mesure de référence en montre encore, alléger le travail du GPU (environ 300 Mo lus et écrits par image ; fusionner conversion, mixage et copie).
- La synchronisation du moteur avec le rafraîchissement de l'écran de sortie n'est pas étudiée.
- Windows n'a pas encore d'équivalent du thread temps réel (MMCSS).

## Archive

- Transcription : `2026-10-14-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `955a36a2600b154657f197381787cd82ef89bef31efd45eb9d96556c2a8b97cd` (session Claude Code unique des tickets 1 à 11, archivée le 2026-10-14 à 12 h 11 ; dossier `~/Documents/prism-archives/sessions/`, hors du dépôt).
