//! `calepin init gibberish` -- generate test content (gibberish .qmd files).

use anyhow::{Context, Result};
use crate::utils::lipsum;

pub fn handle_new_gibberish(
    dir: &std::path::Path,
    num_files: usize,
    num_paragraphs: usize,
    complexity: u8,
) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create directory: {}", dir.display()))?;

    let code_chunks = complexity >= 1;
    let extras = complexity >= 2;

    if extras {
        let bib = generate_library_bib();
        std::fs::write(dir.join("library.bib"), bib)
            .context("Failed to write library.bib")?;
    }

    for i in 0..num_files {
        let filename = format!("page-{:03}.qmd", i + 1);
        let path = dir.join(&filename);

        let title = lipsum::lipsum_words(3 + (i * 7) % 4);
        let mut body = String::new();

        // Distribute paragraphs across 5 headings with 3 subheadings each
        let section_ids: Vec<String> = (0..5)
            .map(|h| format!("sec-{}-{}", i + 1, h + 1))
            .collect();
        let heading_titles: Vec<String> = (0..5)
            .map(|h| lipsum::lipsum_words(2 + ((i + h) * 5) % 4))
            .collect();
        let sub_titles: Vec<String> = (0..15)
            .map(|s| lipsum::lipsum_words(2 + ((i + s) * 3) % 4))
            .collect();

        let mut para_idx = 0;
        let mut table_idx = 0usize;
        let paras_per_section = num_paragraphs / 5;

        for h in 0..5 {
            if extras {
                body.push_str(&format!(
                    "## {} {{#{}}} \n\n",
                    heading_titles[h], section_ids[h]
                ));
            } else {
                body.push_str(&format!("## {}\n\n", heading_titles[h]));
            }

            let paras_before_subs = paras_per_section / 4;
            for _ in 0..paras_before_subs {
                body.push_str(&lipsum::lipsum_paragraphs(1));
                body.push_str("\n\n");
                para_idx += 1;
            }

            let paras_per_sub = (paras_per_section - paras_before_subs) / 3;
            for s in 0..3 {
                let sub_idx = h * 3 + s;
                body.push_str(&format!("### {}\n\n", sub_titles[sub_idx]));
                let count = if s == 2 {
                    paras_per_section - paras_before_subs - paras_per_sub * 2
                } else {
                    paras_per_sub
                };
                for _ in 0..count {
                    // Vary the lipsum by using different offsets
                    let offset = para_idx * 17 + i * 31;
                    let sentence_count = 3 + (offset % 4);
                    let mut sentences = Vec::new();
                    for j in 0..sentence_count {
                        let len = 8 + ((offset + j * 5) % 10);
                        sentences.push(lipsum::lipsum_sentence(len, offset + j * 11));
                    }

                    if extras {
                        // Inject footnotes, cross-refs, and citations into the paragraph
                        let mut para_text = sentences.join(" ");
                        let seed = para_idx * 7 + i * 13;

                        // Footnote every ~5th paragraph
                        if para_idx % 5 == 1 {
                            let fn_text = lipsum::lipsum_sentence(
                                6 + (seed % 5),
                                seed + 3,
                            );
                            para_text.push_str(&format!("^[{}]", fn_text));
                        }

                        // Citation every ~3rd paragraph
                        if para_idx % 3 == 0 {
                            let cite_key = BIB_KEYS[seed % BIB_KEYS.len()];
                            // Vary between inline @key and parenthetical [@key]
                            if seed % 2 == 0 {
                                para_text = format!(
                                    "As shown by @{}, {}",
                                    cite_key, para_text
                                );
                            } else {
                                para_text.push_str(&format!(" [@{}]", cite_key));
                            }
                        }

                        // Cross-ref to another section every ~7th paragraph
                        if para_idx % 7 == 3 {
                            let ref_sec = &section_ids[(h + 1) % 5];
                            para_text.push_str(&format!(
                                " See also @{}.",
                                ref_sec
                            ));
                        }

                        body.push_str(&para_text);
                    } else {
                        body.push_str(&sentences.join(" "));
                    }
                    body.push_str("\n\n");

                    // Insert a code chunk every 4th paragraph, alternating R and Python
                    if code_chunks && para_idx % 4 == 2 {
                        if para_idx % 8 < 4 {
                            body.push_str(&build_r_chunk(offset));
                        } else {
                            body.push_str(&build_python_chunk(offset));
                        }
                        body.push('\n');
                    }

                    // Insert a table every ~10th paragraph (complexity 2 only)
                    if extras && para_idx % 10 == 6 {
                        table_idx += 1;
                        body.push_str(&build_gibberish_table(i, table_idx));
                        body.push('\n');
                    }

                    para_idx += 1;
                }
            }
        }

        // Any remaining paragraphs
        while para_idx < num_paragraphs {
            let offset = para_idx * 17 + i * 31;
            let sentence_count = 3 + (offset % 4);
            let mut sentences = Vec::new();
            for j in 0..sentence_count {
                let len = 8 + ((offset + j * 5) % 10);
                sentences.push(lipsum::lipsum_sentence(len, offset + j * 11));
            }
            body.push_str(&sentences.join(" "));
            body.push_str("\n\n");
            para_idx += 1;
        }

        let front_matter = if extras {
            format!(
                "---\ntitle: \"{}\"\nbibliography: library.bib\n---",
                title
            )
        } else {
            format!("---\ntitle: \"{}\"\n---", title)
        };

        let content = format!("{}\n\n{}", front_matter, body.trim_end());
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }

    let desc = match complexity {
        0 => "prose only",
        1 => "prose + code chunks",
        _ => "prose + code chunks + cross-refs/footnotes/citations/tables",
    };
    eprintln!(
        "Generated {} files in {}/ ({} paragraphs, 5 sections x 3 subsections, {})",
        num_files, dir.display(), num_paragraphs, desc
    );
    Ok(())
}

const R_SNIPPETS: &[&str] = &[
    "library(ggplot2)\n\ndat <- data.frame(\n  x = rnorm(100),\n  y = rnorm(100),\n  group = sample(letters[1:3], 100, replace = TRUE)\n)\n\nggplot(dat, aes(x, y, color = group)) +\n  geom_point(size = 2) +\n  theme_minimal()",
    "fit <- lm(mpg ~ wt + hp + factor(cyl), data = mtcars)\nsummary(fit)\nconfint(fit)",
    "library(dplyr)\n\nmtcars |>\n  group_by(cyl) |>\n  summarise(\n    mean_mpg = mean(mpg),\n    sd_mpg = sd(mpg),\n    n = n()\n  ) |>\n  arrange(desc(mean_mpg))",
    "x <- seq(-2 * pi, 2 * pi, length.out = 200)\nplot(x, sin(x), type = \"l\", col = \"steelblue\", lwd = 2)\nlines(x, cos(x), col = \"coral\", lwd = 2)\nlegend(\"topright\", c(\"sin\", \"cos\"), col = c(\"steelblue\", \"coral\"), lwd = 2)",
    "mat <- matrix(rnorm(20), nrow = 4)\ncolnames(mat) <- paste0(\"V\", 1:5)\nrownames(mat) <- paste0(\"obs\", 1:4)\nknitr::kable(mat, digits = 2)",
];

const PYTHON_SNIPPETS: &[&str] = &[
    "import numpy as np\nimport matplotlib.pyplot as plt\n\nx = np.linspace(0, 10, 100)\ny = np.sin(x) * np.exp(-x / 5)\n\nplt.figure(figsize=(8, 4))\nplt.plot(x, y, linewidth=2)\nplt.xlabel(\"x\")\nplt.ylabel(\"f(x)\")\nplt.title(\"Damped oscillation\")\nplt.show()",
    "import pandas as pd\n\ndf = pd.DataFrame({\n    \"name\": [\"Alice\", \"Bob\", \"Charlie\", \"Diana\"],\n    \"score\": [92, 87, 78, 95],\n    \"grade\": [\"A\", \"B\", \"C\", \"A\"],\n})\ndf.describe()",
    "from collections import Counter\n\nwords = \"the quick brown fox jumps over the lazy dog\".split()\ncounts = Counter(words)\nfor word, count in counts.most_common(5):\n    print(f\"{word}: {count}\")",
    "import numpy as np\n\nA = np.array([[1, 2], [3, 4]])\nb = np.array([5, 6])\nx = np.linalg.solve(A, b)\nprint(f\"Solution: {x}\")\nprint(f\"Verify: {A @ x}\")",
    "def fibonacci(n):\n    a, b = 0, 1\n    for _ in range(n):\n        yield a\n        a, b = b, a + b\n\nlist(fibonacci(10))",
];

fn build_r_chunk(seed: usize) -> String {
    let snippet = R_SNIPPETS[seed % R_SNIPPETS.len()];
    format!("```r\n{}\n```\n", snippet)
}

fn build_python_chunk(seed: usize) -> String {
    let snippet = PYTHON_SNIPPETS[seed % PYTHON_SNIPPETS.len()];
    format!("```python\n{}\n```\n", snippet)
}

const BIB_KEYS: &[&str] = &[
    "lorem2019", "consectetur2021", "veniam2020", "fugiat2022",
    "blanditiis2023", "repellendus2018", "sapiente2020", "asperiores2024",
];

fn generate_library_bib() -> String {
    r#"@article{lorem2019,
  author  = {Lorem, Ipsum and Dolor, Sit Amet},
  title   = {On the Convergence of Adipiscing Processes in Elit Manifolds},
  journal = {Journal of Gibberish Studies},
  year    = {2019},
  volume  = {42},
  number  = {3},
  pages   = {217--234},
}

@book{consectetur2021,
  author    = {Consectetur, Adipiscing},
  title     = {Foundations of Eiusmod Tempor Theory},
  publisher = {Incididunt University Press},
  year      = {2021},
  edition   = {2nd},
}

@inproceedings{veniam2020,
  author    = {Veniam, Quis and Nostrud, Exercitation and Ullamco, Laboris},
  title     = {Aliquip Methods for Commodo Consequat Estimation},
  booktitle = {Proceedings of the International Conference on Duis Aute},
  year      = {2020},
  pages     = {112--119},
}

@article{fugiat2022,
  author  = {Fugiat, Nulla and Pariatur, Excepteur},
  title   = {Occaecat Cupidatat and Non-Proident Structures: A Review},
  journal = {Annals of Culpa Officia},
  year    = {2022},
  volume  = {15},
  pages   = {88--107},
}

@techreport{blanditiis2023,
  author      = {Blanditiis, Praesentium and Voluptatum, Deleniti},
  title       = {Corrupti Quos Dolores: Benchmarking Molestias Frameworks},
  institution = {Obcaecati Research Lab},
  year        = {2023},
  number      = {TR-2023-07},
}

@article{repellendus2018,
  author  = {Repellendus, Temporibus and Quibusdam, Officiis and Debitis, Aut},
  title   = {Rerum Necessitatibus in Saepe Eveniet Networks},
  journal = {Computational Voluptates Research},
  year    = {2018},
  volume  = {9},
  number  = {1},
  pages   = {33--51},
}

@phdthesis{sapiente2020,
  author = {Sapiente, Delectus},
  title  = {Reiciendis Voluptatibus and Their Applications to Maiores Alias Systems},
  school = {University of Perferendis},
  year   = {2020},
}

@article{asperiores2024,
  author  = {Asperiores, Repellat and Ipsum, Dolor},
  title   = {Stochastic Adipiscing with Eiusmod Constraints Under Tempor Uncertainty},
  journal = {Journal of Gibberish Studies},
  year    = {2024},
  volume  = {47},
  number  = {1},
  pages   = {1--29},
}
"#.to_string()
}

const TABLE_HEADERS: &[&[&str]] = &[
    &["Method", "Accuracy", "Precision", "Recall", "F1"],
    &["Parameter", "Value", "Std. Error", "t-stat", "p-value"],
    &["Model", "AIC", "BIC", "RMSE", "R\u{00B2}"],
    &["Dataset", "n", "Mean", "Median", "SD"],
    &["Configuration", "Time (s)", "Memory (MB)", "Iterations", "Status"],
];

fn build_gibberish_table(file_idx: usize, table_idx: usize) -> String {
    let seed = file_idx * 13 + table_idx * 7;
    let headers = TABLE_HEADERS[seed % TABLE_HEADERS.len()];
    let label = format!("tbl-{}-{}", file_idx + 1, table_idx);

    let mut out = String::new();

    // Header row
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n");

    // Separator
    out.push('|');
    for _ in headers {
        out.push_str("--------|");
    }
    out.push('\n');

    // 3-5 data rows
    let num_rows = 3 + (seed % 3);
    let row_labels = ["Baseline", "Eiusmod-A", "Tempor-B", "Hybrid", "Veniam-C"];
    for r in 0..num_rows {
        out.push_str("| ");
        out.push_str(row_labels[r % row_labels.len()]);
        for c in 1..headers.len() {
            let val = ((seed + r * 17 + c * 11) % 900) as f64 / 10.0 + 1.0;
            out.push_str(&format!(" | {:.1}", val));
        }
        out.push_str(" |\n");
    }

    // Caption with cross-ref label
    let caption = lipsum::lipsum_sentence(5 + (seed % 4), seed + 2);
    out.push_str(&format!(
        "\n: {} {{#{}}} \n",
        caption, label
    ));

    out
}
