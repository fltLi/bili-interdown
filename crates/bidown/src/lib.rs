use std::io;

use thiserror::Error;

//////// module ////////

pub mod fetch;
pub mod model;
pub mod solve;
mod utils;

//////// error ////////

/// bidown 统合返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// bidown 统合错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Fetch(#[from] fetch::Error),

    #[error(transparent)]
    Solve(#[from] solve::Error),

    // other
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] io::Error),
}
