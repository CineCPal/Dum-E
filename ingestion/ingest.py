#!/usr/bin/env python3
"""Dum-E manual ingestion.

Scans a directory of PDF manuals, extracts and chunks their text, embeds
each chunk via a local Ollama model, and writes everything into the SQLite
(+ sqlite-vec) database the Tauri runtime queries at chat time.

Safe to re-run any time: unchanged PDFs (by SHA-256) are skipped, so adding
one new manual to books/ and re-running only processes that file. Progress
is reported as one JSON object per line on stdout so a calling process (the
Rust "Re-index" command, or a human) can follow along without parsing
human-oriented log text.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import pymupdf
import requests
import sqlite_vec

from chunker import chunk_pages

OLLAMA_URL = "http://127.0.0.1:11434"
EMBED_BATCH_SIZE = 32
EMBEDDING_DIMENSIONS = 768
DEFAULT_EMBEDDING_MODEL = "nomic-embed-text:latest"


def emit(stage: str, **fields) -> None:
    payload = {"stage": stage, "manual": None, "chunk": None, "total_chunks": None, "message": None}
    payload.update(fields)
    print(json.dumps(payload), flush=True)


def default_db_path() -> Path:
    return Path.home() / "Library" / "Application Support" / "com.dume.app" / "dume.db"


def load_manifest(manifest_path: Path) -> dict:
    if manifest_path.exists():
        return json.loads(manifest_path.read_text())
    return {}


def _slugify(text: str) -> str:
    slug = "".join(c.lower() if c.isalnum() else "-" for c in text)
    while "--" in slug:
        slug = slug.replace("--", "-")
    return slug.strip("-")


def manual_info_for(filename: str, manifest: dict) -> tuple[str, str]:
    """Returns (title, product) for a manual, from manifest.json if listed,
    else derived from the filename. `product` identifies which camera/app
    a manual documents, used at query time to stop e.g. a DaVinci Resolve
    manual from confidently answering a Premiere Pro question."""
    entry = manifest.get(filename)
    if isinstance(entry, dict):
        return entry.get("title") or _fallback_title(filename), entry.get("product") or _slugify(filename)
    if isinstance(entry, str):  # backwards compat with the old title-only manifest shape
        return entry, _slugify(filename)
    return _fallback_title(filename), _slugify(filename)


def _fallback_title(filename: str) -> str:
    return Path(filename).stem.replace("_", " ").replace("-", " ").strip()


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def open_db(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)

    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS manuals (
            id INTEGER PRIMARY KEY,
            filename TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            product TEXT NOT NULL DEFAULT '',
            file_hash TEXT NOT NULL,
            page_count INTEGER NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            ingested_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            manual_id INTEGER NOT NULL REFERENCES manuals(id) ON DELETE CASCADE,
            page_start INTEGER NOT NULL,
            page_end INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            token_count INTEGER NOT NULL,
            content_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chats (
            id INTEGER PRIMARY KEY,
            created_at TEXT NOT NULL,
            title TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            used_fallback INTEGER NOT NULL DEFAULT 0,
            citations_json TEXT NOT NULL DEFAULT '[]'
        );
        """
    )
    # `product` was added after the initial release; add it for databases
    # created before this column existed (no-op once it's already there).
    try:
        conn.execute("ALTER TABLE manuals ADD COLUMN product TEXT NOT NULL DEFAULT ''")
    except sqlite3.OperationalError as exc:
        if "duplicate column" not in str(exc).lower():
            raise

    conn.execute(
        f"CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0("
        f"embedding float[{EMBEDDING_DIMENSIONS}] distance_metric=cosine)"
    )
    conn.execute(
        "INSERT OR IGNORE INTO chats (id, created_at, title) VALUES (1, datetime('now'), 'Dum-E Chat')"
    )
    conn.commit()
    return conn


def existing_hash(conn: sqlite3.Connection, filename: str) -> Optional[str]:
    row = conn.execute("SELECT file_hash FROM manuals WHERE filename = ?", (filename,)).fetchone()
    return row[0] if row else None


def embed_batch(texts: list[str], model: str) -> list[list[float]]:
    resp = requests.post(
        f"{OLLAMA_URL}/api/embed",
        json={"model": model, "input": texts},
        timeout=120,
    )
    resp.raise_for_status()
    return resp.json()["embeddings"]


def ingest_manual(
    conn: sqlite3.Connection,
    pdf_path: Path,
    title: str,
    product: str,
    file_hash: str,
    embedding_model: str,
) -> int:
    doc = pymupdf.open(pdf_path)
    pages = [page.get_text("text") for page in doc]
    page_count = len(pages)
    doc.close()

    emit("parsing", manual=title, message=f"{page_count} pages extracted")
    chunks = chunk_pages(pages)
    total_chunks = len(chunks)

    file_size = pdf_path.stat().st_size
    now = datetime.now(timezone.utc).isoformat()

    # Replacing (rather than updating in place) keeps this simple and
    # correct: ON DELETE CASCADE drops the manual's old chunks/vectors too.
    conn.execute("DELETE FROM manuals WHERE filename = ?", (pdf_path.name,))
    cur = conn.execute(
        "INSERT INTO manuals (filename, title, product, file_hash, page_count, file_size_bytes, ingested_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?)",
        (pdf_path.name, title, product, file_hash, page_count, file_size, now),
    )
    manual_id = cur.lastrowid

    for batch_start in range(0, total_chunks, EMBED_BATCH_SIZE):
        batch = chunks[batch_start : batch_start + EMBED_BATCH_SIZE]
        embeddings = embed_batch([c.text for c in batch], embedding_model)

        for chunk, embedding in zip(batch, embeddings):
            content_hash = hashlib.sha256(chunk.text.encode("utf-8")).hexdigest()
            cur = conn.execute(
                "INSERT INTO chunks (manual_id, page_start, page_end, chunk_index, text, token_count, content_hash) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    manual_id,
                    chunk.page_start,
                    chunk.page_end,
                    chunk.chunk_index,
                    chunk.text,
                    chunk.token_count,
                    content_hash,
                ),
            )
            chunk_id = cur.lastrowid
            conn.execute(
                "INSERT INTO vec_chunks (rowid, embedding) VALUES (?, ?)",
                (chunk_id, json.dumps(embedding)),
            )

        conn.commit()
        emit(
            "embedding",
            manual=title,
            chunk=min(batch_start + len(batch), total_chunks),
            total_chunks=total_chunks,
        )

    return total_chunks


def main() -> int:
    script_dir = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description="Ingest Dum-E's reference manuals into the local RAG index.")
    parser.add_argument("--books-dir", type=Path, default=script_dir.parent / "books")
    parser.add_argument("--db-path", type=Path, default=default_db_path())
    parser.add_argument("--embedding-model", default=DEFAULT_EMBEDDING_MODEL)
    args = parser.parse_args()

    manifest = load_manifest(script_dir / "manifest.json")
    pdf_paths = sorted(args.books_dir.glob("*.pdf"))
    emit("scanning", message=f"Found {len(pdf_paths)} PDF(s) to consider")

    if not pdf_paths:
        emit("done", message="No PDFs found -- nothing to index.")
        return 0

    conn = open_db(args.db_path)
    try:
        for pdf_path in pdf_paths:
            title, product = manual_info_for(pdf_path.name, manifest)
            file_hash = sha256_of(pdf_path)

            if existing_hash(conn, pdf_path.name) == file_hash:
                # Still sync title/product from manifest.json even when the
                # PDF itself is unchanged, so fixing a typo or a product tag
                # doesn't require a full (slow) re-embed.
                conn.execute(
                    "UPDATE manuals SET title = ?, product = ? WHERE filename = ?",
                    (title, product, pdf_path.name),
                )
                conn.commit()
                emit("skip", manual=title, message="unchanged, skipping")
                continue

            chunk_count = ingest_manual(conn, pdf_path, title, product, file_hash, args.embedding_model)
            emit("manual_done", manual=title, total_chunks=chunk_count)
    except Exception as exc:  # noqa: BLE001 -- report a clean JSON line, then re-raise for a non-zero exit
        emit("error", message=str(exc))
        raise
    finally:
        conn.close()

    emit("done", message="Ingestion complete.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
