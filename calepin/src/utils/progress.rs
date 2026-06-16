use std::io::{self, IsTerminal};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Clone)]
pub struct ProgressManager {
    quiet: bool,
    interactive: bool,
    multi: Option<MultiProgress>,
}

impl ProgressManager {
    pub fn new(quiet: bool) -> Self {
        if quiet {
            return Self {
                quiet,
                interactive: false,
                multi: None,
            };
        }
        let interactive = io::stderr().is_terminal();
        let multi = interactive
            .then(|| MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(12)));
        Self {
            quiet,
            interactive,
            multi,
        }
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
        if !manager.interactive {
            eprintln!("{message}");
            return Self::hidden(manager.quiet);
        }

        let Some(multi) = manager.multi.clone() else {
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

impl Drop for Progress {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(bar) = &self.bar {
                bar.finish_and_clear();
            }
        }
    }
}
