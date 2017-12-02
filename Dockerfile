FROM rust:1.20.0

WORKDIR /usr/src/tolla
copy tolla tolla/
copy tolla_proto tolla_proto/
copy src src/
copy Cargo.toml .

RUN cargo build --all

CMD ["target/debug/main", "http"]


