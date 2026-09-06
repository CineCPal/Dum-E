//! Product-name detection used to stop cross-tool bleed-through: e.g. a
//! DaVinci Resolve/Fusion manual chunk scoring well enough to confidently
//! (and misleadingly) answer a Premiere Pro question, just because the
//! underlying concept (chroma keying, masking, ...) is similar across
//! tools. Each manual is tagged with a `product` slug at ingestion time
//! (see `ingestion/manifest.json`); this module maps question text to
//! those same slugs, plus a separate list of products we have no manual
//! for at all.

/// (product slug matching `manuals.product`, aliases that identify a
/// question is about that product).
const COVERED_PRODUCTS: &[(&str, &[&str])] = &[
    ("davinci-resolve", &["davinci resolve", "resolve"]),
    ("fusion", &["fusion"]),
    ("fairlight-live", &["fairlight live"]),
    ("blackmagic-pyxis-6k", &["pyxis"]),
    ("blackmagic-pocket-cinema-camera-4k", &["pocket cinema camera", "bmpcc"]),
    ("blackmagic-streaming", &["blackmagic streaming", "web presenter", "streaming bridge"]),
    ("dji-avata-2", &["avata"]),
    ("dji-mavic-3", &["mavic"]),
    ("dji-mini-5-pro", &["mini 5 pro", "mini 5"]),
    ("dji-neo-2", &["dji neo", "neo 2"]),
    ("atomos-ninja-v", &["ninja v"]),
    ("atomos-shogun-7", &["shogun"]),
    ("portkeys-lh5p", &["portkeys", "lh5p"]),
    ("rode-wireless-pro", &["wireless pro"]),
    ("sony-fs5", &["fs5"]),
    ("sony-fx3", &["fx3"]),
];

/// Tools Dum-E has NO manual for. Mentioning one of these forces the
/// fallback/decline path regardless of retrieval similarity.
const UNCOVERED_PRODUCTS: &[&str] = &[
    "premiere pro",
    "premiere",
    "after effects",
    "blender",
    "final cut pro",
    "final cut",
    "fcpx",
    "avid",
    "media composer",
    "photoshop",
    "capcut",
];

/// Single-word aliases match whole words only (so "resolve" doesn't match
/// inside "unresolved"); multi-word aliases match as a substring phrase.
fn mentions(question_lower: &str, alias: &str) -> bool {
    if alias.contains(' ') {
        question_lower.contains(alias)
    } else {
        question_lower.split(|c: char| !c.is_alphanumeric()).any(|tok| tok == alias)
    }
}

pub fn mentions_uncovered_product(question: &str) -> bool {
    let q = question.to_lowercase();
    UNCOVERED_PRODUCTS.iter().any(|alias| mentions(&q, alias))
}

/// The covered-product slug the question names, if any (first match wins).
pub fn mentioned_covered_product(question: &str) -> Option<&'static str> {
    let q = question.to_lowercase();
    COVERED_PRODUCTS
        .iter()
        .find(|(_, aliases)| aliases.iter().any(|a| mentions(&q, a)))
        .map(|(slug, _)| *slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_uncovered_products() {
        assert!(mentions_uncovered_product("What's the best way to pull a chroma key in Premiere Pro?"));
        assert!(mentions_uncovered_product("How do I create a mask in After Effects?"));
        assert!(!mentions_uncovered_product("How do I create a mask in Fusion?"));
    }

    #[test]
    fn detects_covered_products_as_whole_words() {
        assert_eq!(mentioned_covered_product("How do you set up a CST in Resolve?"), Some("davinci-resolve"));
        assert_eq!(mentioned_covered_product("How do I save a memory recall on the FX3?"), Some("sony-fx3"));
        assert_eq!(mentioned_covered_product("This issue remains unresolved"), None);
    }
}
