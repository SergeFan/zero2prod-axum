FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR /app
RUN apt update && apt install mold clang -y

FROM chef AS planner
COPY . .
# Compute a lock-like file for the project
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build the project dependencies
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin zero2prod-axum

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zero2prod-axum zero2prod-axum
COPY configuration configuration
ENV APP_ENVIRONMENT=prodcution
ENTRYPOINT ["./zero2prod-axum"]
