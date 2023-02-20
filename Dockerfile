FROM rust:1.67 AS base
WORKDIR /opt
RUN --mount=type=cache,target=/var/cache/apt \
    apt update && apt-get install -y python3.9-dev

FROM base AS builder
ADD . /opt
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/opt/target \
    cargo install --path=/opt --locked --root=/opt

FROM base AS runtime
COPY --from=builder /opt/bin/crab /opt/crab
ENTRYPOINT ["/opt/crab"]