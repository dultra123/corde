//! A plausible mapping of the game, for the tests and the development tools: a car going
//! round a circuit at constant speed, the lap counters and times published the way the
//! game publishes them (the time of a lap a moment after the line), another car ahead.
//!
//! Every name in it is invented.

use crate::layout::{flags, session_state, track_loc, VarType, MAX_CARS, STATUS_CONNECTED};
use crate::reader::Image;
use crate::vars::VarTable;

/// Index of the player's car, of the car ahead, of the pace car and of a spectator.
pub const PLAYER: usize = 2;
pub const RIVAL: usize = 1;
pub const PACE_CAR: usize = 0;
pub const SPECTATOR: usize = 3;

#[derive(Debug, Clone)]
pub struct FakeSession {
    /// What a lap lasts, seconds.
    pub lap_s: f64,
    pub track_m: f32,
    /// 0: the practice, 1: the race (see [`session_text`]).
    pub session_num: i32,
    /// Where on the lap the car is at `t = 0`.
    pub start_fraction: f64,
    /// `Lap` at `t = 0`.
    pub first_lap: i32,
    /// `LapCompleted` − `Lap` (−1: the lap being driven is not completed).
    pub completed_offset: i32,
    /// How long after the line the game publishes the time of the lap.
    pub last_time_delay_s: f64,
    /// Line crossings (1: the first after `t = 0`) whose lap the game refuses to time.
    pub refused: Vec<i32>,
    /// By how much the published times differ from lap to lap (0: every lap gets exactly
    /// the same time).
    pub time_step_s: f64,
    /// Session clock at `t = 0`.
    pub clock_at_start: f64,
    pub connected: bool,
    pub on_track: bool,
    /// The car is out of the world (garage): the counters of the game read 0.
    pub in_world: bool,
    pub incidents: i32,
    pub session_flags: u32,
    pub info_update: i32,
}

impl Default for FakeSession {
    fn default() -> Self {
        Self {
            lap_s: 100.0,
            track_m: 6930.0,
            session_num: 0,
            start_fraction: 0.5,
            first_lap: 0,
            completed_offset: -1,
            last_time_delay_s: 0.5,
            refused: Vec::new(),
            time_step_s: 0.003,
            clock_at_start: 500.0,
            connected: true,
            on_track: true,
            in_world: true,
            incidents: 0,
            session_flags: 0,
            info_update: 1,
        }
    }
}

/// The session text of [`FakeSession`], as the game writes one (invented names).
pub fn session_text(fake: &FakeSession) -> String {
    format!(
        r#"---
WeekendInfo:
 TrackName: spa up
 TrackID: 163
 TrackLength: {km:.2} km
 TrackDisplayName: Circuit de Spa-Francorchamps
 TrackConfigName: Grand Prix Pits
 SubSessionID: 0
 BuildVersion: 2026.11.04.01
 WeekendOptions:
  NumStarters: 3
  IsFixedSetup: 0
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: unlimited
   SessionType: Practice
   SessionName: PRACTICE
 - SessionNum: 1
   SessionLaps: 12
   SessionTime: 1800.0000 sec
   SessionType: Race
   SessionName: RACE
DriverInfo:
 DriverCarIdx: {player}
 DriverUserID: 424242
 PaceCarIdx: {pace}
 DriverCarIdleRPM: 1000.000
 DriverCarRedLine: 7500.000
 DriverCarFuelMaxLtr: 120.000
 DriverCarMaxFuelPct: 1.000
 DriverCarGearNumForward: 6
 DriverCarEstLapTime: {lap:.4}
 DriverSetupName: essai: "pluie".sto
 DriverTires:
 - TireIndex: 0
   TireCompoundType: "Hard"
 - TireIndex: 1
   TireCompoundType: "Wet"
 Drivers:
 - CarIdx: {pace}
   UserName: Pace Car
   UserID: -1
   CarNumber: "0"
   CarPath: safety pcporsche911cup
   CarIsPaceCar: 1
   CarIsAI: 0
   CarScreenName: Safety Car
   CarClassShortName:
   CarClassEstLapTime: 150.0000
   IsSpectator: 0
 - CarIdx: {rival}
   UserName: Rival: "le rapide" Un
   UserID: 111111
   TeamName: Écurie d'en face
   CarNumber: "7"
   CarPath: ferrari296gt3
   CarIsPaceCar: 0
   CarIsAI: 0
   CarScreenName: Ferrari 296 GT3
   CarClassShortName: GT3
   CarClassEstLapTime: {lap:.4}
   IsSpectator: 0
 - CarIdx: {player}
   UserName: Pilote Essai
   UserID: 424242
   TeamName: Pilote Essai
   CarNumber: "21"
   CarPath: bmwm4gt3
   CarIsPaceCar: 0
   CarIsAI: 0
   CarScreenName: BMW M4 GT3
   CarClassShortName: GT3
   CarClassEstLapTime: {lap:.4}
   IsSpectator: 0
 - CarIdx: {spectator}
   UserName: Spectateur Curieux
   UserID: 333333
   CarNumber: "99"
   CarPath: bmwm4gt3
   CarIsPaceCar: 0
   CarIsAI: 0
   CarScreenName: BMW M4 GT3
   CarClassShortName: GT3
   CarClassEstLapTime: {lap:.4}
   IsSpectator: 1
SplitTimeInfo:
 Sectors:
 - SectorNum: 0
   SectorStartPct: 0.000000
 - SectorNum: 1
   SectorStartPct: 0.340000
 - SectorNum: 2
   SectorStartPct: 0.710000
...
"#,
        km = fake.track_m / 1000.0,
        lap = fake.lap_s,
        player = PLAYER,
        rival = RIVAL,
        pace = PACE_CAR,
        spectator = SPECTATOR,
    )
}

/// The variables the fake game publishes: those Corde reads, as the game types them.
pub fn table() -> VarTable {
    use VarType::{BitField, Bool, Double, Float, Int};
    let mut vars: Vec<(String, VarType, usize)> = [
        ("SessionTime", Double, 1),
        ("SessionTimeRemain", Double, 1),
        ("SessionNum", Int, 1),
        ("SessionState", Int, 1),
        ("SessionFlags", BitField, 1),
        ("SessionLapsTotal", Int, 1),
        ("SessionTimeOfDay", Float, 1),
        ("IsOnTrack", Bool, 1),
        ("IsReplayPlaying", Bool, 1),
        ("IsInGarage", Bool, 1),
        ("OnPitRoad", Bool, 1),
        ("PlayerCarInPitStall", Bool, 1),
        ("PlayerTrackSurface", Int, 1),
        ("PlayerCarIdx", Int, 1),
        ("PlayerCarPosition", Int, 1),
        ("PlayerCarMyIncidentCount", Int, 1),
        ("PlayerTireCompound", Int, 1),
        ("Lap", Int, 1),
        ("LapCompleted", Int, 1),
        ("LapDist", Float, 1),
        ("LapDistPct", Float, 1),
        ("LapCurrentLapTime", Float, 1),
        ("LapLastLapTime", Float, 1),
        ("LapBestLapTime", Float, 1),
        ("LapDeltaToBestLap", Float, 1),
        ("LapDeltaToBestLap_OK", Bool, 1),
        ("Speed", Float, 1),
        ("RPM", Float, 1),
        ("Gear", Int, 1),
        ("Throttle", Float, 1),
        ("Brake", Float, 1),
        ("Clutch", Float, 1),
        ("SteeringWheelAngle", Float, 1),
        ("SteeringWheelAngleMax", Float, 1),
        ("LatAccel", Float, 1),
        ("LongAccel", Float, 1),
        ("VertAccel", Float, 1),
        ("YawRate", Float, 1),
        ("Yaw", Float, 1),
        ("VelocityX", Float, 1),
        ("VelocityY", Float, 1),
        ("VelocityZ", Float, 1),
        ("Lat", Double, 1),
        ("Lon", Double, 1),
        ("Alt", Float, 1),
        ("FuelLevel", Float, 1),
        ("FuelLevelPct", Float, 1),
        ("WaterTemp", Float, 1),
        ("OilTemp", Float, 1),
        ("ManifoldPress", Float, 1),
        ("EngineWarnings", BitField, 1),
        ("AirTemp", Float, 1),
        ("TrackTempCrew", Float, 1),
        ("TrackWetness", Int, 1),
        ("Precipitation", Float, 1),
        ("WindVel", Float, 1),
        ("WindDir", Float, 1),
        ("Skies", Int, 1),
        ("dcBrakeBias", Float, 1),
        ("dcTractionControl", Float, 1),
        ("dcABS", Float, 1),
        ("BrakeABSactive", Bool, 1),
        ("CarIdxLap", Int, MAX_CARS),
        ("CarIdxLapCompleted", Int, MAX_CARS),
        ("CarIdxLapDistPct", Float, MAX_CARS),
        ("CarIdxTrackSurface", Int, MAX_CARS),
        ("CarIdxOnPitRoad", Bool, MAX_CARS),
        ("CarIdxPosition", Int, MAX_CARS),
        ("CarIdxClassPosition", Int, MAX_CARS),
        ("CarIdxF2Time", Float, MAX_CARS),
        ("CarIdxEstTime", Float, MAX_CARS),
        ("CarIdxLastLapTime", Float, MAX_CARS),
        ("CarIdxBestLapTime", Float, MAX_CARS),
    ]
    .into_iter()
    .map(|(n, k, c)| (n.to_string(), k, c))
    .collect();
    for wheel in ["LF", "RF", "LR", "RR"] {
        for what in [
            "tempCL",
            "tempCM",
            "tempCR",
            "wearL",
            "wearM",
            "wearR",
            "pressure",
            "rideHeight",
            "shockDefl",
        ] {
            vars.push((format!("{wheel}{what}"), Float, 1));
        }
    }
    let borrowed: Vec<(&str, VarType, usize)> =
        vars.iter().map(|(n, k, c)| (n.as_str(), *k, *c)).collect();
    VarTable::of(&borrowed)
}

impl FakeSession {
    /// The race of the event, at the green flag.
    pub fn race() -> Self {
        Self {
            session_num: 1,
            ..Self::default()
        }
    }

    /// Line crossings since `t = 0`.
    fn crossings(&self, t: f64) -> i32 {
        (self.start_fraction + t / self.lap_s).floor() as i32
    }

    /// The official time of the lap that ended at crossing `k` (−1: none).
    pub fn lap_time(&self, k: i32) -> f32 {
        // The first crossing ends a lap that was not started at the line.
        if k < 1 || self.refused.contains(&k) || (k == 1 && self.start_fraction > 0.0) {
            return -1.0;
        }
        // What the lap lasted, published with a little more that differs lap to lap.
        (self.lap_s + self.time_step_s * f64::from(k % 5)) as f32
    }

    /// The mapping `t` seconds after the start.
    pub fn at(&self, t: f64) -> Image {
        let table = table();
        let row = self.row(&table, t);
        Image {
            status: if self.connected { STATUS_CONNECTED } else { 0 },
            tick_rate: 60,
            session_info_update: self.info_update,
            session_text: session_text(self),
            table,
            // The tick count moves 60 times a second.
            rows: vec![((t * 60.0).round() as i32 + 1, row)],
        }
    }

    /// The row of values `t` seconds after the start, laid out by `table` (see [`table`]).
    pub fn row(&self, table: &VarTable, t: f64) -> Vec<u8> {
        let mut row = table.new_row();
        let mut put = |name: &str, v: f64| {
            assert!(table.put(&mut row, name, 0, v), "{name}");
        };
        let total = self.start_fraction + t / self.lap_s;
        let crossings = self.crossings(t);
        let fraction = total.fract();
        let lap = self.first_lap + crossings;
        // The crossings whose time has been published by now.
        let published = self.crossings(t - self.last_time_delay_s).max(0);
        let last = self.lap_time(published);
        let best = (1..=published)
            .map(|k| self.lap_time(k))
            .filter(|v| *v > 0.0)
            .fold(f32::INFINITY, f32::min);
        let speed = f64::from(self.track_m) / self.lap_s;

        put("SessionTime", self.clock_at_start + t);
        put(
            "SessionTimeRemain",
            if self.session_num == 1 {
                1800.0 - t
            } else {
                604_800.0
            },
        );
        put("SessionNum", f64::from(self.session_num));
        put("SessionState", f64::from(session_state::RACING));
        put("SessionFlags", f64::from(self.session_flags | flags::GREEN));
        put(
            "SessionLapsTotal",
            if self.session_num == 1 {
                12.0
            } else {
                32_767.0
            },
        );
        put("SessionTimeOfDay", 14.0 * 3600.0 + t);
        put(
            "IsOnTrack",
            f64::from(u8::from(self.on_track && self.in_world)),
        );
        put("IsInGarage", f64::from(u8::from(!self.in_world)));
        put(
            "PlayerTrackSurface",
            f64::from(if self.in_world {
                track_loc::ON_TRACK
            } else {
                track_loc::NOT_IN_WORLD
            }),
        );
        put("PlayerCarIdx", PLAYER as f64);
        put("PlayerCarPosition", 2.0);
        put("PlayerCarMyIncidentCount", f64::from(self.incidents));
        put("PlayerTireCompound", 0.0);
        if self.in_world {
            put("Lap", f64::from(lap));
            put("LapCompleted", f64::from(lap + self.completed_offset));
        }
        put("LapDist", fraction * f64::from(self.track_m));
        put("LapDistPct", fraction);
        // The clock of the lap runs from the line, also for a lap joined on the way.
        put("LapCurrentLapTime", fraction * self.lap_s);
        put("LapLastLapTime", f64::from(last));
        put(
            "LapBestLapTime",
            if best.is_finite() {
                f64::from(best)
            } else {
                0.0
            },
        );
        put("LapDeltaToBestLap", -0.123);
        put(
            "LapDeltaToBestLap_OK",
            f64::from(u8::from(best.is_finite())),
        );
        put("Speed", speed);
        put("RPM", 6200.0);
        put("Gear", 4.0);
        put("Throttle", 0.8);
        put("Brake", 0.0);
        put("Clutch", 1.0);
        put("SteeringWheelAngle", 0.5);
        put("SteeringWheelAngleMax", 4.0);
        put("LatAccel", 9.806_65);
        put("LongAccel", -4.903_325);
        put("VertAccel", 9.806_65);
        put("YawRate", 0.1);
        put("Yaw", 1.0);
        put("VelocityX", speed);
        put("Lat", 50.437 + 0.001 * fraction);
        put("Lon", 5.971 + 0.002 * fraction);
        put("Alt", 410.0);
        put("FuelLevel", 60.0 - 0.02 * t);
        put("FuelLevelPct", (60.0 - 0.02 * t) / 120.0);
        put("WaterTemp", 88.0);
        put("OilTemp", 104.0);
        put("ManifoldPress", 1.4);
        put("AirTemp", 21.0);
        put("TrackTempCrew", 29.5);
        put("TrackWetness", 1.0);
        put("WindVel", 3.0);
        put("Skies", 1.0);
        put("dcBrakeBias", 54.5);
        put("dcTractionControl", 4.0);
        put("dcABS", 3.0);
        for (i, wheel) in ["LF", "RF", "LR", "RR"].into_iter().enumerate() {
            put(&format!("{wheel}tempCL"), 80.0 + i as f64);
            put(&format!("{wheel}tempCM"), 85.0 + i as f64);
            put(&format!("{wheel}tempCR"), 90.0 + i as f64);
            for part in ["wearL", "wearM", "wearR"] {
                put(&format!("{wheel}{part}"), 0.9);
            }
            put(&format!("{wheel}pressure"), 172.0 + i as f64);
            put(&format!("{wheel}rideHeight"), 0.06);
            put(&format!("{wheel}shockDefl"), 0.03);
        }
        // The cars: nobody in the world but the player and the rival, three seconds ahead.
        let mut car = |name: &str, i: usize, v: f64| {
            assert!(table.put(&mut row, name, i, v), "{name}");
        };
        for i in 0..MAX_CARS {
            car("CarIdxTrackSurface", i, f64::from(track_loc::NOT_IN_WORLD));
            car("CarIdxLapDistPct", i, -1.0);
            car("CarIdxLapCompleted", i, -1.0);
            car("CarIdxLastLapTime", i, -1.0);
            car("CarIdxBestLapTime", i, -1.0);
        }
        let ahead = 3.0 / self.lap_s;
        let rival = (total + ahead).fract();
        for (i, pct, place, behind) in [(RIVAL, rival, 1.0, 0.0), (PLAYER, fraction, 2.0, 3.0)] {
            if i == PLAYER && !self.in_world {
                continue;
            }
            car("CarIdxTrackSurface", i, f64::from(track_loc::ON_TRACK));
            car("CarIdxLapDistPct", i, pct);
            car("CarIdxEstTime", i, pct * self.lap_s);
            car("CarIdxLap", i, f64::from(lap));
            car(
                "CarIdxLapCompleted",
                i,
                f64::from(lap + self.completed_offset),
            );
            car("CarIdxPosition", i, place);
            car("CarIdxClassPosition", i, place);
            car("CarIdxF2Time", i, behind);
            car("CarIdxLastLapTime", i, f64::from(last));
            car(
                "CarIdxBestLapTime",
                i,
                if best.is_finite() {
                    f64::from(best)
                } else {
                    -1.0
                },
            );
        }
        row
    }
}
