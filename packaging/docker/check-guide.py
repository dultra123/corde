"""Le texte compose collé par le guide (docs/SYNOLOGY.md, § 6) doit être packaging/docker/compose.yaml
sans ses commentaires (numéro de version de l'image mis à part). Utilisé par la CI (job server-docker) :

    python3 -I packaging/docker/check-guide.py
"""

import pathlib
import re
import sys
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[2]


def normalise(text: str) -> list[str]:
    out = []
    for line in textwrap.dedent(text).splitlines():
        if not line.strip() or line.strip().startswith("#"):
            continue
        out.append(re.sub(r"corde-server:[0-9][^\s\"']*", "corde-server:<version>", line.rstrip()))
    return out


def main() -> int:
    compose = normalise((ROOT / "packaging/docker/compose.yaml").read_text(encoding="utf-8"))
    guide = (ROOT / "docs/SYNOLOGY.md").read_text(encoding="utf-8")
    blocks = [b for b in re.findall(r"```yaml\n(.*?)```", guide, flags=re.S) if "services:" in b]
    if len(blocks) != 1:
        print(f"docs/SYNOLOGY.md : {len(blocks)} bloc(s) yaml avec « services: », attendu 1", file=sys.stderr)
        return 1
    pasted = normalise(blocks[0])
    if pasted != compose:
        print("docs/SYNOLOGY.md § 6 et packaging/docker/compose.yaml diffèrent :", file=sys.stderr)
        for a, b in zip(pasted + [""] * len(compose), compose + [""] * len(pasted)):
            if a != b:
                print(f"  guide   : {a!r}\n  compose : {b!r}", file=sys.stderr)
                break
        return 1
    print("guide et compose.yaml concordent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
