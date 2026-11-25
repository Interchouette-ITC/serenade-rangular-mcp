use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Profile;
use crate::corpus;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodingTask {
    CreateApp,
    Html,
    Spa,
    Ssg,
    Universal,
    RangularComponent,
    RangularSerenade,
}

pub fn guide(task: CodingTask, profile: Profile) -> Result<Value, ErrorData> {
    let (products, summary, steps, sources): (&[&str], &str, &[&str], &[&str]) = match task {
        CodingTask::CreateApp => (
            &["serenade"],
            "Create a standalone Serenade bundle application using published crates.",
            &[
                "Install the CLI: cargo install serenade-cli --locked",
                "Run serenade new <name>; enter the created application directory and run cargo test and cargo run.",
                "Use serenade recipe list and serenade recipe apply <id> to add available recipes. A scaffold is not automatically a web server.",
                "For a web application choose the html, spa, ssg or universal guide; adapt the example to your application.",
            ],
            &[
                "serenade/docs-dev/RECIPES.md",
                "serenade/guides/first-steps/installation.md",
            ],
        ),
        CodingTask::Html => (
            &["serenade"],
            "Build a server-rendered HTML application.",
            &[
                "In the Serenade source checkout, run make html to study the HTML example at http://127.0.0.1:8080/.",
                "Study demo-html/src/app.rs and the forms and routing code. For a new application use published crates and adapt the example; it is a workspace demo, not a standalone template.",
                "Validate with the relevant Cargo tests and exercise the application's routes and forms.",
            ],
            &[
                "serenade/docs-dev/FRONT-MODES.md",
                "serenade/examples/demo-html/README.md",
                "serenade/examples/demo-html/src/app.rs",
                "serenade/docs-dev/FORMS.md",
            ],
        ),
        CodingTask::Spa => (
            &["serenade"],
            "Build a client-rendered Leptos SPA using Serenade's managed Trunk.",
            &[
                "Install the wasm target: rustup target add wasm32-unknown-unknown.",
                "In the Serenade source checkout run make spa; the demo and its shared UI are under examples/demo-monorepo.",
                "Adapt the SPA and shared UI to your application. Serenade downloads and verifies its managed Trunk on first frontend build; do not assume a system Trunk is the same fork.",
                "Run your application's build and tests and verify its generated browser assets.",
            ],
            &[
                "serenade/docs-dev/FRONT-MODES.md",
                "serenade/examples/demo-monorepo/README.md",
                "serenade/examples/demo-monorepo/apps/demo-spa/src/main.rs",
                "serenade/examples/demo-monorepo/libs/demo-ui/src/lib.rs",
                "serenade/guides/first-steps/installation.md",
            ],
        ),
        CodingTask::Ssg => (
            &["serenade"],
            "Prerender static routes and hydrate them in the browser.",
            &[
                "Install the wasm target: rustup target add wasm32-unknown-unknown.",
                "In the Serenade source checkout run make ssg; inspect the prerenderer and shared UI in examples/demo-monorepo.",
                "Define the application's routes and head data, use the Trunk asset manifest and static document writer, then build the frontend.",
                "Verify each generated route and browser hydration; SSG prerenders at build time, unlike request-time SSR.",
            ],
            &[
                "serenade/docs-dev/FRONT-MODES.md",
                "serenade/examples/demo-monorepo/README.md",
                "serenade/examples/demo-monorepo/apps/demo-ssg/src/bin/prerender.rs",
                "serenade/docs-dev/VIEW.md",
            ],
        ),
        CodingTask::Universal => (
            &["serenade"],
            "Build a frontend served by a Serenade backend, with an SSR and hydration example.",
            &[
                "Install the wasm target: rustup target add wasm32-unknown-unknown.",
                "In the Serenade source checkout run make universal; inspect / and /cms/hello at http://127.0.0.1:8080/.",
                "Study the frontend, backend, shared contract and Rangular templates together; the example is a workspace, not a standalone package.",
                "Build and test both sides; check the Trunk asset manifest, server-rendered HTML and browser hydration.",
            ],
            &[
                "serenade/examples/demo-universal/README.md",
                "serenade/examples/demo-universal/apps/demo-front/src/main.rs",
                "serenade/examples/demo-universal/apps/demo-back/src/main.rs",
                "serenade/examples/demo-universal/libs/demo-contract/src/lib.rs",
                "serenade/docs-dev/VIEW.md",
            ],
        ),
        CodingTask::RangularComponent => (
            &["rangular"],
            "Add a Rangular AOT component to a Leptos application.",
            &[
                "Use the published rangular facade as a dependency and rangular-build as a build dependency; choose the appropriate Leptos CSR, SSR or hydration features.",
                "Place a component's HTML, optional SCSS and Rust Host under src/components/<name>/; map template values and events through the Host implementation.",
                "In build.rs call rangular_build::build(rangular_build::Config::new(\"src/components\", \"style/components.css\")); include the generated OUT_DIR/rangular/<name>_view.rs and render the view using HostCell.",
                "Link the generated stylesheet, build the application and test the component. Use validate_rangular_template for parser diagnostics, not as a substitute for the build.",
            ],
            &[
                "rangular/crates/rangular/README.md",
                "rangular/crates/rangular-build/README.md",
                "rangular/docs/SPEC.md",
                "rangular/demo-leptos/src/components/item_list/item_list.html",
                "rangular/demo-leptos/src/components/item_list/item_list.rs",
            ],
        ),
        CodingTask::RangularSerenade => (
            &["serenade", "rangular"],
            "Use Rangular templates with Serenade complete documents and Leptos.",
            &[
                "Use published rangular and rangular-build crates and select serenade-view's leptos-ssr or leptos-client features as needed.",
                "Register application-owned head and body templates with rangular_build::Config, then include the generated views.",
                "Render with Serenade Document and the Trunk asset manifest; use ManagedHead for client head lifecycle where needed.",
                "Use the Universal demo as a source-backed example, then test server output and browser hydration in your own application.",
            ],
            &[
                "serenade/docs-dev/VIEW.md",
                "serenade/examples/demo-universal/README.md",
                "serenade/examples/demo-universal/build_support/cms_hello.rs",
                "rangular/crates/rangular-build/README.md",
                "rangular/docs/SPEC.md",
            ],
        ),
    };
    for product in products {
        if !profile.includes(product) {
            return Err(ErrorData::invalid_params(
                format!("task requires the {product} profile"),
                None,
            ));
        }
    }
    let documents: Vec<_> = sources
        .iter()
        .map(|id| {
            corpus::find(id).map(corpus::entry).ok_or_else(|| {
                ErrorData::internal_error(format!("coding guide source missing: {id}"), None)
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "summary": summary,
        "steps": steps,
        "sources": documents,
        "scope": "Guidance only; run commands yourself or use the local stdio project tools. Development docs may describe unreleased changes."
    }))
}
