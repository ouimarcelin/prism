# 0011 – Finitions de la phase 1

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Avant de valider la phase 1, plusieurs manques gênaient un usage en spectacle : Cmd+Q fermait l'application sans proposer d'enregistrer, l'écran pouvait se mettre en veille pendant la sortie, l'interface consommait beaucoup au repos, rien n'était retenu d'une session à l'autre.

## Décisions

- **Cmd+Q.** Le menu macOS par défaut de winit appelle directement la fermeture du système, sans possibilité de l'annuler. Il est désactivé : Cmd+Q arrive dans l'interface, qui demande d'enregistrer. Contrepartie : plus de menu Prism standard (Masquer, Cmd+H). Quitter depuis le Dock ferme encore sans confirmation.
- **Veille de l'écran.** Tant que la sortie est ouverte, l'outil système `caffeinate` (macOS) empêche la mise en veille ; il s'arrête avec la sortie ou avec Prism. Windows demandera son équivalent.
- **Préférences** dans un fichier JSON du dossier de l'utilisateur (`~/Library/Application Support/Prism/preferences.json` sur macOS) : projets récents (8), nom de l'écran de la dernière sortie, durée du fondu automatique. Un fichier absent ou illisible donne les valeurs par défaut.
- **Crossfader automatique** vers A ou B, animé par l'interface ; toucher le crossfader l'interrompt. L'interface se redessine alors au rythme de l'écran.
- **Consommation.** L'interface se redessine au rythme de l'écran pendant 1,5 s après une action, puis 30 fois par seconde : l'aperçu et les têtes de lecture restent vivants, la sortie n'est pas concernée (thread de rendu).

## Conséquences

- Le crossfader automatique dépend du thread de l'interface : une interface bloquée (boîte de dialogue) le suspend. Le jour où le tempo arrive (phase 2), le moteur pourra porter ce mouvement.
