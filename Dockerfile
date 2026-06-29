# Multi-stage build — no host Rust toolchain needed.
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY resources ./resources
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/nano-bank-modern-core /usr/local/bin/modern-core
ENV PORT=8091
EXPOSE 8091
CMD ["modern-core"]
