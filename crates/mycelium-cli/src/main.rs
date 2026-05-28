//! `mycelium` — the command-line entry point.
//!
//! Subcommands are intentionally limited at v0.0; the Hive will flesh
//! them out per RFCs.

use anyhow::Result;
use clap::{Parser, Subcommand};

/// The `mycelium` CLI. See `mycelium --help` for details.
#[derive(Debug, Parser)]
#[command(
    name = "mycelium",
    version,
    about = "Reactive code intelligence graph — the wood-wide-web of your codebase.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
enum Cmd {
    /// Print the engine version.
    Version,
    /// Placeholder for `mycelium init` (creates a `.mycelium/` config dir).
    Init,
    /// Placeholder for `mycelium index` (full re-index of a project).
    Index,
    /// Placeholder for `mycelium query <hyphae>`.
    Query {
        /// The Hyphae expression. Syntax is RFC-0003 (forthcoming).
        expr: String,
    },
    /// Start the MCP server over stdio.
    Serve {
        /// Use MCP protocol over stdio.
        #[arg(long)]
        mcp: bool,
    },
}

// Subcommand implementations will use `?` once they land (RFC-0001 follow-up).
#[allow(clippy::unnecessary_wraps)]
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mycelium=info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Cmd::Version => {
            println!("mycelium {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Cmd::Init => {
            // Placeholder. Real implementation arrives with the Store API.
            tracing::warn!(
                "`mycelium init` is not implemented yet — tracked under RFC-0001 follow-up"
            );
            Ok(())
        }
        Cmd::Index => {
            tracing::warn!("`mycelium index` is not implemented yet");
            Ok(())
        }
        Cmd::Query { expr } => {
            tracing::warn!(
                "`mycelium query` is not implemented yet (query={expr:?}) — see RFC-0003"
            );
            Ok(())
        }
        Cmd::Serve { mcp } => {
            tracing::warn!("`mycelium serve --mcp={mcp}` is not implemented yet");
            Ok(())
        }
    }
}
