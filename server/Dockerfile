FROM node:22-slim AS ui
WORKDIR /build
COPY ui/package.json ui/package-lock.json ./
# The lockfile is generated with npm 11; node:22's bundled npm 10 disagrees
# about optional-dependency entries.
RUN npm install -g npm@11 && npm ci
COPY ui/ ./
RUN npm run build

FROM rust:1.89-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY agent/ agent/
COPY common/ common/
COPY server/ server/
RUN cargo build --release -p ekokube-server

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /build/target/release/ekokube-server /ekokube-server
COPY --from=ui /build/dist /static
ENV EKOKUBE_STATIC_DIR=/static
ENTRYPOINT ["/ekokube-server"]
