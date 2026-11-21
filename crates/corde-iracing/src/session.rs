//! What Corde takes from the session text of the game: the circuit, the sessions of the
//! event, the player's car and every driver. Read once each time the game rewrites the
//! text, not at every frame.

use crate::yaml::{self, Node};
use corde_protocol::{CompoundType, SessionType, VehicleClass};

/// One session of the event (practice, qualifying, race…).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionEntry {
    pub num: i32,
    pub kind: SessionType,
    pub name: String,
    /// `None`: no lap limit.
    pub laps: Option<i32>,
}

/// One car of the session.
#[derive(Debug, Clone, PartialEq)]
pub struct Driver {
    pub idx: usize,
    pub name: String,
    pub car: String,
    pub car_path: String,
    pub class_name: String,
    pub class: VehicleClass,
    pub pace_car: bool,
    pub spectator: bool,
    pub ai: bool,
    /// Lap time the game expects from this class of car, seconds (0: not said).
    pub est_lap_s: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionDesc {
    /// Identifier of the circuit and layout (« spa up »).
    pub track_id: String,
    /// Name shown to the driver, with the layout when there is one.
    pub track_name: String,
    pub track_length_m: f32,
    pub is_fixed_setup: bool,
    /// An event with other people (it has a session number on the game's servers).
    pub multiplayer: bool,
    pub sessions: Vec<SessionEntry>,
    /// Index of the player's car (−1: not said).
    pub player_idx: i32,
    pub fuel_capacity_l: f32,
    pub red_line_rpm: f32,
    pub forward_gears: u8,
    pub est_lap_s: f32,
    pub drivers: Vec<Driver>,
    /// Where each sector of the lap starts, as fractions of the lap (the first is 0).
    pub sectors: Vec<f32>,
    /// Names of the tyre compounds of the player's car, by index.
    pub compounds: Vec<String>,
}

pub fn session_type(name: &str) -> SessionType {
    let n = name.to_ascii_lowercase();
    if n.contains("race") {
        SessionType::Race
    } else if n.contains("qual") {
        SessionType::Qualify
    } else if n.contains("warm") {
        SessionType::Warmup
    } else if n.contains("test") {
        SessionType::TestDay
    } else if n.contains("practice") {
        SessionType::Practice
    } else {
        SessionType::Unknown
    }
}

/// How Corde writes a class the game did not name.
fn class_label(class: VehicleClass) -> &'static str {
    match class {
        VehicleClass::Gt3 => "GT3",
        VehicleClass::Gte => "GTE",
        VehicleClass::Lmp2 => "LMP2",
        VehicleClass::Lmp3 => "LMP3",
        VehicleClass::Hypercar => "GTP",
        _ => "",
    }
}

/// The class Corde knows, from what the game calls the class or the car.
pub fn vehicle_class(class_name: &str, car: &str) -> VehicleClass {
    let both = format!("{class_name} {car}").to_ascii_lowercase();
    let has = |tag: &str| {
        both.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == tag)
    };
    if has("gt3") {
        VehicleClass::Gt3
    } else if has("gte") {
        VehicleClass::Gte
    } else if has("lmp2") {
        VehicleClass::Lmp2
    } else if has("lmp3") {
        VehicleClass::Lmp3
    } else if has("gtp") || has("hypercar") || has("lmdh") {
        VehicleClass::Hypercar
    } else {
        VehicleClass::Unknown
    }
}

pub fn compound_type(name: &str) -> CompoundType {
    let n = name.to_ascii_lowercase();
    if n.contains("wet") || n.contains("rain") {
        CompoundType::Wet
    } else if n.contains("soft") {
        CompoundType::Soft
    } else if n.contains("medium") {
        CompoundType::Medium
    } else if n.contains("hard") {
        CompoundType::Hard
    } else {
        CompoundType::Unknown
    }
}

impl SessionDesc {
    pub fn parse(text: &str) -> Self {
        Self::from_tree(&yaml::parse(text))
    }

    pub fn from_tree(root: &Node) -> Self {
        let w = root.get("WeekendInfo");
        let d = root.get("DriverInfo");
        let track_id = w.get("TrackName").str().to_string();
        let display = match w.get("TrackDisplayName").str() {
            "" => track_id.clone(),
            name => name.to_string(),
        };
        let track_name = match w.get("TrackConfigName").str() {
            "" => display,
            layout => format!("{display} — {layout}"),
        };
        // « 6.93 km ».
        let track_length_m = (w.get("TrackLength").number().unwrap_or(0.0) * 1000.0) as f32;
        let sessions = root
            .get("SessionInfo")
            .get("Sessions")
            .items()
            .iter()
            .map(|s| SessionEntry {
                num: s.get("SessionNum").int().unwrap_or(0) as i32,
                kind: session_type(s.get("SessionType").str()),
                name: s.get("SessionName").str().to_string(),
                laps: s
                    .get("SessionLaps")
                    .int()
                    .map(|v| v as i32)
                    .filter(|v| *v > 0),
            })
            .collect();
        let drivers = d
            .get("Drivers")
            .items()
            .iter()
            .filter_map(|n| {
                let idx = n.get("CarIdx").int()?;
                if !(0..crate::layout::MAX_CARS as i64).contains(&idx) {
                    return None;
                }
                let car = n.get("CarScreenName").str().to_string();
                let class_name = n.get("CarClassShortName").str().to_string();
                let pace_car = n.get("CarIsPaceCar").int() == Some(1);
                Some(Driver {
                    idx: idx as usize,
                    name: n.get("UserName").str().to_string(),
                    class: if pace_car {
                        VehicleClass::PaceCar
                    } else {
                        vehicle_class(&class_name, &car)
                    },
                    car_path: n.get("CarPath").str().to_string(),
                    // Alone on the circuit, the game names no class: the one found in
                    // the name of the car is shown.
                    class_name: if class_name.is_empty() && !pace_car {
                        class_label(vehicle_class("", &car)).to_string()
                    } else {
                        class_name
                    },
                    car,
                    pace_car,
                    spectator: n.get("IsSpectator").int() == Some(1),
                    ai: n.get("CarIsAI").int() == Some(1),
                    est_lap_s: n.get("CarClassEstLapTime").number().unwrap_or(0.0) as f32,
                })
            })
            .collect();
        let mut sectors: Vec<f32> = root
            .get("SplitTimeInfo")
            .get("Sectors")
            .items()
            .iter()
            .filter_map(|s| s.get("SectorStartPct").number())
            .map(|v| v as f32)
            .filter(|v| (0.0..1.0).contains(v))
            .collect();
        sectors.sort_by(|a, b| a.total_cmp(b));
        let mut compounds = Vec::new();
        for t in d.get("DriverTires").items() {
            let i = t.get("TireIndex").int().unwrap_or(-1);
            if (0..16).contains(&i) {
                let i = i as usize;
                if compounds.len() <= i {
                    compounds.resize(i + 1, String::new());
                }
                compounds[i] = t.get("TireCompoundType").str().to_string();
            }
        }
        let max_fuel = d.get("DriverCarFuelMaxLtr").number().unwrap_or(0.0);
        // The share of the tank the series allows (1 when it says nothing).
        let allowed = match d.get("DriverCarMaxFuelPct").number() {
            Some(v) if v > 0.0 && v <= 1.0 => v,
            _ => 1.0,
        };
        Self {
            track_id,
            track_name,
            track_length_m,
            is_fixed_setup: w.get("WeekendOptions").get("IsFixedSetup").int() == Some(1),
            multiplayer: w.get("SubSessionID").int().unwrap_or(0) != 0,
            sessions,
            player_idx: d.get("DriverCarIdx").int().unwrap_or(-1) as i32,
            fuel_capacity_l: (max_fuel * allowed) as f32,
            red_line_rpm: d.get("DriverCarRedLine").number().unwrap_or(0.0) as f32,
            forward_gears: d
                .get("DriverCarGearNumForward")
                .int()
                .unwrap_or(0)
                .clamp(0, 20) as u8,
            est_lap_s: d.get("DriverCarEstLapTime").number().unwrap_or(0.0) as f32,
            drivers,
            sectors,
            compounds,
        }
    }

    pub fn session(&self, num: i32) -> Option<&SessionEntry> {
        self.sessions.iter().find(|s| s.num == num)
    }

    pub fn driver(&self, idx: usize) -> Option<&Driver> {
        self.drivers.iter().find(|d| d.idx == idx)
    }

    pub fn player(&self) -> Option<&Driver> {
        usize::try_from(self.player_idx)
            .ok()
            .and_then(|i| self.driver(i))
    }

    /// The sector (1..3) at this fraction of the lap. The game cuts some circuits in
    /// more than three parts: they are then grouped in three, at the game's own cuts
    /// nearest to a third and to two thirds of the lap. 0: the game gives fewer than
    /// three parts.
    pub fn sector_at(&self, fraction: f32) -> u8 {
        let Some([second, third]) = self.thirds() else {
            return 0;
        };
        if fraction >= third {
            3
        } else if fraction >= second {
            2
        } else {
            1
        }
    }

    /// Where the second and the third sector start (fractions of the lap).
    pub fn thirds(&self) -> Option<[f32; 2]> {
        let cuts: Vec<f32> = self
            .sectors
            .iter()
            .copied()
            .filter(|s| *s > 0.0 && *s < 1.0)
            .collect();
        if cuts.len() < 2 {
            return None;
        }
        let nearest = |to: f32, among: &[f32]| {
            among
                .iter()
                .copied()
                .min_by(|a, b| (a - to).abs().total_cmp(&(b - to).abs()))
        };
        // The second cut is taken after the first, so that the three parts exist.
        let second = nearest(1.0 / 3.0, &cuts[..cuts.len() - 1])?;
        let after: Vec<f32> = cuts.iter().copied().filter(|s| *s > second).collect();
        let third = nearest(2.0 / 3.0, &after)?;
        Some([second, third])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_text_is_understood() {
        let desc = SessionDesc::parse(&crate::fake::session_text(
            &crate::fake::FakeSession::default(),
        ));
        assert_eq!(desc.track_id, "spa up");
        assert_eq!(
            desc.track_name,
            "Circuit de Spa-Francorchamps — Grand Prix Pits"
        );
        assert!((desc.track_length_m - 6930.0).abs() < 1.0);
        assert_eq!(desc.player_idx, 2);
        assert_eq!(desc.fuel_capacity_l, 120.0);
        assert_eq!(desc.red_line_rpm, 7500.0);
        assert_eq!(desc.forward_gears, 6);
        assert_eq!(desc.sessions.len(), 2);
        assert_eq!(desc.session(0).unwrap().kind, SessionType::Practice);
        assert_eq!(desc.session(0).unwrap().laps, None);
        assert_eq!(desc.session(1).unwrap().kind, SessionType::Race);
        assert_eq!(desc.session(1).unwrap().laps, Some(12));
        let me = desc.player().unwrap();
        assert_eq!(me.car, "BMW M4 GT3");
        assert_eq!(me.class, VehicleClass::Gt3);
        assert!(desc.driver(0).unwrap().pace_car);
        assert_eq!(desc.driver(0).unwrap().class, VehicleClass::PaceCar);
        assert_eq!(desc.sectors, [0.0, 0.34, 0.71]);
        assert_eq!(desc.sector_at(0.1), 1);
        assert_eq!(desc.sector_at(0.34), 2);
        assert_eq!(desc.sector_at(0.99), 3);
        assert_eq!(desc.compounds, ["Hard", "Wet"]);
    }

    #[test]
    fn session_and_class_names() {
        for (name, kind) in [
            ("Offline Testing", SessionType::TestDay),
            ("Practice", SessionType::Practice),
            ("Open Practice", SessionType::Practice),
            ("Lone Qualify", SessionType::Qualify),
            ("Open Qualify", SessionType::Qualify),
            ("Warmup", SessionType::Warmup),
            ("Race", SessionType::Race),
            ("Heat Race", SessionType::Race),
            ("", SessionType::Unknown),
        ] {
            assert_eq!(session_type(name), kind, "{name}");
        }
        assert_eq!(vehicle_class("GT3", "Ferrari 296 GT3"), VehicleClass::Gt3);
        assert_eq!(vehicle_class("", "BMW M4 GT3"), VehicleClass::Gt3);
        assert_eq!(
            vehicle_class("GTP", "Cadillac V-Series.R"),
            VehicleClass::Hypercar
        );
        assert_eq!(vehicle_class("LMP2", "Dallara P217"), VehicleClass::Lmp2);
        assert_eq!(vehicle_class("", "Mazda MX-5 Cup"), VehicleClass::Unknown);
        // A circuit the game cuts in more than three parts: grouped in three, at the
        // game's own cuts nearest to the thirds of the lap.
        let desc = SessionDesc {
            sectors: vec![0.0, 0.2, 0.4, 0.6, 0.8],
            ..SessionDesc::default()
        };
        assert_eq!(desc.thirds(), Some([0.4, 0.6]));
        assert_eq!(
            [0.0, 0.39, 0.4, 0.59, 0.6, 0.99].map(|f| desc.sector_at(f)),
            [1, 1, 2, 2, 3, 3]
        );
        // Three parts: the game's.
        let three = SessionDesc {
            sectors: vec![0.0, 0.34, 0.71],
            ..SessionDesc::default()
        };
        assert_eq!(three.thirds(), Some([0.34, 0.71]));
        // Two cuts only close to each other, at the end of the lap: still three parts.
        let odd = SessionDesc {
            sectors: vec![0.0, 0.8, 0.9],
            ..SessionDesc::default()
        };
        assert_eq!(odd.thirds(), Some([0.8, 0.9]));
        // Fewer than three parts: not known.
        for sectors in [vec![], vec![0.0], vec![0.0, 0.5]] {
            let desc = SessionDesc {
                sectors,
                ..SessionDesc::default()
            };
            assert_eq!(desc.sector_at(0.7), 0);
        }
        // Alone on the circuit, the game names no class.
        let text = crate::fake::session_text(&crate::fake::FakeSession::default())
            .replace("CarClassShortName: GT3", "CarClassShortName:");
        let desc = SessionDesc::parse(&text);
        assert_eq!(desc.player().unwrap().class_name, "GT3");
        assert_eq!(desc.player().unwrap().class, VehicleClass::Gt3);
    }
}
