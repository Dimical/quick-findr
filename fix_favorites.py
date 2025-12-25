#!/usr/bin/env python3
# Script pour corriger la section des favoris dans app_window.slint

import re

file_path = r"c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"

# Lire le fichier
with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

# Trouver et remplacer la section corrompue des favoris
# Pattern pour trouver la section "for fav in root.favorites"
pattern = r'(for fav in root\.favorites : Rectangle \{[^}]*?\n\s+HorizontalLayout \{)(.*?)(\n\s+\}\n\s+\})'

# Nouvelle section propre
replacement = r'''\1
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
                                    }\3'''

# Appliquer le remplacement
content_fixed = re.sub(pattern, replacement, content, flags=re.DOTALL)

# Sauvegarder
with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content_fixed)

print("Fichier corrigé avec succès!")
