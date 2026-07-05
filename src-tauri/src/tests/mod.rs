//! DiMA Desktop - Tauri Backend Tests
//!
//! Integration tests using the sample FASTA file.

use std::path::PathBuf;

/// Get the path to the sample FASTA file
fn get_sample_fasta_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("samples")
        .join("mers_spike_aa.fasta")
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::commands::validate::validate_fasta_blocking_public;

    // Note: These tests require the sample file to exist
    // Run with: cargo test -p dima-desktop

    #[test]
    fn test_validate_nonexistent_file() {
        let result = validate_fasta_blocking_public("/nonexistent/file.fasta");
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert!(!validation.is_valid);
        assert_eq!(validation.errors.len(), 1);
        assert_eq!(validation.errors[0].error_type, "file_not_found");
    }

    #[test]
    fn test_validate_sample_fasta() {
        let sample_path = get_sample_fasta_path();
        
        // Skip if sample file doesn't exist
        if !sample_path.exists() {
            println!("Skipping test: sample file not found at {:?}", sample_path);
            return;
        }

        let result = validate_fasta_blocking_public(&sample_path.to_string_lossy());
        assert!(result.is_ok(), "validate_fasta should not error");
        
        let validation = result.unwrap();
        assert!(validation.is_valid, "Sample FASTA should be valid");
        assert!(validation.sequence_count > 0, "Should have sequences");
        assert!(validation.sequence_length.is_some(), "Should have sequence length");
        assert!(!validation.sample_headers.is_empty(), "Should have sample headers");
        assert!(validation.file_size_bytes > 0, "Should have file size");
        assert_eq!(validation.detected_alphabet, "protein", "Should detect protein alphabet");
    }

    #[test]
    fn test_validate_with_alphabet_hint() {
        let sample_path = get_sample_fasta_path();
        
        if !sample_path.exists() {
            return;
        }

        let result = validate_fasta_blocking_public(&sample_path.to_string_lossy());
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert!(validation.is_valid);
    }
}

#[cfg(test)]
mod project_tests {
    use crate::project::sanitize_project_name;

    #[test]
    fn test_sanitize_project_name() {
        assert_eq!(sanitize_project_name("My Project"), "My Project");
        assert_eq!(sanitize_project_name("test/slash"), "test_slash");
        assert_eq!(sanitize_project_name("has:colon"), "has_colon");
        assert_eq!(sanitize_project_name("project-name_1"), "project-name_1");
    }

    #[test]
    fn test_sanitize_special_characters() {
        assert_eq!(sanitize_project_name("test<>file"), "test__file");
        assert_eq!(sanitize_project_name("test\"quotes\""), "test_quotes_");
        assert_eq!(sanitize_project_name("name|pipe"), "name_pipe");
    }
}

#[cfg(test)]
mod export_tests {
    use crate::commands::export::{ExportFormat, ExportRequest};

    #[test]
    fn test_export_format_deserialization() {
        let json = r#"{"project_path": "/test", "output_path": "/out.json", "format": "json"}"#;
        let request: ExportRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(request.format, ExportFormat::Json));

        let json = r#"{"project_path": "/test", "output_path": "/out.dima", "format": "dima"}"#;
        let request: ExportRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(request.format, ExportFormat::Dima));
    }
}

#[cfg(test)]
mod settings_tests {
    use crate::commands::settings::AppSettings;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.decimal_precision, 4);
        assert_eq!(settings.default_kmer_length, 9);
        assert_eq!(settings.default_support_threshold, 100);
        assert!(settings.default_output_directory.is_none());
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, settings.theme);
        assert_eq!(parsed.decimal_precision, settings.decimal_precision);
    }
}
