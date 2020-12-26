FROM rust:1.40 as builder
WORKDIR /usr/src/shelflife
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get -y install libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/shelflife /usr/local/bin/shelflife
CMD ["shelflife"]