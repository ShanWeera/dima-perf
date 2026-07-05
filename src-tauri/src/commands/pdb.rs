//! PDB Commands
//!
//! Tauri commands for fetching, parsing, and processing PDB files
//! for 3D structure visualization with HCS highlighting.

use crate::error::AppError;
use bio::alignment::pairwise::Aligner;
use bio::alignment::AlignmentOperation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Information about a chain in a PDB file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: String,
    pub sequence: String,
    pub residue_numbers: Vec<i32>,
}

/// Position mapping between MSA positions and PDB residue numbers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionMapping {
    pub msa_to_pdb: HashMap<usize, i32>,
    pub alignment_score: f64,
    pub coverage: f64,
}

/// Fetch a PDB file from RCSB PDB by ID.
///
/// Accepts both legacy 4-character IDs (e.g. "6VXX") and the new extended
/// wwPDB format (e.g. "pdb_00001abc"). RCSB resolves both at the same
/// download endpoint.
#[tauri::command]
pub async fn fetch_pdb(
    pdb_id: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, AppError> {
    let pdb_id = pdb_id.trim().to_uppercase();

    // Validate PDB ID format:
    //   Legacy: exactly 4 alphanumeric chars (e.g. "6VXX")
    //   Extended: "PDB_" prefix + 8 alphanumeric chars (e.g. "PDB_00001ABC")
    let is_legacy = pdb_id.len() == 4 && pdb_id.chars().all(|c| c.is_ascii_alphanumeric());
    let is_extended = pdb_id.len() == 12
        && pdb_id.starts_with("PDB_")
        && pdb_id[4..].chars().all(|c| c.is_ascii_alphanumeric());

    if !is_legacy && !is_extended {
        return Err(AppError::ValidationError(
            "PDB ID must be a 4-character code (e.g. 6VXX) or extended format (e.g. pdb_00001abc)"
                .to_string(),
        ));
    }

    // RCSB download endpoint accepts both formats directly
    let url = format!("https://files.rcsb.org/download/{}.pdb", pdb_id);

    let retry_config = super::http_retry::RetryConfig::default();
    let response =
        super::http_retry::send_with_retry(&state.http_client, &retry_config, |client| {
            client.get(&url)
        })
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(match status.as_u16() {
            404 => AppError::NotFound(format!("PDB ID '{}' not found", pdb_id)),
            429 => AppError::NetworkError(
                "Rate limited by RCSB (HTTP 429). Please wait a moment and try again.".to_string(),
            ),
            500..=599 => AppError::NetworkError(format!(
                "RCSB server error (HTTP {}). The service may be temporarily unavailable.",
                status
            )),
            _ => AppError::NetworkError(format!(
                "Failed to fetch PDB '{}' (HTTP {})",
                pdb_id, status
            )),
        });
    }

    // Guard against oversized responses regardless of Content-Length presence.
    // Chunked transfer-encoding may omit Content-Length. (Fix 4.39)
    const MAX_PDB_SIZE: u64 = 100 * 1024 * 1024;
    if let Some(len) = response.content_length() {
        if len > MAX_PDB_SIZE {
            return Err(AppError::ValidationError(format!(
                "PDB file too large ({:.1} MB). Maximum supported: 100 MB.",
                len as f64 / (1024.0 * 1024.0)
            )));
        }
    }

    // Read body in chunks with an aggregate cap to prevent OOM from
    // chunked-transfer responses that lack Content-Length. (Fix 4.39)
    let mut body_bytes = Vec::new();
    let mut stream = response;
    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to read PDB content: {}", e)))?
    {
        body_bytes.extend_from_slice(&chunk);
        if body_bytes.len() as u64 > MAX_PDB_SIZE {
            return Err(AppError::ValidationError(format!(
                "PDB response exceeds size limit ({:.1} MB). Maximum supported: 100 MB.",
                body_bytes.len() as f64 / (1024.0 * 1024.0)
            )));
        }
    }

    String::from_utf8(body_bytes)
        .map_err(|e| AppError::ValidationError(format!("PDB content is not valid UTF-8: {}", e)))
}

/// Parse PDB content and extract sequence information for each chain.
/// Handles insertion codes (iCode), alternate conformations, multi-MODEL files,
/// and negative residue sequence numbers correctly per PDB format specification.
/// Runs on a blocking thread to avoid stalling the Tauri worker thread (Fix 4.22).
#[tauri::command]
pub async fn parse_pdb_sequence(pdb_content: String) -> Result<Vec<ChainInfo>, AppError> {
    tokio::task::spawn_blocking(move || parse_pdb_sequence_inner(&pdb_content))
        .await
        .map_err(|e| AppError::AnalysisError(format!("PDB parsing task panicked: {}", e)))?
}

fn parse_pdb_sequence_inner(pdb_content: &str) -> Result<Vec<ChainInfo>, AppError> {
    // Guard against excessively large PDB files that could cause OOM.
    // Typical PDB files are under 20MB; 50MB is extremely generous.
    const MAX_PDB_SIZE: usize = 50 * 1024 * 1024;
    if pdb_content.len() > MAX_PDB_SIZE {
        return Err(AppError::ValidationError(format!(
            "PDB content exceeds maximum size ({} MB). File is {} MB.",
            MAX_PDB_SIZE / (1024 * 1024),
            pdb_content.len() / (1024 * 1024)
        )));
    }

    // Detect mmCIF format early — common mistake when uploading .cif as .pdb
    let first_line = pdb_content.lines().next().unwrap_or("");
    if first_line.starts_with("data_") || pdb_content.contains("_atom_site.") {
        return Err(AppError::ValidationError(
            "This file appears to be in mmCIF/PDBx format, not legacy PDB format. \
                    Please convert to PDB format or download the .pdb version from RCSB."
                .to_string(),
        ));
    }

    let mut chains: HashMap<String, (Vec<char>, Vec<i32>)> = HashMap::new();
    // Track last (resSeq, iCode) per chain to deduplicate atoms of same residue
    let mut last_residue: HashMap<String, (i32, char)> = HashMap::new();
    // Parse the first MODEL that contains actual ATOM records. For NMR/ensemble
    // PDB files, the first MODEL may be empty; continue to subsequent models
    // rather than giving up after an empty first model. (Fix 4.16)
    for line in pdb_content.lines() {
        if line.starts_with("MODEL ") {
            if !chains.is_empty() {
                break;
            }
            continue;
        }
        if line.starts_with("ENDMDL") {
            if !chains.is_empty() {
                break;
            }
            continue;
        }

        // Parse ATOM records for protein residues
        if line.starts_with("ATOM  ") || line.starts_with("HETATM") {
            if line.len() < 54 {
                continue;
            }

            // PDB format: column 17 is altLoc (alternate conformation indicator)
            // Only accept blank or 'A' (first alternate) to avoid duplicates
            let alt_loc = line.as_bytes().get(16).copied().unwrap_or(b' ');
            if alt_loc != b' ' && alt_loc != b'A' {
                continue;
            }

            // Extract chain ID (column 22, 0-indexed: 21)
            let chain_id = line.as_bytes().get(21).copied().unwrap_or(b' ');
            if chain_id == b' ' {
                continue;
            }
            let chain_id = String::from(chain_id as char);

            // Extract residue number (columns 23-26, 0-indexed: 22-25)
            let resi_str = &line[22..26];
            let resi: i32 = match resi_str.trim().parse() {
                Ok(n) => n,
                Err(_) => continue,
            };

            // Extract insertion code (column 27, 0-indexed: 26)
            // Used to uniquely identify residues with same sequence number
            let icode = line.as_bytes().get(26).copied().unwrap_or(b' ');

            // Extract residue name (columns 18-20, 0-indexed: 17-19)
            let resn = &line[17..20];
            let resn = resn.trim();

            // Convert 3-letter code to 1-letter code
            let one_letter = three_to_one(resn);
            if one_letter == 'X' && line.starts_with("HETATM") {
                continue;
            }

            // Deduplicate: same chain + residue number + insertion code = same residue
            let chain_entry = chains
                .entry(chain_id.clone())
                .or_insert((Vec::new(), Vec::new()));
            let last = last_residue.get(&chain_id);

            // A new residue if either resSeq or iCode differs from the last seen
            let is_new = match last {
                Some((last_resi, last_icode)) => *last_resi != resi || *last_icode != icode as char,
                None => true,
            };

            if is_new {
                chain_entry.0.push(one_letter);
                chain_entry.1.push(resi);
                last_residue.insert(chain_id, (resi, icode as char));
            }
        }
    }

    // Convert to ChainInfo structs
    let mut result: Vec<ChainInfo> = chains
        .into_iter()
        .map(|(chain_id, (seq, residues))| ChainInfo {
            chain_id,
            sequence: seq.into_iter().collect(),
            residue_numbers: residues,
        })
        .collect();

    // Sort by chain ID
    result.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));

    if result.is_empty() {
        return Err(AppError::NotFound(
            "No protein chains found in PDB file".to_string(),
        ));
    }

    Ok(result)
}

/// Align MSA sequence to PDB sequence and return position mapping.
/// Runs on a blocking thread since Needleman-Wunsch is O(n*m) (Fix 4.22).
#[tauri::command]
pub async fn align_sequences(
    msa_sequence: String,
    pdb_sequence: String,
    pdb_residue_numbers: Vec<i32>,
) -> Result<PositionMapping, AppError> {
    tokio::task::spawn_blocking(move || {
        align_sequences_inner(msa_sequence, pdb_sequence, pdb_residue_numbers)
    })
    .await
    .map_err(|e| AppError::AnalysisError(format!("Alignment task panicked: {}", e)))?
}

fn align_sequences_inner(
    msa_sequence: String,
    pdb_sequence: String,
    pdb_residue_numbers: Vec<i32>,
) -> Result<PositionMapping, AppError> {
    if msa_sequence.is_empty() || pdb_sequence.is_empty() {
        return Err(AppError::ValidationError(
            "Sequences cannot be empty".to_string(),
        ));
    }

    if pdb_sequence.len() != pdb_residue_numbers.len() {
        return Err(AppError::ValidationError(
            "PDB sequence length must match residue numbers length".to_string(),
        ));
    }

    // Guard against pathological inputs: Needleman-Wunsch builds an O(n*m)
    // DP matrix which can spike memory for very long sequences.
    const MAX_ALIGNMENT_LENGTH: usize = 50_000;
    const MAX_ALIGNMENT_PRODUCT: usize = 4_000_000;
    if msa_sequence.len() > MAX_ALIGNMENT_LENGTH || pdb_sequence.len() > MAX_ALIGNMENT_LENGTH {
        return Err(AppError::ValidationError(format!(
            "Sequence too long for alignment (max {} residues). MSA: {}, PDB: {}",
            MAX_ALIGNMENT_LENGTH,
            msa_sequence.len(),
            pdb_sequence.len()
        )));
    }
    // Product cap prevents OOM from the O(n*m) DP matrix even when individual
    // sequences are below the per-sequence limit (e.g., 3000 × 3000 = 9M is fine,
    // but 50000 × 50000 = 2.5B cells would require ~10 GB).
    let product = msa_sequence.len().saturating_mul(pdb_sequence.len());
    if product > MAX_ALIGNMENT_PRODUCT {
        return Err(AppError::ValidationError(format!(
            "Alignment matrix too large ({} × {} = {} cells, max {}). Use shorter sequences.",
            msa_sequence.len(),
            pdb_sequence.len(),
            product,
            MAX_ALIGNMENT_PRODUCT
        )));
    }

    // Needleman-Wunsch global alignment scoring parameters. (Fix 5.6)
    // These values are standard for simple protein/nucleotide alignment:
    //   - Match reward (+2): positive reinforcement for identical residues
    //   - Mismatch penalty (-1): mild penalty allows partial homology
    //   - Gap open (-5): strong penalty discourages gap introduction
    //   - Gap extend (-1): lower penalty permits extending existing gaps
    const MATCH_SCORE: i32 = 2;
    const MISMATCH_SCORE: i32 = -1;
    const GAP_OPEN: i32 = -5;
    const GAP_EXTEND: i32 = -1;

    let score_fn = |a: u8, b: u8| if a == b { MATCH_SCORE } else { MISMATCH_SCORE };

    let mut aligner = Aligner::with_capacity(
        msa_sequence.len(),
        pdb_sequence.len(),
        GAP_OPEN,
        GAP_EXTEND,
        score_fn,
    );

    let alignment = aligner.global(msa_sequence.as_bytes(), pdb_sequence.as_bytes());

    // Build position mapping from alignment.
    // Keys are 1-based MSA positions (matching analysis output `position.position`).
    let mut msa_to_pdb: HashMap<usize, i32> = HashMap::new();
    let mut msa_pos = 0usize; // 0-based index into MSA sequence
    let mut pdb_pos = 0usize; // 0-based index into PDB sequence
    let mut matches = 0usize;

    for op in &alignment.operations {
        match op {
            AlignmentOperation::Match => {
                matches += 1;
                // Only map residue-matched positions for confident PDB highlighting
                if pdb_pos < pdb_residue_numbers.len() {
                    // 1-based key to match frontend position.position values
                    msa_to_pdb.insert(msa_pos + 1, pdb_residue_numbers[pdb_pos]);
                }
                msa_pos += 1;
                pdb_pos += 1;
            }
            AlignmentOperation::Subst => {
                // Mismatched residues — still advance both counters but don't create
                // a confident mapping (substitution could indicate structural divergence)
                msa_pos += 1;
                pdb_pos += 1;
            }
            AlignmentOperation::Ins => {
                // Gap in PDB sequence (insertion in MSA) — no PDB counterpart
                msa_pos += 1;
            }
            AlignmentOperation::Del => {
                // Gap in MSA sequence (deletion, residue only in PDB)
                pdb_pos += 1;
            }
            AlignmentOperation::Xclip(n) => {
                // Clipping at start/end of MSA sequence — advance by clip length
                msa_pos += n;
            }
            AlignmentOperation::Yclip(n) => {
                // Clipping at start/end of PDB sequence — advance by clip length
                pdb_pos += n;
            }
        }
    }

    let coverage = if msa_sequence.is_empty() {
        0.0
    } else {
        (msa_to_pdb.len() as f64 / msa_sequence.len() as f64) * 100.0
    };

    let alignment_score = if msa_sequence.is_empty() {
        0.0
    } else {
        (matches as f64 / msa_sequence.len() as f64) * 100.0
    };

    Ok(PositionMapping {
        msa_to_pdb,
        alignment_score,
        coverage,
    })
}

/// Create a direct 1:1 position mapping with an optional offset.
/// `msa_positions` are already 1-based (from position.position in frontend).
/// Maps MSA position N to PDB residue N + offset.
#[tauri::command]
pub fn create_direct_mapping(
    msa_positions: Vec<usize>,
    pdb_residue_numbers: Vec<i32>,
    offset: i32,
) -> Result<PositionMapping, AppError> {
    const MAX_POSITIONS: usize = 1_000_000;
    if msa_positions.len() > MAX_POSITIONS {
        return Err(AppError::ValidationError(format!(
            "MSA positions count ({}) exceeds limit ({})",
            msa_positions.len(),
            MAX_POSITIONS
        )));
    }
    let mut msa_to_pdb: HashMap<usize, i32> = HashMap::new();
    let pdb_set: std::collections::HashSet<i32> = pdb_residue_numbers.iter().copied().collect();

    let mut mapped_count = 0usize;

    for msa_pos in &msa_positions {
        // Safe arithmetic to prevent i32 overflow for large MSA positions (Fix 4.41)
        let pdb_resi = match (*msa_pos as i64).checked_add(offset as i64) {
            Some(v) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => v as i32,
            _ => continue, // Skip positions that would overflow i32
        };

        if pdb_set.contains(&pdb_resi) {
            msa_to_pdb.insert(*msa_pos, pdb_resi);
            mapped_count += 1;
        }
    }

    let coverage = if !msa_positions.is_empty() {
        (mapped_count as f64 / msa_positions.len() as f64) * 100.0
    } else {
        0.0
    };

    Ok(PositionMapping {
        msa_to_pdb,
        // For direct (non-alignment) mapping, there's no alignment score — use 0.0
        // to distinguish from the alignment path which computes a real score. (Fix 5.6)
        alignment_score: 0.0,
        coverage,
    })
}

/// Convert 3-letter amino acid code to 1-letter code
fn three_to_one(resn: &str) -> char {
    match resn.to_uppercase().as_str() {
        "ALA" => 'A',
        "ARG" => 'R',
        "ASN" => 'N',
        "ASP" => 'D',
        "CYS" => 'C',
        "GLN" => 'Q',
        "GLU" => 'E',
        "GLY" => 'G',
        "HIS" => 'H',
        "ILE" => 'I',
        "LEU" => 'L',
        "LYS" => 'K',
        "MET" => 'M',
        "PHE" => 'F',
        "PRO" => 'P',
        "SER" => 'S',
        "THR" => 'T',
        "TRP" => 'W',
        "TYR" => 'Y',
        "VAL" => 'V',
        // Non-standard but common
        "MSE" => 'M', // Selenomethionine
        "SEC" => 'U', // Selenocysteine
        "PYL" => 'O', // Pyrrolysine
        // Nucleotides (for DNA/RNA structures)
        "DA" | "A" => 'A',
        "DT" | "T" => 'T',
        "DG" | "G" => 'G',
        "DC" | "C" => 'C',
        "U" => 'U',
        _ => 'X', // Unknown
    }
}
