FROM rust:1.20.0

WORKDIR /usr/src/client
copy . .

RUN cargo build

CMD ["target/debug/client"]


