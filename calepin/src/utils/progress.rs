use std::io::{self, IsTerminal};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

pub struct Progress {
    quiet: bool,
    finished: bool,
    bar: Option<ProgressBar>,
    _multi: Option<MultiProgress>,
}

impl Progress {
    pub fn spinner(message: impl Into<String>, quiet: bool) -> Self {
        let message = message.into();
        if quiet {
            return Self::hidden(quiet);
        }

        if !io::stderr().is_terminal() {
            eprintln!("{message}");
            return Self {
                quiet,
                finished: false,
                bar: None,
                _multi: None,
            };
        }

        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(12));
        let bar = multi.add(ProgressBar::new_spinner());
        let style = ProgressStyle::with_template("{spinner} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("|/-\\");
        bar.set_style(style);
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(120));

        Self {
            quiet,
            finished: false,
            bar: Some(bar),
            _multi: Some(multi),
        }
    }

    pub fn bar(message: impl Into<String>, len: u64, quiet: bool) -> Self {
        let message = message.into();
        if quiet {
            return Self::hidden(quiet);
        }

        if !io::stderr().is_terminal() {
            eprintln!("{message}");
            return Self {
                quiet,
                finished: false,
                bar: None,
                _multi: None,
            };
        }

        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(12));
        let bar = multi.add(ProgressBar::new(len));
        let style = ProgressStyle::with_template(
            "{spinner} [{elapsed_precise}] [{bar:32.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
        .tick_chars("|/-\\");
        bar.set_style(style);
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(120));

        Self {
            quiet,
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
        if !self.quiet {
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
