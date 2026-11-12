# The Leos API.
#
# The build stage compiles the workspace and the runtime stage keeps only
# the binary. The migrations are embedded in the binary by the `sqlx`
# macro, so nothing else is needed at runtime apart from a certificate
# bundle for the database connection.

FROM rust:1.90-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --package leos-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/leos-server /usr/local/bin/leos-server

ENV LEOS_DATABASE_URL=postgres://leos:leos@postgres:5432/leos
ENV LEOS_BIND_ADDR=0.0.0.0:8080
ENV LEOS_DEV_MODE=true
ENV LEOS_SEED=true

EXPOSE 8080

CMD ["leos-server"]
