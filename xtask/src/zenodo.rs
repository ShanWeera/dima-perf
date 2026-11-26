//! `.zenodo.json`: the metadata Zenodo uses when it archives a GitHub release. When this file
//! exists Zenodo ignores `CITATION.cff`, so both are generated from the same registry.

use dima_core::citations;
use serde_json::json;

pub fn render() -> String {
    let creators: Vec<_> = citations::SOFTWARE_AUTHORS
        .iter()
        .map(|(family, given)| json!({ "name": format!("{family}, {given}") }))
        .collect();
    let paper = citations::dima_paper();
    let value = json!({
        "title": citations::SOFTWARE_TITLE,
        "upload_type": "software",
        "version": citations::SOFTWARE_VERSION,
        "description": "<p>A fast, deterministic Rust implementation of DiMA, the sequence diversity dynamics analyser for viruses (Tharanga et al. 2025, <i>Briefings in Bioinformatics</i> 26(1):bbae607). It quantifies the diversity of viral protein or nucleotide alignments by k-mer position: Shannon entropy with a sample-size correction, optional similarity-weighted entropy (Wu 2025, <i>Virus Evolution</i> 11(1):veaf029), Index/Major/Minor/Unique motif classification, metadata stratification and historically conserved sequences. Command-line tool and desktop application.</p>",
        "creators": creators,
        "license": "MIT",
        "access_right": "open",
        "keywords": citations::SOFTWARE_KEYWORDS,
        "related_identifiers": [{
            "identifier": paper.doi.unwrap_or_default(),
            "relation": "isSupplementTo",
            "scheme": "doi",
            "resource_type": "publication-article"
        }, {
            "identifier": citations::SOFTWARE_REPOSITORY,
            "relation": "isSupplementTo",
            "scheme": "url"
        }]
    });
    let mut s = serde_json::to_string_pretty(&value).expect("JSON values always serialise");
    s.push('\n');
    s
}
