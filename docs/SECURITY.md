# Security policy

## Supported versions

Security fixes target the latest tip of the `dev` branch and tagged releases
published from this repository.

## Reporting a vulnerability

Do **not** open a public GitHub issue for an unfixed vulnerability.

Prefer a private [GitHub Security Advisory](https://github.com/Interchouette-ITC/serenade-rangular-mcp/security/advisories/new)
when available. Otherwise email [contact@interchouette.net](mailto:contact@interchouette.net)
with a description, impact and reproduction steps when possible.

We will acknowledge receipt and follow up. Do not expect a fixed SLA.

## Transport and dependencies

The HTTP transport is read-only and has no authentication; stdio project tools
can create files and run Cargo. See [README.md](README.md) for deployment
precautions. `make audit` and `make deny` check dependencies; they are not
vulnerability reporting channels.
