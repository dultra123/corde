# @corde/ui — tableau de bord et navigateur de sessions

Interface web de Corde pour Le Mans Ultimate (React 19, gridstack 14, uPlot, zustand). Deux
sections, derrière des routes `#/…` (pas de bibliothèque de routing) :

- **Direct** (`#/`, page par défaut) : la grille de widgets de télémétrie. Simple client WebSocket
  de `corde-server` : elle reçoit des `TelemetryFrame` normalisées (voir `packages/protocol`) et
  ne connaît rien du jeu.
- **Sessions** (`#/sessions…`) : les sessions enregistrées par le serveur (`crates/corde-store`),
  lues via l'API REST `/api/sessions…`. La connexion WebSocket vit hors de React
  (`net/connection.ts`) : changer de section n'interrompt jamais le flux.

## Lancer

```sh
pnpm install                       # à la racine du monorepo
pnpm --filter @corde/ui dev        # http://localhost:5173 — /ws et /api sont proxifiés vers 127.0.0.1:8471
pnpm --filter @corde/ui build      # typecheck + bundle dans dist/ (servi par corde-server)
pnpm --filter @corde/ui preview    # sert dist/ en local
```

Qualité :

```sh
pnpm --filter @corde/ui typecheck
pnpm --filter @corde/ui test       # vitest (parse des messages, autonomie carburant, registre, générateur de démo)
pnpm --filter @corde/ui lint       # biome check
pnpm --filter @corde/ui screenshot # build d'abord ; écrit screenshots/desktop.png et screenshots/mobile.png
node scripts/screenshot-sessions.mjs   # idem pour les pages Sessions + Ingénieur, contre un serveur sim (voir en-tête du script)
node scripts/screenshot-sessions.mjs --spawn --engineer-only   # seulement la page Ingénieur (desktop, page entière, mobile)
python3 scripts/compose-mobile.py   # illustration « téléphone » du README (docs/images/mobile.png) : trois écrans entiers, à la même échelle
```

## Pages et routes

| Route | Page | Contenu |
|---|---|---|
| `#/` | `dashboard/Dashboard.tsx` | grille de widgets (voir plus bas) |
| `#/sessions` | `sessions/SessionsPage.tsx` | état de l'enregistreur (pastille, Démarrer / Arrêter), filtres circuit / type, tableau des sessions (date, circuit, type, voiture, tours, meilleur tour, météo, durée, taille ≈), suppression avec confirmation, stockage total en pied |
| `#/sessions/<id>` | `sessions/SessionPage.tsx` | en-tête (circuit, type, voiture, date, durée, météo, cadence, taille), KPI (meilleur tour, tour moyen, régularité = écart-type, conso / tour, vitesse max, tours valides), graphe des temps au tour (uPlot), tableau des tours (violet = meilleur de la session, vert = meilleur perso à ce moment, gris = invalide / rentrée / sortie, secteurs colorés de même, carburant, V. max, mini-barres pneus / freins / usure) ; cocher un tour → « Analyser », deux → « Comparer » |
| `#/sessions/<id>/laps/<n>` | `sessions/LapPage.tsx` | graphes uPlot empilés et synchronisés (curseur commun, zoom par glisser, double-clic pour revenir), axe X distance ou temps, panneaux : vitesse, gaz / frein / volant, rapport & régime, G, T° pneus (centre ou zones), pressions, T° freins, slip ratio, hauteurs de caisse ; « Ajouter un graphe » (sélecteur de canaux groupés par catégorie, depuis `/api/channels`) ; « Exporter CSV » (construit côté client) |
| `#/sessions/<id>/compare?a=3&b=5` | `sessions/LapPage.tsx` | même page avec deux tours : A en trait plein, B en pointillé (même couleur par canal), B rééchantillonné sur l'axe de A, panneau « Delta temps A − B » calculé côté client (vert = A devant) |
| `#/sessions/<id>/engineer` | `engineer/EngineerPage.tsx` | le rapport de l'ingénieur (`GET /api/sessions/{id}/analysis`, calcul de quelques secondes la première fois ; 422 = pas assez de tours propres, « Recalculer » = `POST …/analysis/recompute`) : en-tête avec pastille qualité des données, résumé, style de pilotage (barres 0–100), tours, pneus (tuiles I / M / E disposées comme la voiture), freins (fenêtre froid / ok / chaud), matrice d'équilibre phase × vitesse (bleu survirage → rouge sous-virage, alpha = confiance) + virages repliables, suspension (saturation, histogrammes amortisseurs), constats filtrables par gravité, trois plans de réglage (Agressif / Soft & sûr / Pluie, « Copier en texte », « Exporter JSON »), et le ressenti du pilote (`GET` / `POST /api/sessions/{id}/feedback`, texte 1–2000 caractères + plan testé) |

Les routes sont analysées dans `nav/routes.ts` (`parseRoute`, `routeHash`, `navigate`,
`useRoute`) ; une route inconnue renvoie au dashboard. La chaîne de requête de la page
(`?server=…`) n'est jamais touchée : seul le hash change.

Chargement des tours : `points=2500` par défaut, et rechargement à `points=6000` au premier zoom
si le tour compte nettement plus d'échantillons (`total_rows`). Le delta temps interpole le temps
du tour B à chaque distance du tour A (`sessions/lib/delta.ts`, testé).

### Se connecter au serveur

Dans l'ordre : `?server=192.168.1.20:8471` dans l'URL, puis l'adresse enregistrée dans les
réglages (⚙, stockée en localStorage — c'est le chemin à utiliser sur un téléphone), sinon
l'hôte de la page (`ws://<host>/ws`, le cas normal quand `corde-server` sert `dist/`).

Mode démo : `?demo=1`, le bouton ▶ de la barre, ou automatiquement si aucun serveur ne répond
après 3 s (la reconnexion continue en arrière-plan et la démo s'arrête dès que le serveur dit
`hello`). La démo génère une course de 20 voitures sur un tracé synthétique façon Paul Ricard
(~115 s/tour, 20 Hz), avec deux tours bouclés en silence au démarrage pour avoir un meilleur
tour, un delta et un historique dès l'ouverture.

## Structure

```
src/
├─ main.tsx, App.tsx         # point d'entrée : polices, CSS, démarrage de la connexion
├─ nav/
│  └─ routes.ts              # routes `#/…` : parse / construction / hook useRoute (sans lib)
├─ net/
│  ├─ client.ts              # CordeClient : WebSocket, reconnexion exponentielle, set_rate, ping, Hz reçus
│  ├─ sessions-api.ts        # GET/DELETE /api/sessions…, /api/recording, /api/storage, /api/channels (typé, ApiError)
│  ├─ engineer-api.ts        # GET /api/sessions/{id}/analysis, POST …/recompute, GET/POST …/feedback
│  ├─ messages.ts            # parse des ServerMessage tagués `type` (tolérant : inconnu → ignoré)
│  ├─ connection.ts          # orchestre client ↔ démo ↔ store (bascule auto après 3 s)
│  ├─ url.ts                 # résolution de l'adresse serveur (?server=, réglage, page)
│  ├─ layouts-api.ts         # GET/PUT /api/layouts + fusion (updated_ms le plus récent gagne)
│  ├─ layout-sync.ts         # synchro différée des dispositions quand un serveur répond
│  └─ rate.ts                # mesure de cadence
├─ demo/
│  ├─ track.ts               # tracé fermé 2D + profil de vitesse (80–300 km/h, ~115 s)
│  ├─ generator.ts           # DemoRace : frames complètes, LapRecord, événements, 19 adversaires
│  └─ runner.ts              # timer 20 Hz + warm-up
├─ store/
│  ├─ telemetry.ts           # dernière frame, status, tours, buffers circulaires 30 s, phase de connexion
│  ├─ dashboard.ts           # dispositions (localStorage), disposition courante par appareil, mode édition
│  ├─ recording.ts           # état de l'enregistreur : poll 2 s sur les pages Sessions + événements WS recording_*
│  └─ settings.ts            # adresse serveur, langue, cadence (localStorage)
├─ dashboard/
│  ├─ Dashboard.tsx          # shell : détection desktop/mobile (< 700 px → 4 colonnes), hauteur de cellule
│  ├─ GridCanvas.tsx         # intégration gridstack ↔ React (positions écrites dans le store)
│  ├─ GridItem.tsx           # chrome d'un widget : titre, config, ×, error boundary
│  ├─ TopBar.tsx             # partagée par les deux sections : onglets Direct / Sessions, pastille d'état, réglages
│  ├─ StatusPill.tsx, LayoutMenu.tsx, Palette.tsx, SettingsDialog.tsx
│  └─ defaults.ts            # dispositions par défaut (12 colonnes × 16 lignes bureau, 4 colonnes mobile)
├─ engineer/
│  ├─ EngineerPage.tsx       # chargement du rapport (états chargement / 422 / 404 / erreur, recalcul), en-tête, résumé
│  ├─ Sections.tsx           # style, tours, pneus, freins, équilibre (matrice + virages), suspension, constats
│  ├─ PlansSection.tsx       # onglets Agressif / Soft & sûr / Pluie, changements numérotés, copie texte, export JSON
│  ├─ FeedbackSection.tsx    # formulaire de ressenti + liste des ressentis enregistrés
│  └─ lib.ts                 # couleur de l'indice d'équilibre, formatage des changements / preuves, texte et JSON d'un plan — testé
├─ sessions/
│  ├─ SessionsShell.tsx      # coquille de la section : TopBar, page, réglages, poll de l'enregistreur
│  ├─ SessionsPage.tsx, SessionPage.tsx, LapPage.tsx
│  ├─ RecordingBanner.tsx    # « ● Enregistrement en cours — circuit, tour N, X Mo » + Démarrer / Arrêter
│  ├─ LapTimesChart.tsx      # temps au tour (points pleins / creux, ligne du meilleur, clic = sélection)
│  ├─ ChartPanel.tsx         # un graphe uPlot synchronisé (cursor.sync), légende au survol sans re-rendu React
│  ├─ ChannelPicker.tsx      # sélecteur de canaux (catégories, recherche, familles par roue ×4)
│  ├─ panels.ts              # panneaux prédéfinis + personnalisés → séries / échelles uPlot
│  ├─ channels.ts            # libellés, catégories, couleurs par roue, unités d'affichage des canaux
│  ├─ uplot-theme.ts, shared.tsx
│  └─ lib/                   # stats (meilleur, moyenne, écart-type, tons), delta (interpolation), csv — testés
├─ widgets/
│  ├─ registry.ts            # registerWidget / getWidget / listWidgets
│  ├─ index.ts               # importe (= enregistre) tous les widgets, dans l'ordre de la palette
│  ├─ shared/                # hooks (rAF sur frameTick, ResizeObserver, canvas DPR), Trace uPlot, Empty
│  └─ Laps, Gaps, Inputs, Tyres, Brakes, Fuel, Map, Standings, Session, Electronics
├─ lib/                      # format (temps, deltas), color (échelles T°, classes), fuel (autonomie), ring-buffer
├─ i18n/                     # fr.ts (référence), en.ts, useT()
└─ styles/                   # tokens.css (variables), base.css, dashboard.css, widgets.css, sessions.css, profile.css, engineer.css
```

Conventions reprises du protocole : roues FL, FR, RL, RR ; `temp_c[0..2]` = gauche / centre /
droite du pneu (pas intérieur/extérieur) ; un temps absent est `null`.

## Ajouter un widget

1. Créer `src/widgets/MonWidget.tsx` :

   ```tsx
   import { usePlayer } from "../store/telemetry";
   import { registerWidget, type WidgetProps } from "./registry";
   import { Empty } from "./shared/Empty";

   function MonWidget({ config }: WidgetProps) {
     const player = usePlayer(); // re-rend à chaque frame (20 Hz) ; pour un canvas, voir useFrameDraw()
     if (!player) return <Empty />;
     return <div className="mon-widget num">{player.engine.oil_c.toFixed(0)} °C</div>;
   }

   export const monWidget = registerWidget({
     id: "oil",                       // = WidgetPlacement.kind
     titleKey: "widget.oil",          // à ajouter dans i18n/fr.ts et en.ts
     minW: 2, minH: 2,
     defaultSize: { w: 3, h: 3 },     // grille 12 colonnes
     mobileSize: { w: 4, h: 3 },      // grille 4 colonnes (optionnel)
     component: MonWidget,
     defaultConfig: { unit: "c" },    // optionnel
     configFields: [                  // optionnel : rendu en <select> dans l'en-tête en mode édition
       { key: "unit", labelKey: "oil.unit", options: [{ value: "c", labelKey: "oil.c" }] },
     ],
   });
   ```

2. L'importer dans `src/widgets/index.ts` (l'ordre = l'ordre de la palette).
3. Ajouter les clés de traduction dans `src/i18n/fr.ts` (référence typée) et `src/i18n/en.ts`.
4. Styler dans `src/styles/widgets.css`. Le corps du widget est un *container* CSS : utiliser
   `cqh`/`cqw`/`cqmin` (ex. `font-size: clamp(16px, 12cqh, 32px)`) pour que les grandes valeurs
   suivent la taille du widget, et `@container (max-width: …)` pour les variantes étroites.
   La classe `.num` applique la police condensée à chiffres tabulaires.
5. Pour un rendu canvas ou uPlot, ne pas passer par l'état React : `useFrameDraw(cb)` appelle
   `cb` à chaque nouvelle frame (rAF) et `useTelemetry.getState()` lit le store sans re-rendu ;
   les buffers 30 s (`buffers.speed`, `throttle`, `brake`, `steering`, `gLat`, `gLong`, `rpm`,
   `tyreFL…RR`) sont des `RingBuffer` lus via `since(t)`.

Le test `src/widgets/registry.test.ts` vérifie que chaque widget a des tailles cohérentes et des
titres traduits, et que les dispositions par défaut ne référencent que des widgets existants.

## Ajouter un panneau à la page Tour

1. Ajouter un `PanelKind` et son cas dans `buildPanel()` / `panelChannels()` (`src/sessions/panels.ts`) :
   les canaux à charger, les séries (couleur, échelle, pas / lissage) et les échelles (gauche / droite,
   plage fixe ou auto).
2. L'ajouter à `DEFAULT_PANELS` et créer la clé `panel.<kind>` dans `i18n/fr.ts` et `en.ts`.
3. Les canaux par roue utilisent `WHEEL_COLORS` (AVG bleu, AVD ambre, ARG violet, ARD vert) ;
   les libellés et catégories des canaux sont dans `src/sessions/channels.ts`.
