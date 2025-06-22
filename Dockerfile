# Build Stage
FROM rust:1-alpine AS builder
WORKDIR /usr/src/
RUN rustup update
RUN apk add --no-cache musl-dev
RUN USER=root cargo new fronius_meter_emulation
WORKDIR /usr/src/fronius_meter_emulation
COPY Cargo.toml Cargo.lock ./
# Seed the cache from dependencies from the Cargo.* files
RUN cargo build --release
# Now load in the source code (Layer that changes more frequently)
COPY src ./src
RUN cargo install --path .

# Bundle Stage
FROM alpine:latest
WORKDIR /usr/local/bin
COPY --from=builder /usr/local/cargo/bin/fronius_meter_emulation .
USER 1000
EXPOSE 5502
HEALTHCHECK CMD netstat -an | grep 5502 > /dev/null; if [ 0 != $? ]; then exit 1; fi;
CMD ["/usr/local/bin/fronius_meter_emulation"]
