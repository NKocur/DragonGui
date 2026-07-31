//! Abstract command-queue models used to prove replacement ordering before the
//! production queue data structure changes.

use std::collections::{HashMap, VecDeque};
use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ModelCommand {
    SetProp {
        widget: u32,
        prop: u32,
        value: u32,
    },
    Theme {
        value: u32,
    },
    SetSheet {
        origin: u8,
        id: Option<u8>,
        value: u32,
    },
    RemoveSheet {
        origin: u8,
        id: u8,
    },
    ClearSheets {
        origin: u8,
    },
    Scatter {
        widget: u8,
        value: u32,
        fit: bool,
    },
    Line {
        widget: u8,
        series: u8,
        value: u32,
        fit: bool,
    },
    Histogram {
        widget: u8,
        value: u32,
        auto_fit: bool,
    },
    ScalarBar {
        widget: u8,
        value: u32,
    },
    Actor {
        widget: u8,
        actor: u8,
        value: u32,
    },
    Lossless {
        value: u32,
    },
    Snapshot {
        request: u32,
    },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum CoalesceKey {
    SetProp(u32, u32),
    Theme,
    Sheet(u8, Option<u8>),
    ClearSheets(u8),
    Scatter(u8),
    Line(u8, u8),
    Histogram(u8),
    ScalarBar(u8),
    Actor(u8, u8),
}

impl ModelCommand {
    fn key(&self) -> Option<CoalesceKey> {
        match self {
            Self::SetProp { widget, prop, .. } => Some(CoalesceKey::SetProp(*widget, *prop)),
            Self::Theme { .. } => Some(CoalesceKey::Theme),
            Self::SetSheet { origin, id, .. } => Some(CoalesceKey::Sheet(*origin, *id)),
            Self::RemoveSheet { origin, id } => Some(CoalesceKey::Sheet(*origin, Some(*id))),
            Self::ClearSheets { origin } => Some(CoalesceKey::ClearSheets(*origin)),
            Self::Scatter { widget, .. } => Some(CoalesceKey::Scatter(*widget)),
            Self::Line { widget, series, .. } => Some(CoalesceKey::Line(*widget, *series)),
            Self::Histogram { widget, .. } => Some(CoalesceKey::Histogram(*widget)),
            Self::ScalarBar { widget, .. } => Some(CoalesceKey::ScalarBar(*widget)),
            Self::Actor { widget, actor, .. } => Some(CoalesceKey::Actor(*widget, *actor)),
            Self::Lossless { .. } | Self::Snapshot { .. } => None,
        }
    }

    fn merge_sticky_flags_from(&mut self, previous: &Self) {
        match (self, previous) {
            (Self::Scatter { fit, .. }, Self::Scatter { fit: old, .. })
            | (Self::Line { fit, .. }, Self::Line { fit: old, .. }) => *fit |= *old,
            (Self::Histogram { auto_fit, .. }, Self::Histogram { auto_fit: old, .. }) => {
                *auto_fit |= *old
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct ReferenceQueue {
    items: VecDeque<ModelCommand>,
}

impl ReferenceQueue {
    fn push(&mut self, mut command: ModelCommand) {
        match &command {
            ModelCommand::ClearSheets { origin } => {
                self.items.retain(|queued| {
                    !matches!(
                        queued,
                        ModelCommand::SetSheet { origin: queued_origin, .. }
                            | ModelCommand::RemoveSheet { origin: queued_origin, .. }
                            | ModelCommand::ClearSheets { origin: queued_origin }
                            if queued_origin == origin
                    )
                });
            }
            _ => {
                if let Some(key) = command.key() {
                    let mut removed = Vec::new();
                    self.items.retain(|queued| {
                        if queued.key().as_ref() == Some(&key) {
                            removed.push(queued.clone());
                            false
                        } else {
                            true
                        }
                    });
                    for previous in &removed {
                        command.merge_sticky_flags_from(previous);
                    }
                }
            }
        }
        self.items.push_back(command);
    }

    fn drain_limited(&mut self, limit: usize) -> Vec<ModelCommand> {
        (0..limit).filter_map(|_| self.items.pop_front()).collect()
    }

    fn drain(&mut self) -> Vec<ModelCommand> {
        self.items.drain(..).collect()
    }
}

/// First candidate: append stable ordering tokens, invalidate replaced slots,
/// and compact deterministically. Superseded payloads are dropped immediately.
#[derive(Default)]
struct StableSlotQueue {
    slots: Vec<Option<ModelCommand>>,
    order: VecDeque<usize>,
    latest: HashMap<CoalesceKey, usize>,
    live: usize,
    stale: usize,
    compactions: usize,
    peak_physical: usize,
}

impl StableSlotQueue {
    const COMPACT_MIN_PHYSICAL: usize = 64;

    fn invalidate(&mut self, index: usize) -> Option<ModelCommand> {
        let previous = self.slots.get_mut(index)?.take();
        if previous.is_some() {
            self.live -= 1;
            self.stale += 1;
        }
        previous
    }

    fn push(&mut self, mut command: ModelCommand) {
        if let ModelCommand::ClearSheets { origin } = command {
            let keys = self
                .latest
                .keys()
                .filter(|key| {
                    matches!(
                        key,
                        CoalesceKey::Sheet(queued_origin, _)
                            | CoalesceKey::ClearSheets(queued_origin)
                            if *queued_origin == origin
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                if let Some(index) = self.latest.remove(&key) {
                    self.invalidate(index);
                }
            }
            command = ModelCommand::ClearSheets { origin };
        } else if let Some(key) = command.key() {
            if let Some(index) = self.latest.remove(&key) {
                if let Some(previous) = self.invalidate(index) {
                    command.merge_sticky_flags_from(&previous);
                }
            }
        }

        let key = command.key();
        let index = self.slots.len();
        self.slots.push(Some(command));
        self.order.push_back(index);
        if let Some(key) = key {
            self.latest.insert(key, index);
        }
        self.live += 1;
        self.peak_physical = self.peak_physical.max(self.order.len());
        self.compact_if_needed();
    }

    fn drain_limited(&mut self, limit: usize) -> Vec<ModelCommand> {
        let mut drained = Vec::with_capacity(limit.min(self.live));
        while drained.len() < limit {
            let Some(index) = self.order.pop_front() else {
                break;
            };
            let Some(command) = self.slots[index].take() else {
                self.stale -= 1;
                continue;
            };
            if let Some(key) = command.key() {
                if self.latest.get(&key) == Some(&index) {
                    self.latest.remove(&key);
                }
            }
            self.live -= 1;
            drained.push(command);
        }
        self.compact_if_needed();
        drained
    }

    fn drain(&mut self) -> Vec<ModelCommand> {
        self.drain_limited(usize::MAX)
    }

    fn compact_if_needed(&mut self) {
        if self.order.len() < Self::COMPACT_MIN_PHYSICAL || self.stale * 2 < self.order.len() {
            return;
        }
        let mut slots = Vec::with_capacity(self.live);
        let mut order = VecDeque::with_capacity(self.live);
        let mut latest = HashMap::with_capacity(self.latest.len());
        while let Some(index) = self.order.pop_front() {
            let Some(command) = self.slots[index].take() else {
                continue;
            };
            let new_index = slots.len();
            if let Some(key) = command.key() {
                latest.insert(key, new_index);
            }
            slots.push(Some(command));
            order.push_back(new_index);
        }
        self.slots = slots;
        self.order = order;
        self.latest = latest;
        self.stale = 0;
        self.compactions += 1;
    }
}

#[derive(Debug)]
struct LinkedNode {
    command: ModelCommand,
    previous: Option<usize>,
    next: Option<usize>,
}

/// Second candidate: an indexed intrusive list. Key replacement unlinks the
/// old node and appends a reused slot without tombstones or compaction.
#[derive(Default)]
struct LinkedSlotQueue {
    nodes: Vec<Option<LinkedNode>>,
    free: Vec<usize>,
    latest: HashMap<CoalesceKey, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    live: usize,
    peak_slots: usize,
}

impl LinkedSlotQueue {
    fn unlink(&mut self, index: usize) -> ModelCommand {
        let node = self.nodes[index]
            .take()
            .expect("linked queue index must reference a live node");
        if let Some(previous) = node.previous {
            self.nodes[previous]
                .as_mut()
                .expect("previous node must be live")
                .next = node.next;
        } else {
            self.head = node.next;
        }
        if let Some(next) = node.next {
            self.nodes[next]
                .as_mut()
                .expect("next node must be live")
                .previous = node.previous;
        } else {
            self.tail = node.previous;
        }
        self.live -= 1;
        self.free.push(index);
        node.command
    }

    fn append(&mut self, command: ModelCommand) -> usize {
        let index = self.free.pop().unwrap_or(self.nodes.len());
        let node = LinkedNode {
            command,
            previous: self.tail,
            next: None,
        };
        if index == self.nodes.len() {
            self.nodes.push(Some(node));
        } else {
            self.nodes[index] = Some(node);
        }
        if let Some(tail) = self.tail {
            self.nodes[tail]
                .as_mut()
                .expect("tail node must be live")
                .next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
        self.live += 1;
        self.peak_slots = self.peak_slots.max(self.nodes.len());
        index
    }

    fn push(&mut self, mut command: ModelCommand) {
        if let ModelCommand::ClearSheets { origin } = command {
            let keys = self
                .latest
                .keys()
                .filter(|key| {
                    matches!(
                        key,
                        CoalesceKey::Sheet(queued_origin, _)
                            | CoalesceKey::ClearSheets(queued_origin)
                            if *queued_origin == origin
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                if let Some(index) = self.latest.remove(&key) {
                    self.unlink(index);
                }
            }
            command = ModelCommand::ClearSheets { origin };
        } else if let Some(key) = command.key() {
            if let Some(index) = self.latest.remove(&key) {
                let previous = self.unlink(index);
                command.merge_sticky_flags_from(&previous);
            }
        }

        let key = command.key();
        let index = self.append(command);
        if let Some(key) = key {
            self.latest.insert(key, index);
        }
    }

    fn drain_limited(&mut self, limit: usize) -> Vec<ModelCommand> {
        let mut drained = Vec::with_capacity(limit.min(self.live));
        while drained.len() < limit {
            let Some(index) = self.head else {
                break;
            };
            let key = self.nodes[index]
                .as_ref()
                .expect("head node must be live")
                .command
                .key();
            if let Some(key) = key {
                if self.latest.get(&key) == Some(&index) {
                    self.latest.remove(&key);
                }
            }
            drained.push(self.unlink(index));
        }
        drained
    }

    fn drain(&mut self) -> Vec<ModelCommand> {
        self.drain_limited(usize::MAX)
    }
}

#[derive(Debug)]
enum GenerationEntry {
    Keyed { key: CoalesceKey, generation: u64 },
    Lossless(ModelCommand),
}

/// Third candidate: ordering contains lightweight generation tokens while the
/// latest keyed payloads live in a map. Replaced payloads are released at once;
/// stale tokens are skipped during drain and periodically compacted.
#[derive(Default)]
struct GenerationalQueue {
    order: VecDeque<GenerationEntry>,
    latest: HashMap<CoalesceKey, (u64, ModelCommand)>,
    next_generation: u64,
    live: usize,
    stale: usize,
    compactions: usize,
    peak_physical: usize,
}

impl GenerationalQueue {
    const COMPACT_MIN_PHYSICAL: usize = 64;

    fn invalidate_key(&mut self, key: &CoalesceKey) -> Option<ModelCommand> {
        let (_, previous) = self.latest.remove(key)?;
        self.live -= 1;
        self.stale += 1;
        Some(previous)
    }

    fn push(&mut self, mut command: ModelCommand) {
        if let ModelCommand::ClearSheets { origin } = command {
            let keys = self
                .latest
                .keys()
                .filter(|key| {
                    matches!(
                        key,
                        CoalesceKey::Sheet(queued_origin, _)
                            | CoalesceKey::ClearSheets(queued_origin)
                            if *queued_origin == origin
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                self.invalidate_key(&key);
            }
            command = ModelCommand::ClearSheets { origin };
        } else if let Some(key) = command.key() {
            if let Some(previous) = self.invalidate_key(&key) {
                command.merge_sticky_flags_from(&previous);
            }
        }

        if let Some(key) = command.key() {
            let generation = self.next_generation;
            self.next_generation = self.next_generation.wrapping_add(1);
            self.latest.insert(key.clone(), (generation, command));
            self.order
                .push_back(GenerationEntry::Keyed { key, generation });
        } else {
            self.order.push_back(GenerationEntry::Lossless(command));
        }
        self.live += 1;
        self.peak_physical = self.peak_physical.max(self.order.len());
        self.compact_if_needed();
    }

    fn drain_limited(&mut self, limit: usize) -> Vec<ModelCommand> {
        let mut drained = Vec::with_capacity(limit.min(self.live));
        while drained.len() < limit {
            let Some(entry) = self.order.pop_front() else {
                break;
            };
            match entry {
                GenerationEntry::Lossless(command) => {
                    self.live -= 1;
                    drained.push(command);
                }
                GenerationEntry::Keyed { key, generation } => {
                    let is_latest = self
                        .latest
                        .get(&key)
                        .is_some_and(|(current, _)| *current == generation);
                    if !is_latest {
                        self.stale -= 1;
                        continue;
                    }
                    let (_, command) = self
                        .latest
                        .remove(&key)
                        .expect("latest generation must retain its payload");
                    self.live -= 1;
                    drained.push(command);
                }
            }
        }
        self.compact_if_needed();
        drained
    }

    fn drain(&mut self) -> Vec<ModelCommand> {
        self.drain_limited(usize::MAX)
    }

    fn compact_if_needed(&mut self) {
        if self.order.len() < Self::COMPACT_MIN_PHYSICAL || self.stale * 2 < self.order.len() {
            return;
        }
        self.order.retain(|entry| match entry {
            GenerationEntry::Lossless(_) => true,
            GenerationEntry::Keyed { key, generation } => self
                .latest
                .get(key)
                .is_some_and(|(current, _)| current == generation),
        });
        self.stale = 0;
        self.compactions += 1;
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn bounded(&mut self, bound: u32) -> u32 {
        self.next() % bound
    }
}

fn generated_command(rng: &mut Lcg, sequence: u32) -> ModelCommand {
    let widget = rng.bounded(8) as u8;
    match rng.bounded(12) {
        0 => ModelCommand::SetProp {
            widget: widget as u32,
            prop: rng.bounded(4),
            value: sequence,
        },
        1 => ModelCommand::Theme { value: sequence },
        2 => ModelCommand::SetSheet {
            origin: rng.bounded(3) as u8,
            id: (rng.bounded(4) != 0).then(|| rng.bounded(4) as u8),
            value: sequence,
        },
        3 => ModelCommand::RemoveSheet {
            origin: rng.bounded(3) as u8,
            id: rng.bounded(4) as u8,
        },
        4 => ModelCommand::ClearSheets {
            origin: rng.bounded(3) as u8,
        },
        5 => ModelCommand::Scatter {
            widget,
            value: sequence,
            fit: rng.bounded(5) == 0,
        },
        6 => ModelCommand::Line {
            widget,
            series: rng.bounded(3) as u8,
            value: sequence,
            fit: rng.bounded(5) == 0,
        },
        7 => ModelCommand::Histogram {
            widget,
            value: sequence,
            auto_fit: rng.bounded(5) == 0,
        },
        8 => ModelCommand::ScalarBar {
            widget,
            value: sequence,
        },
        9 => ModelCommand::Actor {
            widget,
            actor: rng.bounded(3) as u8,
            value: sequence,
        },
        10 => ModelCommand::Lossless { value: sequence },
        _ => ModelCommand::Snapshot { request: sequence },
    }
}

#[test]
fn stable_slot_candidate_matches_reference_for_seeded_mixed_streams() {
    for seed in 0..2_000_u64 {
        let mut rng = Lcg(seed ^ 0x9e3779b97f4a7c15);
        let mut reference = ReferenceQueue::default();
        let mut candidate = StableSlotQueue::default();
        let mut linked = LinkedSlotQueue::default();
        let mut generational = GenerationalQueue::default();
        for sequence in 0..200 {
            let command = generated_command(&mut rng, sequence);
            reference.push(command.clone());
            candidate.push(command.clone());
            linked.push(command.clone());
            generational.push(command);
            if rng.bounded(11) == 0 {
                let limit = rng.bounded(7) as usize;
                let expected = reference.drain_limited(limit);
                assert_eq!(
                    candidate.drain_limited(limit),
                    expected,
                    "stable-slot partial drain differed for seed {seed} at sequence {sequence}",
                );
                assert_eq!(
                    linked.drain_limited(limit),
                    expected,
                    "linked-slot partial drain differed for seed {seed} at sequence {sequence}",
                );
                assert_eq!(
                    generational.drain_limited(limit),
                    expected,
                    "generational partial drain differed for seed {seed} at sequence {sequence}",
                );
            }
        }
        let expected = reference.drain();
        assert_eq!(
            candidate.drain(),
            expected,
            "stable-slot final drain differed for seed {seed}",
        );
        assert_eq!(
            linked.drain(),
            expected,
            "linked-slot final drain differed for seed {seed}",
        );
        assert_eq!(
            generational.drain(),
            expected,
            "generational final drain differed for seed {seed}",
        );
    }
}

#[test]
fn linked_slot_candidate_reuses_slots_without_tombstone_growth() {
    let mut candidate = LinkedSlotQueue::default();
    for value in 0..100_000 {
        candidate.push(ModelCommand::SetProp {
            widget: 1,
            prop: 1,
            value,
        });
        assert_eq!(candidate.live, 1);
        assert_eq!(candidate.nodes.len(), 1);
    }
    assert_eq!(candidate.peak_slots, 1);
    assert_eq!(candidate.drain().len(), 1);
}

#[test]
fn generational_candidate_releases_payloads_and_bounds_stale_tokens() {
    let mut candidate = GenerationalQueue::default();
    for value in 0..100_000 {
        candidate.push(set_prop(1, value));
        assert_eq!(candidate.live, 1);
        assert!(candidate.order.len() < GenerationalQueue::COMPACT_MIN_PHYSICAL * 2);
    }
    assert!(candidate.compactions > 0);
    assert_eq!(candidate.latest.len(), 1);
    assert_eq!(candidate.drain().len(), 1);
}

fn set_prop(index: u32, value: u32) -> ModelCommand {
    ModelCommand::SetProp {
        widget: index,
        prop: 0,
        value,
    }
}

#[test]
#[ignore = "manual model microbenchmark; run with --ignored --nocapture"]
fn queue_candidate_timing_report() {
    println!("scenario,pending,updates,legacy_ms,tombstone_ms,linked_ms,generational_ms");

    for pending in [32_u32, 1_000, 10_000] {
        let started = Instant::now();
        let mut legacy = ReferenceQueue::default();
        for index in 0..pending {
            legacy.push(set_prop(index, index));
        }
        let legacy_ms = started.elapsed().as_secs_f64() * 1_000.0;

        let started = Instant::now();
        let mut tombstone = StableSlotQueue::default();
        for index in 0..pending {
            tombstone.push(set_prop(index, index));
        }
        let tombstone_ms = started.elapsed().as_secs_f64() * 1_000.0;

        let started = Instant::now();
        let mut linked = LinkedSlotQueue::default();
        for index in 0..pending {
            linked.push(set_prop(index, index));
        }
        let linked_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let started = Instant::now();
        let mut generational = GenerationalQueue::default();
        for index in 0..pending {
            generational.push(set_prop(index, index));
        }
        let generational_ms = started.elapsed().as_secs_f64() * 1_000.0;
        black_box((
            legacy.drain(),
            tombstone.drain(),
            linked.drain(),
            generational.drain(),
        ));
        println!(
            "distinct_insert,{pending},{pending},{legacy_ms:.6},{tombstone_ms:.6},{linked_ms:.6},{generational_ms:.6}"
        );
    }

    for (pending, updates) in [(32_u32, 10_000_u32), (1_000, 10_000), (100_000, 1_000)] {
        let seeded = (0..pending).map(|index| set_prop(index, index));
        let mut legacy = ReferenceQueue {
            items: seeded.clone().collect(),
        };
        let mut tombstone = StableSlotQueue::default();
        let mut linked = LinkedSlotQueue::default();
        let mut generational = GenerationalQueue::default();
        for command in seeded {
            tombstone.push(command.clone());
            linked.push(command.clone());
            generational.push(command);
        }

        let started = Instant::now();
        for value in 0..updates {
            legacy.push(set_prop(0, value));
        }
        let legacy_ms = started.elapsed().as_secs_f64() * 1_000.0;

        let started = Instant::now();
        for value in 0..updates {
            tombstone.push(set_prop(0, value));
        }
        let tombstone_ms = started.elapsed().as_secs_f64() * 1_000.0;

        let started = Instant::now();
        for value in 0..updates {
            linked.push(set_prop(0, value));
        }
        let linked_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let started = Instant::now();
        for value in 0..updates {
            generational.push(set_prop(0, value));
        }
        let generational_ms = started.elapsed().as_secs_f64() * 1_000.0;
        black_box((
            legacy.drain(),
            tombstone.drain(),
            linked.drain(),
            generational.drain(),
        ));
        println!(
            "prefilled_hot_key,{pending},{updates},{legacy_ms:.6},{tombstone_ms:.6},{linked_ms:.6},{generational_ms:.6}"
        );
    }

    let updates = 100_000_u32;
    let started = Instant::now();
    let mut legacy = ReferenceQueue::default();
    for value in 0..updates {
        legacy.push(set_prop(0, value));
    }
    let legacy_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let started = Instant::now();
    let mut tombstone = StableSlotQueue::default();
    for value in 0..updates {
        tombstone.push(set_prop(0, value));
    }
    let tombstone_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let started = Instant::now();
    let mut linked = LinkedSlotQueue::default();
    for value in 0..updates {
        linked.push(set_prop(0, value));
    }
    let linked_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let started = Instant::now();
    let mut generational = GenerationalQueue::default();
    for value in 0..updates {
        generational.push(set_prop(0, value));
    }
    let generational_ms = started.elapsed().as_secs_f64() * 1_000.0;
    black_box((
        legacy.drain(),
        tombstone.drain(),
        linked.drain(),
        generational.drain(),
    ));
    println!(
        "empty_same_key,{updates},{updates},{legacy_ms:.6},{tombstone_ms:.6},{linked_ms:.6},{generational_ms:.6}"
    );
}

#[test]
fn stable_slot_candidate_releases_payloads_and_bounds_stale_tokens() {
    let mut candidate = StableSlotQueue::default();
    for value in 0..100_000 {
        candidate.push(ModelCommand::SetProp {
            widget: 1,
            prop: 1,
            value,
        });
        assert!(candidate.live <= 1);
        assert!(candidate.order.len() < StableSlotQueue::COMPACT_MIN_PHYSICAL * 2);
    }
    assert!(candidate.compactions > 0);
    assert_eq!(candidate.drain().len(), 1);
}
