//! First pairing without a terminal (`--first-pairing-code`, docs/SECURITY-MODEL.md §7).
//!
//! On a server where the owner has no comfortable shell (a NAS running the container image,
//! docs/SYNOLOGY.md), the first pairing code cannot come from `corde-server pair`. With the
//! option, and **only while no device at all is paired**, the hub issues one administrator
//! code at start-up through the usual mechanism ([`HubAuth::issue_code`]: single use, valid
//! 10 minutes, three wrong attempts, rate limits) and prints it, framed, on the standard
//! output — the service's log. It is never written anywhere else (no file, no API). Once a
//! device is paired, nothing is printed any more; a restart with still no device gives a
//! fresh code.

use super::auth::{HubAuth, IssuedCode, PAIRING_CODE_TTL};
use super::cli::format_code;
use chrono::{DateTime, Utc};

/// The code to print at start-up: `Some` only when no device is paired.
pub fn issue_if_unpaired(auth: &HubAuth) -> Option<IssuedCode> {
    if !auth.list().is_empty() {
        return None;
    }
    Some(auth.issue_code(true))
}

/// When a code issued at `issued` stops being valid.
pub fn valid_until(issued: DateTime<Utc>) -> DateTime<Utc> {
    issued
        + chrono::Duration::from_std(PAIRING_CODE_TTL).unwrap_or_else(|_| chrono::Duration::zero())
}

const RULE: &str = "  ==========================================================================";

/// The framed block written to the log (French, for the owner).
pub fn block(code: &IssuedCode, port: u16, until: DateTime<Utc>) -> Vec<String> {
    let minutes = code.expires_in_s / 60;
    vec![
        String::new(),
        RULE.to_string(),
        "   PREMIER APPAIRAGE : aucun appareil n'est encore appairé à ce serveur.".to_string(),
        String::new(),
        format!(
            "   Code d'appairage : {}   (l'appareil sera administrateur)",
            format_code(&code.code)
        ),
        String::new(),
        format!(
            "   Valable {minutes} minutes, une seule fois : jusqu'à {} UTC le {}",
            until.format("%H:%M"),
            until.format("%d/%m/%Y")
        ),
        "   (heure UTC : en France et en Suisse, ajoute 1 h en hiver, 2 h en été).".to_string(),
        "   Trois codes faux l'annulent.".to_string(),
        String::new(),
        "   Où le taper :".to_string(),
        "   - sur le PC : Corde → Réglages → Serveur Corde → Adresse du serveur".to_string(),
        "     (l'adresse de cette machine sur ton réseau), Code d'appairage → Appairer ;".to_string(),
        format!(
            "   - ou sur le téléphone : ouvre http://<adresse-de-cette-machine>:{port} et tape ce code."
        ),
        String::new(),
        "   Code expiré ou annulé ? Redémarre Corde serveur (le conteneur) : tant qu'aucun".to_string(),
        "   appareil n'est appairé, un nouveau code s'affiche ici. Ensuite, plus rien ne".to_string(),
        "   s'affiche : les appareils suivants s'appairent avec « corde-server pair »".to_string(),
        "   (docs/SYNOLOGY.md, « Appairer d'autres appareils »).".to_string(),
        RULE.to_string(),
        String::new(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::auth::DeviceKind;
    use std::net::IpAddr;

    fn dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "corde-hub-first-{tag}-{}-{}",
            std::process::id(),
            corde_store::db::now_ms()
        ))
    }

    #[test]
    fn a_code_only_while_no_device_is_paired_and_it_works_once() {
        let d = dir("once");
        let auth = HubAuth::open(&d).unwrap();
        let code = issue_if_unpaired(&auth).expect("no device yet: a code");
        assert!(code.admin);
        assert_eq!(code.expires_in_s, PAIRING_CODE_TTL.as_secs());
        let ip: IpAddr = [192, 168, 1, 42].into();
        let (dev, _) = auth
            .pair(ip, &code.code, "PC", DeviceKind::App)
            .expect("the printed code pairs");
        assert!(dev.admin);
        assert!(
            auth.pair(ip, &code.code, "x", DeviceKind::App).is_err(),
            "single use"
        );
        // A device exists: nothing more, now or after a restart.
        assert!(issue_if_unpaired(&auth).is_none());
        drop(auth);
        let auth = HubAuth::open(&d).unwrap();
        assert!(issue_if_unpaired(&auth).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_block_names_the_code_where_to_type_it_and_until_when() {
        let code = IssuedCode {
            code: "48151623".into(),
            expires_in_s: 600,
            admin: true,
        };
        let issued = DateTime::parse_from_rfc3339("2026-11-15T13:22:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let until = valid_until(issued);
        let text = block(&code, 8471, until).join("\n");
        assert!(text.contains("Code d'appairage : 4815 1623"), "{text}");
        assert!(text.contains("10 minutes"), "{text}");
        assert!(text.contains("jusqu'à 13:32 UTC le 15/11/2026"), "{text}");
        assert!(text.contains("Réglages → Serveur Corde"), "{text}");
        assert!(text.contains(":8471"), "{text}");
        assert!(text.contains("administrateur"), "{text}");
        // Framed.
        assert_eq!(text.matches(RULE).count(), 2);
    }
}
