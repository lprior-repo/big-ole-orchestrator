//! wtf-api - Axum HTTP API

pub mod app;
pub mod handlers;
pub mod health;
pub mod routes;
pub mod sse;
pub mod types;

#[cfg(test)]
mod tests {
    mod unit {
        include!("../tests/unit/signal_handler_test.rs");
    }
    mod unit_terminate {
        use super::super::*;
        include!("../tests/unit/terminate_handler_test.rs");
    }
}
