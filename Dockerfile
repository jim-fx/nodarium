FROM node:24-alpine

RUN apk add --no-cache --update curl rclone g++

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --silent --show-error --location --fail --retry 3 \
  --proto '=https' --tlsv1.2 \
  --output /tmp/rustup-init.sh https://sh.rustup.rs \
  && sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal \
  && rm /tmp/rustup-init.sh \
  && rustup target add wasm32-unknown-unknown \
  && npm i -g pnpm
