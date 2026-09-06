"""Page-aware text chunking for manual ingestion.

Splits a manual's per-page extracted text into overlapping windows sized for
embedding, while tracking which page(s) each chunk came from so the runtime
can cite "(Manual Name, p. X)" accurately.
"""

import bisect
from dataclasses import dataclass

TARGET_CHARS = 3200  # ~800 tokens at ~4 chars/token
OVERLAP_CHARS = 600  # ~150 tokens
# Only look this far back from a chunk's raw target end for a paragraph or
# sentence break, so we don't produce tiny chunks when no good break exists.
BOUNDARY_LOOKBACK = 400


@dataclass
class Chunk:
    page_start: int
    page_end: int
    chunk_index: int
    text: str
    token_count: int


def _page_for_offset(page_offsets: list[int], offset: int) -> int:
    """1-based page number containing the given character offset into the
    concatenated manual text."""
    return bisect.bisect_right(page_offsets, offset) + 1


def _find_break(text: str, target_end: int) -> int:
    """Looks for a paragraph break, then a sentence break, within
    BOUNDARY_LOOKBACK characters before target_end; falls back to target_end
    itself if neither is found nearby."""
    window_start = max(0, target_end - BOUNDARY_LOOKBACK)
    window = text[window_start:target_end]

    para_break = window.rfind("\n\n")
    if para_break != -1:
        return window_start + para_break + 2

    sentence_break = max(window.rfind(". "), window.rfind(".\n"))
    if sentence_break != -1:
        return window_start + sentence_break + 2

    return target_end


def chunk_pages(pages: list[str]) -> list[Chunk]:
    """Chunks a manual's per-page text into ~800-token (~3,200 char) windows
    with ~150-token (~600 char) overlap, preferring paragraph then sentence
    boundaries so a chunk never splits mid-sentence."""
    parts: list[str] = []
    page_offsets: list[int] = []  # page_offsets[i] = end offset of page i+1
    offset = 0
    for text in pages:
        parts.append(text)
        offset += len(text)
        page_offsets.append(offset)
        parts.append("\n\n")
        offset += 2

    full_text = "".join(parts)
    total_len = len(full_text)
    page_count = len(pages)

    chunks: list[Chunk] = []
    start = 0
    index = 0

    while start < total_len:
        target_end = min(start + TARGET_CHARS, total_len)
        end = target_end if target_end >= total_len else _find_break(full_text, target_end)
        if end <= start:
            end = target_end

        chunk_text = full_text[start:end].strip()
        if chunk_text:
            page_start = min(_page_for_offset(page_offsets, start), page_count) if page_count else 1
            page_end = min(_page_for_offset(page_offsets, max(end - 1, start)), page_count) if page_count else 1
            chunks.append(
                Chunk(
                    page_start=page_start,
                    page_end=max(page_end, page_start),
                    chunk_index=index,
                    text=chunk_text,
                    # Rough token estimate (~4 chars/token for English) -- good
                    # enough for sizing/telemetry, not an exact tokenizer count.
                    token_count=max(1, len(chunk_text) // 4),
                )
            )
            index += 1

        if end >= total_len:
            break
        start = max(end - OVERLAP_CHARS, start + 1)

    return chunks
