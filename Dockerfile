FROM rust:1.40 as builder
WORKDIR /usr/src/shelflife
COPY src ./src
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo install --path .
FROM debian:buster-slim
RUN apt-get update -y && apt-get upgrade -y && apt-get install libssl-dev -y
#WORKDIR /usr/local/bin
#COPY --from=builder /usr/local/cargo/bin/shelflife .
#ENTRYPOINT ["./shelflife"]

WORKDIR /usr/local/bin
COPY --from=builder /usr/local/cargo/bin/shelflife .
USER 1001
CMD ["/bin/sh"]
#CMD ["./shelflife"]
