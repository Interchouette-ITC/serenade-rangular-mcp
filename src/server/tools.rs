use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{DocsServer, guidance, local, manifest};
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
    /// Keywords or a natural-language question.
    query: String,
    /// Optional product: serenade or rangular.
    product: Option<String>,
    /// Maximum results, from 1 to 50.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    /// Stable document ID returned by `list_docs` or `search_docs`.
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GuideArgs {
    /// The application or component workflow to build.
    task: guidance::CodingTask,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManifestArgs {
    /// Contents of a Cargo.toml, at most 64,000 bytes.
    manifest: String,
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

fn command_result(success: bool, value: &impl Serialize) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string(value)
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    Ok(if success {
        CallToolResult::success(vec![ContentBlock::text(text)])
    } else {
        CallToolResult::error(vec![ContentBlock::text(text)])
    })
}

fn snippet(doc: &Document, terms: &[String]) -> String {
    let source = corpus::text(doc);
    let line = source
        .lines()
        .find(|line| {
            let line = line.to_lowercase();
            terms.iter().any(|term| contains_term(&line, term))
        })
        .unwrap_or("");
    line.chars().take(240).collect()
}

fn search_variants(term: &str) -> &[&str] {
    match term {
        "website" | "websites" => &["website", "web", "front", "html", "spa", "ssg"],
        _ => &[],
    }
}

fn contains_term(text: &str, term: &str) -> bool {
    text.contains(term)
        || search_variants(term)
            .iter()
            .any(|variant| text.contains(variant))
}

#[tool_router(vis = "pub(super)")]
impl DocsServer {
    #[tool(
        description = "Create a new Serenade application using the installed serenade CLI in an existing absolute local directory. Does not overwrite an existing destination. Stdio only.",
        annotations(read_only_hint = false)
    )]
    async fn scaffold_serenade_app(
        &self,
        Parameters(args): Parameters<local::ScaffoldArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.local_actions || !self.profile.includes("serenade") {
            return Err(ErrorData::invalid_params(
                "local Serenade actions are disabled",
                None,
            ));
        }
        let (success, output) = local::scaffold(args).await?;
        command_result(success, &output)
    }

    #[tool(
        description = "Run Cargo check, build, or test in a supplied absolute local project directory. Cargo may execute project build scripts and tests. Stdio only.",
        annotations(read_only_hint = false)
    )]
    async fn run_cargo_task(
        &self,
        Parameters(args): Parameters<local::CargoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.local_actions {
            return Err(ErrorData::invalid_params(
                "local actions are disabled",
                None,
            ));
        }
        let (success, output) = local::cargo(args).await?;
        command_result(success, &output)
    }

    #[tool(
        description = "Get actionable, source-backed steps for creating a Serenade app, choosing a web mode, or building a Rangular component.",
        annotations(read_only_hint = true)
    )]
    async fn get_coding_guide(
        &self,
        Parameters(args): Parameters<GuideArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        result(&guidance::guide(args.task, self.profile)?)
    }

    #[tool(
        description = "Inspect supplied Cargo manifest text for Serenade and Rangular dependencies, features, aliases and target scopes. Does not read files or resolve the Cargo graph.",
        annotations(read_only_hint = true)
    )]
    async fn inspect_cargo_manifest(
        &self,
        Parameters(args): Parameters<ManifestArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        result(&manifest::inspect(&args.manifest, self.profile)?)
    }

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
        description = "Search embedded Serenade and Rangular guides and examples by keywords or natural-language question; retrieve full text with get_doc.",
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
        let all_terms: Vec<String> = args
            .query
            .split_whitespace()
            .map(|term| {
                term.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                    .to_lowercase()
            })
            .filter(|term| !term.is_empty())
            .collect();
        let stop_words = [
            "a", "an", "and", "can", "do", "for", "how", "i", "in", "is", "my", "of", "on",
            "please", "the", "to", "using", "what", "with",
        ];
        let mut terms: Vec<_> = all_terms
            .iter()
            .filter(|term| !stop_words.contains(&term.as_str()))
            .cloned()
            .collect();
        if terms.len() > 1 {
            terms.retain(|term| term != "serenade" && term != "rangular");
        }
        if terms.is_empty() {
            terms = all_terms;
        }
        terms.sort();
        terms.dedup();
        if terms.is_empty() {
            return Err(ErrorData::invalid_params(
                "query must contain searchable text",
                None,
            ));
        }
        let mut matches: Vec<_> = corpus::manifest()
            .documents
            .iter()
            .filter(|doc| self.profile.includes(&doc.product))
            .filter(|doc| product.is_none_or(|value| doc.product == value))
            .filter_map(|doc| {
                let title = doc.title.to_lowercase();
                let path = doc.path.to_lowercase();
                let text = corpus::text(doc).to_lowercase();
                let matching: Vec<_> = terms
                    .iter()
                    .filter(|term| {
                        contains_term(&title, term)
                            || contains_term(&path, term)
                            || contains_term(&text, term)
                    })
                    .collect();
                if matching.is_empty() {
                    return None;
                }
                let score: usize = matching.len() * 100
                    + matching
                        .iter()
                        .map(|term| {
                            let weight = if term.as_str() == "build" { 1 } else { 4 };
                            let score = if contains_term(&title, term) {
                                10
                            } else if contains_term(&path, term) {
                                8
                            } else if corpus::text(doc).lines().any(|line| {
                                line.starts_with('#') && contains_term(&line.to_lowercase(), term)
                            }) {
                                5
                            } else {
                                1
                            };
                            score * weight
                        })
                        .sum::<usize>();
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
        description = "Parse supplied Rangular component or head template text with the released crates.io parser. Development documentation may differ; this is not a full app build or AOT validation.",
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
            "version": { "version": rangular_parser::VERSION, "status": "crates.io release" },
            "documentation_version": corpus::manifest().products["rangular"],
            "scope": "released parser only; development documentation may differ; not a full application build"
        }))
    }
}
