# zero2prod-axum

## What is this project?

This is a personal practicing project trying to replicate
the [Zero To Production In Rust](https://www.zero2prod.com/index.html)
but with a different web framework [Axum](https://crates.io/crates/axum) and others.

## Main Differences

- Web framework: [Axum](https://github.com/tokio-rs/axum) instead
  of [Actix Web](https://github.com/actix/actix-web)
- SQL tool: [SeaORM](https://github.com/SeaQL/sea-orm) instead of pure [sqlx](https://github.com/launchbadge/sqlx)
- etc.

## Known Issues

- Chapter 04: Telemetry is not well implemented dues to lack of dedicated package to replace `tracing-actix-web` in
  Axum and rich logging features that Actix Web provides.
- Chapter 07: Postmark and other email API providers usually require a private domain (which I don't have one) to use,
  the confirmation email is not functional at live environment at this moment.
- Chapter 08: Error Log output looks slightly different from the origin, due to the different tracing implementation
  back in Chapter 04.
