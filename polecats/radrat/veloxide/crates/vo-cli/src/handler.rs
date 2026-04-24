use std::future::Future;
use std::pin::Pin;

use crate::cli::{Cli, CliError};

pub trait CommandHandler: Send + Sync {
    fn name(&self) -> &'static str;

    fn execute(&self, cli: &Cli)
        -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>>;
}
