//! Memory profiling utilities for DMN subsystem benchmarks (Phase 36).
//!
//! Provides RSS peak measurement via `/proc/self/status` on Linux and a fallback
//! using Criterion's allocation tracking. Used by `engine/benches/memory_baseline.rs`
//! to capture baseline snapshots of the v6.0 corpus.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// RSS (Resident Set Size) snapshot from `/proc/self/status`.
#[derive(Debug, Clone)]
pub struct RssSnapshot {
    /// VmRSS in bytes (from /proc/self/status).
    pub vm_rss_bytes: u64,
    /// Timestamp when the snapshot was taken.
    pub timestamp_ns: u128,
}

impl RssSnapshot {
    /// Take a current RSS snapshot from `/proc/self/status`.
    pub fn take() -> Self {
        let content = fs::read_to_string("/proc/self/status")
            .unwrap_or_default();
        let vm_rss = content
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .and_then(|line| {
                line.split_whitespace()
                    .nth(1)
                    .and_then(|kb_str| kb_str.parse::<u64>().ok())
                    .map(|kb| kb * 1024) // Convert KB to bytes.
            })
            .unwrap_or(0);

        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        RssSnapshot {
            vm_rss_bytes: vm_rss,
            timestamp_ns,
        }
    }

    /// Calculate RSS delta from this snapshot to another.
    pub fn delta_from(&self, other: &RssSnapshot) -> i64 {
        self.vm_rss_bytes as i64 - other.vm_rss_bytes as i64
    }
}

/// Per-phase allocation breakdown for memory baseline snapshots.
#[derive(Debug, Clone)]
pub struct PhaseBreakdown {
    /// Phase name (e.g., "parse", "evaluate", "drd_resolution").
    pub phase: String,
    /// RSS delta from cold start to end of phase (bytes).
    pub rss_delta_bytes: i64,
    /// Number of allocations (from Criterion or estimated).
    pub allocation_count: u64,
}

/// Memory baseline snapshot for a DMN corpus evaluation.
#[derive(Debug, Clone)]
pub struct MemoryBaseline {
    /// Corpus name (e.g., "v6.0-tck").
    pub corpus_name: String,
    /// Total number of .dmn files in the corpus.
    pub file_count: usize,
    /// Cold-start RSS before any work begins.
    pub cold_start_rss_bytes: u64,
    /// Per-phase breakdown.
    pub phases: Vec<PhaseBreakdown>,
    /// Peak RSS observed during evaluation.
    pub peak_rss_bytes: u64,
    /// Total wall-clock time in nanoseconds.
    pub total_duration_ns: u128,
}

impl MemoryBaseline {
    /// Serialize the baseline to JSON format (human-readable and diffable).
    pub fn to_json(&self) -> String {
        let phases_json: Vec<String> = self.phases.iter().map(|p| {
            format!(
                r#"    {{
      "phase": {},
      "rss_delta_bytes": {},
      "allocation_count": {}
    }}"#,
                json_string(&p.phase),
                p.rss_delta_bytes,
                p.allocation_count
            )
        }).collect();

        format!(
            r#"{{
  "corpus_name": {},
  "file_count": {},
  "cold_start_rss_bytes": {},
  "phases": [
{}
  ],
  "peak_rss_bytes": {},
  "total_duration_ns": {}
}}"#,
            json_string(&self.corpus_name),
            self.file_count,
            self.cold_start_rss_bytes,
            phases_json.join(",\n"),
            self.peak_rss_bytes,
            self.total_duration_ns,
        )
    }

    /// Load a baseline from a JSON file (for regression comparison).
    pub fn from_json(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("failed to read baseline {}: {}", path.display(), e))?;

        // Parse the JSON manually for simplicity (no serde dependency in benchmarks).
        // This is a simplified parser that handles our known schema.
        let corpus_name = extract_field(&content, "corpus_name")?;
        let file_count: usize = extract_field(&content, "file_count")?.parse()
            .map_err(|e| format!("invalid file_count: {}", e))?;
        let cold_start_rss_bytes: u64 = extract_field(&content, "cold_start_rss_bytes")?.parse()
            .map_err(|e| format!("invalid cold_start_rss_bytes: {}", e))?;
        let peak_rss_bytes: u64 = extract_field(&content, "peak_rss_bytes")?.parse()
            .map_err(|e| format!("invalid peak_rss_bytes: {}", e))?;
        let total_duration_ns: u128 = extract_field(&content, "total_duration_ns")?.parse()
            .map_err(|e| format!("invalid total_duration_ns: {}", e))?;

        // Parse phases array.
        let phases_start = content.find("\"phases\": [")
            .ok_or("missing phases field")? + "\"phases\": [".len();
        let phases_end = content[phases_start..].find(']')
            .ok_or("unclosed phases array")? + phases_start;

        let phases_json = &content[phases_start..phases_end];
        let mut phases = Vec::new();

        // Split by "}" to get individual phase objects.
        for phase_str in phases_json.split('}').filter(|s| !s.trim().is_empty()) {
            let phase_name = extract_field(phase_str, "phase")?;
            let rss_delta: i64 = extract_field(phase_str, "rss_delta_bytes")?.parse()
                .map_err(|e| format!("invalid rss_delta_bytes: {}", e))?;
            let alloc_count: u64 = extract_field(phase_str, "allocation_count")?.parse()
                .map_err(|e| format!("invalid allocation_count: {}", e))?;

            phases.push(PhaseBreakdown {
                phase: phase_name,
                rss_delta_bytes: rss_delta,
                allocation_count: alloc_count,
            });
        }

        Ok(MemoryBaseline {
            corpus_name,
            file_count,
            cold_start_rss_bytes,
            phases,
            peak_rss_bytes,
            total_duration_ns,
        })
    }
}

/// Extract a string field value from JSON (simplified parser).
fn extract_field<'a>(json: &'a str, field: &str) -> Result<String, String> {
    let key = format!("\"{}\":", field);
    let pos = json.find(&key)
        .ok_or_else(|| format!("field {} not found", field))? + key.len();

    // Skip whitespace.
    let mut pos = pos;
    while pos < json.len() && json[pos..].starts_with(char::is_whitespace) {
        pos += 1;
    }

    if pos >= json.len() {
        return Err(format!("field {} has no value", field));
    }

    let c = json.as_bytes()[pos];
    if c == b'"' {
        // String value.
        let start = pos + 1;
        let end = json[start..].find('"')
            .ok_or_else(|| format!("unclosed string for field {}", field))? + start;
        Ok(json[start..end].to_string())
    } else {
        // Numeric/boolean/null value — read until comma, }, or ].
        let mut end = pos;
        while end < json.len() && !matches!(json.as_bytes()[end], b',' | b'}' | b']') {
            end += 1;
        }
        Ok(json[pos..end].trim().to_string())
    }
}

/// Wrap a string in JSON quotes (escape special characters).
fn json_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('"');
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

/// Collect every `.dmn` file under a directory (recursively).
pub fn collect_dmn_files(root: &Path) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(d).expect("read dir") {
            let e = entry.expect("read entry");
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |ext| ext == "dmn") {
                out.push(p);
            }
        }
    }
    walk(root, &mut files);
    files.sort();
    files
}

/// Locate the TCK fixture root.
pub fn tck_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().unwrap_or_else(|| Path::new("."));
    repo_root.join("engine/tests/fixtures/tck")
}

/// Locate the import-test corpus.
pub fn import_corpus_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().unwrap_or_else(|| Path::new("."));
    repo_root.join("api/dmn")
}

/// Load the combined corpus: TCK fixtures + import-test DMN files.
pub fn load_corpus_files() -> Vec<PathBuf> {
    let mut files = collect_dmn_files(&tck_root());
    files.extend(collect_dmn_files(&import_corpus_root()));
    files.sort();
    files
}

/// Write a baseline snapshot to the snapshots directory (engine/benches/snapshots/).
pub fn write_baseline_snapshot(baseline: &MemoryBaseline, corpus_name: &str) -> Result<PathBuf, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().unwrap_or_else(|| Path::new("."));
    let snapshots_dir = repo_root.join("engine/benches/snapshots");

    fs::create_dir_all(&snapshots_dir)
        .map_err(|e| format!("failed to create snapshots dir: {}", e))?;

    let filename = format!("{}.json", corpus_name);
    let path = snapshots_dir.join(filename);

    let json = baseline.to_json();
    fs::write(&path, &json)
        .map_err(|e| format!("failed to write baseline {}: {}", path.display(), e))?;

    Ok(path)
}
