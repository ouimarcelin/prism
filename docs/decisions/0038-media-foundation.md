# 0038 – Media Foundation (Windows)

- Date : 2026-10-18
- Statut : Acceptée (prise en mode autonome ; non essayée sur un PC Windows)

## Contexte

Sous macOS, AVFoundation décode les vidéos autres que HAP (H.264, ProRes…), leur son, et capture les caméras (ADR 0023). Après le ticket 72, Windows ne lisait que le HAP, les images et les séquences PNG, sans caméra (ADR 0037).

## Décision

- **Media Foundation**, inclus dans Windows (aucune bibliothèque à installer), par le crate `windows` 0.62 : un lecteur de source (`IMFSourceReader`) décode et convertit.
- **Vidéo** : sortie demandée en BGRX 32 bits (`MFVideoFormat_RGB32`, conversion par le lecteur, décodage matériel permis), copiée en BGRA opaque ligne par ligne, retournée si les lignes sont rangées du bas vers le haut (pas négatif). Nombre d'images et durée lus dans le conteneur, comme sur macOS ; déplacement au-delà de 30 images d'avance ou en arrière (`SetCurrentPosition`, en centaines de nanosecondes).
- **Son des clips** : PCM 16 bits stéréo à 48 kHz demandé au lecteur, qui rééchantillonne.
- **Caméras et cartes de capture** : périphériques de capture vidéo de Media Foundation (`MFEnumDeviceSources`), identifiés par leur lien symbolique, nommés par leur nom courant ; une carte de capture dont le pilote expose un périphérique vidéo Windows apparaît dans « Caméra… ». Capture sur un thread, l'image la plus récente gardée, comme sur macOS.
- COM en appartement multithread sur chaque thread qui utilise Media Foundation ; Media Foundation démarré une fois par processus.

## Conséquences

- Codecs : ceux de Windows (H.264, HEVC si l'extension est installée, MPEG-4…). ProRes n'est pas fourni par Windows.
- La conversion en BGRX passe par le processeur du lecteur de source : plus lent que le chemin YUV de macOS ; piste si besoin : demander NV12 et convertir sur le GPU.
