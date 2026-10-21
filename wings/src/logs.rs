//! Console, install and daemon lines, batched into `Event::Logs`. Live only,
//! nothing is kept: a panel that is not connected misses them.

use std::time::Duration;

use chrono::Utc;
use shared::msg::{Event, LogLine, LogStream};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::store::Store;

const FLUSH: Duration = Duration::from_millis(250);
const BATCH: usize = 256;
/// A longer partial line is sent as is, so one never ends up in memory whole.
const MAX_LINE: usize = 64 * 1024;

/// Sends one server's lines to its batcher. The batcher ends once every clone is dropped.
#[derive(Clone)]
pub struct Logs(mpsc::Sender<LogLine>);

impl Logs {
  pub fn spawn(store: Store, uuid: Uuid) -> Self {
    let (tx, rx) = mpsc::channel(4 * BATCH);
    tokio::spawn(batch(store, uuid, rx));
    Self(tx)
  }

  pub async fn line(&self, stream: LogStream, line: String) {
    let line = LogLine {
      at: Utc::now(),
      stream,
      line,
    };
    // the batcher only ends once every sender is gone
    let _ = self.0.send(line).await;
  }

  /// Wings' own message to the user.
  pub async fn daemon(&self, line: impl Into<String>) {
    self.line(LogStream::Daemon, line.into()).await;
  }
}

async fn batch(store: Store, uuid: Uuid, mut rx: mpsc::Receiver<LogLine>) {
  let mut lines = Vec::new();
  let mut tick = tokio::time::interval(FLUSH);
  loop {
    let closed = tokio::select! {
      n = rx.recv_many(&mut lines, BATCH) => n == 0,
      _ = tick.tick() => false,
    };
    while !lines.is_empty() {
      let rest = lines.split_off(lines.len().min(BATCH));
      store.send(Event::Logs {
        uuid,
        lines: std::mem::replace(&mut lines, rest),
      });
    }
    if closed {
      return;
    }
  }
}

/// Splits raw console output into lines. A `\r` without `\n` overwrites the
/// line like a terminal does, trailing whitespace is dropped.
#[derive(Default)]
pub struct Lines {
  partial: Vec<u8>,
}

impl Lines {
  pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
    self.partial.extend_from_slice(chunk);
    let mut lines = Vec::new();
    while let Some(end) = self.partial.iter().position(|b| *b == b'\n') {
      let raw: Vec<u8> = self.partial.drain(..=end).collect();
      lines.push(clean(&raw));
    }
    if self.partial.len() > MAX_LINE {
      lines.push(clean(&std::mem::take(&mut self.partial)));
    }
    lines
  }

  /// The partial line left at the end of the output.
  pub fn finish(&mut self) -> Option<String> {
    let rest = std::mem::take(&mut self.partial);
    let line = clean(&rest);
    (!line.is_empty()).then_some(line)
  }
}

fn clean(raw: &[u8]) -> String {
  let line = String::from_utf8_lossy(raw);
  let line = line.trim_end();
  line.rsplit('\r').next().unwrap_or_default().to_string()
}

/// Feeds a Docker output stream into `logs` until it ends.
pub async fn follow<S, B, E>(mut output: S, logs: &Logs, stream: LogStream)
where
  S: futures::Stream<Item = Result<B, E>> + Unpin,
  B: AsRef<[u8]>,
{
  use futures::StreamExt;
  let mut lines = Lines::default();
  while let Some(Ok(chunk)) = output.next().await {
    for line in lines.push(chunk.as_ref()) {
      logs.line(stream, line).await;
    }
  }
  if let Some(line) = lines.finish() {
    logs.line(stream, line).await;
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::store::test::memory;

  #[test]
  fn splits_lines() {
    let mut lines = Lines::default();
    assert_eq!(lines.push(b"hel"), Vec::<String>::new());
    assert_eq!(lines.push(b"lo\r\nwor"), vec!["hello"]);
    assert_eq!(
      lines.push(b"ld  \n\n10%\r50%\r100%\n"),
      vec!["world", "", "100%"]
    );
    assert_eq!(lines.push(b"> "), Vec::<String>::new());
    assert_eq!(lines.finish(), Some(">".into()));
    assert_eq!(lines.finish(), None);
    // invalid utf-8 is replaced, not dropped
    assert_eq!(lines.push(b"a\xffb\n"), vec!["a\u{fffd}b"]);
    let long = vec![b'x'; MAX_LINE + 1];
    assert_eq!(lines.push(&long)[0].len(), MAX_LINE + 1);
  }

  #[tokio::test]
  async fn batches_lines() {
    let store = memory();
    let mut events = store.subscribe();
    let uuid = Uuid::new_v4();
    let logs = Logs::spawn(store.clone(), uuid);
    for i in 0..300 {
      logs.daemon(format!("{i}")).await;
    }
    drop(logs);
    let mut sizes = Vec::new();
    let mut all = Vec::new();
    while let Ok(Ok(Event::Logs { uuid: u, lines })) =
      tokio::time::timeout(Duration::from_secs(1), events.recv()).await
    {
      assert_eq!(u, uuid);
      sizes.push(lines.len());
      all.extend(lines.into_iter().map(|l| l.line));
    }
    assert!(sizes.iter().all(|n| *n <= BATCH));
    assert_eq!(all, (0..300).map(|i| i.to_string()).collect::<Vec<_>>());
  }
}
