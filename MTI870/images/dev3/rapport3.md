# Rapport devoir 3

## Intersection Rayon Triangle

|     Triangles      |     Cornellbox      |           Cornellbox MIS           |
| :----------------: | :-----------------: | :--------------------------------: |
| ![](triangles.png) | ![](cornellbox.png) | ![](cornell_path-mis_refactor.png) |

## Sampling

|         Uniforme         |                  Ref Uniforme                  |
| :----------------------: | :--------------------------------------------: |
| ![](test_sphere-pdf.png) | ![](../../scenes/devoir3/tests/sphere-pdf.png) |
|    ![](group-pdf.png)    | ![](../../scenes/devoir3/tests/group-pdf.png)  |

## Odyssey_mats Direct Strategies

|            BSDF            |            Emitter            |            Naive            |            MIS            |
| :------------------------: | :---------------------------: | :-------------------------: | :-----------------------: |
| ![](odyssey_mats_bsdf.png) | ![](odyssey_mats_emitter.png) | ![](odyssey_mats_naive.png) | ![](odyssey_mats_mis.png) |

## Odyssey_triangle_mats Direct Strategies

|                BSDF                 |                Emitter                 |                Naive                 |                MIS                 |
| :---------------------------------: | :------------------------------------: | :----------------------------------: | :--------------------------------: |
| ![](odyssey_triangle_mats_bsdf.png) | ![](odyssey_triangle_mats_emitter.png) | ![](odyssey_triangle_mats_naive.png) | ![](odyssey_triangle_mats_mis.png) |

## All mats Direct Strategies

|         BSDF          |         Emitter          |         Naive          |         MIS          |
| :-------------------: | :----------------------: | :--------------------: | :------------------: |
| ![](allmats_bsdf.png) | ![](allmats_emitter.png) | ![](allmats_naive.png) | ![](allmats_mis.png) |

## All mats Path vs Path MIS vs Bonus Path MIS

|       Path       |         Path-MIS (20 sec)          |     BONUS Path-MIS (14 sec)     |
| :--------------: | :--------------------------------: | :-----------------------------: |
| ![](allmats.png) | ![](allmats_path-mis_refactor.png) | ![](allmats_path-mis_bonus.png) |

## Veach Direct MIS

|    Veach N=512     |                  Ref Veach                  |
| :----------------: | :-----------------------------------------: |
| ![](veach-mis.png) | ![](../../scenes/devoir3/ref-veach-mis.png) |

## Bonus

### BONUS Path-MIS (14 sec)

![](allmats_path-mis_bonus.png)

### BONUS Amélioration de l'échantillonnage de la sphère

|          Solid Angle           |                  Ref Solid Angle                  |
| :----------------------------: | :-----------------------------------------------: |
| ![](test_sphere_solid-pdf.png) | ![](../../scenes/devoir3/tests/sphere-pdf-SA.png) |

#### using Direct Emitter

|        |               Uniforme               |             Solid Angle             |
| :----: | :----------------------------------: | :---------------------------------: |
| small  | ![](sphere_small_sampling_false.png) | ![](sphere_small_sampling_true.png) |
| medium |  ![](sphere_med_sampling_false.png)  |  ![](sphere_med_sampling_true.png)  |
|  big   | ![](sphere_large_sampling_false.png) | ![](sphere_large_sampling_true.png) |
