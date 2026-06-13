# metrics-service

A small, production-shaped **real-time metrics aggregation service** written in Rust with [axum](https://github.com/tokio-rs/axum) and [tokio](https://tokio.rs).

Clients push metric events over HTTP. The service keeps a running aggregation (average, min, max, count) per `service + metric` in memory, and fans live updates out to subscribers over **Server-Sent Events (SSE)**.

It is intentionally the kind of infrastructure service Rust is good at: many concurrent writers, long-lived streaming connections, low memory floor, predictable latency, no GC pauses.

---

## Table of contents

- [Features](#features)
- [Architecture](#architecture)
- [Project structure](#project-structure)
- [API reference](#api-reference)
- [Running it](#running-it)
- [Configuration](#configuration)
- [Health checks](#health-checks)
- [How it works internally](#how-it-works-internally)

---

## Features

- `POST /metrics` to record a single metric event; returns updated aggregate.
- `GET /stats/{service}/{metric}` to read current aggregate (404 if unknown).
- `GET /stats/watch/{service}` opens an SSE stream of live updates for a service.
- `GET /health` liveness probe.
- Concurrent in-memory store (`DashMap`) — no manual lock management.
- Broadcast fan-out to all SSE subscribers via a `tokio::sync::broadcast` channel.
- Graceful shutdown on Ctrl+C / SIGTERM (drains in-flight requests).
- Ships a multi-stage `Dockerfile` producing a ~4.6 MB `scratch` image.

---

## Architecture

```
                 +-------------------+
  POST /metrics  |                   |   record + aggregate
  ------------>  |  record_metric    |---------------------+
                 |  (handlers)       |                     v
                 +-------------------+            +------------------+
                                                  |  DashMap<String, |
  GET /stats/... |                   |  read      |  RunningStats>   |
  ------------>  |  get_stats        |<-----------|  (shared state)  |
                 +-------------------+            +------------------+
                                                          |
                 +-------------------+   broadcast        |
  GET /stats/    |  watch_stats      |<-------------------+
  watch/{svc}    |  (SSE stream)     |   StatsUpdate fan-out
  ------------>  +-------------------+
        SSE  <---  data: {...}  (filtered to one service)
```

- Every recorded metric updates the shared `DashMap` **and** is published on a broadcast channel.
- Each `watch` connection is a lightweight async task subscribed to that channel, filtered to one service name.
- The map key is `"{service_name}:{metric_name}"`.

---

## Project structure

```
metrics-service/
├── Cargo.toml
├── Dockerfile            # multi-stage musl build -> scratch image
├── .dockerignore         # keeps target/ out of the build context
├── docker-compose.yml    # one-command build + run
└── src/
    ├── main.rs               # bootstrap, graceful shutdown, healthcheck subcommand
    ├── config.rs             # env configuration (PORT, BROADCAST_CAPACITY)
    ├── error.rs              # AppError -> HTTP status mapping
    ├── state.rs              # AppState: DashMap + broadcast publisher
    ├── router.rs             # route registration
    ├── models/
    │   ├── metric.rs         # MetricEvent (incoming request)
    │   └── stats.rs          # RunningStats + StatsUpdate (response)
    └── handlers/
        ├── health.rs         # GET /health
        ├── metrics.rs        # POST /metrics
        └── stats.rs          # GET /stats/* and SSE streaming
```

---

## API reference

### `GET /health`

Liveness probe. Always 200 while the process is up.

```json
{ "status": "ok" }
```

### `POST /metrics`

Record one metric event. Returns the updated aggregate and broadcasts it to SSE subscribers.

Request body:

```json
{ "service_name": "payments", "metric_name": "latency_ms", "value": 42.5 }
```

Response `200 OK`:

```json
{
  "service_name": "payments",
  "metric_name": "latency_ms",
  "avg": 42.5,
  "min": 42.5,
  "max": 42.5,
  "count": 1
}
```

### `GET /stats/{service}/{metric}`

Current aggregate for one metric. `404` with `{"error":"metric not found"}` if nothing recorded yet.

```bash
curl http://localhost:3000/stats/payments/latency_ms
```

### `GET /stats/watch/{service}`

Server-Sent Events stream. Emits a JSON line every time **any** metric for that service is recorded. Keeps the connection open; sends a `ping` keep-alive every 15s.

```bash
curl -N http://localhost:3000/stats/watch/payments
```

Each event:

```
data: {"service_name":"payments","metric_name":"latency_ms","avg":50.0,"min":42.5,"max":57.5,"count":2}
```

---

## Running it

### Option A — Local (cargo)

Requires a Rust toolchain ([rustup](https://rustup.rs)).

```bash
cargo run            # debug
cargo run --release  # optimized
```

### Option B — Docker (no local Rust needed)

```bash
docker compose up -d --build    # build + run
docker compose logs -f          # logs
docker compose down             # stop
```

Or plain Docker:

```bash
docker build -t metrics-service:latest .
docker run -d --name metrics-service -p 3000:3000 metrics-service:latest
```

The image is built in two stages: a `rust:slim` builder produces a fully static
**musl** binary, and the runtime stage is `FROM scratch` — nothing but the
binary ships. Result: ~4.6 MB image, ~2 MB idle RAM.

### Quick smoke test

```bash
curl http://localhost:3000/health

curl -X POST http://localhost:3000/metrics \
  -H "Content-Type: application/json" \
  -d '{"service_name":"payments","metric_name":"latency_ms","value":42.5}'

curl http://localhost:3000/stats/payments/latency_ms

# In one terminal, watch; in another, POST a metric for "payments":
curl -N http://localhost:3000/stats/watch/payments
```

---

## Configuration

Set via environment variables:

| Variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | TCP port to listen on |
| `BROADCAST_CAPACITY` | `1024` | Buffered events per SSE subscriber before lag-drop |
| `RUST_LOG` | `metrics_service=debug,info` | Log filter (e.g. `RUST_LOG=debug`) |

Example:

```bash
PORT=8080 RUST_LOG=debug cargo run
```

In `docker-compose.yml` these are set under `environment:`.

---

## Health checks

The `scratch` image has no shell or `curl`, so the binary health-checks itself:

```bash
metrics-service healthcheck
```

This opens a TCP connection to `127.0.0.1:$PORT`, sends `GET /health`, and exits
`0` (healthy) or `1` (failed). The Docker `HEALTHCHECK` uses it; check status with:

```bash
docker inspect -f '{{.State.Health.Status}}' metrics-service
```

The same command works as a Kubernetes `exec` liveness/readiness probe.

---

## How it works internally

**Shared state** (`state.rs`) is a `DashMap<String, RunningStats>` plus a
`broadcast::Sender<StatsUpdate>`, wrapped in an `Arc` and injected into every
handler. `DashMap` shards its locks, so writers to different keys rarely contend
— no global mutex, no manual locking.

**Recording** (`handlers/metrics.rs`): `record_metric` updates the running
aggregate for the key, builds a `StatsUpdate`, and publishes it on the broadcast
channel. Send errors are ignored — they only mean no subscribers are connected.

**Streaming** (`handlers/stats.rs`): `watch_stats` subscribes to the broadcast
channel and wraps it in a stream filtered to the requested service. A slow
subscriber that lags is skipped silently rather than blocking others.

**Aggregation** (`models/stats.rs`): `RunningStats` keeps `count`, `sum`,
`min`, `max`; `avg` is derived on read. `StatsUpdate` is the serializable
snapshot returned to clients.

**Errors** (`error.rs`): handlers return `Result<_, AppError>`. `AppError`
maps to HTTP in one place — `NotFound` → 404, `Internal` → 500, both as JSON.

**Shutdown** (`main.rs`): the server waits for Ctrl+C or SIGTERM, then stops
accepting connections and lets in-flight requests finish — important for clean
rolling deploys in Kubernetes.

> Note: state is **in-memory only**. Restarting the service clears all
> aggregates. That is by design for this example; a real deployment would add
> persistence or a time-window/rollup strategy.
