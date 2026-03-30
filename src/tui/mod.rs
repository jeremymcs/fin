// Fin — Terminal User Interface (ratatui + crossterm)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

mod app;
mod tui_io;
mod widgets;

use crate::cli::Cli;

pub async fn run(args: Cli) -> anyhow::Result<()> {
    app::run_app(args).await
}
