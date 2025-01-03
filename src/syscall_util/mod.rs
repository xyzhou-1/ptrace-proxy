#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86_64;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use self::x86_64::*;

mod common;
pub use self::common::*;
