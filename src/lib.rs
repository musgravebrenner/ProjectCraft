//! PlanCraft: a project scheduling app driven by one command registry.
//!
//! The GUI, the command line, the JSON control channel and the MCP server all go through
//! [`commands::Registry`], so anything that can be clicked can also be scripted.

pub mod calendar;
pub mod cli;
pub mod commands;
pub mod control;
pub mod fields;
pub mod gui;
pub mod io;
pub mod mcp;
pub mod model;
pub mod mpp;
pub mod prefs;
pub mod print;
pub mod recent;
pub mod reports;
pub mod sample;
pub mod schedule;
pub mod session;
pub mod templates;

pub const APP_NAME: &str = "PlanCraft";
pub const DEFAULT_PORT: u16 = 47811;
