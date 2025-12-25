# Guide de Développement - QuickFindr (Rust + Slint)

Ce guide détaille les phases d'implémentation pour garantir une application multi-threadée performante sous Windows 11.

## Phase 1 : Initialisation du Projet
1. Configurer `Cargo.toml` avec les dépendances : `slint`, `rayon`, `ignore`, `walkdir`, `clipboard`.
2. Activer les fonctionnalités Windows pour le support des chemins longs (Manifest).
3. Définir la structure des dossiers : `/src`, `/ui`.

## Phase 2 : Développement du Frontend (Slint)
**Objectif : Créer une interface fluide à 60 FPS.**
1. Implémenter `ui/app_window.slint` en respectant les codes de Windows 11.
2. Définir les modèles de données (`struct` Slint) pour les résultats.
3. Exposer les callbacks : `search-query-changed`, `row-activated`, `open-folder-requested`.

## Phase 3 : Moteur de Recherche (Backend Rust)
**Objectif : Performance brute et non-blocage de l'UI.**
1. **Logic (engine.rs)** :
    * Utiliser `ignore::WalkBuilder` pour respecter les `.gitignore`.
    * Implémenter le scan multi-cœurs avec `rayon`.
    * Utiliser des buffers de lecture (`BufReader`) pour le contenu des fichiers.
2. **Optimisation** :
    * Arrêt précoce (early return) dès qu'un match est trouvé dans un fichier.
    * Ignorer par défaut les répertoires : `target/`, `.git/`, `.settings/`.

## Phase 4 : Bridge UI-Backend (Main Loop)
**Objectif : Streaming des données.**
1. Gérer l'asynchronisme : Le thread de scan doit envoyer les résultats vers le thread UI via un `slint::ComponentHandle`.
2. Utiliser `slint::Model` pour mettre à jour la liste en temps réel.
3. Implémenter la gestion native des raccourcis clavier (Crossterm ou gestion native Slint).

## Phase 5 : Intégration Windows & Finitions
1. Détection dynamique du thème système (Clair/Sombre).
2. Application des effets visuels avancés (Mica via `window-vibrancy`).
3. Compilation en mode `--release` pour valider les performances sur de larges volumes de données.

## Critères d'Acceptation
* Recherche instantanée dans un projet de +10 000 fichiers.
* Zéro latence sur l'interface pendant le scan.
* Fonctionnement optimal des raccourcis clavier système.