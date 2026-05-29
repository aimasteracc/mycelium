//! `mycelium` — the command-line entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::runtime::Runtime;

mod index;
#[allow(
    clippy::redundant_pub_crate,
    reason = "items used by main.rs require pub(crate); bin-crate root cannot consume private child-mod items"
)]
mod query;

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

/// Output format for `mycelium query`. Stable values; the MCP twin tool
/// `mycelium_query` accepts the same set.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum QueryFormat {
    Text,
    Json,
}

impl From<QueryFormat> for query::Format {
    fn from(f: QueryFormat) -> Self {
        match f {
            QueryFormat::Text => Self::Text,
            QueryFormat::Json => Self::Json,
        }
    }
}

/// Subcommands.
#[derive(Debug, Subcommand)]
enum Cmd {
    /// Print the engine version.
    Version,
    /// Placeholder for `mycelium init` (creates a `.mycelium/` config dir).
    /// Hidden until implemented — see issue #154.
    #[command(hide = true)]
    Init,
    /// Index a project directory and report symbol statistics.
    Index {
        /// Root directory to index (defaults to current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Execute a Hyphae DSL selector against the project's index.
    Query {
        /// The Hyphae expression. See RFC-0003 for the full grammar.
        ///
        /// Examples:
        ///   `#login`          match symbols named `login`
        ///   `.function`       match all function symbols
        ///   `.class>.method`  methods of classes (direct child)
        expr: String,

        /// Project root (defaults to current directory). The index is read
        /// from `<root>/.mycelium/index.rmp`.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Output format. `text` writes one match per line. `json` writes a
        /// JSON array of strings — the stable contract used by the MCP twin
        /// tool `mycelium_query`.
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Start the MCP server over stdio.
    Serve {
        /// Use MCP protocol over stdio.
        #[arg(long)]
        mcp: bool,
        /// Pre-load (or build) the symbol index from this root directory.
        ///
        /// Loads `.mycelium/index.rmp` if present; otherwise runs a full index
        /// and saves the snapshot before the server accepts connections.
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    // Route all tracing to stderr (never stdout). For `serve --mcp` this is
    // mandatory: stdout is reserved for JSON-RPC frames. For other subcommands
    // it's harmless. ANSI is disabled so piped consumers (CI logs, MCP clients
    // that surface stderr) don't see escape sequences.
    // Regression test: tests/mcp_stdout_purity.rs (issue #150).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mycelium=info".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Cmd::Version => {
            println!("mycelium {}", env!("CARGO_PKG_VERSION"));
        }
        Cmd::Init => {
            tracing::warn!(
                "`mycelium init` is not implemented yet — tracked under RFC-0001 follow-up"
            );
        }
        Cmd::Index { path } => {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            println!("Indexing {} …", canonical.display());
            let (store, stats) = index::index_path(&canonical)?;
            println!(
                "Done.  {} file(s) indexed, {} error(s).",
                stats.files, stats.errors
            );
            // RFC-0006: auto-save to .mycelium/index.rmp
            let snap = canonical.join(".mycelium").join("index.rmp");
            store.save(&snap)?;
            println!("Index saved to .mycelium/index.rmp");
        }
        Cmd::Query { expr, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            query::run(&canonical, &expr, format.into())?;
        }
        Cmd::Serve { mcp: true, root } => {
            let root = root.map(|p| p.canonicalize().unwrap_or(p));
            let rt = Runtime::new()?;
            rt.block_on(mycelium_mcp::serve_stdio(root))?;
        }
        Cmd::Serve { mcp: false, .. } => {
            tracing::warn!("`mycelium serve` requires `--mcp` flag (other transports are v0.2)");
        }
    }
    Ok(())
}
