# Task 8: Caustics and Light Source Size

## Observations

> Grande lumière = moins de bruits, petite lumière beaucoup de bruits.
> Une lumière plus grande occupe un plus grand angle solide vue depuis la caméra. Donc plus d'échantillons valides (pas noirs).
> [Wikipedia: Caustique](https://fr.wikipedia.org/wiki/Caustique)
> Les rayons caustiques passent probablement par plusieurs rebonds avant d'atteindre la caméra. Si la source lumineuse est petite, l'ensemble des rayons qui après réfraction/réflexion atteignent la lumière doit être petit, alors les contributions sont rares et de forte intensité = haute variance = bruit.
>
> Moins de lumière crée moins d'informations alors plus de rayon "noirs" sont reçues dans la caméra.

## Visual Comparison

### Grande lumière
![Caustique grande lumière](06_caustic_big.png)

### Petite lumière
![Caustique petite lumière](06_caustic_small.png)

### Référence
![Caustique référence](06_caustic.png)

## Analyse

- **Grande lumière** : Plus de rayons atteignent la lumière, donc moins de bruit dans l'image.
- **Petite lumière** : Moins de rayons valides, donc plus de bruit (haute variance).
- **Caustiques** : Les motifs lumineux sont dus à la concentration des rayons après réflexion/réfraction, et sont plus nets avec une source lumineuse plus grande.

# Bonus

## Flou de profondeur
### Référence
![Ref](example_scene2-spp128.png)
### Flou
![Flou](example_scene2_defocus.png)

## Fresnel pour les métaux
### Référence
![Ref](example_scene3-spp128.png)
### Fresnel
![Fresnel metaux](example_scene4.png)

## Carte d'environnement
### Référence
![Fresnel](example_scene4.png)
### Satara
![Satara](example_scene5_satara.png)
### Qwantani
![Qwantani](example_scene5_qwantani.png)

