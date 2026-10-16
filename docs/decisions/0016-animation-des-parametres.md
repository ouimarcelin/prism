# 0016 – Animation des paramètres

- Date : 2026-10-14
- Statut : Acceptée

## Contexte

Le cahier des charges demande que chaque paramètre soit animable (LFO, enveloppe, synchro BPM), et le critère de la phase 2 qu'un effet cadencé sur le BPM reste synchrone après 10 minutes.

## Décisions

- **Une animation par réglage d'effet**, dans l'effet lui-même (`Effect::animations`, par nom de paramètre). Tant qu'elle existe, elle remplace la valeur réglée, qui reste gardée pour quand l'animation s'arrête.
- **Forme** : sinus, triangle, rampe, carré, aléatoire (une nouvelle valeur à chaque cycle, tenue jusqu'au suivant). **Période** en temps, au tempo de la composition, ou en secondes. **Valeurs** basse et haute, dans les bornes du réglage. **Phase** pour décaler le cycle.
- **Valeur calculée, jamais accumulée** : elle ne dépend que de la position en temps de l'horloge (ADR 0014) ou du temps écoulé depuis le démarrage du moteur. Une image sautée ne la décale pas ; une animation en temps reste calée sur le tempo aussi longtemps que le tempo l'est. L'aléatoire est un hachage du numéro de cycle : le même cycle donne toujours la même valeur.
- Le calcul se fait au rendu, sur le thread de rendu, à chaque image, avant l'écriture des réglages des shaders. Les positions sont en `f64` ; les shaders reçoivent des `f32`.

## Options écartées

- **Animations dans une liste à part** (modulateurs reliés aux réglages) : plus général, utile pour un même LFO sur plusieurs réglages. Plus lourd à présenter ; possible plus tard sans changer le calcul.
- **Valeur accumulée image par image** : dérive et dépend de la cadence.

## Conséquences

- Le test automatique du critère de la phase 2 vérifie chaque image de la dixième minute à 128 BPM. Le test à la main reste à faire, sur une musique à tempo connu.
- Restent pour la suite : animer l'opacité des couches et les autres réglages qui ne sont pas des effets, enveloppes, tableau de 8 curseurs par clip, pilotage par l'audio (FFT).
- **Ajout du 2026-10-16 (ticket 19)** : l'opacité des couches s'anime comme un réglage d'effet (`Layer::opacity_animation`). Chaque clip a un tableau de bord de 8 curseurs ; un réglage d'effet du clip, de sa couche ou d'un générateur peut suivre un curseur (`Effect::links`), qui parcourt toute la plage du réglage. Ordre de priorité : animation, puis curseur, puis valeur réglée. Les effets de la composition n'ont pas de tableau de bord.
