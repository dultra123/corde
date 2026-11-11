//! Debug helper: prints every lap-related transition found in sample files.
//! `cargo run -p corde-store --example dump_boundaries -- <file.parquet>...`
use corde_store::{samples::read_columns, CHANNELS};
use std::path::Path;

fn main() {
    let names = [
        "t_s",
        "lap_number",
        "lap_dist_m",
        "speed_kmh",
        "in_pits",
        "in_pit_lane",
        "lap_invalidated",
        "current_sector",
        "fuel_l",
    ];
    let chans: Vec<_> = names
        .iter()
        .map(|n| CHANNELS.iter().find(|c| c.name == *n).expect("channel"))
        .collect();
    for arg in std::env::args().skip(1) {
        let cols = read_columns(Path::new(&arg), &chans, 0, u64::MAX).expect("read");
        let n = cols[0].len();
        println!("== {arg} rows={n}");
        let g = |c: usize, i: usize| cols[c][i].unwrap_or(f64::NAN);
        let mut max_dist = 0f64;
        for i in 0..n {
            max_dist = max_dist.max(g(2, i));
        }
        println!("   max lap_dist = {max_dist:.1}");
        let show = |i: usize, why: &str| {
            println!(
                "   row {i:>6} t={:>9.2} lap={:>3} dist={:>8.1} v={:>5.1} pits={} lane={} inval={} sect={} fuel={:.2}  <- {why}",
                g(0, i), g(1, i), g(2, i), g(3, i), g(4, i), g(5, i), g(6, i), g(7, i), g(8, i)
            );
        };
        if n == 0 {
            continue;
        }
        show(0, "first");
        for i in 1..n {
            let mut why = Vec::new();
            if g(1, i) != g(1, i - 1) {
                why.push("lap_number");
            }
            if g(2, i - 1) - g(2, i) > 0.5 * max_dist {
                why.push("dist wrap");
            } else if (g(2, i) - g(2, i - 1)).abs() > 150.0 {
                why.push("dist jump");
            }
            if g(4, i) != g(4, i - 1) {
                why.push("in_pits");
            }
            if g(5, i) != g(5, i - 1) {
                why.push("pit_lane");
            }
            if g(6, i) != g(6, i - 1) {
                why.push("invalidated");
            }
            if g(0, i) < g(0, i - 1) || g(0, i) - g(0, i - 1) > 1.0 {
                why.push("time gap");
            }
            if !why.is_empty() {
                show(i - 1, "before");
                show(i, &why.join("+"));
            }
        }
        show(n - 1, "last");
    }
}
