FROM node:24-alpine

# Install all required packages in one layer
RUN apk add --no-cache curl git jq g++ make

# Set Rust paths
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Install Rust, wasm target, and pnpm
RUN curl --silent --show-error --location --fail --retry 3 \
      --proto '=https' --tlsv1.2 \
      --output /tmp/rustup-init.sh https://sh.rustup.rs \
    && sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal \
    && rm /tmp/rustup-init.sh \
    && rustup target add wasm32-unknown-unknown \
    && rm -rf /usr/local/rustup/toolchains/*/share/doc \
    && npm i -g pnpm
