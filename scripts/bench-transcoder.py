#!/usr/bin/env python3
"""Benchmark pley-transcoder on local files: the real program, over its HTTP API.

Each run starts a transcoder of its own (port 17691 by default), asks for one
job and fetches its segments as a player would, as fast as they come, then
stops the transcoder and reads what it and its FFmpeg processes used:

- start: from POST /v1/jobs to the first segment fully received (seconds);
- seek: from asking for a segment far ahead to having it (seek cases);
- speed: media seconds fetched per second of wall time after the first one
  (x real time);
- cpu: processor seconds per media second made, and the share of the
  processors allowed in use over the run (cpu %);
- mem: the peak memory of the largest process (on Windows its private
  commit, on Linux its resident set) and, on Windows, of all of them;
- bytes: the size of the segments fetched.

Cheaper hardware is emulated with --cores (the transcoder and FFmpeg run on
that many logical processors: a Windows job object's affinity, Linux's
sched_setaffinity), --sw (no GPU: hwaccel none, no hardware decoding) and,
on Windows, --mem-mb (a job memory limit: processes fail past it).

Two builds compare with --bin A=path --bin B=path: their runs alternate,
in turns that change order each round, so a machine shared with other work
weighs on both alike. Results go to stdout as a table and to --out as JSON
lines.

  python scripts/bench-transcoder.py --make-clips DIR --source 4k-hdr.mkv
  python scripts/bench-transcoder.py --media DIR --bin base=target/release/pley-transcoder \\
      --cases all --variants gpu,sw,sw4 --repeat 3

No third-party modules: Python 3.9 or later, standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

WINDOWS = os.name == "nt"

# ---------------------------------------------------------------------------
# Test clips
# ---------------------------------------------------------------------------

CLIP_SECONDS = 60


def make_clips(out: Path, source: str | None, ffmpeg: str) -> None:
    """Clips for the cases, from a 4K HDR source when given (real pictures
    cost what films cost to encode), else from FFmpeg's test patterns."""
    out.mkdir(parents=True, exist_ok=True)

    def run(args: list[str], name: str) -> None:
        target = out / name
        if target.exists():
            print(f"have {target}")
            return
        print(f"making {target}")
        subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-y", *args, str(target)], check=True)

    if source:
        video_in = ["-t", str(CLIP_SECONDS), "-i", source]
        sdr = (
            "zscale=w=1920:h=-2:t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,"
            "tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p"
        )
        run(
            [*video_in, "-map", "0:v:0", "-map", "0:a:0", "-vf", sdr, "-c:v", "libx264", "-preset", "medium",
             "-b:v", "8M", "-maxrate", "10M", "-bufsize", "16M", "-g", "48", "-c:a", "aac", "-ac", "6", "-b:a", "384k"],
            "film-1080p-h264.mkv",
        )
    else:
        run(
            ["-f", "lavfi", "-i", f"testsrc2=size=1920x1080:rate=24:duration={CLIP_SECONDS}",
             "-f", "lavfi", "-i", f"sine=frequency=440:duration={CLIP_SECONDS}:sample_rate=48000",
             "-filter_complex", "[1:a]pan=5.1|c0=c0|c1=c0|c2=c0|c3=c0|c4=c0|c5=c0[a]", "-map", "0:v", "-map", "[a]",
             "-c:v", "libx264", "-preset", "medium", "-b:v", "8M", "-g", "48", "-c:a", "aac", "-b:a", "384k"],
            "film-1080p-h264.mkv",
        )
    base = out / "film-1080p-h264.mkv"
    run(["-i", str(base), "-map", "0:v", "-map", "0:a", "-c:v", "copy", "-c:a", "truehd", "-strict", "-2"],
        "film-truehd.mkv")
    run(["-i", str(base), "-map", "0:v", "-map", "0:a", "-c:v", "copy", "-c:a", "dca", "-strict", "-2",
         "-b:a", "1509k"], "film-dts.mkv")
    # Ten minutes, for playbacks watched in real time (session cases).
    run(["-stream_loop", "9", "-i", str(base), "-map", "0", "-c", "copy"], "film-long.mkv")
    if source:
        run(["-stream_loop", "3", "-i", source, "-map", "0:v:0", "-map", "0:a:0", "-c", "copy"], "hdr-long.mkv")
    sup = out / "film-pgs.sup"
    if not sup.exists():
        make_pgs(sup)
    srt = out / "film.srt"
    if not srt.exists():
        lines = []
        for i in range(CLIP_SECONDS // 2):
            a, b = i * 2, i * 2 + 2
            lines.append(f"{i + 1}\n00:00:{a:02d},100 --> 00:00:{b:02d},000\n"
                         f"Subtitle line {i + 1}, drawn onto the video\nwith a second line of text\n")
        srt.write_text("\n".join(lines).replace("00:00:60", "00:01:00"), encoding="utf-8")


PGS_SAMPLE = "https://fate-suite.ffmpeg.org/sub/pgs_sub.sup"


def make_pgs(target: Path) -> None:
    """A minute of picture subtitles: FFmpeg's FATE sample (0.4 s of them)
    every 2 s, its segments' times moved on (PGS: "PG", PTS, DTS, type, size)."""
    import struct

    print(f"making {target} from {PGS_SAMPLE}")
    with urllib.request.urlopen(PGS_SAMPLE, timeout=60) as resp:
        src = resp.read()
    segments, i = [], 0
    while i + 13 <= len(src) and src[i:i + 2] == b"PG":
        pts, dts, _kind, size = struct.unpack(">IIBH", src[i + 2:i + 13])
        segments.append((pts, dts, src[i + 10:i + 13 + size]))
        i += 13 + size
    out = bytearray()
    for k in range(CLIP_SECONDS // 2):
        shift = k * 2 * 90_000
        for pts, dts, rest in segments:
            out += b"PG" + struct.pack(">II", pts + shift, dts + shift if dts else 0) + rest
    target.write_bytes(bytes(out))


# ---------------------------------------------------------------------------
# Cases
# ---------------------------------------------------------------------------


def cases(media: Path, hdr: str, remux: str) -> dict[str, dict]:
    film = str(media / "film-1080p-h264.mkv")
    aac_copy = {"mode": "copy"}
    return {
        # Playbacks watched in real time (--session seconds), throttling on
        # as the server asks by default: what the transcoder makes and
        # spends meanwhile, and whether the player ever waited.
        "session-720p": {
            "input": str(media / "film-long.mkv"),
            "video": {"mode": "transcode", "codec": "h264", "max_height": 720, "max_bitrate_kbps": 4000},
            "audio": aac_copy,
            "session": True,
        },
        "session-hdr4k": {
            "input": str(media / "hdr-long.mkv"),
            "video": {"mode": "transcode", "codec": "h264", "max_height": 1080, "max_bitrate_kbps": 8000},
            "audio": aac_copy,
            "session": True,
        },
        "session-remux": {
            "input": str(media / "hdr-long.mkv"),
            "video": {"mode": "copy"},
            "audio": aac_copy,
            "session": True,
        },
        # 4K HEVC 10-bit HDR10 to 1080p H.264 SDR: decode, tone mapping, scaling.
        "hdr4k-1080p": {
            "input": hdr,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 1080, "max_bitrate_kbps": 8000},
            "audio": aac_copy,
        },
        "1080p-720p": {
            "input": film,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 720, "max_bitrate_kbps": 4000},
            "audio": aac_copy,
        },
        "truehd-aac": {
            "input": str(media / "film-truehd.mkv"),
            "video": {"mode": "copy"},
            "audio": {"mode": "transcode", "codec": "aac", "channels": 2},
        },
        "dts-ac3": {
            "input": str(media / "film-dts.mkv"),
            "video": {"mode": "copy"},
            "audio": {"mode": "transcode", "codec": "ac3", "channels": 6},
        },
        "remux": {"input": remux, "video": {"mode": "copy"}, "audio": {"mode": "copy"}},
        "seek-720p": {
            "input": film,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 720, "max_bitrate_kbps": 4000},
            "audio": aac_copy,
            "seek": True,
        },
        "burn-720p": {
            "input": film,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 720, "max_bitrate_kbps": 4000},
            "audio": aac_copy,
            "subtitle": {"external_path": str(media / "film.srt")},
        },
        "burn-hdr4k": {
            "input": hdr,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 1080, "max_bitrate_kbps": 8000},
            "audio": aac_copy,
            "subtitle": {"external_path": str(media / "film.srt")},
        },
        # Picture subtitles (PGS, as on discs): film-pgs.sup, made by hand
        # from FFmpeg's FATE sample sub/pgs_sub.sup (FFmpeg has no PGS encoder).
        "pgs-720p": {
            "input": film,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 720, "max_bitrate_kbps": 4000},
            "audio": aac_copy,
            "subtitle": {"external_path": str(media / "film-pgs.sup")},
        },
        "pgs-hdr4k": {
            "input": hdr,
            "video": {"mode": "transcode", "codec": "h264", "max_height": 1080, "max_bitrate_kbps": 8000},
            "audio": aac_copy,
            "subtitle": {"external_path": str(media / "film-pgs.sup")},
        },
    }


VARIANTS = {
    # What the machine has: GPU encoders, decoders and tone mappers.
    "gpu": {"options": {"hwaccel": "auto", "hardware_decoding": True}, "cores": None},
    # No GPU at all: tone mapping on the processor too (zimg).
    "sw": {"options": {"hwaccel": "none", "hardware_decoding": False, "tonemap": "zscale"}, "cores": None},
    # No GPU, four processors: an Intel N100 or an old i5 with its encoder off.
    "sw4": {"options": {"hwaccel": "none", "hardware_decoding": False, "tonemap": "zscale"}, "cores": 4},
    # No GPU, two: a Celeron, an ARM board's big cores.
    "sw2": {"options": {"hwaccel": "none", "hardware_decoding": False, "tonemap": "zscale"}, "cores": 2},
}

# ---------------------------------------------------------------------------
# Process accounting
# ---------------------------------------------------------------------------

if WINDOWS:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    ntdll = ctypes.WinDLL("ntdll")

    class IO_COUNTERS(ctypes.Structure):
        _fields_ = [(n, ctypes.c_ulonglong) for n in (
            "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
            "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]

    class BASIC_LIMIT(ctypes.Structure):
        _fields_ = [
            ("PerProcessUserTimeLimit", ctypes.c_longlong),
            ("PerJobUserTimeLimit", ctypes.c_longlong),
            ("LimitFlags", wintypes.DWORD),
            ("MinimumWorkingSetSize", ctypes.c_size_t),
            ("MaximumWorkingSetSize", ctypes.c_size_t),
            ("ActiveProcessLimit", wintypes.DWORD),
            ("Affinity", ctypes.c_size_t),
            ("PriorityClass", wintypes.DWORD),
            ("SchedulingClass", wintypes.DWORD),
        ]

    class EXTENDED_LIMIT(ctypes.Structure):
        _fields_ = [
            ("BasicLimitInformation", BASIC_LIMIT),
            ("IoInfo", IO_COUNTERS),
            ("ProcessMemoryLimit", ctypes.c_size_t),
            ("JobMemoryLimit", ctypes.c_size_t),
            ("PeakProcessMemoryUsed", ctypes.c_size_t),
            ("PeakJobMemoryUsed", ctypes.c_size_t),
        ]

    class BASIC_ACCOUNTING(ctypes.Structure):
        _fields_ = [
            ("TotalUserTime", ctypes.c_longlong),
            ("TotalKernelTime", ctypes.c_longlong),
            ("ThisPeriodTotalUserTime", ctypes.c_longlong),
            ("ThisPeriodTotalKernelTime", ctypes.c_longlong),
            ("TotalPageFaultCount", wintypes.DWORD),
            ("TotalProcesses", wintypes.DWORD),
            ("ActiveProcesses", wintypes.DWORD),
            ("TotalTerminatedProcesses", wintypes.DWORD),
        ]

    JOB_OBJECT_LIMIT_AFFINITY = 0x10
    JOB_OBJECT_LIMIT_JOB_MEMORY = 0x200
    CREATE_SUSPENDED = 0x4

    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.OpenProcess.restype = wintypes.HANDLE


class Accounting:
    """Wraps the transcoder so that it and every process it starts are counted."""

    def __init__(self, cmd: list[str], env: dict, cores: int | None, mem_mb: int | None, log):
        self.cores = cores
        self.cpus = cores or os.cpu_count() or 1
        if WINDOWS:
            self.job = kernel32.CreateJobObjectW(None, None)
            info = EXTENDED_LIMIT()
            if cores:
                info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_AFFINITY
                # The first logical processor of each core: no SMT siblings.
                mask = 0
                for i in range(cores):
                    mask |= 1 << (2 * i)
                info.BasicLimitInformation.Affinity = mask
            if mem_mb:
                info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY
                info.JobMemoryLimit = mem_mb << 20
            if info.BasicLimitInformation.LimitFlags:
                ok = kernel32.SetInformationJobObject(self.job, 9, ctypes.byref(info), ctypes.sizeof(info))
                if not ok:
                    raise OSError(ctypes.get_last_error(), "SetInformationJobObject")
            self.proc = subprocess.Popen(cmd, env=env, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                         stderr=log, creationflags=CREATE_SUSPENDED)
            handle = kernel32.OpenProcess(0x1F0FFF, False, self.proc.pid)
            if not kernel32.AssignProcessToJobObject(self.job, handle):
                raise OSError(ctypes.get_last_error(), "AssignProcessToJobObject")
            ntdll.NtResumeProcess(handle)
            kernel32.CloseHandle(handle)
        else:
            self.result = tempfile.NamedTemporaryFile(prefix="pley-bench-", suffix=".json", delete=False).name
            preexec = None
            if cores:
                allowed = sorted(os.sched_getaffinity(0))[:cores]
                preexec = lambda: os.sched_setaffinity(0, allowed)  # noqa: E731
            self.proc = subprocess.Popen([sys.executable, "-c", LINUX_WRAPPER, self.result, *cmd], env=env,
                                         stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=log,
                                         preexec_fn=preexec)

    def snapshot(self) -> float:
        """Processor seconds so far (Windows; Linux reads them at the end)."""
        if not WINDOWS:
            return 0.0
        acc = BASIC_ACCOUNTING()
        kernel32.QueryInformationJobObject(self.job, 1, ctypes.byref(acc), ctypes.sizeof(acc), None)
        return (acc.TotalUserTime + acc.TotalKernelTime) / 1e7

    def finish(self) -> dict:
        """Stops the transcoder (stdin closed: --exit-with-parent) and reads the totals."""
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        if WINDOWS:
            cpu = self.snapshot()
            info = EXTENDED_LIMIT()
            kernel32.QueryInformationJobObject(self.job, 9, ctypes.byref(info), ctypes.sizeof(info), None)
            kernel32.CloseHandle(self.job)
            return {"cpu_s": cpu, "peak_proc_mb": info.PeakProcessMemoryUsed / 2**20,
                    "peak_all_mb": info.PeakJobMemoryUsed / 2**20}
        with open(self.result) as f:
            usage = json.load(f)
        os.unlink(self.result)
        return {"cpu_s": usage["cpu_s"], "peak_proc_mb": usage["maxrss_kb"] / 1024, "peak_all_mb": None}


# Runs the transcoder and writes what it and its (reaped) children used.
LINUX_WRAPPER = r"""
import json, resource, signal, subprocess, sys
p = subprocess.Popen(sys.argv[2:], stdin=sys.stdin)
signal.signal(signal.SIGTERM, lambda *a: p.terminate())
p.wait()
r = resource.getrusage(resource.RUSAGE_CHILDREN)
json.dump({"cpu_s": r.ru_utime + r.ru_stime, "maxrss_kb": r.ru_maxrss}, open(sys.argv[1], "w"))
"""

# ---------------------------------------------------------------------------
# Runs
# ---------------------------------------------------------------------------


def http(method: str, url: str, body: dict | None = None, timeout: float = 120) -> tuple[int, bytes]:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as err:
        return err.code, err.read()


def run_case(binary: str, name: str, case: dict, variant: str, args) -> dict:
    v = VARIANTS[variant]
    port = args.port
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ)
    env.pop("PLEY_TRANSCODER_TOKEN", None)
    env["PLEY_LOG"] = args.log_level
    cmd = [binary, "--listen", f"127.0.0.1:{port}", "--work-dir", args.work_dir, "--exit-with-parent",
           "--ffmpeg", args.ffmpeg, "--ffprobe", args.ffprobe]
    log = open(Path(args.work_dir) / "bench-transcoder.log", "ab")
    acct = Accounting(cmd, env, v["cores"], args.mem_mb, log)
    try:
        deadline = time.time() + 120
        while True:
            try:
                if http("GET", base + "/health", timeout=2)[0] == 200:
                    break
            except OSError:
                pass
            if time.time() > deadline or acct.proc.poll() is not None:
                raise RuntimeError("the transcoder did not start")
            time.sleep(0.1)
        cpu0 = acct.snapshot()
        request = {k: case[k] for k in ("input", "video", "audio", "subtitle") if k in case}
        request["segment_seconds"] = args.segment_seconds
        options = dict(v["options"])
        options["throttle"] = bool(case.get("session"))
        options["preset"] = args.preset
        request["options"] = options
        t0 = time.perf_counter()
        status, body = http("POST", base + "/v1/jobs", request)
        if status != 201:
            raise RuntimeError(f"job refused ({status}): {body[:300]!r}")
        created = json.loads(body)
        job, count, seg_s = created["id"], created["segment_count"], created["segment_seconds"]
        want = min(count, max(1, round(args.seconds / seg_s)))

        def fetch(n: int) -> int:
            s, data = http("GET", f"{base}/v1/jobs/{job}/{n}.ts")
            if s != 200:
                raise RuntimeError(f"segment {n}: {s} {data[:300]!r}")
            return len(data)

        total = fetch(0)
        start = time.perf_counter() - t0
        seek = None
        fetched = 1
        segments = list(range(1, want))
        if case.get("seek"):
            # Two more from the start, then the middle and on from there.
            for n in (1, 2):
                total += fetch(n)
                fetched += 1
            far = count // 2
            t = time.perf_counter()
            total += fetch(far)
            seek = time.perf_counter() - t
            fetched += 1
            segments = list(range(far + 1, min(count, far + want - 3)))
        t_first = time.perf_counter()
        stalls, stalled = 0, 0.0
        if case.get("session"):
            # A player: it shows the picture in real time from the first
            # segment on, and asks for the next one while it holds less than
            # its buffer ahead of what it shows; one late stalls it.
            n = 1
            while n < count:
                shown = time.perf_counter() - t_first - stalled
                if shown >= args.session:
                    break
                if n * seg_s - shown > args.player_buffer:
                    time.sleep(min(0.25, n * seg_s - shown - args.player_buffer))
                    continue
                total += fetch(n)
                fetched += 1
                late = (time.perf_counter() - t_first - stalled) - n * seg_s
                if late > 0:
                    stalls += 1
                    stalled += late
                n += 1
        else:
            for n in segments:
                total += fetch(n)
                fetched += 1
        rest = time.perf_counter() - t_first
        wall = time.perf_counter() - t0
        st = json.loads(http("GET", f"{base}/v1/jobs/{job}")[1])
        made = st.get("segments_ready", fetched)
        cpu_run = acct.snapshot() - cpu0
        http("DELETE", f"{base}/v1/jobs/{job}")
    finally:
        totals = acct.finish()
        log.close()
    cpu = cpu_run if WINDOWS else totals["cpu_s"]
    media_s = fetched * seg_s
    after_first = (fetched - 1) * seg_s
    return {
        "case": name,
        "variant": variant,
        "start_s": round(start, 3),
        "seek_s": None if seek is None else round(seek, 3),
        "speed_x": None if case.get("session") else round(after_first / rest, 2) if rest > 0 and after_first else None,
        "cpu_s_per_media_s": round(cpu / max(made * seg_s, media_s), 3),
        "cpu_s": round(cpu, 1),
        "cpu_pct": round(100 * cpu / wall / acct.cpus, 1),
        "peak_proc_mb": round(totals["peak_proc_mb"]),
        "peak_all_mb": None if totals["peak_all_mb"] is None else round(totals["peak_all_mb"]),
        "mbytes": round(total / 2**20, 2),
        "made_s": made * seg_s,
        "stalls": stalls,
        "stalled_s": round(stalled, 2),
        "media_s": media_s,
        "wall_s": round(wall, 2),
        "encoder": (st.get("video") or {}).get("encoder"),
        "pipeline": (st.get("video") or {}).get("pipeline"),
        "tonemap": (st.get("video") or {}).get("tonemap"),
        "preset": (st.get("process") or {}).get("preset"),
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--make-clips", metavar="DIR", help="make the test clips in DIR and stop")
    ap.add_argument("--source", help="4K HDR file the clips are cut from (else test patterns)")
    ap.add_argument("--media", help="folder of the clips (--make-clips)")
    ap.add_argument("--hdr", help="4K HEVC HDR10 file (default: the source)")
    ap.add_argument("--remux", help="file for the remux case (default: the HDR one)")
    ap.add_argument("--bin", action="append", default=[], metavar="NAME=PATH", help="transcoder build (repeatable)")
    ap.add_argument("--cases", default="all", help="comma-separated case names, or all")
    ap.add_argument("--variants", default="gpu,sw4", help=f"comma-separated, of {', '.join(VARIANTS)}")
    ap.add_argument("--repeat", type=int, default=1)
    ap.add_argument("--seconds", type=float, default=30, help="media fetched per run (seconds)")
    ap.add_argument("--segment-seconds", type=float, default=2, help="as the pley server asks")
    ap.add_argument("--session", type=float, default=180, help="session cases: seconds watched")
    ap.add_argument("--player-buffer", type=float, default=30, help="session cases: seconds a player holds ahead")
    ap.add_argument("--preset", default="auto", help="the server's default is auto")
    ap.add_argument("--mem-mb", type=int, help="Windows: memory limit of the transcoder and its FFmpeg")
    ap.add_argument("--ffmpeg", default="ffmpeg")
    ap.add_argument("--ffprobe", default="ffprobe")
    ap.add_argument("--port", type=int, default=17691)
    ap.add_argument("--work-dir", help="the transcoder's work folder (kept between runs: capabilities)")
    ap.add_argument("--log-level", default="warn")
    ap.add_argument("--out", help="append JSON lines here")
    args = ap.parse_args()

    if args.make_clips:
        make_clips(Path(args.make_clips), args.source, args.ffmpeg)
        return
    if not args.media or not args.bin:
        ap.error("--media and at least one --bin are needed")
    args.ffmpeg = shutil.which(args.ffmpeg) or args.ffmpeg
    args.ffprobe = shutil.which(args.ffprobe) or args.ffprobe
    args.work_dir = args.work_dir or tempfile.mkdtemp(prefix="pley-bench-")
    Path(args.work_dir).mkdir(parents=True, exist_ok=True)
    hdr = args.hdr or args.source
    if not hdr:
        ap.error("--hdr (or --source) names the 4K HDR file")
    all_cases = cases(Path(args.media), hdr, args.remux or hdr)
    names = (
        [n for n in all_cases if not n.startswith("session")] if args.cases == "all" else args.cases.split(",")
    )
    bins = [b.split("=", 1) for b in args.bin]
    # A first start tests the encoders (capabilities.json): not a run's work.
    for _, path in bins:
        warm = Accounting([path, "--listen", f"127.0.0.1:{args.port}", "--work-dir", args.work_dir,
                           "--exit-with-parent", "--ffmpeg", args.ffmpeg, "--ffprobe", args.ffprobe],
                          dict(os.environ), None, None, subprocess.DEVNULL)
        deadline = time.time() + 300
        while time.time() < deadline and warm.proc.poll() is None:
            try:
                if http("GET", f"http://127.0.0.1:{args.port}/v1/info", timeout=2)[0] == 200:
                    break
            except OSError:
                time.sleep(0.2)
        warm.finish()
    results: list[dict] = []
    out = open(args.out, "a") if args.out else None
    for r in range(args.repeat):
        order = bins if r % 2 == 0 else bins[::-1]
        for name in names:
            for variant in args.variants.split(","):
                for label, path in order:
                    try:
                        res = run_case(path, name, all_cases[name], variant, args)
                    except Exception as err:  # noqa: BLE001 - reported, the others go on
                        res = {"case": name, "variant": variant, "error": str(err)}
                    res["bin"] = label
                    res["round"] = r
                    results.append(res)
                    print(json.dumps(res), flush=True)
                    if out:
                        out.write(json.dumps(res) + "\n")
                        out.flush()
    print_table(results)


def print_table(results: list[dict]) -> None:
    keys = ["start_s", "seek_s", "speed_x", "cpu_s_per_media_s", "cpu_pct", "cpu_s", "peak_proc_mb", "peak_all_mb",
            "mbytes", "made_s", "stalls"]
    groups: dict[tuple, list[dict]] = {}
    for r in results:
        groups.setdefault((r["case"], r["variant"], r["bin"]), []).append(r)
    print()
    print("| case | variant | bin | " + " | ".join(keys) + " | n |")
    print("|---" * (len(keys) + 4) + "|")
    for (case, variant, label), rs in groups.items():
        ok = [r for r in rs if "error" not in r]
        cells = []
        for k in keys:
            vals = [r[k] for r in ok if r.get(k) is not None]
            cells.append(f"{statistics.median(vals):g}" if vals else "-")
        print(f"| {case} | {variant} | {label} | " + " | ".join(cells) + f" | {len(ok)}/{len(rs)} |")


if __name__ == "__main__":
    main()
