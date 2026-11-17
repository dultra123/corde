//! The memory iRacing publishes, as its SDK describes it (interface version 2).
//!
//! One mapping, `Local\IRSDKMemMapFileName`:
//!
//! ```text
//! offset 0      header (112 bytes): version, status, tick rate, where the rest is
//! …             session text (YAML), rewritten when `session_info_update` moves
//! …             table of the variables (144 bytes each): type, offset, count, name, unit
//! …             up to 4 rows of values, written in turn at the tick rate (60 Hz);
//!               the newest is the one with the highest tick count
//! ```
//!
//! Unlike the other games, nothing here is a fixed structure: the game says which
//! variables exist for the car being driven and where each one is. Everything is read
//! through that table, by name, and a variable the car does not have is simply absent.
//!
//! All integers are little-endian `i32`. Offsets are counted from the start of the
//! mapping and are never trusted: every read is checked against the mapped size.

/// Name of the mapping.
pub const MAP_NAME: &str = "Local\\IRSDKMemMapFileName";
/// The interface version this module knows.
pub const SDK_VERSION: i32 = 2;
pub const HEADER_SIZE: usize = 112;
pub const VAR_HEADER_SIZE: usize = 144;
pub const MAX_BUFS: usize = 4;
/// Bit of [`Header::status`]: the simulator is in a session.
pub const STATUS_CONNECTED: i32 = 1;
/// Cars the per-car arrays (`CarIdx…`) describe.
pub const MAX_CARS: usize = 64;

// Sanity limits: the real values are far below (≈ 350 variables, rows of ≈ 8 KiB, a
// session text of ≈ 100 KiB).
pub const MAX_VARS: usize = 4096;
pub const MAX_ROW: usize = 1 << 20;
pub const MAX_SESSION_TEXT: usize = 4 << 20;

const NAME_LEN: usize = 32;
const DESC_LEN: usize = 64;

fn int(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn put_int(out: &mut [u8], at: usize, v: i32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// One of the rows the game writes in turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VarBuf {
    pub tick_count: i32,
    pub buf_offset: i32,
}

/// The header at offset 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Header {
    pub ver: i32,
    pub status: i32,
    pub tick_rate: i32,
    pub session_info_update: i32,
    pub session_info_len: i32,
    pub session_info_offset: i32,
    pub num_vars: i32,
    pub var_header_offset: i32,
    pub num_buf: i32,
    pub buf_len: i32,
    pub var_buf: [VarBuf; MAX_BUFS],
}

impl Header {
    pub fn parse(b: &[u8; HEADER_SIZE]) -> Self {
        let mut var_buf = [VarBuf::default(); MAX_BUFS];
        for (i, vb) in var_buf.iter_mut().enumerate() {
            // 12 integers (two of padding), then 16 bytes per row.
            let at = 48 + i * 16;
            *vb = VarBuf {
                tick_count: int(b, at),
                buf_offset: int(b, at + 4),
            };
        }
        Self {
            ver: int(b, 0),
            status: int(b, 4),
            tick_rate: int(b, 8),
            session_info_update: int(b, 12),
            session_info_len: int(b, 16),
            session_info_offset: int(b, 20),
            num_vars: int(b, 24),
            var_header_offset: int(b, 28),
            num_buf: int(b, 32),
            buf_len: int(b, 36),
            var_buf,
        }
    }

    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut b = [0u8; HEADER_SIZE];
        for (i, v) in [
            self.ver,
            self.status,
            self.tick_rate,
            self.session_info_update,
            self.session_info_len,
            self.session_info_offset,
            self.num_vars,
            self.var_header_offset,
            self.num_buf,
            self.buf_len,
        ]
        .into_iter()
        .enumerate()
        {
            put_int(&mut b, i * 4, v);
        }
        for (i, vb) in self.var_buf.iter().enumerate() {
            put_int(&mut b, 48 + i * 16, vb.tick_count);
            put_int(&mut b, 48 + i * 16 + 4, vb.buf_offset);
        }
        b
    }

    /// The simulator is in a session (it leaves the mapping behind when it closes).
    pub fn connected(&self) -> bool {
        self.status & STATUS_CONNECTED != 0
    }

    /// The newest row: its index and description.
    pub fn latest(&self) -> Option<(usize, VarBuf)> {
        let n = self.num_buf.clamp(0, MAX_BUFS as i32) as usize;
        self.var_buf[..n]
            .iter()
            .copied()
            .enumerate()
            .max_by_key(|(_, vb)| vb.tick_count)
    }
}

/// Type of a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Char,
    Bool,
    Int,
    BitField,
    Float,
    Double,
}

impl VarType {
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::Char,
            1 => Self::Bool,
            2 => Self::Int,
            3 => Self::BitField,
            4 => Self::Float,
            5 => Self::Double,
            _ => return None,
        })
    }

    pub fn raw(self) -> i32 {
        match self {
            Self::Char => 0,
            Self::Bool => 1,
            Self::Int => 2,
            Self::BitField => 3,
            Self::Float => 4,
            Self::Double => 5,
        }
    }

    /// Bytes of one value.
    pub fn size(self) -> usize {
        match self {
            Self::Char | Self::Bool => 1,
            Self::Int | Self::BitField | Self::Float => 4,
            Self::Double => 8,
        }
    }
}

/// A text of the game (ISO-8859-1, NUL-terminated).
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| b as char).collect()
}

fn put_text(out: &mut [u8], s: &str) {
    for (o, c) in out
        .iter_mut()
        .zip(s.chars().take_while(|c| (*c as u32) < 256))
    {
        *o = c as u8;
    }
}

/// One line of the table of the variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarHeader {
    pub kind: VarType,
    /// Offset in a row.
    pub offset: usize,
    /// Number of values (per-car arrays: 64).
    pub count: usize,
    pub name: String,
    pub desc: String,
    pub unit: String,
}

impl VarHeader {
    /// `None` for a line that makes no sense (unknown type, negative offset or count).
    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() < VAR_HEADER_SIZE {
            return None;
        }
        let kind = VarType::from_raw(int(b, 0))?;
        let (offset, count) = (int(b, 4), int(b, 8));
        if offset < 0 || count <= 0 {
            return None;
        }
        let name = text(&b[16..16 + NAME_LEN]);
        if name.is_empty() {
            return None;
        }
        Some(Self {
            kind,
            offset: offset as usize,
            count: count as usize,
            name,
            desc: text(&b[48..48 + DESC_LEN]),
            unit: text(&b[112..112 + NAME_LEN]),
        })
    }

    pub fn to_bytes(&self) -> [u8; VAR_HEADER_SIZE] {
        let mut b = [0u8; VAR_HEADER_SIZE];
        put_int(&mut b, 0, self.kind.raw());
        put_int(&mut b, 4, self.offset as i32);
        put_int(&mut b, 8, self.count as i32);
        put_text(&mut b[16..16 + NAME_LEN - 1], &self.name);
        put_text(&mut b[48..48 + DESC_LEN - 1], &self.desc);
        put_text(&mut b[112..112 + NAME_LEN - 1], &self.unit);
        b
    }

    /// Bytes this variable takes in a row.
    pub fn len(&self) -> usize {
        self.kind.size() * self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// `SessionState`.
pub mod session_state {
    pub const INVALID: i32 = 0;
    pub const GET_IN_CAR: i32 = 1;
    pub const WARMUP: i32 = 2;
    pub const PARADE_LAPS: i32 = 3;
    pub const RACING: i32 = 4;
    pub const CHECKERED: i32 = 5;
    pub const COOL_DOWN: i32 = 6;
}

/// Bits of `SessionFlags`.
pub mod flags {
    pub const CHECKERED: u32 = 0x0000_0001;
    pub const WHITE: u32 = 0x0000_0002;
    pub const GREEN: u32 = 0x0000_0004;
    pub const YELLOW: u32 = 0x0000_0008;
    pub const RED: u32 = 0x0000_0010;
    pub const BLUE: u32 = 0x0000_0020;
    pub const YELLOW_WAVING: u32 = 0x0000_0100;
    pub const CAUTION: u32 = 0x0000_4000;
    pub const CAUTION_WAVING: u32 = 0x0000_8000;
    pub const BLACK: u32 = 0x0001_0000;
    pub const DISQUALIFY: u32 = 0x0002_0000;
}

/// Where a car is (`PlayerTrackSurface`, `CarIdxTrackSurface`).
pub mod track_loc {
    pub const NOT_IN_WORLD: i32 = -1;
    pub const OFF_TRACK: i32 = 0;
    pub const IN_PIT_STALL: i32 = 1;
    pub const APPROACHING_PITS: i32 = 2;
    pub const ON_TRACK: i32 = 3;
}

/// Bits of `EngineWarnings`.
pub mod engine_warnings {
    pub const WATER_TEMP: u32 = 0x01;
    pub const ENGINE_STALLED: u32 = 0x08;
    pub const PIT_SPEED_LIMITER: u32 = 0x10;
    pub const OIL_TEMP: u32 = 0x40;
}

/// `TrackWetness`: 1 dry … 7 extremely wet (0: not known).
pub mod wetness {
    pub const DRY: i32 = 1;
    pub const EXTREMELY_WET: i32 = 7;
}

/// What the game writes where a session has no lap limit, and no time limit.
pub const UNLIMITED_LAPS: i32 = 32_767;
pub const UNLIMITED_TIME_S: f64 = 604_800.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_is_112_bytes_and_survives_a_round_trip() {
        let h = Header {
            ver: SDK_VERSION,
            status: STATUS_CONNECTED,
            tick_rate: 60,
            session_info_update: 7,
            session_info_len: 1234,
            session_info_offset: 112,
            num_vars: 3,
            var_header_offset: 2000,
            num_buf: 3,
            buf_len: 40,
            var_buf: [
                VarBuf {
                    tick_count: 10,
                    buf_offset: 3000,
                },
                VarBuf {
                    tick_count: 12,
                    buf_offset: 3040,
                },
                VarBuf {
                    tick_count: 11,
                    buf_offset: 3080,
                },
                // Not used (num_buf is 3), whatever it holds.
                VarBuf {
                    tick_count: 99,
                    buf_offset: 0,
                },
            ],
        };
        let b = h.to_bytes();
        assert_eq!(b.len(), 112);
        // Where the SDK puts the fields.
        assert_eq!(int(&b, 4), STATUS_CONNECTED);
        assert_eq!(int(&b, 36), 40, "buf_len");
        assert_eq!(int(&b, 48), 10, "tick count of the first row");
        assert_eq!(int(&b, 52), 3000, "offset of the first row");
        assert_eq!(int(&b, 64), 12, "tick count of the second row");
        assert_eq!(Header::parse(&b), h);
        assert!(h.connected());
        // The newest of the rows in use.
        assert_eq!(h.latest().unwrap().0, 1);
        assert_eq!(Header::default().latest(), None);
    }

    #[test]
    fn a_variable_line_is_144_bytes() {
        let v = VarHeader {
            kind: VarType::Float,
            offset: 16,
            count: 64,
            name: "CarIdxLapDistPct".into(),
            desc: "Percentage distance around lap by car index".into(),
            unit: "%".into(),
        };
        let b = v.to_bytes();
        assert_eq!(b.len(), 144);
        assert_eq!(int(&b, 0), 4, "float");
        assert_eq!(&b[16..19], b"Car", "the name starts at 16");
        assert_eq!(b[112], b'%', "the unit starts at 112");
        assert_eq!(VarHeader::parse(&b), Some(v.clone()));
        assert_eq!(v.len(), 256);
        // Nonsense is refused rather than read.
        let mut bad = b;
        put_int(&mut bad, 0, 9);
        assert_eq!(VarHeader::parse(&bad), None);
        let mut bad = b;
        put_int(&mut bad, 4, -1);
        assert_eq!(VarHeader::parse(&bad), None);
        assert_eq!(VarHeader::parse(&b[..100]), None);
    }
}
