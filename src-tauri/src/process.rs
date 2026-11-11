//! Programs started by store plugins with the `process` permission (e.g. the
//! command line tools of AI agents). They always run in the vault folder;
//! their output is streamed to the plugin through a channel, and they are
//! killed when the plugin asks, or when the vault is closed or switched.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dvoix_core::process::{Utf8Decoder, resolve_program};
use serde::Serialize;
use tauri::ipc::Channel;

/// What the plugin gets from a running program.
#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum ProcessEvent {
    Stdout(String),
    Stderr(String),
    /// The exit code; `None` when the program was killed by a signal.
    Exit(Option<i32>),
}

#[derive(Default)]
pub struct Processes {
    next_id: AtomicU32,
    running: Mutex<HashMap<u32, Arc<Mutex<Child>>>>,
}

pub struct SpawnSpec {
    pub command: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub env: HashMap<String, String>,
}

impl Processes {
    pub fn spawn(
        self: &Arc<Self>,
        cwd: &std::path::Path,
        spec: SpawnSpec,
        events: Channel<ProcessEvent>,
    ) -> Result<u32, String> {
        let pathext = if cfg!(windows) {
            std::env::var("PATHEXT").ok()
        } else {
            None
        };
        let program = resolve_program(
            &spec.command,
            cwd,
            std::env::var_os("PATH").as_deref(),
            pathext.as_deref(),
        )
        .ok_or_else(|| format!("Program not found: {}", spec.command))?;

        let mut cmd = Command::new(program);
        cmd.args(&spec.args)
            .envs(&spec.env)
            .current_dir(cwd)
            .stdin(if spec.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Otherwise every start flashes a console window.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{} could not be started: {e}", spec.command))?;

        if let (Some(text), Some(mut stdin)) = (spec.stdin, child.stdin.take()) {
            // In its own thread: a program that writes a lot before reading
            // all of its input would block this one otherwise.
            thread::spawn(move || {
                let _ = stdin.write_all(text.as_bytes());
            });
        }
        // Output is collected over a short window and sent as one event per
        // stream, so a chatty program costs a handful of webview calls per
        // second instead of one per read.
        let (tx, rx) = mpsc::channel::<ProcessEvent>();
        let flusher = flush_events(rx, events.clone());
        let readers = [
            child
                .stdout
                .take()
                .map(|s| pump(s, tx.clone(), ProcessEvent::Stdout)),
            child
                .stderr
                .take()
                .map(|s| pump(s, tx.clone(), ProcessEvent::Stderr)),
        ];
        drop(tx);

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let child = Arc::new(Mutex::new(child));
        self.running
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, Arc::clone(&child));

        let this = Arc::clone(self);
        thread::spawn(move || {
            // The lock is only held while asking whether the program ended,
            // never while waiting, so `kill` can take it at any time. The
            // pause grows to 50 ms, so a long-running program is not asked
            // twenty times a second.
            let mut pause = Duration::from_millis(1);
            let status = loop {
                match child.lock().map(|mut c| c.try_wait()) {
                    Ok(Ok(Some(status))) => break Some(status),
                    Ok(Ok(None)) => {
                        thread::sleep(pause);
                        pause = (pause * 2).min(Duration::from_millis(50));
                    }
                    _ => break None,
                }
            };
            // All output first, so the plugin sees the exit last.
            for r in readers.into_iter().flatten() {
                let _ = r.join();
            }
            let _ = flusher.join();
            if let Ok(mut running) = this.running.lock() {
                running.remove(&id);
            }
            let _ = events.send(ProcessEvent::Exit(status.and_then(|s| s.code())));
        });
        Ok(id)
    }

    pub fn kill(&self, id: u32) {
        let child = self.running.lock().ok().and_then(|r| r.get(&id).cloned());
        if let Some(child) = child
            && let Ok(mut c) = child.lock()
        {
            let _ = c.kill();
        }
    }

    pub fn kill_all(&self) {
        let ids: Vec<u32> = self
            .running
            .lock()
            .map(|r| r.keys().copied().collect())
            .unwrap_or_default();
        for id in ids {
            self.kill(id);
        }
    }
}

/// How long output is collected before it is sent as one event. Long enough
/// that a stream of small writes becomes a handful of events per second,
/// short enough that an interactive program still answers at once.
const FLUSH_WINDOW: Duration = Duration::from_millis(25);

/// Which stream the output being collected belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stream {
    Out,
    Err,
}

/// Sends the output of a program to the plugin, collected over
/// [`FLUSH_WINDOW`]: what arrives within one window becomes one event.
/// stdout and stderr stay separate events, in the order they came in.
fn flush_events(
    rx: Receiver<ProcessEvent>,
    events: Channel<ProcessEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut pending: Option<(Stream, String)> = None;
        let mut deadline = Instant::now();
        loop {
            // Without output there is nothing to wait for: the thread sleeps
            // until a piece arrives instead of waking up on its own.
            let next = match &pending {
                Some(_) => rx.recv_timeout(deadline.saturating_duration_since(Instant::now())),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match next {
                Ok(ProcessEvent::Stdout(text)) => {
                    append(&mut pending, &mut deadline, Stream::Out, text);
                }
                Ok(ProcessEvent::Stderr(text)) => {
                    append(&mut pending, &mut deadline, Stream::Err, text);
                }
                // Exit never travels this way; it is sent after this thread
                // ended, so the plugin sees it last.
                Ok(ProcessEvent::Exit(_)) => {}
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if Instant::now() >= deadline
                && let Some((stream, text)) = pending.take()
            {
                let _ = events.send(wrap(stream, text));
            }
        }
        if let Some((stream, text)) = pending {
            let _ = events.send(wrap(stream, text));
        }
    })
}

/// Turns collected output back into the event of its stream.
fn wrap(stream: Stream, text: String) -> ProcessEvent {
    match stream {
        Stream::Out => ProcessEvent::Stdout(text),
        Stream::Err => ProcessEvent::Stderr(text),
    }
}

/// Adds a piece of output to the event being collected. A different stream
/// closes the one before it, so the order of the output stays as it was.
fn append(
    pending: &mut Option<(Stream, String)>,
    deadline: &mut Instant,
    stream: Stream,
    text: String,
) {
    match pending {
        Some((kind, buf)) if *kind == stream => buf.push_str(&text),
        _ => {
            *pending = Some((stream, text));
            *deadline = Instant::now() + FLUSH_WINDOW;
        }
    }
}

fn pump(
    mut source: impl Read + Send + 'static,
    tx: Sender<ProcessEvent>,
    wrap: fn(String) -> ProcessEvent,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut decoder = Utf8Decoder::default();
        let mut buf = [0u8; 8192];
        loop {
            match source.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = decoder.push(&buf[..n]);
                    if !text.is_empty() {
                        let _ = tx.send(wrap(text));
                    }
                }
            }
        }
        let rest = decoder.finish();
        if !rest.is_empty() {
            let _ = tx.send(wrap(rest));
        }
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tauri::ipc::InvokeResponseBody;

    fn run(spec: SpawnSpec) -> (Arc<Processes>, u32, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel();
        let events = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                let _ = tx.send(json);
            }
            Ok(())
        });
        let processes = Arc::new(Processes::default());
        let dir = std::env::temp_dir();
        let id = processes.spawn(&dir, spec, events).expect("spawn");
        (processes, id, rx)
    }

    fn collect(rx: &mpsc::Receiver<String>) -> Vec<String> {
        let mut out = vec![];
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(10)) {
            let exit = e.contains("\"exit\"");
            out.push(e);
            if exit {
                break;
            }
        }
        out
    }

    #[test]
    fn streams_stdin_through_and_reports_exit_last() {
        let spec = SpawnSpec {
            command: "cat".into(),
            args: vec![],
            stdin: Some("héllo".into()),
            env: HashMap::new(),
        };
        let (_, _, rx) = run(spec);
        let events = collect(&rx);
        assert_eq!(
            events.first().map(String::as_str),
            Some(r#"{"event":"stdout","data":"héllo"}"#)
        );
        assert_eq!(
            events.last().map(String::as_str),
            Some(r#"{"event":"exit","data":0}"#)
        );
    }

    #[test]
    fn kill_ends_a_running_program() {
        let spec = SpawnSpec {
            command: "sleep".into(),
            args: vec!["30".into()],
            stdin: None,
            env: HashMap::new(),
        };
        let (processes, id, rx) = run(spec);
        processes.kill(id);
        assert_eq!(
            collect(&rx).last().map(String::as_str),
            Some(r#"{"event":"exit","data":null}"#)
        );
    }

    #[test]
    fn unknown_program_is_an_error() {
        let processes = Arc::new(Processes::default());
        let spec = SpawnSpec {
            command: "no-such-program-dvoix".into(),
            args: vec![],
            stdin: None,
            env: HashMap::new(),
        };
        let err = processes
            .spawn(&std::env::temp_dir(), spec, Channel::new(|_| Ok(())))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    /// Many small writes become a handful of events, and all the text still
    /// arrives, in order, before the exit.
    #[test]
    fn many_small_writes_are_collected_into_few_events() {
        let spec = SpawnSpec {
            command: "sh".into(),
            args: vec![
                "-c".into(),
                "i=0; while [ $i -lt 200 ]; do printf 'line %s\\n' \"$i\"; i=$((i+1)); done".into(),
            ],
            stdin: None,
            env: HashMap::new(),
        };
        let (_, _, rx) = run(spec);
        let events = collect(&rx);
        let outputs: Vec<&str> = events
            .iter()
            .filter(|e| e.contains("\"stdout\""))
            .map(String::as_str)
            .collect();
        assert!(
            outputs.len() < 100,
            "200 lines should not cost 200 events: {}",
            outputs.len()
        );
        let mut text = String::new();
        for e in &outputs {
            let value: serde_json::Value = serde_json::from_str(e).expect("json");
            text.push_str(value["data"].as_str().expect("text"));
        }
        for i in 0..200 {
            assert!(text.contains(&format!("line {i}\n")), "line {i} is missing");
        }
        assert_eq!(
            events.last().map(String::as_str),
            Some(r#"{"event":"exit","data":0}"#)
        );
    }
}
