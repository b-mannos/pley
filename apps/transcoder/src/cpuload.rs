// SPDX-License-Identifier: LicenseRef-Restricted
//! How busy the computer's processors are, and how much of that is other
//! programs': the share of their time that went to any work, and to work
//! not this transcoder's nor its FFmpeg processes', between two looks.
//! Windows: `GetSystemTimes`, and the job object the transcoder's
//! processes share (see [`crate::util::kill_children_on_exit`]). Linux:
//! `/proc`, the whole computer's even in a container. Elsewhere unknown.

use parking_lot::Mutex;

/// Processor time so far, in the system's own unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Times {
    /// All processors' time busy, and in all.
    busy: u64,
    total: u64,
    /// What this process and its children took; `None` where unknown.
    ours: Option<u64>,
}

/// Shares of the processors' time, 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shares {
    busy: f64,
    /// Busy with other programs' work.
    foreign: Option<f64>,
}

/// Looks at the processors' time now and then (the reaper's turn, every few
/// seconds), and keeps how busy they were lately.
#[derive(Debug, Default)]
pub struct CpuMeter {
    /// The last look, and the shares so far.
    state: Mutex<(Option<Times>, Option<Shares>)>,
}

impl CpuMeter {
    pub fn sample(&self) {
        if let Some(now) = read() {
            self.record(now);
        }
    }

    /// Each look weighs as much as all those before it together, so that the
    /// shares follow the last two or three (10 s or so).
    fn record(&self, now: Times) {
        let mut state = self.state.lock();
        if let Some(new) = state.0.and_then(|last| shares_between(last, now)) {
            let mean = |old: Option<f64>, new: Option<f64>| match (old, new) {
                (Some(old), Some(new)) => Some((old + new) / 2.0),
                (old, new) => old.or(new),
            };
            state.1 = Some(state.1.map_or(new, |old| Shares {
                busy: (old.busy + new.busy) / 2.0,
                foreign: mean(old.foreign, new.foreign),
            }));
        }
        state.0 = Some(now);
    }

    /// The share of the processors' time that went to work lately, 0 to 1;
    /// `None` before two looks, or where it can't be measured.
    pub fn busy(&self) -> Option<f64> {
        self.state.lock().1.map(|s| s.busy)
    }

    /// The share that went to other programs' work: not this transcoder's,
    /// nor its FFmpeg processes'.
    pub fn foreign(&self) -> Option<f64> {
        self.state.lock().1.and_then(|s| s.foreign)
    }
}

fn shares_between(a: Times, b: Times) -> Option<Shares> {
    let total = b.total.checked_sub(a.total)?;
    let busy = b.busy.checked_sub(a.busy)?;
    let ours = a.ours.zip(b.ours).and_then(|(a, b)| b.checked_sub(a));
    let share = |time: u64| (time as f64 / total as f64).clamp(0.0, 1.0);
    (total > 0).then(|| Shares { busy: share(busy), foreign: ours.map(|ours| share(busy.saturating_sub(ours))) })
}

#[cfg(windows)]
fn read() -> Option<Times> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::JobObjects::{
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation, QueryInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut idle, mut kernel, mut user) = (zero, zero, zero);
    // SAFETY: plain Win32 call writing three FILETIMEs of our own.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let ticks = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    // Kernel time counts the idle time too.
    let total = ticks(kernel) + ticks(user);
    // The job object this process is in, which its FFmpeg processes join
    // as they start: the time all of them took, those ended included, in
    // the same 100 ns.
    // SAFETY: plain Win32 call writing a struct of our own of the size given.
    let ours = unsafe {
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
        let ok = QueryInformationJobObject(
            std::ptr::null_mut(),
            JobObjectBasicAccountingInformation,
            (&raw mut info).cast(),
            std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            std::ptr::null_mut(),
        ) != 0;
        ok.then(|| info.TotalUserTime.max(0) as u64 + info.TotalKernelTime.max(0) as u64)
    };
    Some(Times { busy: total.saturating_sub(ticks(idle)), total, ours })
}

#[cfg(target_os = "linux")]
fn read() -> Option<Times> {
    let (busy, total) = parse_proc_stat(&std::fs::read_to_string("/proc/stat").ok()?)?;
    Some(Times { busy, total, ours: ours() })
}

/// The time this process and its children took, in clock ticks: theirs
/// while they run, and its children's once it waited for them.
#[cfg(target_os = "linux")]
fn ours() -> Option<u64> {
    let me = std::process::id();
    let mut ours = 0;
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        if !entry.file_name().to_str().is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit())) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path().join("stat")) else { continue };
        if let Some(stat) = parse_pid_stat(&text).filter(|s| s.pid == me || s.parent == me) {
            ours += stat.time;
        }
    }
    Some(ours)
}

/// The first line of `/proc/stat`, `cpu  user nice system idle iowait irq
/// softirq steal …` in clock ticks: busy and in all, where idle, and
/// waiting for the disk, aren't busy.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn parse_proc_stat(text: &str) -> Option<(u64, u64)> {
    let fields = text.lines().next()?.strip_prefix("cpu ")?;
    let ticks: Vec<u64> = fields.split_whitespace().take(8).map(|f| f.parse().ok()).collect::<Option<_>>()?;
    let total: u64 = ticks.iter().sum();
    let idle = ticks.get(3)? + ticks.get(4).copied().unwrap_or(0);
    Some((total.saturating_sub(idle), total))
}

/// A process in `/proc/<pid>/stat`.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
#[derive(Debug, PartialEq, Eq)]
struct PidStat {
    pid: u32,
    parent: u32,
    /// Its time, and its children's it waited for, in clock ticks.
    time: u64,
}

/// `pid (name) state ppid … utime stime cutime cstime …`, whose name may
/// hold spaces and parentheses.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn parse_pid_stat(text: &str) -> Option<PidStat> {
    let (pid, rest) = text.split_once(" (")?;
    let fields: Vec<&str> = rest.get(rest.rfind(')')? + 1..)?.split_whitespace().collect();
    let parent = fields.get(1)?.parse().ok()?;
    let time =
        fields.get(11..15)?.iter().map(|f| f.parse::<i64>().ok().map(|t| t.max(0) as u64)).sum::<Option<u64>>()?;
    Some(PidStat { pid: pid.trim().parse().ok()?, parent, time })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_stat_counts_all_but_idle_and_waiting_for_the_disk() {
        let stat = "cpu  100 20 30 800 50 0 0 0 0 0\ncpu0 50 10 15 400 25 0 0 0 0 0\nintr 1\n";
        assert_eq!(parse_proc_stat(stat), Some((150, 1000)));
        assert_eq!(parse_proc_stat("intr 1\n"), None);
    }

    #[test]
    fn a_process_counts_its_time_and_its_ended_childrens() {
        let stat = "4242 (ffmpeg (x) y) S 1000 4242 4242 0 -1 4194560 100 0 0 0 250 50 7 3 20 0 1 0 900 1 1\n";
        assert_eq!(parse_pid_stat(stat), Some(PidStat { pid: 4242, parent: 1000, time: 310 }));
        assert_eq!(parse_pid_stat("4242 (ffmpeg) S 1000"), None);
    }

    fn times(busy: u64, total: u64, ours: Option<u64>) -> Times {
        Times { busy, total, ours }
    }

    #[test]
    fn the_shares_follow_the_last_looks() {
        let meter = CpuMeter::default();
        meter.record(times(0, 0, Some(0)));
        assert_eq!(meter.busy(), None, "one look says nothing yet");
        meter.record(times(80, 100, Some(60)));
        assert_eq!((meter.busy(), meter.foreign()), (Some(0.8), Some(0.2)));
        meter.record(times(100, 200, Some(60)));
        assert_eq!((meter.busy(), meter.foreign()), (Some(0.5), Some(0.2)), "(0.8 + 0.2) / 2, (0.2 + 0.2) / 2");
        // A counter that went back (a wrap, a new boot) is skipped.
        meter.record(times(10, 20, Some(0)));
        assert_eq!(meter.busy(), Some(0.5));
        // Ours unknown: the others' share as it was.
        meter.record(times(30, 40, None));
        assert_eq!((meter.busy(), meter.foreign()), (Some(0.75), Some(0.2)));
    }

    #[test]
    fn this_computer_can_be_measured() {
        if cfg!(any(windows, target_os = "linux")) {
            let meter = CpuMeter::default();
            meter.sample();
            std::thread::sleep(std::time::Duration::from_millis(50));
            meter.sample();
            assert!(meter.busy().is_some_and(|b| (0.0..=1.0).contains(&b)));
            assert!(meter.foreign().is_none_or(|f| (0.0..=1.0).contains(&f)));
        }
    }
}
