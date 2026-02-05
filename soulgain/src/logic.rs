use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::plasticity::{Event, Plasticity};
use crate::types::{SkillLibrary, UVal};
use crate::vm::{Op, SKILL_OPCODE_BASE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    Number,
    Bool,
    String,
    Any,
}

impl From<&UVal> for ValueType {
    fn from(value: &UVal) -> Self {
        match value {
            UVal::Number(_) => ValueType::Number,
            UVal::Bool(_) => ValueType::Bool,
            UVal::String(_) => ValueType::String,
            UVal::Object(_) | UVal::Nil => ValueType::Any,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Signature {
    inputs: Vec<ValueType>,
    outputs: Vec<ValueType>,
}

impl Signature {
    fn new(inputs: Vec<ValueType>, outputs: Vec<ValueType>) -> Self {
        Self { inputs, outputs }
    }
}

#[derive(Clone, Debug)]
struct SearchNode {
    score: f64,
    cost: usize,
    stack: Vec<ValueType>,
    program: Vec<i64>,
    last_op: i64,
}

impl Eq for SearchNode {}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct WeightedInferenceEngine<'a> {
    plasticity: &'a Plasticity,
    signatures: HashMap<i64, Signature>,
    candidate_ops: Vec<i64>,
}

impl<'a> WeightedInferenceEngine<'a> {
    pub fn new(plasticity: &'a Plasticity, skills: &'a SkillLibrary) -> Self {
        let mut signatures = base_signatures();
        let skill_signatures = infer_skill_signatures(skills, &signatures);
        signatures.extend(skill_signatures);

        let mut candidate_ops = signatures.keys().cloned().collect::<Vec<_>>();
        candidate_ops.extend([
            Op::Swap.as_i64(),
            Op::Dup.as_i64(),
            Op::Over.as_i64(),
            Op::Drop.as_i64(),
        ]);
        candidate_ops.sort();
        candidate_ops.dedup();

        Self {
            plasticity,
            signatures,
            candidate_ops,
        }
    }

    pub fn deduce(
        &self,
        input: &[UVal],
        expected: &[UVal],
        max_steps: usize,
        max_depth: usize,
    ) -> Option<Vec<f64>> {
        let start_stack = input.iter().map(ValueType::from).collect::<Vec<_>>();
        let goal_stack = expected.iter().map(ValueType::from).collect::<Vec<_>>();

        let start_node = SearchNode {
            score: 0.0,
            cost: 0,
            stack: start_stack,
            program: Vec::new(),
            last_op: Op::Literal.as_i64(),
        };

        let mut frontier = BinaryHeap::new();
        frontier.push(start_node);
        let mut visited: HashSet<(Vec<ValueType>, i64)> = HashSet::new();

        let mut steps = 0usize;
        while let Some(node) = frontier.pop() {
            if node.stack == goal_stack {
                return Some(node.program.iter().map(|op| *op as f64).collect());
            }

            if node.cost >= max_depth {
                continue;
            }

            if steps >= max_steps {
                break;
            }
            steps += 1;

            let state_key = (node.stack.clone(), node.last_op);
            if visited.contains(&state_key) {
                continue;
            }
            visited.insert(state_key);

            for &op in &self.candidate_ops {
                if let Some(next_stack) = self.apply_op(op, &node.stack) {
                    let weight = self.weight_for(node.last_op, op, next_stack.len());
                    let heuristic = self.stack_distance(&next_stack, &goal_stack);
                    let score = weight - (node.cost + 1) as f64 - heuristic * 0.5;

                    let mut program = node.program.clone();
                    program.push(op);

                    frontier.push(SearchNode {
                        score,
                        cost: node.cost + 1,
                        stack: next_stack,
                        program,
                        last_op: op,
                    });
                }
            }
        }
        None
    }

    fn apply_op(&self, op: i64, stack: &[ValueType]) -> Option<Vec<ValueType>> {
        if let Some(sig) = self.signatures.get(&op) {
            let mut next_stack = stack.to_vec();
            for expected in sig.inputs.iter().rev() {
                let actual = next_stack.pop()?;
                if !type_compatible(expected, &actual) {
                    return None;
                }
            }
            next_stack.extend(sig.outputs.iter().cloned());
            return Some(next_stack);
        }

        match Op::from_i64(op) {
            Some(Op::Swap) => {
                if stack.len() < 2 {
                    return None;
                }
                let mut next_stack = stack.to_vec();
                let len = next_stack.len();
                next_stack.swap(len - 1, len - 2);
                Some(next_stack)
            }
            Some(Op::Dup) => {
                let mut next_stack = stack.to_vec();
                let top = next_stack.last().cloned()?;
                next_stack.push(top);
                Some(next_stack)
            }
            Some(Op::Over) => {
                if stack.len() < 2 {
                    return None;
                }
                let mut next_stack = stack.to_vec();
                let len = next_stack.len();
                let val = next_stack[len - 2];
                next_stack.push(val);
                Some(next_stack)
            }
            Some(Op::Drop) => {
                if stack.is_empty() {
                    return None;
                }
                let mut next_stack = stack.to_vec();
                next_stack.pop();
                Some(next_stack)
            }
            _ => None,
        }
    }

    fn stack_distance(&self, stack: &[ValueType], goal: &[ValueType]) -> f64 {
        let len_diff = if stack.len() > goal.len() {
            (stack.len() - goal.len()) as f64
        } else {
            (goal.len() - stack.len()) as f64
        };
        let mismatches = stack
            .iter()
            .zip(goal.iter())
            .filter(|(a, b)| a != b)
            .count() as f64;
        len_diff + mismatches
    }

    fn weight_for(&self, last_op: i64, next_op: i64, stack_depth: usize) -> f64 {
        let norm_depth = normalize_depth(stack_depth);
        let from = Event::Opcode {
            opcode: last_op,
            stack_depth: norm_depth,
        };
        let to = Event::Opcode {
            opcode: next_op,
            stack_depth: norm_depth,
        };
        if let Ok(mem) = self.plasticity.memory.read() {
            if let Some(targets) = mem.weights.get(&from) {
                if let Some(weight) = targets.get(&to) {
                    return *weight;
                }
            }
        }
        0.0
    }
}

fn type_compatible(expected: &ValueType, actual: &ValueType) -> bool {
    matches!(expected, ValueType::Any) || expected == actual
}

fn normalize_depth(depth: usize) -> usize {
    std::cmp::min(depth, 5)
}

fn base_signatures() -> HashMap<i64, Signature> {
    let mut signatures = HashMap::new();
    signatures.insert(
        Op::Add.as_i64(),
        Signature::new(vec![ValueType::Number, ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Sub.as_i64(),
        Signature::new(vec![ValueType::Number, ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Mul.as_i64(),
        Signature::new(vec![ValueType::Number, ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Mod.as_i64(),
        Signature::new(vec![ValueType::Number, ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Inc.as_i64(),
        Signature::new(vec![ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Dec.as_i64(),
        Signature::new(vec![ValueType::Number], vec![ValueType::Number]),
    );
    signatures.insert(
        Op::Eq.as_i64(),
        Signature::new(vec![ValueType::Any, ValueType::Any], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::Gt.as_i64(),
        Signature::new(vec![ValueType::Number, ValueType::Number], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::Not.as_i64(),
        Signature::new(vec![ValueType::Any], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::And.as_i64(),
        Signature::new(vec![ValueType::Any, ValueType::Any], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::Or.as_i64(),
        Signature::new(vec![ValueType::Any, ValueType::Any], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::Xor.as_i64(),
        Signature::new(vec![ValueType::Any, ValueType::Any], vec![ValueType::Bool]),
    );
    signatures.insert(
        Op::IsZero.as_i64(),
        Signature::new(vec![ValueType::Any], vec![ValueType::Bool]),
    );
    signatures
}

fn infer_skill_signatures(
    skills: &SkillLibrary,
    known: &HashMap<i64, Signature>,
) -> HashMap<i64, Signature> {
    let mut signatures = HashMap::new();
    let mut changed = true;

    while changed {
        changed = false;
        for (id, program) in &skills.macros {
            if signatures.contains_key(id) {
                continue;
            }
            if let Some(signature) = infer_macro_signature(program, known, &signatures) {
                signatures.insert(*id, signature);
                changed = true;
            }
        }
    }

    signatures
}

fn infer_macro_signature(
    program: &[f64],
    known: &HashMap<i64, Signature>,
    inferred: &HashMap<i64, Signature>,
) -> Option<Signature> {
    let mut inputs: Vec<ValueType> = Vec::new();
    let mut stack: Vec<ValueType> = Vec::new();
    let mut idx = 0usize;

    while idx < program.len() {
        let raw = program[idx];
        idx += 1;

        if !raw.is_finite() {
            return None;
        }
        let opcode = raw.round() as i64;

        if opcode == Op::Halt.as_i64() {
            break;
        }

        if opcode == Op::Literal.as_i64() {
            if idx >= program.len() {
                return None;
            }
            idx += 1;
            stack.push(ValueType::Number);
            continue;
        }

        if opcode >= SKILL_OPCODE_BASE {
            if let Some(sig) = inferred.get(&opcode).or_else(|| known.get(&opcode)) {
                apply_signature(sig, &mut stack, &mut inputs)?;
            } else {
                return None;
            }
            continue;
        }

        match Op::from_i64(opcode) {
            Some(Op::Swap) => apply_stack_swap(&mut stack, &mut inputs)?,
            Some(Op::Dup) => apply_stack_dup(&mut stack, &mut inputs)?,
            Some(Op::Over) => apply_stack_over(&mut stack, &mut inputs)?,
            Some(Op::Drop) => apply_stack_drop(&mut stack, &mut inputs)?,
            Some(_) => {
                if let Some(sig) = known.get(&opcode) {
                    apply_signature(sig, &mut stack, &mut inputs)?;
                } else {
                    return None;
                }
            }
            None => return None,
        }
    }

    Some(Signature::new(inputs, stack))
}

fn apply_signature(
    signature: &Signature,
    stack: &mut Vec<ValueType>,
    inputs: &mut Vec<ValueType>,
) -> Option<()> {
    for expected in signature.inputs.iter().rev() {
        if let Some(actual) = stack.pop() {
            if !type_compatible(expected, &actual) {
                return None;
            }
        } else {
            inputs.push(*expected);
        }
    }
    stack.extend(signature.outputs.iter().cloned());
    Some(())
}

fn apply_stack_swap(stack: &mut Vec<ValueType>, inputs: &mut Vec<ValueType>) -> Option<()> {
    while stack.len() < 2 {
        inputs.push(ValueType::Any);
        stack.push(ValueType::Any);
    }
    let len = stack.len();
    stack.swap(len - 1, len - 2);
    Some(())
}

fn apply_stack_dup(stack: &mut Vec<ValueType>, inputs: &mut Vec<ValueType>) -> Option<()> {
    if stack.is_empty() {
        inputs.push(ValueType::Any);
        stack.push(ValueType::Any);
    }
    let top = *stack.last()?;
    stack.push(top);
    Some(())
}

fn apply_stack_over(stack: &mut Vec<ValueType>, inputs: &mut Vec<ValueType>) -> Option<()> {
    while stack.len() < 2 {
        inputs.push(ValueType::Any);
        stack.push(ValueType::Any);
    }
    let len = stack.len();
    let val = stack[len - 2];
    stack.push(val);
    Some(())
}

fn apply_stack_drop(stack: &mut Vec<ValueType>, inputs: &mut Vec<ValueType>) -> Option<()> {
    if stack.is_empty() {
        inputs.push(ValueType::Any);
        stack.push(ValueType::Any);
    }
    stack.pop();
    Some(())
}
