//! The variables of the game, read by name through the table the game publishes.
//!
//! The table says, for the car being driven, which variables exist, their type and where
//! each one is in a row. Nothing of it is trusted: a line that makes no sense or points
//! outside a row is left out, and a variable that is not there reads as « absent ».

use crate::layout::{VarHeader, VarType, MAX_VARS, VAR_HEADER_SIZE};
use std::collections::HashMap;

/// The table of the variables.
#[derive(Debug, Clone, Default)]
pub struct VarTable {
    vars: Vec<VarHeader>,
    by_name: HashMap<String, usize>,
    row_len: usize,
}

/// Bytes a variable takes, `None` when that does not fit a number.
fn span(v: &VarHeader) -> Option<usize> {
    v.kind.size().checked_mul(v.count)
}

impl VarTable {
    /// From the bytes of the table, for rows of `row_len` bytes. Lines that make no sense,
    /// that point outside a row, or that repeat a name, are left out.
    pub fn parse(bytes: &[u8], row_len: usize) -> Self {
        let mut table = Self {
            row_len,
            ..Self::default()
        };
        for line in bytes.chunks_exact(VAR_HEADER_SIZE).take(MAX_VARS) {
            let Some(v) = VarHeader::parse(line) else {
                continue;
            };
            let fits = span(&v)
                .and_then(|len| v.offset.checked_add(len))
                .is_some_and(|end| end <= row_len);
            if fits && !table.by_name.contains_key(&v.name) {
                table.by_name.insert(v.name.clone(), table.vars.len());
                table.vars.push(v);
            }
        }
        table
    }

    /// A table made here (the tests, the fake game): the variables one after the other.
    pub fn of(vars: &[(&str, VarType, usize)]) -> Self {
        let mut table = Self::default();
        for (name, kind, count) in vars {
            // Each variable starts on a multiple of its size, as the game lays them out.
            let size = kind.size();
            let offset = table.row_len.div_ceil(size) * size;
            let v = VarHeader {
                kind: *kind,
                offset,
                count: *count,
                name: (*name).to_string(),
                desc: String::new(),
                unit: String::new(),
            };
            table.row_len = offset + v.len();
            table.by_name.insert(v.name.clone(), table.vars.len());
            table.vars.push(v);
        }
        // Rows are whole multiples of 8 bytes.
        table.row_len = table.row_len.div_ceil(8) * 8;
        table
    }

    pub fn get(&self, name: &str) -> Option<&VarHeader> {
        self.by_name.get(name).map(|i| &self.vars[*i])
    }

    pub fn has(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn vars(&self) -> &[VarHeader] {
        &self.vars
    }

    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Bytes of a row.
    pub fn row_len(&self) -> usize {
        self.row_len
    }

    /// The table as the game writes it.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.vars.iter().flat_map(|v| v.to_bytes()).collect()
    }

    /// A row, read through this table.
    pub fn row<'a>(&'a self, bytes: &'a [u8]) -> Row<'a> {
        Row { table: self, bytes }
    }

    /// An empty row of the right size.
    pub fn new_row(&self) -> Vec<u8> {
        vec![0; self.row_len]
    }

    /// Writes one value in a row (the tests, the fake game). `false`: no such variable.
    pub fn put(&self, row: &mut [u8], name: &str, i: usize, value: f64) -> bool {
        let Some(v) = self.get(name) else {
            return false;
        };
        let size = v.kind.size();
        let Some(out) = (i < v.count)
            .then(|| v.offset + i * size)
            .and_then(|at| row.get_mut(at..at + size))
        else {
            return false;
        };
        match v.kind {
            VarType::Char => out[0] = value as u8,
            VarType::Bool => out[0] = (value != 0.0) as u8,
            VarType::Int => out.copy_from_slice(&(value as i32).to_le_bytes()),
            VarType::BitField => out.copy_from_slice(&(value as u32).to_le_bytes()),
            VarType::Float => out.copy_from_slice(&(value as f32).to_le_bytes()),
            VarType::Double => out.copy_from_slice(&value.to_le_bytes()),
        }
        true
    }
}

/// One row of values, read by name.
#[derive(Clone, Copy)]
pub struct Row<'a> {
    table: &'a VarTable,
    bytes: &'a [u8],
}

impl Row<'_> {
    pub fn has(&self, name: &str) -> bool {
        self.table.has(name)
    }

    /// The `i`-th value of `name` as a number, whatever its type. `None`: the car has no
    /// such variable, `i` is beyond it, or the value is not a number (NaN, infinite).
    pub fn num_at(&self, name: &str, i: usize) -> Option<f64> {
        let v = self.table.get(name)?;
        if i >= v.count {
            return None;
        }
        let size = v.kind.size();
        let at = v.offset + i * size;
        let b = self.bytes.get(at..at + size)?;
        let n = match v.kind {
            VarType::Char => b[0] as f64,
            VarType::Bool => (b[0] != 0) as u8 as f64,
            VarType::Int => i32::from_le_bytes(b.try_into().ok()?) as f64,
            VarType::BitField => u32::from_le_bytes(b.try_into().ok()?) as f64,
            VarType::Float => f32::from_le_bytes(b.try_into().ok()?) as f64,
            VarType::Double => f64::from_le_bytes(b.try_into().ok()?),
        };
        n.is_finite().then_some(n)
    }

    pub fn num(&self, name: &str) -> Option<f64> {
        self.num_at(name, 0)
    }

    /// A value, 0 when the car does not have it.
    pub fn f32(&self, name: &str) -> f32 {
        self.num(name).unwrap_or(0.0) as f32
    }

    pub fn f32_at(&self, name: &str, i: usize) -> f32 {
        self.num_at(name, i).unwrap_or(0.0) as f32
    }

    pub fn f64(&self, name: &str) -> f64 {
        self.num(name).unwrap_or(0.0)
    }

    pub fn i32(&self, name: &str) -> Option<i32> {
        self.num(name).map(|v| v as i32)
    }

    pub fn i32_at(&self, name: &str, i: usize) -> Option<i32> {
        self.num_at(name, i).map(|v| v as i32)
    }

    /// A yes / no, « no » when the car does not have it.
    pub fn flag(&self, name: &str) -> bool {
        self.num(name).is_some_and(|v| v != 0.0)
    }

    pub fn flag_at(&self, name: &str, i: usize) -> bool {
        self.num_at(name, i).is_some_and(|v| v != 0.0)
    }

    /// A set of bits, empty when the car does not have it.
    pub fn bits(&self, name: &str) -> u32 {
        self.num(name).unwrap_or(0.0) as u32
    }

    pub fn bits_at(&self, name: &str, i: usize) -> u32 {
        self.num_at(name, i).unwrap_or(0.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_read_by_name_whatever_their_type() {
        let table = VarTable::of(&[
            ("IsOnTrack", VarType::Bool, 1),
            ("Gear", VarType::Int, 1),
            ("SessionFlags", VarType::BitField, 1),
            ("Speed", VarType::Float, 1),
            ("SessionTime", VarType::Double, 1),
            ("CarIdxLap", VarType::Int, 64),
        ]);
        // Laid out like the game does: each on a multiple of its size, rows of whole 8s.
        assert_eq!(table.get("Gear").unwrap().offset, 4);
        assert_eq!(table.get("SessionTime").unwrap().offset, 16);
        assert_eq!(table.row_len() % 8, 0);
        let mut bytes = table.new_row();
        assert!(table.put(&mut bytes, "IsOnTrack", 0, 1.0));
        assert!(table.put(&mut bytes, "Gear", 0, -1.0));
        assert!(table.put(&mut bytes, "SessionFlags", 0, 0x8000_0000u32 as f64));
        assert!(table.put(&mut bytes, "Speed", 0, 61.5));
        assert!(table.put(&mut bytes, "SessionTime", 0, 1234.5678));
        assert!(table.put(&mut bytes, "CarIdxLap", 63, 7.0));
        assert!(
            !table.put(&mut bytes, "CarIdxLap", 64, 7.0),
            "beyond the array"
        );
        assert!(!table.put(&mut bytes, "Nope", 0, 1.0));
        let row = table.row(&bytes);
        assert!(row.flag("IsOnTrack"));
        assert_eq!(row.i32("Gear"), Some(-1));
        assert_eq!(row.bits("SessionFlags"), 0x8000_0000);
        assert_eq!(row.f32("Speed"), 61.5);
        assert_eq!(row.f64("SessionTime"), 1234.5678);
        assert_eq!(row.i32_at("CarIdxLap", 63), Some(7));
        assert_eq!(row.i32_at("CarIdxLap", 64), None);
        // What the car does not have.
        assert_eq!(row.num("dcBrakeBias"), None);
        assert_eq!(row.f32("dcBrakeBias"), 0.0);
        assert!(!row.flag("dcBrakeBias"));
        // Not a number: absent rather than propagated.
        table.put(&mut bytes, "Speed", 0, f64::NAN);
        assert_eq!(table.row(&bytes).num("Speed"), None);
    }

    #[test]
    fn a_table_of_the_game_is_never_trusted() {
        let good = VarTable::of(&[("Speed", VarType::Float, 1), ("Gear", VarType::Int, 1)]);
        let mut bytes = good.to_bytes();
        // A third line that points beyond the row, a fourth that repeats a name, a fifth
        // with a size that overflows, and half a line.
        let mut beyond = good.get("Speed").unwrap().clone();
        beyond.name = "Beyond".into();
        beyond.offset = 6;
        bytes.extend(beyond.to_bytes());
        bytes.extend(good.get("Gear").unwrap().to_bytes());
        let mut huge = beyond.clone();
        huge.name = "Huge".into();
        huge.kind = VarType::Double;
        huge.count = i32::MAX as usize;
        bytes.extend(huge.to_bytes());
        bytes.extend([1u8; 70]);
        let table = VarTable::parse(&bytes, 8);
        assert_eq!(table.len(), 2);
        assert!(table.has("Speed") && table.has("Gear") && !table.has("Beyond"));
        // A row shorter than announced: absent, not a panic.
        let short = [0u8; 5];
        assert_eq!(table.row(&short).num("Gear"), None);
        assert_eq!(table.row(&short).num("Speed"), Some(0.0));
        // Round trip.
        assert_eq!(
            VarTable::parse(&good.to_bytes(), good.row_len()).vars(),
            good.vars()
        );
    }
}
