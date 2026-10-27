//! How busy the machine is and how much of it pley takes: the processors'
//! time and the memory, sampled every [`INTERVAL`] for the dashboard's
//! charts (Plex's "CPU" and "RAM"). pley here is this process and every
//! process it started, so a managed transcoder and its FFmpegs count too;
//! an external transcoder, perhaps on another machine, doesn't.
//!
//! A sample costs a few system calls and, for the processes, one listing of
//! them (Windows) or one read of each `/proc/<pid>/stat` (Linux): well under
//! a millisecond of one processor every two seconds.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

/// How often the machine is sampled.
pub const INTERVAL: Duration = Duration::from_secs(2);
/// The samples kept: five minutes of them.
const KEEP: usize = 150;

/// The machine and pley at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Sample {
    /// When, in Unix milliseconds.
    pub at: i64,
    /// The share of all the processors' time used since the sample before,
    /// 0 to 100, by everything on the machine.
    pub cpu: f32,
    /// The same, by pley.
    pub pley_cpu: f32,
    /// Memory in use on the machine, in bytes (without what only caches
    /// files and could be given back at once).
    pub memory: u64,
    /// Memory pley holds: its processes' resident sets, in bytes.
    pub pley_memory: u64,
}

/// What `GET /api/system/resources` answers.
#[derive(Debug, Serialize)]
pub struct Resources {
    /// Whether this system can be sampled at all (Windows, Linux and macOS).
    pub available: bool,
    pub interval_ms: u64,
    /// Logical processors on the machine.
    pub cpus: u32,
    /// Its memory, in bytes.
    pub memory_total: u64,
    /// Oldest first.
    pub samples: Vec<Sample>,
}

/// The samples of the last few minutes, taken in the background.
pub struct Monitor {
    samples: Mutex<VecDeque<Sample>>,
    cpus: u32,
    memory_total: u64,
}

impl Monitor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            samples: Mutex::new(VecDeque::with_capacity(KEEP)),
            cpus: os::cpus().max(1),
            memory_total: os::memory().map(|m| m.total).unwrap_or(0),
        })
    }

    /// Sample every [`INTERVAL`] until `stopping`.
    pub fn start(self: &Arc<Self>, stopping: CancellationToken) {
        let monitor = self.clone();
        if !os::SUPPORTED {
            return;
        }
        tokio::spawn(async move {
            let mut sampler = Sampler::new(monitor.cpus);
            let mut tick = tokio::time::interval(INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = stopping.cancelled() => break,
                    _ = tick.tick() => {}
                }
                if let Some(sample) = sampler.sample() {
                    monitor.push(sample);
                }
            }
        });
    }

    fn push(&self, sample: Sample) {
        let mut samples = self.samples.lock();
        if samples.len() == KEEP {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    /// The samples taken after `since` (Unix milliseconds), or all of them.
    pub fn resources(&self, since: Option<i64>) -> Resources {
        let samples = self.samples.lock().iter().filter(|s| since.is_none_or(|t| s.at > t)).copied().collect();
        Resources {
            available: os::SUPPORTED,
            interval_ms: INTERVAL.as_millis() as u64,
            cpus: self.cpus,
            memory_total: self.memory_total,
            samples,
        }
    }
}

/// Processor time so far, as the system counts it (any unit, the same each time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CpuTimes {
    busy: u64,
    total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Memory {
    total: u64,
    used: u64,
}

/// One of pley's processes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Process {
    /// Its id and when it started (in the system's own unit), so that an id
    /// used again by a new process isn't taken for the old one.
    key: (u32, u64),
    /// Processor time it has used, in nanoseconds.
    cpu_ns: u64,
    /// Its resident set, in bytes.
    memory: u64,
}

/// Turns the system's running totals into the shares between two samples.
struct Sampler {
    cpus: u32,
    last: Option<(Instant, CpuTimes)>,
    /// Each process's processor time at the sample before.
    processes: HashMap<(u32, u64), u64>,
}

impl Sampler {
    fn new(cpus: u32) -> Self {
        Self { cpus, last: None, processes: HashMap::new() }
    }

    /// `None` the first time (a share needs two readings) or when the system
    /// can't be read.
    fn sample(&mut self) -> Option<Sample> {
        let now = Instant::now();
        let times = os::cpu_times()?;
        let memory = os::memory()?;
        let tree = os::processes(std::process::id());
        let pley_memory = tree.iter().map(|p| p.memory).sum();
        let previous = std::mem::replace(&mut self.processes, tree.iter().map(|p| (p.key, p.cpu_ns)).collect());
        let last = self.last.replace((now, times));
        let (then, before) = last?;
        let elapsed = now.duration_since(then).as_nanos() as f64 * f64::from(self.cpus);
        // A process seen for the first time started since the sample before
        // (or this is the second sample, and every process is new).
        let used: u64 = tree.iter().map(|p| p.cpu_ns.saturating_sub(previous.get(&p.key).copied().unwrap_or(0))).sum();
        Some(Sample {
            at: crate::util::now_ms(),
            cpu: share(times.busy.saturating_sub(before.busy) as f64, times.total.saturating_sub(before.total) as f64),
            pley_cpu: share(used as f64, elapsed),
            memory: memory.used,
            pley_memory,
        })
    }
}

/// `part` of `whole` as a percentage, 0 to 100, to a tenth.
fn share(part: f64, whole: f64) -> f32 {
    if whole <= 0.0 {
        return 0.0;
    }
    ((part / whole * 100.0).clamp(0.0, 100.0) * 10.0).round() as f32 / 10.0
}

#[cfg(windows)]
mod os {
    use std::collections::HashMap;
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::{
        ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetProcessTimes, GetSystemTimes, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use super::{CpuTimes, Memory, Process};

    pub const SUPPORTED: bool = true;

    fn ticks(t: FILETIME) -> u64 {
        (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
    }

    pub fn cpus() -> u32 {
        unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) }
    }

    pub fn cpu_times() -> Option<CpuTimes> {
        let (mut idle, mut kernel, mut user) = unsafe { (zeroed(), zeroed(), zeroed()) };
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return None;
        }
        // Kernel time includes the idle time.
        let total = ticks(kernel) + ticks(user);
        Some(CpuTimes { busy: total.saturating_sub(ticks(idle)), total })
    }

    pub fn memory() -> Option<Memory> {
        let mut status: MEMORYSTATUSEX = unsafe { zeroed() };
        status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
            return None;
        }
        Some(Memory { total: status.ullTotalPhys, used: status.ullTotalPhys.saturating_sub(status.ullAvailPhys) })
    }

    /// `root` and the processes it started, theirs and so on.
    pub fn processes(root: u32) -> Vec<Process> {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Vec::new();
            }
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                if entry.th32ProcessID != entry.th32ParentProcessID {
                    children.entry(entry.th32ParentProcessID).or_default().push(entry.th32ProcessID);
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        let mut found = Vec::new();
        let mut queue = vec![(root, 0u64)];
        while let Some((pid, parent_started)) = queue.pop() {
            let Some(process) = process(pid) else { continue };
            // Windows keeps a parent's id after the parent ends, and may give
            // it to a newer process: a child is never older than its parent.
            if process.key.1 < parent_started {
                continue;
            }
            for &child in children.get(&pid).into_iter().flatten() {
                queue.push((child, process.key.1));
            }
            found.push(process);
        }
        found
    }

    fn process(pid: u32) -> Option<Process> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let (mut created, mut exited, mut kernel, mut user) = (zeroed(), zeroed(), zeroed(), zeroed());
            let times = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) != 0;
            let mut counters: PROCESS_MEMORY_COUNTERS = zeroed();
            counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let memory = K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0;
            CloseHandle(handle);
            times.then(|| Process {
                key: (pid, ticks(created)),
                // In units of 100 ns.
                cpu_ns: (ticks(kernel) + ticks(user)) * 100,
                memory: if memory { counters.WorkingSetSize as u64 } else { 0 },
            })
        }
    }
}

#[cfg(target_os = "linux")]
mod os {
    use std::collections::HashMap;

    use super::{CpuTimes, Memory, Process};

    pub const SUPPORTED: bool = true;

    pub fn cpus() -> u32 {
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        stat.lines().filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit)).count()
            as u32
    }

    pub fn cpu_times() -> Option<CpuTimes> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        parse_cpu_times(stat.lines().next()?)
    }

    /// The first line of `/proc/stat`: "cpu  user nice system idle iowait irq
    /// softirq steal guest guest_nice". Guest time is already in user time.
    pub(super) fn parse_cpu_times(line: &str) -> Option<CpuTimes> {
        let mut fields = line.split_ascii_whitespace();
        if fields.next()? != "cpu" {
            return None;
        }
        let values: Vec<u64> = fields.take(8).map(|f| f.parse().unwrap_or(0)).collect();
        if values.len() < 4 {
            return None;
        }
        let total: u64 = values.iter().sum();
        let idle = values[3] + values.get(4).copied().unwrap_or(0);
        Some(CpuTimes { busy: total.saturating_sub(idle), total })
    }

    pub fn memory() -> Option<Memory> {
        parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
    }

    pub(super) fn parse_meminfo(text: &str) -> Option<Memory> {
        let field = |name: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
                .and_then(|v| v.split_ascii_whitespace().next()?.parse::<u64>().ok())
                .map(|kib| kib * 1024)
        };
        let total = field("MemTotal")?;
        // Kernels before 3.14 have no MemAvailable.
        let available = field("MemAvailable")
            .or_else(|| Some(field("MemFree")? + field("Buffers").unwrap_or(0) + field("Cached").unwrap_or(0)))?;
        Some(Memory { total, used: total.saturating_sub(available) })
    }

    /// One `/proc/<pid>/stat`: its parent and the process as counted here.
    pub(super) fn parse_stat(pid: u32, text: &str, tick_ns: u64, page: u64) -> Option<(u32, Process)> {
        // The name, in parentheses, may itself hold spaces and parentheses.
        let rest = &text[text.rfind(')')? + 1..];
        let fields: Vec<&str> = rest.split_ascii_whitespace().collect();
        // Field 3 (state) is fields[0]; ppid (4) fields[1]; utime (14)
        // fields[11]; stime (15) fields[12]; starttime (22) fields[19]; rss
        // (24) fields[21].
        let number = |i: usize| fields.get(i).and_then(|f| f.parse::<u64>().ok());
        let parent = number(1)? as u32;
        Some((
            parent,
            Process {
                key: (pid, number(19)?),
                cpu_ns: (number(11)? + number(12)?) * tick_ns,
                memory: number(21)? * page,
            },
        ))
    }

    /// `root` and the processes it started, theirs and so on.
    pub fn processes(root: u32) -> Vec<Process> {
        let (tick_ns, page) = unsafe { (libc::sysconf(libc::_SC_CLK_TCK), libc::sysconf(libc::_SC_PAGESIZE)) };
        let tick_ns = if tick_ns > 0 { 1_000_000_000 / tick_ns as u64 } else { 10_000_000 };
        let page = if page > 0 { page as u64 } else { 4096 };
        let Ok(dir) = std::fs::read_dir("/proc") else { return Vec::new() };
        let mut all: HashMap<u32, (u32, Process)> = HashMap::new();
        for entry in dir.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
            // A process may end between the listing and the read.
            if let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/stat"))
                && let Some(read) = parse_stat(pid, &text, tick_ns, page)
            {
                all.insert(pid, read);
            }
        }
        super::descendants(root, &all)
    }
}

#[cfg(target_os = "macos")]
mod os {
    #![allow(deprecated)] // mach_host_self and mach_timebase_info: libc points to the mach2 crate.

    use std::mem::{size_of, zeroed};
    use std::sync::OnceLock;

    use super::{CpuTimes, Memory, Process};

    pub const SUPPORTED: bool = true;

    fn host() -> libc::mach_port_t {
        static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
        *HOST.get_or_init(|| unsafe { libc::mach_host_self() })
    }

    fn sysctl_u64(name: &std::ffi::CStr) -> Option<u64> {
        let mut value: u64 = 0;
        let mut len = size_of::<u64>();
        let ok =
            unsafe { libc::sysctlbyname(name.as_ptr(), (&raw mut value).cast(), &mut len, std::ptr::null_mut(), 0) };
        (ok == 0).then_some(value)
    }

    pub fn cpus() -> u32 {
        let mut value: i32 = 0;
        let mut len = size_of::<i32>();
        let ok = unsafe {
            libc::sysctlbyname(c"hw.logicalcpu".as_ptr(), (&raw mut value).cast(), &mut len, std::ptr::null_mut(), 0)
        };
        if ok == 0 { value.max(1) as u32 } else { 1 }
    }

    pub fn cpu_times() -> Option<CpuTimes> {
        let mut load: libc::host_cpu_load_info = unsafe { zeroed() };
        let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
        let ok = unsafe { libc::host_statistics(host(), libc::HOST_CPU_LOAD_INFO, (&raw mut load).cast(), &mut count) };
        if ok != libc::KERN_SUCCESS {
            return None;
        }
        let t = load.cpu_ticks.map(u64::from);
        let total = t[libc::CPU_STATE_USER as usize]
            + t[libc::CPU_STATE_SYSTEM as usize]
            + t[libc::CPU_STATE_IDLE as usize]
            + t[libc::CPU_STATE_NICE as usize];
        Some(CpuTimes { busy: total - t[libc::CPU_STATE_IDLE as usize], total })
    }

    pub fn memory() -> Option<Memory> {
        let total = sysctl_u64(c"hw.memsize")?;
        let mut vm: libc::vm_statistics64 = unsafe { zeroed() };
        let mut count =
            (size_of::<libc::vm_statistics64>() / size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
        let ok = unsafe { libc::host_statistics64(host(), libc::HOST_VM_INFO64, (&raw mut vm).cast(), &mut count) };
        if ok != libc::KERN_SUCCESS {
            return None;
        }
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
        // What Activity Monitor calls Memory Used: app memory, wired and compressed.
        let pages = u64::from(vm.internal_page_count).saturating_sub(u64::from(vm.purgeable_count))
            + u64::from(vm.wire_count)
            + u64::from(vm.compressor_page_count);
        Some(Memory { total, used: (pages * page).min(total) })
    }

    /// Mach time units per nanosecond, as numerator and denominator.
    fn timebase() -> (u64, u64) {
        static TIMEBASE: OnceLock<(u64, u64)> = OnceLock::new();
        *TIMEBASE.get_or_init(|| {
            let mut info = libc::mach_timebase_info { numer: 1, denom: 1 };
            unsafe { libc::mach_timebase_info(&mut info) };
            (u64::from(info.numer), u64::from(info.denom.max(1)))
        })
    }

    fn process(pid: i32) -> Option<Process> {
        let mut info: libc::proc_taskinfo = unsafe { zeroed() };
        let size = size_of::<libc::proc_taskinfo>() as i32;
        let read = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, (&raw mut info).cast(), size) };
        if read != size {
            return None;
        }
        let (numer, denom) = timebase();
        Some(Process {
            // Ids come back slowly here (they count up to 99999 first).
            key: (pid as u32, 0),
            cpu_ns: (info.pti_total_user + info.pti_total_system) * numer / denom,
            memory: info.pti_resident_size,
        })
    }

    /// `root` and the processes it started, theirs and so on.
    pub fn processes(root: u32) -> Vec<Process> {
        let mut found = Vec::new();
        let mut queue = vec![root as i32];
        while let Some(pid) = queue.pop() {
            let Some(process) = process(pid) else { continue };
            found.push(process);
            let mut children = [0i32; 256];
            let count =
                unsafe { libc::proc_listchildpids(pid, children.as_mut_ptr().cast(), size_of::<[i32; 256]>() as i32) };
            queue.extend(children.iter().take(count.max(0) as usize).filter(|&&c| c > 0));
        }
        found
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod os {
    use super::{CpuTimes, Memory, Process};

    pub const SUPPORTED: bool = false;

    pub fn cpus() -> u32 {
        std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1)
    }

    pub fn cpu_times() -> Option<CpuTimes> {
        None
    }

    pub fn memory() -> Option<Memory> {
        None
    }

    pub fn processes(_root: u32) -> Vec<Process> {
        Vec::new()
    }
}

/// `root` and its descendants among `all` (each process by id, with its parent's id).
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn descendants(root: u32, all: &HashMap<u32, (u32, Process)>) -> Vec<Process> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&pid, &(parent, _)) in all {
        if pid != parent {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut found = Vec::new();
    let mut queue = vec![root];
    while let Some(pid) = queue.pop() {
        let Some(&(_, process)) = all.get(&pid) else { continue };
        found.push(process);
        queue.extend(children.get(&pid).into_iter().flatten());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, cpu_ns: u64) -> Process {
        Process { key: (pid, 1), cpu_ns, memory: u64::from(pid) }
    }

    #[test]
    fn descendants_are_the_root_its_children_and_theirs() {
        let all: HashMap<u32, (u32, Process)> = [
            (1, (0, proc(1, 0))),
            (10, (1, proc(10, 0))),
            (11, (10, proc(11, 0))),
            (12, (10, proc(12, 0))),
            (20, (1, proc(20, 0))),
        ]
        .into_iter()
        .collect();
        let mut pids: Vec<u32> = descendants(10, &all).iter().map(|p| p.key.0).collect();
        pids.sort_unstable();
        assert_eq!(pids, [10, 11, 12]);
        assert!(descendants(99, &all).is_empty());
    }

    #[test]
    fn shares_are_percentages_to_a_tenth() {
        assert_eq!(share(1.0, 3.0), 33.3);
        assert_eq!(share(5.0, 0.0), 0.0);
        assert_eq!(share(7.0, 5.0), 100.0);
    }

    #[test]
    fn only_samples_after_since_are_answered() {
        let monitor = Monitor::new();
        for at in [1_000, 3_000, 5_000] {
            monitor.push(Sample { at, cpu: 1.0, pley_cpu: 0.5, memory: 10, pley_memory: 5 });
        }
        assert_eq!(monitor.resources(None).samples.len(), 3);
        assert_eq!(monitor.resources(Some(3_000)).samples.iter().map(|s| s.at).collect::<Vec<_>>(), [5_000]);
        for at in 0..KEEP as i64 + 10 {
            monitor.push(Sample { at: 10_000 + at, cpu: 0.0, pley_cpu: 0.0, memory: 0, pley_memory: 0 });
        }
        assert_eq!(monitor.resources(None).samples.len(), KEEP);
    }

    #[test]
    fn this_machine_samples() {
        if !os::SUPPORTED {
            return;
        }
        let mut sampler = Sampler::new(os::cpus());
        assert!(sampler.cpus >= 1);
        assert_eq!(sampler.sample(), None, "a share needs two readings");
        // Some work of our own, so pley's share is above nothing.
        let start = Instant::now();
        let mut x = 0u64;
        while start.elapsed() < Duration::from_millis(150) {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        std::hint::black_box(x);
        let sample = sampler.sample().expect("a sample");
        let memory = os::memory().unwrap();
        assert!(sample.memory > 0 && sample.memory <= memory.total);
        assert!(sample.pley_memory > 0 && sample.pley_memory < memory.total);
        assert!(sample.pley_cpu > 0.0 && sample.pley_cpu <= 100.0, "{sample:?}");
        assert!(sample.cpu >= 0.0 && sample.cpu <= 100.0);
    }

    #[test]
    fn processes_started_here_count_as_pley() {
        if !os::SUPPORTED {
            return;
        }
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd").args(["/C", "ping -n 3 127.0.0.1 > NUL"]).spawn().unwrap();
        #[cfg(not(windows))]
        let mut child = std::process::Command::new("sleep").arg("2").spawn().unwrap();
        let tree = os::processes(std::process::id());
        let _ = child.kill();
        let _ = child.wait();
        assert!(tree.iter().any(|p| p.key.0 == std::process::id()));
        assert!(tree.iter().any(|p| p.key.0 == child.id()), "the child is pley's too");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_files_are_read() {
        assert_eq!(os::parse_cpu_times("cpu  100 10 50 800 40 0 5 0 0 0"), Some(CpuTimes { busy: 165, total: 1005 }));
        let meminfo = "MemTotal:       16000000 kB\nMemFree:         1000000 kB\nMemAvailable:    6000000 kB\n";
        assert_eq!(os::parse_meminfo(meminfo), Some(Memory { total: 16_000_000 * 1024, used: 10_000_000 * 1024 }));
        let stat = "42 (a (weird) name) S 7 42 42 0 -1 4194560 100 0 0 0 30 20 0 0 20 0 4 0 12345 1000000 250 18446744073709551615";
        let (parent, process) = os::parse_stat(42, stat, 10_000_000, 4096).unwrap();
        assert_eq!(parent, 7);
        assert_eq!(process, Process { key: (42, 12345), cpu_ns: 500_000_000, memory: 250 * 4096 });
    }
}
