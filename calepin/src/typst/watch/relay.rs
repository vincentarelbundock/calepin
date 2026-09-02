use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use crate::typst::run::strip_ansi_codes;

pub(super) fn relay_typst_watch_output<R, W>(reader: R, writer: W) -> io::Result<()>
where
    R: Read,
    W: Write,
{
    relay_typst_watch_output_inner(reader, writer, None)
}

struct TypstWatchOutputRelay<W> {
    writer: W,
    seen_watching: bool,
    seen_writing: bool,
    pending_blank: bool,
}

impl<W: Write> TypstWatchOutputRelay<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            seen_watching: false,
            seen_writing: false,
            pending_blank: false,
        }
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            self.pending_blank = true;
            return Ok(());
        }

        let should_write = match typst_watch_line(line) {
            TypstWatchLine::Watching => {
                let first = !self.seen_watching;
                self.seen_watching = true;
                first
            }
            TypstWatchLine::Writing => {
                let first = !self.seen_writing;
                self.seen_writing = true;
                first
            }
            TypstWatchLine::Status => true,
            TypstWatchLine::Other => {
                if self.pending_blank {
                    writeln!(self.writer)?;
                }
                true
            }
        };
        self.pending_blank = false;

        if should_write {
            writeln!(self.writer, "{line}")?;
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TypstWatchLine {
    Watching,
    Writing,
    Status,
    Other,
}

pub(super) fn typst_watch_line(line: &str) -> TypstWatchLine {
    let line = strip_ansi_codes(line);
    let line = line.trim();

    if line.starts_with("watching ") {
        TypstWatchLine::Watching
    } else if line.starts_with("writing to ") {
        TypstWatchLine::Writing
    } else if line.starts_with('[') && line.contains("] ") {
        TypstWatchLine::Status
    } else {
        TypstWatchLine::Other
    }
}

pub(super) fn typst_watch_output_path(line: &str) -> Option<PathBuf> {
    let line = strip_ansi_codes(line);
    let line = line.trim();
    let marker = "writing to ";
    let start = line.find(marker)? + marker.len();
    let path = line[start..].trim();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

pub(super) fn relay_typst_watch_output_with_events<R, W>(
    reader: R,
    writer: W,
    on_write: Sender<PathBuf>,
) -> io::Result<()>
where
    R: Read,
    W: Write,
{
    relay_typst_watch_output_inner(reader, writer, Some(on_write))
}

fn relay_typst_watch_output_inner<R, W>(
    reader: R,
    writer: W,
    on_write: Option<Sender<PathBuf>>,
) -> io::Result<()>
where
    R: Read,
    W: Write,
{
    let mut reader = BufReader::new(reader);
    let mut relay = TypstWatchOutputRelay::new(writer);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if let Some(on_write) = &on_write {
            if let Some(path) = typst_watch_output_path(&line) {
                on_write.send(path).ok();
            }
        }
        relay.write_line(&line)?;
    }

    relay.finish()
}

pub(super) fn join_relay(name: &str, handle: thread::JoinHandle<io::Result<()>>) {
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            cwarn!("failed to relay typst watch {name}: {}", error);
        }
        Err(_) => {
            cwarn!("typst watch {name} relay panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_typst_watch_output_compacts_status_lines() {
        let input = "\
watching example.typ
writing to example.pdf

[10:17:43] compiling ...

watching example.typ
writing to example.pdf

[10:17:43] compiled successfully in 88.22 ms

watching example.typ
writing to example.pdf

[10:17:53] compiling ...
";
        let mut output = Vec::new();

        relay_typst_watch_output(input.as_bytes(), &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\
watching example.typ
writing to example.pdf
[10:17:43] compiling ...
[10:17:43] compiled successfully in 88.22 ms
[10:17:53] compiling ...
"
        );
    }

    #[test]
    fn relay_typst_watch_output_preserves_diagnostic_spacing() {
        let input = "\
error: failed

hint: check this
";
        let mut output = Vec::new();

        relay_typst_watch_output(input.as_bytes(), &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\
error: failed

hint: check this
"
        );
    }
}
