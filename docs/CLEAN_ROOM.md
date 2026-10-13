# Politique et attestation clean room

Ce document décrit comment Prism est développé sans reprendre le travail protégé de Resolume, et quelles preuves sont conservées pour le démontrer. Il fait partie du dépôt : chaque modification est datée et attribuée par git.

> Ce document n'est pas un avis juridique. Avant toute diffusion publique ou commerciale, le projet doit être relu par un avocat en propriété intellectuelle (voir `docs/SPEC.md`, section « Points ouverts »). Une démarche clean room ne protège pas contre les brevets.

## 1. Principe

Prism reproduit un **flux de travail** (grille de clips, couches, colonnes, crossfader, sortie avancée), pas une œuvre. Les idées et comportements fonctionnels ne sont pas protégés par le droit d'auteur ; le code, l'interface graphique, les textes, les icônes et la marque le sont. Le projet n'utilise donc que des sources publiques et des descriptions de comportement rédigées avec nos propres mots.

## 2. Rôles

| Rôle | Qui | Peut consulter | Produit |
| --- | --- | --- | --- |
| **Observateur** | Le mainteneur (@ouimarcelin) | Le logiciel Resolume en usage normal (essai ou licence), son manuel et sa page produit publics | `docs/SPEC.md`, `docs/observations/*.md` : descriptions de comportement, sans capture ni extrait |
| **Implémenteur** | Claude Code et tout contributeur de code | Uniquement les sources autorisées de la section 3 | Code, tests, ADR, journal |

L'implémenteur ne lit **jamais** le manuel, le site ou un fichier de Resolume, même si c'est techniquement autorisé pour l'observateur. Quand une information de comportement manque, il demande une observation écrite au lieu de chercher la réponse.

Limite connue : sur un projet mené seul, l'observateur dirige aussi l'implémenteur. Pour compenser, tous les prompts sont conservés dans le journal (section 5). Ils prouvent que seules des descriptions de comportement ont été transmises.

## 3. Sources

| Autorisé pour l'implémentation | Interdit à tous |
| --- | --- |
| `docs/SPEC.md`, `docs/observations/`, `docs/decisions/` | Décompiler, désassembler ou analyser les binaires de Resolume |
| Spécifications et code de formats ouverts sous licence compatible (HAP, PNG, ProRes, H.264 via FFmpeg…) | Lire ou importer les fichiers de projet `.avc`, les presets ou les fichiers de configuration de Resolume |
| Documentation des crates et bibliothèques utilisées | Implémenter le codec propriétaire DXV |
| Littérature technique générale (shaders, mixage, warping, edge blending) | Réutiliser le nom, les logos, les icônes, les polices, les couleurs de marque ou les captures |
| | Copier l'interface pixel pour pixel |

Toute source consultée, autorisée ou non, est inscrite dans `docs/SOURCES.md` dans le même commit que le travail qui en découle.

## 4. Procédure

1. L'observateur décrit un comportement dans `docs/observations/NNN-sujet.md` ou dans `docs/SPEC.md`.
2. Un ticket GitHub reprend ce comportement et fixe un critère d'acceptation.
3. Une session Claude Code est consacrée à ce seul ticket. Le prompt ne contient que le ticket et des descriptions de comportement.
4. Chaque commit porte une ligne `Signed-off-by` (DCO).
5. La pull request coche la déclaration clean room du modèle de PR ; la CI vérifie les termes interdits et la DCO.
6. La session est notée dans `docs/journal/`, avec l'empreinte de la transcription archivée.

## 5. Preuves conservées

| Preuve | Emplacement | Ce qu'elle démontre |
| --- | --- | --- |
| Historique git, branche `main` protégée | GitHub | Qui a écrit quoi, dans quel ordre, sans réécriture |
| Premier commit limité à `docs/SPEC.md` | Historique git | La spécification précède le code |
| Registre des sources | `docs/SOURCES.md` | Ce qui a été consulté, par qui et pourquoi |
| Observations | `docs/observations/` | Les comportements transmis, rédigés avec nos mots |
| Décisions techniques | `docs/decisions/` | Le raisonnement indépendant derrière chaque choix |
| Journal de session | `docs/journal/` | Le prompt exact et le résultat de chaque session |
| Transcriptions Claude Code | Archive hors dépôt, empreinte `sha256` dans le journal | Le contenu intégral des sessions, non modifié |
| Lignes `Signed-off-by` | Chaque commit | L'engagement DCO de chaque auteur |
| Déclarations de PR | Pull requests GitHub | L'engagement clean room de chaque contributeur |
| Dépôts à date certaine | Section 7 | L'antériorité, opposable à un tiers |

## 6. Contrôles automatiques

- `scripts/check-clean-room.sh` : échoue si un terme interdit apparaît dans le code ou les assets (seule exception : la mention de non-affiliation, marquée `clean-room-allow`), ou si un fichier `.avc` ou `.dxv` est versionné.
- Vérification DCO : chaque commit d'une pull request doit porter `Signed-off-by`.
- `cargo-deny` : licences des dépendances limitées à une liste compatible MIT/Apache 2.0, sources limitées à crates.io.

Les trois contrôles tournent dans la CI GitHub Actions (`.github/workflows/ci.yml`).

## 7. Date certaine

Les dates git peuvent être falsifiées. À chaque fin de phase, une archive du dépôt (`git bundle` ou `git archive` du tag de version) est déposée auprès d'un tiers horodateur :

- **Enveloppe e-Soleau** (INPI), conservée 5 ans, renouvelable ;
- ou **APP** (Agence pour la protection des programmes), adaptée au code source ;
- et, dès que le dépôt est public, archivage automatique par **Software Heritage**.

| Date | Version / tag | Organisme | Numéro de dépôt | Empreinte `sha256` de l'archive |
| --- | --- | --- | --- | --- |
| | | | | |

## 8. Contributions extérieures

Toute contribution exige :

- une ligne `Signed-off-by` sur chaque commit (`git commit -s`), qui vaut acceptation du [Developer Certificate of Origin 1.1](https://developercertificate.org) ;
- la case clean room cochée dans la pull request ;
- l'ajout dans `docs/SOURCES.md` de toute nouvelle source consultée.

Une personne qui a eu accès au code source, aux binaires désassemblés ou aux formats internes de Resolume ne contribue pas au code de Prism.

## 9. Incident de contamination

Si du contenu interdit a été consulté ou introduit :

1. arrêter le travail concerné ;
2. décrire l'incident dans `docs/clean-room/incidents/AAAA-MM-JJ-sujet.md` (quoi, quand, qui, quels fichiers) ;
3. retirer le code touché ; le faire réécrire par une autre personne ou une nouvelle session Claude Code qui n'a pas eu accès au contenu, à partir de la seule spécification ;
4. relier le commit de retrait et le commit de réécriture à la note d'incident.

## 10. Attestation

Chaque attestation est ajoutée par un commit au nom de son signataire. L'historique git en fixe la date et l'auteur.

> Je soussigné, mainteneur du projet Prism, atteste que :
>
> - je n'ai décompilé, désassemblé ni analysé aucun binaire de Resolume ;
> - je n'ai transmis à l'implémenteur aucun code, fichier, capture ou extrait de documentation de Resolume, seulement des descriptions de comportement rédigées avec mes propres mots ;
> - je n'ai versionné aucun média, preset, icône, police ou capture issu de Resolume ;
> - toutes les sources que j'ai consultées sont inscrites dans `docs/SOURCES.md`.

| Date | Signataire | Périmètre | Commit |
| --- | --- | --- | --- |
| | | | |
