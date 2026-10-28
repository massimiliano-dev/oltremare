#!/bin/bash
# Compila Oltremare firmato con il tuo certificato «Apple Development» e lo installa in Applicazioni.
# Con la stessa firma macOS ricorda i permessi (schermo, accessibilità, disco, microfono) tra una versione e l'altra.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  # Il primo certificato Apple Development valido (si usa l'impronta: i nomi possono ripetersi)
  APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning | grep "Apple Development" | grep -v REVOKED | head -1 | awk '{print $2}')
fi
if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
  echo "Nessun certificato Apple Development: apri Xcode → Impostazioni → Account e creane uno (gratis)." >&2
  exit 1
fi
export APPLE_SIGNING_IDENTITY

# --solo-installa: usa la build già pronta, senza ricompilare
if [ "${1:-}" != "--solo-installa" ]; then
  npm run tauri build -- --bundles app
fi

APP="src-tauri/target/release/bundle/macos/Oltremare.app"
codesign --verify --deep --strict "$APP"
osascript -e 'tell application "Oltremare" to quit' 2>/dev/null || true
sleep 2
pkill -f "/Applications/Oltremare.app/Contents/MacOS/oltremare" 2>/dev/null || true
rm -rf /Applications/Oltremare.app
ditto "$APP" /Applications/Oltremare.app
open /Applications/Oltremare.app
echo "Oltremare installato in /Applications."
