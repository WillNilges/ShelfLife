FROM rust:1.40 as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get upgrade && apt-get install libssl-dev
#&& apt-get install -y extra-runtime-dependencies
COPY --from=builder /usr/local/cargo/bin/shelflife /usr/local/bin/shelflife
CMD ["shelflife"]
