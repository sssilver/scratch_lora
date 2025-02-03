FROM espressif/idf-rust:esp32s3_latest

RUN cargo install cargo-watch

WORKDIR /project

CMD ["cargo", "watch", "-x", "build --release"]