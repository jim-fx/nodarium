FROM jimfx/nodes:latest AS builder

WORKDIR /app

COPY pnpm-lock.yaml pnpm-workspace.yaml package.json Cargo.lock Cargo.toml ./

COPY packages/ ./packages/
COPY nodes/ ./nodes/
COPY app/package.json ./app/

RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  pnpm install --frozen-lockfile

COPY . .

RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  pnpm build:nodes && \
  pnpm --filter @nodarium/app... build

FROM nginx:alpine AS runner

RUN rm /etc/nginx/conf.d/default.conf

COPY app/docker/app.conf /etc/nginx/conf.d/app.conf

COPY --from=builder /app/app/build /app

EXPOSE 80
