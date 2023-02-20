FROM rust:1.67 AS base
WORKDIR /opt
RUN --mount=type=cache,target=/var/cache/apt \
    apt update && apt-get install -y python3.9-dev

FROM base AS builder
ADD . /opt
#ENV PYO3_CONFIG_FILE=/opt/docker/pyo3_config
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/opt/target \
    cargo build --release

FROM base AS runtime
RUN --mount=type=cache,target=/opt/target cp /opt/target/release/crab /opt/crab
ENTRYPOINT ["/opt/crab"]