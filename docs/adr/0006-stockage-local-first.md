# ADR 0006 — Stockage local-first : SQLite + Parquet, aucun cloud en V1

- **Statut** : acceptée — 2026-11-02
- **Source** : `docs/research/03-stack-technique.md` §4 et §5, `docs/ARCHITECTURE.md` §1 et §5

## Contexte

Corde doit enregistrer des sessions longues (une course de 6 h à 100 Hz ≈ 2,2 millions d'échantillons par
canal), les relire pour l'analyse et le profil pilote, et rester utilisable sans réseau. Le client veut « un
profil avec toutes les données de toutes les courses ». Les concurrents sont majoritairement cloud et
abonnement (brief 03 §7) ; la différenciation voulue est **local-first, Linux natif, explicable**.

Options évaluées : SQLite seul (ligne par échantillon ≈ brut × 1,5, lent en balayage), Parquet seul (pas de
requêtes relationnelles), DuckDB embarqué (excellent SQL analytique mais compilation lourde, écritures
petites lentes), binaire maison (dette), InfluxDB (pas d'embarqué sérieux).

## Décision

1. **Métadonnées relationnelles dans SQLite** (`rusqlite`, bundled, migrations versionnées) : `session`,
   `lap` + `sector`, `event`, `setup` (versionné, `parent_id`, source `game_rest | svm | user | recommender`),
   `channel` (catalogue par session), `driver_profile`, `recommendation` (entrées, règles déclenchées,
   interprétation du ressenti, changements proposés, accepté/refusé), `layout`.
2. **Échantillons en Parquet** (crate `parquet`/`arrow`, compression zstd, un row-group par ≈ 5 s) : un fichier
   par session, colonnes = canaux normalisés du protocole, `t` en secondes depuis le début de session, `lap`,
   `lap_dist_m`. Ratio de compression attendu 4–8× sur signaux lisses, **à mesurer** sur données réelles.
3. **Emplacement** : `%LOCALAPPDATA%\Corde\` (Windows), `~/.local/share/corde/` (Linux) :
   `library.sqlite` + `sessions/<année>/<mois>/<id>/samples.parquet` + `meta.json` (copie dénormalisée pour la
   résilience).
4. **Aucune donnée ne quitte le PC sans action explicite.** Pas de compte, pas de serveur Corde, pas de
   télémétrie d'usage. Seule exception, désactivée par défaut : l'envoi du **texte du ressenti** à une API de
   modèle de langage avec clé fournie par l'utilisateur (le modèle ne reçoit ni télémétrie brute, ni nom de
   pilote). Une synchronisation cloud, si elle arrive un jour, sera optionnelle et chiffrée.
5. **DuckDB différé** : non embarqué en V1. Les fichiers DuckDB *du jeu* (`UserData\Telemetry\*.duckdb`)
   restent une source d'import optionnelle (tour + setup complet) via la bibliothèque DuckDB (MIT) derrière
   un feature flag.
6. **Export** : CSV depuis Arrow (choix des canaux, décimation) dès la phase 1 ; MoTeC `.ld` plus tard
   (crate `motec-i2`), suppression complète des données en un clic.

## Conséquences

- Une saison de 50 sessions d'1–2 h tient en quelques Go ; quota et rotation configurables.
- Crash en course : les row-groups Parquet déjà écrits restent lisibles, `meta.json` est mis à jour en
  continu ; SQLite est ACID.
- L'analyse (Rust) lit Parquet via `arrow` ; des notebooks Python peuvent lire les mêmes fichiers pour la R&D
  des règles sans dépendance en production.
- Les dispositions de dashboard sont persistées côté serveur dès la phase 0 (JSON dans le dossier de
  données, puis table `layout`).
- Le profil pilote (SPEC §7) est une vue sur ces tables ; plusieurs profils = plusieurs `driver_profile`,
  pas plusieurs bases.

## Références

- rusqlite : https://docs.rs/rusqlite — parquet/arrow : https://docs.rs/parquet
- DuckDB client Rust (différé) : https://duckdb.org/docs/2.0/clients/rust/overview.html — comparatif
  DuckDB / SQLite : https://www.datacamp.com/blog/duckdb-vs-sqlite-complete-database-comparison
- Enregistrement natif LMU en DuckDB :
  https://guide.lemansultimate.com/hc/en-gb/articles/14524956311695-Telemetry-Recording ,
  https://github.com/mzluzifer/lmu-telemetry-analyzer
- MoTeC `.ld` : https://github.com/afonso360/motec-i2
- Sorties structurées de l'API Claude (ressenti) :
  https://platform.claude.com/docs/en/build-with-claude/structured-outputs
