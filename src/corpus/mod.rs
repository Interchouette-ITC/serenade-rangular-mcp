//! Embedded documentation, provenance, search, and section navigation.

use std::{collections::BTreeMap, sync::OnceLock};

use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};

static DOCUMENTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/corpus/documents");
static MANIFEST: OnceLock<Manifest> = OnceLock::new();
const MAX_CONTENT: usize = 60_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Product {
    pub version: String,
    pub revision: String,
    pub status: String,
    pub repository: String,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Document {
    pub id: String,
    pub product: String,
    pub path: String,
    pub title: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub products: BTreeMap<String, Product>,
    pub documents: Vec<Document>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Entry<'a> {
    #[serde(flatten)]
    pub document: &'a Document,
    pub group: &'a str,
    pub version: &'a Product,
    pub source_url: String,
}

#[derive(Debug, Serialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    #[serde(skip)]
    start: usize,
    #[serde(skip)]
    level: usize,
}

#[derive(Debug, Serialize)]
pub struct DocumentContent<'a> {
    pub document: Entry<'a>,
    pub markdown: Option<&'a str>,
    pub sections: Vec<Section>,
    pub oversized: bool,
    pub next_offset: Option<usize>,
}

pub fn manifest() -> &'static Manifest {
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../../corpus/manifest.json"))
            .expect("validated embedded manifest")
    })
}

pub fn text(document: &Document) -> &'static str {
    DOCUMENTS
        .get_file(&document.id)
        .and_then(|file| file.contents_utf8())
        .expect("validated embedded document")
}

pub fn entry(document: &Document) -> Entry<'_> {
    let version = &manifest().products[&document.product];
    Entry {
        document,
        group: document.path.split('/').next().unwrap_or(""),
        version,
        source_url: format!(
            "{}/blob/{}/{}",
            version.repository, version.revision, document.path
        ),
    }
}

pub fn find(id: &str) -> Option<&'static Document> {
    manifest()
        .documents
        .iter()
        .find(|document| document.id == id)
}

fn slug(title: &str) -> String {
    let mut result = String::new();
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || character == '_' || character == '-' {
            result.push(character);
        } else if character.is_whitespace() && !result.ends_with('-') {
            result.push('-');
        }
    }
    result.trim_matches('-').to_owned()
}

fn sections(source: &str) -> Vec<Section> {
    let mut headings = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    let mut offset = 0;
    let mut fence: Option<char> = None;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = trimmed.chars().next().expect("fence marker");
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
        } else if fence.is_none() {
            let level = line.bytes().take_while(|byte| *byte == b'#').count();
            if (1..=6).contains(&level) && line.as_bytes().get(level) == Some(&b' ') {
                let title = line[level..].trim().trim_end_matches('#').trim();
                let base = slug(title);
                let count = counts.entry(base.clone()).or_default();
                let id = if *count == 0 {
                    base
                } else {
                    format!("{base}-{count}")
                };
                *count += 1;
                headings.push(Section {
                    id,
                    title: title.to_owned(),
                    start: offset,
                    level,
                });
            }
        }
        offset += line.len();
    }
    headings
}

pub fn content<'a>(
    document: &'a Document,
    section_id: Option<&str>,
    offset: Option<usize>,
) -> Result<DocumentContent<'a>, String> {
    let source = text(document);
    let headings = sections(source);
    let (start, end) = if let Some(id) = section_id {
        let index = headings
            .iter()
            .position(|heading| heading.id == id)
            .ok_or_else(|| format!("unknown section: {id}"))?;
        let heading = &headings[index];
        let end = headings[index + 1..]
            .iter()
            .find(|other| other.level <= heading.level)
            .map_or(source.len(), |other| other.start);
        (heading.start, end)
    } else {
        (0, source.len())
    };
    let body = &source[start..end];
    let oversized = body.len() > MAX_CONTENT;
    let (markdown, next_offset) = chunk(body, offset)?;
    let contained = headings
        .into_iter()
        .filter(|heading| heading.start >= start && heading.start < end)
        .collect();
    Ok(DocumentContent {
        document: entry(document),
        markdown,
        sections: contained,
        oversized,
        next_offset,
    })
}

fn chunk(body: &str, offset: Option<usize>) -> Result<(Option<&str>, Option<usize>), String> {
    Ok(if let Some(offset) = offset {
        if offset > body.len() || !body.is_char_boundary(offset) {
            return Err("offset is outside the selected text or not a UTF-8 boundary".to_owned());
        }
        let mut end = offset.saturating_add(MAX_CONTENT).min(body.len());
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        (Some(&body[offset..end]), (end < body.len()).then_some(end))
    } else {
        ((body.len() <= MAX_CONTENT).then_some(body), None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_ignore_code_fences_and_disambiguate_duplicates() {
        let source = "# First\n```\n# fake\n```\n## First\n## First\n";
        let headings = sections(source);
        assert_eq!(
            headings.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
            ["first", "first-1", "first-2"]
        );
    }

    #[test]
    fn unknown_section_is_an_error() {
        let doc = find("rangular/docs/SPEC.md").expect("specification");
        assert!(content(doc, Some("not-a-real-section"), None).is_err());
    }

    #[test]
    fn example_manifests_with_workspace_dependencies_are_not_embedded() {
        assert!(manifest().documents.iter().all(|doc| {
            !(doc.path.starts_with("examples/") || doc.path.starts_with("demo-"))
                || !doc.path.ends_with("/Cargo.toml")
        }));
    }

    #[test]
    fn oversized_text_pages_without_splitting_utf8() {
        let body = format!("{}éend", "a".repeat(MAX_CONTENT - 1));
        assert_eq!(chunk(&body, None).unwrap(), (None, None));
        let (first, next) = chunk(&body, Some(0)).unwrap();
        let next = next.unwrap();
        assert_eq!(next, MAX_CONTENT - 1);
        let (last, end) = chunk(&body, Some(next)).unwrap();
        assert_eq!(format!("{}{}", first.unwrap(), last.unwrap()), body);
        assert_eq!(end, None);
        assert!(chunk(&body, Some(next + 1)).is_err());
        assert!(chunk(&body, Some(body.len() + 1)).is_err());
    }
}
