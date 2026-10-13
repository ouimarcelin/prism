# 0001 – Processus clean room

- Date : 2026-10-13
- Statut : Acceptée

## Contexte

Prism reprend le flux de travail d'un logiciel commercial. Le projet doit pouvoir démontrer qu'il n'a réutilisé ni code, ni visuel, ni format propriétaire de ce logiciel. Le mainteneur travaille seul avec Claude Code.

## Options

1. **Règles écrites seulement**, sans preuve particulière. Simple, mais rien ne démontre l'indépendance en cas de litige.
2. **Clean room à deux rôles avec preuves conservées.** L'observateur décrit les comportements ; l'implémenteur code uniquement à partir de ces descriptions ; chaque étape laisse une trace datée. Demande un peu de discipline à chaque session.

## Décision

Option 2, décrite dans `docs/CLEAN_ROOM.md` :

- le mainteneur est l'observateur, Claude Code et les contributeurs sont les implémenteurs ;
- registre des sources (`docs/SOURCES.md`), observations écrites (`docs/observations/`), journal de session avec empreinte des transcriptions archivées ;
- DCO obligatoire, déclaration clean room dans chaque pull request ;
- contrôles automatiques dans la CI (termes interdits, DCO, licences des dépendances) ;
- dépôt à date certaine (e-Soleau ou APP) à chaque fin de phase.

## Conséquences

- Claude Code ne consulte jamais le manuel ni le site de Resolume, même si c'est autorisé pour l'observateur.
- Les pull requests sans DCO ou sans déclaration clean room sont refusées.
- Chaque fin de phase comprend un dépôt d'archive auprès d'un tiers horodateur.
