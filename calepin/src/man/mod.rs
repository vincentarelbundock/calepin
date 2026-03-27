//! `calepin man` -- extract package documentation as .qmd files.
//!
//! Subcommands:
//!   - `calepin man r <package>` -- R package docs via Rd AST
//!   - `calepin man python <package>` -- Python package docs via ruff AST

pub mod r;
pub mod python;

pub use r::handle_man_r;
pub use python::handle_man_python;
