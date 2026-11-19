FROM rust:1-bookworm AS build
RUN apt-get update -y && apt-get install -y --no-install-recommends \
    libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY config ./config
RUN cargo build --release --bin chiroo-server --bin chiroo-cli

FROM debian:bookworm-slim
RUN apt-get update -y && apt-get install -y --no-install-recommends \
    libssl3 libpq5 libgcc-s1 ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd -r -u 10001 chiroo
WORKDIR /app
COPY --from=build /app/target/release/chiroo-server /usr/local/bin/
COPY --from=build /app/target/release/chiroo-cli   /usr/local/bin/
COPY prompts ./prompts
COPY assets  ./assets
USER chiroo
EXPOSE 3000 3001
CMD ["chiroo-server"]
