//! Min/max-preserving decimation for the graph API.
//!
//! The response of `lap_samples` shares one `t_s` / `lap_dist_m` axis between
//! every requested channel, so all channels must keep the **same** rows. We
//! split the lap into buckets and keep, per bucket, the row of the minimum
//! and the row of the maximum of *each* channel (plus the first and last row
//! of the lap). Peaks of every channel survive; the output never exceeds
//! `max_points` (modulo the two end points).

/// Indices to keep from `n` rows so that, for each channel, every bucket's
/// min and max are preserved. Returns an increasing, duplicate-free list.
pub fn select_indices(columns: &[&[Option<f64>]], n: usize, max_points: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let max_points = max_points.max(2);
    if n <= max_points {
        return (0..n).collect();
    }
    let k = columns.len().max(1);
    // Each bucket contributes up to 2 rows per channel; keep 2 rows for the ends.
    let buckets = ((max_points - 2) / (2 * k)).max(1);
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    for b in 0..buckets {
        let start = b * n / buckets;
        let end = ((b + 1) * n / buckets).min(n);
        if start >= end {
            continue;
        }
        for col in columns {
            let mut min_i = None;
            let mut max_i = None;
            let mut min_v = f64::INFINITY;
            let mut max_v = f64::NEG_INFINITY;
            for (i, v) in col.iter().enumerate().take(end).skip(start) {
                let Some(v) = v else { continue };
                if *v < min_v {
                    min_v = *v;
                    min_i = Some(i);
                }
                if *v > max_v {
                    max_v = *v;
                    max_i = Some(i);
                }
            }
            match (min_i, max_i) {
                (Some(a), Some(b)) => {
                    keep[a] = true;
                    keep[b] = true;
                }
                _ => {
                    // All null: keep the bucket's first row so the gap is visible.
                    keep[start] = true;
                }
            }
        }
    }
    keep.iter()
        .enumerate()
        .filter_map(|(i, k)| k.then_some(i))
        .collect()
}

/// Apply [`select_indices`] to a column.
pub fn take<T: Copy>(col: &[T], indices: &[usize]) -> Vec<T> {
    indices.iter().map(|&i| col[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_everything_when_small() {
        let c: Vec<Option<f64>> = (0..10).map(|i| Some(i as f64)).collect();
        assert_eq!(select_indices(&[&c], 10, 10), (0..10).collect::<Vec<_>>());
        assert!(select_indices(&[&c], 0, 10).is_empty());
    }

    #[test]
    fn preserves_extremes_of_every_channel_within_budget() {
        let n = 10_000;
        // Channel A: smooth sine with one sharp spike; channel B: sawtooth with a sharp dip.
        let mut a: Vec<Option<f64>> = (0..n)
            .map(|i| Some((i as f64 / 300.0).sin() * 100.0))
            .collect();
        a[6_789] = Some(1e6);
        let mut b: Vec<Option<f64>> = (0..n).map(|i| Some((i % 500) as f64)).collect();
        b[1_234] = Some(-1e6);
        b[42] = None;
        let max_points = 400;
        let idx = select_indices(&[&a, &b], n, max_points);
        assert!(idx.len() <= max_points + 2, "{}", idx.len());
        assert!(idx.len() > max_points / 2, "{}", idx.len());
        assert!(idx.windows(2).all(|w| w[1] > w[0]), "sorted, unique");
        assert_eq!(idx[0], 0);
        assert_eq!(*idx.last().unwrap(), n - 1);
        assert!(idx.contains(&6_789), "spike of A kept");
        assert!(idx.contains(&1_234), "dip of B kept");
        let ta = take(&a, &idx);
        let tb = take(&b, &idx);
        assert_eq!(ta.iter().flatten().cloned().fold(f64::MIN, f64::max), 1e6);
        assert_eq!(tb.iter().flatten().cloned().fold(f64::MAX, f64::min), -1e6);
        // Per-bucket min and max of A are all present.
        let buckets = (max_points - 2) / 4;
        for bkt in 0..buckets {
            let (s, e) = (bkt * n / buckets, (bkt + 1) * n / buckets);
            let (mut mi, mut ma) = (s, s);
            for i in s..e {
                if a[i] < a[mi] {
                    mi = i;
                }
                if a[i] > a[ma] {
                    ma = i;
                }
            }
            assert!(idx.contains(&mi) && idx.contains(&ma), "bucket {bkt}");
        }
    }

    #[test]
    fn all_null_bucket_keeps_a_row() {
        let c: Vec<Option<f64>> = vec![None; 1000];
        let idx = select_indices(&[&c], 1000, 20);
        assert!(idx.len() >= 2 && idx.len() <= 22);
    }
}
