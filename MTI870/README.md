# MTI870 - Automne 2024

Code de départ pour le cours MTI870: rendu photoréaliste à l'ÉTS.
Ce code de départ sera mis à jour à chaque devoir, en rajoutant les fonctionnalités nécessaires. Pour cloner ce dépôt, il faut impérativement utiliser la commande suivante:

```
git clone <URL-du-depot>
```

Les instructions pour compiler ce code de base sont fournies dans le devoir 0. Ce premier devoir contient des informations pour configurer correctement votre dépôt.

## Objectifs

Les objectifs de ce code de base est de vous fournir un code plus avancé que la série de livres de Peter Shirley ["Ray tracing in one weekend"](https://raytracing.github.io/), tout en vous permettant d'évoluer vers un système de rendu moderne de type Mitsuba ou PBRT.

Pour ce faire, ce code de base vous fournir des fonctionnalités qui serait compliquée a mettre en place par vous-même ou qui prendrait du temps sans grande valeur pédagogique. Ces fonctionnalités sont:

- Le chargement de scène 3D basée sur la syntaxe Json
- La lecture et écriture d'images 2D (p. ex. OpenEXR, png, jpg)
- Le chargement de géométrie 3D (p. ex. fichiers obj)
- Les opérations matricielles et vectorielles

## Organisation du code

- `examples/`: dossier contenant les codes source spécifiques à chaque devoir. Chaque fichier source produit un exécutable distinct. Vous devrez modifier ces codes sources pour chaque devoir.
- `src/`: dossier contenant l'implémentation du moteur de rendu.
- `instructions/`: dossier contenant les instructions pour les devoirs.
- `scenes/`: dossier contenant les images, scènes ou autres ressources nécessaire pour les devoirs. **Ce dossier contient les images de references vous permettant de verifier que vous avez la bonne implementation.**

## Dépendances

Construire un moteur de rendu nécessite beaucoup de code qui serait très difficile de produire soit même. C'est pour cette raison que ce code de base se base sur de nombreuses dépendances. Voici la liste des dépendances du projet sont visible dans le `Cargo.toml`. Pour plus d'information sur ces dépendences: https://crates.io/. Vous pouvez trouver la documentation avec: `cargo doc --open`.

## Acknowledgement

Ce code s'inspire fortement de l'excellent code **darts**, le moteur de rendu minimaliste utilisé dans le cours CS 87/287 a Dartmouth et enseigné par Prof. Wojciech Jarosz. Il y a aussi des inspirations de **Nori**, moteur de rendu utilisé à l'EPFL pour les cours sur le rendu photoréaliste.

En règle général, le code s'inspire aussi de **PBRT**, de **Mitsuba** et de **Rustlight** avec lequel j'effectue ma recherche en rendu photoréaliste.
