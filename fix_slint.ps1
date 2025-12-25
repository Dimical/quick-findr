# Script pour corriger le fichier app_window.slint
$file = "c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint"
$backup = "c:\Users\dimfo\Documents\Projects\quick-findr\ui\app_window.slint.backup"

# Lire le contenu
$content = Get-Content $file -Raw

# Supprimer les lignes problématiques avec spacing dans Rectangle
$content = $content -replace 'Rectangle\s*\{\s*spacing:\s*2px;', 'Rectangle {'

# Supprimer les accolades en trop à la fin
$lines = $content -split "`n"
$lastLines = $lines[-10..-1]
$accoladeCount = ($lastLines | Select-String -Pattern '\}' -AllMatches).Matches.Count
Write-Host "Nombre d'accolades dans les 10 dernières lignes: $accoladeCount"

# Sauvegarder
Set-Content $file $content -NoNewline
Write-Host "Fichier corrigé"
