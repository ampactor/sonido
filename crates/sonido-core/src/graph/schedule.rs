//! Compiled schedule types for the DAG routing engine.
//!
//! A [`CompiledSchedule`] is an immutable snapshot produced by
//! [`ProcessingGraph::compile()`](super::ProcessingGraph::compile). It contains a
//! flat list of [`ProcessStep`] instructions that the audio thread executes
//! sequentially, plus metadata about buffer requirements and latency compensation.
//!
//! The schedule is shared with the audio thread via `Arc` — the audio thread
//! never sees partial state.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Maximum number of fan-out targets for a single Split node.
///
/// Fixed-size array eliminates the `Vec<usize>` heap allocation from `SplitCopy`,
/// making the `ProcessStep` enum entirely stack-allocated.
pub const MAX_SPLIT_TARGETS: usize = 8;

/// A single instruction in the compiled processing schedule.
///
/// Steps are executed sequentially by the audio thread. The instruction set
/// is minimal: write external input, process an effect, copy/accumulate buffers,
/// apply latency compensation, handle feedback loops and control-rate nodes,
/// and read final output.
///
/// All variants are stack-allocated (no heap pointers) for RT-safety.
#[derive(Debug)]
pub enum ProcessStep {
    /// Write external audio input into a buffer slot.
    WriteInput {
        /// Buffer slot to write the external input into.
        buffer_idx: usize,
    },

    /// Process audio through an effect node.
    ///
    /// Reads from `input_buf`, writes to `output_buf`. The node index
    /// references into `ProcessingGraph.nodes`. When `sidechain_buf` is
    /// `Some`, the effect's sidechain kernel path is invoked. When
    /// `is_control_rate` is `true`, only the first sample is processed
    /// and the output is held via a subsequent `SampleAndHold` step.
    ProcessEffect {
        /// Index into the graph's node storage.
        node_idx: usize,
        /// Buffer slot containing the input signal.
        input_buf: usize,
        /// Buffer slot to write the processed output into.
        output_buf: usize,
        /// Optional buffer slot containing the external sidechain signal.
        ///
        /// When `Some`, the node's kernel receives this buffer as sidechain
        /// input instead of deriving detection from the main signal.
        sidechain_buf: Option<usize>,
        /// Whether this node operates at control rate.
        ///
        /// When `true`, the executor processes only 1 sample instead of the
        /// full block.  A `SampleAndHold` step must follow immediately to
        /// replicate that sample across the buffer.
        is_control_rate: bool,
    },

    /// Process a sub-graph as a single opaque step.
    ///
    /// The inner [`super::ProcessingGraph`] is stored inside the
    /// [`NodeKind::SubGraph`](super::node::NodeKind::SubGraph) variant.  The
    /// schedule compiler emits this step instead of descending into the inner
    /// graph's nodes individually.
    ProcessSubGraph {
        /// Node index of the `SubGraph` node in the outer graph's node array.
        node_idx: usize,
        /// Buffer slot containing the input signal for the sub-graph.
        input_buf: usize,
        /// Buffer slot to write the sub-graph's output into.
        output_buf: usize,
    },

    /// Copy a source buffer to one or more destination buffers (fan-out from Split node).
    ///
    /// Uses a fixed-size array to avoid heap allocation. `dest_count` indicates
    /// how many entries in `dest_bufs` are valid.
    SplitCopy {
        /// Buffer slot to copy from.
        source_buf: usize,
        /// Buffer slots to copy into (first `dest_count` entries are valid).
        dest_bufs: [usize; MAX_SPLIT_TARGETS],
        /// Number of valid entries in `dest_bufs`.
        dest_count: usize,
    },

    /// Clear a buffer to silence (used before accumulation at Merge nodes).
    ClearBuffer {
        /// Buffer slot to zero out.
        buffer_idx: usize,
    },

    /// Add (accumulate) a source buffer into a destination buffer, scaled by `gain`.
    ///
    /// Used for fan-in at Merge nodes: clear dest, then accumulate each input.
    /// Gain is `1.0 / path_count` so that N paths sum to unity, preventing
    /// amplitude doubling on split→merge topologies.
    AccumulateBuffer {
        /// Buffer slot to read from.
        source_buf: usize,
        /// Buffer slot to add into.
        dest_buf: usize,
        /// Gain applied to each accumulated sample (typically `1.0 / path_count`).
        gain: f32,
    },

    /// Apply latency compensation delay to a buffer.
    ///
    /// Inserted automatically during compilation for shorter parallel paths
    /// feeding the same Merge node.
    DelayCompensate {
        /// Buffer slot to delay in-place.
        buffer_idx: usize,
        /// Index into the persistent `ProcessingGraph.audio_delay_lines` array.
        delay_line_idx: usize,
    },

    /// Apply a 1-block feedback delay to a buffer.
    ///
    /// Inserted on feedback edges to make the cycle causal.  The persistent
    /// delay line stores the previous block's samples; each block it outputs
    /// the stored samples and then stores the incoming samples for next block.
    ///
    /// This is structurally similar to `DelayCompensate` but semantically
    /// distinct: it exists for causality (not latency alignment) and its delay
    /// is always exactly `block_size` samples.
    FeedbackDelay {
        /// Buffer slot containing the feedback signal (modified in-place).
        buffer_idx: usize,
        /// Index into the persistent `ProcessingGraph.feedback_delay_lines` array.
        delay_line_idx: usize,
    },

    /// Replicate the first sample of a buffer across the whole block.
    ///
    /// Inserted immediately after a control-rate `ProcessEffect` step.
    /// The control-rate node writes exactly 1 sample into `buffer_idx`; this
    /// step copies that sample to positions `1..block_len` so downstream
    /// audio-rate nodes see a full-length block.
    SampleAndHold {
        /// Buffer slot to replicate.
        buffer_idx: usize,
        /// Number of samples in the block.
        block_len: usize,
    },

    /// Read the final output from a buffer slot.
    ReadOutput {
        /// Buffer slot containing the final mixed output.
        buffer_idx: usize,
    },
}

/// Immutable compiled snapshot of the processing graph.
///
/// Shared with the audio thread via `Arc`. Never mutated after creation.
/// The audio thread sees complete state or nothing — no partial updates.
///
/// Buffer pool and compensation delay lines are owned by [`ProcessingGraph`](super::ProcessingGraph)
/// as persistent fields for RT-safe execution (zero per-block allocations).
/// This schedule stores only the counts/sizes needed to validate or rebuild them.
pub struct CompiledSchedule {
    /// Flat list of processing instructions, in execution order.
    pub(crate) steps: Vec<ProcessStep>,
    /// Number of buffer slots required (determined by liveness analysis).
    pub(crate) buffer_count: usize,
    /// Delay sample counts for latency compensation on parallel paths.
    /// Each entry corresponds to a `DelayCompensate` step's `delay_line_idx`.
    pub(crate) delay_sample_counts: Vec<usize>,
    /// Total graph latency in samples (longest path from input to output).
    pub(crate) total_latency: usize,
    /// Block size used for `SampleAndHold` and `FeedbackDelay` steps.
    pub(crate) block_size: usize,
    /// Number of feedback delay lines needed.
    /// Each entry corresponds to a `FeedbackDelay` step's `delay_line_idx`.
    pub(crate) feedback_delay_count: usize,
}

impl CompiledSchedule {
    /// Returns the number of processing steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns the number of buffer slots allocated.
    pub fn buffer_count(&self) -> usize {
        self.buffer_count
    }

    /// Returns the total graph latency in samples.
    pub fn total_latency(&self) -> usize {
        self.total_latency
    }

    /// Returns the number of compensation delay lines.
    pub fn delay_line_count(&self) -> usize {
        self.delay_sample_counts.len()
    }

    /// Returns the block size this schedule was compiled for.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Returns the number of feedback delay lines.
    pub fn feedback_delay_count(&self) -> usize {
        self.feedback_delay_count
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::boxed::Box;

    use super::*;
    use crate::Effect;
    use crate::effect_with_params::EffectWithParams;
    use crate::graph::processing::ProcessingGraph;
    use crate::param_info::{ParamDescriptor, ParameterInfo};

    // ── minimal no-op effect for topology tests ────────────────────────────

    struct Passthrough;

    impl Effect for Passthrough {
        fn process(&mut self, input: f32) -> f32 {
            input
        }
        fn set_sample_rate(&mut self, _sr: f32) {}
        fn reset(&mut self) {}
    }

    impl ParameterInfo for Passthrough {
        fn param_count(&self) -> usize {
            0
        }
        fn param_info(&self, _: usize) -> Option<ParamDescriptor> {
            None
        }
        fn get_param(&self, _: usize) -> f32 {
            0.0
        }
        fn set_param(&mut self, _: usize, _: f32) {}
    }

    fn pt() -> Box<dyn EffectWithParams + Send> {
        Box::new(Passthrough)
    }

    /// Build a minimal graph with just Input → Output (no effects).
    fn empty_graph() -> ProcessingGraph {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let input = g.add_input();
        let output = g.add_output();
        g.connect(input, output).unwrap();
        g
    }

    // ── CompiledSchedule accessors ─────────────────────────────────────────

    #[test]
    fn empty_graph_compiles_without_error() {
        let mut g = empty_graph();
        let sched = g.compile();
        assert!(sched.is_ok());
    }

    #[test]
    fn empty_graph_produces_nonzero_steps() {
        let mut g = empty_graph();
        let sched = g.compile().unwrap();
        // Minimum: WriteInput + ReadOutput = 2 steps.
        assert!(sched.step_count() >= 2);
    }

    #[test]
    fn empty_graph_zero_total_latency() {
        let mut g = empty_graph();
        let sched = g.compile().unwrap();
        assert_eq!(sched.total_latency(), 0);
    }

    #[test]
    fn empty_graph_has_no_delay_lines() {
        let mut g = empty_graph();
        let sched = g.compile().unwrap();
        assert_eq!(sched.delay_line_count(), 0);
    }

    #[test]
    fn schedule_block_size_matches_graph() {
        let block_size = 128;
        let mut g = ProcessingGraph::new(48000.0, block_size);
        let inp = g.add_input();
        let out = g.add_output();
        g.connect(inp, out).unwrap();
        let sched = g.compile().unwrap();
        assert_eq!(sched.block_size(), block_size);
    }

    // ── Linear chain produces correct step ordering ────────────────────────

    #[test]
    fn linear_chain_compiles_successfully() {
        // Input → fx_a → fx_b → Output
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx_a).unwrap();
        g.connect(fx_a, fx_b).unwrap();
        g.connect(fx_b, out).unwrap();
        assert!(g.compile().is_ok());
    }

    #[test]
    fn linear_chain_step_order_starts_with_write_input() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx).unwrap();
        g.connect(fx, out).unwrap();
        let sched = g.compile().unwrap();
        assert!(
            matches!(sched.steps[0], ProcessStep::WriteInput { .. }),
            "first step must be WriteInput"
        );
    }

    #[test]
    fn linear_chain_step_order_ends_with_read_output() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx).unwrap();
        g.connect(fx, out).unwrap();
        let sched = g.compile().unwrap();
        let last = sched.steps.last().unwrap();
        assert!(
            matches!(last, ProcessStep::ReadOutput { .. }),
            "last step must be ReadOutput"
        );
    }

    #[test]
    fn linear_chain_contains_process_effect_step() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx).unwrap();
        g.connect(fx, out).unwrap();
        let sched = g.compile().unwrap();
        let has_process = sched
            .steps
            .iter()
            .any(|s| matches!(s, ProcessStep::ProcessEffect { .. }));
        assert!(has_process, "schedule must contain a ProcessEffect step");
    }

    #[test]
    fn two_effect_linear_chain_has_two_process_steps() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx_a).unwrap();
        g.connect(fx_a, fx_b).unwrap();
        g.connect(fx_b, out).unwrap();
        let sched = g.compile().unwrap();
        let count = sched
            .steps
            .iter()
            .filter(|s| matches!(s, ProcessStep::ProcessEffect { .. }))
            .count();
        assert_eq!(count, 2);
    }

    // ── Parallel paths produce valid topological order ─────────────────────

    #[test]
    fn parallel_paths_compile_successfully() {
        // Input → Split → fx_a ─┐
        //                 fx_b ─┤→ Merge → Output
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let split = g.add_split();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let merge = g.add_merge();
        let out = g.add_output();
        g.connect(inp, split).unwrap();
        g.connect(split, fx_a).unwrap();
        g.connect(split, fx_b).unwrap();
        g.connect(fx_a, merge).unwrap();
        g.connect(fx_b, merge).unwrap();
        g.connect(merge, out).unwrap();
        assert!(g.compile().is_ok());
    }

    #[test]
    fn parallel_paths_schedule_starts_with_write_input() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let split = g.add_split();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let merge = g.add_merge();
        let out = g.add_output();
        g.connect(inp, split).unwrap();
        g.connect(split, fx_a).unwrap();
        g.connect(split, fx_b).unwrap();
        g.connect(fx_a, merge).unwrap();
        g.connect(fx_b, merge).unwrap();
        g.connect(merge, out).unwrap();
        let sched = g.compile().unwrap();
        assert!(matches!(sched.steps[0], ProcessStep::WriteInput { .. }));
    }

    #[test]
    fn parallel_paths_schedule_ends_with_read_output() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let split = g.add_split();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let merge = g.add_merge();
        let out = g.add_output();
        g.connect(inp, split).unwrap();
        g.connect(split, fx_a).unwrap();
        g.connect(split, fx_b).unwrap();
        g.connect(fx_a, merge).unwrap();
        g.connect(fx_b, merge).unwrap();
        g.connect(merge, out).unwrap();
        let sched = g.compile().unwrap();
        let last = sched.steps.last().unwrap();
        assert!(matches!(last, ProcessStep::ReadOutput { .. }));
    }

    #[test]
    fn parallel_paths_write_input_precedes_all_process_effects() {
        // WriteInput must appear before every ProcessEffect in the step list.
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let split = g.add_split();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let merge = g.add_merge();
        let out = g.add_output();
        g.connect(inp, split).unwrap();
        g.connect(split, fx_a).unwrap();
        g.connect(split, fx_b).unwrap();
        g.connect(fx_a, merge).unwrap();
        g.connect(fx_b, merge).unwrap();
        g.connect(merge, out).unwrap();
        let sched = g.compile().unwrap();

        let write_pos = sched
            .steps
            .iter()
            .position(|s| matches!(s, ProcessStep::WriteInput { .. }))
            .expect("WriteInput must be present");
        for (pos, step) in sched.steps.iter().enumerate() {
            if matches!(step, ProcessStep::ProcessEffect { .. }) {
                assert!(
                    pos > write_pos,
                    "ProcessEffect at step {pos} must come after WriteInput at step {write_pos}"
                );
            }
        }
    }

    #[test]
    fn parallel_paths_split_copy_precedes_both_process_effects() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let split = g.add_split();
        let fx_a = g.add_effect(pt());
        let fx_b = g.add_effect(pt());
        let merge = g.add_merge();
        let out = g.add_output();
        g.connect(inp, split).unwrap();
        g.connect(split, fx_a).unwrap();
        g.connect(split, fx_b).unwrap();
        g.connect(fx_a, merge).unwrap();
        g.connect(fx_b, merge).unwrap();
        g.connect(merge, out).unwrap();
        let sched = g.compile().unwrap();

        let split_pos = sched
            .steps
            .iter()
            .position(|s| matches!(s, ProcessStep::SplitCopy { .. }))
            .expect("SplitCopy must be present for a split node");
        let first_effect_pos = sched
            .steps
            .iter()
            .position(|s| matches!(s, ProcessStep::ProcessEffect { .. }))
            .expect("ProcessEffect must be present");
        assert!(
            split_pos < first_effect_pos,
            "SplitCopy must precede all ProcessEffect steps"
        );
    }

    // ── buffer_count is always at least 1 ─────────────────────────────────

    #[test]
    fn compiled_schedule_buffer_count_at_least_one() {
        let mut g = empty_graph();
        let sched = g.compile().unwrap();
        assert!(sched.buffer_count() >= 1);
    }

    #[test]
    fn compiled_schedule_buffer_count_for_linear_chain() {
        let mut g = ProcessingGraph::new(48000.0, 64);
        let inp = g.add_input();
        let fx = g.add_effect(pt());
        let out = g.add_output();
        g.connect(inp, fx).unwrap();
        g.connect(fx, out).unwrap();
        let sched = g.compile().unwrap();
        // A simple linear chain only ever needs 2 live buffers at once (ping-pong).
        assert!(sched.buffer_count() >= 1);
        assert!(sched.buffer_count() <= 4, "linear chain needs few buffers");
    }

    // ── MAX_SPLIT_TARGETS constant ─────────────────────────────────────────

    #[test]
    fn max_split_targets_is_at_least_two() {
        assert!(MAX_SPLIT_TARGETS >= 2);
    }
}
