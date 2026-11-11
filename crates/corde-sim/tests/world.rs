//! Behavioural tests of the simulator through its public API.

use corde_core::LapTracker;
use corde_protocol::{MotorState, TelemetryFrame, VehicleClass, PROTOCOL_VERSION};
use corde_sim::{PlayerClass, SimConfig, SimWorld};

fn cfg(seed: u64) -> SimConfig {
    SimConfig {
        seed,
        wall_base_ms: Some(1_700_000_000_000),
        ..SimConfig::default()
    }
}

fn run(cfg: SimConfig, n: usize) -> Vec<TelemetryFrame> {
    let mut w = SimWorld::new(cfg);
    (0..n).map(|_| w.tick()).collect()
}

#[test]
fn same_seed_same_frames() {
    let a = run(cfg(42), 600);
    let b = run(cfg(42), 600);
    assert_eq!(a.len(), 600);
    assert_eq!(
        a, b,
        "two worlds with the same seed must produce identical frames"
    );
    let a_json = serde_json::to_string(&a[599]).unwrap();
    let b_json = serde_json::to_string(&b[599]).unwrap();
    assert_eq!(a_json, b_json);

    let c = run(cfg(43), 600);
    assert_ne!(
        a[599].player, c[599].player,
        "a different seed must change the world"
    );
}

#[test]
fn lap_tracker_detects_a_lap_close_to_the_nominal_time() {
    let lap_time = 115.0f32;
    let c = SimConfig {
        lap_time_s: lap_time,
        rate_hz: 100.0,
        ..cfg(7)
    };
    let mut world = SimWorld::new(c);
    let mut tracker = LapTracker::new();
    let mut records = Vec::new();
    // 2.5 nominal laps → at least two completed laps.
    let ticks = (2.5 * lap_time * 100.0) as usize;
    for _ in 0..ticks {
        let f = world.tick();
        for r in tracker.feed(&f) {
            records.push(r);
        }
    }
    assert!(
        records.len() >= 2,
        "expected ≥ 2 lap records, got {}",
        records.len()
    );
    for r in &records {
        assert!(
            (r.lap_time_s - lap_time).abs() < 1.0,
            "lap {} took {} s (nominal {lap_time})",
            r.lap_number,
            r.lap_time_s
        );
        assert!(r.valid, "sim laps are valid");
        let (s1, s2, s3) = (r.s1.unwrap(), r.s2.unwrap(), r.s3.unwrap());
        assert!(
            ((s1 + s2 + s3) - r.lap_time_s).abs() < 0.05,
            "sectors must add up"
        );
        assert!(
            s1 > 20.0 && s2 > 20.0 && s3 > 30.0,
            "sectors {s1} {s2} {s3}"
        );
        assert!(
            r.fuel_used_l > 1.5 && r.fuel_used_l < 4.0,
            "fuel per lap {}",
            r.fuel_used_l
        );
        assert!(
            r.top_speed_kmh > 250.0 && r.top_speed_kmh <= 310.0,
            "top speed {}",
            r.top_speed_kmh
        );
        assert!(
            r.avg_tyre_temp_c.iter().all(|t| (70.0..=110.0).contains(t)),
            "avg tyre temps {:?}",
            r.avg_tyre_temp_c
        );
        assert!(r
            .max_brake_temp_c
            .iter()
            .all(|t| (250.0..=720.0).contains(t)));
        assert_eq!(r.front_compound, "Medium");
    }
    assert_eq!(records[1].lap_number, records[0].lap_number + 1);
}

#[test]
fn line_crossing_updates_lap_counters_and_last_lap() {
    let frames = run(
        SimConfig {
            rate_hz: 50.0,
            ..cfg(3)
        },
        50 * 120,
    );
    let first = frames[0].player.as_ref().unwrap();
    let last = frames.last().unwrap().player.as_ref().unwrap();
    assert_eq!(
        last.lap.lap_number,
        first.lap.lap_number + 1,
        "exactly one line crossing in 120 s"
    );
    assert_eq!(last.lap.laps_completed, first.lap.laps_completed + 1);
    let lap = last.lap.last_lap_s.expect("last lap set after the line");
    assert!((lap - 115.0).abs() < 1.0, "last lap {lap}");
    assert!(last.lap.best_lap_s.unwrap() <= lap + 1e-3);
    assert!(last.lap.lap_start_s > first.lap.lap_start_s);
    assert!(last.lap.current_lap_s < 10.0);
    // Live delta exists once a best lap exists and stays small on a steady lap.
    assert!(last.lap.delta_best_s.abs() < 3.0);
}

#[test]
fn standings_are_sorted_and_consistent() {
    for f in run(cfg(11), 1500).iter().step_by(250) {
        let st = &f.standings;
        assert_eq!(st.len(), 20, "19 AI + player");
        assert_eq!(f.session.num_vehicles, 20);
        let mut by_class = std::collections::HashMap::<VehicleClass, u8>::new();
        for (i, s) in st.iter().enumerate() {
            assert_eq!(s.place as usize, i + 1, "standings must be sorted by place");
            let expected_class_place = by_class.entry(s.vehicle_class).or_insert(0);
            *expected_class_place += 1;
            assert_eq!(
                s.class_place, *expected_class_place,
                "class_place of {}",
                s.driver_name
            );
            assert!(!s.driver_name.is_empty() && !s.vehicle_name.is_empty());
            assert!((0.0..=1.0).contains(&s.lap_fraction));
            assert!(s.lap_dist_m >= 0.0 && s.lap_dist_m <= f.session.track_length_m);
            assert!((1..=3).contains(&s.sector));
            if i > 0 {
                let prev = &st[i - 1];
                let behind =
                    (prev.laps_completed, prev.lap_fraction) >= (s.laps_completed, s.lap_fraction);
                assert!(
                    behind,
                    "{} (P{}) must be behind {}",
                    s.driver_name, s.place, prev.driver_name
                );
                assert!(s.time_behind_next_s >= 0.0 && s.time_behind_leader_s >= 0.0);
                assert!(s.laps_behind_leader >= s.laps_behind_next);
            } else {
                assert_eq!(s.time_behind_leader_s, 0.0);
            }
        }
        let players: Vec<_> = st.iter().filter(|s| s.is_player).collect();
        assert_eq!(players.len(), 1);
        let p = f.player.as_ref().unwrap();
        assert_eq!(players[0].place, p.place);
        assert!(
            by_class.len() == 3,
            "classes must be mixed: {:?}",
            by_class.keys().collect::<Vec<_>>()
        );
        assert!(
            by_class[&VehicleClass::Hypercar] >= 4
                && by_class[&VehicleClass::Lmp2] >= 3
                && by_class[&VehicleClass::Gt3] >= 6
        );
        // Hypercars are faster: the leader is one, after a couple of laps.
        assert_eq!(st[0].vehicle_class, VehicleClass::Hypercar);
    }
}

#[test]
fn physical_quantities_stay_in_range() {
    let frames = run(cfg(5), 100 * 130);
    let mut speed_min = f32::MAX;
    let mut speed_max = 0.0f32;
    let mut gears = std::collections::BTreeSet::new();
    let mut fuel_prev = f32::MAX;
    let mut tyre_hot = 0.0f32;
    let mut tyre_cold = f32::MAX;
    for f in &frames {
        assert_eq!(f.v, PROTOCOL_VERSION);
        assert_eq!(f.source, "sim");
        assert!(f.wall_ms >= 1_700_000_000_000);
        let p = f.player.as_ref().expect("player always present in the sim");
        let m = &p.motion;
        speed_min = speed_min.min(m.speed_kmh);
        speed_max = speed_max.max(m.speed_kmh);
        assert!((m.speed_ms * 3.6 - m.speed_kmh).abs() < 0.01);
        assert!(
            m.g_lat.abs() < 2.6 && m.g_long.abs() < 2.2,
            "g {} {}",
            m.g_lat,
            m.g_long
        );
        assert!(m.heading_rad.is_finite());

        let e = &p.engine;
        assert!(e.rpm >= 1000.0 && e.rpm <= e.max_rpm, "rpm {}", e.rpm);
        assert_eq!(e.max_rpm, 8500.0);
        assert!((1..=6).contains(&e.gear));
        assert_eq!(e.max_gears, 6);
        gears.insert(e.gear);
        assert!((80.0..=100.0).contains(&e.water_c) && (95.0..=125.0).contains(&e.oil_c));

        let i = &p.inputs;
        assert!((0.0..=1.0).contains(&i.throttle) && (0.0..=1.0).contains(&i.brake));
        assert!(
            !(i.throttle > 0.3 && i.brake > 0.3),
            "no simultaneous throttle and brake"
        );
        assert!((-1.0..=1.0).contains(&i.steering));
        assert_eq!(i.wheel_range_deg, 540.0);
        assert!((i.steering_angle_deg - i.steering * 270.0).abs() < 0.01);

        for w in &p.wheels {
            for t in w.temp_c {
                assert!((60.0..=115.0).contains(&t), "tyre temp {t}");
                tyre_hot = tyre_hot.max(t);
                tyre_cold = tyre_cold.min(t);
            }
            assert!(
                (145.0..=180.0).contains(&w.pressure_kpa),
                "pressure {}",
                w.pressure_kpa
            );
            assert!(
                (150.0..=720.0).contains(&w.brake_temp_c),
                "brake {}",
                w.brake_temp_c
            );
            assert!(
                (0.9..=1.0).contains(&w.wear_remaining),
                "wear {}",
                w.wear_remaining
            );
            assert!((0.0..=1.0).contains(&w.grip_fract));
            assert!(
                w.load_n >= 100.0 && w.load_n < 20_000.0,
                "load {}",
                w.load_n
            );
            assert!(w.slip_angle_deg.abs() <= 6.0 && w.slip_ratio.abs() <= 0.1);
        }
        let en = &p.energy;
        assert!(en.fuel_l <= fuel_prev + 1e-6, "fuel must not increase");
        fuel_prev = en.fuel_l;
        assert!(en.fuel_l > 50.0 && en.fuel_capacity_l == 120.0);
        assert!((en.fuel_fraction - en.fuel_l / en.fuel_capacity_l).abs() < 1e-4);
        assert!(
            en.virtual_energy.is_none() && en.battery_soc.is_none(),
            "GT3 has no hybrid"
        );
        assert_eq!(en.motor_state, MotorState::Unavailable);

        let l = &p.lap;
        assert!((1..=3).contains(&l.current_sector));
        assert!((0.0..=1.0).contains(&l.lap_fraction));
        assert!((l.lap_fraction * f.session.track_length_m - l.lap_dist_m).abs() < 0.01);
        assert!(l.current_lap_s >= 0.0 && l.current_lap_s < 130.0);

        let wx = &f.session.weather;
        assert!((20.0..=27.0).contains(&wx.ambient_c) && (27.0..=35.0).contains(&wx.track_c));
        assert_eq!(wx.rain, 0.0);
        assert_eq!(p.vehicle_model, "Mercedes-AMG GT3");
        assert_eq!(p.vehicle_class, VehicleClass::Gt3);
    }
    assert!((75.0..100.0).contains(&speed_min), "speed min {speed_min}");
    assert!(
        speed_max > 250.0 && speed_max <= 305.0,
        "speed max {speed_max}"
    );
    assert_eq!(gears.len(), 6, "all six gears used over a lap: {gears:?}");
    assert!(
        tyre_hot - tyre_cold > 8.0,
        "tyres must warm up in corners ({tyre_cold} → {tyre_hot})"
    );
    let first_fuel = frames[0].player.as_ref().unwrap().energy.fuel_l;
    let last_fuel = frames
        .last()
        .unwrap()
        .player
        .as_ref()
        .unwrap()
        .energy
        .fuel_l;
    let used = first_fuel - last_fuel;
    assert!(used > 2.0 && used < 4.0, "≈ 2.6–3 L per lap, used {used}");
}

#[test]
fn hypercar_has_hybrid_fields_and_rain_wets_the_track() {
    let c = SimConfig {
        player_class: PlayerClass::Hypercar,
        rain: 0.6,
        lap_time_s: 97.0,
        ..cfg(9)
    };
    let frames = run(c, 100 * 20);
    let f = frames.last().unwrap();
    let p = f.player.as_ref().unwrap();
    assert_eq!(p.vehicle_class, VehicleClass::Hypercar);
    assert_eq!(p.engine.max_gears, 7);
    let ve = p
        .energy
        .virtual_energy
        .expect("virtual energy for Hypercar");
    assert!(ve > 0.5 && ve <= 1.0, "ve {ve}");
    let soc = p.energy.battery_soc.expect("battery soc");
    assert!((0.1..=1.0).contains(&soc));
    assert!(p.energy.regen_kw.is_some());
    assert!(p.electronics.motor_map_max > 0);
    assert!(f.session.weather.rain > 0.5 && f.session.weather.wetness_avg > 0.3);
    assert!(f.session.weather.track_c < 30.0);
    assert_eq!(p.electronics.wiper, 1);
    assert!(p
        .wheels
        .iter()
        .all(|w| w.surface == corde_protocol::SurfaceType::Wet));
    // Rain slows the field: the first lap takes longer than 97 s.
    let mut w = SimWorld::new(SimConfig {
        player_class: PlayerClass::Hypercar,
        rain: 0.6,
        lap_time_s: 97.0,
        ..cfg(9)
    });
    let mut last = None;
    for _ in 0..(100 * 115) {
        let fr = w.tick();
        if let Some(t) = fr.player.as_ref().unwrap().lap.last_lap_s {
            if fr.player.as_ref().unwrap().lap.laps_completed > 2 {
                last = Some(t);
                break;
            }
        }
    }
    let t = last.expect("one lap in 115 s");
    assert!(t > 100.0 && t < 110.0, "wet lap {t}");
}

#[test]
fn no_ai_field_still_works() {
    let frames = run(
        SimConfig {
            num_ai: 0,
            ..cfg(1)
        },
        50,
    );
    let f = frames.last().unwrap();
    assert_eq!(f.standings.len(), 1);
    assert!(f.standings[0].is_player);
    let p = f.player.as_ref().unwrap();
    assert_eq!(p.place, 1);
    assert!(p.gaps.car_ahead_s.is_none() && p.gaps.place_behind_s.is_none());
}
