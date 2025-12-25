#!/usr/bin/env python3
# Script pour corriger complètement le fichier app_window.slint

file_path = r"c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"

# Lire le fichier
with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

# Compter les accolades ouvrantes et fermantes
open_braces = content.count('{')
close_braces = content.count('}')

print(f"Accolades ouvrantes: {open_braces}")
print(f"Accolades fermantes: {close_braces}")
print(f"Différence: {open_braces - close_braces}")

# Si il y a plus d'accolades fermantes, on doit en enlever
if close_braces > open_braces:
    diff = close_braces - open_braces
    print(f"Il faut enlever {diff} accolade(s) fermante(s)")
    
    # Enlever les accolades en trop à la fin
    lines = content.split('\n')
    for i in range(diff):
        # Chercher la dernière ligne avec juste une accolade fermante
        for j in range(len(lines) - 1, -1, -1):
            if lines[j].strip() == '}':
                lines.pop(j)
                break
    
    content = '\n'.join(lines)

# Si il manque des accolades fermantes, on doit en ajouter
elif open_braces > close_braces:
    diff = open_braces - close_braces
    print(f"Il faut ajouter {diff} accolade(s) fermante(s)")
    content += '\n' + '}\n' * diff

# Sauvegarder
with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content)

print("Fichier corrigé!")
