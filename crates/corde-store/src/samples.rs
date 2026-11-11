//! Parquet sample files: `<data-dir>/sessions/<session_id>/samples-NNN.parquet`.
//!
//! [`SampleWriter`] buffers rows in typed Arrow builders (no intermediate
//! `Vec<Row>`), writes one row group per [`WriterConfig::row_group_rows`]
//! rows, rotates files every [`WriterConfig::max_rows_per_file`] rows or
//! [`WriterConfig::max_file_duration`], and **checkpoints** on demand: after a
//! checkpoint the current file carries a valid footer, so it stays readable
//! if the process dies before the file is closed. The provisional footer is
//! truncated away before the next row group is appended (standard Parquet
//! readers locate the footer from the end of the file). The real footer is
//! written when the file is closed.
//!
//! Row indices handed out by [`SampleWriter::append`] are **global to the
//! session** (continue across files); [`FileInfo`] maps them to files.

use crate::channels::{self, Channel, ChannelKind, Ctx, Value, CHANNELS};
use crate::error::{Result, StoreError};
use arrow_array::builder::{
    BooleanBuilder, Float32Builder, Float64Builder, Int32Builder, Int64Builder, Int8Builder,
    UInt8Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::arrow_reader::{
    ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder, RowSelection, RowSelector,
};
use parquet::arrow::{encode_arrow_schema, ArrowWriter, ProjectionMask};
use parquet::basic::{Compression, Encoding, ZstdLevel};
use parquet::file::metadata::{FileMetaData, KeyValue, ParquetMetaData, ParquetMetaDataWriter};
use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};
use parquet::schema::types::ColumnPath;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Key of the Parquet key-value metadata holding [`channels::SCHEMA_VERSION`].
pub const KV_CHANNELS_VERSION: &str = "corde:channels";

#[derive(Debug, Clone, PartialEq)]
pub struct WriterConfig {
    /// Rows per row group (and per Arrow batch handed to the Parquet writer).
    pub row_group_rows: usize,
    /// Start a new file past this many rows…
    pub max_rows_per_file: u64,
    /// …or past this duration (wall clock) since the file was opened.
    pub max_file_duration: Duration,
    /// zstd level (1 = fast, 3 = default, 9+ = slow).
    pub zstd_level: i32,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            row_group_rows: 4096,
            max_rows_per_file: 50_000,
            max_file_duration: Duration::from_secs(600),
            zstd_level: 3,
        }
    }
}

/// Where a file's rows sit in the session's global row space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub index: u32,
    pub name: String,
    pub row_start: u64,
    /// Exclusive. Rows handed to the Parquet writer (flushed or not).
    pub row_end: u64,
    pub bytes: u64,
    pub closed: bool,
}

/// Name of the `i`-th file of a session.
pub fn file_name(index: u32) -> String {
    format!("samples-{index:03}.parquet")
}

/// Arrow schema derived from [`CHANNELS`].
pub fn arrow_schema() -> SchemaRef {
    let fields: Vec<Field> = CHANNELS
        .iter()
        .map(|c| Field::new(c.name, data_type(c.kind), c.nullable()))
        .collect();
    Arc::new(Schema::new(fields))
}

pub fn data_type(kind: ChannelKind) -> DataType {
    match kind {
        ChannelKind::F64 => DataType::Float64,
        ChannelKind::F32 | ChannelKind::OptF32 => DataType::Float32,
        ChannelKind::I64 => DataType::Int64,
        ChannelKind::I32 => DataType::Int32,
        ChannelKind::I8 => DataType::Int8,
        ChannelKind::U8 => DataType::UInt8,
        ChannelKind::Bool => DataType::Boolean,
    }
}

/// Encodings measured on simulator data (16 k rows, 163 columns, zstd 3):
/// plain + dictionary ≈ 634 B/row, plain without dictionary ≈ 446 B/row,
/// BYTE_STREAM_SPLIT floats + DELTA_BINARY_PACKED ints without dictionary
/// ≈ 335 B/row (≈ 60 MB per hour at 50 Hz). Dictionaries are useless on
/// continuously varying floats and cost a dictionary page per column chunk.
fn writer_properties(cfg: &WriterConfig) -> WriterProperties {
    let mut b = WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_2_0)
        .set_compression(Compression::ZSTD(
            ZstdLevel::try_new(cfg.zstd_level).unwrap_or_default(),
        ))
        .set_max_row_group_row_count(Some(cfg.row_group_rows.max(1)))
        .set_write_batch_size(cfg.row_group_rows.clamp(1, 4096))
        // Chunk-level min/max are enough to skip row groups; page indexes
        // would add ~40 % to the footer of a 163-column file.
        .set_statistics_enabled(EnabledStatistics::Chunk)
        .set_dictionary_enabled(false)
        .set_created_by(format!("corde-store {}", env!("CARGO_PKG_VERSION")))
        .set_key_value_metadata(Some(vec![KeyValue::new(
            KV_CHANNELS_VERSION.to_string(),
            channels::SCHEMA_VERSION.to_string(),
        )]));
    for ch in CHANNELS {
        let enc = match ch.kind {
            ChannelKind::F64 | ChannelKind::F32 | ChannelKind::OptF32 => {
                Encoding::BYTE_STREAM_SPLIT
            }
            ChannelKind::Bool => Encoding::RLE,
            ChannelKind::I64 | ChannelKind::I32 | ChannelKind::I8 | ChannelKind::U8 => {
                Encoding::DELTA_BINARY_PACKED
            }
        };
        b = b.set_column_encoding(ColumnPath::from(ch.name), enc);
    }
    b.build()
}

// ---------------------------------------------------------------------------
// Column builders
// ---------------------------------------------------------------------------

enum ColBuilder {
    F64(Float64Builder),
    F32(Float32Builder),
    I64(Int64Builder),
    I32(Int32Builder),
    I8(Int8Builder),
    U8(UInt8Builder),
    Bool(BooleanBuilder),
}

impl ColBuilder {
    fn new(kind: ChannelKind, cap: usize) -> Self {
        match kind {
            ChannelKind::F64 => Self::F64(Float64Builder::with_capacity(cap)),
            ChannelKind::F32 | ChannelKind::OptF32 => Self::F32(Float32Builder::with_capacity(cap)),
            ChannelKind::I64 => Self::I64(Int64Builder::with_capacity(cap)),
            ChannelKind::I32 => Self::I32(Int32Builder::with_capacity(cap)),
            ChannelKind::I8 => Self::I8(Int8Builder::with_capacity(cap)),
            ChannelKind::U8 => Self::U8(UInt8Builder::with_capacity(cap)),
            ChannelKind::Bool => Self::Bool(BooleanBuilder::with_capacity(cap)),
        }
    }

    fn push(&mut self, v: Value) {
        match (self, v) {
            (Self::F64(b), Value::F64(v)) => b.append_value(v),
            (Self::F32(b), Value::F32(v)) => b.append_value(v),
            (Self::F32(b), Value::OptF32(v)) => b.append_option(v),
            (Self::I64(b), Value::I64(v)) => b.append_value(v),
            (Self::I32(b), Value::I32(v)) => b.append_value(v),
            (Self::I8(b), Value::I8(v)) => b.append_value(v),
            (Self::U8(b), Value::U8(v)) => b.append_value(v),
            (Self::Bool(b), Value::Bool(v)) => b.append_value(v),
            // Kinds are fixed by `CHANNELS`; a mismatch is a programming error.
            (b, v) => unreachable!("channel kind mismatch: {:?} into {}", v, b.kind_name()),
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::F64(_) => "f64",
            Self::F32(_) => "f32",
            Self::I64(_) => "i64",
            Self::I32(_) => "i32",
            Self::I8(_) => "i8",
            Self::U8(_) => "u8",
            Self::Bool(_) => "bool",
        }
    }

    fn finish(&mut self) -> ArrayRef {
        match self {
            Self::F64(b) => Arc::new(b.finish()),
            Self::F32(b) => Arc::new(b.finish()),
            Self::I64(b) => Arc::new(b.finish()),
            Self::I32(b) => Arc::new(b.finish()),
            Self::I8(b) => Arc::new(b.finish()),
            Self::U8(b) => Arc::new(b.finish()),
            Self::Bool(b) => Arc::new(b.finish()),
        }
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

struct OpenFile {
    info: FileInfo,
    path: PathBuf,
    writer: ArrowWriter<File>,
    opened: Instant,
    /// A provisional footer sits at the end of the file, starting at `data_end`.
    checkpointed: bool,
    data_end: u64,
    checkpointed_row_groups: usize,
}

/// Buffers samples and writes them as Parquet files in one session directory.
pub struct SampleWriter {
    dir: PathBuf,
    cfg: WriterConfig,
    schema: SchemaRef,
    props: WriterProperties,
    builders: Vec<ColBuilder>,
    pending: usize,
    current: Option<OpenFile>,
    next_index: u32,
    /// Rows appended so far (pending included) = next row index.
    total_rows: u64,
    closed_files: Vec<FileInfo>,
}

impl std::fmt::Debug for SampleWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleWriter")
            .field("dir", &self.dir)
            .field("total_rows", &self.total_rows)
            .field("pending", &self.pending)
            .field("files", &self.closed_files.len())
            .finish()
    }
}

impl SampleWriter {
    /// Create the session directory and a writer with no file open yet (the
    /// first file is created on the first row group).
    pub fn new(dir: &Path, cfg: WriterConfig) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let cap = cfg.row_group_rows.max(1);
        let builders = CHANNELS
            .iter()
            .map(|c| ColBuilder::new(c.kind, cap))
            .collect();
        Ok(Self {
            dir: dir.to_path_buf(),
            props: writer_properties(&cfg),
            cfg,
            schema: arrow_schema(),
            builders,
            pending: 0,
            current: None,
            next_index: 0,
            total_rows: 0,
            closed_files: Vec::new(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Rows appended so far (also the index of the next row).
    pub fn rows(&self) -> u64 {
        self.total_rows
    }

    /// Rows buffered in memory, not yet handed to Parquet.
    pub fn pending_rows(&self) -> usize {
        self.pending
    }

    /// Name of the file that holds (or will hold) the next row.
    pub fn current_file_name(&self) -> String {
        match &self.current {
            Some(f) if !self.must_rotate(f) => f.info.name.clone(),
            _ => file_name(self.next_index),
        }
    }

    /// Bytes written to disk across every file of the session.
    pub fn bytes(&self) -> u64 {
        self.closed_files.iter().map(|f| f.bytes).sum::<u64>()
            + self
                .current
                .as_ref()
                .map(|f| f.writer.bytes_written() as u64)
                .unwrap_or(0)
    }

    /// Number of files so far (open one included).
    pub fn file_count(&self) -> usize {
        self.closed_files.len() + usize::from(self.current.is_some())
    }

    /// Every file so far (closed ones, then the open one if any).
    pub fn files(&self) -> Vec<FileInfo> {
        let mut v = self.closed_files.clone();
        if let Some(f) = &self.current {
            let mut info = f.info.clone();
            info.bytes = f.writer.bytes_written() as u64;
            v.push(info);
        }
        v
    }

    /// Append one sample. Returns its global row index.
    pub fn append(&mut self, ctx: &Ctx<'_>) -> Result<u64> {
        for (b, ch) in self.builders.iter_mut().zip(CHANNELS.iter()) {
            b.push((ch.get)(ctx));
        }
        let idx = self.total_rows;
        self.total_rows += 1;
        self.pending += 1;
        if self.pending >= self.cfg.row_group_rows.max(1) {
            self.write_pending()?;
        }
        Ok(idx)
    }

    /// Hand the buffered rows to Parquet (a row group) and make the current
    /// file readable as it stands. Call it every ~30 s; cheap when nothing new.
    pub fn checkpoint(&mut self) -> Result<Vec<FileInfo>> {
        self.write_pending()?;
        if let Some(f) = self.current.as_mut() {
            Self::write_checkpoint_footer(f, &self.schema, &self.props)?;
        }
        Ok(self.files())
    }

    /// Flush everything and close the open file. The writer can keep being
    /// used afterwards (a new file would be opened), but normally it is dropped.
    pub fn close(&mut self) -> Result<Vec<FileInfo>> {
        self.write_pending()?;
        self.close_current()?;
        Ok(self.files())
    }

    fn must_rotate(&self, f: &OpenFile) -> bool {
        let rows = f.info.row_end - f.info.row_start;
        rows >= self.cfg.max_rows_per_file || f.opened.elapsed() >= self.cfg.max_file_duration
    }

    fn write_pending(&mut self) -> Result<()> {
        if self.pending == 0 {
            return Ok(());
        }
        let arrays: Vec<ArrayRef> = self.builders.iter_mut().map(|b| b.finish()).collect();
        let batch = RecordBatch::try_new(Arc::clone(&self.schema), arrays)?;
        let n = batch.num_rows() as u64;
        self.pending = 0;
        let row_start = self.total_rows - n;

        if self.current.as_ref().is_some_and(|f| self.must_rotate(f)) {
            self.close_current()?;
        }
        if self.current.is_none() {
            self.open_file(row_start)?;
        }
        let f = self.current.as_mut().expect("file open");
        Self::drop_checkpoint_footer(f)?;
        f.writer.write(&batch)?;
        // One row group per batch: keeps memory flat and row groups aligned
        // with checkpoints.
        f.writer.flush()?;
        f.info.row_end += n;
        f.info.bytes = f.writer.bytes_written() as u64;
        Ok(())
    }

    fn open_file(&mut self, row_start: u64) -> Result<()> {
        let index = self.next_index;
        self.next_index += 1;
        let name = file_name(index);
        let path = self.dir.join(&name);
        let file = File::create(&path)?;
        let writer =
            ArrowWriter::try_new(file, Arc::clone(&self.schema), Some(self.props.clone()))?;
        self.current = Some(OpenFile {
            info: FileInfo {
                index,
                name,
                row_start,
                row_end: row_start,
                bytes: 0,
                closed: false,
            },
            path,
            writer,
            opened: Instant::now(),
            checkpointed: false,
            data_end: 0,
            checkpointed_row_groups: 0,
        });
        Ok(())
    }

    fn close_current(&mut self) -> Result<()> {
        let Some(mut f) = self.current.take() else {
            return Ok(());
        };
        Self::drop_checkpoint_footer(&mut f)?;
        let md = f.writer.finish()?;
        let file = f.writer.inner_mut();
        file.flush()?;
        let _ = file.sync_data();
        let bytes = f.writer.bytes_written() as u64;
        drop(f.writer);
        if md.file_metadata().num_rows() == 0 {
            // Nothing was ever written (rotation right after opening): no empty file.
            let _ = std::fs::remove_file(&f.path);
            return Ok(());
        }
        f.info.bytes = bytes;
        f.info.closed = true;
        self.closed_files.push(f.info);
        Ok(())
    }

    /// Append a provisional footer so that the file is a complete Parquet
    /// file as of the last flushed row group.
    fn write_checkpoint_footer(
        f: &mut OpenFile,
        schema: &Schema,
        props: &WriterProperties,
    ) -> Result<()> {
        let row_groups = f.writer.flushed_row_groups();
        if row_groups.is_empty()
            || (f.checkpointed && f.checkpointed_row_groups == row_groups.len())
        {
            return Ok(());
        }
        let num_rows: i64 = row_groups.iter().map(|rg| rg.num_rows()).sum();
        let descr = row_groups[0].schema_descr_ptr();
        let mut kv = props.key_value_metadata().cloned().unwrap_or_default();
        kv.push(KeyValue::new(
            "ARROW:schema".to_string(),
            encode_arrow_schema(schema),
        ));
        let file_md = FileMetaData::new(
            props.writer_version().as_num(),
            num_rows,
            Some(props.created_by().to_string()),
            Some(kv),
            descr,
            None,
        );
        let md = ParquetMetaData::new(file_md, row_groups.to_vec());
        let mut footer = Vec::with_capacity(64 * 1024);
        ParquetMetaDataWriter::new(&mut footer, &md).finish()?;
        let n_groups = row_groups.len();

        f.writer.sync()?; // BufWriter → file
        let data_end = f.writer.bytes_written() as u64;
        let file = f.writer.inner_mut();
        file.seek(SeekFrom::Start(data_end))?;
        file.write_all(&footer)?;
        file.flush()?;
        let _ = file.sync_data();
        f.checkpointed = true;
        f.data_end = data_end;
        f.checkpointed_row_groups = n_groups;
        f.info.bytes = data_end + footer.len() as u64;
        Ok(())
    }

    /// Remove the provisional footer before appending more data.
    fn drop_checkpoint_footer(f: &mut OpenFile) -> Result<()> {
        if !f.checkpointed {
            return Ok(());
        }
        let end = f.data_end;
        let file = f.writer.inner_mut();
        file.set_len(end)?;
        file.seek(SeekFrom::Start(end))?;
        f.checkpointed = false;
        Ok(())
    }
}

impl Drop for SampleWriter {
    fn drop(&mut self) {
        if self.current.is_some() || self.pending > 0 {
            if let Err(e) = self.close() {
                tracing::error!("sample writer: close on drop failed: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Columns of `channels` for rows `[row_start, row_end)` **of one file**
/// (file-local indices), as `Option<f64>` (null → `None`). A channel the file
/// does not hold (written before the channel was added) comes back as nulls.
pub fn read_columns(
    path: &Path,
    channels: &[&Channel],
    row_start: u64,
    row_end: u64,
) -> Result<Vec<Vec<Option<f64>>>> {
    let file = File::open(path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let total = builder.metadata().file_metadata().num_rows().max(0) as u64;
    let row_end = row_end.min(total);
    if row_start >= row_end {
        return Ok(channels.iter().map(|_| Vec::new()).collect());
    }
    let parquet_schema = builder.parquet_schema();
    let mut leaf_indices = Vec::with_capacity(channels.len());
    // Position of each requested channel inside the projected batch (`None`:
    // absent from this file, e.g. a channel added after it was written).
    let mut positions: Vec<Option<usize>> = Vec::with_capacity(channels.len());
    for ch in channels {
        let Some(idx) = parquet_schema
            .columns()
            .iter()
            .position(|c| c.name() == ch.name)
        else {
            positions.push(None);
            continue;
        };
        if let Some(p) = leaf_indices.iter().position(|&i| i == idx) {
            positions.push(Some(p));
        } else {
            positions.push(Some(leaf_indices.len()));
            leaf_indices.push(idx);
        }
    }
    let n = (row_end - row_start) as usize;
    if leaf_indices.is_empty() {
        return Ok(channels.iter().map(|_| vec![None; n]).collect());
    }
    // Projection masks keep file order: sort and remap.
    let mut order: Vec<usize> = (0..leaf_indices.len()).collect();
    order.sort_by_key(|&i| leaf_indices[i]);
    let mut rank = vec![0usize; leaf_indices.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let sorted: Vec<usize> = order.iter().map(|&i| leaf_indices[i]).collect();
    let mask = ProjectionMask::leaves(parquet_schema, sorted);
    let mut selectors = Vec::with_capacity(2);
    if row_start > 0 {
        selectors.push(RowSelector::skip(row_start as usize));
    }
    selectors.push(RowSelector::select(n));
    let selection = RowSelection::from(selectors);
    let reader: ParquetRecordBatchReader = builder
        .with_projection(mask)
        .with_row_selection(selection)
        .with_batch_size(8192)
        .build()?;
    let mut out: Vec<Vec<Option<f64>>> = (0..leaf_indices.len())
        .map(|_| Vec::with_capacity(n))
        .collect();
    for batch in reader {
        let batch = batch?;
        for (i, col) in out.iter_mut().enumerate() {
            let array = batch.column(rank[i]);
            append_as_f64(array, col)?;
        }
    }
    let rows = out.first().map_or(n, Vec::len);
    Ok(positions
        .into_iter()
        .map(|p| match p {
            Some(p) => out[p].clone(),
            None => vec![None; rows],
        })
        .collect())
}

fn append_as_f64(array: &ArrayRef, out: &mut Vec<Option<f64>>) -> Result<()> {
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Float32Type, Float64Type, Int32Type, Int64Type, Int8Type, UInt8Type};
    use arrow_array::Array;
    match array.data_type() {
        DataType::Float64 => out.extend(array.as_primitive::<Float64Type>().iter()),
        DataType::Float32 => out.extend(
            array
                .as_primitive::<Float32Type>()
                .iter()
                .map(|v| v.map(|v| v as f64)),
        ),
        DataType::Int64 => out.extend(
            array
                .as_primitive::<Int64Type>()
                .iter()
                .map(|v| v.map(|v| v as f64)),
        ),
        DataType::Int32 => out.extend(
            array
                .as_primitive::<Int32Type>()
                .iter()
                .map(|v| v.map(|v| v as f64)),
        ),
        DataType::Int8 => out.extend(
            array
                .as_primitive::<Int8Type>()
                .iter()
                .map(|v| v.map(|v| v as f64)),
        ),
        DataType::UInt8 => out.extend(
            array
                .as_primitive::<UInt8Type>()
                .iter()
                .map(|v| v.map(|v| v as f64)),
        ),
        DataType::Boolean => out.extend(
            array
                .as_boolean()
                .iter()
                .map(|v| v.map(|v| if v { 1.0 } else { 0.0 })),
        ),
        other => {
            return Err(StoreError::invalid(
                "column type",
                format!("{other:?} (len {})", array.len()),
            ))
        }
    }
    Ok(())
}

/// Number of rows and column names of a Parquet file (diagnostics / tests).
pub fn file_summary(path: &Path) -> Result<(i64, Vec<String>)> {
    let file = File::open(path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let rows = builder.metadata().file_metadata().num_rows();
    let names = builder
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    Ok((rows, names))
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::{PlayerTelemetry, TelemetryFrame};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-samples-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn frame(i: u64) -> TelemetryFrame {
        let mut f = TelemetryFrame {
            wall_ms: 1_762_200_000_000 + i * 20,
            ..Default::default()
        };
        f.session.elapsed_s = i as f64 * 0.02;
        let mut p = PlayerTelemetry::default();
        p.lap.lap_number = (i / 100) as i32;
        p.motion.speed_kmh = (i % 300) as f32;
        p.engine.gear = (i % 7) as i8 - 1;
        p.energy.virtual_energy = if i % 2 == 0 { Some(i as f32) } else { None };
        p.wheels[2].brake_temp_c = 400.0 + i as f32;
        p.electronics.tc_active = i % 3 == 0;
        f.player = Some(p);
        f
    }

    fn append_n(w: &mut SampleWriter, from: u64, n: u64) {
        for i in from..from + n {
            let f = frame(i);
            let ctx = Ctx {
                frame: &f,
                player: f.player.as_ref().unwrap(),
            };
            assert_eq!(w.append(&ctx).unwrap(), i);
        }
    }

    #[test]
    fn parquet_schema_matches_channels() {
        let dir = tmp("schema");
        let mut w = SampleWriter::new(&dir, WriterConfig::default()).unwrap();
        append_n(&mut w, 0, 10);
        let files = w.close().unwrap();
        assert_eq!(files.len(), 1);
        let path = dir.join(&files[0].name);
        let (rows, names) = file_summary(&path).unwrap();
        assert_eq!(rows, 10);
        assert_eq!(names, channels::channel_names());
        // Types survive the round trip (Int8 / UInt8 / Boolean / nullable f32).
        let file = File::open(&path).unwrap();
        let b = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        for (field, ch) in b.schema().fields().iter().zip(CHANNELS) {
            assert_eq!(field.data_type(), &data_type(ch.kind), "{}", ch.name);
            assert_eq!(field.is_nullable(), ch.nullable(), "{}", ch.name);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_checkpoint_and_read_back() {
        let dir = tmp("rot");
        let cfg = WriterConfig {
            row_group_rows: 50,
            max_rows_per_file: 120,
            max_file_duration: Duration::from_secs(3600),
            zstd_level: 1,
        };
        let mut w = SampleWriter::new(&dir, cfg).unwrap();
        append_n(&mut w, 0, 130); // → file 0: rows 0..150? no: groups of 50 → 0..100 then rotate at ≥120
        let files = w.checkpoint().unwrap();
        // 130 rows = 2 full row groups (100) + 30 pending flushed by checkpoint
        // → file 0 holds 0..130 (rotation happens only once a file has ≥ 120 rows *before* a write).
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(files[0].row_end, 130);
        // The open file is readable right now thanks to the checkpoint footer.
        let (rows, _) = file_summary(&dir.join(&files[0].name)).unwrap();
        assert_eq!(rows, 130);

        append_n(&mut w, 130, 100); // next write rotates (130 ≥ 120): file 1 gets 130..230
        let files = w.checkpoint().unwrap();
        assert_eq!(files.len(), 2, "{files:?}");
        assert!(files[0].closed && files[0].row_start == 0 && files[0].row_end == 130);
        assert!(!files[1].closed && files[1].row_start == 130 && files[1].row_end == 230);
        let (rows0, _) = file_summary(&dir.join(&files[0].name)).unwrap();
        let (rows1, _) = file_summary(&dir.join(&files[1].name)).unwrap();
        assert_eq!((rows0, rows1), (130, 100));

        // A second checkpoint without new rows is a no-op; one with new rows
        // replaces the provisional footer (no stale footer left behind).
        let size_before = std::fs::metadata(dir.join(&files[1].name)).unwrap().len();
        w.checkpoint().unwrap();
        assert_eq!(
            std::fs::metadata(dir.join(&files[1].name)).unwrap().len(),
            size_before
        );
        append_n(&mut w, 230, 10);
        let files = w.close().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[1].closed && files[1].row_end == 240);
        assert_eq!(w.rows(), 240);

        // Read back a range from file 1 with mixed types + duplicates.
        let ch = |n: &str| channels::channel(n).unwrap();
        let cols = read_columns(
            &dir.join(&files[1].name),
            &[
                ch("speed_kmh"),
                ch("t_s"),
                ch("gear"),
                ch("virtual_energy"),
                ch("tc_active"),
                ch("brake_temp_c_rl"),
                ch("speed_kmh"),
            ],
            5,
            15,
        )
        .unwrap();
        assert_eq!(cols.len(), 7);
        for c in &cols {
            assert_eq!(c.len(), 10);
        }
        // file-local row 5 of file 1 = global row 135.
        assert_eq!(cols[0][0], Some(135.0));
        assert_eq!(cols[6][0], Some(135.0));
        assert!((cols[1][0].unwrap() - 135.0 * 0.02).abs() < 1e-9);
        assert_eq!(cols[2][0], Some((135 % 7 - 1) as f64));
        assert_eq!(cols[3][0], None); // 135 is odd → None
        assert_eq!(cols[3][1], Some(136.0));
        assert_eq!(cols[4][0], Some(if 135 % 3 == 0 { 1.0 } else { 0.0 }));
        assert_eq!(cols[5][9], Some(400.0 + 144.0));
        // Out-of-range request is clamped / empty.
        let cols = read_columns(&dir.join(&files[1].name), &[ch("t_s")], 500, 600).unwrap();
        assert!(cols[0].is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn channel_absent_from_an_older_file_reads_as_nulls() {
        // A file written before `optimal_temp_c_*` existed: only two columns.
        let dir = tmp("older");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(file_name(0));
        let schema = Arc::new(Schema::new(vec![
            Field::new("t_s", DataType::Float64, false),
            Field::new("speed_kmh", DataType::Float32, false),
        ]));
        let mut t = Float64Builder::new();
        let mut v = Float32Builder::new();
        for i in 0..20 {
            t.append_value(i as f64 * 0.02);
            v.append_value(100.0 + i as f32);
        }
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(t.finish()) as ArrayRef, Arc::new(v.finish())],
        )
        .unwrap();
        let mut w = ArrowWriter::try_new(File::create(&path).unwrap(), schema, None).unwrap();
        w.write(&batch).unwrap();
        w.close().unwrap();

        let ch = |n: &str| channels::channel(n).unwrap();
        let cols = read_columns(
            &path,
            &[ch("speed_kmh"), ch("optimal_temp_c_fl"), ch("t_s")],
            2,
            7,
        )
        .unwrap();
        assert_eq!(cols.len(), 3);
        assert_eq!(
            cols[0],
            (2..7).map(|i| Some(100.0 + i as f64)).collect::<Vec<_>>()
        );
        assert_eq!(cols[1], vec![None; 5]);
        assert_eq!(cols[2].len(), 5);
        // Only absent channels: nulls, no projection error.
        let cols = read_columns(&path, &[ch("optimal_temp_c_rr")], 0, 50).unwrap();
        assert_eq!(cols[0], vec![None; 20]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn checkpointed_file_survives_a_crash() {
        // Simulate a crash: checkpoint, then drop the writer *without* close
        // (std::mem::forget skips Drop) — the file must still be readable.
        let dir = tmp("crash");
        let cfg = WriterConfig {
            row_group_rows: 40,
            ..Default::default()
        };
        let mut w = SampleWriter::new(&dir, cfg).unwrap();
        append_n(&mut w, 0, 95);
        let files = w.checkpoint().unwrap();
        // Rows appended after the checkpoint are lost on crash, by design.
        append_n(&mut w, 95, 20);
        std::mem::forget(w);
        let (rows, _) = file_summary(&dir.join(&files[0].name)).unwrap();
        assert_eq!(rows, 95);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
