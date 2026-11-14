#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Essai de fumée de l'application Android sur l'émulateur de la CI (.github/workflows/android.yml,
# job « Émulateur ») : l'APK Debug est installé, lancé, puis essayé comme un pilote le ferait.
#
#   android-smoke.sh <apk Debug> <dossier de sortie>
#
#  1. lancement : « application started » dans le journal, capture « home » ;
#  2. lien corde://races (comme un toucher de widget ou un lien) : capture « races » ;
#  3. bouton retour : revient à l'accueil (« back: back »), puis quitte depuis l'accueil
#     (« back: exit ») ;
#  4. trafic en clair et origine : la page appelle http://10.0.2.2:8471/api/health (la machine de la
#     CI vue de l'émulateur), un faux serveur Corde note l'en-tête Origin (attendu : http://localhost) ;
#  5. lecteur de QR code (ZXing, sans ML Kit ni services Google) : il s'ouvre sans planter,
#     capture « scan » ;
#  6. rappel de test dans 60 s, programmé par la page comme un vrai (canal, son, icône, alarme
#     exacte), puis l'application est TUÉE et le téléphone mis en veille (écran éteint, Doze
#     forcé) : la notification doit arriver quand même (dumpsys notification), capture
#     « notification » du volet ouvert.
#
# Les captures sont réduites en JPEG (390 px de large) et écrites EN BASE64 DANS LE JOURNAL entre
#   -----BEGIN CORDE SCREENSHOT <nom>-----
#   -----END CORDE SCREENSHOT-----
# comme pour l'iPhone (docs/IOS.md §3 pour les décoder). Les points 1, 2 et 6 sont exigés ; les
# autres donnent un avertissement.
#
# Lancé par reactivecircus/android-emulator-runner (adb déjà connecté à l'émulateur).

set -uo pipefail

apk="${1:?usage : android-smoke.sh <apk> <dossier de sortie>}"
out="${2:?usage : android-smoke.sh <apk> <dossier de sortie>}"
pkg="io.github.dultra123.corde"
activity="${pkg}/.MainActivity"
mkdir -p "$out"
errors=0

err() {
  echo "::error::$*"
  errors=$((errors + 1))
}
warn() { echo "::warning::$*"; }
ok() { echo "✓ $*"; }

shot() {
  local name="$1"
  adb exec-out screencap -p > "$out/${name}.png" || {
    warn "capture ${name} impossible"
    return
  }
  # ImageMagick (installé par le workflow avant l'émulateur).
  convert "$out/${name}.png" -resize 390x -quality 70 "$out/${name}.jpg" || {
    warn "capture ${name} : conversion en JPEG impossible"
    return
  }
  echo "-----BEGIN CORDE SCREENSHOT ${name}-----"
  base64 -w 76 "$out/${name}.jpg"
  echo "-----END CORDE SCREENSHOT-----"
}

# Le journal du système depuis le dernier `adb logcat -c`.
logs() { adb logcat -d 2> /dev/null; }

# keep <nom> : garde le journal (artefact) et montre les lignes de Corde depuis le dernier effacement.
keep() {
  logs > "$out/logcat-$1.txt"
  echo "::group::Journal : $1"
  grep -E "\[corde\]|CordeDebug|AndroidRuntime|FATAL|ANR in|not responding" "$out/logcat-$1.txt" |
    sed 's/.*Msg: //' | tail -n 60 | tr -d '\r'
  echo "::endgroup::"
}

# wait_log <motif> <secondes> : le motif apparaît dans le journal.
wait_log() {
  local pattern="$1" seconds="$2" i
  for ((i = 0; i < seconds; i++)); do
    if logs | grep -E -q -- "$pattern"; then return 0; fi
    sleep 1
  done
  return 1
}

start_app() { adb shell am start -W -n "$activity" "$@" > /dev/null; }

# Faux serveur Corde sur la machine de la CI (port 8471) : répond à /api/health comme Corde et
# note l'en-tête Origin de chaque requête.
python3 - "$out/fake-server.log" > /dev/null 2>&1 << 'PY' &
import http.server, json, sys
log = open(sys.argv[1], "a", buffering=1)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        log.write(f"{self.command} {self.path} Origin: {self.headers.get('Origin')}\n")
        body = json.dumps({"ok": True, "version": "1.0.0", "role": "desktop"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", self.headers.get("Origin") or "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *a):
        pass
http.server.ThreadingHTTPServer(("0.0.0.0", 8471), H).serve_forever()
PY
server_pid=$!
trap 'kill "$server_pid" 2> /dev/null || true' EXIT

adb wait-for-device
# Un émulateur qui vient de démarrer affiche parfois « L'interface système ne répond pas » : laisser
# finir le démarrage, puis fermer les fenêtres du système.
sleep 15
adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS > /dev/null 2>&1 || true
adb shell input keyevent KEYCODE_WAKEUP || true
adb shell wm dismiss-keyguard || true
adb shell settings put system screen_off_timeout 1800000 || true
echo "Android $(adb shell getprop ro.build.version.release | tr -d '\r') (API $(adb shell getprop ro.build.version.sdk | tr -d '\r')), WebView $(adb shell dumpsys package com.android.webview 2> /dev/null | sed -n 's/^ *versionName=//p' | head -n 1 | tr -d '\r')"
adb install -r "$apk" > /dev/null || {
  err "installation de l'APK refusée"
  exit 1
}
# En français (langue de l'application seulement, Android 13+ : sans redémarrer le téléphone).
adb shell cmd locale set-app-locales "$pkg" --locales fr-FR > /dev/null 2>&1 || true
echo "Langue de l'application : $(adb shell cmd locale get-app-locales "$pkg" 2> /dev/null | tr -d '\r') ; du téléphone : $(adb shell settings get system system_locales 2> /dev/null | tr -d '\r')"
echo "::group::Autorisations demandées par l'application (manifeste fusionné)"
adb shell dumpsys package "$pkg" | sed -n '/requested permissions:/,/install permissions:/p' | tr -d '\r'
echo "::endgroup::"
if adb shell dumpsys package "$pkg" | grep -q "android.permission.CAMERA: granted=true"; then
  warn "l'appareil photo est déjà autorisé à l'installation"
else
  ok "appareil photo non autorisé avant « Scanner le QR code »"
fi

# 1. Lancement : l'accueil.
adb logcat -c
start_app
if wait_log "\[corde\] application started \(android\)" 150; then
  ok "démarrage de l'interface (android)"
else
  err "l'interface n'a pas démarré (« application started » absent du journal)"
fi
sleep 6
shot home
keep 1-start

# 2. Lien corde://races.
adb shell am start -W -a android.intent.action.VIEW -d "corde://races" "$pkg" > /dev/null
if wait_log "\[corde\] link: #/races" 30; then
  ok "lien corde://races reçu par l'interface"
else
  err "lien corde://races : « link: #/races » absent du journal"
fi
sleep 6
shot races

# 3. Retour. Un toucher sur l'onglet « Accueil » (le premier des cinq, en bas de l'écran), comme le
#    pilote : une page ouverte par un lien, sans toucher, ne compte pas dans l'historique de la
#    webview (règle de Chromium contre les pages qui empilent l'historique). Puis « retour » jusqu'à
#    ce que l'application se ferme : au moins une fois la page précédente, puis la sortie.
size="$(adb shell wm size | tr -d '\r' | sed -n 's/.*: \([0-9]*\)x\([0-9]*\)$/\1 \2/p' | tail -n 1)"
width="${size% *}"
height="${size#* }"
adb logcat -c
adb shell input tap "$((width / 10))" "$((height * 92 / 100))"
sleep 4
went_back=false
exited=false
for _ in 1 2 3 4; do
  adb shell input keyevent KEYCODE_BACK
  sleep 3
  if logs | grep -q "\[corde\] back: exit"; then
    exited=true
    break
  fi
  logs | grep -q "\[corde\] back: back" && went_back=true
done
if $went_back; then ok "retour : page précédente"; else warn "retour : « back: back » absent"; fi
if $exited; then ok "retour depuis la première page : l'application se ferme"; else warn "retour : « back: exit » absent"; fi
sleep 2
if adb shell dumpsys activity activities | grep -E "mResumedActivity|topResumedActivity" | grep -q "$pkg"; then
  warn "l'application est encore au premier plan après le second retour"
else
  ok "l'application n'est plus au premier plan"
fi
keep 3-back

# 4. Trafic en clair vers une adresse privée, origine de la page.
start_app --es cordeDebug fetch --es cordeUrl "http://10.0.2.2:8471/api/health"
if wait_log "\[corde\] debug: fetch" 30; then
  logs | grep -E -o "\[corde\] debug: fetch.*" | tail -n 1
  if logs | grep -E -q "\[corde\] debug: fetch 200 "; then ok "http:// vers 10.0.2.2:8471 (clair, réseau privé) : 200"; else warn "http:// vers 10.0.2.2:8471 : pas de réponse 200"; fi
else
  warn "appel http:// de la page : rien dans le journal (« debug: fetch »)"
  logs | grep -E "\[corde\] debug" | tail -n 3 || true
fi
if grep -q "Origin: http://localhost$" "$out/fake-server.log" 2> /dev/null; then
  ok "origine de la page : http://localhost (acceptée par Corde 0.8 et plus, sans droit « ce PC »)"
else
  warn "origine reçue par le faux serveur : $(tail -n 1 "$out/fake-server.log" 2> /dev/null || echo aucune)"
fi

# 5. Lecteur de QR code (ZXing).
adb shell pm grant "$pkg" android.permission.CAMERA || true
adb logcat -c
start_app --es cordeDebug scan
sleep 14
shot scan
if logs | grep -E -q "FATAL EXCEPTION|NoClassDefFoundError|ClassNotFoundException"; then
  warn "le lecteur de QR code a planté :"
  logs | grep -E -A 12 "FATAL EXCEPTION|NoClassDefFoundError|ClassNotFoundException" | head -n 40
else
  ok "lecteur de QR code ouvert sans plantage"
fi
adb shell input keyevent KEYCODE_BACK
wait_log "\[corde\] debug: scan" 15 && logs | grep -E -o "\[corde\] debug: scan.*" | tail -n 1

# 6. Rappel application fermée, téléphone en veille.
adb shell pm grant "$pkg" android.permission.POST_NOTIFICATIONS || true
adb shell appops set "$pkg" SCHEDULE_EXACT_ALARM allow || true
adb logcat -c
start_app --es cordeDebug reminder --ei cordeDelay 60
if wait_log "\[corde\] debug: reminder in 60 s" 30; then
  logs | grep -E -o "\[corde\] debug: reminder.*" | tail -n 1
  scheduled_at=$(date +%s)
else
  err "le rappel de test n'a pas été programmé"
  scheduled_at=$(date +%s)
fi
echo "::group::Alarmes de Corde (dumpsys alarm)"
adb shell dumpsys alarm | grep -E -B 1 -A 3 "$pkg" | head -n 20 | tr -d '\r'
echo "::endgroup::"
adb shell input keyevent KEYCODE_HOME
sleep 3
adb shell am kill "$pkg" || true
sleep 2
if [ -z "$(adb shell pidof "$pkg" | tr -d '\r')" ]; then
  ok "application tuée avant le rappel"
else
  warn "l'application tourne encore (am kill)"
fi
adb shell dumpsys battery unplug || true
adb shell input keyevent KEYCODE_SLEEP || true
sleep 2
adb shell dumpsys deviceidle enable > /dev/null 2>&1 || true
idle="$(adb shell dumpsys deviceidle force-idle deep 2>&1 | tr -d '\r')"
echo "deviceidle force-idle : ${idle}"
if adb shell dumpsys deviceidle get deep 2>&1 | tr -d '\r' | grep -q -x "IDLE"; then
  ok "téléphone en veille (Doze profond forcé, écran éteint)"
else
  warn "Doze n'a pas pu être forcé : le rappel est essayé écran éteint seulement"
fi
remaining=$((scheduled_at + 75 - $(date +%s)))
[ "$remaining" -gt 0 ] && sleep "$remaining"
delivered=false
for _ in $(seq 1 30); do
  # A posted notification (« NotificationRecord ») of Corde on the reminders' channel — not the
  # channel's own description, which exists since the start.
  if adb shell dumpsys notification --noredact | tr -d '\r' |
    grep -A 12 -E "NotificationRecord\(.*pkg=${pkg}" | grep -q "corde-races"; then
    delivered=true
    break
  fi
  sleep 2
done
adb shell dumpsys deviceidle unforce > /dev/null 2>&1 || true
adb shell dumpsys battery reset > /dev/null 2>&1 || true
adb shell input keyevent KEYCODE_WAKEUP || true
adb shell wm dismiss-keyguard || true
if $delivered; then
  ok "notification de rappel affichée application fermée, téléphone en veille (canal corde-races)"
else
  err "le rappel de test n'est pas arrivé (dumpsys notification)"
fi
echo "::group::Notification et canal (dumpsys notification)"
adb shell dumpsys notification --noredact | tr -d '\r' |
  grep -A 12 -E "NotificationRecord\(.*pkg=${pkg}" | head -n 30
adb shell dumpsys notification --noredact | tr -d '\r' | grep -E "mId='corde-races'" | head -n 2
echo "::endgroup::"
sleep 2
adb shell cmd statusbar expand-notifications || true
sleep 3
shot notification
adb shell cmd statusbar collapse || true

keep 6-reminder

if [ "$errors" -gt 0 ]; then
  echo "${errors} point(s) exigé(s) en échec"
  exit 1
fi
echo "Essai de fumée Android : réussi"
