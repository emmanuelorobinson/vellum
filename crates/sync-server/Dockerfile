# syntax=docker/dockerfile:1
FROM rust:1.99.0-bookworm@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6 AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release -p vellum-sync-server && \
    cp target/release/vellum-sync-server /usr/local/bin/vellum-sync-server

FROM debian:12@sha256:f37a335e82bca302e955fa39f9dfe28f1be618f016f8a2b56318e5a5111afc26 AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 vellum && useradd --uid 10001 --gid 10001 --no-create-home vellum \
    && mkdir /data && chown 10001:10001 /data && chmod 700 /data
COPY --from=build /usr/local/bin/vellum-sync-server /usr/local/bin/
USER 10001:10001
VOLUME ["/data"]
EXPOSE 7310
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 CMD ["curl", "--fail", "--silent", "http://127.0.0.1:7310/health"]
ENTRYPOINT ["/usr/local/bin/vellum-sync-server", "--data", "/data"]
CMD ["serve", "--listen", "0.0.0.0:7310", "--instance", "self-host"]
