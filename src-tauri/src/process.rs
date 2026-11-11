//! Programs started by store plugins with the `process` permission (e.g. the
//! command line tools of AI agents). They always run in the vault folder;
//! their output is streamed to the plugin through a channel, and they are
//! killed when the plugin asks, or when the vault is closed or switched.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
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
/// the lock of `child`; `kill` therefore never waits for that lock. It reads
/// [`Running::state`] instead, which names the program only until the reaper
/// has reaped its exit. The exit is noticed at once, without polling.
struct Running {
    /// What a kill needs to end the program, and whether the exit has been
    /// reaped already. The reaper sets `exited` and clears the target under
    /// this lock immediately after `wait()` returns, that is right after the
    /// reap: a kill that still finds a target therefore always names a child
    /// that has not been reaped, whose id the system cannot have handed out
    /// again yet.
    state: Mutex<ReapState>,
    child: Mutex<Child>,
}

/// The kill target of a running program, taken under [`Running::state`].
#[derive(Default)]
struct ReapState {
    /// Set once `wait()` has reaped the exit. A kill after this never
    /// signals anything.
    exited: bool,
    /// `None` once the exit was reaped.
    target: Option<KillTarget>,
}

/// What a kill needs to end the program without the child handle, which the
/// reaper holds in its blocking `wait()`.
#[derive(Clone)]
struct KillTarget {
    /// The process id. A kill takes it under the state lock before the
    /// reaper marks the reap, so at that moment the child is unreaped and
    /// the id still names our program. On Linux the pidfd below closes even
    /// the small window between taking the target and sending the signal;
    /// the id fallback on the other systems keeps that window when a kill
    /// races the program's natural end.
    pid: u32,
    /// A pidfd on Linux: it names exactly this process, so a signal through
    /// it can never reach a recycled id, even one sent late. `None` on older
    /// kernels and on the other systems. Shared because a kill takes a copy
    /// of the target while the reaper may already be done with the child.
    #[cfg(target_os = "linux")]
    pidfd: Option<Arc<PidFd>>,
    /// When the process was created, as the system counts it (100ns units
    /// since 1601 on Windows). Lets a kill check that an id still names our
    /// program and not one that got the id after ours ended.
    #[cfg(windows)]
    created: Option<u64>,
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
        let target = kill_target(&child);
        let child = Arc::new(Running {
            state: Mutex::new(ReapState {
                exited: false,
                target: Some(target),
            }),
            child: Mutex::new(child),
        });
        self.running
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, Arc::clone(&child));

        let this = Arc::clone(self);
        thread::spawn(move || {
            // A blocking wait: the exit is noticed at once and a running
            // program costs no wakeups at all. The child lock stays held
            // while waiting; `kill` only needs the state lock (see `Running`).
            let status = match child.child.lock() {
                Ok(mut c) => {
                    let status = c.wait().ok();
                    // `wait()` has reaped the exit, so the process id may be
                    // handed out again from now on. The state lock makes the
                    // marking atomic against `kill`: a kill either saw the
                    // target before this section (and signals the id that
                    // was still ours then), or it sees `exited` afterwards
                    // and signals nothing. The section is only two stores,
                    // so a kill never waits noticeably for it.
                    if let Ok(mut state) = child.state.lock() {
                        state.exited = true;
                        state.target = None;
                    }
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
        // One short critical section for the check and the target: the
        // reaper marks the reap under the same lock, so a target that is
        // still here belongs to a child that has not been reaped yet.
        let target = match child.state.lock() {
            Ok(state) if !state.exited => state.target.clone(),
            _ => None,
        };
        if let Some(target) = target {
            signal_kill(&target);
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

/// Gathers what a kill needs while the child is freshly spawned and cannot
/// have been reaped yet, so the process id still names our program.
fn kill_target(child: &Child) -> KillTarget {
    KillTarget {
        pid: child.id(),
        #[cfg(target_os = "linux")]
        pidfd: PidFd::open(child.id()),
        #[cfg(windows)]
        created: creation_time(child.id()),
    }
}

/// Ends a program while the reaper holds the handle in `wait()`. The target
/// was taken before the exit was marked as reaped (see [`Running::state`]).
/// On unix SIGKILL, like `Child::kill`; on Windows `TerminateProcess`, the
/// same call `Child::kill` makes.
#[cfg(unix)]
fn signal_kill(target: &KillTarget) {
    // The pidfd names exactly our process: even a signal that arrives after
    // the reap can never reach a program that got the id in the meantime.
    // A pidfd signal that fails (the process is gone, or the kernel has no
    // pidfds) falls through to the id, which is safe while the child is
    // unreaped.
    #[cfg(target_os = "linux")]
    if let Some(pidfd) = &target.pidfd
        && pidfd.send_sigkill()
    {
        return;
    }
    // SAFETY: `kill` only sends a signal to the given process id and touches
    // no memory. The target was taken under the state lock before the reaper
    // marked the exit, so the id named our program then. Without a pidfd a
    // kill that races the natural end can land just after the reap; that
    // residual window is what the pidfd closes on Linux.
    unsafe {
        libc::kill(target.pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn signal_kill(target: &KillTarget) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };
    // SAFETY: every handle comes from `OpenProcess` and is closed again. The
    // id alone is not enough on Windows: it can be handed out again while
    // our (zombie) process handle still exists. The creation time therefore
    // confirms that the id still names our program before it is ended.
    unsafe {
        let handle = OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            target.pid,
        );
        if handle.is_null() {
            return;
        }
        if let Some(created) = target.created {
            let mut creation = std::mem::zeroed();
            let mut exit = std::mem::zeroed();
            let mut kernel = std::mem::zeroed();
            let mut user = std::mem::zeroed();
            let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0;
            let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
            if !ok || ticks != created {
                // The id belongs to another program now; ours has ended.
                CloseHandle(handle);
                return;
            }
        }
        TerminateProcess(handle, 1);
        CloseHandle(handle);
    }
}

/// The creation time of a freshly spawned process, in the 100ns units of
/// `FILETIME`. `None` when it cannot be read; the kill then relies on the
/// state lock alone, as it does on unix.
#[cfg(windows)]
fn creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle comes from `OpenProcess` and is closed again; the
    // out parameters are valid `FILETIME` storage.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut creation = std::mem::zeroed();
        let mut exit = std::mem::zeroed();
        let mut kernel = std::mem::zeroed();
        let mut user = std::mem::zeroed();
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0;
        CloseHandle(handle);
        ok.then(|| ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
    }
}

/// An owned pidfd (Linux 5.3+): a descriptor that names one specific process
/// for as long as it is open, independent of its process id. The syscalls are
/// called through libc directly so no new dependency is needed; on kernels
/// without them `open` returns `None` and the kill falls back to the id,
/// which is safe while the child is unreaped (see [`signal_kill`]).
#[cfg(target_os = "linux")]
struct PidFd(std::os::fd::OwnedFd);

#[cfg(target_os = "linux")]
impl PidFd {
    fn open(pid: u32) -> Option<Arc<Self>> {
        use std::os::fd::FromRawFd;
        // SAFETY: `pidfd_open` only takes the id and flags; on success the
        // returned descriptor is fresh and owned, on failure nothing is
        // created. `OwnedFd` closes it again on drop.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0u32) };
        if fd < 0 {
            return None;
        }
        Some(Arc::new(Self(unsafe {
            std::os::fd::OwnedFd::from_raw_fd(fd as i32)
        })))
    }

    /// Sends SIGKILL through the descriptor. True when the signal was sent;
    /// a process that already exited reports ESRCH, which is fine.
    fn send_sigkill(&self) -> bool {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor is owned and open; the remaining arguments
        // are null and zero, so no siginfo is read by the kernel.
        let sent = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null_mut::<libc::siginfo_t>(),
                0u32,
            )
        };
        sent == 0
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

    /// Once the exit has been reaped, the process id may belong to another
    /// program already. A kill arriving then must not signal anything: the
    /// state is marked under the lock the kill reads, not after it.
    #[test]
    fn a_kill_after_the_reap_signals_nothing() {
        let spec = SpawnSpec {
            command: "sh".into(),
            args: vec!["-c".into(), "sleep 0.1".into()],
            stdin: None,
            env: HashMap::new(),
        };
        let (processes, id, rx) = run(spec);
        let entry = processes
            .running
            .lock()
            .expect("running")
            .get(&id)
            .cloned()
            .expect("entry");
        // The program ends on its own; wait until the reaper has marked the
        // reap. The wait releases the lock so the reaper can take it.
        let mut state = entry.state.lock().expect("state");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !state.exited {
            assert!(Instant::now() < deadline, "the exit was never marked");
            let (guard, _) = std::sync::Condvar::new()
                .wait_timeout(state, Duration::from_millis(5))
                .expect("wait");
            state = guard;
        }
        assert!(
            state.target.is_none(),
            "a reaped child keeps no kill target"
        );
        drop(state);
        // The late kill must signal nothing: the marked state stops it (or
        // the entry is already gone). The program's own exit still arrives.
        processes.kill(id);
        assert_eq!(
            collect(&rx).last().map(String::as_str),
            Some(r#"{"event":"exit","data":0}"#)
        );
    }

    /// Many kills racing against the natural end of short programs: every
    /// exit must still be reported, and the kills must end running programs
    /// without blocking on the reaper's wait.
    #[test]
    fn kills_racing_the_exit_stay_prompt_and_report_every_exit() {
        for i in 0u64..16 {
            let spec = SpawnSpec {
                command: "sh".into(),
                args: vec!["-c".into(), format!("sleep 0.0{}", i % 4)],
                stdin: None,
                env: HashMap::new(),
            };
            let (processes, id, rx) = run(spec);
            thread::sleep(Duration::from_millis((i % 8) * 5));
            let start = Instant::now();
            processes.kill(id);
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "kill {i} blocked on the reaper"
            );
            let events = collect(&rx);
            let last = events.last().map(String::as_str).unwrap_or_default();
            assert!(
                last == r#"{"event":"exit","data":0}"# || last == r#"{"event":"exit","data":null}"#,
                "run {i} did not report an exit: {last}"
            );
        }
    }

    /// On Linux the kill goes through a pidfd when the kernel has one: it
    /// names exactly our process, so even a late signal cannot reach a
    /// recycled id.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_pidfd_names_only_its_own_process() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        let pidfd = PidFd::open(child.id()).expect("pidfd on a modern kernel");
        assert!(pidfd.send_sigkill());
        let status = child.wait().expect("wait");
        assert!(
            !status.success(),
            "the signal should have ended the program"
        );
        // After the reap the descriptor still names the old process only;
        // sending again fails with ESRCH instead of reaching a new one.
        assert!(!pidfd.send_sigkill());
    }
}
