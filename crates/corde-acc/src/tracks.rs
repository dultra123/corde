//! Lengths of the circuits of the game.
//!
//! The game does not publish the length of the lap (`track_spline_length` is 0), only the
//! position along it as a fraction. These are the official lengths of the layouts the game
//! ships, by the identifier of its static page. A circuit that is not listed — a new one, a
//! mod — gets its length measured on the first full lap ([`crate::convert`]).

/// Official length of a circuit, in metres, by the game's identifier (any case).
pub fn length_m(track: &str) -> Option<f32> {
    let id = track.trim().to_ascii_lowercase();
    let id = id
        .strip_suffix("_2019")
        .or_else(|| id.strip_suffix("_2020"))
        .unwrap_or(&id);
    let m = match id {
        "barcelona" => 4655.0,
        "brands_hatch" => 3908.0,
        "cota" => 5513.0,
        "donington" => 4020.0,
        "hungaroring" => 4381.0,
        "imola" => 4909.0,
        "indianapolis" => 4167.0,
        "kyalami" => 4522.0,
        "laguna_seca" => 3602.0,
        "misano" => 4226.0,
        "monza" => 5793.0,
        "mount_panorama" => 6213.0,
        "nurburgring" => 5137.0,
        "nurburgring_24h" => 25378.0,
        "oulton_park" => 4307.0,
        "paul_ricard" => 5770.0,
        "red_bull_ring" => 4318.0,
        "silverstone" => 5891.0,
        "snetterton" => 4779.0,
        "spa" => 7004.0,
        "suzuka" => 5807.0,
        "valencia" => 4005.0,
        "watkins_glen" => 5552.0,
        "zandvoort" => 4252.0,
        "zolder" => 4011.0,
        _ => return None,
    };
    Some(m)
}

/// A readable name from an identifier of the game: « mount_panorama » → « Mount Panorama ».
pub fn pretty(id: &str) -> String {
    id.split(['_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_circuits_have_a_length_and_a_name() {
        assert_eq!(length_m("monza"), Some(5793.0));
        assert_eq!(length_m("Spa"), Some(7004.0));
        assert_eq!(length_m("zolder_2019"), Some(4011.0));
        assert_eq!(length_m("somewhere_new"), None);
        assert_eq!(pretty("mount_panorama"), "Mount Panorama");
        assert_eq!(pretty("bmw_m4_gt3"), "Bmw M4 Gt3");
        assert_eq!(pretty(""), "");
    }
}
