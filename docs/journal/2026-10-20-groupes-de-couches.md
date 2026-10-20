# 2026-10-20 – Tickets 86 à 89 : démarrage, curseurs, groupes de couches

- Rôles : @ouimarcelin observateur, Claude Code implémenteur
- Transcription : `2026-10-20-9233d07a-0995-4568-8e6b-4782ab963c3a.jsonl`, sha256 `17e80530fd5b97a4d1ae663c1fa78b1b51beb528604c0f7d9f41a4d118b2b2bc`

## Prompts

- « augmente grave la valeur du ease-in, supprimes l'animation scale du logo »
- Maquettes de curseur (deux images), « intègre cette nouvelle ui de parametre glissière », puis « tu n'as pas compris, voici le parametre glissière à intégrer… plusieurs états de la glissière »
- « listes moi les futures implémentations de features pour le logiciel. », puis « vas y » (groupes de couches)

## Résultat

- Ticket 86 : démarrage en 2 s, ease-in marqué (sinusoïde de t³), logo fixe.
- Tickets 87 et 88 : curseurs à piste arrondie et poignée en capsule blanche logée au bout du remplissage (ADR 0041 révisée).
- Ticket 89 : groupes de couches en sous-mixage (ADR 0043) : modèle, rendu, contrôle, interface, tests du modèle et test GPU.

## Problèmes

- Ticket 87 : maquette mal lue (capsule prise pour une marque d'origine) ; corrigé au ticket 88 avec la maquette des états.
- Ticket 88 : un commit parti sans `cargo fmt` (chaîne de commandes avec `;`) ; corrigé dans un second commit.
- Disque plein (1 Gio libre) : l'édition de liens a écrit un binaire tronqué de 3 Ko, installé dans Prism.app, qui ne s'ouvrait plus. `cargo clean --profile dev` (43 Gio), nouvelle édition de liens forcée.
