//! MCP tools and resources.

mod guidance;
mod local;
mod manifest;
mod tools;

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{
        Implementation, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
        ServerConfig,
    },
    service::RequestContext,
    tool_handler,
};

use crate::corpus;

#[derive(Clone, Copy, Debug, clap::ValueEnum, PartialEq, Eq)]
pub enum Profile {
    Serenade,
    Rangular,
    All,
}

impl Profile {
    fn includes(self, product: &str) -> bool {
        matches!(
            (self, product),
            (Self::All, "serenade" | "rangular")
                | (Self::Serenade, "serenade")
                | (Self::Rangular, "rangular")
        )
    }

    fn product(self, product: Option<&str>) -> Result<Option<&str>, ErrorData> {
        if let Some(product) = product
            && !self.includes(product)
        {
            return Err(ErrorData::invalid_params(
                format!("product {product:?} is unknown or disabled by the profile"),
                None,
            ));
        }
        Ok(product)
    }
}

#[derive(Clone)]
pub struct DocsServer {
    pub(crate) profile: Profile,
    pub(crate) tool_router: ToolRouter<Self>,
    pub(crate) local_actions: bool,
}

impl DocsServer {
    pub fn new(profile: Profile) -> Self {
        Self::with_local_actions(profile, false)
    }

    pub fn with_local_actions(profile: Profile, local_actions: bool) -> Self {
        let mut tool_router = Self::tool_router();
        if profile == Profile::Serenade {
            tool_router.remove_route("validate_rangular_template");
        }
        if !local_actions {
            tool_router.remove_route("scaffold_serenade_app");
            tool_router.remove_route("run_cargo_task");
        } else if profile == Profile::Rangular {
            tool_router.remove_route("scaffold_serenade_app");
        }
        Self {
            profile,
            tool_router,
            local_actions,
        }
    }

    pub(crate) fn read_only(self) -> Self {
        Self::new(self.profile)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DocsServer {
    fn get_info(&self) -> ServerConfig {
        let instructions = match self.profile {
            Profile::Serenade => {
                "For a standalone app, install serenade-cli from crates.io and use serenade new; \
                 read serenade/guides/first-steps/installation.md and serenade/docs-dev/RECIPES.md."
            }
            Profile::Rangular => {
                "Read rangular/docs/README.md for published rangular and rangular-build dependencies. \
                 Template validation uses the released parser, which may differ from development \
                 documentation; it is not an application build."
            }
            Profile::All => {
                "For a standalone app, install serenade-cli from crates.io and use serenade new; \
                 read serenade/guides/first-steps/installation.md and serenade/docs-dev/RECIPES.md. \
                 Read rangular/docs/README.md for published rangular and rangular-build dependencies. \
                 Template validation uses the released parser, which may differ from development \
                 documentation; it is not an application build."
            }
        };
        let actions = match (self.local_actions, self.profile) {
            (true, Profile::Rangular) => {
                "Local stdio tools can run Cargo check, build or test in a supplied directory."
            }
            (true, _) => {
                "Local stdio tools can scaffold a Serenade application and run Cargo check, build or test in a supplied directory."
            }
            (false, _) => "This server does not read client files or run commands.",
        };
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(
            "serenade-rangular-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(format!(
            "Search and read the embedded, revision-labelled documentation. \
             Code examples refer to the documented source revisions; repository examples may \
             require their original workspaces. Use get_coding_guide for concrete app and \
             component workflows, and inspect_cargo_manifest for supplied Cargo manifest text. \
             {actions} {instructions}"
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources = corpus::manifest()
            .documents
            .iter()
            .filter(|doc| self.profile.includes(&doc.product))
            .map(|doc| {
                Resource::new(format!("docs://{}", doc.id), doc.id.clone())
                    .with_title(doc.title.clone())
                    .with_description(format!(
                        "{} at {}",
                        doc.product,
                        corpus::manifest().products[&doc.product].revision
                    ))
                    .with_mime_type(if doc.path.ends_with(".md") {
                        "text/markdown"
                    } else {
                        "text/plain"
                    })
            })
            .collect();
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let id = request
            .uri
            .strip_prefix("docs://")
            .ok_or_else(|| ErrorData::invalid_params("invalid resource URI", None))?;
        let doc = corpus::find(id)
            .filter(|doc| self.profile.includes(&doc.product))
            .ok_or_else(|| ErrorData::resource_not_found("unknown resource", None))?;
        let body = corpus::content(doc, None, None)
            .map_err(|message| ErrorData::invalid_params(message, None))?;
        let text = body.markdown.ok_or_else(|| {
            ErrorData::invalid_params("resource is too large; use get_doc with a section", None)
        })?;
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type(
                if doc.path.ends_with(".md") {
                    "text/markdown"
                } else {
                    "text/plain"
                },
            ),
        ])
        .into())
    }
}
