#![allow(clippy::redundant_pattern_matching)]

pub mod config;
pub mod dispatch;
pub mod doctor;
pub mod e2e_pipeline;
pub mod errors;
pub mod exit_codes;
pub mod helpers;
pub mod history;
pub mod misc;
pub mod output;
pub mod parsing;
pub mod registry;
pub mod utils;

pub use helpers::{create_elf_binary, create_workflow_binary, setup_project};
