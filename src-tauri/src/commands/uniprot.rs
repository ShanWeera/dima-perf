//! UniProt Commands
//!
//! Tauri commands for fetching protein annotations from UniProt
//! and resolving UniProt accessions from PDB IDs via RCSB Data API.

use crate::error::AppError;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types returned to the frontend
// ---------------------------------------------------------------------------

/// A single protein feature annotation from UniProt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProteinFeature {
    pub feature_type: String,
    pub category: String,
    pub description: String,
    /// Start position in UniProt numbering (1-based)
    pub begin: u32,
    /// End position in UniProt numbering (1-based)
    pub end: u32,
    pub evidences: Vec<String>,
}

/// Resolved UniProt protein metadata together with its features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniProtInfo {
    pub accession: String,
    pub protein_name: String,
    pub organism: String,
    pub sequence_length: u32,
    /// Full UniProt canonical sequence (needed for alignment to PDB)
    pub sequence: String,
    pub features: Vec<ProteinFeature>,
}

// ---------------------------------------------------------------------------
// Internal deserialization types for the RCSB Data API JSON response
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RcsbPolymerEntity {
    rcsb_polymer_entity_container_identifiers: Option<RcsbIds>,
}

#[derive(Deserialize)]
struct RcsbIds {
    reference_sequence_identifiers: Option<Vec<RefSeqId>>,
}

#[derive(Deserialize)]
struct RefSeqId {
    database_name: Option<String>,
    database_accession: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal deserialization types for the UniProt Proteins API JSON response
// ---------------------------------------------------------------------------

/// Root response from https://www.ebi.ac.uk/proteins/api/proteins/{accession}
#[derive(Deserialize)]
struct UniProtEntry {
    protein: Option<UniProtProtein>,
    organism: Option<UniProtOrganism>,
    sequence: Option<UniProtSequence>,
    features: Option<Vec<UniProtFeatureRaw>>,
}

#[derive(Deserialize)]
struct UniProtProtein {
    #[serde(rename = "recommendedName")]
    recommended_name: Option<UniProtName>,
    #[serde(rename = "submittedName")]
    submitted_name: Option<Vec<UniProtName>>,
}

#[derive(Deserialize)]
struct UniProtName {
    #[serde(rename = "fullName")]
    full_name: Option<UniProtValue>,
}

#[derive(Deserialize)]
struct UniProtValue {
    value: Option<String>,
}

#[derive(Deserialize)]
struct UniProtOrganism {
    names: Option<Vec<UniProtOrgName>>,
}

#[derive(Deserialize)]
struct UniProtOrgName {
    #[serde(rename = "type")]
    name_type: Option<String>,
    value: Option<String>,
}

#[derive(Deserialize)]
struct UniProtSequence {
    length: Option<u32>,
    sequence: Option<String>,
}

#[derive(Deserialize)]
struct UniProtFeatureRaw {
    #[serde(rename = "type")]
    feature_type: Option<String>,
    category: Option<String>,
    description: Option<String>,
    begin: Option<String>,
    end: Option<String>,
    evidences: Option<Vec<UniProtEvidence>>,
}

#[derive(Deserialize)]
struct UniProtEvidence {
    code: Option<String>,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Look up the UniProt accession associated with a PDB polymer entity.
///
/// Queries the RCSB Data API for the polymer entity identified by
/// `pdb_id` (legacy 4-char or extended 12-char) and `entity_id`
/// (usually 1 for the first chain).
/// Returns the first UniProt accession found, or an error if none exists.
#[tauri::command]
pub async fn fetch_uniprot_accession(
    pdb_id: String,
    entity_id: u32,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, AppError> {
    let pdb_id = pdb_id.trim().to_uppercase();

    let is_legacy = pdb_id.len() == 4
        && pdb_id.chars().all(|c| c.is_ascii_alphanumeric());
    let is_extended = pdb_id.len() == 12
        && pdb_id.starts_with("PDB_")
        && pdb_id[4..].chars().all(|c| c.is_ascii_alphanumeric());

    if !is_legacy && !is_extended {
        return Err(AppError::ValidationError(
            "PDB ID must be a 4-character code (e.g. 6VXX) or extended format (e.g. pdb_00001abc)".to_string()
        ));
    }

    let url = format!(
        "https://data.rcsb.org/rest/v1/core/polymer_entity/{}/{}",
        pdb_id, entity_id
    );

    let retry_config = super::http_retry::RetryConfig::default();
    let response = super::http_retry::send_with_retry(
        &state.http_client,
        &retry_config,
        |client| client.get(&url),
    ).await?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(match status.as_u16() {
            404 => AppError::NotFound(format!(
                "RCSB entity {}/{} not found", pdb_id, entity_id
            )),
            _ => AppError::NetworkError(format!(
                "RCSB query failed for entity {}/{} (HTTP {})", pdb_id, entity_id, status
            )),
        });
    }

    const MAX_RCSB_SIZE: u64 = 10 * 1024 * 1024;
    let mut body_bytes = Vec::new();
    let mut stream = response;
    while let Some(chunk) = stream.chunk().await
        .map_err(|e| AppError::NetworkError(format!("Failed to read RCSB response: {}", e)))? {
        body_bytes.extend_from_slice(&chunk);
        if body_bytes.len() as u64 > MAX_RCSB_SIZE {
            return Err(AppError::ValidationError("RCSB response exceeds 10 MB size limit".to_string()));
        }
    }
    let entity: RcsbPolymerEntity = serde_json::from_slice(&body_bytes)
        .map_err(|e| AppError::InternalError(format!("Failed to parse RCSB response: {}", e)))?;

    // Walk the reference_sequence_identifiers looking for a UniProt entry
    if let Some(ids) = entity.rcsb_polymer_entity_container_identifiers {
        if let Some(refs) = ids.reference_sequence_identifiers {
            for r in &refs {
                if r.database_name.as_deref() == Some("UniProt") {
                    if let Some(acc) = &r.database_accession {
                        return Ok(acc.clone());
                    }
                }
            }
        }
    }

    Err(AppError::NotFound(format!(
        "No UniProt accession found for PDB entity {}/{}",
        pdb_id, entity_id
    )))
}

/// Fetch protein information and feature annotations from UniProt.
///
/// Calls the EBI Proteins API for the given accession, parses the JSON
/// response, and returns a structured `UniProtInfo` containing metadata
/// and all feature annotations.
#[tauri::command]
pub async fn fetch_uniprot_features(
    accession: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<UniProtInfo, AppError> {
    let accession = accession.trim().to_uppercase();
    if accession.is_empty() {
        return Err(AppError::ValidationError("UniProt accession cannot be empty".to_string()));
    }
    if !accession.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::ValidationError("UniProt accession contains invalid characters".to_string()));
    }
    if accession.len() > 20 {
        return Err(AppError::ValidationError("UniProt accession is too long".to_string()));
    }

    let url = format!(
        "https://www.ebi.ac.uk/proteins/api/proteins/{}",
        accession
    );

    let retry_config = super::http_retry::RetryConfig::default();
    let url_clone = url.clone();
    let response = super::http_retry::send_with_retry(
        &state.http_client,
        &retry_config,
        |client| client.get(&url_clone).header("Accept", "application/json"),
    ).await?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(match status.as_u16() {
            404 => AppError::NotFound(format!("UniProt accession '{}' not found", accession)),
            _ => AppError::NetworkError(format!(
                "UniProt query failed for '{}' (HTTP {})", accession, status
            )),
        });
    }

    const MAX_UNIPROT_SIZE: u64 = 50 * 1024 * 1024;
    if let Some(len) = response.content_length() {
        if len > MAX_UNIPROT_SIZE {
            return Err(AppError::ValidationError(format!(
                "UniProt response too large ({:.1} MB). Maximum supported: 50 MB.",
                len as f64 / (1024.0 * 1024.0)
            )));
        }
    }

    let mut body_bytes = Vec::new();
    let mut stream = response;
    while let Some(chunk) = stream.chunk().await
        .map_err(|e| AppError::NetworkError(format!("Failed to read UniProt response: {}", e)))? {
        body_bytes.extend_from_slice(&chunk);
        if body_bytes.len() as u64 > MAX_UNIPROT_SIZE {
            return Err(AppError::ValidationError(format!(
                "UniProt response exceeds size limit ({:.1} MB). Maximum supported: 50 MB.",
                body_bytes.len() as f64 / (1024.0 * 1024.0)
            )));
        }
    }

    let entry: UniProtEntry = serde_json::from_slice(&body_bytes)
        .map_err(|e| AppError::InternalError(format!("Failed to parse UniProt response: {}", e)))?;

    // Extract protein name
    let protein_name = entry
        .protein
        .as_ref()
        .and_then(|p| {
            p.recommended_name
                .as_ref()
                .and_then(|n| n.full_name.as_ref().and_then(|v| v.value.clone()))
                .or_else(|| {
                    p.submitted_name.as_ref().and_then(|names| {
                        names.first().and_then(|n| {
                            n.full_name.as_ref().and_then(|v| v.value.clone())
                        })
                    })
                })
        })
        .unwrap_or_else(|| "Unknown protein".to_string());

    // Extract organism (scientific name preferred)
    let organism = entry
        .organism
        .as_ref()
        .and_then(|o| {
            o.names.as_ref().and_then(|names| {
                names
                    .iter()
                    .find(|n| n.name_type.as_deref() == Some("scientific"))
                    .or_else(|| names.first())
                    .and_then(|n| n.value.clone())
            })
        })
        .unwrap_or_else(|| "Unknown organism".to_string());

    // Extract sequence
    let sequence = entry
        .sequence
        .as_ref()
        .and_then(|s| s.sequence.clone())
        .unwrap_or_default();

    let sequence_length = entry
        .sequence
        .as_ref()
        .and_then(|s| s.length)
        .unwrap_or(sequence.len() as u32);

    // Parse features, filtering to the categories we care about
    let supported_types = [
        "DOMAIN", "REGION", "BINDING", "NP_BIND", "ACT_SITE", "SIGNAL",
        "TRANSMEM", "CARBOHYD", "DISULFID", "TOPO_DOM", "MOTIF",
    ];

    let features: Vec<ProteinFeature> = entry
        .features
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| {
            let ft_raw = f.feature_type.as_deref().unwrap_or("");
            // Case-insensitive comparison: UniProt API may return differently
            // cased feature types (e.g. "Domain" vs "DOMAIN"). (Fix 5.4)
            let ft_upper = ft_raw.to_uppercase();
            if !supported_types.contains(&ft_upper.as_str()) {
                return None;
            }
            let ft = ft_upper;

            let begin: u32 = f.begin.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
            let end: u32 = f.end.as_deref().and_then(|s| s.parse().ok()).unwrap_or(begin);

            // Skip features with invalid positions (0 means unparseable,
            // begin > end means the range is inverted/corrupt)
            if begin == 0 || begin > end {
                return None;
            }

            let evidences: Vec<String> = f
                .evidences
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| e.code)
                .collect();

            Some(ProteinFeature {
                feature_type: ft,
                category: f.category.unwrap_or_default(),
                description: f.description.unwrap_or_default(),
                begin,
                end,
                evidences,
            })
        })
        .collect();

    Ok(UniProtInfo {
        accession,
        protein_name,
        organism,
        sequence_length,
        sequence,
        features,
    })
}
