# 0037 – Portage Windows

- Date : 2026-10-18
- Statut : Acceptée (prise en mode autonome, à revoir sur demande ; non essayée sur un PC Windows)

## Contexte

La phase 6 demande le « portage Windows (Spout, cartes de capture, DirectX 12 via wgpu) » (`docs/SPEC.md`). Le code isole déjà ce qui est propre à macOS derrière `cfg(target_os = "macos")`. Aucun PC Windows n'est disponible ; la CI GitHub (tâche `rust (Windows)`) est bloquée par la facturation du compte au moment du ticket.

## Décisions

- **Rendu** : wgpu choisit DirectX 12 (ou Vulkan) sous Windows avec `Backends::PRIMARY`, sans code à part.
- **Vérification** : `cargo clippy --target x86_64-pc-windows-msvc` en local (vérification des types, sans édition des liens ni tests), et tâche `rust (Windows)` de la CI, non bloquante tant que le portage n'a pas été essayé.
- **Thread de rendu** : priorité `THREAD_PRIORITY_TIME_CRITICAL` et résolution du minuteur système à 1 ms (`timeBeginPeriod`), sans quoi une attente dure jusqu'à 15,6 ms ; threads de décodage en `ABOVE_NORMAL` (`windows-sys`).
- **Écran allumé** : `SetThreadExecutionState` sur le thread principal pendant les sorties plein écran (équivalent de `caffeinate` sur macOS).
- **Spout** (équivalent de Syphon) :
  - sortie de l'image programme (préférence « Publier sur Spout », la même que « Publier sur Syphon ») et des écrans de la scène (case « Spout ») ;
  - entrée « Spout… » dans le menu d'une case vide (`MediaSource::Spout`) ;
  - protocole du SDK Spout 2 (BSD 2 clauses) : liste `SpoutSenderNames` (256 octets par nom), mémoire partagée au nom de l'émetteur (`SharedTextureInfo`, 280 octets : poignée partagée, taille, format DXGI), mutex `<nom>_SpoutAccessMutex`, émetteur actif `ActiveSenderName` ;
  - texture Direct3D 11 partagée BGRA 8 bits ; l'image passe par le processeur (lecture de l'image programme déjà utilisée par NDI, séparée dans `readback.rs`), car wgpu ne partage pas ses textures avec Direct3D 11 sans interopérabilité native ;
  - un thread par émetteur et par récepteur.
- **NDI** sous Windows : le moteur dépend maintenant de `prism-io-video` partout ; la bibliothèque `Processing.NDI.Lib.x64.dll` est cherchée via `NDI_RUNTIME_DIR_V6`.

## Limites (tickets à part)

*Vidéos, son et caméras : faits au ticket 73 (ADR 0038).*

- **Vidéos autres que HAP** (H.264, ProRes) et **son des clips** : décodés par AVFoundation sur macOS ; sous Windows, refusés avec un message. HAP, images fixes et séquences PNG marchent. Piste : Media Foundation.
- **Caméras** : AVFoundation sur macOS ; sous Windows, « non prises en charge sur ce système ». Piste : Media Foundation.
- **Cartes de capture** : sur macOS, une carte dont le pilote expose une caméra système apparaît dans « Caméra… » (à confirmer par une observation avec une carte réelle). Les SDK DeckLink (Blackmagic) et NTV2 (AJA) demandent chacun une intégration propre ; la licence du SDK DeckLink n'est pas compatible avec MIT/Apache 2.0 pour reprendre ses en-têtes, il faudrait des déclarations écrites à partir de sa documentation, chargées à l'exécution comme NDI.
- Spout : partage par le processeur (une copie de plus qu'un partage GPU direct) ; compteur d'images (`<nom>_Count_Semaphore`) non utilisé.

## Conséquences

- Rien n'a tourné sous Windows : la première exécution sur un PC est le vrai test (lancement, rendu DX12, Spout avec un autre logiciel, NDI).
