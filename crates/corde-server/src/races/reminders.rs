//! Reminders of the picked races: when `start − delay` comes, every dashboard
//! receives a `race_reminder` message (banner + sound in the UI).
//!
//! [`due_reminders`] is the whole rule, pure and tested;
//! [`super::run_periodic`] calls it every few seconds and remembers what it sent, on disk, so a restart
//! does not send a reminder twice.

use super::store::Pick;
use corde_protocol::{RaceReminder, RaceReminderKind};
use std::collections::BTreeSet;

/// A reminder more than this late is not sent any more (the server was off).
pub const GRACE_MS: i64 = 60_000;

/// One reminder to send now, and the key that marks it as sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub key: String,
    pub reminder: RaceReminder,
}

/// Every reminder of `picks` due at `now_ms`: its time has come, it is at most
/// [`GRACE_MS`] late and it was not sent yet. Races: one per delay of `leads`
/// (`start − delay`). Special events: the evening before (`eve_ms`, when the
/// pick has one) and the day itself (`start_ms`). Sorted by due time.
pub fn due_reminders(
    picks: &[Pick],
    leads: &[u32],
    now_ms: i64,
    fired: &BTreeSet<String>,
) -> Vec<Due> {
    let mut out = Vec::new();
    for p in picks {
        let mut slots: Vec<(String, i64, RaceReminderKind, Option<u32>)> = Vec::new();
        if p.cat == "special" {
            if let Some(eve) = p.eve_ms {
                slots.push(("eve".into(), eve, RaceReminderKind::Eve, None));
            }
            slots.push(("day".into(), p.start_ms, RaceReminderKind::Day, None));
        } else {
            for &lead in leads {
                slots.push((
                    lead.to_string(),
                    p.start_ms - i64::from(lead) * 60_000,
                    RaceReminderKind::Lead,
                    Some(lead),
                ));
            }
        }
        for (tag, at, kind, lead_min) in slots {
            let key = format!("{}#{tag}", p.id);
            if at > now_ms || now_ms - at > GRACE_MS || fired.contains(&key) {
                continue;
            }
            out.push(Due {
                key,
                reminder: RaceReminder {
                    pick_id: p.id.clone(),
                    race_id: p.race_id.clone(),
                    name: p.name.clone(),
                    track: p.track.clone(),
                    cat: p.cat.clone(),
                    level: p.level.clone(),
                    start_ms: p.start_ms,
                    kind,
                    lead_min,
                    due_ms: at,
                },
            });
        }
    }
    out.sort_by(|a, b| {
        a.reminder
            .due_ms
            .cmp(&b.reminder.due_ms)
            .then(a.key.cmp(&b.key))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::races::store::pick_id;

    const START: i64 = 1_762_892_700_000; // 2026-11-11 20:25 UTC
    const MIN: i64 = 60_000;

    fn pick(cat: &str, start: i64, eve: Option<i64>) -> Pick {
        Pick {
            id: pick_id("race", start),
            race_id: "race".into(),
            cat: cat.into(),
            src: "rc".into(),
            name: "LMP3 Fixed".into(),
            level: Some("beginner".into()),
            track: "Monza".into(),
            duration: "32 min".into(),
            image: None,
            start_ms: start,
            eve_ms: eve,
        }
    }

    fn keys(d: &[Due]) -> Vec<&str> {
        d.iter().map(|d| d.key.as_str()).collect()
    }

    #[test]
    fn a_race_is_reminded_once_per_delay() {
        let picks = [pick("daily", START, None)];
        let leads = [15, 5, 0];
        let none = BTreeSet::new();
        assert!(due_reminders(&picks, &leads, START - 16 * MIN, &none).is_empty());
        let d = due_reminders(&picks, &leads, START - 15 * MIN, &none);
        assert_eq!(keys(&d), [format!("{}#15", picks[0].id)]);
        let r = &d[0].reminder;
        assert_eq!(r.kind, RaceReminderKind::Lead);
        assert_eq!(r.lead_min, Some(15));
        assert_eq!(r.due_ms, START - 15 * MIN);
        assert_eq!(r.start_ms, START);
        assert_eq!(r.name, "LMP3 Fixed");
        // 59 s late: still sent; already sent: not again.
        assert_eq!(
            due_reminders(&picks, &leads, START - 15 * MIN + 59_000, &none).len(),
            1
        );
        let fired: BTreeSet<String> = d.iter().map(|d| d.key.clone()).collect();
        assert!(due_reminders(&picks, &leads, START - 15 * MIN + 1000, &fired).is_empty());
        // At the start.
        let d = due_reminders(&picks, &leads, START, &fired);
        assert_eq!(keys(&d), [format!("{}#0", picks[0].id)]);
        assert_eq!(d[0].reminder.lead_min, Some(0));
    }

    #[test]
    fn nothing_for_a_reminder_more_than_a_minute_late() {
        // The server was off at 20:10 and comes back at 20:11:01: the 15 min
        // reminder is dropped, the 5 min one will come.
        let picks = [pick("daily", START, None)];
        let none = BTreeSet::new();
        let d = due_reminders(&picks, &[15, 5], START - 15 * MIN + GRACE_MS + 1000, &none);
        assert!(d.is_empty());
        let d = due_reminders(&picks, &[15, 5], START - 5 * MIN, &none);
        assert_eq!(d.len(), 1);
        // A pick made 3 minutes before the start: no 15 / 5 min reminder at all.
        assert!(due_reminders(&picks, &[15, 5], START - 3 * MIN, &none).is_empty());
    }

    #[test]
    fn special_events_the_evening_before_and_the_day() {
        let day = START + 86_400_000;
        let eve = day - 16 * 3_600_000;
        let picks = [pick("special", day, Some(eve))];
        let none = BTreeSet::new();
        // Delays do not apply to special events.
        assert!(due_reminders(&picks, &[15], day - 15 * MIN, &none).is_empty());
        let d = due_reminders(&picks, &[15], eve, &none);
        assert_eq!(keys(&d), [format!("{}#eve", picks[0].id)]);
        assert_eq!(d[0].reminder.kind, RaceReminderKind::Eve);
        assert_eq!(d[0].reminder.lead_min, None);
        let d = due_reminders(&picks, &[15], day + 30_000, &none);
        assert_eq!(keys(&d), [format!("{}#day", picks[0].id)]);
        assert_eq!(d[0].reminder.kind, RaceReminderKind::Day);
    }

    #[test]
    fn several_picks_come_out_in_time_order() {
        let a = pick("daily", START + 10 * MIN, None);
        let b = pick("daily", START, None);
        let d = due_reminders(&[a.clone(), b.clone()], &[10, 0], START, &BTreeSet::new());
        assert_eq!(keys(&d), [format!("{}#0", b.id), format!("{}#10", a.id)]);
    }
}
