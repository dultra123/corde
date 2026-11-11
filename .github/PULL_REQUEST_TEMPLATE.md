## Ce que change cette PR

<!-- En quelques lignes : quoi, pourquoi. Lien vers le ticket s'il y en a un. -->

## Vérifications

- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`
- [ ] `pnpm -r typecheck && pnpm -r lint && pnpm -r test && pnpm -r build` (si l'interface ou le protocole change)
- [ ] Documentation mise à jour si l'usage change (`docs/`, README) — voir `docs/CONTRIBUTING.md`

## Licence et code tiers

- [ ] Ma contribution est publiée sous la licence du projet, **GPL-3.0-or-later** (`LICENSE`).
- [ ] Je n'ai copié **aucun code tiers** sous une licence incompatible : **ni le header de Studio 397**
      (`SharedMemoryInterface.hpp`, `InternalsPlugin.hpp`), **ni du code de TinyPedal, du plugin rF2 ou de
      winecarte**. On lit, on réécrit, on teste les tailles et offsets
      ([ADR 0005](https://github.com/dultra123/corde/blob/main/docs/adr/0005-licences-et-header-s397.md)).
- [ ] Toute nouvelle dépendance a une licence de la liste de `deny.toml`.

## Données personnelles

- [ ] **Aucun fichier de résultats du jeu tel quel** (noms des autres pilotes) : les échantillons de
      `reference/` sont anonymisés (noms inventés).
- [ ] Aucun journal, capture d'écran ou fixture contenant un chemin personnel, un nom réel, une adresse
      e-mail, un identifiant Steam ou une clé d'API.
