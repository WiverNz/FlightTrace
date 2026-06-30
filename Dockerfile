FROM rust:1-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples

RUN cargo build --locked --release --package flighttrace-cli

FROM debian:bookworm-slim AS runtime

RUN groupadd --system --gid 10001 flighttrace \
    && useradd --system --uid 10001 --gid flighttrace --no-create-home flighttrace \
    && mkdir /data \
    && chown flighttrace:flighttrace /data

COPY --from=builder /app/target/release/flighttrace /usr/local/bin/flighttrace

USER flighttrace

VOLUME ["/data"]
EXPOSE 8080

ENTRYPOINT ["flighttrace"]
CMD ["serve", "--bind", "0.0.0.0:8080", "--database", "/data/flighttrace.db"]
