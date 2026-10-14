# 0013 – Quatre couches par passe de mixage

- Date : 2026-10-14
- Statut : Acceptée ; remplace la décision « une passe par couche » de l'ADR 0004

## Contexte

Le test de la phase 1 montre encore des images sautées. Dans les mesures d'`engine-run`, elles viennent de blocages du GPU de 50 à 80 ms (ADR 0012). Plus le travail du moteur sur le GPU est court, moins un autre programme qui occupe le GPU le retarde.

Pour chaque image, le GPU faisait :

- une passe de conversion par couche (trame HAP vers texture de couche 1080p en 16 bits flottants) ;
- une passe de fond noir ;
- une passe de mixage par couche, qui relisait le résultat des couches inférieures et réécrivait une texture 1080p complète ;
- une copie du résultat dans l'image programme.

L'outil `gpu-bench` mesure ce travail : 3,66 ms de GPU par image pour 4 couches HAP 1080p sur Apple M1.

## Options

1. **Mixer plusieurs couches par passe.** Le shader lit les textures de plusieurs couches et garde le résultat dans ses registres d'une couche à l'autre. Les textures de couche restent : les effets de la phase 2 en auront besoin.
2. **Fusionner la conversion et le mixage.** Le shader de mixage lirait directement les textures HAP compressées, sans textures de couche. Le gain est plus grand, mais les effets par couche de la phase 2 demanderont de toute façon une texture de couche.
3. **Textures de couche en 8 bits** au lieu de 16 bits flottants : deux fois moins d'octets, mais au prix de la précision des couleurs prémultipliées sombres et translucides.

## Décision

- Option 1, avec **quatre couches par passe**. La passe lit les textures des 4 couches et de leurs clips précédents pendant un fondu, soit 8 textures de couche, et garde le mixage en 32 bits flottants dans les registres. Elle n'écrit le résultat qu'une fois, en `Rgba16Float`.
- La première passe part du **noir opaque** dans le shader : plus de passe de fond.
- La dernière passe écrit **directement dans la destination**. Le moteur mixe dans l'image programme, sans copie. Les lecteurs (aperçu, sorties) voient toujours une image complète : le GPU exécute le travail dans l'ordre des soumissions.
- Au-delà de 4 couches, les passes s'enchaînent sur deux textures intermédiaires, créées seulement à ce moment-là.
- Les formules, l'espace de mixage et la référence CPU de l'ADR 0004 ne changent pas. Les tests comparent toujours le GPU à la référence, et un test couvre le passage d'une passe à la suivante.

## Mesures

Sur Apple M1, avec `gpu-bench` et des couches HAP 1080p de bruit :

| Couches | Avant | Après |
| --- | --- | --- |
| 4 | 3,66 ms par image | 2,19 ms par image |
| 8 | 6,91 ms par image | 5,62 ms par image |

Moteur sur 60 s (`engine-run`, 4 couches) : 2,05 ms de travail par image sur le processeur, verdict FLUIDE. Empreinte mémoire : 190 Mo au lieu de 232 Mo, sans les deux textures intermédiaires.

## Conséquences

- Le résultat est un peu plus précis qu'avant, puisqu'il n'est plus arrondi en 16 bits entre deux couches.
- Les passes de conversion pèsent maintenant le plus : à 8 couches, elles expliquent l'essentiel du temps restant. L'option 2 ou 3 reste possible plus tard, si les mesures le demandent.
- Le travail du moteur sur le GPU est plus court, mais un blocage du GPU par un autre programme reste possible : ce changement réduit l'exposition, il ne la supprime pas.
