use clap::Parser;
use trace_cli::{Cli, run};

/// `TraceErr` derives `miette::Diagnostic`, so returning a `miette::Result`
/// gets the error's code and help text rendered for free rather than a bare
/// `Debug` dump.
fn main() -> miette::Result<()> {
    run(Cli::parse())?;
    Ok(())
}
