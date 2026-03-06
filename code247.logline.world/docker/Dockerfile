FROM rust:1.82-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd -m -u 10001 appuser
WORKDIR /app
COPY --from=builder /app/target/release/dual-agents-rust /usr/local/bin/dual-agents-rust
RUN mkdir -p /app/data /app/evidence && chown -R appuser:appuser /app
USER appuser
ENV DB_PATH=/app/data/dual_agents.db \
    EVIDENCE_PATH=/app/evidence \
    REPO_ROOT=/app \
    GIT_BRANCH=main \
    HEALTH_PORT=4001 \
    RUST_LOG=info
EXPOSE 4001
CMD ["/usr/local/bin/dual-agents-rust"]
