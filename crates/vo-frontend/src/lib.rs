//! Frontend UI components for vo-engine.
//!
//! This crate provides the Dioxus-based web interface for the Veloxide workflow engine,
//! enabling workflow visualization, management, and monitoring through a web UI.
//!
//! # Purpose
//!
//! The frontend provides:
//! - Workflow visualization and DAG rendering
//! - Real-time execution monitoring
//! - Signal injection and event inspection
//! - Administrative dashboard functionality
//!
//! # Architecture
//!
//! Built with Dioxus 0.7 for reactive web UI. The frontend communicates
//! with the vo-api crate via HTTP/REST for workflow operations.

pub mod flow_extender;
pub mod hooks;
pub mod metrics;
pub mod ui;
