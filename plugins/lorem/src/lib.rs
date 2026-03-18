use std::collections::HashMap;

use extism_pdk::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct ShortcodeContext {
    name: String,
    args: Vec<String>,
    kwargs: HashMap<String, String>,
    #[allow(dead_code)]
    format: String,
}

const WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing",
    "elit", "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore",
    "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam",
    "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure",
    "in", "reprehenderit", "voluptate", "velit", "esse", "cillum",
    "fugiat", "nulla", "pariatur", "excepteur", "sint", "occaecat",
    "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
    "deserunt", "mollit", "anim", "id", "est", "laborum", "at", "vero",
    "eos", "accusamus", "iusto", "odio", "dignissimos", "ducimus",
    "blanditiis", "praesentium", "voluptatum", "deleniti", "atque",
    "corrupti", "quos", "dolores", "quas", "molestias", "excepturi",
    "obcaecati", "cupiditate", "provident", "similique", "optio",
    "cumque", "nihil", "impedit", "quo", "minus", "quod", "maxime",
    "placeat", "facere", "possimus", "omnis", "voluptas", "assumenda",
    "repellendus", "temporibus", "autem", "quibusdam", "officiis",
    "debitis", "aut", "rerum", "necessitatibus", "saepe", "eveniet",
    "voluptates", "repudiandae", "recusandae", "itaque", "earum",
    "hic", "tenetur", "sapiente", "delectus", "reiciendis", "voluptatibus",
    "maiores", "alias", "perferendis", "doloribus", "asperiores",
    "repellat",
];

/// Generate `n` words of lorem ipsum.
fn gen_words(n: usize) -> String {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(WORDS[i % WORDS.len()]);
    }
    let mut s = out.join(" ");
    // Capitalize first letter
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// Generate a sentence of roughly `word_count` words.
fn gen_sentence(word_count: usize, offset: usize) -> String {
    let mut out = Vec::with_capacity(word_count);
    for i in 0..word_count {
        out.push(WORDS[(i + offset) % WORDS.len()]);
    }
    let mut s = out.join(" ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s.push('.');
    s
}

/// Generate `n` sentences.
fn gen_sentences(n: usize) -> String {
    let mut sentences = Vec::with_capacity(n);
    for i in 0..n {
        // Vary sentence length between 8 and 15 words
        let len = 8 + (i * 3) % 8;
        sentences.push(gen_sentence(len, i * 7));
    }
    sentences.join(" ")
}

/// Generate `n` paragraphs.
fn gen_paragraphs(n: usize) -> String {
    let mut paragraphs = Vec::with_capacity(n);
    for i in 0..n {
        // Each paragraph has 3-5 sentences
        let count = 3 + (i * 2) % 3;
        let mut sentences = Vec::with_capacity(count);
        for j in 0..count {
            let len = 8 + ((i * 5 + j * 3) % 8);
            sentences.push(gen_sentence(len, i * 17 + j * 11));
        }
        paragraphs.push(sentences.join(" "));
    }
    paragraphs.join("\n\n")
}

/// Shortcode: `{{< lorem >}}`, `{{< lorem words=50 >}}`,
/// `{{< lorem sentences=3 >}}`, `{{< lorem paragraphs=2 >}}`.
///
/// Default: 1 paragraph.
#[plugin_fn]
pub fn shortcode(Json(ctx): Json<ShortcodeContext>) -> FnResult<Json<Option<String>>> {
    if ctx.name != "lorem" {
        return Ok(Json(None));
    }

    // Check kwargs first
    if let Some(n) = ctx.kwargs.get("words") {
        let n: usize = n.parse().unwrap_or(50);
        return Ok(Json(Some(gen_words(n))));
    }
    if let Some(n) = ctx.kwargs.get("sentences") {
        let n: usize = n.parse().unwrap_or(3);
        return Ok(Json(Some(gen_sentences(n))));
    }
    if let Some(n) = ctx.kwargs.get("paragraphs") {
        let n: usize = n.parse().unwrap_or(1);
        return Ok(Json(Some(gen_paragraphs(n))));
    }

    // Positional: {{< lorem 3 >}} means 3 paragraphs
    if let Some(n_str) = ctx.args.first() {
        let n: usize = n_str.parse().unwrap_or(1);
        return Ok(Json(Some(gen_paragraphs(n))));
    }

    // Default: 1 paragraph
    Ok(Json(Some(gen_paragraphs(1))))
}
