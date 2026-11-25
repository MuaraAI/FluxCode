use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const T_HOT: f64 = 1.0;
const T_ENV: f64 = 0.05;
const T_STAGE1_WARM: f64 = 0.65;
const T_STAGE2_COOL: f64 = 0.22;

const MAX_CACHED_FILES: usize = 6;
const MAX_SNAPSHOT_BYTES: usize = 200_000;
/// Maximum delta-to-full byte ratio (information gain threshold) before falling back to full resource.
const DELTA_RATIO_LIMIT: f64 = 0.40;

/// Adaptive thermodynamic + critically damped harmonic memory controller for a live ACP agent.
///
/// Uses Newton's Law of Cooling `T(t) = T_env + (T_0 - T_env) * exp(-lambda * t)` with an
/// EMA-estimated inter-turn half-life to trim cold Working Set pages and engage Windows EcoQoS
/// when quiet, without ever stopping or disconnecting the agent.
pub struct ThermalController {
    last_turn_at: Option<Instant>,
    ema_gap_secs: f64,
    last_ws_mb: f64,
    last_sample_at: Option<Instant>,
    pub stage: u8,
    pub eco_active: bool,
}

impl Default for ThermalController {
    fn default() -> Self {
        Self {
            last_turn_at: None,
            ema_gap_secs: 24.0,
            last_ws_mb: 0.0,
            last_sample_at: None,
            stage: 0,
            eco_active: false,
        }
    }
}

impl ThermalController {
    /// Records the start of a new user turn, updating the inter-arrival EMA (Little's Law rate)
    /// and resetting thermal temperature back to hot (`T = 1.0`).
    pub fn on_turn_start(&mut self, now: Instant) {
        if let Some(prev) = self.last_turn_at {
            let gap = now.saturating_duration_since(prev).as_secs_f64().clamp(2.0, 180.0);
            const ALPHA: f64 = 0.35;
            self.ema_gap_secs = (ALPHA * gap + (1.0 - ALPHA) * self.ema_gap_secs).clamp(6.0, 90.0);
        }
        self.last_turn_at = Some(now);
        self.stage = 0;
        self.eco_active = false;
    }

    /// Adaptive half-life tau_1/2 (seconds) derived from user's inter-turn arrival rhythm.
    pub fn half_life_secs(&self) -> f64 {
        (0.25 * self.ema_gap_secs).clamp(4.0, 24.0)
    }

    /// Cooling decay constant lambda = ln(2) / tau_1/2.
    pub fn lambda(&self) -> f64 {
        std::f64::consts::LN_2 / self.half_life_secs()
    }

    /// Newton's Law of Cooling temperature at elapsed quiet seconds `dt`.
    #[cfg(test)]
    pub fn temperature(&self, dt_secs: f64) -> f64 {
        let dt = dt_secs.max(0.0);
        T_ENV + (T_HOT - T_ENV) * (-self.lambda() * dt).exp()
    }

    /// Closed-form time to cool from `T_HOT` to target temperature `target`.
    pub fn time_to_temp(&self, target: f64) -> Duration {
        let ratio = ((target - T_ENV) / (T_HOT - T_ENV)).clamp(1e-4, 1.0);
        let secs = (-ratio.ln() / self.lambda()).clamp(2.0, 90.0);
        Duration::from_secs_f64(secs)
    }

    /// Post-boot settle delay to reclaim one-shot Bun/OpenCode startup bytecode pages.
    pub fn boot_settle_delay(&self) -> Duration {
        Duration::from_secs(3)
    }

    /// Stage 1 (Warm trim): evicts cold working-set pages while keeping normal CPU priority.
    pub fn stage1_delay(&self) -> Duration {
        self.time_to_temp(T_STAGE1_WARM)
    }

    /// Stage 2 (Cool EcoQoS + deep trim): second trim + Windows EcoQoS for near-zero idle power.
    pub fn stage2_delay(&self) -> Duration {
        self.time_to_temp(T_STAGE2_COOL)
    }

    /// Critically damped harmonic oscillator (`zeta = 1`, `c = 2 * sqrt(k)`) memory pressure check.
    ///
    /// Returns true when restoring force `F = -k*(M - M_eq) - c*(dM/dt)` indicates excess
    /// resident working set worth trimming, avoiding redundant trims once already lean.
    pub fn should_trim(&mut self, ws_mb: f64, now: Instant, force: bool) -> bool {
        let dt = self
            .last_sample_at
            .map(|t| now.saturating_duration_since(t).as_secs_f64().max(0.25))
            .unwrap_or(5.0);
        let prev = self.last_ws_mb;
        self.last_ws_mb = ws_mb;
        self.last_sample_at = Some(now);

        if ws_mb <= 0.1 {
            return force;
        }
        if force {
            return ws_mb > 36.0;
        }
        const M_EQ: f64 = 96.0;
        const K: f64 = 1.0;
        const C: f64 = 2.0; // critical damping c = 2 * sqrt(k)
        let displacement = ws_mb - M_EQ;
        let velocity = if prev > 0.1 { (ws_mb - prev) / dt } else { 0.0 };
        let force_net = -K * displacement - C * velocity;
        let potential_energy = 0.5 * K * displacement.max(0.0).powi(2);
        ws_mb > 48.0 && (potential_energy > 32.0 && force_net < 0.0)
    }
}

/// Statistical-mechanics Gibbs-Boltzmann softmax + Lagrangian water-filling budget allocator.
///
/// Each context block `i` supplies `(energy, min_bytes, actual_bytes)` where lower energy means
/// higher relevance (e.g. explicit `@wiztree` / `@file` / selection / errors).
pub fn boltzmann_allocate(items: &[(f64, usize, usize)], total_budget: usize, beta: f64) -> Vec<usize> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }
    let total_demand: usize = items.iter().map(|x| x.2).sum();
    if total_demand <= total_budget {
        return items.iter().map(|x| x.2).collect();
    }

    let min_e = items.iter().map(|x| x.0).fold(f64::INFINITY, f64::min);
    let weights: Vec<f64> = items.iter().map(|&(e, _, _)| (-beta * (e - min_e)).exp()).collect();

    let mut alloc = vec![0usize; n];
    // Guarantee floors first (clamped to actual size and remaining budget).
    let mut rem = total_budget;
    for (i, &(_, min_b, actual)) in items.iter().enumerate() {
        let floor = min_b.min(actual).min(rem);
        alloc[i] = floor;
        rem = rem.saturating_sub(floor);
    }

    // Iterative water-filling over Boltzmann weights for any remaining budget.
    for _ in 0..8 {
        if rem == 0 {
            break;
        }
        let mut active_w = 0.0f64;
        for i in 0..n {
            if alloc[i] < items[i].2 {
                active_w += weights[i];
            }
        }
        if active_w <= 0.0 {
            break;
        }
        let mut spent = 0usize;
        let start_rem = rem;
        for i in 0..n {
            let need = items[i].2.saturating_sub(alloc[i]);
            if need == 0 {
                continue;
            }
            let share = ((weights[i] / active_w) * start_rem as f64).floor() as usize;
            let give = share.min(need).min(rem.saturating_sub(spent));
            alloc[i] += give;
            spent += give;
        }
        rem = rem.saturating_sub(spent);
        if spent == 0 {
            // Distribute leftover rounding bytes to lowest-energy unsatisfied items.
            let mut order: Vec<usize> = (0..n).filter(|&i| alloc[i] < items[i].2).collect();
            order.sort_by(|&a, &b| items[a].0.total_cmp(&items[b].0));
            for i in order {
                if rem == 0 {
                    break;
                }
                let give = (items[i].2 - alloc[i]).min(rem);
                alloc[i] += give;
                rem -= give;
            }
            break;
        }
    }
    alloc
}

/// Lossless information-theoretic encoding result for a workspace file in the active ACP session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePayload {
    /// First time sent or large rewrite (`eta >= 0.40`): transmit full resource.
    Full,
    /// Conditional entropy `H(X_t | X_{t-1}) = 0`: exact byte-identical content already in session.
    Unchanged(String),
    /// Compact line-numbered diff hunks (`eta < 0.40`): lossless incremental update.
    Delta(String),
}

struct CachedDoc {
    hash: u64,
    lines: Vec<String>,
    seq: u64,
}

#[derive(Default)]
pub struct ContextCache {
    files: HashMap<PathBuf, CachedDoc>,
    seq: u64,
    pub git_sig: u64,
}

impl ContextCache {
    pub fn clear(&mut self) {
        self.files.clear();
        self.seq = 0;
        self.git_sig = 0;
    }

    pub fn encode_file(
        &mut self,
        path: &Path,
        rel: &str,
        body: &str,
        cursor_line: Option<usize>,
    ) -> FilePayload {
        let hash = fnv1a_64(body.as_bytes());
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;

        if let Some(prev) = self.files.get_mut(path) {
            prev.seq = seq;
            if prev.hash == hash {
                let loc = cursor_line.map(|l| format!(" (cursor line {l})")).unwrap_or_default();
                return FilePayload::Unchanged(format!(
                    "[FluxCode context] {rel}{loc} is unchanged since earlier in this session; reuse its earlier contents or read with your tools if needed."
                ));
            }
            let new_lines: Vec<String> = body.lines().map(String::from).collect();
            let delta = build_line_delta(rel, &prev.lines, &new_lines);
            prev.hash = hash;
            prev.lines = new_lines;
            if let Some(d) = delta {
                let ratio = d.len() as f64 / body.len().max(1) as f64;
                if ratio < DELTA_RATIO_LIMIT {
                    return FilePayload::Delta(d);
                }
            }
            return FilePayload::Full;
        }

        if body.len() <= MAX_SNAPSHOT_BYTES {
            self.evict_if_full();
            self.files.insert(
                path.to_path_buf(),
                CachedDoc {
                    hash,
                    lines: body.lines().map(String::from).collect(),
                    seq,
                },
            );
        }
        FilePayload::Full
    }

    fn evict_if_full(&mut self) {
        while self.files.len() >= MAX_CACHED_FILES {
            let Some(oldest) = self
                .files
                .iter()
                .min_by_key(|(_, v)| v.seq)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.files.remove(&oldest);
        }
    }
}

pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Builds a compact unified line-numbered delta between `old` and `new` snapshots using
/// prefix/suffix bounding and anchor resynchronization (pure Rust, O(N) fast path).
fn build_line_delta(rel: &str, old: &[String], new: &[String]) -> Option<String> {
    use std::fmt::Write;
    let common_prefix = old.iter().zip(new.iter()).take_while(|(a, b)| a == b).count();
    if common_prefix == old.len() && common_prefix == new.len() {
        return None;
    }
    let old_rem = &old[common_prefix..];
    let new_rem = &new[common_prefix..];
    let common_suffix = old_rem
        .iter()
        .rev()
        .zip(new_rem.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let old_end = old.len().saturating_sub(common_suffix);
    let new_end = new.len().saturating_sub(common_suffix);
    let old_mid = &old[common_prefix..old_end];
    let new_mid = &new[common_prefix..new_end];

    let ctx_before_start = common_prefix.saturating_sub(2);
    let ctx_after_end = (new_end + 2).min(new.len());

    let mut out = String::new();
    let _ = writeln!(
        out,
        "[FluxCode context delta] {rel} updated since earlier in this session ({} -> {} lines):\n@@ -{},{} +{},{} @@",
        old.len(),
        new.len(),
        ctx_before_start + 1,
        (old_end - ctx_before_start) + (ctx_after_end - new_end),
        ctx_before_start + 1,
        ctx_after_end - ctx_before_start,
    );
    for l in &new[ctx_before_start..common_prefix] {
        let _ = writeln!(out, " {l}");
    }
    for l in old_mid {
        let _ = writeln!(out, "-{l}");
    }
    for l in new_mid {
        let _ = writeln!(out, "+{l}");
    }
    for l in &new[new_end..ctx_after_end] {
        let _ = writeln!(out, " {l}");
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{boltzmann_allocate, ContextCache, FilePayload, ThermalController};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn newtons_cooling_and_harmonic_controller_behave_monotonically() {
        let mut tc = ThermalController::default();
        let t0 = Instant::now();
        tc.on_turn_start(t0);
        // Temperature decays monotonically from 1.0 toward T_ENV.
        assert!((tc.temperature(0.0) - 1.0).abs() < 1e-6);
        assert!(tc.temperature(5.0) < tc.temperature(1.0));
        assert!(tc.temperature(60.0) < tc.temperature(10.0));
        assert!(tc.stage1_delay() < tc.stage2_delay());

        // Faster user cadence shortens half-life cleanly.
        let fast_half = {
            let mut f = ThermalController::default();
            f.on_turn_start(t0);
            f.on_turn_start(t0 + Duration::from_secs(4));
            f.on_turn_start(t0 + Duration::from_secs(8));
            f.half_life_secs()
        };
        assert!(fast_half < tc.half_life_secs());

        // Critically damped harmonic pressure skips trim when lean, trims when bloated.
        assert!(!tc.should_trim(32.0, t0 + Duration::from_secs(5), false));
        assert!(tc.should_trim(320.0, t0 + Duration::from_secs(10), false));
    }

    #[test]
    fn boltzmann_water_filling_prioritizes_low_energy_blocks_without_waste() {
        // Explicit @wiztree (E=0.1, 2000B) & error (E=0.2, 500B) vs huge ambient map (E=1.5, 20000B)
        let items = vec![(0.1, 1000, 2000), (0.2, 400, 500), (1.5, 500, 20_000)];
        let alloc = boltzmann_allocate(&items, 6000, 1.6);
        assert_eq!(alloc[0], 2000);
        assert_eq!(alloc[1], 500);
        assert_eq!(alloc[2], 3500);
        assert_eq!(alloc.iter().sum::<usize>(), 6000);
    }

    #[test]
    fn context_cache_collapses_unchanged_and_compresses_small_edits() {
        let mut cache = ContextCache::default();
        let p = Path::new("src/main.rs");
        let lines: Vec<String> = (1..=120).map(|i| format!("let line_{i} = {i} * 2;")).collect();
        let v1 = lines.join("\n");

        // Turn 1: Full
        assert_eq!(cache.encode_file(p, "src/main.rs", &v1, Some(10)), FilePayload::Full);

        // Turn 2: Unchanged -> zero conditional entropy
        match cache.encode_file(p, "src/main.rs", &v1, Some(42)) {
            FilePayload::Unchanged(msg) => assert!(msg.contains("unchanged") && msg.contains("line 42")),
            other => panic!("expected Unchanged, got {other:?}"),
        }

        // Turn 3: 1-line edit -> compact delta (<40% of full size)
        let mut lines2 = lines.clone();
        lines2[50] = "let line_51 = 999;".into();
        let v2 = lines2.join("\n");
        match cache.encode_file(p, "src/main.rs", &v2, Some(51)) {
            FilePayload::Delta(delta) => {
                assert!(delta.contains("-let line_51 = 51 * 2;"));
                assert!(delta.contains("+let line_51 = 999;"));
                assert!(delta.len() * 5 < v2.len(), "delta should be drastically smaller than full file");
            }
            other => panic!("expected Delta, got {other:?}"),
        }
    }
}
