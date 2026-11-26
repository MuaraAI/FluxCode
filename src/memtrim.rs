use gpui_kit::{AsyncWindowContext, Context, Window};
use std::ops::Index;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static GEN: AtomicU64 = AtomicU64::new(0);
static ECO_ACTIVE: AtomicBool = AtomicBool::new(false);
static GOVERNOR: OnceLock<Mutex<MemoryGovernor>> = OnceLock::new();

const MIN_TRIM_GAP: Duration = Duration::from_secs(4);
const EQ_ACTIVE_MB: f64 = 38.0;
const EQ_BACKGROUND_MB: f64 = 24.0;

/// Contiguous string arena + byte-offset table (SoA/Arena layout: 2 heap allocations total
/// instead of N separate `String` heap blocks, cutting allocator metadata and boosting L1/L2 locality).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlatStrings {
    buf: String,
    ends: Vec<u32>,
}

impl FlatStrings {
    pub fn from_iter<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let it = iter.into_iter();
        let (low, _) = it.size_hint();
        let mut buf = String::with_capacity(low.saturating_mul(32));
        let mut ends = Vec::with_capacity(low);
        for s in it {
            let r = s.as_ref();
            if buf.len() + r.len() <= u32::MAX as usize {
                buf.push_str(r);
                ends.push(buf.len() as u32);
            }
        }
        buf.shrink_to_fit();
        ends.shrink_to_fit();
        Self { buf, ends }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    #[inline]
    pub fn get(&self, ix: usize) -> Option<&str> {
        let end = *self.ends.get(ix)? as usize;
        let start = ix.checked_sub(1).and_then(|p| self.ends.get(p).copied()).unwrap_or(0) as usize;
        self.buf.get(start..end)
    }

    pub fn iter(&self) -> FlatStringsIter<'_> {
        FlatStringsIter { arena: self, ix: 0 }
    }

    #[allow(dead_code)]
    pub fn allocated_bytes(&self) -> usize {
        self.buf.capacity() + self.ends.capacity() * std::mem::size_of::<u32>()
    }
}

impl Index<usize> for FlatStrings {
    type Output = str;

    #[inline]
    fn index(&self, ix: usize) -> &Self::Output {
        self.get(ix).expect("FlatStrings index out of bounds")
    }
}

pub struct FlatStringsIter<'a> {
    arena: &'a FlatStrings,
    ix: usize,
}

impl<'a> Iterator for FlatStringsIter<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let s = self.arena.get(self.ix)?;
        self.ix += 1;
        Some(s)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.arena.len().saturating_sub(self.ix);
        (rem, Some(rem))
    }
}

impl<'a> ExactSizeIterator for FlatStringsIter<'a> {}

/// Critically damped harmonic oscillator (`zeta = 1`, `c = 2*sqrt(k)`) + Newton cooling
/// cadence estimator (`T(t) = T_env + (T_0 - T_env) * exp(-lambda * t)`) for FluxCode's own process.
pub struct MemoryGovernor {
    last_blur_at: Option<Instant>,
    ema_gap_secs: f64,
    last_ws_mb: f64,
    last_sample_at: Option<Instant>,
    last_trim_at: Option<Instant>,
}

impl Default for MemoryGovernor {
    fn default() -> Self {
        Self {
            last_blur_at: None,
            ema_gap_secs: 18.0,
            last_ws_mb: 0.0,
            last_sample_at: None,
            last_trim_at: None,
        }
    }
}

impl MemoryGovernor {
    pub fn on_blur(&mut self, now: Instant) {
        if let Some(prev) = self.last_blur_at {
            let gap = now.saturating_duration_since(prev).as_secs_f64().clamp(2.0, 120.0);
            const ALPHA: f64 = 0.35;
            self.ema_gap_secs = (ALPHA * gap + (1.0 - ALPHA) * self.ema_gap_secs).clamp(5.0, 60.0);
        }
        self.last_blur_at = Some(now);
    }

    pub fn lambda(&self) -> f64 {
        let half_life = (0.3 * self.ema_gap_secs).clamp(3.0, 16.0);
        std::f64::consts::LN_2 / half_life
    }

    pub fn stage1_delay(&self) -> Duration {
        let secs = (-((0.60_f64 - 0.05) / 0.95).ln() / self.lambda()).clamp(3.0, 14.0);
        Duration::from_secs_f64(secs)
    }

    pub fn stage2_delay(&self) -> Duration {
        let secs = (-((0.20_f64 - 0.05) / 0.95).ln() / self.lambda()).clamp(10.0, 35.0);
        Duration::from_secs_f64(secs)
    }

    /// Evaluates critically damped restoring force `F = -k*(M - M_eq) - c*(dM/dt)`.
    pub fn should_trim(&mut self, ws_mb: f64, background: bool, now: Instant, force: bool) -> bool {
        if let Some(last) = self.last_trim_at {
            if !force && now.saturating_duration_since(last) < MIN_TRIM_GAP {
                return false;
            }
        }
        let dt = self
            .last_sample_at
            .map(|t| now.saturating_duration_since(t).as_secs_f64().max(0.25))
            .unwrap_or(4.0);
        let prev = self.last_ws_mb;
        self.last_ws_mb = ws_mb;
        self.last_sample_at = Some(now);

        let eq = if background { EQ_BACKGROUND_MB } else { EQ_ACTIVE_MB };
        if ws_mb <= 0.1 {
            return force;
        }
        if force {
            return ws_mb > eq * 0.9;
        }
        const K: f64 = 1.0;
        const C: f64 = 2.0; // critical damping c = 2 * sqrt(k)
        let displacement = ws_mb - eq;
        let velocity = if prev > 0.1 { (ws_mb - prev) / dt } else { 0.0 };
        let force_net = -K * displacement - C * velocity;
        let potential_energy = 0.5 * K * displacement.max(0.0).powi(2);
        ws_mb > eq && potential_energy > 8.0 && force_net < 0.0
    }

    pub fn mark_trimmed(&mut self, now: Instant) {
        self.last_trim_at = Some(now);
    }
}

fn governor() -> &'static Mutex<MemoryGovernor> {
    GOVERNOR.get_or_init(|| Mutex::new(MemoryGovernor::default()))
}

/// Enables Windows Low-Fragmentation Heap (LFH) once at startup so small AST/UI allocations
/// bucket cleanly instead of fragmenting committed pages.
pub fn init_process_heap() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetProcessHeap() -> isize;
            fn HeapSetInformation(heap: isize, class: u32, info: *const u32, len: usize) -> i32;
        }
        unsafe {
            let heap = GetProcessHeap();
            if heap != 0 {
                let lfh: u32 = 2;
                let _ = HeapSetInformation(heap, 0, &lfh, std::mem::size_of::<u32>());
            }
        }
    }
}

pub fn working_set_mb() -> f64 {
    crate::boot::memory_mb().0
}

pub fn set_eco_qos(enable: bool) {
    if ECO_ACTIVE.swap(enable, Ordering::Relaxed) == enable {
        return;
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct PowerThrottlingState {
            version: u32,
            control_mask: u32,
            state_mask: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn SetProcessInformation(proc: isize, class: u32, info: *const std::ffi::c_void, len: u32) -> i32;
        }
        const PROCESS_POWER_THROTTLING: u32 = 4;
        const EXECUTION_SPEED: u32 = 0x1;
        let pt = PowerThrottlingState {
            version: 1,
            control_mask: EXECUTION_SPEED,
            state_mask: if enable { EXECUTION_SPEED } else { 0 },
        };
        unsafe {
            let _ = SetProcessInformation(
                GetCurrentProcess(),
                PROCESS_POWER_THROTTLING,
                &pt as *const PowerThrottlingState as *const std::ffi::c_void,
                std::mem::size_of::<PowerThrottlingState>() as u32,
            );
        }
    }
}

pub fn trim() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn GetProcessHeap() -> isize;
            fn HeapCompact(heap: isize, flags: u32) -> usize;
            fn K32EmptyWorkingSet(process: isize) -> i32;
        }
        unsafe {
            let heap = GetProcessHeap();
            if heap != 0 {
                let _ = HeapCompact(heap, 0);
            }
            K32EmptyWorkingSet(GetCurrentProcess());
        }
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe {
            malloc_trim(0);
        }
    }
    governor().lock().unwrap_or_else(|e| e.into_inner()).mark_trimmed(Instant::now());
}

pub fn trim_if_needed(force: bool, background: bool) {
    let ws = working_set_mb();
    let should = governor()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .should_trim(ws, background, Instant::now(), force);
    if should {
        trim();
    }
}

pub fn watch<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    cx.spawn_in(window, async move |_, cx: &mut AsyncWindowContext| {
        cx.background_executor().timer(Duration::from_secs(4)).await;
        trim_if_needed(true, false);
    })
    .detach();
    cx.observe_window_activation(window, |_, window, cx| {
        let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
        if window.is_window_active() {
            set_eco_qos(false);
            return;
        }
        let (d1, d2) = {
            let mut g = governor().lock().unwrap_or_else(|e| e.into_inner());
            g.on_blur(Instant::now());
            (g.stage1_delay(), g.stage2_delay())
        };
        cx.spawn_in(window, async move |_, cx: &mut AsyncWindowContext| {
            cx.background_executor().timer(d1).await;
            if GEN.load(Ordering::Relaxed) != gen {
                return;
            }
            trim_if_needed(false, true);
            let rem = d2.saturating_sub(d1);
            if !rem.is_zero() {
                cx.background_executor().timer(rem).await;
            }
            if GEN.load(Ordering::Relaxed) == gen {
                set_eco_qos(true);
                trim_if_needed(true, true);
            }
        })
        .detach();
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_strings_pack_contiguously_and_slice_utf8() {
        let items = vec!["src/main.rs", "RP/texts/en_US.lang", "halo_ütf8.txt", ""];
        let arena = FlatStrings::from_iter(items.iter().copied());
        assert_eq!(arena.len(), items.len());
        for (i, expected) in items.iter().enumerate() {
            assert_eq!(arena.get(i), Some(*expected));
            assert_eq!(&arena[i], *expected);
        }
        assert_eq!(arena.get(items.len()), None);
        let collected: Vec<&str> = arena.iter().collect();
        assert_eq!(collected, items);
        assert!(arena.allocated_bytes() < 96);
    }

    #[test]
    fn memory_governor_damping_and_cooling_behave_monotonically() {
        let mut gov = MemoryGovernor::default();
        let t0 = Instant::now();
        assert!(gov.stage1_delay() < gov.stage2_delay());
        assert!(!gov.should_trim(18.0, false, t0, false));
        let _ = gov.should_trim(64.0, false, t0 + Duration::from_secs(5), false);
        assert!(gov.should_trim(90.0, false, t0 + Duration::from_secs(10), false));
        gov.mark_trimmed(t0 + Duration::from_secs(10));
        assert!(!gov.should_trim(90.0, false, t0 + Duration::from_secs(11), false));
    }
}
