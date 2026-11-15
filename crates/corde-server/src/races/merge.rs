//! « Mes courses » shared between devices (ADR 0019): the rule that merges two copies of
//! the picks, wherever they come from — the PC, the home server, the phone.
//!
//! Each copy is a [`PickSet`]: the picks with the time of their last change, and the picks
//! that were removed with the time of their removal (kept until the start is three hours
//! old, like a pick). [`merge`] is the whole rule, pure:
//!
//! * for one identifier, **the latest change wins**; at the same instant a removal wins over
//!   a pick, and between two picks a fixed order of their fields decides (so that two devices
//!   always end up with the same one);
//! * what started more than [`KEEP_AFTER_START_MS`] ago is dropped;
//! * at most [`MAX_PICKS`] picks (the earliest starts) and [`MAX_REMOVED`] removals (the
//!   latest ones).
//!
//! The order of the two copies does not matter, merging twice changes nothing, and merging
//! three copies gives the same result in any order (below the two limits). The same rule is
//! written in TypeScript for the phone (`packages/ui/src/races/picks-merge.ts`); both are
//! checked against the same cases, `merge-vectors.json`.
//!
//! The instants are clocks of the devices. A device that changes a pick stamps it later than
//! what it knows of this pick (`known + 1` at least): its own change always wins over what
//! it has seen, even if another device's clock runs ahead.

use super::store::{valid_pick_id, Pick, KEEP_AFTER_START_MS, MAX_PICKS};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;

/// Most removals remembered at once.
pub const MAX_REMOVED: usize = 200;
/// Largest body of `POST /api/races/picks/sync`.
pub const MAX_SYNC_BODY: usize = 256 * 1024;
/// Latest instant accepted for a change (2100-01-01).
const MAX_STAMP_MS: i64 = 4_102_444_800_000;

/// A pick and the instant of its last change (0: picked before the picks were shared).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamped {
    pub pick: Pick,
    pub changed_ms: i64,
}

/// A pick that was removed, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removed {
    pub id: String,
    pub removed_ms: i64,
}

/// One copy of « Mes courses »: what `POST /api/races/picks/sync` receives and answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PickSet {
    /// By start, then identifier.
    pub picks: Vec<Stamped>,
    /// By identifier.
    pub removed: Vec<Removed>,
}

/// The start written in a pick identifier (`<race_id>@<start_ms>`).
pub fn start_of(id: &str) -> Option<i64> {
    id.rsplit_once('@').and_then(|(_, s)| s.parse().ok())
}

fn valid_stamp(ms: i64) -> bool {
    (0..=MAX_STAMP_MS).contains(&ms)
}

// One entry per pick, at most a few hundred, short-lived: not worth a box.
#[allow(clippy::large_enum_variant)]
enum Entry {
    Live(Stamped),
    Gone(i64),
}

impl Entry {
    fn stamp(&self) -> i64 {
        match self {
            Entry::Live(s) => s.changed_ms,
            Entry::Gone(ms) => *ms,
        }
    }
}

/// The order that decides between two picks changed at the same instant.
fn tie_key(p: &Pick) -> (&str, &str, &str, &str, &str, &str, &str, i64) {
    (
        &p.name,
        &p.track,
        &p.duration,
        p.image.as_deref().unwrap_or(""),
        &p.cat,
        &p.src,
        p.level.as_deref().unwrap_or(""),
        p.eve_ms.unwrap_or(-1),
    )
}

/// `new` replaces `old` for the same identifier.
fn wins(new: &Entry, old: &Entry) -> bool {
    if new.stamp() != old.stamp() {
        return new.stamp() > old.stamp();
    }
    match (new, old) {
        (Entry::Gone(_), Entry::Live(_)) => true,
        (Entry::Live(a), Entry::Live(b)) => tie_key(&a.pick) > tie_key(&b.pick),
        _ => false,
    }
}

fn offer(into: &mut BTreeMap<String, Entry>, id: String, e: Entry) {
    match into.get(&id) {
        Some(old) if !wins(&e, old) => {}
        _ => {
            into.insert(id, e);
        }
    }
}

fn fold(into: &mut BTreeMap<String, Entry>, set: PickSet) {
    for s in set.picks {
        offer(into, s.pick.id.clone(), Entry::Live(s));
    }
    for r in set.removed {
        offer(into, r.id, Entry::Gone(r.removed_ms));
    }
}

/// The two copies as one (see the module documentation), as it stands at `now_ms`.
pub fn merge(a: PickSet, b: PickSet, now_ms: i64) -> PickSet {
    let mut all = BTreeMap::new();
    fold(&mut all, a);
    fold(&mut all, b);
    let limit = now_ms - KEEP_AFTER_START_MS;
    let mut out = PickSet::default();
    for (id, e) in all {
        match e {
            Entry::Live(s) if s.pick.start_ms > limit => out.picks.push(s),
            Entry::Gone(ms) if start_of(&id).is_some_and(|start| start > limit) => {
                out.removed.push(Removed { id, removed_ms: ms })
            }
            _ => {}
        }
    }
    out.picks.sort_by(|x, y| {
        x.pick
            .start_ms
            .cmp(&y.pick.start_ms)
            .then_with(|| x.pick.id.cmp(&y.pick.id))
    });
    out.picks.truncate(MAX_PICKS);
    if out.removed.len() > MAX_REMOVED {
        out.removed.sort_by(|x, y| {
            y.removed_ms
                .cmp(&x.removed_ms)
                .then_with(|| x.id.cmp(&y.id))
        });
        out.removed.truncate(MAX_REMOVED);
        out.removed.sort_by(|x, y| x.id.cmp(&y.id));
    }
    out
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn entries<'a>(obj: &'a Value, key: &str, max: usize) -> io::Result<&'a [Value]> {
    match obj.get(key) {
        None => Ok(&[]),
        Some(Value::Array(items)) if items.len() <= max => Ok(items),
        Some(Value::Array(_)) => Err(bad(format!("`{key}` : au plus {max} éléments"))),
        Some(_) => Err(bad(format!("`{key}` : liste attendue"))),
    }
}

impl PickSet {
    /// The copy sent by another device (`POST /api/races/picks/sync`) or read from disk.
    ///
    /// The envelope is strict — an object, two bounded lists — and each entry is checked like
    /// a pick sent on its own. An entry that does not pass is **left out**, not refused: a
    /// device of another version may know a field this one does not, and the rest of its
    /// picks must still be shared. Returns the copy and how many entries were left out.
    pub fn from_wire(body: &[u8]) -> io::Result<(Self, usize)> {
        if body.len() > MAX_SYNC_BODY {
            return Err(bad("courses : envoi trop gros"));
        }
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| bad(format!("courses : JSON invalide ({e})")))?;
        if !value.is_object() {
            return Err(bad("courses : un objet {picks, removed} est attendu"));
        }
        let mut set = PickSet::default();
        let mut skipped = 0;
        for v in entries(&value, "picks", MAX_PICKS)? {
            match serde_json::from_value::<Stamped>(v.clone()) {
                Ok(s) if s.pick.validate().is_ok() && valid_stamp(s.changed_ms) => {
                    set.picks.push(s)
                }
                _ => skipped += 1,
            }
        }
        for v in entries(&value, "removed", MAX_REMOVED)? {
            match serde_json::from_value::<Removed>(v.clone()) {
                Ok(r) if valid_pick_id(&r.id) && valid_stamp(r.removed_ms) => set.removed.push(r),
                _ => skipped += 1,
            }
        }
        Ok((set, skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::races::store::pick_id;

    const NOW: i64 = 1_762_891_800_000; // 2026-11-11 20:10 UTC
    const H: i64 = 3_600_000;

    fn pick(race: &str, start: i64) -> Pick {
        Pick {
            id: pick_id(race, start),
            race_id: race.into(),
            cat: "daily".into(),
            src: "rc".into(),
            name: "LMP3 Fixed".into(),
            level: Some("beginner".into()),
            track: "Monza".into(),
            duration: "32 min".into(),
            image: None,
            start_ms: start,
            eve_ms: None,
        }
    }

    fn live(race: &str, start: i64, changed_ms: i64) -> Stamped {
        Stamped {
            pick: pick(race, start),
            changed_ms,
        }
    }

    fn gone(race: &str, start: i64, removed_ms: i64) -> Removed {
        Removed {
            id: pick_id(race, start),
            removed_ms,
        }
    }

    fn set(picks: Vec<Stamped>, removed: Vec<Removed>) -> PickSet {
        PickSet { picks, removed }
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        now: i64,
        a: PickSet,
        b: PickSet,
        want: PickSet,
    }

    #[derive(Deserialize)]
    struct Vectors {
        keep_after_start_ms: i64,
        max_picks: usize,
        max_removed: usize,
        cases: Vec<Case>,
    }

    /// The cases shared with the TypeScript rule (`packages/ui/src/races/picks-merge.test.ts`).
    #[test]
    fn the_shared_cases_give_the_same_result_in_both_orders() {
        let v: Vectors = serde_json::from_str(include_str!("merge-vectors.json")).unwrap();
        assert_eq!(v.keep_after_start_ms, KEEP_AFTER_START_MS);
        assert_eq!(v.max_picks, MAX_PICKS);
        assert_eq!(v.max_removed, MAX_REMOVED);
        assert!(v.cases.len() >= 10);
        for c in v.cases {
            let ab = merge(c.a.clone(), c.b.clone(), c.now);
            assert_eq!(ab, c.want, "{} (a, b)", c.name);
            assert_eq!(
                merge(c.b.clone(), c.a.clone(), c.now),
                c.want,
                "{} (b, a)",
                c.name
            );
            assert_eq!(
                merge(ab.clone(), c.a, c.now),
                c.want,
                "{} (again, a)",
                c.name
            );
            assert_eq!(merge(ab, c.b, c.now), c.want, "{} (again, b)", c.name);
        }
    }

    #[test]
    fn three_copies_merge_the_same_in_any_order() {
        let a = set(
            vec![live("one", NOW + H, 10), live("two", NOW + 2 * H, 50)],
            vec![gone("three", NOW + 3 * H, 70)],
        );
        let b = set(
            vec![live("three", NOW + 3 * H, 60), live("four", NOW + 4 * H, 5)],
            vec![gone("one", NOW + H, 20)],
        );
        let c = set(
            vec![live("one", NOW + H, 30), live("three", NOW + 3 * H, 70)],
            vec![gone("two", NOW + 2 * H, 40), gone("four", NOW + 4 * H, 5)],
        );
        let m = |x: &PickSet, y: &PickSet| merge(x.clone(), y.clone(), NOW);
        let want = m(&m(&a, &b), &c);
        assert_eq!(m(&a, &m(&b, &c)), want);
        assert_eq!(m(&m(&c, &a), &b), want);
        assert_eq!(m(&m(&b, &c), &a), want);
        // one: picked again at 30 after the removal at 20; two: the pick at 50 is later than
        // the removal at 40; three: removed at 70, the same instant as a pick → removed;
        // four: removed at 5, the same instant as its pick → removed.
        let ids: Vec<&str> = want.picks.iter().map(|s| s.pick.race_id.as_str()).collect();
        assert_eq!(ids, ["one", "two"]);
        let gone_ids: Vec<&str> = want.removed.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            gone_ids,
            [pick_id("four", NOW + 4 * H), pick_id("three", NOW + 3 * H)]
        );
    }

    #[test]
    fn the_limits_keep_the_earliest_picks_and_the_latest_removals() {
        let picks = (0..MAX_PICKS as i64 + 5)
            .map(|i| live("race", NOW + H + i * 60_000, i))
            .collect();
        let removed = (0..MAX_REMOVED as i64 + 5)
            .map(|i| gone("gone", NOW + H + i * 60_000, i))
            .collect();
        let m = merge(set(picks, removed), PickSet::default(), NOW);
        assert_eq!(m.picks.len(), MAX_PICKS);
        assert_eq!(m.picks[0].pick.start_ms, NOW + H);
        assert_eq!(
            m.picks.last().unwrap().pick.start_ms,
            NOW + H + (MAX_PICKS as i64 - 1) * 60_000
        );
        assert_eq!(m.removed.len(), MAX_REMOVED);
        assert!(m.removed.iter().all(|r| r.removed_ms >= 5));
        assert!(m.removed.windows(2).all(|w| w[0].id < w[1].id));
    }

    #[test]
    fn the_wire_copy_is_bounded_and_leaves_out_what_is_invalid() {
        let good = live("one", NOW + H, 12);
        let body = serde_json::json!({
            "picks": [
                good,
                { "pick": pick("two", NOW + H), "changed_ms": -1 },
                { "pick": { "id": "x@1" }, "changed_ms": 1 },
                { "pick": pick("three", NOW + H), "changed_ms": 1, "more": true },
                "text",
            ],
            "removed": [
                gone("four", NOW + H, 3),
                { "id": "../x@1", "removed_ms": 3 },
                { "id": pick_id("five", NOW + H), "removed_ms": 5_000_000_000_000_i64 },
            ],
            "later": "ignored"
        });
        let (set, skipped) = PickSet::from_wire(body.to_string().as_bytes()).unwrap();
        // `more` is an unknown key of the envelope of one entry: serde ignores it there (only
        // the pick itself refuses unknown fields), so « three » is kept.
        assert_eq!(set.picks.len(), 2);
        assert_eq!(set.picks[0], good);
        assert_eq!(set.removed, vec![gone("four", NOW + H, 3)]);
        assert_eq!(skipped, 5);

        let (empty, skipped) = PickSet::from_wire(b"{}").unwrap();
        assert_eq!((empty, skipped), (PickSet::default(), 0));
        for bad_body in [
            &b"[]"[..],
            b"not json",
            br#"{"picks":{}}"#,
            br#"{"removed":1}"#,
        ] {
            assert!(PickSet::from_wire(bad_body).is_err());
        }
        let many: Vec<Stamped> = (0..MAX_PICKS as i64 + 1)
            .map(|i| live("race", NOW + H + i, 0))
            .collect();
        let too_many = serde_json::json!({ "picks": many }).to_string();
        assert!(PickSet::from_wire(too_many.as_bytes()).is_err());
        let huge = format!(r#"{{"picks":[],"pad":"{}"}}"#, "x".repeat(MAX_SYNC_BODY));
        assert!(PickSet::from_wire(huge.as_bytes()).is_err());
    }
}
