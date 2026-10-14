# 0012 – Cadence du thread de rendu : temps réel et tampons d'envoi réutilisés

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Le critère de la phase 1 demande 4 clips HAP 1080p mixés à 60 images/s sans saccade pendant 30 minutes. Le premier essai d'endurance (`engine-run`, 4 clips HAP et HAP Q de bruit, Apple M1 8 Go) a tenu 60 images/s en moyenne, mais avec 5 saccades, dont une de 139 ms. Le temps de travail de cette image était aussi de 139 ms : le thread de rendu s'est bloqué pendant qu'il préparait l'image, ce n'était pas un réveil en retard.

L'enquête a relevé deux causes possibles.

- **Allocations à chaque image.** Dans wgpu 30, `Queue::write_texture` crée à chaque appel un tampon Metal neuf, de la taille de la trame. Avec 4 couches HAP 1080p, cela fait environ 6 Mo de mémoire fraîche par image, soit 360 Mo par seconde. Quand la machine manque de mémoire (2,1 Go de swap pendant les mesures, 60 Mo de mémoire libre), obtenir ces pages peut bloquer le thread. Sur 60 s, la pire durée d'envoi au GPU a atteint 15 ms.
- **Réveils en retard.** Lancé depuis Claude Code, `engine-run` tourne avec une priorité réduite (`nice 5`), et la classe « interface utilisateur » demandée pour le thread de rendu n'apparaît pas dans sa priorité (31, celle de tous les threads). Le thread se réveillait jusqu'à 10 ms trop tard : le pire écart entre deux images allait de 23 à 27 ms selon les essais, au lieu de 16,7 ms.

## Décisions

- **Tampons d'envoi réutilisés.** Les trames passent par une `StagingBelt` de wgpu. Ce sont des tampons de 8 Mo, gardés d'une image à l'autre, qui reviennent au moteur quand le GPU a fini de les lire. Une trame plus grande reçoit un tampon à sa taille, lui aussi gardé. La copie vers la texture est enregistrée dans l'encodeur de l'image, juste avant la passe de conversion. Avant les envois, le moteur interroge le GPU sans attendre, pour récupérer plus tôt les tampons libres. Les paramètres de conversion et de couche ne sont écrits que s'ils changent. Une fois la lecture lancée, le thread de rendu n'alloue plus de mémoire GPU.
- **Thread de rendu temps réel sur macOS.** Le thread demande la politique d'ordonnancement « time constraint » de Mach, celle des threads audio et d'affichage. Il déclare sa période (l'inverse de la fréquence de la composition), un calcul d'au plus un tiers de la période, terminé avant la moitié de la période, et il reste préemptible. Le système le réveille alors à l'heure même quand d'autres processus occupent les cœurs. La politique est réappliquée quand la fréquence change. Si le système la refuse, le thread garde la classe « interface utilisateur ». L'état publié du moteur indique si le temps réel est actif.
- **Au plus 3 images en attente sur le GPU.** Le moteur compte les images envoyées et celles que le GPU a terminées (rappel de fin de travail de wgpu). Si 3 images attendent encore, il n'envoie rien pendant ce tour : l'image précédente reste à l'écran, et l'image suivante compte comme une saccade. Sans cette limite, un GPU arrêté un instant (par un autre programme, par exemple) laisse le moteur empiler des images, et chacune garde un tampon d'envoi. Le moteur mesure aussi le plus long délai entre l'envoi d'une image et la fin de son travail.
- **Mesure par étape.** Le moteur mesure chaque étape d'une image : lecture, envoi au GPU, mixage et sorties. Il garde le pire de chacune et décrit la dernière saccade, avec les étapes de l'image d'avant. `engine-run` note chaque incident avec la pagination de la machine sur la seconde écoulée.

## Options écartées

- **Thread d'envoi séparé**, qui copierait les trames vers le GPU hors du thread de rendu. Plus complexe : il faudrait un anneau de textures par couche et une synchronisation avec le mixage. Il n'est pas nécessaire tant que les envois ne bloquent plus.
- **Attente active** avant l'échéance de chaque image. Essayée pendant le ticket 11, elle occupe un cœur entier pour gagner quelques millisecondes. Elle a été retirée.

## Conséquences

- Les tampons d'envoi ne sont jamais rendus au système : leur nombre reste celui du pire moment. Sans limite d'images en attente, une endurance de 30 minutes est passée de 11 à 13 tampons GPU (+24 Mo) vers la dixième minute, sans aucune saccade vue par le processeur : le GPU avait pris plusieurs images de retard. Avec la limite, le nombre de tampons d'envoi est borné : au plus 3 images en attente et celle en préparation. Sur une nouvelle endurance de 30 minutes, le nombre de tampons GPU est resté à 11 du début à la fin.
- Un retard du GPU de plus de 3 images devient visible dans les mesures (images sautées, saccade) au lieu de passer inaperçu.
- Un thread temps réel qui dépasse longtemps le calcul déclaré est rétrogradé par macOS pendant un moment. C'est un garde-fou, pas une panne : il redevient un thread ordinaire.
- Windows n'a pas encore d'équivalent. Son tour viendra avec le portage : MMCSS, via `AvSetMmThreadCharacteristics`.
- Une mesure lancée depuis Claude Code (`nice 5`) n'est pas représentative : la mesure de référence se lance depuis le Terminal, les autres applications fermées.
- Le moteur garde sa propre horloge. La synchronisation avec le rafraîchissement de l'écran de sortie reste à étudier, avec un vrai écran ou projecteur.
