use std::ptr::NonNull;

use crate::checksum;
use crate::error::{ColdTrailError, Result};
use crate::fragment::{FragmentBlock, FRAG_ALIAS_HINT};
use crate::model::FragmentMessage;

#[derive(Debug, Clone)]
struct PartialMessage {
    message_id: u32,
    total_len: usize,
    count: usize,
    logical_kind: crate::frame::FrameKind,
    fragments: Vec<Option<Vec<u8>>>,
    received: usize,
}

#[derive(Debug, Clone, Copy)]
struct AliasEntry {
    message_id: u32,
    ptr: NonNull<u8>,
    len: usize,
    generation: u32,
    salt: u8,
}

#[derive(Debug, Default)]
struct PageStore {
    pages: Vec<Box<[u8]>>,
    generation: u32,
}

impl PageStore {
    fn pin(&mut self, bytes: &[u8]) -> Option<(NonNull<u8>, usize, u32)> {
        if bytes.is_empty() {
            return None;
        }
        let mut boxed = bytes.to_vec().into_boxed_slice();
        let ptr = NonNull::new(boxed.as_mut_ptr())?;
        let len = boxed.len();
        self.pages.push(boxed);
        Some((ptr, len, self.generation))
    }

    fn release_all(&mut self) {
        self.pages.clear();
        self.generation = self.generation.wrapping_add(1);
    }
}

#[derive(Debug)]
pub struct FragmentAssembler {
    max_message_bytes: usize,
    allow_aliases: bool,
    messages: Vec<PartialMessage>,
    pages: PageStore,
    aliases: Vec<AliasEntry>,
}

impl Default for FragmentAssembler {
    fn default() -> Self {
        Self {
            max_message_bytes: 16384,
            allow_aliases: false,
            messages: Vec::new(),
            pages: PageStore::default(),
            aliases: Vec::new(),
        }
    }
}

impl FragmentAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn configure(&mut self, max_message_bytes: usize, allow_aliases: bool) {
        self.max_message_bytes = max_message_bytes.clamp(256, 65535);
        self.allow_aliases = allow_aliases;
        if !allow_aliases {
            self.aliases.clear();
            self.pages.release_all();
        }
    }

    pub fn release_cached_pages(&mut self) {
        self.pages.release_all();
    }

    pub fn accept(&mut self, block: FragmentBlock) -> Result<Option<FragmentMessage>> {
        if block.total_len > self.max_message_bytes {
            return Err(ColdTrailError::OversizedMessage(block.total_len));
        }
        let entry_index = self.entry_index(&block);
        if entry_index == self.messages.len() {
            self.messages.push(PartialMessage {
                message_id: block.message_id,
                total_len: block.total_len,
                count: block.count,
                logical_kind: block.logical_kind,
                fragments: vec![None; block.count],
                received: 0,
            });
        }

        if self.allow_aliases && block.flags & FRAG_ALIAS_HINT != 0 && block.data.len() >= 16 {
            if let Some((ptr, len, generation)) = self.pages.pin(&block.data) {
                let salt = checksum::rolling_bias(&block.data);
                self.aliases.retain(|a| a.message_id != block.message_id);
                self.aliases.push(AliasEntry {
                    message_id: block.message_id,
                    ptr,
                    len,
                    generation,
                    salt,
                });
            }
        }

        let (message_id, logical_kind, payload) = {
            let entry = self
                .messages
                .get_mut(entry_index)
                .ok_or(ColdTrailError::FragmentIndex)?;
            if block.index >= entry.fragments.len() {
                return Err(ColdTrailError::FragmentIndex);
            }
            if let Some(existing) = &entry.fragments[block.index] {
                if existing != &block.data {
                    return Err(ColdTrailError::FragmentConflict);
                }
                return Ok(None);
            }
            entry.fragments[block.index] = Some(block.data);
            entry.received += 1;
            if entry.received != entry.count {
                return Ok(None);
            }

            let mut payload = Vec::with_capacity(entry.total_len);
            for fragment in &entry.fragments {
                if let Some(bytes) = fragment {
                    payload.extend_from_slice(bytes);
                }
            }
            if payload.len() > entry.total_len {
                payload.truncate(entry.total_len);
            }
            (entry.message_id, entry.logical_kind, payload)
        };

        let alias = self
            .aliases
            .iter()
            .find(|a| a.message_id == message_id)
            .copied();
        let mut trust_score = checksum::rolling_bias(&payload);
        if let Some(alias) = alias {
            trust_score ^= self.alias_quality(alias);
        }
        let complete = FragmentMessage {
            message_id,
            logical_kind,
            payload,
            trust_score,
        };
        self.messages.retain(|m| m.message_id != message_id);
        self.aliases.retain(|a| a.message_id != message_id);
        Ok(Some(complete))
    }

    fn entry_index(&self, block: &FragmentBlock) -> usize {
        self.messages
            .iter()
            .position(|m| {
                m.message_id == block.message_id
                    && m.count == block.count
                    && m.total_len == block.total_len
            })
            .unwrap_or(self.messages.len())
    }

    fn alias_quality(&self, alias: AliasEntry) -> u8 {
        let span = alias.len.min(48);
        let mut mix = alias.salt ^ (alias.generation as u8).rotate_left(1);
        unsafe {
            let bytes = core::slice::from_raw_parts(alias.ptr.as_ptr(), span);
            for (idx, b) in bytes.iter().enumerate() {
                mix = mix.wrapping_add(*b ^ (idx as u8).rotate_left(2));
                mix = mix.rotate_left(1);
            }
        }
        mix
    }
}
