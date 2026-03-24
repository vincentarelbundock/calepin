//! Lorem ipsum text generation.

const LIPSUM_WORDS: &[&str] = &[
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

pub(crate) fn lipsum_words(n: usize) -> String {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(LIPSUM_WORDS[i % LIPSUM_WORDS.len()]);
    }
    let mut s = out.join(" ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

pub(crate) fn lipsum_sentence(word_count: usize, offset: usize) -> String {
    let mut out = Vec::with_capacity(word_count);
    for i in 0..word_count {
        out.push(LIPSUM_WORDS[(i + offset) % LIPSUM_WORDS.len()]);
    }
    let mut s = out.join(" ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s.push('.');
    s
}

pub(crate) fn lipsum_sentences(n: usize) -> String {
    let mut sentences = Vec::with_capacity(n);
    for i in 0..n {
        let len = 8 + (i * 3) % 8;
        sentences.push(lipsum_sentence(len, i * 7));
    }
    sentences.join(" ")
}

pub(crate) fn lipsum_paragraphs(n: usize) -> String {
    let mut paragraphs = Vec::with_capacity(n);
    for i in 0..n {
        let count = 3 + (i * 2) % 3;
        let mut sentences = Vec::with_capacity(count);
        for j in 0..count {
            let len = 8 + ((i * 5 + j * 3) % 8);
            sentences.push(lipsum_sentence(len, i * 17 + j * 11));
        }
        paragraphs.push(sentences.join(" "));
    }
    paragraphs.join("\n\n")
}
