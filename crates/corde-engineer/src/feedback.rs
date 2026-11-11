//! Driver feedback → symptoms ([`FeedbackInterpretation`]).
//!
//! Two extractors implement [`SymptomExtractor`]:
//!
//! * [`DictionaryExtractor`] (default, offline): `knowledge/feedback-dictionary.yaml`
//!   (embedded, overridable by a file) plus built-in French / English regular
//!   expressions for the wordings drivers actually type ("ça sous-vire",
//!   "l'arrière part à la remise des gaz", "je bloque", "it snaps on exit"…).
//!   The text is normalised (lower case, accents and punctuation removed),
//!   split into clauses (", mais, et, puis, but, and…"), and each clause is
//!   scanned for symptom patterns and modifiers: corner phase (entry / braking,
//!   mid, exit), corner speed (slow / medium / fast), axle (front / rear),
//!   intensity ("un peu" / "beaucoup" / "trop"), negation ("pas de", "ne …
//!   plus", "no …") and rain. A clause without symptom lends its modifiers to
//!   the neighbouring symptom clause ("ça sous-vire, surtout dans les lents").
//! * [`AnthropicExtractor`] (opt-in): the Anthropic Messages API with a strict
//!   JSON schema; any failure falls back to the dictionary with a note.
//!
//! Symptom ids are the `symptom` values of `knowledge/setup-rules.yaml` when the
//! text is precise enough (`US_ENTRY_SLOW`), or a *generic* family that
//! `iterate.rs` expands with the data: `US`, `US_ENTRY`, `OS_FAST`, `BRK_LOCK`,
//! `TRC_WHEELSPIN`, `TYR_HOT`, `NO_GRIP`, `JUMPS`.

use crate::error::{EngineerError, Result};
use corde_protocol::{
    Axle, CornerPhase, CornerSpeed, DetectedSymptom, ExtractorKind, FeedbackInterpretation,
    SymptomIntensity,
};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

pub const FEEDBACK_DICTIONARY_YAML: &str =
    include_str!("../../../knowledge/feedback-dictionary.yaml");

// ---------------------------------------------------------------------------
// YAML model (knowledge/README.md §3.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeedbackDictionary {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<FeelEntry>,
    #[serde(default)]
    pub aliases: Vec<FeelAlias>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeelEntry {
    pub id: String,
    #[serde(default)]
    pub fr: Vec<String>,
    #[serde(default)]
    pub en: Vec<String>,
    #[serde(default)]
    pub symptoms: Vec<String>,
    #[serde(default)]
    pub default_phase: Option<String>,
    #[serde(default)]
    pub default_corner_speed: Option<String>,
    #[serde(default)]
    pub clarify: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeelAlias {
    pub id: String,
    pub alias_of: String,
}

impl FeedbackDictionary {
    pub fn from_yaml(text: &str) -> Result<Self> {
        serde_yaml::from_str(text)
            .map_err(|e| EngineerError::Knowledge(format!("feedback-dictionary.yaml : {e}")))
    }

    /// The dictionary compiled into the binary.
    pub fn embedded() -> Result<Self> {
        Self::from_yaml(FEEDBACK_DICTIONARY_YAML)
    }

    /// `<dir>/feedback-dictionary.yaml`, or the embedded one when absent.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        match std::fs::read_to_string(dir.join("feedback-dictionary.yaml")) {
            Ok(text) => Self::from_yaml(&text),
            Err(_) => Self::embedded(),
        }
    }

    /// An entry by id, following aliases.
    pub fn entry(&self, id: &str) -> Option<&FeelEntry> {
        let id = self
            .aliases
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.alias_of.as_str())
            .unwrap_or(id);
        self.entries.iter().find(|e| e.id == id)
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Turns free text into symptoms. `lang_hint` is `"fr"` / `"en"` when the UI
/// knows it; both languages are always recognised.
pub trait SymptomExtractor: Send + Sync {
    fn extract(&self, text: &str, lang_hint: Option<&str>) -> FeedbackInterpretation;
    fn kind(&self) -> ExtractorKind;
}

/// Convenience: `extractor.extract(text, None)`.
pub fn interpret(text: &str, extractor: &dyn SymptomExtractor) -> FeedbackInterpretation {
    extractor.extract(text, None)
}

// ---------------------------------------------------------------------------
// Normalisation
// ---------------------------------------------------------------------------

/// Lower case ASCII, accents folded, apostrophes and most punctuation turned
/// into spaces, sentence punctuation (`.;!?` and line breaks) into `.`, clause
/// punctuation (`,:()/`) into `,`. One output char per input char, so indices
/// map one to one.
pub fn normalise(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' | 'ý' => 'y',
            'ç' => 'c',
            'ñ' => 'n',
            'œ' => 'o',
            'æ' => 'a',
            'À' | 'Â' | 'Ä' | 'Á' => 'a',
            'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'Î' | 'Ï' => 'i',
            'Ô' | 'Ö' => 'o',
            'Ù' | 'Û' | 'Ü' => 'u',
            'Ç' => 'c',
            '.' | ';' | '!' | '?' | '\n' | '\r' | '|' | '…' => '.',
            ',' | ':' | '(' | ')' | '[' | ']' | '/' => ',',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            c if c.is_ascii_uppercase() => c.to_ascii_lowercase(),
            _ => ' ',
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Symptom labels
// ---------------------------------------------------------------------------

fn phase_of_tag(tag: &str) -> Option<CornerPhase> {
    match tag {
        "ENTRY" => Some(CornerPhase::Entry),
        "MID" => Some(CornerPhase::Mid),
        "EXIT" => Some(CornerPhase::Exit),
        _ => None,
    }
}

fn speed_of_tag(tag: &str) -> Option<CornerSpeed> {
    match tag {
        "SLOW" => Some(CornerSpeed::Slow),
        "FAST" => Some(CornerSpeed::Fast),
        _ => None,
    }
}

/// `US_ENTRY_SLOW` → (US, Some(Entry), Some(Slow)); `OS_FAST` → (OS, None,
/// Some(Fast)); `BRK_LOCK_FRONT` → None.
pub fn parse_balance_id(id: &str) -> Option<(&str, Option<CornerPhase>, Option<CornerSpeed>)> {
    let mut parts = id.split('_');
    let fam = parts.next()?;
    if fam != "US" && fam != "OS" {
        return None;
    }
    let mut phase = None;
    let mut speed = None;
    for p in parts {
        if let Some(ph) = phase_of_tag(p) {
            phase = Some(ph);
        } else {
            speed = Some(speed_of_tag(p)?);
        }
    }
    Some((fam, phase, speed))
}

/// French / English label of a symptom id (rule symptoms and generic families).
pub fn symptom_label(id: &str) -> (String, String) {
    if let Some((fam, phase, speed)) = parse_balance_id(id) {
        let (fr, en) = if fam == "US" {
            ("Sous-virage", "Understeer")
        } else {
            ("Survirage", "Oversteer")
        };
        let (pfr, pen) = match phase {
            Some(CornerPhase::Entry) => (" en entrée", " on entry"),
            Some(CornerPhase::Mid) => (" en milieu de virage", " mid-corner"),
            Some(CornerPhase::Exit) => (" en sortie", " on exit"),
            None => ("", ""),
        };
        let (sfr, sen) = match speed {
            Some(CornerSpeed::Slow) => (" (virages lents)", " (slow corners)"),
            Some(CornerSpeed::Fast) => (" (virages rapides)", " (fast corners)"),
            _ => ("", ""),
        };
        return (format!("{fr}{pfr}{sfr}"), format!("{en}{pen}{sen}"));
    }
    let (fr, en) = match id {
        "BRK_LOCK_FRONT" => ("Blocage des roues avant", "Front lock-ups"),
        "BRK_LOCK_REAR" => ("Blocage des roues arrière", "Rear lock-ups"),
        "BRK_LOCK_INNER_FRONT" => ("Blocage de l'avant intérieur", "Inside-front lock-up"),
        "BRK_LOCK" => ("Blocage de roues", "Wheel lock-ups"),
        "BRK_UNSTABLE_STRAIGHT" => ("Instable au freinage", "Unstable under braking"),
        "BRK_COLD" => ("Freins froids", "Cold brakes"),
        "BRK_OVERHEAT" => (
            "Freins qui surchauffent / fading",
            "Brake fade / overheating",
        ),
        "ABS_TOO_ACTIVE" => ("ABS trop présent", "ABS too active"),
        "TRC_WHEELSPIN_BOTH" => ("Patinage en sortie", "Wheelspin on exit"),
        "TRC_WHEELSPIN_INNER" => (
            "Patinage de la roue intérieure",
            "Inside-wheel spin on exit",
        ),
        "TRC_WHEELSPIN" => (
            "Patinage / manque de motricité",
            "Wheelspin / poor traction",
        ),
        "TC_TOO_INTRUSIVE" => ("TC trop intrusif", "TC too intrusive"),
        "PLT_BOTTOMING" => ("Talonnage", "Bottoming out"),
        "PLT_FLOOR_STALL" => ("Décrochage du fond plat", "Floor stall"),
        "PLT_KERB_UNSETTLED" => ("Déstabilisée sur les vibreurs", "Unsettled by the kerbs"),
        "PLT_BUMPY_LOSS" => ("Perte de grip sur les bosses", "Loses grip over bumps"),
        "PLT_FLOATY" => ("Voiture qui flotte / roule", "Floaty / wallowing car"),
        "PLT_NERVOUS" => ("Voiture nerveuse / instable", "Nervous / unstable car"),
        "JUMPS" => ("Ça saute (vibreurs ou bosses ?)", "Jumps (kerbs or bumps?)"),
        "UNSTABLE" => ("Instable (à préciser)", "Unstable (where?)"),
        "STR_SLOW_DIRECTION_CHANGE" => ("Lente à changer de direction", "Slow to change direction"),
        "STR_TOP_SPEED_LOW" => ("Manque de vitesse de pointe", "Lacks top speed"),
        "TYR_HOT_FRONT" => ("Pneus avant qui surchauffent", "Front tyres overheating"),
        "TYR_HOT_REAR" => ("Pneus arrière qui surchauffent", "Rear tyres overheating"),
        "TYR_HOT" => ("Pneus qui surchauffent", "Tyres overheating"),
        "TYR_HOT_INNER" => ("Intérieur du pneu trop chaud", "Inner shoulder too hot"),
        "TYR_HOT_OUTER" => ("Extérieur du pneu trop chaud", "Outer shoulder too hot"),
        "TYR_WEAR_FAST" => ("Usure / dégradation rapide", "Fast wear / degradation"),
        "TYR_COLD" => ("Pneus froids / pas de grip", "Cold tyres / no grip"),
        "NO_GRIP" => ("Pas de grip (à préciser)", "No grip (where / when?)"),
        "WET_NO_TEMP" => (
            "Pneus pluie froids / pas de grip sous la pluie",
            "Cold wets / no grip in the rain",
        ),
        "WET_BASELINE" => ("Conditions de pluie", "Wet conditions"),
        "HYB_SOC_EMPTY" => ("Batterie vide / pas de boost", "Battery flat / no boost"),
        "HYB_SOC_FULL" => (
            "Batterie pleine (freinage change)",
            "Battery full (braking changes)",
        ),
        "PHILOSOPHY_AGGRESSIVE" => ("Veut plus de rotation / attaquer", "Wants more rotation"),
        "PHILOSOPHY_SAFE" => ("Veut plus de stabilité", "Wants more stability"),
        "ENERGY_SHORT" => ("Consommation trop élevée", "Burning too much energy"),
        other => (other, other),
    };
    (fr.to_string(), en.to_string())
}

// ---------------------------------------------------------------------------
// Dictionary extractor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Pattern {
    re: Regex,
    /// Symptom templates: `US_{PHASE}_{SPEED}`, `BRK_LOCK_{AXLE}`,
    /// `TRC_WHEELSPIN_{INNER}`, `TYR_HOT_{AXLE}`, `UNSTABLE`, `NO_GRIP`,
    /// `JUMPS`, or a plain symptom id.
    templates: Vec<String>,
    base_confidence: f32,
    default_phase: Option<CornerPhase>,
    default_speed: Option<CornerSpeed>,
    clarify: Vec<String>,
}

/// Where a modifier regex matched, with what it sets.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Modifier {
    Phase(CornerPhase),
    Speed(CornerSpeed),
    Axle(Axle),
    Intensity(SymptomIntensity),
    Wet,
    Braking,
    Inner,
    Cold,
    Hot,
    Kerb,
    Bump,
    Bottom,
}

/// Modifiers found in one clause.
#[derive(Debug, Clone, Default)]
struct Context {
    phase: Option<CornerPhase>,
    speed: Option<CornerSpeed>,
    axle: Option<Axle>,
    axle_conflict: bool,
    intensity: Option<SymptomIntensity>,
    wet: bool,
    braking: bool,
    inner: bool,
    cold: bool,
    hot: bool,
    kerb: bool,
    bump: bool,
    bottom: bool,
    /// Something was found (used to decide whether a clause is "understood").
    any: bool,
}

impl Context {
    fn apply(&mut self, m: Modifier) {
        self.any = true;
        match m {
            Modifier::Phase(p) => {
                if self.phase.is_none() {
                    self.phase = Some(p);
                }
            }
            Modifier::Speed(s) => {
                if self.speed.is_none() {
                    self.speed = Some(s);
                }
            }
            Modifier::Axle(a) => match self.axle {
                None => self.axle = Some(a),
                Some(x) if x != a => self.axle_conflict = true,
                _ => {}
            },
            Modifier::Intensity(i) => {
                if self.intensity.is_none_or(|cur| i > cur) {
                    self.intensity = Some(i);
                }
            }
            Modifier::Wet => self.wet = true,
            Modifier::Braking => self.braking = true,
            Modifier::Inner => self.inner = true,
            Modifier::Cold => self.cold = true,
            Modifier::Hot => self.hot = true,
            Modifier::Kerb => self.kerb = true,
            Modifier::Bump => self.bump = true,
            Modifier::Bottom => self.bottom = true,
        }
    }

    /// Fill what is missing from another context (a neighbouring clause).
    fn inherit(&mut self, other: &Context) {
        if self.phase.is_none() {
            self.phase = other.phase;
        }
        if self.speed.is_none() {
            self.speed = other.speed;
        }
        if self.axle.is_none() && !self.axle_conflict {
            self.axle = other.axle;
        }
        if self.intensity.is_none() {
            self.intensity = other.intensity;
        }
        self.wet |= other.wet;
        self.braking |= other.braking;
    }

    /// Phase, speed, braking zone and rain carry over inside a sentence
    /// (not the axle nor the intensity, which belong to their clause).
    fn inherit_sentence(&mut self, other: &Context) {
        if self.phase.is_none() {
            self.phase = other.phase;
            self.braking |= other.braking;
        }
        if self.speed.is_none() {
            self.speed = other.speed;
        }
        self.wet |= other.wet;
    }

    fn effective_axle(&self) -> Option<Axle> {
        if self.axle_conflict {
            None
        } else {
            self.axle
        }
    }
}

struct ModifierRule {
    re: Regex,
    set: Modifier,
}

/// Built-in symptom patterns: `(regex over normalised text, templates, confidence, default phase, default speed)`.
/// `braking` as default phase means "the braking zone" (phase entry + braking flag).
const BUILTIN: &[(&str, &[&str], f32, &str, &str)] = &[
    // --- understeer ---------------------------------------------------------
    (
        r"\b(l ?avant|le train avant|le nez|l av|the front|front end|the nose)\b( qui)?( ne| n)?( veut pas| refuse de| arrive pas a| n arrive pas a)?( sous ?-? ?vire| pousse( tout droit)?| tire tout droit| va tout droit| tourne (pas|plus)| tourner| s inscrit (pas|plus)| inscrit (pas|plus)| glisse| s enfuit| s echappe| elargit| s ouvre| part tout droit| derape| decroche| est leger| est legere| flotte| mord (pas|plus)| manque de grip| accroche (pas|plus)| rentre (pas|plus)| pushes| washes out| washes| slides| won t turn( in)?| doesn t turn( in)?| wont turn| won t bite| doesn t bite| is light| is vague| goes wide| goes straight| won t grip| doesn t grip)",
        &["US_{PHASE}_{SPEED}"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(ca|elle|la voiture|la caisse|the car|it)\b( qui)?( ne| n)?( veut pas| refuse de)?( sous ?-? ?vire| pousse( tout droit)?| tire tout droit| va tout droit| tourne (pas|plus)| tourner| s inscrit (pas|plus)| inscrit (pas|plus)| elargit| s ouvre| part tout droit| rentre (pas|plus)| understeers| pushes( wide)?| runs wide| goes wide| won t turn( in)?| doesn t turn( in)?| wont turn| goes straight)",
        &["US_{PHASE}_{SPEED}"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(sous ?-? ?virage|sous ?-? ?vireuse|sous ?-? ?vireur|sousvirage|sous ?-? ?vire|understeer(s|ing|y)?|pushing wide|runs? wide|running wide|no front end|washes out|front end wash|pousse tout droit|tout droit)\b",
        &["US_{PHASE}_{SPEED}"],
        0.75,
        "",
        "",
    ),
    // --- oversteer ----------------------------------------------------------
    (
        r"\b(l ?arriere|le train arriere|le cul|l ar|le derriere|the rear|rear end|back end|the tail|the back)\b( qui)?( ne| n)?( veut)?( sur ?-? ?vire| decroche( d un coup)?| part( en travers| tout seul| sans prevenir| d un coup)?| se derobe| glisse| chasse| passe devant| passe| se barre| s echappe| s en va| est leger| est legere| flotte| me met en travers| vient| pivote| se balade| balade| louvoie| bouge| danse| embarque| sort| lache| se promene| derape| decroche| steps out| snaps| comes around| slides| goes| is loose| is light| is nervous| breaks away| kicks out| swings| wants to come around| lets go| goes light| moves around| moves| wiggles| wobbles| dances| comes round| rotates too much| overtakes)",
        &["OS_{PHASE}_{SPEED}"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(ca|elle|la voiture|la caisse|the car|it)\b( qui)?( ne| n)?( sur ?-? ?vire| part en travers| se met en travers| me met en travers| part de l ?arriere| glisse de l ?arriere| decroche de l ?arriere| part du cul| fait un tete a queue| part en tete a queue| tourne trop| pivote trop| est trop pointue| snaps| spins( out)?| steps out| comes around| swaps ends| gets loose| is loose| is tail happy| rotates too much| oversteers| kicks out)",
        &["OS_{PHASE}_{SPEED}"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(sur ?-? ?virage|sur ?-? ?vireuse|sur ?-? ?vireur|survirage|sur ?-? ?vire|tete ?-? ?a ?-? ?queue|oversteer(s|ing|y)?|loose|snap oversteer|tail happy|swaps ends|spun|en travers)\b",
        &["OS_{PHASE}_{SPEED}"],
        0.75,
        "",
        "",
    ),
    // --- instability (resolved by context: braking → straight-line, entry → OS, else nervous) ---
    (
        r"\b(instable|instables|pas stable|louvoie|serpente|zigzague|nerveuse|nerveux|pas posee|trop vive|imprevisible|unstable|twitchy|skittish|darty|edgy|wanders|wandering|snakes|squirmy|squirms|not stable|unpredictable|inconsistent|nervous|ca bouge|elle bouge|it moves around|moves around)\b",
        &["UNSTABLE"],
        0.7,
        "",
        "",
    ),
    // --- lock-ups -----------------------------------------------------------
    (
        r"\b(je bloque|ca bloque|elle bloque|bloque les roues|bloque des roues|bloque de l ?avant|bloque de l ?arriere|bloque une roue|bloque l ?avant|bloque l ?arriere|blocage|blocages|bloquent|bloque|roues? bloquees?|roue bloque|lock ?-? ?ups?|locks? up|locking( up)?( the)?( fronts| rears| front| rear)?|locked( up)?|lock the (fronts|rears)|flat ?-? ?spots?|flat ?-? ?spotted|plat sur les pneus|je plate|ca plate)\b",
        &["BRK_LOCK_{AXLE}"],
        0.8,
        "braking",
        "",
    ),
    // --- wheelspin ----------------------------------------------------------
    (
        r"\b(patine|patinent|patinage|ca patine|s allume|s allument|ca cire|cire|pas de motricite|manque de motricite|peu de motricite|motricite|wheel ?-? ?spins?|wheel ?-? ?spinning|spins? up|spinning up|no traction|lack of traction|poor traction|bad traction|traction issues?|traction problems?|can t put the power down|cant put the power down|can t get on the power|lights up|light up the rears|lights up the rears|one wheel peel)\b",
        &["TRC_WHEELSPIN_{INNER}"],
        0.8,
        "exit",
        "slow",
    ),
    // --- traction control --------------------------------------------------
    (
        r"\b(le tc coupe( trop)?|tc coupe( trop)?|tc trop intrusif|tc intrusif|le tc bride|ca bride|le tc intervient trop|tc intervient trop|le moteur s etouffe|tc kills|tc (is )?too intrusive|tc cuts( too much| in too much)?|tc kicks in too much|tc is too active|bogs down)\b",
        &["TC_TOO_INTRUSIVE"],
        0.8,
        "exit",
        "",
    ),
    // --- kerbs / bumps / bottoming ------------------------------------------
    (
        r"\b(talonne|talonnage|touche le sol|tape le fond|tape le sol|touche le fond|etincelles|bottoms? out|bottoming( out)?|scrapes?|scraping|sparks)\b",
        &["PLT_BOTTOMING"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(vibreurs?|vibros?|kerbs?|curbs?|curbing|kerbing)\b",
        &["PLT_KERB_UNSETTLED"],
        0.6,
        "",
        "",
    ),
    (
        r"\b(bosses?|bosselee?|cahots?|bumps?|bumpy)\b",
        &["PLT_BUMPY_LOSS"],
        0.6,
        "",
        "",
    ),
    (
        r"\b(ca saute|saute|sautille|sautent|rebondit|ca tape|tape|decolle|s envole|se desunit|perd le contact|jumps?|bounces|hops|skips|unsettled|unsettles|gets upset)\b",
        &["JUMPS"],
        0.65,
        "",
        "",
    ),
    // --- tyres --------------------------------------------------------------
    (
        r"\b(pneus?|gommes?|tyres?|tires?|les avants|les arrieres|the fronts|the rears|fronts|rears)\b( avant| arriere| av| ar| front| rear)?( qui)?( sont)?( ne)?( surchauffe| surchauffent| chauffe( trop)?| chauffent( trop)?| cuit| cuits| mort| morts| grille| grilles| crame| crament| sont cuits| sont morts| sont trop chauds| trop chauds| overheat(s|ing)?| cook(ed|s)?| are cooked| are dead| are fried| fried| too hot| are too hot| get too hot| are hot)\b",
        &["TYR_HOT_{AXLE}"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(pneus?|gommes?|tyres?|tires?|les avants|les arrieres|the fronts|the rears|fronts|rears)\b( avant| arriere| av| ar| front| rear)?( qui)?( ne)?( s usent| s use| usent| use| s usent trop vite| degrade| degradent| se degrade| se degradent| tombent| tombe| s en vont| s en va| lache| lachent| s ecroulent| wear( out)?| wears( out)?| wearing| go off| goes off| fall off| fall away| drop off| dropped off| are gone| degrade| are going off)\b",
        &["TYR_WEAR_FAST"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(pneus?|gommes?|tyres?|tires?)\b( qui)?( glissent| glisse| slide| slip| start sliding| commencent a glisser)\b.{0,12}\b(apres|after|au bout de|a partir de|fin de relais|late in the stint|end of (the )?stint)\b",
        &["TYR_HOT_{AXLE}", "TYR_WEAR_FAST"],
        0.75,
        "",
        "",
    ),
    (
        r"\b(usure|usure rapide|degradation|grosse degradation|grosse deg|big deg|high deg|degradation is high|wear is high|je perds \d+ ?s|lose a second|lose \d+ ?s)\b",
        &["TYR_WEAR_FAST"],
        0.75,
        "",
        "",
    ),
    (
        r"\b(pneus? froids?|gommes? froides?|cold tyres|cold tires|tyres are cold|tires are cold|pas de temperature( dans les pneus)?|pas de temp( dans les pneus)?|ne montent pas en temperature|montent pas en temperature|ne prennent pas de temperature|won t come in|wont come in|don t come in|never come in|never get up to temp|not up to temp|no temp( in the tyres| in the tires)?|tyres? never get warm|tires? never get warm)\b",
        &["TYR_COLD"],
        0.8,
        "",
        "",
    ),
    (
        r"\b(pas de grip|aucun grip|zero grip|pas d adherence|aucune adherence|manque de grip|manque d adherence|peu de grip|plus de grip|no grip|zero grip|lack of grip|lacks grip|not enough grip|no adhesion|ca glisse partout|glisse partout|slides everywhere|sliding everywhere)\b",
        &["NO_GRIP"],
        0.7,
        "",
        "",
    ),
    (
        r"\b(aquaplaning|aquaplane|aquaplanning|hydroplan\w*|aquaplan\w*|flaques|standing water|puddles)\b",
        &["WET_BASELINE"],
        0.75,
        "",
        "",
    ),
    // --- brakes -------------------------------------------------------------
    (
        r"\b(freins?|brakes?|brake pedal|la pedale|pedale)\b( qui)?( de frein)?( faiblit| faiblissent| faibli| s evanoui| s evanouit| s evanouissent| fade| fadent| disparait| disparaissent| lache| lachent| chauffe| chauffent| surchauffe| surchauffent| cuisent| sont cuits| s affaiblit| s affaiblissent| devient longue| deviennent longs| s allonge| perdent| perd| est longue| sont longs| fades| fading| go away| goes away| get long| gets long| goes long| are gone| overheat| overheating| are cooked| are fading| is long| are spongy| spongy)\b",
        &["BRK_OVERHEAT"],
        0.8,
        "braking",
        "",
    ),
    (
        r"\b(fading|brake fade|long pedal|spongy brakes|pedale longue|freins spongieux|freins qui faiblissent|plus de freins)\b",
        &["BRK_OVERHEAT"],
        0.75,
        "braking",
        "",
    ),
    (
        r"\b(freins froids|pas de freins a froid|freins ne chauffent pas|freins qui ne chauffent pas|pas de freins au premier tour|no brakes when cold|brakes are cold|cold brakes|brakes don t come in|brakes won t warm up)\b",
        &["BRK_COLD"],
        0.8,
        "braking",
        "",
    ),
    (
        r"\b(l abs pompe|abs pompe|abs tout le temps|trop d abs|abs trop present|abs kicks in|abs pulsing|abs is pulsing|abs all the time|too much abs|abs is too active)\b",
        &["ABS_TOO_ACTIVE"],
        0.8,
        "braking",
        "",
    ),
    // --- rotation / stability wishes ----------------------------------------
    (
        r"\b(plus de rotation|manque de rotation|pas assez de rotation|elle tourne pas assez|tourne pas assez|ne tourne pas assez|plus d attaque|trop sure|trop safe|too safe|more rotation|not enough rotation|lacks rotation|doesn t rotate enough|won t rotate|could be more aggressive)\b",
        &["PHILOSOPHY_AGGRESSIVE", "US_{PHASE}_{SPEED}"],
        0.7,
        "",
        "",
    ),
    (
        r"\b(plus de stabilite|plus stable|pas assez stable|trop vive pour un relais|fatigante|epuisante|je corrige tout le temps|more stability|more stable|not stable enough|tiring|exhausting|constantly correcting)\b",
        &["PHILOSOPHY_SAFE"],
        0.7,
        "",
        "",
    ),
    // --- direction change / top speed --------------------------------------
    (
        r"\b(lente a changer de direction|molle dans les chicanes|paresseuse|lente dans les chicanes|ne veut pas changer d appui|lazy|slow to change direction|sluggish in (the )?chicanes|won t take a set|slow in the chicanes)\b",
        &["STR_SLOW_DIRECTION_CHANGE"],
        0.75,
        "",
        "medium",
    ),
    (
        r"\b(pas de vitesse de pointe|manque de vitesse de pointe|trop d aile|trop d appui|je me fais deposer|on me depose|deposee? en ligne droite|lente en ligne droite|no top speed|slow on the straights|too much wing|too much downforce|slow in a straight line|getting passed on the straights)\b",
        &["STR_TOP_SPEED_LOW"],
        0.75,
        "",
        "",
    ),
    // --- hybrid --------------------------------------------------------------
    (
        r"\b(plus de boost|pas de boost|batterie vide|batterie a plat|le moteur electrique ne pousse plus|no boost|battery flat|battery empty|no deployment|out of battery)\b",
        &["HYB_SOC_EMPTY"],
        0.8,
        "",
        "",
    ),
];

/// Modifier patterns (FR + EN, normalised).
const MODIFIERS: &[(&str, Modifier)] = &[
    // phase
    (
        r"\b(entree|entrees|en entree|a l entree|a l inscription|inscription|au freinage|en freinage|sur les freins|au lacher de frein|au lacher des freins|lacher de frein|quand je freine|en freinant|turn ?-? ?in|entry|entries|under braking|on the brakes|when i brake|brake release|on entry|into the corner|going in|corner entry|when i turn in|as i turn in)\b",
        Modifier::Phase(CornerPhase::Entry),
    ),
    (
        r"\b(milieu|milieu de virage|mi ?-? ?virage|en appui|a la corde|corde|apex|mid ?-? ?corner|middle|in the middle|loaded up|through the corner|mid)\b",
        Modifier::Phase(CornerPhase::Mid),
    ),
    (
        r"\b(sortie|sorties|en sortie|a la remise des gaz|remise des gaz|remise de gaz|a la reacceleration|reacceleration|a l acceleration|a l accel|quand j accelere|quand je remets les gaz|quand je reaccelere|sous gaz|sur les gaz|en acceleration|a la relance|relance|exit|exits|on throttle|on the throttle|on power|on the power|under power|under acceleration|when i accelerate|when i get on the power|getting on the power|power down|putting the power down|corner exit|on exit|coming out|when i get on the gas|on the gas)\b",
        Modifier::Phase(CornerPhase::Exit),
    ),
    // braking zone (sets the flag; the phase is entry)
    (
        r"\b(freinage|freinages|frein|freins|freine|freiner|freinant|braking|brake|brakes|brake zone|stopping|retrogradage|downshift|downshifts|downshifting)\b",
        Modifier::Braking,
    ),
    // corner speed
    (
        r"\b(lent|lents|lente|lentes|epingle|epingles|virages? lents?|courbes? lentes?|hairpins?|slow|tight|low ?-? ?speed|slow speed|1st gear|2nd gear|premiere|seconde|deuxieme|first gear|second gear)\b",
        Modifier::Speed(CornerSpeed::Slow),
    ),
    (
        r"\b(rapide|rapides|courbes? rapides?|grande vitesse|haute vitesse|a haute vitesse|a fond|a 250|a 200|fast|quick|high ?-? ?speed|flat ?-? ?out|high speed|at speed)\b",
        Modifier::Speed(CornerSpeed::Fast),
    ),
    (
        r"\b(moyen|moyens|moyenne|moyennes|medium|mid ?-? ?speed|3rd gear|troisieme|third gear|chicanes?)\b",
        Modifier::Speed(CornerSpeed::Medium),
    ),
    // axle
    (
        r"\b(avant|avants|av|lavant|lav|train avant|le nez|nez|front|fronts|front end|nose|front tyres|front tires|front wheels|roues avant|roue avant)\b",
        Modifier::Axle(Axle::Front),
    ),
    (
        r"\b(arriere|arrieres|ar|larriere|lar|train arriere|cul|derriere|rear|rears|rear end|back end|tail|back|rear tyres|rear tires|rear wheels|roues arriere|roue arriere)\b",
        Modifier::Axle(Axle::Rear),
    ),
    // intensity (the strongest wins)
    (
        r"\b(un peu|legerement|un poil|un chouia|un tout petit peu|un petit peu|a peine|slightly|a bit|a little|a touch|a tad|mildly|somewhat|marginally)\b",
        Modifier::Intensity(SymptomIntensity::Mild),
    ),
    (
        r"\b(beaucoup|pas mal|franchement|vraiment|clairement|fortement|nettement|sacrement|grave|quite|a lot|really|clearly|strongly|badly|heavily|plenty|very|pretty|seriously)\b",
        Modifier::Intensity(SymptomIntensity::Marked),
    ),
    (
        r"\b(enormement|trop|beaucoup trop|terriblement|horriblement|violemment|brutalement|d un coup|sans prevenir|massivement|inconduisible|catastrophique|affreux|affreusement|way too|massively|hugely|terribly|violently|suddenly|snap|undriveable|undrivable|awful|horrible|unbearable|out of nowhere)\b",
        Modifier::Intensity(SymptomIntensity::Severe),
    ),
    // rain
    (
        r"\b(pluie|sous la pluie|mouille|mouillee|humide|piste humide|piste mouillee|aquaplaning|aquaplane|wet|in the wet|rain|raining|damp|standing water|hydroplan\w*|aquaplan\w*)\b",
        Modifier::Wet,
    ),
    // small context words used to resolve generic families
    (
        r"\b(interieure?|roue interieure|inside|inner|inside wheel|one wheel)\b",
        Modifier::Inner,
    ),
    (
        r"\b(froid|froids|froide|froides|a froid|debut de relais|premiers tours|premier tour|out ?-? ?lap|cold|first laps?|early in the stint|start of the stint)\b",
        Modifier::Cold,
    ),
    (
        r"\b(chaud|chauds|chaude|chaudes|a chaud|surchauffe|fin de relais|apres \d+ tours|au bout de \d+ tours|hot|overheat\w*|late in the stint|end of the stint|after \d+ laps)\b",
        Modifier::Hot,
    ),
    (
        r"\b(vibreurs?|vibros?|kerbs?|curbs?|curbing|kerbing)\b",
        Modifier::Kerb,
    ),
    (
        r"\b(bosses?|bosselee?|cahots?|bumps?|bumpy)\b",
        Modifier::Bump,
    ),
    (
        r"\b(fond plat|le fond|etincelles?|talonne|talonnage|bottom\w*|sparks?|the floor)\b",
        Modifier::Bottom,
    ),
];

/// Words that split a text into clauses (besides `.` from the normalisation).
const CLAUSE_SPLIT: &str = r"\s*(\.|,|\bmais\b|\bet\b|\bpuis\b|\balors que\b|\bpar contre\b|\bsinon\b|\bbut\b|\band\b|\bthen\b|\bwhile\b|\bwhereas\b|\bhowever\b|\bthough\b)\s*";

/// Negation words looked for just before a match.
const NEG_BEFORE: &[&str] = &[
    "pas de",
    "pas d",
    "plus de",
    "plus d",
    "aucun",
    "aucune",
    "sans",
    "zero",
    "jamais de",
    "jamais",
    "no",
    "not",
    "never",
    "isn t",
    "doesn t",
    "don t",
    "didn t",
    "aren t",
    "wasn t",
    "no more",
    "hardly any",
    "without",
    "pas",
    "plus",
    "nothing",
    "zero",
    "non",
];
/// Words right after a `ne … ` match that negate it ("ne glisse plus").
const NEG_AFTER: &[&str] = &["plus", "pas", "jamais", "more", "anymore", "any more"];
/// Trailing words that say the symptom is gone.
const GONE_AFTER: &[&str] = &[
    "a disparu",
    "est parti",
    "est partie",
    "est regle",
    "est reglee",
    "is gone",
    "has gone",
    "gone",
    "is fixed",
    "fixed",
    "resolu",
    "resolue",
];

const VERDICT: &[(&str, &str, &str)] = &[
    (
        r"\b(c est mieux|mieux qu avant|bien mieux|beaucoup mieux|nettement mieux|un peu mieux|ca va mieux|better than before|much better|it s better|better now|improved)\b",
        "Verdict du pilote : mieux qu'avant.",
        "Driver's verdict: better than before.",
    ),
    (
        r"\b(c est pire|pire qu avant|bien pire|beaucoup pire|worse than before|much worse|it s worse|worse now|got worse)\b",
        "Verdict du pilote : pire qu'avant.",
        "Driver's verdict: worse than before.",
    ),
    (
        r"\b(c est pareil|pareil qu avant|pas de difference|aucune difference|rien n a change|same as before|no difference|no change|feels the same)\b",
        "Verdict du pilote : pareil qu'avant.",
        "Driver's verdict: no change.",
    ),
];

/// FR → EN of the clarification questions used by the dictionary.
const CLARIFY_EN: &[(&str, &str)] = &[
    (
        "Dans les virages lents, rapides ou les deux ?",
        "In slow corners, fast corners or both?",
    ),
    ("Lents ou rapides ?", "Slow or fast corners?"),
    ("Vitesse du virage ?", "How fast is the corner?"),
    (
        "Toujours sur les freins à ce moment-là, ou déjà relâchés ?",
        "Still on the brakes at that point, or already released?",
    ),
    (
        "Ça empire en fin de relais ?",
        "Does it get worse late in the stint?",
    ),
    ("Avec combien de gaz ?", "With how much throttle?"),
    ("Plus avec le plein ?", "Worse with a full tank?"),
    (
        "La roue bloque vraiment ou ça glisse juste ?",
        "Does the wheel really lock, or does it just slide?",
    ),
    (
        "Début ou fin de freinage ?",
        "Start or end of the braking zone?",
    ),
    (
        "Plutôt à l'inscription ou en appui ?",
        "On turn-in or once loaded up?",
    ),
    ("Freins encore engagés ?", "Still on the brakes?"),
    (
        "En ligne droite (avant de tourner) ou une fois le volant tourné ?",
        "In a straight line (before turning) or once the wheel is turned?",
    ),
    ("Au rétrogradage ?", "On downshifts?"),
    (
        "Plutôt bienvenu (rotation) ou dangereux ?",
        "Welcome (rotation) or dangerous?",
    ),
    ("Pneus chauds ou froids ?", "Tyres hot or cold?"),
    ("Sur bosses/vibreurs ?", "Over bumps / kerbs?"),
    (
        "Les deux roues patinent ou seulement l'intérieure ? (ou laisser la télémétrie trancher)",
        "Both wheels spinning or just the inside one? (or let the telemetry decide)",
    ),
    (
        "Dès le début du relais ou en fin ?",
        "From the start of the stint or late in it?",
    ),
    ("Avec le TC à combien ?", "With TC at what level?"),
    ("Intérieure seule ?", "Inside wheel only?"),
    ("Sur bosses ?", "Over bumps?"),
    ("TC réglé à combien ?", "TC set to what?"),
    ("Sur les vibreurs intérieurs ?", "On the inside kerbs?"),
    (
        "Est-ce que ça patinerait sans ?",
        "Would it spin without it?",
    ),
    (
        "Valeurs TC/cut/slip actuelles ?",
        "Current TC / cut / slip values?",
    ),
    ("Batterie vide ?", "Battery empty?"),
    ("Map moteur ?", "Engine map?"),
    (
        "Au freinage, en appui ou sous gaz ?",
        "Under braking, loaded up or on power?",
    ),
    (
        "Sur une bosse/compression précise (Eau Rouge, Porsche Curves) ?",
        "On a specific bump / compression (Eau Rouge, Porsche Curves)?",
    ),
    (
        "Sensation de talonnage ?",
        "Does it feel like bottoming out?",
    ),
    ("Étincelles / bruit de fond plat ?", "Sparks / floor noise?"),
    ("Avec le plein ?", "With a full tank?"),
    ("Plutôt rapide ou lent ?", "Fast or slow corners?"),
    ("Pneus dans la fenêtre ?", "Tyres in their window?"),
    (
        "Plus en début ou fin de relais ?",
        "More at the start or the end of the stint?",
    ),
    ("Avant de tourner le volant ?", "Before turning the wheel?"),
    ("Sur bosses (Sebring) ?", "Over bumps (Sebring)?"),
    (
        "Début (haute vitesse) ou fin (basse vitesse) du freinage ?",
        "Start (high speed) or end (low speed) of braking?",
    ),
    (
        "Les deux roues ou l'intérieure ?",
        "Both wheels or the inside one?",
    ),
    (
        "Hypercar : migration réglée à combien ? Batterie ?",
        "Hypercar: brake migration setting? Battery?",
    ),
    ("LMH : batterie pleine ?", "LMH: battery full?"),
    (
        "Avec beaucoup de volant encore sur les freins ?",
        "With a lot of steering still on the brakes?",
    ),
    (
        "Combien de tours avant que ça vienne ?",
        "How many laps before they come in?",
    ),
    ("Conduits actuels ?", "Current brake ducts?"),
    ("Début ou fin de relais ?", "Start or end of the stint?"),
    ("Après combien de tours ?", "After how many laps?"),
    ("Avant ou arrière ?", "Front or rear?"),
    (
        "Hypercar : état de charge quand ça arrive ?",
        "Hypercar: state of charge when it happens?",
    ),
    (
        "Vous cherchez plus de rotation ou plus de sécurité ?",
        "Looking for more rotation or more safety?",
    ),
    ("Avec blocages ou sans ?", "With lock-ups or without?"),
    ("Où sur le circuit ?", "Where on the track?"),
    (
        "Sur bosses ou en compression aéro ?",
        "Over bumps or in an aero compression?",
    ),
    (
        "Vibreurs intérieurs (apex) ou extérieurs (sortie) ?",
        "Inside (apex) or outside (exit) kerbs?",
    ),
    (
        "C'est l'avant ou l'arrière qui réagit ?",
        "Is it the front or the rear that reacts?",
    ),
    (
        "Freinage, virage ou ligne droite ?",
        "Braking, cornering or straight line?",
    ),
    ("Quel virage ?", "Which corner?"),
    (
        "En roulis (virages) ou en tangage (freinage/accélération) ?",
        "In roll (corners) or pitch (braking / acceleration)?",
    ),
    (
        "Surtout sur bosses ou partout ?",
        "Mostly over bumps or everywhere?",
    ),
    ("Entrée ou milieu ?", "Entry or mid-corner?"),
    ("Rapides ?", "Fast corners?"),
    ("Chicanes rapides ou lentes ?", "Fast or slow chicanes?"),
    (
        "Même carburant/ERS que les autres ?",
        "Same fuel / ERS as the others?",
    ),
    ("Régime max atteint ?", "Hitting the rev limiter?"),
    (
        "Dès le début ou après X tours ?",
        "From the start or after a few laps?",
    ),
    (
        "Ça pousse en même temps ?",
        "Does it push at the same time?",
    ),
    ("Patinage en sortie ?", "Wheelspin on exit?"),
    ("TC ?", "TC?"),
    (
        "Pression/température vues au HUD ?",
        "Pressure / temperature seen on the HUD?",
    ),
    ("Quel composé ?", "Which compound?"),
    ("Température piste ?", "Track temperature?"),
    ("Composé ?", "Compound?"),
    (
        "Pression actuelle vs mini ?",
        "Current pressure vs the minimum?",
    ),
    ("Motor map / regen actuels ?", "Current motor map / regen?"),
    (
        "Où manque la rotation : entrée, milieu, sortie ?",
        "Where is rotation missing: entry, mid or exit?",
    ),
    (
        "Quelle phase vous coûte le plus ?",
        "Which phase costs you the most?",
    ),
    ("Réglage FFB vérifié ?", "FFB setting checked?"),
    (
        "Léger tout le temps ou seulement quand ça pousse ?",
        "Light all the time or only when it pushes?",
    ),
    ("Rétrogradages tardifs ?", "Late downshifts?"),
    ("Lift & coast pratiqué ?", "Lift & coast used?"),
    ("Fuel ratio ?", "Fuel ratio?"),
];

fn clarify_en(fr: &str) -> String {
    CLARIFY_EN
        .iter()
        .find(|(f, _)| *f == fr)
        .map(|(_, e)| e.to_string())
        .unwrap_or_else(|| fr.to_string())
}

/// Generic questions, by the attribute that is missing.
const Q_PHASE: (&str, &str) = (
    "À quel moment du virage : entrée (freinage), milieu (corde) ou sortie (remise des gaz) ?",
    "At which point of the corner: entry (braking), mid (apex) or exit (on throttle)?",
);
const Q_SPEED: (&str, &str) = (
    "Dans les virages lents, rapides ou les deux ?",
    "In slow corners, fast corners or both?",
);
const Q_AXLE: (&str, &str) = ("Plutôt l'avant ou l'arrière ?", "Front or rear?");
const Q_INNER: (&str, &str) = (
    "Les deux roues patinent ou seulement l'intérieure ?",
    "Both wheels spinning or just the inside one?",
);
const Q_JUMPS: (&str, &str) = (
    "Ça saute sur les vibreurs, sur les bosses, ou la voiture talonne (étincelles) ?",
    "Does it jump on the kerbs, over bumps, or is the car bottoming out (sparks)?",
);
const Q_NO_GRIP: (&str, &str) = (
    "Pas de grip : pneus froids (début de relais), surchauffe (fin de relais), ou un problème d'équilibre avant / arrière ?",
    "No grip: cold tyres (start of stint), overheating (end of stint), or a front / rear balance problem?",
);
const Q_UNSTABLE: (&str, &str) = (
    "Instable où : au freinage en ligne droite, à l'entrée du virage, ou dans les courbes rapides ?",
    "Unstable where: braking in a straight line, on corner entry, or in fast corners?",
);
const Q_NOTHING: (&str, &str) = (
    "Je n'ai pas reconnu de symptôme. Dis-moi ce que fait la voiture (sous-virage, survirage, blocage, patinage…), à quel moment du virage et dans quels virages (lents ou rapides).",
    "I did not recognise a symptom. Tell me what the car does (understeer, oversteer, lock-ups, wheelspin…), at which point of the corner and in which corners (slow or fast).",
);

/// Dictionary + regex extractor (offline).
pub struct DictionaryExtractor {
    patterns: Vec<Pattern>,
    modifiers: Vec<ModifierRule>,
    clause_split: Regex,
    verdicts: Vec<(Regex, &'static str, &'static str)>,
}

impl std::fmt::Debug for DictionaryExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DictionaryExtractor({} patterns)", self.patterns.len())
    }
}

fn phase_of_yaml(s: Option<&str>) -> (Option<CornerPhase>, bool) {
    match s {
        Some("entry") => (Some(CornerPhase::Entry), false),
        Some("mid") => (Some(CornerPhase::Mid), false),
        Some("exit") => (Some(CornerPhase::Exit), false),
        Some("braking") => (Some(CornerPhase::Entry), true),
        _ => (None, false),
    }
}

fn speed_of_yaml(s: Option<&str>) -> Option<CornerSpeed> {
    match s {
        Some("slow") => Some(CornerSpeed::Slow),
        Some("medium") => Some(CornerSpeed::Medium),
        Some("fast") => Some(CornerSpeed::Fast),
        _ => None,
    }
}

/// Symptom templates of a dictionary entry: balance symptoms of one family
/// collapse into `FAM_{PHASE}_{SPEED}` with the parts every id shares fixed;
/// globs and axle / wheel pairs become their generic family.
type PhaseSpeedSets = (Vec<Option<CornerPhase>>, Vec<Option<CornerSpeed>>);

fn templates_of(symptoms: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut balance: HashMap<&str, PhaseSpeedSets> = HashMap::new();
    for s in symptoms {
        let s = s.as_str();
        if let Some(glob) = s.strip_suffix("_*") {
            match glob {
                "US" | "OS" => {
                    balance.entry(glob).or_default();
                }
                "US_ENTRY" | "US_MID" | "US_EXIT" | "OS_ENTRY" | "OS_MID" | "OS_EXIT" => {
                    let (fam, ph, _) = parse_balance_id(glob).expect("balance glob");
                    let e = balance.entry(fam).or_default();
                    e.0.push(ph);
                }
                "TRC" => push_unique(&mut out, "TRC_WHEELSPIN_{INNER}"),
                "BRK_LOCK" => push_unique(&mut out, "BRK_LOCK_{AXLE}"),
                _ => {}
            }
            continue;
        }
        if let Some((fam, ph, sp)) = parse_balance_id(s) {
            let e = balance.entry(fam).or_default();
            e.0.push(ph);
            e.1.push(sp);
            continue;
        }
        push_unique(&mut out, s);
    }
    // Pairs that are one family with an unknown wheel / axle.
    for (pair, generic) in [
        (
            ["TRC_WHEELSPIN_BOTH", "TRC_WHEELSPIN_INNER"],
            "TRC_WHEELSPIN_{INNER}",
        ),
        (["BRK_LOCK_FRONT", "BRK_LOCK_REAR"], "BRK_LOCK_{AXLE}"),
        (["TYR_HOT_FRONT", "TYR_HOT_REAR"], "TYR_HOT_{AXLE}"),
    ] {
        if pair.iter().all(|p| out.iter().any(|o| o == p)) {
            out.retain(|o| !pair.contains(&o.as_str()));
            push_unique(&mut out, generic);
        }
    }
    let mut fams: Vec<(&str, PhaseSpeedSets)> = balance.into_iter().collect();
    fams.sort_by(|a, b| a.0.cmp(b.0));
    for (fam, (phases, speeds)) in fams {
        let fixed_phase = single(&phases);
        let fixed_speed = single(&speeds);
        let ph = match fixed_phase {
            Some(CornerPhase::Entry) => "ENTRY",
            Some(CornerPhase::Mid) => "MID",
            Some(CornerPhase::Exit) => "EXIT",
            None => "{PHASE}",
        };
        let sp = match fixed_speed {
            Some(CornerSpeed::Slow) => "SLOW",
            Some(CornerSpeed::Fast) => "FAST",
            _ => "{SPEED}",
        };
        out.push(format!("{fam}_{ph}_{sp}"));
    }
    out
}

/// The one value every element carries, if they all carry the same one.
fn single<T: PartialEq + Copy>(v: &[Option<T>]) -> Option<T> {
    let mut it = v.iter().flatten();
    let first = *it.next()?;
    if v.iter().all(|x| *x == Some(first)) {
        Some(first)
    } else {
        None
    }
}

fn push_unique(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// A literal phrase of the dictionary as a regex over normalised text.
fn phrase_regex(phrase: &str) -> Option<Regex> {
    let norm = normalise(phrase);
    let words: Vec<&str> = norm
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return None;
    }
    let body = words
        .iter()
        .map(|w| regex::escape(w))
        .collect::<Vec<_>>()
        .join(r"\s+");
    Regex::new(&format!(r"\b{body}\b")).ok()
}

impl DictionaryExtractor {
    /// The embedded dictionary.
    pub fn embedded() -> Result<Self> {
        Self::new(&FeedbackDictionary::embedded()?)
    }

    pub fn new(dict: &FeedbackDictionary) -> Result<Self> {
        let mut patterns = Vec::new();
        for (re, templates, conf, phase, speed) in BUILTIN {
            let (default_phase, _) = phase_of_yaml(Some(*phase).filter(|p| !p.is_empty()));
            patterns.push(Pattern {
                re: Regex::new(re)
                    .map_err(|e| EngineerError::Knowledge(format!("regex {re}: {e}")))?,
                templates: templates.iter().map(|t| t.to_string()).collect(),
                base_confidence: *conf,
                default_phase,
                default_speed: speed_of_yaml(Some(*speed).filter(|s| !s.is_empty())),
                clarify: Vec::new(),
            });
        }
        for e in &dict.entries {
            let templates = templates_of(&e.symptoms);
            if templates.is_empty() {
                continue; // FEEL_STEERING_HEAVY: not a setup symptom
            }
            let (default_phase, _) = phase_of_yaml(e.default_phase.as_deref());
            let default_speed = speed_of_yaml(e.default_corner_speed.as_deref());
            for phrase in e.fr.iter().chain(e.en.iter()) {
                let Some(re) = phrase_regex(phrase) else {
                    continue;
                };
                patterns.push(Pattern {
                    re,
                    templates: templates.clone(),
                    base_confidence: 0.85,
                    default_phase,
                    default_speed,
                    clarify: e.clarify.clone(),
                });
            }
        }
        let modifiers = MODIFIERS
            .iter()
            .map(|(re, m)| {
                Regex::new(re)
                    .map(|re| ModifierRule { re, set: *m })
                    .map_err(|e| EngineerError::Knowledge(format!("regex {re}: {e}")))
            })
            .collect::<Result<Vec<_>>>()?;
        let verdicts = VERDICT
            .iter()
            .map(|(re, fr, en)| {
                Regex::new(re)
                    .map(|re| (re, *fr, *en))
                    .map_err(|e| EngineerError::Knowledge(format!("regex {re}: {e}")))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            patterns,
            modifiers,
            clause_split: Regex::new(CLAUSE_SPLIT).expect("clause regex"),
            verdicts,
        })
    }

    /// Number of compiled symptom patterns (dictionary phrases + built-ins).
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    fn context_of(&self, clause: &str) -> Context {
        let mut ctx = Context::default();
        // Speed words first, then mask them so "mid-speed" does not read as phase "mid".
        let mut masked = clause.to_string();
        for m in &self.modifiers {
            if let Modifier::Speed(_) = m.set {
                for hit in m.re.find_iter(clause) {
                    ctx.apply(m.set);
                    masked.replace_range(hit.range(), &" ".repeat(hit.len()));
                }
            }
        }
        for m in &self.modifiers {
            if let Modifier::Speed(_) = m.set {
                continue;
            }
            let hay: &str = if let Modifier::Phase(_) = m.set {
                &masked
            } else {
                clause
            };
            if m.re.is_match(hay) {
                ctx.apply(m.set);
            }
        }
        if ctx.braking && ctx.phase.is_none() {
            ctx.phase = Some(CornerPhase::Entry);
        }
        ctx
    }

    /// Non-overlapping symptom matches of a clause (longest first, then earliest).
    fn matches_of(&self, clause: &str) -> Vec<(usize, usize, usize)> {
        let mut hits: Vec<(usize, usize, usize)> = Vec::new();
        for (i, p) in self.patterns.iter().enumerate() {
            for m in p.re.find_iter(clause) {
                if m.end() > m.start() {
                    hits.push((m.start(), m.end(), i));
                }
            }
        }
        hits.sort_by(|a, b| (b.1 - b.0).cmp(&(a.1 - a.0)).then(a.0.cmp(&b.0)));
        let mut kept: Vec<(usize, usize, usize)> = Vec::new();
        for h in hits {
            if kept.iter().all(|k| h.1 <= k.0 || h.0 >= k.1) {
                kept.push(h);
            }
        }
        kept.sort_by_key(|k| k.0);
        kept
    }
}

fn words_before(clause: &str, start: usize, n: usize) -> Vec<&str> {
    let mut w: Vec<&str> = clause[..start].split_whitespace().collect();
    let keep = w.len().saturating_sub(n);
    w.drain(..keep);
    w
}

fn words_after(clause: &str, end: usize, n: usize) -> Vec<&str> {
    clause[end..].split_whitespace().take(n).collect()
}

/// Is the match negated by the words around it?
fn negated(clause: &str, start: usize, end: usize) -> bool {
    let before = words_before(clause, start, 3).join(" ");
    for neg in NEG_BEFORE {
        if before == *neg || before.ends_with(&format!(" {neg}")) {
            return true;
        }
    }
    let matched = &clause[start..end];
    let after = words_after(clause, end, 2);
    let after1 = after.first().copied().unwrap_or("");
    let after2 = after.join(" ");
    let last_word = matched.split_whitespace().last().unwrap_or("");
    if NEG_AFTER.contains(&last_word) {
        // "ne tourne pas" is the symptom itself, not a negation of it.
        return false;
    }
    let has_ne = before.ends_with(" ne")
        || before == "ne"
        || before.ends_with(" n")
        || before == "n"
        || matched.contains(" ne ")
        || matched.starts_with("ne ")
        || matched.contains(" n ");
    if has_ne && NEG_AFTER.contains(&after1) {
        return true;
    }
    GONE_AFTER
        .iter()
        .any(|g| after2 == *g || after2.starts_with(&format!("{g} ")) || after1 == *g)
}

/// Resolve one template in the context of its clause.
fn resolve(
    tpl: &str,
    ctx: &Context,
    p: &Pattern,
) -> Vec<(String, Option<CornerPhase>, Option<CornerSpeed>, f32)> {
    let phase = ctx.phase.or(p.default_phase);
    let speed = ctx.speed.or(p.default_speed);
    let axle = ctx.effective_axle();
    let balance =
        |fam: &str, fixed_phase: Option<CornerPhase>, fixed_speed: Option<CornerSpeed>| {
            let ph = fixed_phase.or(phase);
            let sp = fixed_speed.or(speed);
            let mut id = fam.to_string();
            if let Some(ph) = ph {
                id.push('_');
                id.push_str(match ph {
                    CornerPhase::Entry => "ENTRY",
                    CornerPhase::Mid => "MID",
                    CornerPhase::Exit => "EXIT",
                });
            }
            if let Some(sp) = sp {
                id.push('_');
                id.push_str(match sp {
                    CornerSpeed::Fast => "FAST",
                    _ => "SLOW",
                });
            }
            (id, ph, sp, 1.0)
        };
    match tpl {
        t if t.starts_with("US_") || t.starts_with("OS_") => {
            let mut parts = t.split('_');
            let fam = parts.next().unwrap_or("US");
            let ph = parts.next().and_then(phase_of_tag);
            let sp = parts.next().and_then(speed_of_tag);
            vec![balance(fam, ph, sp)]
        }
        "BRK_LOCK_{AXLE}" => {
            let id = if ctx.inner {
                "BRK_LOCK_INNER_FRONT"
            } else {
                match axle {
                    Some(Axle::Front) => "BRK_LOCK_FRONT",
                    Some(Axle::Rear) => "BRK_LOCK_REAR",
                    None => "BRK_LOCK",
                }
            };
            vec![(id.into(), Some(CornerPhase::Entry), speed, 1.0)]
        }
        "TRC_WHEELSPIN_{INNER}" => {
            let id = if ctx.inner {
                "TRC_WHEELSPIN_INNER"
            } else {
                "TRC_WHEELSPIN"
            };
            vec![(id.into(), Some(CornerPhase::Exit), speed, 1.0)]
        }
        "TYR_HOT_{AXLE}" => {
            let id = match axle {
                Some(Axle::Front) => "TYR_HOT_FRONT",
                Some(Axle::Rear) => "TYR_HOT_REAR",
                None => "TYR_HOT",
            };
            vec![(id.into(), None, None, 1.0)]
        }
        "UNSTABLE" => {
            if ctx.braking {
                vec![(
                    "BRK_UNSTABLE_STRAIGHT".into(),
                    Some(CornerPhase::Entry),
                    speed,
                    1.0,
                )]
            } else if phase == Some(CornerPhase::Entry) {
                vec![balance("OS", Some(CornerPhase::Entry), None)]
            } else if speed == Some(CornerSpeed::Fast) {
                vec![balance("OS", None, Some(CornerSpeed::Fast))]
            } else if axle == Some(Axle::Rear) {
                vec![balance("OS", None, None)]
            } else {
                vec![("PLT_NERVOUS".into(), phase, speed, 0.75)]
            }
        }
        "NO_GRIP" => {
            if ctx.wet {
                vec![("WET_NO_TEMP".into(), None, None, 1.0)]
            } else if ctx.cold {
                vec![("TYR_COLD".into(), None, None, 1.0)]
            } else if ctx.hot {
                let id = match axle {
                    Some(Axle::Front) => "TYR_HOT_FRONT",
                    Some(Axle::Rear) => "TYR_HOT_REAR",
                    None => "TYR_HOT",
                };
                vec![(id.into(), None, None, 1.0)]
            } else if axle == Some(Axle::Front) {
                vec![balance("US", None, None)]
            } else if axle == Some(Axle::Rear) {
                vec![balance("OS", None, None)]
            } else {
                vec![("NO_GRIP".into(), phase, speed, 0.8)]
            }
        }
        "JUMPS" => {
            if ctx.kerb {
                vec![("PLT_KERB_UNSETTLED".into(), phase, speed, 1.0)]
            } else if ctx.bump {
                vec![("PLT_BUMPY_LOSS".into(), phase, speed, 1.0)]
            } else if ctx.bottom {
                vec![("PLT_BOTTOMING".into(), phase, speed, 1.0)]
            } else {
                vec![("JUMPS".into(), phase, speed, 0.8)]
            }
        }
        "TYR_COLD" if ctx.wet => vec![("WET_NO_TEMP".into(), None, None, 1.0)],
        other => {
            let ph = if other.starts_with("BRK_") || other == "ABS_TOO_ACTIVE" {
                Some(CornerPhase::Entry)
            } else if other.starts_with("TRC_") || other == "TC_TOO_INTRUSIVE" {
                Some(CornerPhase::Exit)
            } else {
                phase
            };
            vec![(other.to_string(), ph, speed, 1.0)]
        }
    }
}

#[derive(Debug, Clone)]
struct Clause {
    start: usize,
    end: usize,
    text: String,
    /// The separator before this clause ended a sentence (`.`), so the
    /// previous clause's context does not carry over.
    new_sentence: bool,
}

impl SymptomExtractor for DictionaryExtractor {
    fn kind(&self) -> ExtractorKind {
        ExtractorKind::Dictionary
    }

    fn extract(&self, text: &str, _lang_hint: Option<&str>) -> FeedbackInterpretation {
        let original: Vec<char> = text.chars().collect();
        let norm = normalise(text);
        // Clauses, with their position in the normalised (= original) text.
        let mut clauses: Vec<Clause> = Vec::new();
        let mut last = 0usize;
        let mut new_sentence = true;
        for sep in self.clause_split.find_iter(&norm) {
            if sep.start() > last {
                clauses.push(Clause {
                    start: last,
                    end: sep.start(),
                    text: norm[last..sep.start()].to_string(),
                    new_sentence,
                });
            }
            new_sentence = sep.as_str().contains('.');
            last = sep.end();
        }
        if last < norm.len() {
            clauses.push(Clause {
                start: last,
                end: norm.len(),
                text: norm[last..].to_string(),
                new_sentence,
            });
        }
        let clauses: Vec<Clause> = clauses
            .into_iter()
            .filter(|c| c.text.trim().len() >= 2)
            .collect();

        // Pass 1: per clause, matches + context.
        struct Scanned {
            clause: Clause,
            ctx: Context,
            hits: Vec<(usize, usize, usize)>,
        }
        let mut scanned: Vec<Scanned> = clauses
            .into_iter()
            .map(|clause| {
                let ctx = self.context_of(&clause.text);
                let hits = self.matches_of(&clause.text);
                Scanned { clause, ctx, hits }
            })
            .collect();

        // Pass 2: modifier-only clauses lend their context forward, then backward; inside
        // one sentence, a symptom clause without phase / speed keeps the previous clause's
        // ("instable au freinage, l'arrière bouge").
        let n = scanned.len();
        let mut pending: Option<Context> = None;
        let mut used_as_modifier = vec![false; n];
        let mut carried: Option<Context> = None;
        for i in 0..n {
            if scanned[i].hits.is_empty() {
                if scanned[i].ctx.any {
                    let mut p = pending.take().unwrap_or_default();
                    p.inherit(&scanned[i].ctx);
                    pending = Some(p);
                    used_as_modifier[i] = true;
                }
            } else {
                if let Some(p) = pending.take() {
                    scanned[i].ctx.inherit(&p);
                }
                if !scanned[i].clause.new_sentence {
                    if let Some(c) = &carried {
                        scanned[i].ctx.inherit_sentence(c);
                    }
                }
                carried = Some(scanned[i].ctx.clone());
            }
            if scanned[i].clause.new_sentence && scanned[i].hits.is_empty() {
                carried = None;
            }
        }
        if let Some(p) = pending {
            if let Some(last_sym) = scanned.iter().rposition(|s| !s.hits.is_empty()) {
                scanned[last_sym].ctx.inherit(&p);
            }
        }

        // Pass 3: symptoms.
        let mut symptoms: Vec<DetectedSymptom> = Vec::new();
        let mut questions: Vec<(String, String)> = Vec::new();
        let mut notes_fr: Vec<String> = Vec::new();
        let mut unmatched: Vec<String> = Vec::new();
        let mut matched_clauses = 0usize;
        let mut total_clauses = 0usize;
        for (i, s) in scanned.iter().enumerate() {
            let is_verdict = self
                .verdicts
                .iter()
                .any(|(re, _, _)| re.is_match(&s.clause.text));
            for (re, fr, _) in &self.verdicts {
                if re.is_match(&s.clause.text) && !notes_fr.iter().any(|n| n == fr) {
                    notes_fr.push(fr.to_string());
                }
            }
            if is_verdict && s.hits.is_empty() {
                continue;
            }
            total_clauses += 1;
            if s.hits.is_empty() {
                if !used_as_modifier[i] {
                    let frag: String = original[s.clause.start..s.clause.end].iter().collect();
                    let frag = frag.trim().to_string();
                    if !frag.is_empty() {
                        unmatched.push(frag.chars().take(80).collect());
                    }
                }
                continue;
            }
            matched_clauses += 1;
            // Explicit phase word in the clause (not just a default) → OS + braking stays OS.
            let explicit_phase = self
                .modifiers
                .iter()
                .any(|m| matches!(m.set, Modifier::Phase(_)) && m.re.is_match(&s.clause.text));
            for &(start, end, pi) in &s.hits {
                let p = &self.patterns[pi];
                let matched_text: String = original[s.clause.start + start..s.clause.start + end]
                    .iter()
                    .collect();
                let neg = negated(&s.clause.text, start, end);
                // Axle words inside the match win over the clause (one clause may name both ends).
                let mut ctx = s.ctx.clone();
                if ctx.axle_conflict {
                    let inner = self.context_of(&s.clause.text[start..end]);
                    if let Some(a) = inner.axle {
                        ctx.axle = Some(a);
                        ctx.axle_conflict = false;
                    }
                }
                let mut resolved: Vec<(String, Option<CornerPhase>, Option<CornerSpeed>, f32)> =
                    Vec::new();
                for tpl in &p.templates {
                    resolved.extend(resolve(tpl, &ctx, p));
                }
                // Oversteer felt in the braking zone without an explicit phase word: could as
                // well be straight-line instability (FEEL_REAR_LOOSE_ON_BRAKES).
                if ctx.braking
                    && !explicit_phase
                    && resolved
                        .iter()
                        .any(|r| r.0.starts_with("OS") && !r.0.contains("FAST"))
                    && !resolved.iter().any(|r| r.0 == "BRK_UNSTABLE_STRAIGHT")
                {
                    resolved.push((
                        "BRK_UNSTABLE_STRAIGHT".into(),
                        Some(CornerPhase::Entry),
                        ctx.speed,
                        0.9,
                    ));
                    questions.push((
                        "En ligne droite (avant de tourner) ou une fois le volant tourné ?".into(),
                        "In a straight line (before turning) or once the wheel is turned?".into(),
                    ));
                }
                let spread = match resolved.len() {
                    0 | 1 => 1.0,
                    2 => 0.75,
                    _ => 0.6,
                };
                for (id, phase, speed, factor) in resolved {
                    let mut conf = p.base_confidence * factor * spread;
                    if phase.is_some() && parse_balance_id(&id).is_some() {
                        conf += 0.05;
                    }
                    if speed.is_some() && parse_balance_id(&id).is_some() {
                        conf += 0.05;
                    }
                    let conf = conf.clamp(0.05, 0.95);
                    let (label_fr, label_en) = symptom_label(&id);
                    let axle = match id.as_str() {
                        s if s.starts_with("US") || s.contains("FRONT") || s == "TYR_HOT_FRONT" => {
                            Some(Axle::Front)
                        }
                        s if s.starts_with("OS") || s.contains("REAR") || s.starts_with("TRC_") => {
                            Some(Axle::Rear)
                        }
                        _ => ctx.effective_axle(),
                    };
                    let sym = DetectedSymptom {
                        symptom_id: id,
                        label_fr,
                        label_en,
                        phase,
                        corner_speed: speed,
                        axle,
                        wet: ctx.wet,
                        intensity: ctx.intensity,
                        negated: neg,
                        matched_text: matched_text.trim().to_string(),
                        confidence: conf,
                    };
                    // Same symptom + attributes already seen: keep the surest one.
                    if let Some(prev) = symptoms.iter_mut().find(|x| {
                        x.symptom_id == sym.symptom_id
                            && x.phase == sym.phase
                            && x.corner_speed == sym.corner_speed
                            && x.negated == sym.negated
                    }) {
                        if sym.confidence > prev.confidence {
                            *prev = sym;
                        }
                        continue;
                    }
                    symptoms.push(sym);
                }
                // Dictionary questions that are not about phase / speed (those are generated).
                for q in p.clarify.iter().take(2) {
                    let lower = normalise(q);
                    let about_phase_speed = lower.contains("lent")
                        || lower.contains("rapide")
                        || lower.contains("entree")
                        || lower.contains("milieu")
                        || lower.contains("sortie")
                        || lower.contains("vitesse du virage");
                    if !about_phase_speed && !questions.iter().any(|(f, _)| f == q) {
                        questions.push((q.clone(), clarify_en(q)));
                    }
                }
            }
        }

        // "rear snaps on exit, way too loose": the bare family (OS) of a later clause is the
        // same symptom as the precise one (OS_EXIT); merge it, keeping its intensity.
        let mut i = 0;
        while i < symptoms.len() {
            let generic = symptoms[i].clone();
            let Some((fam, gp, gs)) = parse_balance_id(&generic.symptom_id) else {
                i += 1;
                continue;
            };
            let target = symptoms.iter().position(|o| {
                o.symptom_id != generic.symptom_id
                    && o.negated == generic.negated
                    && parse_balance_id(&o.symptom_id).is_some_and(|(f, p, s)| {
                        f == fam && (gp.is_none() || p == gp) && (gs.is_none() || s == gs)
                    })
            });
            match target {
                Some(t) => {
                    let o = &mut symptoms[t];
                    if o.intensity.is_none() {
                        o.intensity = generic.intensity;
                    }
                    o.wet |= generic.wet;
                    o.confidence = o.confidence.max(generic.confidence);
                    symptoms.remove(i);
                }
                None => i += 1,
            }
        }

        // A kerb / bump / bottoming word next to a generic "jumps" explains it.
        if symptoms.iter().any(|s| {
            ["PLT_KERB_UNSETTLED", "PLT_BUMPY_LOSS", "PLT_BOTTOMING"]
                .contains(&s.symptom_id.as_str())
        }) {
            symptoms.retain(|s| s.symptom_id != "JUMPS");
        }
        // A precise balance symptom makes the bare "no grip" of the same text redundant.
        if symptoms
            .iter()
            .any(|s| parse_balance_id(&s.symptom_id).is_some())
        {
            symptoms.retain(|s| s.symptom_id != "NO_GRIP");
        }

        // Generated questions for what is still missing (non-negated symptoms only).
        let mut generated: Vec<(&str, &str)> = Vec::new();
        for s in symptoms.iter().filter(|s| !s.negated) {
            match s.symptom_id.as_str() {
                id if parse_balance_id(id).is_some() => {
                    if s.phase.is_none() {
                        generated.push(Q_PHASE);
                    }
                    if s.corner_speed.is_none() {
                        generated.push(Q_SPEED);
                    }
                }
                "BRK_LOCK" | "TYR_HOT" => generated.push(Q_AXLE),
                "TRC_WHEELSPIN" => generated.push(Q_INNER),
                "JUMPS" => generated.push(Q_JUMPS),
                "NO_GRIP" => generated.push(Q_NO_GRIP),
                "PLT_NERVOUS" if s.matched_text.len() < 14 => generated.push(Q_UNSTABLE),
                _ => {}
            }
        }
        let mut questions_fr: Vec<String> = Vec::new();
        let mut questions_en: Vec<String> = Vec::new();
        for (fr, en) in generated
            .into_iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .chain(questions)
        {
            if !questions_fr.contains(&fr) && questions_fr.len() < 4 {
                questions_fr.push(fr);
                questions_en.push(en);
            }
        }
        if symptoms.is_empty() {
            questions_fr = vec![Q_NOTHING.0.to_string()];
            questions_en = vec![Q_NOTHING.1.to_string()];
        }

        let confidence = if symptoms.is_empty() {
            0.0
        } else {
            let mean = symptoms.iter().map(|s| s.confidence).sum::<f32>() / symptoms.len() as f32;
            let coverage = if total_clauses == 0 {
                1.0
            } else {
                matched_clauses as f32 / total_clauses as f32
            };
            (mean * (0.5 + 0.5 * coverage)).clamp(0.0, 1.0)
        };
        FeedbackInterpretation {
            feedback_id: 0,
            symptoms,
            questions_fr,
            questions_en,
            unmatched_fragments: unmatched,
            extractor: ExtractorKind::Dictionary,
            confidence,
            notes_fr,
        }
    }
}

/// Shared embedded extractor (compiled once per process).
pub fn default_extractor() -> &'static DictionaryExtractor {
    static CELL: OnceLock<DictionaryExtractor> = OnceLock::new();
    CELL.get_or_init(|| DictionaryExtractor::embedded().expect("embedded feedback dictionary"))
}

// ---------------------------------------------------------------------------
// Anthropic extractor (opt-in)
// ---------------------------------------------------------------------------

/// Environment variable holding the API key (enables the LLM extractor).
pub const ANTHROPIC_KEY_ENV: &str = "CORDE_ANTHROPIC_API_KEY";
pub const ANTHROPIC_DEFAULT_MODEL: &str = "claude-haiku-5-5";
pub const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const ANTHROPIC_TIMEOUT: Duration = Duration::from_secs(8);

/// Calls the Anthropic Messages API with a strict JSON schema; falls back to
/// the dictionary (with a note) on any error or empty answer.
pub struct AnthropicExtractor {
    api_key: String,
    model: String,
    endpoint: String,
    timeout: Duration,
    fallback: DictionaryExtractor,
    allowed: Vec<String>,
}

impl std::fmt::Debug for AnthropicExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AnthropicExtractor(model={}, endpoint={})",
            self.model, self.endpoint
        )
    }
}

/// What the model must answer (a subset of [`FeedbackInterpretation`]).
#[derive(Debug, Deserialize)]
struct LlmOutput {
    #[serde(default)]
    symptoms: Vec<LlmSymptom>,
    #[serde(default)]
    questions_fr: Vec<String>,
    #[serde(default)]
    questions_en: Vec<String>,
    #[serde(default)]
    unmatched_fragments: Vec<String>,
    #[serde(default)]
    confidence: f32,
}

#[derive(Debug, Deserialize)]
struct LlmSymptom {
    symptom_id: String,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    corner_speed: Option<String>,
    #[serde(default)]
    axle: Option<String>,
    #[serde(default)]
    wet: bool,
    #[serde(default)]
    intensity: Option<String>,
    #[serde(default)]
    negated: bool,
    #[serde(default)]
    matched_text: String,
    #[serde(default)]
    confidence: f32,
}

/// Symptom ids the model may use: every rule symptom plus the generic families.
pub fn allowed_symptom_ids(rule_symptoms: &[String]) -> Vec<String> {
    let mut v: Vec<String> = rule_symptoms.to_vec();
    for g in [
        "US",
        "US_ENTRY",
        "US_MID",
        "US_EXIT",
        "US_SLOW",
        "US_FAST",
        "OS",
        "OS_ENTRY",
        "OS_MID",
        "OS_EXIT",
        "OS_SLOW",
        "OS_FAST",
        "BRK_LOCK",
        "TRC_WHEELSPIN",
        "TYR_HOT",
        "NO_GRIP",
        "JUMPS",
        "UNSTABLE",
    ] {
        push_unique(&mut v, g);
    }
    v.sort();
    v.dedup();
    v
}

impl AnthropicExtractor {
    /// `None` when `CORDE_ANTHROPIC_API_KEY` is absent or empty.
    pub fn from_env(rule_symptoms: &[String]) -> Result<Option<Self>> {
        match corde_store::migrate::env_var(ANTHROPIC_KEY_ENV) {
            Ok(k) if !k.trim().is_empty() => {
                Ok(Some(Self::new(k.trim().to_string(), rule_symptoms)?))
            }
            _ => Ok(None),
        }
    }

    pub fn new(api_key: String, rule_symptoms: &[String]) -> Result<Self> {
        Ok(Self {
            api_key,
            model: ANTHROPIC_DEFAULT_MODEL.into(),
            endpoint: ANTHROPIC_ENDPOINT.into(),
            timeout: ANTHROPIC_TIMEOUT,
            fallback: DictionaryExtractor::embedded()?,
            allowed: allowed_symptom_ids(rule_symptoms),
        })
    }

    /// Another endpoint / model / timeout (tests, proxies).
    pub fn with_endpoint(mut self, endpoint: &str, model: Option<&str>, timeout: Duration) -> Self {
        self.endpoint = endpoint.to_string();
        if let Some(m) = model {
            self.model = m.to_string();
        }
        self.timeout = timeout;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// JSON schema of the answer (structured outputs: strict objects, no
    /// numeric bounds, `anyOf` for nullable enums).
    pub fn output_schema() -> serde_json::Value {
        let nullable_enum = |values: &[&str]| serde_json::json!({ "anyOf": [ { "type": "string", "enum": values }, { "type": "null" } ] });
        serde_json::json!({
            "type": "object",
            "properties": {
                "symptoms": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "symptom_id": { "type": "string" },
                            "phase": nullable_enum(&["entry", "mid", "exit"]),
                            "corner_speed": nullable_enum(&["slow", "medium", "fast"]),
                            "axle": nullable_enum(&["front", "rear"]),
                            "wet": { "type": "boolean" },
                            "intensity": nullable_enum(&["mild", "marked", "severe"]),
                            "negated": { "type": "boolean" },
                            "matched_text": { "type": "string" },
                            "confidence": { "type": "number" }
                        },
                        "required": ["symptom_id", "phase", "corner_speed", "axle", "wet", "intensity", "negated", "matched_text", "confidence"],
                        "additionalProperties": false
                    }
                },
                "questions_fr": { "type": "array", "items": { "type": "string" } },
                "questions_en": { "type": "array", "items": { "type": "string" } },
                "unmatched_fragments": { "type": "array", "items": { "type": "string" } },
                "confidence": { "type": "number" }
            },
            "required": ["symptoms", "questions_fr", "questions_en", "unmatched_fragments", "confidence"],
            "additionalProperties": false
        })
    }

    fn system_prompt(&self) -> String {
        format!(
            "You are the race engineer of a sim-racing telemetry app (Le Mans Ultimate). The driver \
             writes what they felt in the car, in French or English, informally and with typos. \
             Translate it into setup symptoms. Use ONLY these symptom ids: {}. Balance ids follow \
             FAM[_PHASE][_SPEED] with FAM = US (understeer) | OS (oversteer), PHASE = ENTRY | MID | \
             EXIT, SPEED = SLOW | FAST; omit a part the driver did not state (e.g. `US_ENTRY` when \
             the corner speed is unknown). Braking-zone remarks are phase `entry`. `negated` is true \
             when the driver says the symptom is absent or gone (\"pas de sous-virage\", \"ne glisse \
             plus\"). `intensity`: mild (\"un peu\"), marked (\"beaucoup\"), severe (\"trop\", \
             \"énormément\", \"d'un coup\"). `matched_text` quotes the driver's own words. \
             `confidence` is 0..1. Ask at most 3 short clarification questions (French in \
             `questions_fr`, the same in English in `questions_en`) only for what is missing: corner \
             phase, corner speed, axle. Put the parts of the text you could not map into \
             `unmatched_fragments`. Never propose setup values.",
            self.allowed.join(", ")
        )
    }

    /// The request body sent to the Messages API.
    pub fn request_body(&self, text: &str, lang_hint: Option<&str>) -> serde_json::Value {
        let user = match lang_hint {
            Some(l) => format!("Language hint: {l}.\nDriver feedback:\n{text}"),
            None => format!("Driver feedback:\n{text}"),
        };
        serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": self.system_prompt(),
            "messages": [ { "role": "user", "content": user } ],
            "output_config": { "format": { "type": "json_schema", "schema": Self::output_schema() } }
        })
    }

    /// Reads a Messages API response into an interpretation (`extractor = Llm`).
    pub fn parse_response(
        &self,
        body: &serde_json::Value,
    ) -> std::result::Result<FeedbackInterpretation, String> {
        if let Some(err) = body.get("error") {
            return Err(format!(
                "API error: {}",
                err.get("message").and_then(|m| m.as_str()).unwrap_or("?")
            ));
        }
        match body.get("stop_reason").and_then(|s| s.as_str()) {
            Some("refusal") => return Err("the model refused".into()),
            Some("max_tokens") => return Err("answer truncated (max_tokens)".into()),
            _ => {}
        }
        let text = body
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .and_then(|b| b.get("text").and_then(|t| t.as_str()))
            })
            .ok_or_else(|| "no text block in the answer".to_string())?;
        let out: LlmOutput =
            serde_json::from_str(text).map_err(|e| format!("invalid JSON answer: {e}"))?;
        let mut symptoms = Vec::new();
        let mut notes = Vec::new();
        for s in out.symptoms {
            let id = s.symptom_id.trim().to_uppercase();
            if !self.allowed.contains(&id) {
                notes.push(format!("Symptôme inconnu ignoré : {id}."));
                continue;
            }
            let (label_fr, label_en) = symptom_label(&id);
            let phase = match s.phase.as_deref() {
                Some("entry") => Some(CornerPhase::Entry),
                Some("mid") => Some(CornerPhase::Mid),
                Some("exit") => Some(CornerPhase::Exit),
                _ => None,
            }
            .or_else(|| parse_balance_id(&id).and_then(|(_, p, _)| p));
            let corner_speed = match s.corner_speed.as_deref() {
                Some("slow") => Some(CornerSpeed::Slow),
                Some("medium") => Some(CornerSpeed::Medium),
                Some("fast") => Some(CornerSpeed::Fast),
                _ => None,
            }
            .or_else(|| parse_balance_id(&id).and_then(|(_, _, s)| s));
            let axle = match s.axle.as_deref() {
                Some("front") => Some(Axle::Front),
                Some("rear") => Some(Axle::Rear),
                _ => None,
            };
            let intensity = match s.intensity.as_deref() {
                Some("mild") => Some(SymptomIntensity::Mild),
                Some("marked") => Some(SymptomIntensity::Marked),
                Some("severe") => Some(SymptomIntensity::Severe),
                _ => None,
            };
            symptoms.push(DetectedSymptom {
                symptom_id: id,
                label_fr,
                label_en,
                phase,
                corner_speed,
                axle,
                wet: s.wet,
                intensity,
                negated: s.negated,
                matched_text: s.matched_text,
                confidence: s.confidence.clamp(0.0, 1.0),
            });
        }
        let n = out.questions_fr.len().min(out.questions_en.len()).min(3);
        Ok(FeedbackInterpretation {
            feedback_id: 0,
            symptoms,
            questions_fr: out.questions_fr.into_iter().take(n).collect(),
            questions_en: out.questions_en.into_iter().take(n).collect(),
            unmatched_fragments: out.unmatched_fragments,
            extractor: ExtractorKind::Llm,
            confidence: out.confidence.clamp(0.0, 1.0),
            notes_fr: notes,
        })
    }

    fn call(
        &self,
        text: &str,
        lang_hint: Option<&str>,
    ) -> std::result::Result<FeedbackInterpretation, String> {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .timeout_global(Some(self.timeout))
            .http_status_as_error(false)
            .build()
            .new_agent();
        let body = self.request_body(text, lang_hint);
        let mut resp = agent
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status().as_u16();
        let json: serde_json::Value = resp
            .body_mut()
            .read_json()
            .map_err(|e| format!("unreadable answer (HTTP {status}): {e}"))?;
        if status != 200 {
            let msg = json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("");
            return Err(format!("HTTP {status} {msg}").trim().to_string());
        }
        self.parse_response(&json)
    }
}

impl SymptomExtractor for AnthropicExtractor {
    fn kind(&self) -> ExtractorKind {
        ExtractorKind::Llm
    }

    fn extract(&self, text: &str, lang_hint: Option<&str>) -> FeedbackInterpretation {
        match self.call(text, lang_hint) {
            Ok(i) if !i.symptoms.is_empty() => i,
            Ok(_) => {
                let mut d = self.fallback.extract(text, lang_hint);
                d.notes_fr.push(
                    "Le modèle de langage n'a reconnu aucun symptôme : interprétation par le dictionnaire.".into(),
                );
                d
            }
            Err(e) => {
                tracing::warn!("extracteur LLM indisponible ({e}) : repli sur le dictionnaire");
                let mut d = self.fallback.extract(text, lang_hint);
                d.notes_fr.push(format!(
                    "Modèle de langage indisponible ({e}) : interprétation par le dictionnaire."
                ));
                d
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex() -> &'static DictionaryExtractor {
        default_extractor()
    }

    fn ids(i: &FeedbackInterpretation) -> Vec<String> {
        i.symptoms.iter().map(|s| s.symptom_id.clone()).collect()
    }

    fn find<'a>(i: &'a FeedbackInterpretation, id: &str) -> &'a DetectedSymptom {
        i.symptoms
            .iter()
            .find(|s| s.symptom_id == id)
            .unwrap_or_else(|| panic!("no {id} in {:?}", ids(i)))
    }

    #[test]
    fn normalisation_keeps_indices() {
        let t = "Ça SOUS-VIRE, l'avant… (T3)!";
        let n = normalise(t);
        assert_eq!(n.chars().count(), t.chars().count());
        assert_eq!(n, "ca sous vire, l avant. ,t3,.");
    }

    #[test]
    fn dictionary_loads_and_rules_reference_its_entries() {
        let d = FeedbackDictionary::embedded().unwrap();
        assert!(d.entries.len() >= 56, "{}", d.entries.len());
        assert!(d.entry("FEEL_FLOATY").is_some(), "alias resolved");
        let k = crate::rules::Knowledge::embedded().unwrap();
        for r in &k.rules.rules {
            for f in &r.trigger.feel {
                assert!(d.entry(f).is_some(), "{}: unknown feel {f}", r.id);
            }
        }
        assert!(ex().pattern_count() > 250);
    }

    /// (text, expected symptom id, phase, speed, negated, intensity)
    type Case = (
        &'static str,
        &'static str,
        Option<CornerPhase>,
        Option<CornerSpeed>,
        bool,
        Option<SymptomIntensity>,
    );
    const CASES: &[Case] = &[
        // --- French, understeer ---
        (
            "l'avant ne tourne pas en entrée des virages lents",
            "US_ENTRY_SLOW",
            Some(CornerPhase::Entry),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "ça sous-vire beaucoup en milieu de virage dans les rapides",
            "US_MID_FAST",
            Some(CornerPhase::Mid),
            Some(CornerSpeed::Fast),
            false,
            Some(SymptomIntensity::Marked),
        ),
        (
            "Ca sousvire un peu",
            "US",
            None,
            None,
            false,
            Some(SymptomIntensity::Mild),
        ),
        (
            "l avant pousse tout droit à la corde",
            "US_MID",
            Some(CornerPhase::Mid),
            None,
            false,
            None,
        ),
        (
            "la voiture ne tourne pas, elle pousse en entrée",
            "US_ENTRY",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "L'avant glisse à la remise des gaz dans les épingles",
            "US_EXIT_SLOW",
            Some(CornerPhase::Exit),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "sous virage énorme a l'inscription",
            "US_ENTRY",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        // --- French, oversteer ---
        (
            "ça survire en sortie",
            "OS_EXIT",
            Some(CornerPhase::Exit),
            None,
            false,
            None,
        ),
        (
            "l'arrière décroche à la remise des gaz dans les lents",
            "OS_EXIT_SLOW",
            Some(CornerPhase::Exit),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "l'arriere part trop en sortie de virage",
            "OS_EXIT",
            Some(CornerPhase::Exit),
            None,
            false,
            Some(SymptomIntensity::Severe),
        ),
        (
            "l'arrière est léger dans les courbes rapides",
            "OS_FAST",
            None,
            Some(CornerSpeed::Fast),
            false,
            None,
        ),
        (
            "ça me met en travers quand j'accélère",
            "OS_EXIT",
            Some(CornerPhase::Exit),
            None,
            false,
            None,
        ),
        (
            "le cul glisse à l'inscription des virages lents",
            "OS_ENTRY_SLOW",
            Some(CornerPhase::Entry),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        // --- French, braking / traction ---
        (
            "instable au freinage, ça bouge en bout de ligne droite",
            "BRK_UNSTABLE_STRAIGHT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "je bloque les roues avant en fin de freinage",
            "BRK_LOCK_FRONT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "ça bloque de l'arrière au rétrogradage",
            "BRK_LOCK_REAR",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "je bloque",
            "BRK_LOCK",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "ça patine énormément en sortie des lents",
            "TRC_WHEELSPIN",
            Some(CornerPhase::Exit),
            Some(CornerSpeed::Slow),
            false,
            Some(SymptomIntensity::Severe),
        ),
        (
            "pas de motricité, la roue intérieure s'allume",
            "TRC_WHEELSPIN_INNER",
            Some(CornerPhase::Exit),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "les freins faiblissent après 10 tours",
            "BRK_OVERHEAT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        // --- French, platform / tyres / wet ---
        (
            "ça saute sur les vibreurs",
            "PLT_KERB_UNSETTLED",
            None,
            None,
            false,
            None,
        ),
        (
            "elle tape sur les bosses de Sebring",
            "PLT_BUMPY_LOSS",
            None,
            None,
            false,
            None,
        ),
        ("ça saute", "JUMPS", None, None, false, None),
        (
            "les pneus avant surchauffent après 5 tours",
            "TYR_HOT_FRONT",
            None,
            None,
            false,
            None,
        ),
        (
            "les pneus glissent après 8 tours",
            "TYR_WEAR_FAST",
            None,
            None,
            false,
            None,
        ),
        (
            "les arrières s'usent trop vite",
            "TYR_WEAR_FAST",
            None,
            None,
            false,
            Some(SymptomIntensity::Severe),
        ),
        (
            "pas de grip sous la pluie",
            "WET_NO_TEMP",
            None,
            None,
            false,
            None,
        ),
        (
            "aquaplaning dans la ligne droite",
            "WET_BASELINE",
            None,
            None,
            false,
            None,
        ),
        ("pas de grip", "NO_GRIP", None, None, false, None),
        (
            "la voiture est nerveuse",
            "PLT_NERVOUS",
            None,
            None,
            false,
            None,
        ),
        // --- French, negation ---
        (
            "pas de sous-virage cette fois",
            "US",
            None,
            None,
            true,
            None,
        ),
        (
            "l'arrière ne glisse plus en sortie",
            "OS_EXIT",
            Some(CornerPhase::Exit),
            None,
            true,
            None,
        ),
        (
            "ça ne bloque plus",
            "BRK_LOCK",
            Some(CornerPhase::Entry),
            None,
            true,
            None,
        ),
        // --- English ---
        (
            "understeer on entry in slow corners",
            "US_ENTRY_SLOW",
            Some(CornerPhase::Entry),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "the car pushes mid-corner in the fast stuff",
            "US_MID_FAST",
            Some(CornerPhase::Mid),
            Some(CornerSpeed::Fast),
            false,
            None,
        ),
        (
            "rear snaps on exit, way too loose",
            "OS_EXIT",
            Some(CornerPhase::Exit),
            None,
            false,
            Some(SymptomIntensity::Severe),
        ),
        (
            "it's a bit loose on turn-in",
            "OS_ENTRY",
            Some(CornerPhase::Entry),
            None,
            false,
            Some(SymptomIntensity::Mild),
        ),
        (
            "locking the fronts under braking",
            "BRK_LOCK_FRONT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "wheelspin out of the hairpin",
            "TRC_WHEELSPIN",
            Some(CornerPhase::Exit),
            Some(CornerSpeed::Slow),
            false,
            None,
        ),
        (
            "unstable under braking",
            "BRK_UNSTABLE_STRAIGHT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        ("no understeer anymore", "US", None, None, true, None),
        (
            "the rear tyres are cooked after ten laps",
            "TYR_HOT_REAR",
            None,
            None,
            false,
            None,
        ),
        (
            "kerbs unsettle the car badly",
            "PLT_KERB_UNSETTLED",
            None,
            None,
            false,
            Some(SymptomIntensity::Marked),
        ),
        (
            "brake fade late in the stint",
            "BRK_OVERHEAT",
            Some(CornerPhase::Entry),
            None,
            false,
            None,
        ),
        (
            "no grip in the rain",
            "WET_NO_TEMP",
            None,
            None,
            false,
            None,
        ),
    ];

    #[test]
    fn realistic_phrases_fr_en() {
        let mut failures = Vec::new();
        for (text, id, phase, speed, neg, intensity) in CASES {
            let i = ex().extract(text, None);
            let Some(s) = i.symptoms.iter().find(|s| s.symptom_id == *id) else {
                failures.push(format!("{text:?}: expected {id}, got {:?}", ids(&i)));
                continue;
            };
            if s.phase != *phase {
                failures.push(format!("{text:?}: phase {:?} ≠ {:?}", s.phase, phase));
            }
            if s.corner_speed != *speed {
                failures.push(format!(
                    "{text:?}: speed {:?} ≠ {:?}",
                    s.corner_speed, speed
                ));
            }
            if s.negated != *neg {
                failures.push(format!("{text:?}: negated {} ≠ {}", s.negated, neg));
            }
            if s.intensity != *intensity {
                failures.push(format!(
                    "{text:?}: intensity {:?} ≠ {:?}",
                    s.intensity, intensity
                ));
            }
            assert!(
                s.confidence > 0.3 && s.confidence <= 0.95,
                "{text:?}: {}",
                s.confidence
            );
            assert!(!s.matched_text.is_empty(), "{text:?}: matched text");
            assert!(!s.label_fr.is_empty() && !s.label_en.is_empty());
        }
        assert!(
            failures.is_empty(),
            "{} failure(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
        assert!(CASES.len() >= 40);
    }

    #[test]
    fn mixed_sentence_gives_two_symptoms_with_their_own_attributes() {
        let i = ex().extract(
            "le train avant pousse en entrée mais l'arrière part à la remise des gaz dans les lents",
            Some("fr"),
        );
        let us = find(&i, "US_ENTRY");
        assert_eq!(us.phase, Some(CornerPhase::Entry));
        assert_eq!(us.corner_speed, None);
        assert_eq!(us.axle, Some(Axle::Front));
        let os = find(&i, "OS_EXIT_SLOW");
        assert_eq!(os.phase, Some(CornerPhase::Exit));
        assert_eq!(os.corner_speed, Some(CornerSpeed::Slow));
        assert_eq!(os.axle, Some(Axle::Rear));
        assert!(!us.negated && !os.negated);
        // The understeer has no corner speed: one question about it, none about the phase.
        assert!(
            i.questions_fr.iter().any(|q| q.contains("lents, rapides")),
            "{:?}",
            i.questions_fr
        );
        assert!(
            !i.questions_fr.iter().any(|q| q.contains("À quel moment")),
            "{:?}",
            i.questions_fr
        );
        assert_eq!(i.questions_fr.len(), i.questions_en.len());
        assert!(
            i.unmatched_fragments.is_empty(),
            "{:?}",
            i.unmatched_fragments
        );
        assert!(i.confidence > 0.6, "{}", i.confidence);
        assert_eq!(i.extractor, ExtractorKind::Dictionary);
    }

    #[test]
    fn modifier_only_clause_attaches_to_its_neighbour() {
        let i = ex().extract("ça sous-vire, surtout dans les lents", None);
        let s = find(&i, "US_SLOW");
        assert_eq!(s.corner_speed, Some(CornerSpeed::Slow));
        assert!(
            i.unmatched_fragments.is_empty(),
            "{:?}",
            i.unmatched_fragments
        );
        let i = ex().extract("Dans les virages rapides, l'arrière décroche", None);
        let s = find(&i, "OS_FAST");
        assert_eq!(s.corner_speed, Some(CornerSpeed::Fast));
    }

    #[test]
    fn context_carries_over_inside_a_sentence_only() {
        let i = ex().extract(
            "Instable au freinage en bout de ligne droite, l'arrière bouge.",
            None,
        );
        find(&i, "BRK_UNSTABLE_STRAIGHT");
        let os = find(&i, "OS_ENTRY");
        assert_eq!(os.phase, Some(CornerPhase::Entry));
        assert!(i.symptoms.len() <= 3, "{:?}", ids(&i));
        // A new sentence starts afresh: the rear symptom has no phase.
        let i = ex().extract("Instable au freinage. L'arrière bouge.", None);
        let os = find(&i, "OS");
        assert_eq!(os.phase, None);
        // The mixed sentence keeps each clause's own attributes (speed of the rear only).
        let i = ex().extract(
            "ça sous-vire en entrée, et ça survire en sortie dans les lents",
            None,
        );
        find(&i, "US_ENTRY");
        find(&i, "OS_EXIT_SLOW");
    }

    #[test]
    fn dictionary_phrases_win_with_their_defaults_and_questions() {
        let i = ex().extract("ça pousse à l'entrée et je veux plus de stabilité", None);
        let us = find(&i, "US_ENTRY");
        assert_eq!(us.phase, Some(CornerPhase::Entry));
        assert!(us.confidence >= 0.85, "{}", us.confidence);
        find(&i, "PHILOSOPHY_SAFE");
        // "Toujours sur les freins…" comes from the YAML, with its English twin.
        let pos = i
            .questions_fr
            .iter()
            .position(|q| q.starts_with("Toujours sur les freins"))
            .expect("yaml question");
        assert!(i.questions_en[pos].starts_with("Still on the brakes"));
    }

    #[test]
    fn oversteer_under_braking_also_suggests_straight_line_instability() {
        let i = ex().extract("l'arrière se dérobe au freinage", None);
        find(&i, "BRK_UNSTABLE_STRAIGHT");
        let os = find(&i, "OS_ENTRY");
        assert_eq!(os.phase, Some(CornerPhase::Entry));
        assert!(i.questions_fr.iter().any(|q| q.contains("ligne droite")));
    }

    #[test]
    fn nothing_recognised() {
        let i = ex().extract("bonjour, belle journée au circuit", None);
        assert!(i.symptoms.is_empty());
        assert_eq!(i.confidence, 0.0);
        assert_eq!(i.questions_fr.len(), 1);
        assert_eq!(
            i.unmatched_fragments.len(),
            2,
            "{:?}",
            i.unmatched_fragments
        );
    }

    #[test]
    fn verdict_goes_to_notes_and_wet_flag_is_set() {
        let i = ex().extract(
            "c'est mieux qu'avant, mais ça survire encore un peu sous la pluie",
            None,
        );
        assert!(i.notes_fr.iter().any(|n| n.contains("mieux")));
        let os = find(&i, "OS");
        assert!(os.wet);
        assert_eq!(os.intensity, Some(SymptomIntensity::Mild));
        assert!(!os.negated);
    }

    #[test]
    fn negated_and_present_symptoms_in_one_message() {
        let i = ex().extract(
            "Plus de sous-virage en entrée, par contre l'arrière bloque beaucoup au freinage.",
            None,
        );
        let us = find(&i, "US_ENTRY");
        assert!(us.negated, "{:?}", i.symptoms);
        let lock = find(&i, "BRK_LOCK_REAR");
        assert!(!lock.negated);
        assert_eq!(lock.intensity, Some(SymptomIntensity::Marked));
        assert_eq!(lock.axle, Some(Axle::Rear));
    }

    #[test]
    fn typos_and_casing() {
        let i = ex().extract(
            "CA SURVIRE GRAVE EN SORTIE, et sa bloque de lavant au freinnage",
            None,
        );
        find(&i, "OS_EXIT");
        let lock = find(&i, "BRK_LOCK_FRONT");
        assert_eq!(lock.axle, Some(Axle::Front));
    }

    #[test]
    fn interpretation_is_serialisable() {
        let i = ex().extract("l'avant ne tourne pas en entrée des virages lents", None);
        let json = serde_json::to_string(&i).unwrap();
        let back: FeedbackInterpretation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, i);
        assert!(json.contains("\"extractor\":\"dictionary\""));
    }

    // --- Anthropic extractor (no network) ---

    fn llm() -> AnthropicExtractor {
        let k = crate::rules::Knowledge::embedded().unwrap();
        let symptoms: Vec<String> = k.rules.rules.iter().map(|r| r.symptom.clone()).collect();
        AnthropicExtractor::new("sk-test".into(), &symptoms).unwrap()
    }

    #[test]
    fn anthropic_request_is_well_formed() {
        let e = llm();
        let body = e.request_body("l'avant ne tourne pas", Some("fr"));
        assert_eq!(body["model"], ANTHROPIC_DEFAULT_MODEL);
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["messages"][0]["role"], "user");
        assert!(body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("l'avant ne tourne pas"));
        assert!(body["system"].as_str().unwrap().contains("US_ENTRY_SLOW"));
        assert!(body["system"]
            .as_str()
            .unwrap()
            .contains("Never propose setup values"));
        let fmt = &body["output_config"]["format"];
        assert_eq!(fmt["type"], "json_schema");
        assert_eq!(fmt["schema"]["additionalProperties"], false);
        assert_eq!(fmt["schema"]["required"][0], "symptoms");
        let item = &fmt["schema"]["properties"]["symptoms"]["items"];
        assert_eq!(item["additionalProperties"], false);
        assert!(item["properties"]["phase"]["anyOf"].is_array());
        // No numeric bounds (unsupported by structured outputs).
        assert!(!serde_json::to_string(&body).unwrap().contains("minimum"));
    }

    #[test]
    fn anthropic_canonical_response_is_parsed_and_validated() {
        let e = llm();
        let answer = serde_json::json!({
            "symptoms": [
                { "symptom_id": "us_entry_slow", "phase": "entry", "corner_speed": "slow", "axle": "front", "wet": false, "intensity": null, "negated": false, "matched_text": "l'avant ne tourne pas", "confidence": 0.9 },
                { "symptom_id": "OS_EXIT", "phase": null, "corner_speed": null, "axle": "rear", "wet": false, "intensity": "severe", "negated": false, "matched_text": "l'arrière part", "confidence": 1.7 },
                { "symptom_id": "MADE_UP", "phase": null, "corner_speed": null, "axle": null, "wet": false, "intensity": null, "negated": false, "matched_text": "x", "confidence": 0.5 }
            ],
            "questions_fr": ["Lents ou rapides ?"],
            "questions_en": ["Slow or fast corners?"],
            "unmatched_fragments": ["surtout au T3"],
            "confidence": 0.85
        });
        let body = serde_json::json!({
            "id": "msg_01", "type": "message", "role": "assistant", "model": ANTHROPIC_DEFAULT_MODEL,
            "content": [ { "type": "text", "text": answer.to_string() } ],
            "stop_reason": "end_turn", "stop_sequence": null,
            "usage": { "input_tokens": 10, "output_tokens": 20 }
        });
        let i = e.parse_response(&body).unwrap();
        assert_eq!(i.extractor, ExtractorKind::Llm);
        assert_eq!(i.symptoms.len(), 2, "unknown id dropped");
        assert_eq!(i.symptoms[0].symptom_id, "US_ENTRY_SLOW");
        assert_eq!(i.symptoms[0].phase, Some(CornerPhase::Entry));
        assert_eq!(
            i.symptoms[0].label_fr,
            "Sous-virage en entrée (virages lents)"
        );
        assert_eq!(i.symptoms[1].symptom_id, "OS_EXIT");
        assert_eq!(
            i.symptoms[1].phase,
            Some(CornerPhase::Exit),
            "phase from the id"
        );
        assert_eq!(i.symptoms[1].confidence, 1.0, "clamped");
        assert_eq!(i.symptoms[1].intensity, Some(SymptomIntensity::Severe));
        assert_eq!(i.questions_fr, vec!["Lents ou rapides ?"]);
        assert_eq!(i.unmatched_fragments, vec!["surtout au T3"]);
        assert!(i.notes_fr.iter().any(|n| n.contains("MADE_UP")));
        // Refusal / truncation / API error are errors.
        let refused = serde_json::json!({ "content": [], "stop_reason": "refusal" });
        assert!(e.parse_response(&refused).is_err());
        let truncated = serde_json::json!({ "content": [ { "type": "text", "text": "{" } ], "stop_reason": "max_tokens" });
        assert!(e.parse_response(&truncated).is_err());
        let api_err = serde_json::json!({ "type": "error", "error": { "type": "authentication_error", "message": "invalid x-api-key" } });
        assert!(e
            .parse_response(&api_err)
            .unwrap_err()
            .contains("invalid x-api-key"));
    }

    #[test]
    fn anthropic_falls_back_to_the_dictionary_when_unreachable() {
        // Connection refused on a closed local port: immediate, no network needed.
        let e = llm().with_endpoint(
            "http://127.0.0.1:9/v1/messages",
            None,
            Duration::from_millis(1500),
        );
        let i = e.extract("l'avant ne tourne pas en entrée des virages lents", None);
        assert_eq!(i.extractor, ExtractorKind::Dictionary);
        assert_eq!(i.symptoms[0].symptom_id, "US_ENTRY_SLOW");
        assert!(
            i.notes_fr.iter().any(|n| n.contains("indisponible")),
            "{:?}",
            i.notes_fr
        );
        assert!(
            AnthropicExtractor::from_env(&[]).unwrap().is_none()
                || corde_store::migrate::env_var(ANTHROPIC_KEY_ENV).is_ok()
        );
    }
}
