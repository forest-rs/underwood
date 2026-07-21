// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

const FORMAT_MAJOR: u16 = 0;
const FORMAT_MINOR: u16 = 1;
const ORIGIN_PREDECESSOR: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identities {
    pub(crate) document: u64,
    pub(crate) projection: u64,
    pub(crate) region_chain: u64,
    pub(crate) style: u64,
    pub(crate) text_data: u64,
    pub(crate) fonts: u64,
    pub(crate) schema: u64,
    pub(crate) resources: u64,
    pub(crate) features: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Block {
    pub(crate) id: u32,
    pub(crate) fingerprint: u64,
    pub(crate) extent: u32,
    pub(crate) work: u16,
    pub(crate) carried_effect: u64,
    pub(crate) region_end: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Document {
    pub(crate) source_revision: u64,
    pub(crate) identities: Identities,
    pub(crate) blocks: Vec<Block>,
}

impl Document {
    pub(crate) fn synthetic(block_count: u32, region_blocks: u32) -> Self {
        let blocks = (0..block_count)
            .map(|id| Block {
                id,
                fingerprint: mix(0x626c_6f63_6b00_0000, u64::from(id)),
                extent: 16 * 1024 + (id % 7) * 256,
                work: u16::try_from(1 + id % 11).expect("small synthetic work"),
                carried_effect: if id % 997 == 0 {
                    mix(0x6361_7272_6965_6400, u64::from(id))
                } else {
                    0
                },
                region_end: (id + 1) % region_blocks == 0 || id + 1 == block_count,
            })
            .collect();
        Self {
            source_revision: 1,
            identities: Identities {
                document: 0xD0C0,
                projection: 0xA11C,
                region_chain: 0xC011,
                style: 0x57A1,
                text_data: 0xDA7A,
                fonts: 0xF017,
                schema: 0x5C4E,
                resources: 0xAE50,
                features: 0xF10A_7E57,
            },
            blocks,
        }
    }

    pub(crate) fn edit_metrics_preserving(&self, index: usize) -> Self {
        let mut next = self.clone();
        next.source_revision += 1;
        next.blocks[index].fingerprint = mix(next.blocks[index].fingerprint, 1);
        next
    }

    pub(crate) fn edit_extent(&self, index: usize, delta: u32) -> Self {
        let mut next = self.clone();
        next.source_revision += 1;
        next.blocks[index].fingerprint = mix(next.blocks[index].fingerprint, 2);
        next.blocks[index].extent = next.blocks[index]
            .extent
            .checked_add(delta)
            .expect("synthetic extent remains bounded");
        next
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Policy {
    RegionBoundary,
    Fixed {
        blocks: u16,
    },
    Adaptive {
        work_threshold: u32,
        hard_max_blocks: u16,
    },
}

impl Policy {
    pub(crate) fn id(self) -> u64 {
        match self {
            Self::RegionBoundary => 0x7265_6769_6f6e_0001,
            Self::Fixed { blocks } => mix(0x6669_7865_6400_0001, u64::from(blocks)),
            Self::Adaptive {
                work_threshold,
                hard_max_blocks,
            } => mix(
                mix(0x6164_6170_7400_0001, u64::from(work_threshold)),
                u64::from(hard_max_blocks),
            ),
        }
    }

    fn should_checkpoint(self, block: Block, since_blocks: u32, since_work: u32) -> bool {
        if block.region_end {
            return true;
        }
        match self {
            Self::RegionBoundary => false,
            Self::Fixed { blocks } => since_blocks >= u32::from(blocks),
            Self::Adaptive {
                work_threshold,
                hard_max_blocks,
            } => since_work >= work_threshold || since_blocks >= u32::from(hard_max_blocks),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FlowState {
    next_block: u32,
    region: u32,
    block_coordinate: u64,
    counter_state: u64,
    carried_state: u64,
}

impl FlowState {
    fn digest(self) -> u64 {
        let mut digest = mix(u64::from(self.next_block), u64::from(self.region));
        digest = mix(digest, self.block_coordinate);
        digest = mix(digest, self.counter_state);
        mix(digest, self.carried_state)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Fragment {
    pub(crate) block: u32,
    pub(crate) region: u32,
    pub(crate) coordinate: u64,
    pub(crate) extent: u32,
    pub(crate) digest: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Checkpoint {
    major: u16,
    minor: u16,
    policy: u64,
    source_revision: u64,
    identities: Identities,
    predecessor: u32,
    predecessor_fingerprint: u64,
    state: FlowState,
    interval_output_digest: u64,
    successor_state_digest: u64,
    dependency_frontier: u32,
    measurement_frontier: u32,
    generation: u64,
    completed: bool,
}

impl Checkpoint {
    pub(crate) fn next_block(&self) -> u32 {
        self.state.next_block
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(176);
        push_u16(&mut bytes, self.major);
        push_u16(&mut bytes, self.minor);
        for value in [
            self.policy,
            self.source_revision,
            self.identities.document,
            self.identities.projection,
            self.identities.region_chain,
            self.identities.style,
            self.identities.text_data,
            self.identities.fonts,
            self.identities.schema,
            self.identities.resources,
            self.identities.features,
        ] {
            push_u64(&mut bytes, value);
        }
        push_u32(&mut bytes, self.predecessor);
        push_u64(&mut bytes, self.predecessor_fingerprint);
        push_u32(&mut bytes, self.state.next_block);
        push_u32(&mut bytes, self.state.region);
        push_u64(&mut bytes, self.state.block_coordinate);
        push_u64(&mut bytes, self.state.counter_state);
        push_u64(&mut bytes, self.state.carried_state);
        push_u64(&mut bytes, self.interval_output_digest);
        push_u64(&mut bytes, self.successor_state_digest);
        push_u32(&mut bytes, self.dependency_frontier);
        push_u32(&mut bytes, self.measurement_frontier);
        push_u64(&mut bytes, self.generation);
        bytes.push(u8::from(self.completed));
        bytes
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::new(bytes);
        let major = reader.u16()?;
        let minor = reader.u16()?;
        let policy = reader.u64()?;
        let source_revision = reader.u64()?;
        let identities = Identities {
            document: reader.u64()?,
            projection: reader.u64()?,
            region_chain: reader.u64()?,
            style: reader.u64()?,
            text_data: reader.u64()?,
            fonts: reader.u64()?,
            schema: reader.u64()?,
            resources: reader.u64()?,
            features: reader.u64()?,
        };
        let predecessor = reader.u32()?;
        let predecessor_fingerprint = reader.u64()?;
        let state = FlowState {
            next_block: reader.u32()?,
            region: reader.u32()?,
            block_coordinate: reader.u64()?,
            counter_state: reader.u64()?,
            carried_state: reader.u64()?,
        };
        let checkpoint = Self {
            major,
            minor,
            policy,
            source_revision,
            identities,
            predecessor,
            predecessor_fingerprint,
            state,
            interval_output_digest: reader.u64()?,
            successor_state_digest: reader.u64()?,
            dependency_frontier: reader.u32()?,
            measurement_frontier: reader.u32()?,
            generation: reader.u64()?,
            completed: reader.boolean()?,
        };
        if !reader.is_finished() {
            return Err(DecodeError::TrailingBytes);
        }
        Ok(checkpoint)
    }

    fn origin(document: &Document, policy: Policy, generation: u64) -> Self {
        let state = FlowState::default();
        Self {
            major: FORMAT_MAJOR,
            minor: FORMAT_MINOR,
            policy: policy.id(),
            source_revision: document.source_revision,
            identities: document.identities,
            predecessor: ORIGIN_PREDECESSOR,
            predecessor_fingerprint: 0,
            state,
            interval_output_digest: 0,
            successor_state_digest: state.digest(),
            dependency_frontier: 0,
            measurement_frontier: 0,
            generation,
            completed: true,
        }
    }

    fn valid_for(
        &self,
        document: &Document,
        policy: Policy,
        previous_revision: u64,
        earliest_invalidated: usize,
    ) -> bool {
        if self.major != FORMAT_MAJOR
            || self.policy != policy.id()
            || self.identities != document.identities
            || !self.completed
        {
            return false;
        }
        let source_compatible = self.source_revision == document.source_revision
            || (self.source_revision == previous_revision
                && self.state.next_block as usize <= earliest_invalidated);
        if !source_compatible {
            return false;
        }
        if self.predecessor == ORIGIN_PREDECESSOR {
            return true;
        }
        let predecessor = self.predecessor as usize;
        predecessor < earliest_invalidated
            && document.blocks.get(predecessor).is_some_and(|block| {
                block.id == self.predecessor && block.fingerprint == self.predecessor_fingerprint
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    InvalidBoolean,
    Truncated,
    TrailingBytes,
}

#[derive(Debug)]
pub(crate) struct Layout {
    pub(crate) fragments: Vec<Fragment>,
    pub(crate) checkpoints: Vec<Checkpoint>,
    pub(crate) visited_blocks: usize,
    pub(crate) published: bool,
}

pub(crate) fn layout(
    document: &Document,
    policy: Policy,
    generation: u64,
    cancel_after_work: Option<u64>,
) -> Layout {
    let origin = Checkpoint::origin(document, policy, generation);
    resume(
        document,
        policy,
        &origin,
        generation,
        cancel_after_work,
        None,
    )
}

pub(crate) fn select_restart<'a>(
    checkpoints: &'a [Checkpoint],
    document: &Document,
    policy: Policy,
    previous_revision: u64,
    earliest_invalidated: usize,
) -> Option<&'a Checkpoint> {
    checkpoints.iter().rev().find(|checkpoint| {
        checkpoint.valid_for(document, policy, previous_revision, earliest_invalidated)
    })
}

#[derive(Debug)]
pub(crate) struct Reflow {
    pub(crate) restart_block: u32,
    pub(crate) converged_at: Option<u32>,
    pub(crate) emitted_blocks: usize,
    pub(crate) prefix_fragments_emitted: usize,
}

pub(crate) fn reflow_until_convergence(
    previous: &Layout,
    previous_revision: u64,
    document: &Document,
    policy: Policy,
    invalidated: usize,
    generation: u64,
) -> Reflow {
    let origin = Checkpoint::origin(document, policy, generation);
    let restart = select_restart(
        &previous.checkpoints,
        document,
        policy,
        previous_revision,
        invalidated,
    )
    .unwrap_or(&origin);
    let restart_block = restart.next_block();
    let next = resume(
        document,
        policy,
        restart,
        generation,
        None,
        Some(&previous.checkpoints),
    );
    let converged_at = next.checkpoints.last().and_then(|checkpoint| {
        previous
            .checkpoints
            .iter()
            .find(|old| old.next_block() == checkpoint.next_block())
            .filter(|old| checkpoint_equivalent(old, checkpoint))
            .map(Checkpoint::next_block)
    });
    Reflow {
        restart_block,
        converged_at,
        emitted_blocks: next.visited_blocks,
        prefix_fragments_emitted: next
            .fragments
            .iter()
            .filter(|fragment| fragment.block < restart_block)
            .count(),
    }
}

fn resume(
    document: &Document,
    policy: Policy,
    start: &Checkpoint,
    generation: u64,
    cancel_after_work: Option<u64>,
    convergence_targets: Option<&[Checkpoint]>,
) -> Layout {
    let mut state = start.state;
    let mut fragments = Vec::new();
    let mut checkpoints = Vec::new();
    let mut interval_digest = 0_u64;
    let mut since_blocks = 0_u32;
    let mut since_work = 0_u32;
    let mut total_work = 0_u64;

    for block in &document.blocks[state.next_block as usize..] {
        if cancel_after_work.is_some_and(|budget| total_work + u64::from(block.work) > budget) {
            return Layout {
                fragments: Vec::new(),
                checkpoints: Vec::new(),
                visited_blocks: 0,
                published: false,
            };
        }
        total_work += u64::from(block.work);
        let fragment = advance(block, &mut state);
        interval_digest = mix(interval_digest, fragment.digest);
        fragments.push(fragment);
        since_blocks += 1;
        since_work += u32::from(block.work);

        if policy.should_checkpoint(*block, since_blocks, since_work) {
            let checkpoint = Checkpoint {
                major: FORMAT_MAJOR,
                minor: FORMAT_MINOR,
                policy: policy.id(),
                source_revision: document.source_revision,
                identities: document.identities,
                predecessor: block.id,
                predecessor_fingerprint: block.fingerprint,
                state,
                interval_output_digest: interval_digest,
                successor_state_digest: state.digest(),
                dependency_frontier: state.next_block,
                measurement_frontier: state.next_block,
                generation,
                completed: true,
            };
            let converged = convergence_targets.is_some_and(|targets| {
                targets.iter().any(|old| {
                    old.next_block() == checkpoint.next_block()
                        && checkpoint_equivalent(old, &checkpoint)
                })
            });
            checkpoints.push(checkpoint);
            interval_digest = 0;
            since_blocks = 0;
            since_work = 0;
            if converged {
                break;
            }
        }
    }

    Layout {
        visited_blocks: fragments.len(),
        fragments,
        checkpoints,
        published: true,
    }
}

fn advance(block: &Block, state: &mut FlowState) -> Fragment {
    let coordinate = state.block_coordinate;
    let mut digest = mix(block.fingerprint, u64::from(state.region));
    digest = mix(digest, coordinate);
    digest = mix(digest, u64::from(block.extent));
    let fragment = Fragment {
        block: block.id,
        region: state.region,
        coordinate,
        extent: block.extent,
        digest,
    };

    state.next_block = block.id + 1;
    state.block_coordinate += u64::from(block.extent);
    state.counter_state = mix(state.counter_state, u64::from(block.id));
    if block.carried_effect != 0 {
        state.carried_state = mix(state.carried_state, block.carried_effect);
    }
    if block.region_end {
        state.region += 1;
        state.block_coordinate = 0;
    }
    fragment
}

fn checkpoint_equivalent(left: &Checkpoint, right: &Checkpoint) -> bool {
    left.state.next_block == right.state.next_block
        && left.interval_output_digest == right.interval_output_digest
        && left.successor_state_digest == right.successor_state_digest
        && left.dependency_frontier == right.dependency_frontier
        && left.measurement_frontier == right.measurement_frontier
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Debug)]
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        let byte = *self.bytes.get(self.offset).ok_or(DecodeError::Truncated)?;
        self.offset += 1;
        Ok(byte)
    }

    fn boolean(&mut self) -> Result<bool, DecodeError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidBoolean),
        }
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let end = self.offset.checked_add(N).ok_or(DecodeError::Truncated)?;
        let source = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        source.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

pub(crate) const fn mix(left: u64, right: u64) -> u64 {
    left.rotate_left(13) ^ right.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

#[cfg(test)]
mod tests {
    use super::{Checkpoint, Document, Policy, layout, reflow_until_convergence, select_restart};

    #[test]
    fn checkpoint_round_trip_is_byte_stable() {
        let document = Document::synthetic(2048, 512);
        let policy = Policy::Adaptive {
            work_threshold: 768,
            hard_max_blocks: 1024,
        };
        let result = layout(&document, policy, 1, None);
        let checkpoint = result
            .checkpoints
            .first()
            .expect("synthetic flow creates checkpoints");
        let encoded = checkpoint.encode();
        let decoded = Checkpoint::decode(&encoded).expect("canonical checkpoint decodes");
        assert_eq!(decoded, *checkpoint);
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn checkpoint_decoder_rejects_noncanonical_and_trailing_bytes() {
        let document = Document::synthetic(2048, 512);
        let result = layout(&document, Policy::Fixed { blocks: 64 }, 1, None);
        let mut encoded = result.checkpoints[0].encode();
        let last = encoded
            .len()
            .checked_sub(1)
            .expect("checkpoint encoding contains completed flag");
        encoded[last] = 2;
        assert!(Checkpoint::decode(&encoded).is_err());
        encoded[last] = 1;
        encoded.push(0);
        assert!(Checkpoint::decode(&encoded).is_err());
        encoded.pop();
        encoded.pop();
        assert!(Checkpoint::decode(&encoded).is_err());
    }

    #[test]
    fn restart_is_a_strict_valid_predecessor() {
        let document = Document::synthetic(4096, 512);
        let policy = Policy::Fixed { blocks: 64 };
        let previous = layout(&document, policy, 1, None);
        let edited = document.edit_metrics_preserving(2000);
        let restart = select_restart(
            &previous.checkpoints,
            &edited,
            policy,
            document.source_revision,
            2000,
        )
        .expect("a predecessor checkpoint exists");
        assert!(restart.next_block() as usize <= 2000);
        assert!(
            restart.next_block() == 0 || restart.next_block() - 1 < 2000,
            "predecessor must be strictly before invalidation"
        );
    }

    #[test]
    fn metrics_preserving_edit_converges_without_prefix_emission() {
        let document = Document::synthetic(4096, 512);
        let policy = Policy::Adaptive {
            work_threshold: 768,
            hard_max_blocks: 1024,
        };
        let previous = layout(&document, policy, 1, None);
        let edited = document.edit_metrics_preserving(2000);
        let reflow = reflow_until_convergence(
            &previous,
            document.source_revision,
            &edited,
            policy,
            2000,
            2,
        );
        assert_eq!(reflow.prefix_fragments_emitted, 0);
        assert!(reflow.converged_at.is_some());
        assert!(reflow.emitted_blocks < document.blocks.len());
    }

    #[test]
    fn extent_edit_converges_at_a_region_reset() {
        let document = Document::synthetic(4096, 512);
        let policy = Policy::Fixed { blocks: 64 };
        let previous = layout(&document, policy, 1, None);
        let edited = document.edit_extent(2000, 1024);
        let reflow = reflow_until_convergence(
            &previous,
            document.source_revision,
            &edited,
            policy,
            2000,
            2,
        );
        assert_eq!(reflow.prefix_fragments_emitted, 0);
        assert!(
            reflow.converged_at.is_some_and(|block| block > 2000),
            "region coordinate reset should allow later convergence"
        );
    }

    #[test]
    fn cancellation_publishes_nothing() {
        let document = Document::synthetic(4096, 512);
        let result = layout(&document, Policy::Fixed { blocks: 64 }, 1, Some(100));
        assert!(!result.published);
        assert!(result.fragments.is_empty());
        assert!(result.checkpoints.is_empty());
    }
}
