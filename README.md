# dorch

This repository contains the codebase behind **https://gib.gg** — a free platform for hosting and playing custom Doom WADs in your browser.

At a high level, dorch is a Kubernetes-based "Doom orchestrator":

- **A web frontend** for browsing WADs and joining games.
- **Rust microservices** for WAD metadata, server orchestration, identity/auth, and realtime coordination.
- **A Kubernetes operator + CRD** that turns "game requests" into running game pods.
- **Workers** that ingest WAD metadata, generate screenshots, and optionally run analysis pipelines.

If you want to contribute, the most useful things to know are:

- Most runtime wiring happens through the Helm chart in `chart/`.
- The platform's custom Kubernetes API is the `Game` resource in `crds/`.
- The Rust services are built as a workspace (see top-level `Cargo.toml`).

---

## Architecture (quick tour)

The "happy path" for a player is roughly:

1. The `browser/` app renders pages and calls the backend APIs.
2. `dorch-wadinfo` serves WAD metadata and issues URLs for WAD payloads stored in S3-compatible object storage.
3. `dorch-master` manages the *game lifecycle* by creating and updating `Game` CRs in Kubernetes.
4. `dorch-operator` watches `Game` CRs and creates/updates pods to run the actual Doom server workloads.

Supporting components include:

- **Keycloak** for user identity and OIDC.
- **Postgres** (WAD metadata, indexing).
- **Redis** (caching, coordination, rate limiting).
- **NATS** (worker queues / eventing).
- **LiveKit** (+ `webrtc-auth`) for WebRTC-related auth/tokens.
- Optional **SRS/RTMP** integration used by the "jumbotron" feature.

Most services expose `GET /healthz` and `GET /readyz` endpoints (see Helm templates for exact ports).

---

## Repository layout

Top-level directories you'll touch most often:

- `browser/` — Svelte/SvelteKit frontend (Vite + Tailwind).
- `chart/` — Helm chart that deploys dorch onto Kubernetes.
- `crds/` — Kubernetes CRDs (notably `Game`).
- `common/` — Rust shared utilities (logging, config parsing, etc.).
- `types/` — Rust shared types, including the `Game` CRD type definition.
- `operator/` — Kubernetes operator that reconciles `Game` resources into pods.
- `wadinfo/` — Rust service that stores/serves WAD metadata and interacts with Postgres, Redis, S3.
- `master/` — Rust service that manages the game/server lifecycle and talks to Kubernetes.
- `iam/` — Rust service related to identity/account management.
- `auth/` — Rust service related to Doom-side authentication (includes a UDP port for Zandronum auth).
- `sock/` — Rust realtime service for the browser (NATS/Redis/Keycloak wired in via chart).
- `webrtc-auth/` — Rust service to mint/authorize LiveKit access (Keycloak + LiveKit API creds).
- `party/` — Rust service and router (optional in Helm values).
- `archiver/` — Python worker(s) for metadata ingestion and asset processing.
- `analyzer/` — Rust analysis workers (map/WAD), optionally using an LLM via OpenAI-compatible API.
- `downloader/` — Small AWS CLI-based utility image to download WADs/IWAD overrides given wad IDs.
- `zandronum/` — The game engine build context + Dockerfiles for server/client/spectator images.
- `proxy/` —  Rust WebRTC <-> UDP proxy (for in-game networking)
- `scripts/` — Convenience scripts for deploying/upgrading and cluster operations.

Rust workspace members are defined in the top-level `Cargo.toml`:

```
common/
operator/
iam/
proxy/
master/
types/
sock/
party/
auth/
analyzer/
wadinfo/
webrtc-auth/
```

(`iam-client/` exists but is excluded from the workspace.)

---

## Kubernetes API: the `Game` CRD

The core orchestration primitive is the namespaced `Game` custom resource:

- **CRD YAML:** `crds/dorch.beebs.dev_game_crd.yaml`
- **Rust type definition:** `types/src/lib.rs` (`GameSpec`, `GameStatus`)
- **CRD generation:** `operator/build.rs` writes the CRD into `crds/` at build time

Fields you'll see commonly:

- `spec.game_id` — stable identifier for the game
- `spec.iwad` — IWAD name
- `spec.files` — optional list of additional WAD/PK3 identifiers
- `spec.max_players` — max player count
- `spec.gamemode`, `spec.skill`, `spec.dmflags`, `spec.time_limit`, `spec.frag_limit` — gameplay knobs
- `spec.resources` — optional Kubernetes `ResourceRequirements` for the game pod
- `status.phase` — `Pending | Starting | Active | Error | Terminating`

Example (trimmed) `Game` resource:

```yaml
apiVersion: dorch.beebs.dev/v1
kind: Game
metadata:
  name: example-game
spec:
  game_id: "example-game"
  name: "Example Game"
  s3_secret_name: "game-spaces-cred"
  iwad: "[UUID of IWAD file on gib.gg]"
  max_players: 8
  files:
    - "[UUID of WAD file on gib.gg]"
  gamemode: "Cooperative"
  skill: 3
```

---

## Services and ports (as deployed by Helm)

The chart generally uses a pattern of **internal** port `80` (ClusterIP) plus a **public** port `3000` for the Rust HTTP APIs.

Notable exceptions:

- `auth/` exposes TCP `3500` (client) + TCP `2500` (admin/health) + UDP `16666` (Zandronum auth).
- `sock/` is a single public TCP listener (default `3000`).
- `webrtc-auth/` is a single HTTP listener (default `80`).

Prometheus metrics are on TCP `2112` when `prometheus.enabled=true` in Helm values.

---

## Building

### Rust workspace

Build everything:

```sh
cargo build --workspace
```

Run tests:

```sh
cargo test --workspace
```

Format + lint:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

### Frontend (`browser/`)

```sh
cd browser
npm install
npm run dev
```

Note: the frontend depends on both internal and public service endpoints. 

### Docker images (Buildx bake)

This repo uses `docker buildx bake` with targets defined in `docker-bake.hcl`.

- Build/push a set of images (default group):

```sh
./build.sh
```

- Build/push a subset:

```sh
./build.sh wadinfo master browser
```

`build.sh` currently runs bake with `--push` (see the script before using it against your own registry).

---

## Deploying (Kubernetes)

The Helm chart in `chart/` deploys dorch services as **ClusterIP** services. Ingress/HTTP routing is expected to be provided by your cluster (Ingress controller, Gateway API, etc.).

### Apply CRDs

```sh
kubectl apply -f crds/
```

### Install/upgrade with Helm

There's a convenience script:

```sh
./scripts/upgrade_dorch.sh
```

That runs (roughly):

```sh
helm upgrade dorch chart/ \
  --create-namespace \
  --install \
  -n dorch \
  -f scripts/dorch_values.yaml
```

### Build + restart in a dev cluster

`up.sh` is a "push images then restart deployments" helper. It:

1. Builds/pushes the relevant images with `./build.sh`.
2. Applies CRDs in `crds/`.
3. `kubectl rollout restart`'s the selected deployments.
4. Opens `k9s`.

```sh
./up.sh wadinfo master browser
```

---

## Worker pipelines

### Archiver (`archiver/`)

The archiver is a Python worker used for metadata ingestion and related batch jobs. The Helm deployment:

- Uses an init container to download IWADs into a hostPath volume (`/data/iwads`).
- Runs a worker (`/app/meta-worker.py`) and can optionally post results into `wadinfo`.
- Uses NATS for dispatch/queueing.

Python dependencies for these workers are listed in `requirements.txt`.

### Screenshot worker (`archiver/`)

The screenshot pipeline runs as a separate deployment and uses an init container to fetch IWADs, then runs `/app/screenshot-worker.py`.

### Analyzer (`analyzer/`)

The analyzer deployments run `dorch-analyzer wad` and `dorch-analyzer map`.

They're wired to:

- `WADINFO_ENDPOINT` (to fetch artifacts/metadata)
- Redis + NATS
- Optional OpenAI-compatible API credentials via `OPENAI_API_KEY` (+ optional `OPENAI_BASE_URL` and `MODEL`)

In the default Helm values, analyzer replicas are set to `0` (disabled).

---

## Contributing

Contributions are welcome.

Practical guidance:

- **Start with the Helm chart**: most service configuration and wiring is documented implicitly in `chart/templates/`.
- **Prefer small, composable changes**: many services share env vars and conventions via `common/`.
- **Run formatters/linters** before opening a PR:
  - Rust: `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features`
  - Browser: `cd browser && npm run lint`

If you're adding/changing Kubernetes fields:

- Update the Rust CRD types in `types/`.
- Rebuild the operator (or run its build script) to refresh `crds/dorch.beebs.dev_game_crd.yaml`.

---

## License

MIT / Apache 2.0 dual license.