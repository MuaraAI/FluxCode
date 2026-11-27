use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const T_HOT: f64 = 1.0;
const T_ENV: f64 = 0.05;
const T_STAGE1_WARM: f64 = 0.65;
const T_STAGE2_COOL: f64 = 0.22;

const MAX_CACHED_FILES: usize = 12;
const MAX_CACHE_TOTAL_BYTES: usize = 1_200_000;
const MAX_SNAPSHOT_BYTES: usize = 200_000;
/// Information-weighted delta-to-full ratio ceiling before falling back to full resource.
const DELTA_INFO_LIMIT: f64 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProfile {
    OpenCode,
    Antigravity,
    NodeV8,
    PythonUv,
    Native,
}

impl RuntimeProfile {
    pub fn for_agent(id: &str, cmd: Option<&Path>) -> Self {
        if crate::acp::is_opencode(id) {
            return Self::OpenCode;
        }
        if id == "antigravity-acp" {
            return Self::Antigravity;
        }
        if matches!(
            id,
            "claude-acp"
                | "codex-acp"
                | "gemini"
                | "github-copilot-cli"
                | "qwen-code"
                | "kimi-code"
                | "cline"
                | "auggie"
                | "factory-droid"
                | "codebuddy-code"
                | "deepagents"
                | "dimcode"
                | "dirac"
                | "glm-acp-agent"
                | "grok-build"
                | "minimax-code"
                | "nova"
                | "pi-acp"
                | "qoder"
        ) {
            return Self::NodeV8;
        }
        if matches!(id, "fast-agent" | "minion-code") {
            return Self::PythonUv;
        }
        if let Some(p) = cmd {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.starts_with("node") || name.ends_with(".js") || name.ends_with(".mjs") {
                return Self::NodeV8;
            }
            if name.starts_with("uvx") || name.starts_with("python") {
                return Self::PythonUv;
            }
        }
        Self::Native
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenCode => "OpenCode Runtime",
            Self::Antigravity => "Antigravity Runtime",
            Self::NodeV8 => "Node.js V8 Runtime",
            Self::PythonUv => "Python uv Runtime",
            Self::Native => "Native ACP Runtime",
        }
    }
}

pub fn is_optimized(_id: &str) -> bool {
    true
}

/// Gunther's Universal Scalability Law peak `N_max = floor(sqrt((1 - alpha) / beta))`.
pub fn usl_optimal_workers(logical_cores: usize, alpha: f64, beta: f64) -> usize {
    let cores = logical_cores.clamp(2, 8);
    if beta <= 0.0 || alpha >= 1.0 {
        return cores;
    }
    let peak = ((1.0 - alpha) / beta).sqrt().floor() as usize;
    peak.clamp(2, cores)
}

/// Denning Working Set equilibrium scaled smoothly to host physical RAM via `tanh`.
pub fn denning_equilibrium_mb(profile: RuntimeProfile, total_ram_mb: f64) -> f64 {
    let (min_mb, max_mb) = match profile {
        RuntimeProfile::OpenCode => (44.0, 88.0),
        RuntimeProfile::Antigravity => (48.0, 96.0),
        RuntimeProfile::NodeV8 => (68.0, 136.0),
        RuntimeProfile::PythonUv => (52.0, 104.0),
        RuntimeProfile::Native => (44.0, 88.0),
    };
    let ratio = (total_ram_mb.max(1024.0) / 16_384.0).tanh();
    min_mb + (max_mb - min_mb) * ratio
}

/// Applies non-destructive runtime environment tuning per ACP without overriding user env vars.
pub fn apply_agent_runtime_env(cmd: &mut std::process::Command, id: &str, cmd_path: &Path) {
    let set_if_unset = |c: &mut std::process::Command, k: &str, v: &str| {
        if std::env::var_os(k).is_none() {
            c.env(k, v);
        }
    };
    cmd.env("NO_COLOR", "1").env("FORCE_COLOR", "0");
    set_if_unset(cmd, "DO_NOT_TRACK", "1");

    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let workers = usl_optimal_workers(cores, 0.08, 0.045);
    let uv_pool = workers.clamp(2, 4).to_string();
    let go_procs = workers.to_string();
    let profile = RuntimeProfile::for_agent(id, Some(cmd_path));

    match profile {
        RuntimeProfile::OpenCode => {
            // Keep all ACP tools, search, and models intact; suppress redundant watcher/update/log churn.
            set_if_unset(cmd, "OPENCODE_DISABLE_FILEWATCHER", "1");
            set_if_unset(cmd, "OPENCODE_FILEWATCHER_DISABLE", "1");
            set_if_unset(cmd, "OPENCODE_DISABLE_AUTOUPDATE", "1");
            set_if_unset(cmd, "OPENCODE_LOG_LEVEL", "error");
            set_if_unset(cmd, "UV_THREADPOOL_SIZE", &uv_pool);
        }
        RuntimeProfile::Antigravity => {
            set_if_unset(cmd, "GOGC", "75");
            set_if_unset(cmd, "GOMAXPROCS", &go_procs);
            set_if_unset(cmd, "PYTHONDONTWRITEBYTECODE", "1");
        }
        RuntimeProfile::NodeV8 => {
            if id == "claude-acp" {
                set_if_unset(cmd, "DISABLE_AUTOUPDATER", "1");
            }
            set_if_unset(cmd, "NODE_NO_WARNINGS", "1");
            set_if_unset(cmd, "UV_THREADPOOL_SIZE", &uv_pool);
            let is_node_bin = cmd_path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| {
                    let l = n.to_ascii_lowercase();
                    l.starts_with("node") || l.ends_with(".js") || l.ends_with(".mjs")
                });
            if is_node_bin {
                let existing = std::env::var("NODE_OPTIONS").unwrap_or_default();
                if !existing.contains("--max-semi-space-size") {
                    let next = if existing.trim().is_empty() {
                        "--max-semi-space-size=16".to_string()
                    } else {
                        format!("{existing} --max-semi-space-size=16")
                    };
                    cmd.env("NODE_OPTIONS", next);
                }
            }
        }
        RuntimeProfile::PythonUv => {
            set_if_unset(cmd, "PYTHONDONTWRITEBYTECODE", "1");
            set_if_unset(cmd, "PYTHONMALLOC", "pymalloc");
        }
        RuntimeProfile::Native => {
            set_if_unset(cmd, "GOGC", "75");
            set_if_unset(cmd, "GOMAXPROCS", &go_procs);
        }
    }
}

/// 2-state Continuous-Discrete Extended Kalman Filter + Lyapunov critically damped harmonic
/// controller + Weibull-modulated Newton cooling for any live ACP child Job Object.
pub struct ThermalController {
    pub profile: RuntimeProfile,
    pub eq_mb: f64,
    last_turn_at: Option<Instant>,
    ema_gap_secs: f64,
    ema_gap_var: f64,
    kalman_init: bool,
    x_ws: f64,
    x_vel: f64,
    p00: f64,
    p01: f64,
    p11: f64,
    last_sample_at: Option<Instant>,
    pub stage: u8,
    pub eco_active: bool,
    pub trims_done: u32,
    pub last_filtered_ws_mb: f64,
}

impl Default for ThermalController {
    fn default() -> Self {
        Self::for_profile(RuntimeProfile::Native, crate::boot::total_ram_mb())
    }
}

impl ThermalController {
    pub fn for_agent(id: &str) -> Self {
        let cmd = crate::acp::locate(id).map(|l| l.cmd);
        let profile = RuntimeProfile::for_agent(id, cmd.as_deref());
        Self::for_profile(profile, crate::boot::total_ram_mb())
    }

    pub fn for_profile(profile: RuntimeProfile, total_ram_mb: f64) -> Self {
        let eq_mb = denning_equilibrium_mb(profile, total_ram_mb);
        Self {
            profile,
            eq_mb,
            last_turn_at: None,
            ema_gap_secs: 24.0,
            ema_gap_var: 36.0,
            kalman_init: false,
            x_ws: eq_mb,
            x_vel: 0.0,
            p00: 400.0,
            p01: 0.0,
            p11: 64.0,
            last_sample_at: None,
            stage: 0,
            eco_active: false,
            trims_done: 0,
            last_filtered_ws_mb: 0.0,
        }
    }

    /// Updates online inter-turn arrival mean and variance, resetting temperature to hot.
    pub fn on_turn_start(&mut self, now: Instant) {
        if let Some(prev) = self.last_turn_at {
            let gap = now
                .saturating_duration_since(prev)
                .as_secs_f64()
                .clamp(2.0, 180.0);
            const ALPHA: f64 = 0.35;
            let diff = gap - self.ema_gap_secs;
            self.ema_gap_secs = (ALPHA * gap + (1.0 - ALPHA) * self.ema_gap_secs).clamp(6.0, 90.0);
            self.ema_gap_var =
                ((1.0 - ALPHA) * (self.ema_gap_var + ALPHA * diff * diff)).clamp(4.0, 3600.0);
        }
        self.last_turn_at = Some(now);
        self.stage = 0;
        self.eco_active = false;
    }

    pub fn record_trim(&mut self) {
        self.trims_done = self.trims_done.saturating_add(1);
    }

    pub fn half_life_secs(&self) -> f64 {
        (0.25 * self.ema_gap_secs).clamp(4.0, 24.0)
    }

    pub fn lambda(&self) -> f64 {
        std::f64::consts::LN_2 / self.half_life_secs()
    }

    #[cfg(test)]
    pub fn temperature(&self, dt_secs: f64) -> f64 {
        let dt = dt_secs.max(0.0);
        T_ENV + (T_HOT - T_ENV) * (-self.lambda() * dt).exp()
    }

    /// Weibull-burstiness modulated cooling time with Albers CMOS break-even floor.
    pub fn time_to_temp(&self, target: f64) -> Duration {
        let ratio = ((target - T_ENV) / (T_HOT - T_ENV)).clamp(1e-4, 1.0);
        let cv = (self.ema_gap_var.sqrt() / self.ema_gap_secs.max(1.0)).clamp(0.0, 2.0);
        let base_secs = -ratio.ln() / self.lambda();
        let floor = if target <= T_STAGE2_COOL { 6.0 } else { 2.0 };
        let secs = (base_secs / (1.0 + 0.15 * cv)).clamp(floor, 90.0);
        Duration::from_secs_f64(secs)
    }

    pub fn boot_settle_delay(&self) -> Duration {
        Duration::from_secs(3)
    }

    pub fn stage1_delay(&self) -> Duration {
        self.time_to_temp(T_STAGE1_WARM)
    }

    pub fn stage2_delay(&self) -> Duration {
        self.time_to_temp(T_STAGE2_COOL)
    }

    /// Ornstein-Uhlenbeck state prediction + Joseph stabilized covariance Kalman update.
    fn kalman_update(&mut self, z_ws_mb: f64, dt: f64) -> (f64, f64) {
        if !self.kalman_init {
            self.kalman_init = true;
            self.x_ws = z_ws_mb;
            self.x_vel = 0.0;
            self.p00 = 100.0;
            self.p01 = 0.0;
            self.p11 = 25.0;
            self.last_filtered_ws_mb = z_ws_mb;
            return (self.x_ws, self.x_vel);
        }

        const GAMMA: f64 = 0.25;
        const SIGMA_A2: f64 = 9.0;
        const R_MEAS: f64 = 6.25;

        let phi = (-GAMMA * dt).exp();
        let x0_pred = self.x_ws + self.x_vel * dt;
        let x1_pred = self.x_vel * phi;

        // F = [[1, dt], [0, phi]]; compute P^- = F * P * F^T + Q
        let fp00 = self.p00 + dt * self.p01;
        let fp01 = self.p01 + dt * self.p11;
        let fp11 = phi * self.p11;

        let q00 = SIGMA_A2 * (dt * dt * dt) / 3.0;
        let q01 = SIGMA_A2 * (dt * dt) / 2.0;
        let q11 = SIGMA_A2 * dt;

        let pm00 = fp00 + dt * fp01 + q00;
        let pm01 = phi * fp01 + q01;
        let pm11 = phi * fp11 + q11;

        let innov = z_ws_mb - x0_pred;
        let s = (pm00 + R_MEAS).max(1e-6);
        let k0 = pm00 / s;
        let k1 = pm01 / s;

        self.x_ws = (x0_pred + k0 * innov).max(0.0);
        self.x_vel = x1_pred + k1 * innov;

        // Joseph form P = (I - KH) P^- (I - KH)^T + K R K^T preserves positive definiteness.
        let one_m_k0 = 1.0 - k0;
        let a00 = one_m_k0 * pm00;
        let a01 = one_m_k0 * pm01;
        let a10 = -k1 * pm00 + pm01;
        let a11 = -k1 * pm01 + pm11;

        let next_p00 = (a00 * one_m_k0 + k0 * k0 * R_MEAS).max(1e-6);
        let next_p01 = a00 * (-k1) + a01 + k0 * k1 * R_MEAS;
        let next_p11 = (a10 * (-k1) + a11 + k1 * k1 * R_MEAS).max(1e-6);

        self.p00 = next_p00;
        self.p01 = next_p01;
        self.p11 = next_p11;
        self.last_filtered_ws_mb = self.x_ws;
        (self.x_ws, self.x_vel)
    }

    /// Evaluates critically damped Lyapunov energy `V(e, de/dt)` and restoring force to decide trim.
    pub fn should_trim(&mut self, ws_mb: f64, now: Instant, force: bool) -> bool {
        let dt = self
            .last_sample_at
            .map(|t| {
                now.saturating_duration_since(t)
                    .as_secs_f64()
                    .clamp(0.25, 120.0)
            })
            .unwrap_or(5.0);
        self.last_sample_at = Some(now);

        if ws_mb <= 0.1 {
            return force;
        }
        let (ws_est, vel_est) = self.kalman_update(ws_mb, dt);
        if force {
            return ws_est > self.eq_mb * 0.55;
        }
        const K: f64 = 1.0;
        const C: f64 = 2.0; // critical damping c = 2 * sqrt(k)
        const EPS: f64 = 0.25;
        let e = ws_est - self.eq_mb;
        let force_net = -K * e - C * vel_est;
        let lyapunov_v =
            0.5 * K * e.max(0.0).powi(2) + 0.5 * vel_est.powi(2) + EPS * e.max(0.0) * vel_est;
        e > 0.0 && lyapunov_v > 24.0 && force_net < 0.0
    }
}

/// KKT-dual safeguarded Newton-Raphson solver for Gibbs-Boltzmann utility allocation.
///
/// Maximizes `sum w_i * ln(1 + delta_i / s_i)` subject to `floor_i <= x_i <= actual_i`
/// and `sum x_i = min(total_budget, sum actual_i)`.
pub fn boltzmann_allocate(
    items: &[(f64, usize, usize)],
    total_budget: usize,
    beta: f64,
) -> Vec<usize> {
    let n = items.len();
    if n == 0 || total_budget == 0 {
        return vec![0; n];
    }
    let total_demand: usize = items.iter().map(|x| x.2).sum();
    if total_demand <= total_budget {
        return items.iter().map(|x| x.2).collect();
    }

    let min_e = items.iter().map(|x| x.0).fold(f64::INFINITY, f64::min);
    let max_e = items.iter().map(|x| x.0).fold(f64::NEG_INFINITY, f64::max);
    let spread = (max_e - min_e).max(0.25);
    let beta_eff = (beta * (1.2 / spread).clamp(0.75, 1.5)).max(0.1);
    let raw_w: Vec<f64> = items
        .iter()
        .map(|&(e, _, _)| (-beta_eff * (e - min_e)).exp())
        .collect();
    let z = raw_w.iter().sum::<f64>().max(1e-12);
    let weights: Vec<f64> = raw_w.into_iter().map(|w| w / z).collect();

    let mut alloc = vec![0usize; n];
    let floors: Vec<usize> = items
        .iter()
        .map(|&(_, min_b, actual)| min_b.min(actual))
        .collect();
    let floor_sum: usize = floors.iter().sum();

    if floor_sum >= total_budget {
        // Scale floors proportionally by priority weight when combined floors exceed total budget.
        let weighted_floor_sum: f64 = floors
            .iter()
            .enumerate()
            .map(|(i, &f)| f as f64 * weights[i])
            .sum::<f64>()
            .max(1e-9);
        let mut rem = total_budget;
        for i in 0..n {
            let share = ((floors[i] as f64 * weights[i] / weighted_floor_sum) * total_budget as f64)
                .floor() as usize;
            let give = share.min(floors[i]).min(rem);
            alloc[i] = give;
            rem -= give;
        }
        let mut order: Vec<usize> = (0..n).filter(|&i| alloc[i] < floors[i]).collect();
        order.sort_by(|&a, &b| items[a].0.total_cmp(&items[b].0));
        for i in order {
            if rem == 0 {
                break;
            }
            let give = (floors[i] - alloc[i]).min(rem);
            alloc[i] += give;
            rem -= give;
        }
        return alloc;
    }

    alloc.copy_from_slice(&floors);
    let rem_budget = total_budget - floor_sum;
    let caps: Vec<f64> = items
        .iter()
        .enumerate()
        .map(|(i, &(_, _, actual))| (actual - floors[i]) as f64)
        .collect();
    let scales: Vec<f64> = caps.iter().map(|&c| (0.05 * c).max(64.0)).collect();

    let eval = |nu: f64| -> (f64, f64) {
        let mut total = 0.0;
        let mut deriv = 0.0;
        for i in 0..n {
            if caps[i] <= 0.0 {
                continue;
            }
            let raw = weights[i] / nu - scales[i];
            if raw <= 0.0 {
                continue;
            }
            if raw >= caps[i] {
                total += caps[i];
            } else {
                total += raw;
                deriv -= weights[i] / (nu * nu);
            }
        }
        (total, deriv)
    };

    let target = rem_budget as f64;
    let mut nu_hi = (0..n)
        .filter(|&i| caps[i] > 0.0)
        .map(|i| weights[i] / scales[i])
        .fold(1e-12_f64, f64::max)
        * 2.0;
    let mut nu_lo = (0..n)
        .filter(|&i| caps[i] > 0.0)
        .map(|i| weights[i] / (caps[i] + scales[i]))
        .fold(f64::INFINITY, f64::min)
        * 0.5;
    if !nu_lo.is_finite() || nu_lo <= 0.0 {
        nu_lo = 1e-12;
    }
    while eval(nu_lo).0 < target && nu_lo > 1e-18 {
        nu_lo *= 0.25;
    }
    while eval(nu_hi).0 > target && nu_hi < 1e12 {
        nu_hi *= 4.0;
    }

    let mut nu = 0.5 * (nu_lo + nu_hi);
    for _ in 0..24 {
        let (val, deriv) = eval(nu);
        let err = val - target;
        if err.abs() < 0.25 {
            break;
        }
        if err > 0.0 {
            nu_lo = nu;
        } else {
            nu_hi = nu;
        }
        let next = if deriv.abs() > 1e-18 {
            nu - err / deriv
        } else {
            0.5 * (nu_lo + nu_hi)
        };
        nu = if next > nu_lo && next < nu_hi {
            next
        } else {
            0.5 * (nu_lo + nu_hi)
        };
    }

    let mut spent = 0usize;
    for i in 0..n {
        if caps[i] <= 0.0 {
            continue;
        }
        let delta = (weights[i] / nu - scales[i]).clamp(0.0, caps[i]).floor() as usize;
        let give = delta.min(items[i].2 - alloc[i]).min(rem_budget - spent);
        alloc[i] += give;
        spent += give;
    }

    let mut leftover = rem_budget - spent;
    if leftover > 0 {
        let mut order: Vec<usize> = (0..n).filter(|&i| alloc[i] < items[i].2).collect();
        order.sort_by(|&a, &b| items[a].0.total_cmp(&items[b].0));
        for i in order {
            if leftover == 0 {
                break;
            }
            let give = (items[i].2 - alloc[i]).min(leftover);
            alloc[i] += give;
            leftover -= give;
        }
    }
    alloc
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePayload {
    Full,
    Unchanged(String),
    Delta(String),
}

struct CachedDoc {
    hash: u64,
    bytes: usize,
    lines: Vec<String>,
    line_hashes: Vec<u64>,
    age_reg: u8,
    lrfu: f64,
    last_seq: u64,
}

#[derive(Default)]
pub struct ContextCache {
    files: HashMap<PathBuf, CachedDoc>,
    total_bytes: usize,
    seq: u64,
    pub git_sig: u64,
    pub hits_unchanged: u32,
    pub hits_delta: u32,
    pub bytes_saved: usize,
}

impl ContextCache {
    pub fn clear(&mut self) {
        self.files.clear();
        self.total_bytes = 0;
        self.seq = 0;
        self.git_sig = 0;
        self.hits_unchanged = 0;
        self.hits_delta = 0;
        self.bytes_saved = 0;
    }

    fn age_step(&mut self) {
        for doc in self.files.values_mut() {
            doc.age_reg >>= 1;
        }
    }

    fn record_access(doc: &mut CachedDoc, seq: u64) {
        let dt = seq.saturating_sub(doc.last_seq) as f64;
        doc.lrfu = 1.0 + (-0.15 * dt).exp() * doc.lrfu;
        doc.age_reg |= 0x80;
        doc.last_seq = seq;
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
        self.age_step();

        if let Some(prev) = self.files.get_mut(path) {
            Self::record_access(prev, seq);
            if prev.hash == hash {
                let loc = cursor_line
                    .map(|l| format!(" (cursor line {l})"))
                    .unwrap_or_default();
                let note = format!(
                    "[FluxCode context] {rel}{loc} is unchanged since earlier in this session; reuse its earlier contents or read with your tools if needed."
                );
                self.hits_unchanged = self.hits_unchanged.saturating_add(1);
                self.bytes_saved = self
                    .bytes_saved
                    .saturating_add(body.len().saturating_sub(note.len()));
                return FilePayload::Unchanged(note);
            }

            let new_lines: Vec<String> = body.lines().map(String::from).collect();
            let new_hashes: Vec<u64> = new_lines.iter().map(|l| fnv1a_64(l.as_bytes())).collect();
            let delta = build_multi_hunk_delta(
                rel,
                &prev.lines,
                &prev.line_hashes,
                &new_lines,
                &new_hashes,
            );

            let old_bytes = prev.bytes;
            let new_bytes = if body.len() <= MAX_SNAPSHOT_BYTES {
                body.len()
            } else {
                0
            };
            self.total_bytes = self.total_bytes.saturating_sub(old_bytes);
            if new_bytes > 0 {
                prev.hash = hash;
                prev.bytes = new_bytes;
                prev.lines = new_lines;
                prev.line_hashes = new_hashes;
                self.total_bytes = self.total_bytes.saturating_add(new_bytes);
            } else {
                self.files.remove(path);
            }

            if let Some(d) = delta {
                let h_d = shannon_entropy_bpb(d.as_bytes());
                let h_x = shannon_entropy_bpb(body.as_bytes());
                let eta_info = (d.len() as f64 * (1.0 + 0.15 * h_d / 8.0))
                    / (body.len().max(1) as f64 * (1.0 + 0.15 * h_x / 8.0));
                if eta_info < DELTA_INFO_LIMIT && d.len() + 96 < body.len() {
                    self.hits_delta = self.hits_delta.saturating_add(1);
                    self.bytes_saved = self
                        .bytes_saved
                        .saturating_add(body.len().saturating_sub(d.len()));
                    return FilePayload::Delta(d);
                }
            }
            return FilePayload::Full;
        }

        if body.len() <= MAX_SNAPSHOT_BYTES {
            self.evict_if_needed(body.len());
            let lines: Vec<String> = body.lines().map(String::from).collect();
            let line_hashes: Vec<u64> = lines.iter().map(|l| fnv1a_64(l.as_bytes())).collect();
            self.total_bytes = self.total_bytes.saturating_add(body.len());
            self.files.insert(
                path.to_path_buf(),
                CachedDoc {
                    hash,
                    bytes: body.len(),
                    lines,
                    line_hashes,
                    age_reg: 0x80,
                    lrfu: 1.0,
                    last_seq: seq,
                },
            );
        }
        FilePayload::Full
    }

    fn evict_if_needed(&mut self, incoming_bytes: usize) {
        while self.files.len() >= MAX_CACHED_FILES
            || (!self.files.is_empty() && self.total_bytes + incoming_bytes > MAX_CACHE_TOTAL_BYTES)
        {
            let cur_seq = self.seq;
            let victim = self
                .files
                .iter()
                .min_by(|(_, a), (_, b)| {
                    let sa = a.lrfu * (-0.15 * cur_seq.saturating_sub(a.last_seq) as f64).exp();
                    let sb = b.lrfu * (-0.15 * cur_seq.saturating_sub(b.last_seq) as f64).exp();
                    sa.total_cmp(&sb)
                        .then(a.age_reg.cmp(&b.age_reg))
                        .then(a.last_seq.cmp(&b.last_seq))
                })
                .map(|(k, _)| k.clone());
            let Some(k) = victim else { break };
            if let Some(removed) = self.files.remove(&k) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.bytes);
            }
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

/// Zero-allocation Shannon entropy in bits-per-byte using a stack `[u32; 256]` histogram.
pub fn shannon_entropy_bpb(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let n = bytes.len() as f64;
    let mut h = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// Patience LIS multi-hunk unified diff over precomputed line hashes (pure Rust).
fn build_multi_hunk_delta(
    rel: &str,
    old: &[String],
    old_h: &[u64],
    new: &[String],
    new_h: &[u64],
) -> Option<String> {
    use std::fmt::Write;
    let prefix = old_h
        .iter()
        .zip(new_h.iter())
        .enumerate()
        .take_while(|&(i, (a, b))| a == b && old[i] == new[i])
        .count();
    if prefix == old.len() && prefix == new.len() {
        return None;
    }
    let max_suf = (old.len() - prefix).min(new.len() - prefix);
    let suffix = (0..max_suf)
        .take_while(|&k| {
            let oi = old.len() - 1 - k;
            let ni = new.len() - 1 - k;
            old_h[oi] == new_h[ni] && old[oi] == new[ni]
        })
        .count();

    let old_end = old.len() - suffix;
    let new_end = new.len() - suffix;

    // Collect unique matching anchors inside `[prefix..old_end)` x `[prefix..new_end)`.
    let mut old_counts: HashMap<u64, (usize, usize)> = HashMap::new();
    for (i, &h) in old_h.iter().enumerate().take(old_end).skip(prefix) {
        old_counts
            .entry(h)
            .and_modify(|e| e.0 += 1)
            .or_insert((1, i));
    }
    let mut new_counts: HashMap<u64, (usize, usize)> = HashMap::new();
    for (j, &h) in new_h.iter().enumerate().take(new_end).skip(prefix) {
        new_counts
            .entry(h)
            .and_modify(|e| e.0 += 1)
            .or_insert((1, j));
    }

    let mut matches: Vec<(usize, usize)> = Vec::new();
    for (&h, &(oc, oi)) in &old_counts {
        if oc == 1 {
            if let Some(&(nc, ni)) = new_counts.get(&h) {
                if nc == 1 && old[oi] == new[ni] {
                    matches.push((oi, ni));
                }
            }
        }
    }
    matches.sort_unstable_by_key(|&(oi, _)| oi);

    let anchors = patience_lis(&matches);
    let mut gaps: Vec<(usize, usize, usize, usize)> = Vec::new();
    let (mut cur_o, mut cur_n) = (prefix, prefix);
    for (ao, an) in anchors {
        // Trim any matching lines adjacent to the anchor so hunks stay tight.
        let mut o_hi = ao;
        let mut n_hi = an;
        while o_hi > cur_o
            && n_hi > cur_n
            && old_h[o_hi - 1] == new_h[n_hi - 1]
            && old[o_hi - 1] == new[n_hi - 1]
        {
            o_hi -= 1;
            n_hi -= 1;
        }
        if cur_o < o_hi || cur_n < n_hi {
            gaps.push((cur_o, o_hi, cur_n, n_hi));
        }
        cur_o = ao + 1;
        cur_n = an + 1;
        while cur_o < old_end
            && cur_n < new_end
            && old_h[cur_o] == new_h[cur_n]
            && old[cur_o] == new[cur_n]
        {
            cur_o += 1;
            cur_n += 1;
        }
    }
    if cur_o < old_end || cur_n < new_end {
        gaps.push((cur_o, old_end, cur_n, new_end));
    }
    if gaps.is_empty() {
        return None;
    }

    // Merge gaps whose context windows touch or sit within 4 lines of each other.
    const CTX: usize = 2;
    let mut hunks: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (os, oe, ns, ne) in gaps {
        let co_s = os.saturating_sub(CTX);
        let co_e = (oe + CTX).min(old.len());
        let cn_s = ns.saturating_sub(CTX);
        let cn_e = (ne + CTX).min(new.len());
        if let Some(last) = hunks.last_mut() {
            if co_s <= last.1 + 2 && cn_s <= last.3 + 2 {
                last.1 = last.1.max(co_e);
                last.3 = last.3.max(cn_e);
                continue;
            }
        }
        hunks.push((co_s, co_e, cn_s, cn_e));
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "[FluxCode context delta] {rel} updated since earlier in this session ({} -> {} lines):",
        old.len(),
        new.len()
    );
    for (ho_s, ho_e, hn_s, hn_e) in hunks {
        let _ = writeln!(
            out,
            "@@ -{},{} +{},{} @@",
            ho_s + 1,
            ho_e - ho_s,
            hn_s + 1,
            hn_e - hn_s
        );
        let mut oi = ho_s;
        let mut ni = hn_s;
        while oi < ho_e || ni < hn_e {
            if oi < ho_e && ni < hn_e && old_h[oi] == new_h[ni] && old[oi] == new[ni] {
                let _ = writeln!(out, " {}", new[ni]);
                oi += 1;
                ni += 1;
            } else {
                // Emit deletions up to the next matching line or hunk end.
                let mut next_match = None;
                for o_scan in oi..ho_e {
                    if let Some(n_scan) =
                        (ni..hn_e).find(|&nj| old_h[o_scan] == new_h[nj] && old[o_scan] == new[nj])
                    {
                        next_match = Some((o_scan, n_scan));
                        break;
                    }
                }
                let (stop_o, stop_n) = next_match.unwrap_or((ho_e, hn_e));
                while oi < stop_o {
                    let _ = writeln!(out, "-{}", old[oi]);
                    oi += 1;
                }
                while ni < stop_n {
                    let _ = writeln!(out, "+{}", new[ni]);
                    ni += 1;
                }
            }
        }
    }
    Some(out)
}

fn patience_lis(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let mut tails: Vec<usize> = Vec::new();
    let mut tail_idx: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; pairs.len()];

    for (i, &(_, b)) in pairs.iter().enumerate() {
        let pos = tails.partition_point(|&v| v < b);
        if pos == tails.len() {
            tails.push(b);
            tail_idx.push(i);
        } else {
            tails[pos] = b;
            tail_idx[pos] = i;
        }
        if pos > 0 {
            prev[i] = Some(tail_idx[pos - 1]);
        }
    }

    let mut seq = Vec::with_capacity(tails.len());
    let mut cur = tail_idx.last().copied();
    while let Some(i) = cur {
        seq.push(pairs[i]);
        cur = prev[i];
    }
    seq.reverse();
    seq
}

/// Amortized O(1) head-offset line framer avoiding front-drain memmove per chunk.
#[derive(Default)]
pub struct StreamFramer {
    buf: String,
    head: usize,
}

impl StreamFramer {
    pub fn clear(&mut self) {
        self.buf.clear();
        self.head = 0;
        if self.buf.capacity() > 65_536 {
            self.buf.shrink_to(4096);
        }
    }

    pub fn push_and_drain(&mut self, chunk: &str, mut on_line: impl FnMut(&str)) -> usize {
        if !chunk.is_empty() {
            self.buf.push_str(chunk);
        }
        let mut count = 0usize;
        while let Some(rel_nl) = self.buf[self.head..].find('\n') {
            let line_end = self.head + rel_nl;
            let trimmed = self.buf[self.head..line_end].trim();
            if !trimmed.is_empty() {
                on_line(trimmed);
                count += 1;
            }
            self.head = line_end + 1;
        }
        if self.head == self.buf.len() {
            self.clear();
        } else if self.head >= 4096 && self.head * 2 >= self.buf.len() {
            self.buf.drain(..self.head);
            self.head = 0;
        }
        count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyAction {
    None,
    Immediate,
    Defer(Duration),
}

/// Frame-paced Nyquist-Shannon UI repaint coalescer for streaming ACP token bursts.
#[derive(Default)]
pub struct StreamCoalescer {
    last_notify: Option<Instant>,
    last_chunk: Option<Instant>,
    ema_rate_hz: f64,
}

impl StreamCoalescer {
    pub fn reset(&mut self) {
        self.last_notify = None;
        self.last_chunk = None;
        self.ema_rate_hz = 0.0;
    }

    pub fn on_batch(&mut self, parsed_lines: usize, urgent: bool, now: Instant) -> NotifyAction {
        if parsed_lines == 0 && !urgent {
            return NotifyAction::None;
        }
        if let Some(prev) = self.last_chunk {
            let dt = now.saturating_duration_since(prev).as_secs_f64().max(0.002);
            let inst_hz = (parsed_lines.max(1) as f64 / dt).min(500.0);
            self.ema_rate_hz = if self.ema_rate_hz <= 0.0 {
                inst_hz
            } else {
                0.35 * inst_hz + 0.65 * self.ema_rate_hz
            };
        }
        self.last_chunk = Some(now);

        if urgent || self.ema_rate_hz < 30.0 {
            self.last_notify = Some(now);
            return NotifyAction::Immediate;
        }
        let cadence = if self.ema_rate_hz > 120.0 {
            Duration::from_millis(20)
        } else {
            Duration::from_millis(16)
        };
        match self.last_notify {
            None => {
                self.last_notify = Some(now);
                NotifyAction::Immediate
            }
            Some(last) => {
                let elapsed = now.saturating_duration_since(last);
                if elapsed >= cadence {
                    self.last_notify = Some(now);
                    NotifyAction::Immediate
                } else {
                    NotifyAction::Defer(cadence - elapsed)
                }
            }
        }
    }

    pub fn mark_flushed(&mut self, now: Instant) {
        self.last_notify = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn newtons_cooling_kalman_and_lyapunov_behave_monotonically() {
        let mut tc = ThermalController::for_profile(RuntimeProfile::Antigravity, 16_384.0);
        let t0 = Instant::now();
        tc.on_turn_start(t0);
        assert!((tc.temperature(0.0) - 1.0).abs() < 1e-6);
        assert!(tc.temperature(5.0) < tc.temperature(1.0));
        assert!(tc.temperature(60.0) < tc.temperature(10.0));
        assert!(tc.stage1_delay() < tc.stage2_delay());
        assert!(tc.stage2_delay() >= Duration::from_secs(6));

        let fast_half = {
            let mut f = ThermalController::default();
            f.on_turn_start(t0);
            f.on_turn_start(t0 + Duration::from_secs(4));
            f.on_turn_start(t0 + Duration::from_secs(8));
            f.half_life_secs()
        };
        assert!(fast_half < tc.half_life_secs());

        // Kalman + Lyapunov skips trim when lean, trims when bloated and positive definite P stays valid.
        assert!(!tc.should_trim(32.0, t0 + Duration::from_secs(2), false));
        assert!(tc.p00 > 0.0 && tc.p11 > 0.0);
        assert!(tc.should_trim(280.0, t0 + Duration::from_secs(7), false));
        assert!(tc.last_filtered_ws_mb > 40.0);
    }

    #[test]
    fn usl_and_denning_calibrate_by_hardware() {
        assert_eq!(usl_optimal_workers(2, 0.08, 0.045), 2);
        assert_eq!(usl_optimal_workers(16, 0.08, 0.045), 4);
        assert!(
            denning_equilibrium_mb(RuntimeProfile::NodeV8, 8192.0)
                > denning_equilibrium_mb(RuntimeProfile::OpenCode, 8192.0)
        );
        assert!(
            denning_equilibrium_mb(RuntimeProfile::Antigravity, 32768.0)
                > denning_equilibrium_mb(RuntimeProfile::Antigravity, 4096.0)
        );
    }

    #[test]
    fn boltzmann_kkt_conserves_budget_and_prioritizes_low_energy() {
        let items = vec![(0.1, 1000, 2000), (0.2, 400, 500), (1.5, 500, 20_000)];
        let alloc = boltzmann_allocate(&items, 6000, 1.6);
        assert_eq!(alloc.iter().sum::<usize>(), 6000);
        assert!(alloc[0] > alloc[1]);
        assert!(alloc[0] >= 1800 && alloc[1] >= 400);

        let tight = boltzmann_allocate(&[(0.1, 1000, 1000), (2.0, 1000, 1000)], 1200, 1.6);
        assert_eq!(tight.iter().sum::<usize>(), 1200);
        assert!(tight[0] > tight[1]);
    }

    #[test]
    fn context_cache_supports_multi_hunk_patience_diff_and_entropy_gate() {
        let mut cache = ContextCache::default();
        let p = Path::new("src/main.rs");
        let lines: Vec<String> = (1..=160)
            .map(|i| format!("let line_{i} = compute_step({i}, \"alpha_{i}\");"))
            .collect();
        let v1 = lines.join("\n");

        assert_eq!(
            cache.encode_file(p, "src/main.rs", &v1, Some(10)),
            FilePayload::Full
        );
        match cache.encode_file(p, "src/main.rs", &v1, Some(42)) {
            FilePayload::Unchanged(msg) => {
                assert!(msg.contains("unchanged") && msg.contains("line 42"))
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }
        assert_eq!(cache.hits_unchanged, 1);

        let mut lines2 = lines.clone();
        lines2[15] = "let line_16 = 777;".into();
        lines2[130] = "let line_131 = 888;".into();
        let v2 = lines2.join("\n");
        match cache.encode_file(p, "src/main.rs", &v2, Some(16)) {
            FilePayload::Delta(delta) => {
                assert!(delta.contains("-let line_16"));
                assert!(delta.contains("+let line_16 = 777;"));
                assert!(delta.contains("-let line_131"));
                assert!(delta.contains("+let line_131 = 888;"));
                assert!(
                    delta.matches("@@ ").count() == 2,
                    "two distant edits must emit two compact hunks:\n{delta}"
                );
                assert!(delta.len() * 4 < v2.len());
            }
            other => panic!("expected Delta, got {other:?}"),
        }
        assert_eq!(cache.hits_delta, 1);
        assert!(cache.bytes_saved > 0);
    }

    #[test]
    fn stream_framer_and_coalescer_handle_partial_lines_and_bursts() {
        let mut framer = StreamFramer::default();
        let mut seen = Vec::new();
        assert_eq!(
            framer.push_and_drain("{\"a\":1}\n\n{\"b\":", |l| seen.push(l.to_string())),
            1
        );
        assert_eq!(
            framer.push_and_drain("2}\n", |l| seen.push(l.to_string())),
            1
        );
        assert_eq!(seen, vec!["{\"a\":1}", "{\"b\":2}"]);

        let mut coal = StreamCoalescer::default();
        let t0 = Instant::now();
        assert_eq!(coal.on_batch(1, false, t0), NotifyAction::Immediate);
        // Sustained 100 Hz burst (30..=120 Hz) uses 16ms frame cadence -> Defer(6ms).
        assert_eq!(
            coal.on_batch(1, false, t0 + Duration::from_millis(10)),
            NotifyAction::Defer(Duration::from_millis(6))
        );
        // Heavy > 120 Hz burst stretches cadence to 20ms -> Defer(8ms).
        assert_eq!(
            coal.on_batch(4, false, t0 + Duration::from_millis(12)),
            NotifyAction::Defer(Duration::from_millis(8))
        );
        // Urgent state transitions (tool call, permission, RPC result) always notify immediately.
        assert_eq!(
            coal.on_batch(1, true, t0 + Duration::from_millis(14)),
            NotifyAction::Immediate
        );
    }
}
