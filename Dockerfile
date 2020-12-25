FROM rust:1.48.0 AS builder

WORKDIR /app

ADD . /app

RUN cargo build --release

FROM gcr.io/distroless/cc

COPY --from=builder /app/target/release/bond /

CMD ["./bond"]
