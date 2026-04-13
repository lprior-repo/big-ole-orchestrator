use crate::cli::Cli;

pub use crate::middleware::{create_dispatcher, CommandContext, CommandDispatcher, Middleware};
pub use crate::registry::HandlerRegistry;

pub async fn dispatch(cli: Cli) -> Result<(), crate::cli::CliError> {
    let dispatcher = create_dispatcher();
    dispatcher.dispatch(cli).await
}
