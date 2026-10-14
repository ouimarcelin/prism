# 0010 – H.264 et ProRes par AVFoundation

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

La phase 1 demande la lecture H.264 ; le cahier des charges cite aussi ProRes. L'ADR 0003 a écarté FFmpeg (licence GPL de la version Homebrew, décodage sur le processeur) au profit des décodeurs du système. Sur macOS, AVFoundation et VideoToolbox décodent H.264, HEVC et ProRes, avec l'accélération matérielle.

## Options

1. **VideoToolbox directement** (`VTDecompressionSession`) en lui donnant les paquets lus par notre propre lecteur de conteneur. Contrôle total, mais il faut extraire les paramètres du flux (SPS, PPS), gérer l'ordre des images B et la mise en file : beaucoup de code délicat.
2. **AVFoundation** (`AVAssetReader`) : il lit le conteneur, gère l'ordre d'affichage et utilise VideoToolbox. Il décode vers l'avant à partir d'un instant : un saut en arrière impose de relancer le lecteur depuis l'image clé précédente.

## Décision

Option 2, dans le module `avf` de `prism-media` (macOS seulement), derrière l'interface `FrameSource` (ADR 0009) :

- notre lecteur de conteneur reconnaît HAP ; tout autre codec est confié à AVFoundation ;
- lecture enchaînée tant que l'image voulue est devant, à 45 images au plus ; sinon relance du lecteur juste avant l'image voulue ;
- sortie en YUV 4:2:0 (format natif du décodeur, 3 Mo par image 1080p), convertie en couleurs sur le GPU avec la matrice indiquée par l'image (BT.601 ou BT.709, sinon d'après la hauteur) ; les codecs avec alpha (ProRes 4444) sortent en BGRA, alpha non prémultiplié (vérifié par un test) ;
- le thread de décodage ne décode jamais deux fois une image déjà livrée : une demande répétée renverrait AVFoundation à l'image clé ;
- seul module avec du code `unsafe` (appels aux frameworks Apple via les crates `objc2`, licences MIT, Apache 2.0 ou Zlib), chaque bloc justifié ; une nouvelle tentative couvre un démarrage raté du décodeur système.

## Conséquences

- Mesure sur Apple M1 : 4 couches H.264 1080p60 sans image en retard ; le décodeur matériel tient environ 620 images/s au total.
- Le scratch et la lecture arrière restent lents en H.264 (une relance par image) : HAP reste le format recommandé pour ces usages.
- Windows demandera son propre décodeur (Media Foundation) derrière la même interface.
