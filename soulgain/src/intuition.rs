use std::collections::{HashMap, VecDeque};

use crate::types::UVal;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Nil,
    Bool,
    Number,
    String,
    Object,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NumberBand {
    Neg,
    Zero,
    Small,
    Medium,
    Large,
}

#[derive(Clone, Debug)]
pub struct ContextSnapshot {
    pub depth_bucket: u8,
    pub top_types: [Option<ValueKind>; 3],
    pub top_number_bands: [Option<NumberBand>; 3],
    pub recent_opcodes: Vec<i64>,
}

#[derive(Clone, Debug)]
pub struct SkillPattern {
    pub min_depth: u8,
    pub max_depth: u8,
    pub required_top_types: [Option<ValueKind>; 3],
}

#[derive(Clone, Debug)]
pub struct SkillStats {
    pub attempts: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_reward_delta: f64,
    pub base_confidence: f64,
    pub last_used_tick: u64,
    pub times_used_recent_window: u32,
}

impl Default for SkillStats {
    fn default() -> Self {
        Self {
            attempts: 0,
            successes: 0,
            failures: 0,
            avg_reward_delta: 0.0,
            base_confidence: 0.5,
            last_used_tick: 0,
            times_used_recent_window: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkillMetadata {
    pub skill_id: i64,
    pub pattern: SkillPattern,
    pub stats: SkillStats,
}

#[derive(Clone, Debug)]
pub struct SkillOutcome {
    pub success: bool,
    pub reward_delta: f64,
    pub stack_match_after: bool,
    pub used_tick: u64,
}

#[derive(Clone, Debug)]
pub struct IntuitionWeights {
    pub w_match: f64,
    pub w_success: f64,
    pub w_reward: f64,
    pub w_conf: f64,
    pub w_decay: f64,
    pub w_explore: f64,
}

impl Default for IntuitionWeights {
    fn default() -> Self {
        Self {
            w_match: 0.45,
            w_success: 0.2,
            w_reward: 0.15,
            w_conf: 0.1,
            w_decay: 0.07,
            w_explore: 0.03,
        }
    }
}

pub struct IntuitionEngine {
    pub skill_meta: HashMap<i64, SkillMetadata>,
    pub weights: IntuitionWeights,
    pub gate_threshold: f64,
    pub deterministic_mode: bool,
    rng_state: u64,
}

impl Default for IntuitionEngine {
    fn default() -> Self {
        Self {
            skill_meta: HashMap::new(),
            weights: IntuitionWeights::default(),
            gate_threshold: 0.35,
            deterministic_mode: false,
            rng_state: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

impl IntuitionEngine {
    pub fn build_context(&self, stack: &[UVal], recent: &VecDeque<i64>) -> ContextSnapshot {
        let mut top_types: [Option<ValueKind>; 3] = [None, None, None];
        let mut top_number_bands: [Option<NumberBand>; 3] = [None, None, None];

        for (idx, value) in stack.iter().rev().take(3).enumerate() {
            top_types[idx] = Some(value_kind(value));
            top_number_bands[idx] = number_band(value);
        }

        ContextSnapshot {
            depth_bucket: std::cmp::min(stack.len(), 5) as u8,
            top_types,
            top_number_bands,
            recent_opcodes: recent.iter().copied().collect(),
        }
    }

    pub fn ensure_skill_known(&mut self, skill_id: i64) {
        self.skill_meta
            .entry(skill_id)
            .or_insert_with(|| SkillMetadata {
                skill_id,
                pattern: SkillPattern {
                    min_depth: 0,
                    max_depth: 5,
                    required_top_types: [None, None, None],
                },
                stats: SkillStats::default(),
            });
    }

    pub fn bootstrap_pattern_if_empty(&mut self, skill_id: i64, ctx: &ContextSnapshot) {
        self.ensure_skill_known(skill_id);
        if let Some(meta) = self.skill_meta.get_mut(&skill_id) {
            if meta.pattern.required_top_types == [None, None, None] {
                meta.pattern.required_top_types = ctx.top_types.clone();
                meta.pattern.min_depth = ctx.depth_bucket;
                meta.pattern.max_depth = 5;
            }
        }
    }

    pub fn select_skill(
        &mut self,
        ctx: &ContextSnapshot,
        candidates: &[i64],
        tick: u64,
    ) -> Option<i64> {
        let mut scored: Vec<(i64, f64)> = Vec::new();

        for skill_id in candidates {
            self.ensure_skill_known(*skill_id);
            let Some(meta) = self.skill_meta.get(skill_id) else {
                continue;
            };
            let pattern = self.pattern_match(ctx, &meta.pattern);
            if pattern < self.gate_threshold {
                continue;
            }
            let score = self.applicability_score(ctx, meta, tick, pattern);
            if score > 0.0 {
                scored.push((*skill_id, score));
            }
        }

        if scored.is_empty() {
            return None;
        }

        if self.deterministic_mode {
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            return scored.first().map(|(id, _)| *id);
        }

        self.weighted_pick(&scored)
    }

    pub fn update_after_execution(&mut self, skill_id: i64, outcome: SkillOutcome) {
        self.ensure_skill_known(skill_id);
        let Some(meta) = self.skill_meta.get_mut(&skill_id) else {
            return;
        };

        meta.stats.attempts += 1;
        if outcome.success {
            meta.stats.successes += 1;
            meta.stats.base_confidence = (meta.stats.base_confidence + 0.03).clamp(0.05, 0.95);
        } else {
            meta.stats.failures += 1;
            meta.stats.base_confidence = (meta.stats.base_confidence - 0.04).clamp(0.05, 0.95);
        }

        let alpha = 0.25;
        meta.stats.avg_reward_delta =
            (1.0 - alpha) * meta.stats.avg_reward_delta + alpha * outcome.reward_delta;

        if !outcome.stack_match_after {
            meta.stats.base_confidence = (meta.stats.base_confidence - 0.02).clamp(0.05, 0.95);
        }

        meta.stats.last_used_tick = outcome.used_tick;
        meta.stats.times_used_recent_window = meta.stats.times_used_recent_window.saturating_add(1);
    }

    fn pattern_match(&self, ctx: &ContextSnapshot, pattern: &SkillPattern) -> f64 {
        let mut score: f64 = 0.0;
        if ctx.depth_bucket >= pattern.min_depth && ctx.depth_bucket <= pattern.max_depth {
            score += 0.4;
        }

        let mut type_matches = 0.0;
        let mut total = 0.0;
        for i in 0..3 {
            if let Some(required) = &pattern.required_top_types[i] {
                total += 1.0;
                if ctx.top_types[i].as_ref() == Some(required) {
                    type_matches += 1.0;
                }
            }
        }
        if total == 0.0 {
            score += 0.6;
        } else {
            score += 0.6 * (type_matches / total);
        }

        score.clamp(0.0, 1.0)
    }

    fn applicability_score(
        &self,
        _ctx: &ContextSnapshot,
        meta: &SkillMetadata,
        tick: u64,
        pattern_match: f64,
    ) -> f64 {
        let success_rate =
            meta.stats.successes as f64 / std::cmp::max(1, meta.stats.attempts) as f64;
        let normalized_reward = (meta.stats.avg_reward_delta / 100.0).clamp(-1.0, 1.0);
        let recency_penalty = if tick.saturating_sub(meta.stats.last_used_tick) <= 8 {
            1.0
        } else {
            0.0
        };
        let exploration_bonus = 1.0 / (1.0 + meta.stats.attempts as f64);

        self.weights.w_match * pattern_match
            + self.weights.w_success * success_rate
            + self.weights.w_reward * normalized_reward
            + self.weights.w_conf * meta.stats.base_confidence
            - self.weights.w_decay * recency_penalty
            + self.weights.w_explore * exploration_bonus
    }

    fn weighted_pick(&mut self, scored: &[(i64, f64)]) -> Option<i64> {
        let total: f64 = scored.iter().map(|(_, s)| *s).sum();
        if total <= 0.0 {
            return scored
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| *id);
        }

        let mut r = self.next_unit() * total;
        for (id, score) in scored {
            if r <= *score {
                return Some(*id);
            }
            r -= *score;
        }
        scored.last().map(|(id, _)| *id)
    }

    fn next_unit(&mut self) -> f64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = self.rng_state >> 11;
        (v as f64) / ((u64::MAX >> 11) as f64)
    }
}

fn value_kind(v: &UVal) -> ValueKind {
    match v {
        UVal::Nil => ValueKind::Nil,
        UVal::Bool(_) => ValueKind::Bool,
        UVal::Number(_) => ValueKind::Number,
        UVal::String(_) => ValueKind::String,
        UVal::Object(_) => ValueKind::Object,
    }
}

fn number_band(v: &UVal) -> Option<NumberBand> {
    let UVal::Number(n) = v else { return None };
    Some(if *n < 0.0 {
        NumberBand::Neg
    } else if *n == 0.0 {
        NumberBand::Zero
    } else if *n < 10.0 {
        NumberBand::Small
    } else if *n < 1000.0 {
        NumberBand::Medium
    } else {
        NumberBand::Large
    })
}
