#![allow(
    unused_imports,
    unused_variables,
    clippy::collapsible_if,
    clippy::needless_borrow,
    clippy::let_and_return
)]
pub mod aim;
pub mod decision;
pub mod engine;
pub mod general;
pub mod movement;
pub mod rotation;
pub mod traits;
pub mod utility;
pub mod wall;

pub use engine::FeatureEngine;
pub use traits::FeatureExt;
