# 0039 – Installeurs

- Date : 2026-10-18
- Statut : Acceptée (prise en mode autonome ; signature à faire quand les comptes existent)

## Contexte

La phase 6 demande des « installeurs signés », et le cahier des charges une application macOS « signée et notarisée » pour chaque version (section « Versions »). Il n'y a encore ni compte Apple Developer (99 $ par an) ni certificat de signature Windows.

## Décision

- **macOS** : `scripts/make-dmg.sh [--ndi] [--universal]` construit `Prism.app` (même script que `make-app.sh`, sans l'installer), la signe, puis l'image disque `target/dist/Prism-<version>.dmg` (application, lien vers `/Applications`, licences, `NOTICE`).
  - Avec `PRISM_SIGN_IDENTITY` (Developer ID Application) : signature avec runtime renforcé et horodatage, droits `installer/macos/entitlements.plist` (caméra, entrée audio, bibliothèque NDI signée par son éditeur) ; l'image est signée aussi.
  - Avec `PRISM_NOTARY_PROFILE` en plus : notarisation (`notarytool`, attente du résultat) et agrafage du ticket.
  - Sans identité : signature ad hoc (obligatoire sur Apple Silicon) ; Gatekeeper demande un clic droit > Ouvrir.
  - `--universal` : Apple Silicon et Intel dans un seul binaire (`lipo`).
- **Windows** : `scripts/make-installer.ps1 [-Ndi]` construit `prism.exe`, fait l'icône `.ico` depuis le logo, signe avec `signtool` si `PRISM_SIGN_CERT` est défini, puis produit `target/dist/Prism-<version>-setup.exe` avec Inno Setup 6 (`installer/windows/prism.iss` : installation par utilisateur ou machine, raccourcis, désinstallation, français et anglais).
- **Publication** : le workflow `release.yml` se lance sur une étiquette `v*`, fabrique les deux installeurs (signés si les secrets existent) et les joint à une version **brouillon** de GitHub ; la publication reste manuelle.

## Conséquences

- Tant que les comptes manquent, les installeurs ne sont pas signés : avertissements de Gatekeeper et de SmartScreen.
- Secrets à créer le moment venu : `MACOS_CERTIFICATE` (p12 en base64), `MACOS_CERTIFICATE_PASSWORD`, `MACOS_SIGN_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`, `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`.
- L'image disque a été construite et vérifiée sur le Mac de développement (signature ad hoc) ; l'installeur Windows n'a pas été construit (pas de PC).
