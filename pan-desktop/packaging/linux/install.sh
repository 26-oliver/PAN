#!/bin/sh
set -eu
cd "$(dirname "$0")"
prefix="${HOME}/.local"
data="${XDG_DATA_HOME:-$prefix/share}"
install -D -m 755 pan "$prefix/bin/pan"
install -D -m 644 pan.png "$data/icons/hicolor/160x160/apps/pan.png"
install -d "$data/applications"
escaped=$(printf '%s' "$prefix/bin/pan" | sed 's/%/%%/g; s/"/\\"/g')
while IFS= read -r line; do
    case "$line" in
        Exec=*) printf 'Exec="%s" %%F\n' "$escaped" ;;
        *) printf '%s\n' "$line" ;;
    esac
done < pan.desktop > "$data/applications/pan.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data/applications"
fi
printf 'Instalado PAN. Elige Abrir con → PAN en el gestor de archivos.\n'
