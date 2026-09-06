#!/usr/bin/env python3
"""One-off stress test for the product-gating fix in rag/mod.rs + products.rs.

Reimplements the exact retrieval + routing logic (top_k_chunks SQL, then
decide_path's product-gating rules) in Python so many questions can be
checked against the real database and real embeddings without driving the
GUI once per question. Not part of the app -- a throwaway verification
script.
"""

import json
import re
import sqlite3
import sys

import requests
import sqlite_vec

DB_PATH = "/Users/cj/Library/Application Support/com.dume.app/dume.db"
OLLAMA_URL = "http://127.0.0.1:11434"
EMBEDDING_MODEL = "nomic-embed-text:latest"
TOP_K = 6
WIDE_RETRIEVAL_K = 40  # mirrors commands/chat.rs WIDE_RETRIEVAL_K
PRIMARY_THRESHOLD = 0.55
SECONDARY_THRESHOLD = 0.45

# Mirrors src-tauri/src/rag/products.rs
COVERED_PRODUCTS = [
    ("davinci-resolve", ["davinci resolve", "resolve"]),
    ("fusion", ["fusion"]),
    ("fairlight-live", ["fairlight live"]),
    ("blackmagic-pyxis-6k", ["pyxis"]),
    ("blackmagic-pocket-cinema-camera-4k", ["pocket cinema camera", "bmpcc"]),
    ("blackmagic-streaming", ["blackmagic streaming", "web presenter", "streaming bridge"]),
    ("dji-avata-2", ["avata"]),
    ("dji-mavic-3", ["mavic"]),
    ("dji-mini-5-pro", ["mini 5 pro", "mini 5"]),
    ("dji-neo-2", ["dji neo", "neo 2"]),
    ("atomos-ninja-v", ["ninja v"]),
    ("atomos-shogun-7", ["shogun"]),
    ("portkeys-lh5p", ["portkeys", "lh5p"]),
    ("rode-wireless-pro", ["wireless pro"]),
    ("sony-fs5", ["fs5"]),
    ("sony-fx3", ["fx3"]),
]

UNCOVERED_PRODUCTS = [
    "premiere pro", "premiere", "after effects", "blender", "final cut pro",
    "final cut", "fcpx", "avid", "media composer", "photoshop", "capcut",
]


def mentions(q_lower: str, alias: str) -> bool:
    """Mirrors rag/products.rs exactly: single-word aliases match whole
    words only, splitting on any non-alphanumeric character (so trailing
    punctuation like "Blender?" still matches "blender")."""
    if " " in alias:
        return alias in q_lower
    tokens = re.split(r"[^a-z0-9]+", q_lower)
    return alias in tokens


def mentions_uncovered_product(question: str) -> bool:
    q = question.lower()
    return any(mentions(q, a) for a in UNCOVERED_PRODUCTS)


def mentioned_covered_product(question: str):
    q = question.lower()
    for slug, aliases in COVERED_PRODUCTS:
        if any(mentions(q, a) for a in aliases):
            return slug
    return None


def embed(text: str):
    resp = requests.post(
        f"{OLLAMA_URL}/api/embed", json={"model": EMBEDDING_MODEL, "input": [text]}, timeout=60
    )
    resp.raise_for_status()
    return resp.json()["embeddings"][0]


def top_k_chunks(conn, embedding, k):
    q_json = json.dumps(embedding)
    rows = conn.execute(
        """
        SELECT m.title, m.product, c.page_start, c.page_end, v.distance
        FROM vec_chunks v
        JOIN chunks c ON c.id = v.rowid
        JOIN manuals m ON m.id = c.manual_id
        WHERE v.embedding MATCH ? AND k = ?
        ORDER BY v.distance
        """,
        (q_json, k),
    ).fetchall()
    return [
        {"title": r[0], "product": r[1], "page_start": r[2], "page_end": r[3], "similarity": 1 - r[4]}
        for r in rows
    ]


def retrieval_k_for(question: str) -> int:
    return WIDE_RETRIEVAL_K if mentioned_covered_product(question) else TOP_K


def decide(question: str, hits: list, fallback_available: bool = True):
    if mentions_uncovered_product(question):
        return ("FALLBACK" if fallback_available else "DECLINE"), None, "question names an uncovered product (retrieval skipped)"

    product = mentioned_covered_product(question)
    candidates = [h for h in hits if h["product"] == product] if product else hits

    top_ok = bool(candidates) and candidates[0]["similarity"] >= PRIMARY_THRESHOLD
    secondary_count = sum(1 for h in candidates if h["similarity"] >= SECONDARY_THRESHOLD)

    detail = f"filtered to product={product} (retrieval_k={len(hits)})" if product else "no product filter"
    if top_ok and secondary_count >= 2:
        return "MANUAL", candidates[:TOP_K], detail
    if fallback_available:
        return "FALLBACK", None, detail
    return "DECLINE", None, detail


TEST_CASES = [
    # (question, expected_path, note)
    ("What's the best way to pull a chroma key in Premiere Pro?", "FALLBACK", "uncovered: Premiere Pro"),
    ("How do I create a mask in After Effects?", "FALLBACK", "uncovered: After Effects"),
    ("How do I add a keyframe in Blender?", "FALLBACK", "uncovered: Blender"),
    ("What's the shortcut for ripple delete in Final Cut Pro?", "FALLBACK", "uncovered: Final Cut Pro"),
    ("How do I set up a multicam sequence in Avid Media Composer?", "FALLBACK", "uncovered: Avid"),
    ("How do I use content-aware fill in Photoshop?", "FALLBACK", "uncovered: Photoshop"),
    ("How do I add captions in CapCut?", "FALLBACK", "uncovered: CapCut"),
    ("Premiere won't export in H.264, what do I do?", "FALLBACK", "uncovered: Premiere (short form)"),
    ("How do you save a memory recall setting on the FX3?", "MANUAL", "covered: should pull sony-fx3 only"),
    ("How do I white balance the Sony FS5?", "MANUAL", "covered: should pull sony-fs5, not fx3"),
    ("How do I update firmware on the DJI Avata 2?", "MANUAL", "covered: should pull dji-avata-2 only"),
    ("What's the max flight time on the Mavic 3?", "MANUAL", "covered: should pull dji-mavic-3 only"),
    ("How do I do a color space transform in DaVinci Resolve?", "MANUAL", "covered: davinci-resolve only"),
    ("How do I use the Magic Mask tool in Fusion?", "MANUAL_OR_FALLBACK", "covered: fusion only (standalone Fusion manual is thin)"),
    ("How do I set up a mix bus in Fairlight Live?", "MANUAL_OR_FALLBACK", "covered: fairlight-live only (thin manual)"),
    ("How do I white balance my camera before a shoot?", "ANY", "generic, no product named"),
    ("What's a good workflow for organizing raw footage?", "ANY", "generic, no product named"),
    ("This issue remains unresolved, what should I check?", "ANY", "word-collision: 'resolve' must not match inside 'unresolved'"),
]


def main():
    conn = sqlite3.connect(DB_PATH)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)

    failures = 0
    for question, expected, note in TEST_CASES:
        if mentions_uncovered_product(question):
            # Mirrors chat.rs: uncovered products skip embedding/retrieval entirely.
            hits = []
        else:
            try:
                embedding = embed(question)
            except Exception as exc:  # noqa: BLE001
                print(f"[ERROR] embedding failed for {question!r}: {exc}")
                failures += 1
                continue
            hits = top_k_chunks(conn, embedding, retrieval_k_for(question))

        path, candidates, detail = decide(question, hits)

        ok = expected == "ANY" or path == expected or (expected == "MANUAL_OR_FALLBACK" and path in ("MANUAL", "FALLBACK"))
        status = "OK  " if ok else "FAIL"
        if not ok:
            failures += 1

        print(f"[{status}] {path:9s} | {question}")
        print(f"        note: {note} | {detail}")
        if path == "MANUAL" and candidates:
            top = candidates[0]
            print(f"        top hit: {top['title']} (product={top['product']}, sim={top['similarity']:.3f}, p.{top['page_start']}-{top['page_end']})")
        elif not candidates and hits:
            print(f"        (raw top hit before gating: {hits[0]['title']} [{hits[0]['product']}] sim={hits[0]['similarity']:.3f})")
        print()

    print(f"--- {len(TEST_CASES) - failures}/{len(TEST_CASES)} passed ---")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
