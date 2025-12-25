# Spécifications de l'Interface Utilisateur (UI/UX) - QuickFindr

## Vision Design
QuickFindr est conçu pour être une extension native de **Windows 11**. L'application utilise les principes du **Fluent Design** pour offrir une expérience fluide, moderne et performante, comblant les lacunes des outils de recherche standards.

## 1. Identité Visuelle
* **Typographie** : Priorité absolue à "Segoe UI Variable". Fallback : "Segoe UI".
* **Effets de Matériau** :
    * **Mica** : Arrière-plan de la fenêtre semi-transparent avec flou de texture.
    * **Coins arrondis** : Rayon de `12px` sur la fenêtre principale et les conteneurs (cartes).
* **Thématisation** : Support natif du mode Clair (Light) et Sombre (Dark) basé sur les paramètres système.

## 2. Structure de la Fenêtre (`app_window.slint`)
### A. Barre de Recherche (Header)
* Champ de saisie central avec icône de recherche.
* **Focus automatique** dès l'ouverture de l'application.
* Bouton "Scan" discret à l'extrémité droite.

### B. Zone de Résultats (Body)
* **Composant `ResultCard`** :
    * Icône spécifique selon l'extension (`.java`, `.yml`, `.xml`, `.log`).
    * Titre : Nom du fichier (Gras).
    * Sous-titre : Chemin absolu (Tronqué si trop long).
    * Extrait : Ligne correspondante si recherche de contenu.
* **États visuels** : Feedback immédiat au survol (Hover) et lors de la sélection (Active).

### C. Barre de Statut (Footer)
* Indicateurs de performance en temps réel :
    * Débit d'E/S (Go/s).
    * Nombre de fichiers scannés.
    * Nombre de threads actifs (via `rayon`).

## 3. Ergonomie et Accessibilité Clavier
L'application doit être entièrement pilotable sans souris :
* **Flèches Haut/Bas** : Navigation dans la liste des résultats.
* **Entrée** : Copier le chemin complet dans le presse-papier.
* **Ctrl + O** : Ouvrir l'emplacement du fichier dans l'Explorateur Windows.
* **Echap** : Effacer la recherche ou fermer l'application.