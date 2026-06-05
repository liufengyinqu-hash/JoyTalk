//! Infrared (IR) camera support for Joy-Con (R).
//!
//! Requires the `ir` Cargo feature and `joycon-sys` (protocol types from Yamakaky/joy, MIT).

mod session;

pub use session::{EnableProgress, IrSample, IrSession};
