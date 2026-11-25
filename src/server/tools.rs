use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::DocsServer;
use crate::corpus::{self, Document};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArgs {
    /// Optional product: serenade or rangular.
    product: Option<String>,
    /// Filter by source directory, such as docs-dev or docs.
    group: Option<String>,
    /// Zero-based result offset.
    cursor: Option<usize>,
    /// Maximum results, from 1 to 50.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// Search terms; all terms must occur.
    query: String,
    /// Optional product: serenade or rangular.
    product: Option<String>,
    /// Maximum results, from 1 to 50.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    /// Stable document ID returned by list_docs or search_docs.
    id: String,
    /// Heading ID when reading a section of a document.
    section: Option<String>,
    /// Byte offset within the selected text for paginating oversized content.
    offset: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateArgs {
    /// Rangular template text, at most 64,000 bytes.
    source: String,
    /// Template kind: component (default) or head.
    kind: Option<String>,
}

fn limit(value: Option<usize>) -> Result<usize, ErrorData> {
    let value = value.unwrap_or(20);
    if !(1..=50).contains(&value) {
        return Err(ErrorData::invalid_params(
            "limit must be between 1 and 50",
            None,
        ));
    }
    Ok(value)
}

fn result(value: &impl Serialize) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string(value)
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn snippet(doc: &Document, terms: &[String]) -> String {
    let source = corpus::text(doc);
    let line = source
        .lines()
        .find(|line| terms.iter().any(|term| line.to_lowercase().contains(term)))
        .unwrap_or("");
    line.chars().take(240).collect()
}

#[tool_router(vis = "pub(super)")]
impl DocsServer {
    #[tool(
        description = "List versioned Serenade or Rangular documentation and examples. Use the returned ID with get_doc.",
        annotations(read_only_hint = true)
    )]
    async fn list_docs(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let product = self.profile.product(args.product.as_deref())?;
        let limit = limit(args.limit)?;
        let cursor = args.cursor.unwrap_or(0);
        let docs: Vec<_> = corpus::manifest()
            .documents
            .iter()
            .filter(|doc| self.profile.includes(&doc.product))
            .filter(|doc| product.is_none_or(|value| doc.product == value))
            .filter(|doc| {
                args.group
                    .as_deref()
                    .is_none_or(|group| corpus::entry(doc).group == group)
            })
            .collect();
        if cursor > docs.len() {
            return Err(ErrorData::invalid_params(
                "cursor exceeds result count",
                None,
            ));
        }
        let end = cursor.saturating_add(limit).min(docs.len());
        let items: Vec<_> = docs[cursor..end]
            .iter()
            .map(|doc| corpus::entry(doc))
            .collect();
        result(&serde_json::json!({
            "items": items,
            "total": docs.len(),
            "next_cursor": (end < docs.len()).then_some(end)
        }))
    }

    #[tool(
        description = "Search embedded Serenade and Rangular guides and examples. All search terms must match; retrieve full text with get_doc.",
        annotations(read_only_hint = true)
    )]
    async fn search_docs(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let product = self.profile.product(args.product.as_deref())?;
        let limit = limit(args.limit)?;
        if args.query.trim().is_empty() || args.query.len() > 512 {
            return Err(ErrorData::invalid_params(
                "query must contain 1 to 512 bytes",
                None,
            ));
        }
        let terms: Vec<String> = args
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut matches: Vec<_> = corpus::manifest()
            .documents
            .iter()
            .filter(|doc| self.profile.includes(&doc.product))
            .filter(|doc| product.is_none_or(|value| doc.product == value))
            .filter_map(|doc| {
                let title = doc.title.to_lowercase();
                let text = corpus::text(doc).to_lowercase();
                if !terms
                    .iter()
                    .all(|term| title.contains(term) || text.contains(term))
                {
                    return None;
                }
                let score: usize = terms
                    .iter()
                    .map(|term| {
                        if title.contains(term) {
                            10
                        } else if corpus::text(doc)
                            .lines()
                            .any(|line| line.starts_with('#') && line.to_lowercase().contains(term))
                        {
                            5
                        } else {
                            1
                        }
                    })
                    .sum();
                Some((score, doc))
            })
            .collect();
        matches.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.id.cmp(&right.1.id)));
        let hits: Vec<_> = matches.into_iter().take(limit).map(|(score, doc)| {
            serde_json::json!({"document": corpus::entry(doc), "score": score, "snippet": snippet(doc, &terms)})
        }).collect();
        result(&serde_json::json!({"items": hits}))
    }

    #[tool(
        description = "Read embedded source documentation by ID, optionally by heading ID. Large documents return section IDs; use offset 0 and next_offset to page through oversized text.",
        annotations(read_only_hint = true)
    )]
    async fn get_doc(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let doc = corpus::find(&args.id)
            .filter(|doc| self.profile.includes(&doc.product))
            .ok_or_else(|| ErrorData::invalid_params("unknown or disabled document", None))?;
        let content = corpus::content(doc, args.section.as_deref(), args.offset)
            .map_err(|error| ErrorData::invalid_params(error, None))?;
        result(&content)
    }

    #[tool(
        description = "Parse supplied Rangular component or head template text and report RANG diagnostics. This is not a full app build or AOT validation.",
        annotations(read_only_hint = true)
    )]
    async fn validate_rangular_template(
        &self,
        Parameters(args): Parameters<ValidateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.source.len() > 64_000 {
            return Err(ErrorData::invalid_params(
                "template exceeds 64,000 bytes",
                None,
            ));
        }
        let parsed = match args.kind.as_deref().unwrap_or("component") {
            "component" => rangular_parser::parse(&args.source, "template.html"),
            "head" => rangular_parser::parse_head_template(&args.source, "template.html"),
            _ => {
                return Err(ErrorData::invalid_params(
                    "kind must be component or head",
                    None,
                ));
            }
        };
        let diagnostics: Vec<_> = parsed
            .diagnostics
            .iter()
            .take(100)
            .map(|diagnostic| {
                let (line, column) = rangular_parser::line_col(&args.source, diagnostic.span.start);
                serde_json::json!({
                    "code": diagnostic.code,
                    "severity": if diagnostic.is_error() { "error" } else { "warning" },
                    "message": diagnostic.message,
                    "line": line,
                    "column": column,
                    "start": diagnostic.span.start,
                    "end": diagnostic.span.end
                })
            })
            .collect();
        result(&serde_json::json!({
            "valid": parsed.ok(),
            "diagnostics": diagnostics,
            "diagnostic_count": parsed.diagnostics.len(),
            "diagnostics_truncated": parsed.diagnostics.len() > diagnostics.len(),
            "version": corpus::manifest().products["rangular"],
            "scope": "parser only; not a full application build"
        }))
    }
}
