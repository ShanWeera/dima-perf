//! The "How to cite" section of `README.md`, between `<!-- cite:start -->` and
//! `<!-- cite:end -->`, generated from the citation registry (D69).

use std::path::Path;

use dima_core::citations;

const START: &str = "<!-- cite:start -->";
const END: &str = "<!-- cite:end -->";

fn section() -> String {
    let paper = citations::dima_paper();
    format!(
        "{START}\n\
         If you use DiMA, please cite the paper:\n\n\
         > {}\n\n\
         and the version of the software you used:\n\n\
         > {}\n\n\
         `dima cite` prints both (`--format bibtex`, `ris` or `cff` for reference managers), and \
         `dima cite --from results.json` adds a methods paragraph with the references of the \
         methods that run used. [`CITATION.cff`](CITATION.cff) holds the same metadata for \
         GitHub's \"Cite this repository\".\n\n\
         <details>\n<summary>BibTeX</summary>\n\n```bibtex\n{}\n\n{}\n```\n\n</details>\n\
         {END}",
        paper.text(),
        citations::software_text(citations::SOFTWARE_VERSION),
        paper.bibtex().trim_end(),
        citations::software_bibtex(citations::SOFTWARE_VERSION).trim_end(),
    )
}

/// `README.md` with its citation section regenerated (unchanged text if the markers are
/// missing, so the drift check reports it).
pub fn with_citation(root: &Path) -> String {
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap_or_default();
    match (readme.find(START), readme.find(END)) {
        (Some(s), Some(e)) if s < e => {
            format!("{}{}{}", &readme[..s], section(), &readme[e + END.len()..])
        }
        _ => format!("{readme}\n{START} and {END} markers are missing\n"),
    }
}
