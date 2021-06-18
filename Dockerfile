FROM rust:1.53.0 AS builder

WORKDIR /app

RUN cargo install cargo-strip

ADD . /app

RUN cargo build --release --all-features && cargo strip

FROM gcr.io/distroless/cc

COPY --from=builder /app/target/release/bond /

CMD ["./bond"]
