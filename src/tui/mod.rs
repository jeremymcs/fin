// Fin + Terminal User Interface (ratatui + crossterm)

mod app;
mod tui_io;
pub(crate) mod widgets;

use crate::cli::Cli;

pub async fn run(args: Cli) -> anyhow::Result<()> {
    app::run_app(args).await
}
