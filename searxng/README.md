# Dum-E's search fallback: self-hosted SearXNG

Dum-E's internet fallback (used only when the manuals don't cover a
question, and only when you've enabled it in Settings) queries a
[SearXNG](https://docs.searxng.org/) instance running locally in Docker.
No API key, no billing, no third-party vendor -- it's just a metasearch
engine you run yourself, aggregating results from other public search
engines.

`settings.yml` in this folder is SearXNG's default config with one change:
`search.formats` includes `json` (disabled by default), since Dum-E's
Rust backend (`src-tauri/src/search/mod.rs`) queries `/search?format=json`.

## Setup

```bash
cd searxng
export SEARXNG_SECRET=$(openssl rand -hex 32)
docker compose up -d
```

Verify it's working:

```bash
curl -s "http://127.0.0.1:8888/search?q=test&format=json" | head -c 200
```

You should get back JSON (not an error page). If you get an error about
the format not being supported, double check `settings.yml` still has
`json` under `search.formats` and that you re-created the container after
editing it (`docker compose up -d --force-recreate`).

The port is bound to `127.0.0.1` only (see `docker-compose.yml`) -- it's
not reachable from your LAN, only from this Mac.

## Stopping / removing

```bash
docker compose down
```

Dum-E's Settings dialog lets you point at a different SearXNG URL if you
run it elsewhere (a different port, a remote instance you trust, etc.) --
default is `http://127.0.0.1:8888`.
