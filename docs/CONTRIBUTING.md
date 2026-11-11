# Contribuer à Corde

Court et pratique. Les règles de fond sont dans [ARCHITECTURE.md](ARCHITECTURE.md) (§4 conventions) et les
[ADR](adr/) ; ce document dit comment travailler au quotidien.

## 0. Licence des contributions

Corde est un logiciel libre sous **GPL-3.0-or-later** ([LICENSE](../LICENSE)). En proposant une contribution
(PR, correctif, données), tu acceptes qu'elle soit publiée sous cette même licence. Le droit d'auteur reste à
chaque contributeur (« dultra__ et les contributeurs ») ; n'ajoute ni adresse e-mail personnelle ni nom réel
dans le code, les données d'exemple ou les captures.

## 1. Conventions de code (rappel d'ARCHITECTURE §4)

- **Unités humaines dans le protocole** : °C, km/h et m/s, degrés, kPa, litres, secondes. Les conversions
  (Kelvin, radians, fractions) se font dans l'adaptateur, via `corde_core::derive`.
- **Roues** : toujours FL, FR, RL, RR. **Températures** `temp_c[3]` = gauche / centre / droite de la bande.
- **Sentinelles** : un temps ou une valeur absente est `None`, jamais `-1` ou `0`. Un jeu qui ne fournit pas
  un champ laisse `None` / `0` / `Unknown` ; on ne devine pas.
- **Langues** : identifiants, commentaires de code et messages de log en anglais ; documentation utilisateur
  et textes d'interface en français (clé dans `packages/ui/src/i18n/fr.ts`, l'anglais dans `en.ts`, aucune
  chaîne codée en dur dans les composants).
- **Licences** : pas de copie de code GPL (TinyPedal, plugin rF2, winecarte) ni du header S397 — on lit, on
  réécrit, on teste les tailles/offsets ([ADR 0005](adr/0005-licences-et-header-s397.md)).
- **Rust** : `cargo fmt`, `cargo clippy --workspace --all-targets` sans nouvel avertissement, `unsafe`
  uniquement dans les crates d'adaptateurs avec un commentaire `// SAFETY:`. Tout ce qui reste à vérifier
  sur le vrai jeu est marqué `TODO(verify-on-pc)`.
- **TypeScript** : Biome (`pnpm --filter @corde/ui lint`), `tsc --noEmit` propre, tests Vitest à côté du code
  (`*.test.ts`).
- **Ne pas modifier** `packages/protocol/src/generated/` à la main (généré) ni `docs/ARCHITECTURE.md` sans
  ADR ou discussion.

## 2. Commits et branches

Commits conventionnels (https://www.conventionalcommits.org/fr/v1.0.0/) :

```
<type>(<portée>): <résumé à l'impératif, ≤ 72 caractères>

feat(ui): widget électronique (TC/ABS/map)
fix(lmu): torn-read check on mElapsedTime
docs(spec): critères d'acceptation phase 1
chore(ci): cache cargo sur windows
```

Types : `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `ci`, `build`. Portées usuelles :
`protocol`, `core`, `lmu`, `sim`, `server`, `ui`, `knowledge`, `ci`, `docs`.

Une branche par sujet (`feat/ui-map-widget`), PR vers `main` avec la CI verte (`fmt`, `clippy` toléré en
avertissement pour l'instant, `cargo test`, `typecheck`, `test`, `build`). Un changement de
`corde-protocol` incompatible incrémente `PROTOCOL_VERSION` et est signalé dans le titre de la PR.

## 3. Régénérer les types TypeScript

Les types de `@corde/protocol` sont **générés** depuis `crates/corde-protocol/src/lib.rs` par `ts-rs` et
ignorés par git :

```bash
cargo test -p corde-protocol      # écrit packages/protocol/src/generated/*.ts
# ou
pnpm protocol:gen
```

À faire après tout changement de struct ou d'enum du protocole, **avant** `pnpm -r typecheck`. Si tu ajoutes
un type exporté, ajoute aussi sa ligne `export type { … }` dans `packages/protocol/src/index.ts`.

## 4. Ajouter un jeu (adaptateur)

1. Créer `crates/corde-<jeu>/` avec les modules `layout` (structures `#[repr(C)]` + tests `size_of` /
   `offset_of`), `reader` (ouverture du mapping / fichier, lecture cohérente), `convert` (vers
   `TelemetryFrame`) et `source`.
2. Implémenter `corde_core::TelemetrySource` (`crates/corde-core/src/source.rs`) :

   ```rust
   pub trait TelemetrySource: Send {
       fn info(&self) -> SourceInfo;                       // game, id ("acc-shm"…), label, native_rate_hz
       fn connect(&mut self) -> Result<(), SourceError>;   // NotAvailable tant que le jeu n'est pas là
       fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError>;  // Ok(None) = rien de neuf
       fn poll_interval(&self) -> Duration { /* défaut : moitié de la période native */ }
       fn disconnect(&mut self) {}
   }
   ```

   `poll` doit être bon marché (une copie + une conversion) : le serveur l'appelle sur un thread dédié.
3. Ajouter la variante dans `corde_protocol::GameId` si elle n'existe pas, régénérer les types TS.
4. Remplir le protocole en respectant les conventions (§1) ; les champs non fournis restent `None`/`Unknown`.
   Les grandeurs dérivées partagées (slip ratio, slip angle, g) passent par `corde_core::derive`.
5. Livrer des **fixtures** (dump binaire anonymisé dans `reference/`) et un test de conversion bout en bout ;
   brancher la source dans `corde-server` (`--source <id>`).
6. Documenter la source dans `docs/research/` (ce que le jeu expose, licence, Linux) et, si une décision
   s'impose, écrire un ADR.

## 5. Base de connaissances (`knowledge/`)

Données, pas code. Avant de toucher un YAML : lire [`knowledge/README.md`](../knowledge/README.md)
(schémas, niveaux de confiance, identifiants croisés). Pas de chiffre sans source ni sans niveau de
confiance ; jamais de valeur absolue de setup.

## 6. Documentation

- Décision structurante → nouvel ADR `docs/adr/NNNN-sujet.md` (contexte, décision, conséquences, références).
- Changement visible pour le client → `docs/SPEC.md` (critères d'acceptation) et, si l'usage change,
  `docs/TESTING-ON-PC.md`.
