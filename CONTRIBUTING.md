# Contribuer à Prism

Merci de votre intérêt. Prism est développé en **clean room** : lisez [`docs/CLEAN_ROOM.md`](docs/CLEAN_ROOM.md) avant toute contribution.

## Règles clean room

- N'utilisez que des sources publiques et autorisées (section 3 de `docs/CLEAN_ROOM.md`).
- N'apportez aucun code, binaire, fichier de projet, preset, capture, icône, police ou média de Resolume.
- Si vous avez eu accès au code source, aux binaires désassemblés ou aux formats internes de Resolume, ne contribuez pas au code.
- Inscrivez toute nouvelle source consultée dans `docs/SOURCES.md`.

## Signature DCO

Chaque commit doit porter une ligne `Signed-off-by`, ajoutée par :

```bash
git commit -s
```

Cette ligne certifie que vous acceptez le [Developer Certificate of Origin 1.1](https://developercertificate.org) : vous avez le droit de soumettre ce code sous la licence du projet. La CI refuse les pull requests dont un commit n'est pas signé.

## Cycle d'une contribution

1. Choisissez ou ouvrez un ticket avec un critère d'acceptation clair.
2. Créez une branche `feat/<n°>-<sujet>` ou `fix/<n°>-<sujet>`.
3. Faites de petits commits ; chacun compile et passe les tests.
4. Vérifiez en local :
   ```bash
   cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && scripts/check-clean-room.sh
   ```
5. Mettez à jour `CHANGELOG.md`.
6. Ouvrez une pull request et remplissez le modèle, déclaration clean room comprise.

## Conventions

- Code, identifiants et commentaires en anglais ; docs, tickets et messages de commit en français.
- `rustfmt` par défaut, `clippy` sans avertissement.

## Licence

Sauf mention contraire, toute contribution soumise pour inclusion dans Prism est placée sous la double licence MIT / Apache 2.0, sans condition supplémentaire.
