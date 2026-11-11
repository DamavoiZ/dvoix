//! Programs started by store plugins with the `process` permission (e.g. the
//! command line tools of AI agents). They always run in the vault folder;
//! their output is streamed to the plugin through a channel, and they are
//! killed when the plugin asks, or when the vault is closed or switched.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
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

/// A running program. The reaper thread waits for the exit while holding
/// the lock of `child`; `kill` therefore asks for the lock with `try_lock`
/// and, when the reaper holds it, signals the program by its id instead
/// (see `signal_kill`). The exit is noticed at once, without polling.
struct Running {
    /// The process id, kept next to the handle so a kill needs no lock.
    pid: u32,
    /// Set once the exit was reaped, so a kill never signals a process id
    /// that the system may have handed out again in the meantime.
    exited: AtomicBool,
    child: Mutex<Child>,
}

#[derive(Default)]
pub struct Processes {
    next_id: AtomicU32,
    running: Mutex<HashMap<u32, Arc<Running>>>,
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
        let child = Arc::new(Running {
            pid: child.id(),
            exited: AtomicBool::new(false),
            child: Mutex::new(child),
        });
        self.running
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, Arc::clone(&child));

        let this = Arc::clone(self);
        thread::spawn(move || {
            // A blocking wait: the exit is noticed at once and a running
            // program costs no wakeups at all. The lock stays held while
            // waiting; `kill` does not need it (see `Running`).
            let status = match child.child.lock() {
                Ok(mut c) => {
                    let status = c.wait().ok();
                    // The exit is marked before the lock is freed, so a kill
                    // that cannot take the lock knows the id is still ours.
                    child.exited.store(true, Ordering::SeqCst);
                    status
                }
                Err(_) => None,
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
        let Some(child) = child else { return };
        if child.exited.load(Ordering::SeqCst) {
            return;
        }
        match child.child.try_lock() {
            // The reaper is not waiting: kill through the handle as before.
            Ok(mut c) => {
                let _ = c.kill();
            }
            // The reaper holds the lock in `wait()`: the program has not
            // been reaped yet, so its id is still its own. Signal it by the
            // id; the wait returns with the exit right away.
            Err(_) => {
                if !child.exited.load(Ordering::SeqCst) {
                    signal_kill(child.pid);
                }
            }
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

/// Ends a program by its process id while the reaper holds the handle in
/// `wait()`. On unix SIGKILL, like `Child::kill`; on Windows
/// `TerminateProcess`, the same call `Child::kill` makes.
#[cfg(unix)]
fn signal_kill(pid: u32) {
    // SAFETY: `kill` only sends a signal to the given process id and touches
    // no memory. The id was checked against `exited` right before, so it
    // still belongs to our program.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn signal_kill(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    // SAFETY: the handle comes from `OpenProcess` and is closed again below;
    // the id was checked against `exited` right before.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !handle.is_null() {
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
    }
}

/// How long output is collected before it is sent as one event. Long enough
/// that a stream of small writes becomes a handful of events per second,
/// short enough that an interactive program still answers at once.
const FLUSH_WINDOW: Duration = Duration::from_millis(25);

/// Sends the output of a program to the plugin, collected over
/// [`FLUSH_WINDOW`]: what arrives within one window becomes one event.
/// stdout and stderr stay separate events, in the order they came in.
fn flush_events(
    rx: Receiver<ProcessEvent>,
    events: Channel<ProcessEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // One buffer per stream: a piece of the other stream must never
        // throw away what was collected so far.
        let mut out = String::new();
        let mut err = String::new();
        let mut deadline = Instant::now();
        loop {
            let idle = out.is_empty() && err.is_empty();
            if idle {
                // Without output there is nothing to wait for: the thread
                // sleeps until a piece arrives instead of waking up on its own.
                match rx.recv() {
                    Ok(event) => collect(&mut out, &mut err, &mut deadline, event),
                    Err(_) => break,
                }
            } else {
                // Take everything that is already there first, so a busy
                // program is not woken up once per piece.
                let mut open = true;
                loop {
                    match rx.try_recv() {
                        Ok(event) => collect(&mut out, &mut err, &mut deadline, event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            open = false;
                            break;
                        }
                    }
                }
                let wait = deadline.saturating_duration_since(Instant::now());
                if !wait.is_zero() && open {
                    match rx.recv_timeout(wait) {
                        Ok(event) => collect(&mut out, &mut err, &mut deadline, event),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => open = false,
                    }
                }
                if Instant::now() >= deadline || !open {
                    flush(&mut out, &mut err, &events);
                }
                if !open {
                    break;
                }
            }
        }
        flush(&mut out, &mut err, &events);
    })
}

/// Adds one piece of output to the buffer of its stream. The first piece of
/// a window sets its end; the pieces after it only add to the buffers.
fn collect(out: &mut String, err: &mut String, deadline: &mut Instant, event: ProcessEvent) {
    match event {
        ProcessEvent::Stdout(text) => {
            if out.is_empty() && err.is_empty() {
                *deadline = Instant::now() + FLUSH_WINDOW;
            }
            out.push_str(&text);
        }
        ProcessEvent::Stderr(text) => {
            if out.is_empty() && err.is_empty() {
                *deadline = Instant::now() + FLUSH_WINDOW;
            }
            err.push_str(&text);
        }
        // Exit never travels this way; it is sent after this thread ended,
        // so the plugin sees it last.
        ProcessEvent::Exit(_) => {}
    }
}

/// Sends what was collected, stdout before stderr, and empties the buffers.
fn flush(out: &mut String, err: &mut String, events: &Channel<ProcessEvent>) {
    if !out.is_empty() {
        let _ = events.send(ProcessEvent::Stdout(std::mem::take(out)));
    }
    if !err.is_empty() {
        let _ = events.send(ProcessEvent::Stderr(std::mem::take(err)));
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

    /// Output of both streams in one window all arrives: the piece of one
    /// stream must not throw away what was collected of the other.
    #[test]
    fn both_streams_survive_one_collection_window() {
        let spec = SpawnSpec {
            command: "sh".into(),
            args: vec![
                "-c".into(),
                "printf 'out-one\\n'; printf 'err-one\\n' >&2; printf 'out-two\\n'; printf 'err-two\\n' >&2".into(),
            ],
            stdin: None,
            env: HashMap::new(),
        };
        let (_, _, rx) = run(spec);
        let events = collect(&rx);
        let mut out = String::new();
        let mut err = String::new();
        for e in &events {
            let value: serde_json::Value = serde_json::from_str(e).expect("json");
            match value["event"].as_str() {
                Some("stdout") => out.push_str(value["data"].as_str().unwrap_or_default()),
                Some("stderr") => err.push_str(value["data"].as_str().unwrap_or_default()),
                _ => {}
            }
        }
        assert_eq!(out, "out-one\nout-two\n");
        assert_eq!(err, "err-one\nerr-two\n");
        assert_eq!(
            events.last().map(String::as_str),
            Some(r#"{"event":"exit","data":0}"#)
        );
    }

    /// A kill while the reaper waits (the usual case) ends the program and
    /// is reported without waiting for the program's own end.
    #[test]
    fn kill_works_while_the_reaper_waits() {
        let spec = SpawnSpec {
            command: "sleep".into(),
            args: vec!["30".into()],
            stdin: None,
            env: HashMap::new(),
        };
        let (processes, id, rx) = run(spec);
        // Give the reaper time to take the lock and block in `wait()`.
        thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        processes.kill(id);
        let events = collect(&rx);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the kill should not wait for the program"
        );
        assert_eq!(
            events.last().map(String::as_str),
            Some(r#"{"event":"exit","data":null}"#)
        );
    }
}
