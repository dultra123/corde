#!/usr/bin/env python3
"""Numbered releases of Corde: version, release notes and the updater manifest (latest.json).

Used by the `version` job of .github/workflows/ci.yml (ubuntu-latest, Python 3 standard library
only). Every value that ends up in the release comes from the repository itself
(tauri.conf.json, CHANGELOG.md) or from the files built by the CI: nothing typed by a third party.

Commands
--------
  version CONF
      Print the application version of tauri.conf.json (`0.6.0`).

  release-state TAG < releases.json
      Read `gh release list --json tagName,isDraft` on stdin and print `published`, `draft`
      or `absent` for TAG.

  manifest --conf CONF --changelog CHANGELOG --setup SETUP --repo OWNER/NAME
           --out latest.json --notes-out notes.md [--pub-date RFC3339]
      Check the updater signature SETUP.sig (signed for this version and this file, by the key
      whose public half is in tauri.conf.json), then write the tauri-plugin-updater manifest
      and the release notes (the section of the version in CHANGELOG.md).

Exit status 1 with a `::error::` line (GitHub annotation, on stderr) on any inconsistency.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import datetime as dt
import json
import re
import sys
from pathlib import Path

# The updater looks for `windows-x86_64-nsis`, then `windows-x86_64` (tauri-plugin-updater 2.14,
# Updater::get_urls). The installer is the only Windows artifact.
PLATFORM = "windows-x86_64"
# File names that go into a download URL unescaped: what the Tauri bundler produces for
# `Corde_<version>_x64-setup.exe` and nothing else.
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?$")
REPO = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


class Failure(Exception):
    """A problem worth a clear message in the CI log."""


def read_version(conf_path: Path) -> str:
    conf = json.loads(conf_path.read_text(encoding="utf-8"))
    version = conf.get("version")
    if not isinstance(version, str) or not SEMVER.match(version):
        raise Failure(f"{conf_path}: `version` is not a plain semver version: {version!r}")
    return version


def read_pubkey(conf_path: Path) -> str:
    conf = json.loads(conf_path.read_text(encoding="utf-8"))
    try:
        key = conf["plugins"]["updater"]["pubkey"]
    except (KeyError, TypeError) as e:
        raise Failure(f"{conf_path}: plugins > updater > pubkey is missing") from e
    if not isinstance(key, str) or not key.strip():
        raise Failure(f"{conf_path}: plugins > updater > pubkey is empty")
    return key.strip()


def _b64(text: str, what: str) -> bytes:
    try:
        return base64.b64decode(text.strip(), validate=True)
    except (binascii.Error, ValueError) as e:
        raise Failure(f"{what} is not valid base64") from e


def key_id_of_pubkey(pubkey_b64: str) -> bytes:
    """Key id (8 bytes) of a Tauri public key: base64 of a minisign public key box."""
    box = _b64(pubkey_b64, "the public key").decode("utf-8", "replace").splitlines()
    if len(box) < 2:
        raise Failure("the public key is not a minisign public key box")
    raw = _b64(box[1], "the public key line")
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise Failure("the public key is not an Ed25519 minisign key")
    return raw[2:10]


def parse_signature(sig_b64: str) -> tuple[bytes, str]:
    """(key id, trusted comment) of a Tauri updater signature: base64 of a minisign
    signature box (4 lines: untrusted comment, signature, trusted comment, global signature).
    Only the structure is read here; the cryptographic check is the updater's job."""
    lines = _b64(sig_b64, "the signature").decode("utf-8", "replace").splitlines()
    if len(lines) < 4 or not lines[2].startswith("trusted comment: "):
        raise Failure("the signature is not a minisign signature box")
    raw = _b64(lines[1], "the signature line")
    if len(raw) != 74 or raw[:2] not in (b"Ed", b"ED"):
        raise Failure("the signature is not an Ed25519 minisign signature")
    return raw[2:10], lines[2][len("trusted comment: ") :]


def check_signature(sig_b64: str, pubkey_b64: str, version: str, file_name: str) -> None:
    sig_key, trusted = parse_signature(sig_b64)
    pub_key = key_id_of_pubkey(pubkey_b64)
    if sig_key != pub_key:
        raise Failure(
            "the updater signature was made with another key than the public key of "
            f"tauri.conf.json (signature key {sig_key[::-1].hex().upper()}, public key "
            f"{pub_key[::-1].hex().upper()}): check the TAURI_SIGNING_PRIVATE_KEY secret"
        )
    fields = dict(f.split(":", 1) for f in trusted.split("\t") if ":" in f)
    if fields.get("version") != version:
        # tauri.conf.json has `requireSignedVersion`: the updater would refuse it.
        raise Failure(
            f"the signature is for version {fields.get('version')!r}, not {version!r} "
            "(requireSignedVersion would reject it)"
        )
    if fields.get("file") != file_name:
        raise Failure(f"the signature is for file {fields.get('file')!r}, not {file_name!r}")


def changelog_section(changelog: str, version: str) -> str:
    """Body of the `## [x.y.z]` (or `## x.y.z`) section of CHANGELOG.md, up to the next
    `## ` heading, without the heading."""
    heading = re.compile(r"^##\s+\[?v?" + re.escape(version) + r"\]?(\s|$)")
    out: list[str] = []
    inside = False
    for line in changelog.splitlines():
        if line.startswith("## "):
            if inside:
                break
            inside = bool(heading.match(line))
            continue
        if inside:
            out.append(line.rstrip())
    body = "\n".join(out).strip()
    if not body:
        raise Failure(f"CHANGELOG.md has no section (or an empty one) for version {version}")
    return body


def rfc3339_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def build_manifest(
    version: str, notes: str, pub_date: str, signature: str, repo: str, file_name: str
) -> dict:
    if not REPO.match(repo):
        raise Failure(f"unexpected repository name {repo!r}")
    if not SAFE_NAME.match(file_name):
        raise Failure(f"installer name {file_name!r} would need escaping in a URL")
    try:
        dt.datetime.strptime(pub_date, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as e:
        raise Failure(f"pub_date {pub_date!r} is not RFC 3339 (UTC, Z)") from e
    url = f"https://github.com/{repo}/releases/download/v{version}/{file_name}"
    return {
        "version": version,
        "notes": notes,
        "pub_date": pub_date,
        "platforms": {PLATFORM: {"signature": signature, "url": url}},
    }


def release_state(releases: list, tag: str) -> str:
    for r in releases:
        if isinstance(r, dict) and r.get("tagName") == tag:
            return "draft" if r.get("isDraft") else "published"
    return "absent"


def cmd_manifest(a: argparse.Namespace) -> None:
    conf = Path(a.conf)
    version = read_version(conf)
    setup = Path(a.setup)
    expected = f"Corde_{version}_x64-setup.exe"
    if setup.name != expected:
        raise Failure(f"installer is named {setup.name!r}, expected {expected!r}")
    if not setup.is_file():
        raise Failure(f"{setup} not found")
    sig_path = setup.with_name(setup.name + ".sig")
    if not sig_path.is_file():
        raise Failure(f"{sig_path} not found: the updater signature was not produced")
    signature = sig_path.read_text(encoding="utf-8").strip()
    check_signature(signature, read_pubkey(conf), version, setup.name)
    notes = changelog_section(Path(a.changelog).read_text(encoding="utf-8"), version)
    manifest = build_manifest(
        version, notes, a.pub_date or rfc3339_now(), signature, a.repo, setup.name
    )
    Path(a.out).write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", "utf-8")
    Path(a.notes_out).write_text(notes + "\n", "utf-8")
    print(f"{a.out}: version {version}, {manifest['platforms'][PLATFORM]['url']}")


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    v = sub.add_parser("version")
    v.add_argument("conf")
    s = sub.add_parser("release-state")
    s.add_argument("tag")
    m = sub.add_parser("manifest")
    for flag in ("--conf", "--changelog", "--setup", "--repo", "--out", "--notes-out"):
        m.add_argument(flag, required=True)
    m.add_argument("--pub-date")
    a = p.parse_args(argv)
    try:
        if a.cmd == "version":
            print(read_version(Path(a.conf)))
        elif a.cmd == "release-state":
            print(release_state(json.load(sys.stdin), a.tag))
        else:
            cmd_manifest(a)
    except Failure as e:
        # stderr: still shown (and turned into an annotation) when stdout is captured by `$(…)`.
        print(f"::error::{e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
