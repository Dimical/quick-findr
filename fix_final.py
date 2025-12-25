#!/usr/bin/env python3
import re

file_path = r"c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"

with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

# Trouver et remplacer TOUTE la section des favoris (de "for fav in root.favorites" jusqu'à la fin du bloc)
# On cherche le pattern complet
pattern = r'for fav in root\.favorites : Rectangle \{.*?(?=\n\s+\}\s+\n\s+// Séparateur)'

# La nouvelle section propre et complète
new_favorites_section = '''for fav in root.favorites : Rectangle {
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

# Remplacer
content_fixed = re.sub(pattern, new_favorites_section, content, flags=re.DOTALL)

# Sauvegarder
with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content_fixed)

print("Section des favoris reconstruite!")
