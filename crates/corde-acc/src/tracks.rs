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

/// Words the games write in lower case that are read in capitals.
const CAPITALS: [&str; 16] = [
    "amg", "amr", "bmw", "cota", "cs", "gp", "gt", "gtc", "gtr", "ii", "ktm", "lms", "nsx", "rc",
    "rs", "st",
];

/// A readable name from an identifier of the game: « mount_panorama » → « Mount
/// Panorama », « ferrari_296_gt3 » → « Ferrari 296 GT3 » (a word with a figure in it is a
/// model name: in capitals).
pub fn pretty(id: &str) -> String {
    // « ks_ »: the prefix of the first Assetto Corsa's own content.
    let id = id.trim();
    let id = id.strip_prefix("ks_").unwrap_or(id);
    id.split(['_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let lower = w.to_lowercase();
            if w.chars().any(|c| c.is_ascii_digit()) || CAPITALS.contains(&lower.as_str()) {
                return w.to_uppercase();
            }
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The layout of a circuit as the game names it, empty when it names none. Assetto Corsa
/// Competizione always writes « track config » there: its circuits have one layout.
pub fn layout(configuration: &str) -> &str {
    let name = configuration.trim();
    if name.eq_ignore_ascii_case("track config") {
        ""
    } else {
        name
    }
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
        assert_eq!(pretty("bmw_m4_gt3"), "BMW M4 GT3");
        assert_eq!(pretty("ferrari_296_gt3"), "Ferrari 296 GT3");
        assert_eq!(pretty("mclaren_720s_gt3_evo"), "Mclaren 720S GT3 Evo");
        assert_eq!(pretty("audi_r8_lms_evo_ii"), "Audi R8 LMS Evo II");
        assert_eq!(pretty("ks_nurburgring"), "Nurburgring");
        assert_eq!(pretty("layout_gp_a"), "Layout GP A");
        assert_eq!(pretty(""), "");
        // The placeholder Competizione writes where a layout would be.
        assert_eq!(layout("track config"), "");
        assert_eq!(layout(" Track Config "), "");
        assert_eq!(layout("layout_gp_a"), "layout_gp_a");
        assert_eq!(layout(""), "");
    }
}
