# syntax=docker/dockerfile:1.7
# Three stages:
#   1. wasm-build  — Rust → wasm32 via wasm-pack, output the glue JS + .wasm
#                    into `frontend/src/lib/pkg/` so Vite can resolve it.
#   2. frontend    — pnpm install + SvelteKit static build.
#   3. runtime     — nginx:alpine serving the static bundle and proxying
#                    `/api/*` (REST + WebSocket) to the `backend` service.

FROM rust:1-bookworm AS wasm-build
RUN curl -fsSL https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY sorry-core/ sorry-core/
COPY sorry-wasm/ sorry-wasm/
# Stub sorry-server so the workspace resolves without building axum here.
COPY sorry-server/Cargo.toml sorry-server/Cargo.toml
RUN mkdir -p sorry-server/src && echo "fn main() {}" > sorry-server/src/main.rs

RUN --mount=type=cache,id=sorry-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=sorry-wasm-target,target=/app/target,sharing=locked \
    cd sorry-wasm \
    && wasm-pack build --release --target web --out-dir ../frontend/src/lib/pkg

FROM node:22-alpine AS frontend
ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
RUN corepack enable
WORKDIR /app

COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN --mount=type=cache,target=/pnpm/store \
    pnpm install --frozen-lockfile

COPY frontend/ .
COPY --from=wasm-build /app/frontend/src/lib/pkg/ ./src/lib/pkg/

# Build-time override: set `VITE_SERVER_URL` here to bake a specific
# backend origin into the bundle. Leave unset for same-origin deploys
# (the bundle will use `window.location.origin`, and nginx proxies
# `/api/*` to the `backend` service).
ARG VITE_SERVER_URL=""
ENV VITE_SERVER_URL=${VITE_SERVER_URL}

RUN pnpm build

FROM nginx:1.27-alpine AS runtime
RUN rm -rf /usr/share/nginx/html/* \
    && apk add --no-cache wget
COPY --from=frontend /app/build/ /usr/share/nginx/html/
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --quiet --spider http://127.0.0.1/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
