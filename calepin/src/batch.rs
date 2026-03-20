//! Batch rendering: parallel processing of multiple .qmd files.

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct BatchJob {
    pub input: String,
    pub output: Option<String>,
    pub format: Option<String>,
    #[serde(default)]
    pub overrides: Vec<String>,
}

#[derive(Serialize)]
pub struct BatchResult {
    pub input: String,
    pub output: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#abstract: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn run_batch(manifest_source: &str, write_files: bool, quiet: bool) -> Result<()> {
    let json = if manifest_source == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)
            .context("Failed to read manifest from stdin")?;
        buf
    } else {
        fs::read_to_string(manifest_source)
            .with_context(|| format!("Failed to read manifest: {}", manifest_source))?
    };

    let jobs: Vec<BatchJob> = serde_json::from_str(&json)
        .context("Failed to parse batch manifest JSON")?;

    let results: Vec<BatchResult> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs.iter().map(|job| {
            s.spawn(|| render_one_job(job, write_files, quiet))
        }).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let out = serde_json::to_string_pretty(&results)?;
    println!("{}", out);

    Ok(())
}

fn render_one_job(job: &BatchJob, write_files: bool, quiet: bool) -> BatchResult {
    let input = Path::new(&job.input);

    match render_job_inner(job, input, write_files, quiet) {
        Ok(result) => result,
        Err(e) => BatchResult {
            input: job.input.clone(),
            output: job.output.clone().unwrap_or_default(),
            status: "error".to_string(),
            title: None,
            date: None,
            subtitle: None,
            r#abstract: None,
            body: None,
            error: Some(format!("{:#}", e)),
        },
    }
}

fn render_job_inner(
    job: &BatchJob,
    input: &Path,
    write_files: bool,
    quiet: bool,
) -> Result<BatchResult> {
    let (output_path, final_output) = crate::render_file(
        input,
        job.output.as_ref().map(Path::new),
        job.format.as_deref(),
        &job.overrides,
    )?;

    // Extract metadata for the result
    // Re-read YAML to get metadata without re-rendering
    let input_text = fs::read_to_string(input)?;
    let (metadata, _) = crate::parse::yaml::split_yaml(&input_text)?;

    if write_files {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&output_path, &final_output)
            .with_context(|| format!("Failed to write: {}", output_path.display()))?;
        if !quiet {
            eprintln!("→ {}", output_path.display());
        }
    }

    Ok(BatchResult {
        input: job.input.clone(),
        output: output_path.display().to_string(),
        status: "ok".to_string(),
        title: metadata.title,
        date: metadata.date,
        subtitle: metadata.subtitle,
        r#abstract: metadata.abstract_text,
        body: if write_files { None } else { Some(final_output) },
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_job_deserialize() {
        let json = r#"[
            {"input": "a.qmd"},
            {"input": "b.qmd", "output": "out/b.html", "format": "html", "overrides": ["toc=false"]}
        ]"#;
        let jobs: Vec<BatchJob> = serde_json::from_str(json).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].input, "a.qmd");
        assert!(jobs[0].output.is_none());
        assert!(jobs[0].overrides.is_empty());
        assert_eq!(jobs[1].output.as_deref(), Some("out/b.html"));
        assert_eq!(jobs[1].format.as_deref(), Some("html"));
        assert_eq!(jobs[1].overrides, vec!["toc=false"]);
    }

    #[test]
    fn test_batch_result_serialize() {
        let result = BatchResult {
            input: "test.qmd".to_string(),
            output: "test.html".to_string(),
            status: "ok".to_string(),
            title: Some("Hello".to_string()),
            date: None,
            subtitle: None,
            r#abstract: None,
            body: None,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"title\":\"Hello\""));
        // None fields should be skipped
        assert!(!json.contains("\"date\""));
        assert!(!json.contains("\"body\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_batch_result_error_serialize() {
        let result = BatchResult {
            input: "bad.qmd".to_string(),
            output: String::new(),
            status: "error".to_string(),
            title: None,
            date: None,
            subtitle: None,
            r#abstract: None,
            body: None,
            error: Some("file not found".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"error\":\"file not found\""));
        assert!(json.contains("\"status\":\"error\""));
    }

    #[test]
    fn test_render_one_job_nonexistent_file() {
        let job = BatchJob {
            input: "nonexistent_file_xyz.qmd".to_string(),
            output: None,
            format: None,
            overrides: vec![],
        };
        let result = render_one_job(&job, false, true);
        assert_eq!(result.status, "error");
        assert!(result.error.is_some());
    }

    #[test]
    fn test_batch_render_and_stdout() {
        // Create temp files
        let dir = std::env::temp_dir().join("calepin_batch_test");
        fs::create_dir_all(&dir).unwrap();

        let qmd1 = dir.join("a.qmd");
        let qmd2 = dir.join("b.qmd");
        fs::write(&qmd1, "---\ntitle: Alpha\n---\nHello A").unwrap();
        fs::write(&qmd2, "---\ntitle: Beta\n---\nHello B").unwrap();

        // Test stdout mode (write_files=false)
        let job1 = BatchJob {
            input: qmd1.display().to_string(),
            output: None,
            format: Some("html".to_string()),
            overrides: vec![],
        };
        let job2 = BatchJob {
            input: qmd2.display().to_string(),
            output: None,
            format: Some("html".to_string()),
            overrides: vec![],
        };

        let results: Vec<BatchResult> = std::thread::scope(|s| {
            let jobs = vec![&job1, &job2];
            let handles: Vec<_> = jobs.iter().map(|job| {
                s.spawn(|| render_one_job(job, false, true))
            }).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(r.status, "ok");
            assert!(r.body.is_some(), "stdout mode should include body");
            assert!(r.title.is_some());
        }

        // Clean up
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_batch_partial_failure() {
        let dir = std::env::temp_dir().join("calepin_batch_partial");
        fs::create_dir_all(&dir).unwrap();

        let qmd = dir.join("good.qmd");
        fs::write(&qmd, "---\ntitle: Good\n---\nContent").unwrap();

        let job_good = BatchJob {
            input: qmd.display().to_string(),
            output: None,
            format: Some("html".to_string()),
            overrides: vec![],
        };
        let job_bad = BatchJob {
            input: "totally_missing_file.qmd".to_string(),
            output: None,
            format: None,
            overrides: vec![],
        };

        let results: Vec<BatchResult> = std::thread::scope(|s| {
            let handles = vec![
                s.spawn(|| render_one_job(&job_good, false, true)),
                s.spawn(|| render_one_job(&job_bad, false, true)),
            ];
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let ok_count = results.iter().filter(|r| r.status == "ok").count();
        let err_count = results.iter().filter(|r| r.status == "error").count();
        assert_eq!(ok_count, 1);
        assert_eq!(err_count, 1);

        fs::remove_dir_all(&dir).ok();
    }
}
