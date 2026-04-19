use crate::cli::Cli;

pub use crate::dispatch_v2::{
    create_dispatcher_v2, dispatch_v2, CommandDispatcherV2, DefaultDispatchContext,
    DispatchContext, LoggingMiddlewareV2, MetricsMiddlewareV2, MiddlewareResult, MiddlewareV2,
};
pub use crate::registry::HandlerRegistry;

pub async fn dispatch(cli: Cli) -> Result<(), crate::cli::CliError> {
    dispatch_v2(cli).await
}
