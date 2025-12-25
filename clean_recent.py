#!/usr/bin/env python3
import json
import os

# Chemin du fichier de configuration
config_path = os.path.expandvars(r"%APPDATA%\quick-findr\favorites.json")

# Lire le fichier
with open(config_path, 'r', encoding='utf-8') as f:
    data = json.load(f)

# Filtrer les chemins de test
original_count = len(data['recent_folders'])
data['recent_folders'] = [folder for folder in data['recent_folders'] 
                         if not folder['path'].startswith('/test/path')]
new_count = len(data['recent_folders'])

print(f"Nettoyage: {original_count} -> {new_count} dossiers récents")

# Sauvegarder
with open(config_path, 'w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)

print("Fichier nettoyé avec succès!")
