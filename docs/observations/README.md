# Observations

Une observation décrit, **avec nos propres mots**, un comportement constaté en utilisant un logiciel de VJing existant. C'est le seul canal par lequel une information de comportement parvient à l'implémenteur (voir `docs/CLEAN_ROOM.md`).

## Règles

- Un fichier par sujet : `NNN-sujet.md` (exemple : `004-declenchement-colonne.md`).
- Décrire ce que fait l'utilisateur et ce qui se passe, pas comment l'écran est dessiné.
- Aucune capture, vidéo, icône, extrait de manuel ni copier-coller de texte de l'interface.
- Pas de nom de menu ou de libellé recopié : reformuler.
- Une observation peut être fausse ou incomplète : c'est l'intention fonctionnelle qui compte, Prism peut faire mieux.

## Modèle

Copier le bloc suivant dans un nouveau fichier.

```markdown
# NNN – Sujet

- Date : AAAA-MM-JJ
- Observateur : @pseudo
- Contexte : logiciel observé en usage normal (version d'essai ou licence), version, système

## Comportement

Ce que fait l'utilisateur, étape par étape, et ce qu'il constate.

## Cas limites

Ce qui se passe dans les situations particulières (liste vide, valeur maximale, double clic…).

## Ce que Prism doit faire

Comportement attendu dans Prism, éventuellement différent ou amélioré.
```
