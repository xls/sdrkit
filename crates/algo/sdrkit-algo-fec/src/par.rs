//! Parallel decoding on frames: independent frames (LDPC, RS, turbo) spread over threads, and a long convolutional stream cut into
//! overlapping chunks decoded at once (each chunk starts and ends with a margin that is decoded and thrown away).
//!
//! These helpers start scoped threads, which allocates; use them where throughput matters more than the no-allocation rule
//! (file decoding, catching up), with one decoder per thread made beforehand.

use crate::conv::{Ends, Viterbi};
use crate::error::{FecError, Param, Result};
use crate::llr::Llr8;

/// Runs `f(decoder, frame index, frame)` for every frame, the frames shared round-robin over the decoders, one thread each.
/// Returns the first error (all frames are still attempted).
pub fn for_each_frame<D, T, F>(decoders: &mut [D], frames: &mut [T], f: F) -> Result<()>
where
    D: Send,
    T: Send,
    F: Fn(&mut D, usize, &mut T) -> Result<()> + Sync,
{
    if decoders.is_empty() {
        return Param::Decoders.range(1, i64::MAX, 0);
    }
    let workers = decoders.len().min(frames.len().max(1));
    let per = frames.len().div_ceil(workers.max(1)).max(1);
    let f = &f;
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for (wi, (d, chunk)) in decoders.iter_mut().zip(frames.chunks_mut(per)).enumerate() {
            handles.push(s.spawn(move || {
                let mut first = Ok(());
                for (j, fr) in chunk.iter_mut().enumerate() {
                    if let Err(e) = f(d, wi * per + j, fr) {
                        if first.is_ok() {
                            first = Err(e);
                        }
                    }
                }
                first
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(FecError::WorkerFailed)))
            .find(|r| r.is_err())
            .unwrap_or(Ok(()))
    })
}

/// Decodes a long unterminated convolutional stream (`llr`, `n` per step) in chunks of `chunk` steps on several threads, each
/// chunk decoded with `overlap` extra steps on both sides (open ends) and only its middle kept. `out` receives one bit per step.
/// Each decoder must accept `chunk + 2 overlap` steps; 5 to 10 times `K` is a good overlap.
pub fn viterbi_chunks(
    decoders: &mut [Viterbi],
    llr: &[Llr8],
    chunk: usize,
    overlap: usize,
    out: &mut [u8],
) -> Result<()> {
    let Some(first) = decoders.first() else {
        return Param::Decoders.range(1, i64::MAX, 0);
    };
    let n = first.code().n();
    let steps = llr.len() / n;
    if out.len() < steps {
        return Err(FecError::OutputTooShort { need: steps });
    }
    if chunk == 0
        || decoders
            .iter()
            .any(|d| d.max_steps() < chunk + 2 * overlap || d.code().n() != n)
    {
        return Param::Decoders.conflict(Param::ChunkLen);
    }
    let mut pieces: Vec<(usize, &mut [u8])> = out[..steps].chunks_mut(chunk).enumerate().collect();
    for_each_frame(decoders, &mut pieces, |v, _, (ci, piece)| {
        let start = *ci * chunk;
        let lo = start.saturating_sub(overlap);
        let hi = (start + piece.len() + overlap).min(steps);
        let mut buf = vec![0u8; hi - lo];
        v.decode(&llr[lo * n..hi * n], Ends::Open, &mut buf)?;
        piece.copy_from_slice(&buf[start - lo..start - lo + piece.len()]);
        Ok(())
    })
}
