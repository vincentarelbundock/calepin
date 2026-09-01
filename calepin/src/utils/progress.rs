use std::env;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

const PROGRESS_ENV_VAR: &str = "CALEPIN_PROGRESS";

#[derive(Clone)]
pub struct ProgressManager {
    quiet: bool,
    multi: Option<MultiProgress>,
}

impl ProgressManager {
    pub fn new(quiet: bool) -> Self {
        Self::with_draw_target(
            quiet,
            ProgressPreference::from_env(),
            ProgressDrawTarget::stderr_with_hz(12),
        )
    }

    fn with_draw_target(
        quiet: bool,
        preference: ProgressPreference,
        draw_target: ProgressDrawTarget,
    ) -> Self {
        if quiet {
            return Self { quiet, multi: None };
        }

        let multi = match preference {
            ProgressPreference::Auto => {
                let multi = MultiProgress::with_draw_target(draw_target);
                (!multi.is_hidden()).then_some(multi)
            }
            ProgressPreference::Plain => None,
        };

        Self { quiet, multi }
    }

    pub fn spinner(&self, message: impl Into<String>) -> Progress {
        Progress::from_manager(self, ProgressKind::Spinner, message.into(), 0)
    }

    pub fn bar(&self, message: impl Into<String>, len: u64) -> Progress {
        Progress::from_manager(self, ProgressKind::Bar, message.into(), len)
    }
}

pub struct Progress {
    quiet: bool,
    finished: bool,
    bar: Option<ProgressBar>,
    _multi: Option<MultiProgress>,
}

enum ProgressKind {
    Spinner,
    Bar,
}

impl Progress {
    pub fn spinner(message: impl Into<String>, quiet: bool) -> Self {
        ProgressManager::new(quiet).spinner(message)
    }

    pub fn bar(message: impl Into<String>, len: u64, quiet: bool) -> Self {
        ProgressManager::new(quiet).bar(message, len)
    }

    fn from_manager(
        manager: &ProgressManager,
        kind: ProgressKind,
        message: String,
        len: u64,
    ) -> Self {
        if manager.quiet {
            return Self::hidden(manager.quiet);
        }

        let Some(multi) = manager.multi.clone() else {
            eprintln!("{message}");
            return Self::hidden(manager.quiet);
        };
        let bar = match kind {
            ProgressKind::Spinner => {
                let bar = multi.add(ProgressBar::new_spinner());
                let style = ProgressStyle::with_template("{spinner} {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner())
                    .tick_chars("|/-\\");
                bar.set_style(style);
                bar
            }
            ProgressKind::Bar => {
                let bar = multi.add(ProgressBar::new(len));
                let style = ProgressStyle::with_template(
                    "{spinner} [{elapsed_precise}] [{bar:32.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("#>-")
                .tick_chars("|/-\\");
                bar.set_style(style);
                bar
            }
        };
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(120));

        Self {
            quiet: manager.quiet,
            finished: false,
            bar: Some(bar),
            _multi: Some(multi),
        }
    }

    pub fn set_message(&self, message: impl Into<String>) {
        if let Some(bar) = &self.bar {
            bar.set_message(message.into());
        }
    }

    /// Runs `body` with the spinner cleared, so anything it prints to stderr is
    /// not overwritten by the next redraw.
    pub fn suspend<T>(&self, body: impl FnOnce() -> T) -> T {
        match &self.bar {
            Some(bar) => bar.suspend(body),
            None => body(),
        }
    }

    pub fn inc(&self, delta: u64) {
        if let Some(bar) = &self.bar {
            bar.inc(delta);
        }
    }

    pub fn finish(mut self, message: impl AsRef<str>) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
        self.finished = true;
        if !self.quiet && self.bar.is_none() {
            eprintln!("{}", message.as_ref());
        }
    }

    fn hidden(quiet: bool) -> Self {
        Self {
            quiet,
            finished: false,
            bar: None,
            _multi: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressPreference {
    Auto,
    Plain,
}

impl ProgressPreference {
    fn from_env() -> Self {
        Self::from_env_value(env::var(PROGRESS_ENV_VAR).ok().as_deref())
    }

    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("plain") => Self::Plain,
            _ => Self::Auto,
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(bar) = &self.bar {
                bar.finish_and_clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use indicatif::ProgressDrawTarget;

    use super::*;

    #[test]
    fn progress_preference_defaults_to_auto() {
        assert_eq!(
            ProgressPreference::from_env_value(None),
            ProgressPreference::Auto
        );
        assert_eq!(
            ProgressPreference::from_env_value(Some("")),
            ProgressPreference::Auto
        );
    }

    #[test]
    fn progress_preference_accepts_plain_escape_hatch() {
        for value in ["plain", "PLAIN", " plain "] {
            assert_eq!(
                ProgressPreference::from_env_value(Some(value)),
                ProgressPreference::Plain
            );
        }
    }

    #[test]
    fn auto_progress_uses_indicatif_hidden_detection() {
        let manager = ProgressManager::with_draw_target(
            false,
            ProgressPreference::Auto,
            ProgressDrawTarget::hidden(),
        );

        assert!(manager.multi.is_none());
    }

    #[test]
    fn plain_progress_skips_dynamic_draw_target() {
        let manager = ProgressManager::with_draw_target(
            false,
            ProgressPreference::Plain,
            ProgressDrawTarget::stderr_with_hz(12),
        );

        assert!(manager.multi.is_none());
    }
}
