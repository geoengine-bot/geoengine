// configure default clippy lints
#![deny(clippy::correctness)]
#![warn(clippy::complexity, clippy::style, clippy::perf, clippy::pedantic)]
// disable some pedantic lints
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::non_ascii_literal,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::type_repetition_in_bounds,
    clippy::wildcard_imports
)]
// enable some restriction lints
#![warn(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]
//
//
// TODO: re-activate when https://github.com/rust-lang/rust-clippy/issues/7438 is fixed
#![allow(clippy::semicolon_if_nothing_returned)]

pub mod contexts;
pub mod datasets;
pub mod error;
pub mod handlers;
pub mod ogc;
pub mod projects;
pub mod server;
pub mod stac;
#[macro_use]
pub mod util;
pub mod workflows;

/// Compiles Geo Engine Pro
#[cfg(feature = "pro")]
pub mod pro;
