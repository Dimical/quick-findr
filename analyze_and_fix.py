#!/usr/bin/env python3
import re

file_path = r"c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"

with open(file_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Analyser la structure
brace_level = 0
errors = []

for i, line in enumerate(lines, 1):
    # Compter les accolades sur cette ligne
    open_count = line.count('{')
    close_count = line.count('}')
    
    brace_level += open_count - close_count
    
    if brace_level < 0:
        errors.append(f"Ligne {i}: Trop d'accolades fermantes (niveau: {brace_level})")
        print(f"Ligne {i}: {line.strip()[:50]}... - Niveau: {brace_level}")

print(f"\nNiveau final: {brace_level}")
print(f"Nombre d'erreurs: {len(errors)}")

if brace_level != 0:
    print(f"\nIl manque {brace_level} accolade(s) fermante(s)" if brace_level > 0 else f"\nIl y a {-brace_level} accolade(s) fermante(s) en trop")
    
    # Corriger
    if brace_level < 0:
        # Supprimer les accolades en trop à la fin
        for _ in range(-brace_level):
            for j in range(len(lines) - 1, -1, -1):
                if lines[j].strip() == '}':
                    lines.pop(j)
                    break
    else:
        # Ajouter les accolades manquantes
        lines.append('}\n' * brace_level)
    
    # Sauvegarder
    with open(file_path, 'w', encoding='utf-8') as f:
        f.writelines(lines)
    
    print("Fichier corrigé!")
else:
    print("\nLe fichier est équilibré!")
