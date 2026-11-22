//! "Can this PC take the moving background?" Asked when Background animation is turned on in Settings: FluxCode measures its own CPU time for a few seconds while the scene moves, reads the PC's cores, memory and whether it runs on battery, and says plainly whether that is light, fine or heavy here.

/// How the moving background suits this PC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Light,
    Fine,
    Heavy,
}

/// The running check in Settings, or its result.
#[derive(Clone, Debug)]
pub enum Check {
    Running,
    Done(Level, String),
}

/// How long the scene runs while it is measured, after a moment to settle.
pub const SETTLE: std::time::Duration = std::time::Duration::from_millis(800);
pub const MEASURE: std::time::Duration = std::time::Duration::from_secs(3);

/// What matters about the PC here.
#[derive(Clone, Copy, Debug, Default)]
pub struct Specs {
    pub cores: usize,
    pub ram_gb: Option<f32>,
    pub on_battery: bool,
}

pub fn specs() -> Specs {
    Specs { cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1), ram_gb: ram_gb(), on_battery: on_battery() }
}

/// Seconds of CPU this process has used so far (user and kernel), where it can be read.
pub fn cpu_seconds() -> Option<f64> {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct FileTime {
            lo: u32,
            hi: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn GetProcessTimes(p: isize, created: *mut FileTime, exited: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        }
        let (mut a, mut b, mut k, mut u) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
        // SAFETY: four valid out-pointers and the process's own pseudo handle.
        if unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u) } == 0 {
            return None;
        }
        let t = |f: &FileTime| ((f.hi as u64) << 32 | f.lo as u64) as f64 / 1e7;
        Some(t(&k) + t(&u))
    }
    #[cfg(target_os = "linux")]
    {
        // Fields 14 and 15 (utime, stime) in clock ticks, after the ")" that ends the name.
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let rest = &stat[stat.rfind(')')? + 2..];
        let f: Vec<&str> = rest.split_whitespace().collect();
        let ticks = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
        Some(ticks / 100.)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

fn ram_gb() -> Option<f32> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatusEx {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended: u64,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(m: *mut MemoryStatusEx) -> i32;
        }
        let mut m = MemoryStatusEx { length: std::mem::size_of::<MemoryStatusEx>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virtual: 0, avail_virtual: 0, avail_extended: 0 };
        // SAFETY: `length` is set, as the call requires.
        if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
            return None;
        }
        Some(m.total_phys as f32 / (1u64 << 30) as f32)
    }
    #[cfg(not(windows))]
    {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb: f32 = info.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / (1024. * 1024.))
    }
}

fn on_battery() -> bool {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct PowerStatus {
            ac_line: u8,
            battery_flag: u8,
            battery_percent: u8,
            saver: u8,
            life: u32,
            full_life: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemPowerStatus(s: *mut PowerStatus) -> i32;
        }
        let mut s = PowerStatus::default();
        // SAFETY: a valid out-pointer.
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return false;
        }
        // 0: unplugged; 128 in the flags: no battery at all.
        s.ac_line == 0 && s.battery_flag & 128 == 0
    }
    #[cfg(not(windows))]
    {
        std::fs::read_dir("/sys/class/power_supply")
            .map(|d| d.flatten().any(|e| std::fs::read_to_string(e.path().join("status")).is_ok_and(|s| s.trim() == "Discharging")))
            .unwrap_or(false)
    }
}

/// The verdict for `core_pct` (percent of one core the measurement found, None when it can't be read here) on a PC with `s`, and the sentence that says it.
pub fn verdict(core_pct: Option<f32>, s: Specs) -> (Level, String) {
    let ram = s.ram_gb.map(|g| format!("{:.0} GB", g.round())).unwrap_or_else(|| "?".into());
    let spec = crate::trf!("{cores} cores, {ram} RAM"; cores => s.cores, ram => ram);
    let Some(pct) = core_pct else {
        // No measurement: go by the cores alone.
        let level = if s.cores >= 8 { Level::Fine } else { Level::Heavy };
        let text = match level {
            Level::Heavy => crate::trf!("Couldn't measure it here. With {spec}, it may be heavy: leave it off if FluxCode feels slow."; spec => spec),
            _ => crate::trf!("Couldn't measure it here, but {spec} should take it."; spec => spec),
        };
        return (level, text);
    };
    // Its share of the whole processor, as Task Manager shows it.
    let whole = pct / s.cores.max(1) as f32;
    let level = if whole < 2.5 && pct < 20. {
        Level::Light
    } else if whole < 6. {
        Level::Fine
    } else {
        Level::Heavy
    };
    let used = crate::trf!("{pct}% of one core ({whole}% of the whole CPU, {spec})"; pct => format!("{pct:.0}"), whole => format!("{whole:.1}"), spec => spec);
    let mut text = match level {
        Level::Light => crate::trf!("Your PC takes it easily: {used}."; used => used),
        Level::Fine => crate::trf!("It runs fine here: {used}. It is the heaviest thing FluxCode does while you type."; used => used),
        Level::Heavy => crate::trf!("Heavy for this PC: {used}. Leave it off unless you want the motion."; used => used),
    };
    if s.on_battery {
        text.push(' ');
        text.push_str(crate::i18n::tr("You are on battery: it drains a little faster while the scene moves."));
    }
    (level, text)
}

#[cfg(test)]
mod tests {
    use super::{verdict, Level, Specs};

    #[test]
    fn verdicts_follow_the_share_of_the_whole_cpu() {
        let big = Specs { cores: 16, ram_gb: Some(32.), on_battery: false };
        let small = Specs { cores: 2, ram_gb: Some(4.), on_battery: true };
        assert_eq!(verdict(Some(12.), big).0, Level::Light);
        assert_eq!(verdict(Some(12.), Specs { cores: 4, ..big }).0, Level::Fine);
        assert_eq!(verdict(Some(15.), small).0, Level::Heavy);
        assert!(verdict(Some(15.), small).1.contains("battery"));
        assert_eq!(verdict(None, small).0, Level::Heavy);
    }

    #[test]
    fn the_process_cpu_time_can_be_read() {
        if cfg!(any(windows, target_os = "linux")) {
            let a = super::cpu_seconds().expect("cpu time");
            let mut x = 0u64;
            for i in 0..5_000_000u64 {
                x = x.wrapping_add(i * i);
            }
            std::hint::black_box(x);
            assert!(super::cpu_seconds().unwrap() >= a);
        }
        assert!(super::specs().cores >= 1);
    }
}
