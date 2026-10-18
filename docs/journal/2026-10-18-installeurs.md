# 2026-10-18 – Ticket 74 : installeurs

- Rôles : @ouimarcelin observateur, Claude Code implémenteur

## Prompt

« complètes la phase 6 en autonomie. Je ferais les tests à la fin. »

## Plan

- Image disque macOS signée et notarisée quand un Developer ID existe, signée ad hoc sinon.
- Installeur Windows (Inno Setup) avec signature optionnelle.
- Workflow de version vers un brouillon GitHub, sans publication automatique.

## Résultat

- `scripts/make-dmg.sh`, `installer/macos/entitlements.plist`, `scripts/make-installer.ps1`, `installer/windows/prism.iss`, `.github/workflows/release.yml` ; `make-app.sh` accepte `PRISM_NO_INSTALL` et `PRISM_TARGETS` (binaire universel).
- Essai local : `scripts/make-dmg.sh --ndi` donne une image de 8,6 Mo ; contenu et signature ad hoc vérifiés après montage. Image effacée ensuite.
- Non essayé : signature Developer ID et notarisation (pas de compte), installeur Windows (pas de PC), workflow de version (aucune étiquette poussée ; CI arrêtée par la facturation).

## Problèmes

- Les secrets ne peuvent pas servir dans une condition `if:` d'étape : passés par l'environnement de la tâche.
- `scripts/make-icon.swift` dessinait l'ancienne icône : son en-tête dit maintenant de ne jamais écrire sur le logo de l'auteur.
