#!/usr/bin/env python3
import re

file_path = r"c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"

# Lire le fichier
with open(file_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Trouver la ligne qui commence la section des favoris
start_idx = None
for i, line in enumerate(lines):
    if 'for fav in root.favorites : Rectangle {' in line:
        start_idx = i
        break

if start_idx is None:
    print("Section des favoris non trouvée!")
    exit(1)

# Trouver la fin de cette section (chercher la fermeture du for)
end_idx = None
brace_count = 0
for i in range(start_idx, len(lines)):
    brace_count += lines[i].count('{') - lines[i].count('}')
    if brace_count == 0 and i > start_idx:
        end_idx = i
        break

if end_idx is None:
    print("Fin de section non trouvée!")
    exit(1)

print(f"Section trouvée de la ligne {start_idx+1} à {end_idx+1}")

# Nouvelle section propre des favoris
new_section = '''                                for fav in root.favorites : Rectangle {
                                    height: 36px;
                                    background: fav-touch.has-hover ? (root.dark-mode ? #383838 : #f0f0f0) : transparent;
                                    border-radius: 4px;
                                    
                                    HorizontalLayout {
                                        padding-left: 8px;
                                        padding-right: 8px;
                                        spacing: 8px;
                                        
                                        Text {
                                            text: "⭐";
                                            font-family: "Segoe UI Emoji";
                                            font-size: 14px;
                                            vertical-alignment: center;
                                        }
                                        
                                        VerticalLayout {
                                            spacing: 2px;
                                            horizontal-stretch: 1;
                                            
                                            Text {
                                                text: fav.name;
                                                color: root.dark-mode ? #ffffff : #111111;
                                                font-size: 12px;
                                                font-weight: 600;
                                                overflow: elide;
                                            }
                                            
                                            Text {
                                                text: fav.path;
                                                color: root.dark-mode ? #999999 : #666666;
                                                font-size: 10px;
                                                overflow: elide;
                                            }
                                        }
                                        
                                        Rectangle {
                                            width: 24px;
                                            height: 24px;
                                            border-radius: 4px;
                                            background: remove-touch.has-hover ? (root.dark-mode ? #ff4444 : #ffcccc) : transparent;
                                            
                                            Text {
                                                text: "🗑️";
                                                font-family: "Segoe UI Emoji";
                                                font-size: 12px;
                                                vertical-alignment: center;
                                                horizontal-alignment: center;
                                            }
                                            
                                            remove-touch := TouchArea {
                                                mouse-cursor: pointer;
                                                clicked => { 
                                                    root.remove-from-favorites(fav.path);
                                                }
                                            }
                                        }
                                    }
                                    
                                    fav-touch := TouchArea {
                                        mouse-cursor: pointer;
                                        clicked => { 
                                            root.select-favorite(fav.path);
                                            root.favorites-visible = false;
                                        }
                                    }
                                }
'''

# Reconstruire le fichier
new_lines = lines[:start_idx] + [new_section] + lines[end_idx+1:]

# Sauvegarder
with open(file_path, 'w', encoding='utf-8') as f:
    f.writelines(new_lines)

print("Fichier reconstruit avec succès!")
