# srvcs-samplestddev

The sample standard deviation service of the srvcs.cloud distributed standard
library.

Its single concern: **the sample standard deviation of a list of numbers**,
returned as an `f64`. It does no arithmetic of its own. It is a pure
orchestrator over two dependencies:

```text
v      = samplevariance(values).result    # one call to srvcs-samplevariance
result = sqrt(v).result                   # one call to srvcs-sqrt
```

So `samplestddev([1,2,3,4,5]) ~= 1.5811388300841898` — the sample variance is
`2.5`, and its square root is the sample standard deviation.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Compute the sample standard deviation of the numbers in `values` |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"values": [1, 2, 3, 4, 5]}'
# {"values":[1,2,3,4,5],"result":1.5811388300841898}
```

Responses:

- `200 {"values": [...], "result": x}` — evaluated; `result` is an `f64`.
- `422` — a dependency rejected the input (forwarded from `srvcs-samplevariance`).
- `500` — a dependency returned a malformed result.
- `503` — a dependency is unavailable.

## Dependencies

- [`srvcs-samplevariance`](https://github.com/srvcs/samplevariance)
- [`srvcs-sqrt`](https://github.com/srvcs/sqrt)

This service is an orchestrator: it never calls `srvcs-isnumber` directly.
Input validation propagates from its dependencies — an invalid sample (e.g. too
short for a sample variance, or a non-numeric element) is caught by
`srvcs-samplevariance`, whose `422` is forwarded verbatim.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_SAMPLEVARIANCE_URL` | `http://127.0.0.1:8090` | Base URL of `srvcs-samplevariance` |
| `SRVCS_SQRT_URL` | `http://127.0.0.1:8091` | Base URL of `srvcs-sqrt` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up mock dependencies in-process that **actually
compute** (the real sample variance, then the real square root), so the
composition is genuinely exercised against asserted cases — e.g.
`samplestddev([1,2,3,4,5]) ~= 1.5811388300841898` — with a `1e-9` tolerance. See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
