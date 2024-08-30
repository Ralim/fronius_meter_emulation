# Build Stage
FROM rust:1-alpine AS builder
WORKDIR /usr/src/
RUN rustup update
RUN rustup target add x86_64-unknown-linux-musl
RUN apk add --no-cache musl-dev
RUN USER=root cargo new fronius_meter_emulation
WORKDIR /usr/src/fronius_meter_emulation
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

COPY src ./src
RUN cargo install --target x86_64-unknown-linux-musl --path .

# Bundle Stage
FROM curlimages/curl
COPY --from=builder /usr/local/cargo/bin/fronius_meter_emulation .
USER 1000
EXPOSE 5502
EXPOSE 8000
CMD ["./fronius_meter_emulation"]
