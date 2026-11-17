use anyhow::Result;
use clap::Parser;
use laburo::cli::{Cli, run};

fn main() -> Result<()> {
    run(Cli::parse())
}
