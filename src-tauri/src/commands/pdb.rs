//! PDB Commands
//!
//! Tauri commands for fetching, parsing, and processing PDB files
//! for 3D structure visualization with HCS highlighting.

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

/// Fetch a PDB file from RCSB PDB by ID
#[tauri::command]
pub async fn fetch_pdb(pdb_id: String) -> Result<String, String> {
    let pdb_id = pdb_id.trim().to_uppercase();
    
    if pdb_id.len() != 4 {
        return Err("PDB ID must be exactly 4 characters".to_string());
    }
    
    // RCSB PDB download URL
    let url = format!(
        "https://files.rcsb.org/download/{}.pdb",
        pdb_id
    );
    
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch PDB: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!(
            "PDB ID '{}' not found (HTTP {})",
            pdb_id,
            response.status()
        ));
    }
    
    let pdb_content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read PDB content: {}", e))?;
    
    Ok(pdb_content)
}

/// Parse PDB content and extract sequence information for each chain
#[tauri::command]
pub fn parse_pdb_sequence(pdb_content: String) -> Result<Vec<ChainInfo>, String> {
    let mut chains: HashMap<String, (Vec<char>, Vec<i32>)> = HashMap::new();
    let mut last_residue: HashMap<String, (i32, char)> = HashMap::new();
    
    for line in pdb_content.lines() {
        // Parse ATOM records for protein residues
        if line.starts_with("ATOM  ") || line.starts_with("HETATM") {
            if line.len() < 54 {
                continue;
            }
            
            // Extract chain ID (column 22, 0-indexed: 21)
            let chain_id = line.chars().nth(21).unwrap_or(' ').to_string();
            if chain_id.trim().is_empty() {
                continue;
            }
            
            // Extract residue number (columns 23-26, 0-indexed: 22-25)
            let resi_str: String = line.chars().skip(22).take(4).collect();
            let resi: i32 = match resi_str.trim().parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            
            // Extract residue name (columns 18-20, 0-indexed: 17-19)
            let resn: String = line.chars().skip(17).take(3).collect();
            let resn = resn.trim();
            
            // Convert 3-letter code to 1-letter code
            let one_letter = three_to_one(resn);
            if one_letter == 'X' && line.starts_with("HETATM") {
                // Skip non-standard HETATM residues
                continue;
            }
            
            // Check if this is a new residue (avoid duplicates from multiple atoms)
            let chain_entry = chains.entry(chain_id.clone()).or_insert((Vec::new(), Vec::new()));
            let last = last_residue.get(&chain_id);
            
            if last.map(|(r, _)| *r != resi).unwrap_or(true) {
                chain_entry.0.push(one_letter);
                chain_entry.1.push(resi);
                last_residue.insert(chain_id, (resi, one_letter));
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
        return Err("No protein chains found in PDB file".to_string());
    }
    
    Ok(result)
}

/// Align MSA sequence to PDB sequence and return position mapping
#[tauri::command]
pub fn align_sequences(
    msa_sequence: String,
    pdb_sequence: String,
    pdb_residue_numbers: Vec<i32>,
) -> Result<PositionMapping, String> {
    if msa_sequence.is_empty() || pdb_sequence.is_empty() {
        return Err("Sequences cannot be empty".to_string());
    }
    
    if pdb_sequence.len() != pdb_residue_numbers.len() {
        return Err("PDB sequence length must match residue numbers length".to_string());
    }
    
    // Use Needleman-Wunsch global alignment with simple scoring
    let score_fn = |a: u8, b: u8| if a == b { 2i32 } else { -1i32 };
    
    let mut aligner = Aligner::with_capacity(
        msa_sequence.len(),
        pdb_sequence.len(),
        -5,  // gap open penalty
        -1,  // gap extend penalty
        score_fn,
    );
    
    let alignment = aligner.global(
        msa_sequence.as_bytes(),
        pdb_sequence.as_bytes(),
    );
    
    // Build position mapping from alignment
    let mut msa_to_pdb: HashMap<usize, i32> = HashMap::new();
    let mut msa_pos = 0usize;
    let mut pdb_pos = 0usize;
    let mut matches = 0usize;
    
    for op in &alignment.operations {
        match op {
            AlignmentOperation::Match | AlignmentOperation::Subst => {
                if *op == AlignmentOperation::Match {
                    matches += 1;
                }
                if pdb_pos < pdb_residue_numbers.len() {
                    msa_to_pdb.insert(msa_pos, pdb_residue_numbers[pdb_pos]);
                }
                msa_pos += 1;
                pdb_pos += 1;
            }
            AlignmentOperation::Ins => {
                // Gap in PDB sequence (insertion in MSA)
                msa_pos += 1;
            }
            AlignmentOperation::Del => {
                // Gap in MSA sequence (deletion, residue only in PDB)
                pdb_pos += 1;
            }
            AlignmentOperation::Xclip(_) | AlignmentOperation::Yclip(_) => {
                // Clipping, ignore
            }
        }
    }
    
    let coverage = if msa_sequence.len() > 0 {
        (msa_to_pdb.len() as f64 / msa_sequence.len() as f64) * 100.0
    } else {
        0.0
    };
    
    let alignment_score = if msa_sequence.len() > 0 {
        (matches as f64 / msa_sequence.len() as f64) * 100.0
    } else {
        0.0
    };
    
    Ok(PositionMapping {
        msa_to_pdb,
        alignment_score,
        coverage,
    })
}

/// Create a direct 1:1 position mapping with an optional offset
#[tauri::command]
pub fn create_direct_mapping(
    msa_positions: Vec<usize>,
    pdb_residue_numbers: Vec<i32>,
    offset: i32,
) -> Result<PositionMapping, String> {
    let mut msa_to_pdb: HashMap<usize, i32> = HashMap::new();
    let pdb_set: std::collections::HashSet<i32> = pdb_residue_numbers.iter().copied().collect();
    
    let mut mapped_count = 0usize;
    
    for msa_pos in &msa_positions {
        // Apply 1-based conversion: MSA position 0 -> PDB residue 1 + offset
        let pdb_resi = (*msa_pos as i32) + 1 + offset;
        
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
        alignment_score: coverage, // For direct mapping, score = coverage
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
