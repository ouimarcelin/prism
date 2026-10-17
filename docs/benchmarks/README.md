# Benchmarks

Mesures de référence du moteur. La définition de « terminé » demande qu'un ticket ne fasse pas baisser la performance du benchmark 4 couches 1080p : relancez-le avant de fusionner un changement qui touche la lecture, le décodage ou le rendu, et comparez au dernier rapport.

## Benchmark de référence : 4 couches HAP 1080p, pire cas

Les clips de bruit contiennent des blocs aléatoires que Snappy ne compresse pas : c'est le contenu HAP le plus lourd à lire, décompresser et envoyer au GPU (1 Mo par image en HAP, 2 Mo en HAP Q). Un vrai film pèse moins.

Générer les clips (1 seconde chacun, 1080p60, environ 360 Mo au total ; ils tournent en boucle) :

```bash
for i in 1 2; do
  cargo run --release -p prism-media --features test-support --example make-test-clip -- media/noise-hap-$i.mov hap-noise 60
  cargo run --release -p prism-media --features test-support --example make-test-clip -- media/noise-hapq-$i.mov hap-q-noise 60
done
```

Mesure courte (60 s) :

```bash
cargo run --release -p prism-engine --example engine-run -- --seconds 60 --every 20 media/noise-hap-1.mov media/noise-hapq-1.mov media/noise-hap-2.mov media/noise-hapq-2.mov
```

Endurance (critère de la phase 1, 30 min, rapport Markdown) :

```bash
cargo run --release -p prism-engine --example engine-run -- --seconds 1800 --every 60 --report media/endurance.md media/noise-hap-1.mov media/noise-hapq-1.mov media/noise-hap-2.mov media/noise-hapq-2.mov
```

Fermez les autres applications pendant la mesure et lancez-la depuis le Terminal. Lancée par un autre programme, Claude Code par exemple, elle hérite de sa priorité réduite (`nice 5`) et n'est pas représentative.

`engine-run` mesure le moteur seul, sans fenêtre. Le verdict « FLUIDE » exige 59 images/s ou plus à chaque seconde, aucune image en retard, aucune image sautée et aucune saccade (écart de plus de 25 ms entre deux images). Chaque incident s'affiche au moment où il arrive et figure dans le rapport.

## Temps GPU d'une image

`gpu-bench` mesure, sans fenêtre, le travail que le moteur demande au GPU pour une image : envoi des trames HAP 1080p de bruit, conversion en textures de couche, mixage dans l'image programme. Les images partent deux par deux : une fois le GPU limitant, le temps par image est le sien.

```bash
cargo run --release -p prism-render --example gpu-bench -- --layers 4 --frames 600
```

Plus ce temps est court, moins un autre programme qui occupe le GPU retarde les images de Prism.

## Ce que les chiffres veulent dire

- **Images par seconde** : cadence du thread de rendu, sur la dernière seconde.
- **Travail par image** : temps passé à préparer une image (envoi des trames au GPU, mixage), moyenne et pire valeur ; le budget est de 16,7 ms à 60 images/s.
- **Pire écart entre deux images** : la plus longue attente entre deux images ; au-delà de 25 ms, c'est une saccade.
- **Images en retard** : l'image voulue n'était pas décodée à temps, la précédente est restée à l'écran.
- **GPU** : plus long délai entre l'envoi d'une image au GPU et le moment où son travail est vu terminé ; environ une période (16,7 ms) quand le GPU suit. **Images sautées** : le GPU avait encore 3 images en attente, le moteur ne lui en a pas envoyé de nouvelle et la précédente est restée à l'écran.
- **Pire durée de chaque étape** : lecture (trames reçues des threads de décodage, têtes de lecture), envoi au GPU, mixage et soumission, sorties.
- **Thread de rendu** : « temps réel » quand macOS a accepté l'ordonnancement temps réel (ADR 0012).
- **Mémoire** : empreinte du processus après 60 s et à la fin, et nombre de tampons GPU vivants. L'empreinte est mesurée par l'outil `footprint` de macOS : elle compte la mémoire GPU et la mémoire compressée, comme le Moniteur d'activité. Les tampons d'envoi et les caches se mettent en place pendant les premières secondes ; ensuite, ni l'empreinte ni le nombre de tampons ne doivent croître. Ne vous fiez pas à la mémoire résidente de `ps` : elle varie quand macOS compresse les pages inactives.
- **Swap de la machine** : espace d'échange utilisé au début et à la fin, et pagination pendant la mesure (pages lues sur disque, décompressées, échangées). Une machine qui pagine beaucoup fausse la mesure.
- **Incidents** : pour chaque saccade, l'écart et les étapes de l'image d'avant (une étape longue a retardé l'image ; des étapes courtes signifient un réveil en retard) ; pour les images en retard, leur nombre. Chaque incident indique la pagination de la machine sur la seconde écoulée.

## Rapports

| Date | Machine | Contenu | Résultat |
| --- | --- | --- | --- |
| [2026-10-14](2026-10-14-phase-1.md) | Apple M1, 8 Go, lancé depuis Claude Code (`nice 5`), machine qui pagine | 4 × bruit HAP et HAP Q 1080p60, 30 min, avant l'ADR 0012 | PAS FLUIDE : 5 saccades, pire écart 139 ms |
| [2026-10-14](2026-10-14-phase-1.md) | Idem | Idem, thread temps réel et tampons d'envoi réutilisés | FLUIDE côté processeur : 0 saccade, pire écart 16,8 ms, travail moyen 2,5 ms ; retards du GPU non mesurés |
| [2026-10-14](2026-10-14-phase-1.md) | Idem | Idem, code final (au plus 3 images en attente sur le GPU) | PAS FLUIDE : 2 images sautées sur un blocage du GPU de 80 ms ; processeur à l'heure, mémoire stable (11 tampons GPU) |
| [2026-10-14](../decisions/0013-quatre-couches-par-passe.md) | Apple M1, 8 Go | `gpu-bench`, 4 et 8 couches HAP 1080p de bruit, avant et après l'ADR 0013 | 4 couches : 3,66 → 2,19 ms de GPU par image ; 8 couches : 6,91 → 5,62 ms ; `engine-run` 60 s : FLUIDE, 2,05 ms de travail par image |

## Temps GPU des effets

`fx-bench` mesure, sans fenêtre, chaque effet et générateur seul, à ses réglages par défaut, sur une image 1080p (images deux par deux, comme `gpu-bench`). Le coût d'une chaîne est à peu près la somme de ses effets ; à comparer aux 16,7 ms d'une image à 60 images/s, moins le mixage.

```bash
cargo run --release -p prism-fx --example fx-bench -- --frames 300
```
