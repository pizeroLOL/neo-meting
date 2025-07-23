FROM rust:trixie as builder
COPY . /app
WORKDIR /app
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cargo \
    --config source.crates-io.replace-with=\"ustc\" \
    --config source.ustc.registry=\"sparse+https://mirrors.ustc.edu.cn/crates.io-index/\" \
    build \
    -r

FROM debian:trixie
COPY --from=builder /app/target/release/neo-meting /app/neo-meting
WORKDIR /app
CMD [ "/app/neo-meting" ]
