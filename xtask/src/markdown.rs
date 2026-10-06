//! Markdown helpers for generated documentation pages.

/// Escapes text for a CommonMark paragraph: characters that could start markup (emphasis,
/// links, inline HTML, tables, code) are backslash-escaped, so help texts render literally.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '<' | '>' | '[' | ']' | '|' | '~' | '{' | '}'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Percent-encodes the characters of a DOI that are not safe in a URL path.
fn encode_doi(doi: &str) -> String {
    let mut out = String::with_capacity(doi.len());
    for b in doi.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:;".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Escapes `text` and turns every `doi:…` and `https://…` into a link.
pub fn linkify(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text;
    while let Some(start) = [rest.find("doi:"), rest.find("https://")]
        .into_iter()
        .flatten()
        .min()
    {
        out.push_str(&escape(&rest[..start]));
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(tail.len());
        // Sentence punctuation after a link is not part of it.
        let token = tail[..end].trim_end_matches(['.', ',', ')']);
        if let Some(doi) = token.strip_prefix("doi:") {
            out.push_str(&format!(
                "[doi:{}](https://doi.org/{})",
                escape(doi),
                encode_doi(doi)
            ));
        } else {
            out.push_str(&format!("<{token}>"));
        }
        rest = &tail[token.len()..];
    }
    out.push_str(&escape(rest));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_links() {
        assert_eq!(escape("<name>.tsv | a_b"), "\\<name\\>.tsv \\| a\\_b");
        assert_eq!(
            linkify(
                "Shannon 1948, doi:10.1002/j.1538-7305.1948.tb01338.x; LANL, https://www.hiv.lanl.gov/x."
            ),
            "Shannon 1948, [doi:10.1002/j.1538-7305.1948.tb01338.x](https://doi.org/10.1002/j.1538-7305.1948.tb01338.x); LANL, <https://www.hiv.lanl.gov/x>."
        );
        assert_eq!(
            encode_doi("10.1002/(SICI)1<2>"),
            "10.1002/%28SICI%291%3C2%3E"
        );
    }
}
