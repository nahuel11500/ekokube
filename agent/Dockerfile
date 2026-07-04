FROM rust:1.89-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY agent/ agent/
COPY common/ common/
COPY server/ server/
RUN cargo build --release -p ekokube-agent

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /build/target/release/ekokube-agent /ekokube-agent
ENTRYPOINT ["/ekokube-agent"]
