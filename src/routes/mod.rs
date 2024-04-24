mod health_check;
mod subscriptions;

pub use health_check::*;
use std::error::Error;
use std::fmt::Formatter;
pub use subscriptions::*;

pub fn error_chain_fmt(e: &impl Error, f: &mut Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;

    let mut current = e.source();

    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;

        current = cause.source();
    }

    Ok(())
}
