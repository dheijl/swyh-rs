//! [`ChannelStream`]: the bridge between audio capture and HTTP streaming clients.
//!
//! The [`ChannelStream::write`] method receives captured f32 samples from `wave_reader`
//! via a crossbeam channel. The [`std::io::Read`] implementation on `ChannelStream` is
//! called by the HTTP response writer and encodes those samples on-the-fly into
//! LPCM, WAV, RF64, or FLAC format.
use crate::{
    audio::samples_conv::{
        f32x4_to_i32x4_16, f32x4_to_i32x4_16_dither, f32x4_to_i32x4_24, i32_to_i16be, i32_to_i16le,
        i32_to_i24be, i32_to_i24le,
    },
    enums::streaming::{
        BitDepth::{self, *},
        Dither::{self, *},
        Endian::{self, *},
        StreamingContext, StreamingFormat,
    },
    globals::statics::get_config,
};
use crossbeam_channel::{Receiver, Sender};
use ecow::EcoString;
#[cfg(debug_assertions)]
#[cfg(all(test, debug_assertions))]
use fastrand::Rng;
use log::debug;
use std::{
    io::{Error, Read, Result as IoResult},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use wide::f32x4;

use super::flacstream::FlacChannel;

/// Shared audio sample buffer passed between capture and streaming threads.
/// `Arc<[f32]>` rather than `Arc<Vec<f32>>`: the refcount block and the sample
/// data live in one allocation instead of two, halving the allocations done per
/// CPAL capture callback (see `distribute_samples`).
pub type AudioSamples = Arc<[f32]>;

/// Quantize each 4-sample chunk with `quantize`, then pack the resulting `[i32; 4]`
/// into the matching output bytes with `pack`. Generic over both so that each call site
/// in [`LpcmFormat::convert`] — one per `(endian, bd, dither)`
/// combination — monomorphizes its own copy of this loop with a specific
/// quantizer/packer pair resolved at the type level, guaranteeing a branch-free loop
/// body regardless of optimization level.
#[inline(always)]
fn quantize_and_pack<'b, 's, Q, P>(
    chunks_iter: impl Iterator<Item = (&'b mut [u8], &'s [f32; 4])>,
    quantize: Q,
    pack: P,
) where
    Q: Fn(f32x4) -> [i32; 4],
    P: Fn(&[i32; 4], &mut [u8]),
{
    chunks_iter.for_each(|(bch, sch)| {
        pack(&quantize(f32x4::new(*sch)), bch);
    });
}

/// LPCM/WAV/RF64 output sample format: how groups of 4 f32 samples are
/// quantized and packed into the HTTP read buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LpcmFormat {
    /// WAV/RF64 are little endian, raw LPCM is big endian
    endian: Endian,
    bd: BitDepth,
    /// only applies to 16 bit, 24 bit is never dithered
    dither: Dither,
}

impl LpcmFormat {
    /// number of output bytes for one group of 4 samples
    fn bytes_per_group(self) -> usize {
        match self.bd {
            Bits16 => 4 * 2,
            Bits24 => 4 * 3,
        }
    }

    /// Convert `groups` into `out`, which must be exactly
    /// `groups.len() * self.bytes_per_group()` bytes long.
    /// `quantize_and_pack` is generic over the quantizer/packer pair, so each
    /// arm below monomorphizes its own branch-free copy of the loop with a
    /// specific function pair baked in.
    fn convert(self, out: &mut [u8], groups: &[[f32; 4]]) {
        debug_assert_eq!(out.len(), groups.len() * self.bytes_per_group());
        let chunks_iter = out.chunks_exact_mut(self.bytes_per_group()).zip(groups);
        match (self.endian, self.bd, self.dither) {
            (Little, Bits16, Dither) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_16_dither, i32_to_i16le);
            }
            (Little, Bits16, NoDither) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_16, i32_to_i16le);
            }
            (Little, Bits24, _) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_24, i32_to_i24le);
            }
            (Big, Bits16, Dither) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_16_dither, i32_to_i16be);
            }
            (Big, Bits16, NoDither) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_16, i32_to_i16be);
            }
            (Big, Bits24, _) => {
                quantize_and_pack(chunks_iter, f32x4_to_i32x4_24, i32_to_i24be);
            }
        }
    }
}

/// Receive the next chunk of f32 samples from `r`, or a clone of the
/// `silence` buffer if no samples arrive within `timeout`.
fn recv_or_silence(
    r: &Receiver<AudioSamples>,
    timeout: Duration,
    silence: &AudioSamples,
) -> AudioSamples {
    r.recv_timeout(timeout).unwrap_or_else(|_| silence.clone())
}

/// Channelstream - used to transport the f32 samples from the `wave_reader`
/// to the http output stream in LPCM/WAV/FLAC format
/// implements `Read` for the HTTP streaming
#[derive(Clone)]
pub struct ChannelStream {
    pub s: Sender<AudioSamples>,
    pub r: Receiver<AudioSamples>,
    pub remote_ip: EcoString,
    /// distinguishes a SlimProto fetch from an UPnP fetch when
    /// both share the same `remote_ip` (e.g. MoOde exposing both).
    pub is_slim: bool,
    pub streaming_format: StreamingFormat,
    sample_cursor: SampleCursor,
    flac_cursor: ChunkCursor,
    // Arc'd to prevent copying when Cloning in the server
    silence: Arc<[f32]>,
    capture_timeout: Duration,
    wav_hdr: Vec<u8>,
    use_wave_format: bool,
    lpcm_format: LpcmFormat,
    flac_channel: Option<FlacChannel>,
    /// Set by [`Self::request_stop`] to make `Read::read` return EOF on the
    /// next call, ending the HTTP response even if the client itself never
    /// closes its end of the connection
    /// The Arcs following make sure any clone sees the same values
    stop: Arc<AtomicBool>,
    /// When this client connected — to compute delay before latency drop starts
    connected_at: Instant,
    /// How long to wait after `connected_at` before dropping samples.
    drop_delay: Duration,
    /// Number of samples still to discard in whole incoming chunks
    drop_samples_remaining: Arc<AtomicUsize>,
    /// Count of consecutive chunks dropped since the last pass-through
    drop_streak: Arc<AtomicUsize>,
}

impl ChannelStream {
    /// Number of consecutive chunks `next_drop_decision` drops before
    /// forwarding one, during an active latency-reduction drop window.
    const DROPS_PER_CYCLE: usize = 2;
    /// max number of chunks that can be queued in the write channel
    /// before they are dropped
    const MAX_BACKLOG: usize = 10_000;

    pub fn new(
        tx: Sender<AudioSamples>,
        rx: Receiver<AudioSamples>,
        context: &StreamingContext,
        header_offset: usize,
    ) -> ChannelStream {
        let flac_channel = if context.streaming_format == StreamingFormat::Flac {
            Some(FlacChannel::new(
                rx.clone(),
                context.sample_rate,
                context.bits_per_sample,
                2,
                context.use_dither,
            ))
        } else {
            None
        };
        let capture_timeout = u64::from(get_config().capture_timeout.unwrap_or(5));
        let mut wav_hdr = match context.streaming_format {
            StreamingFormat::Wav => {
                create_wav_hdr(context.sample_rate, context.bits_per_sample as u16)
            }
            StreamingFormat::Rf64 => {
                create_rf64_hdr(context.sample_rate, context.bits_per_sample as u16)
            }
            _ => Vec::new(),
        };
        // Trim the header when the client requested Range: bytes=N- (N within header)
        if header_offset > 0 && !wav_hdr.is_empty() {
            wav_hdr.drain(..header_offset.min(wav_hdr.len()));
        }
        let chs = ChannelStream {
            s: tx,
            r: rx,
            sample_cursor: SampleCursor::default(),
            flac_cursor: ChunkCursor::default(),
            silence: get_silence_buffer(context.sample_rate, capture_timeout / 4).into(),
            capture_timeout: Duration::from_millis(capture_timeout), // silence kicks in after CAPTURE_TIMEOUT seconds
            remote_ip: context.remote_ip.clone(),
            is_slim: context.slim,
            wav_hdr,
            use_wave_format: context.needs_wav_hdr(),
            lpcm_format: LpcmFormat {
                endian: if context.needs_wav_hdr() { Little } else { Big },
                bd: context.bits_per_sample,
                dither: context.use_dither,
            },
            streaming_format: context.streaming_format,
            flac_channel,
            stop: Arc::new(AtomicBool::new(false)),
            connected_at: Instant::now(),
            drop_delay: Duration::from_millis(u64::from(context.latency_reduction_delay_msec)),
            drop_samples_remaining: Arc::new(AtomicUsize::new(stereo_sample_count(
                context.sample_rate,
                u64::from(context.latency_reduction_msec),
            ))),
            drop_streak: Arc::new(AtomicUsize::new(0)),
        };
        if context.streaming_format == StreamingFormat::Flac {
            chs.start_flac_encoder();
        }
        chs
    }

    /// start the flac encoder in a seperate thread
    fn start_flac_encoder(&self) {
        if let Some(flac_channel) = &self.flac_channel {
            flac_channel.run();
        }
    }

    /// stop the flac encoder thread
    pub fn stop_flac_encoder(&self) {
        if let Some(flac_channel) = &self.flac_channel {
            flac_channel.stop();
        }
    }

    /// Force this client's HTTP response to end on its next `read()` call,
    /// regardless of whether the client itself ever closes the connection.
    /// See the `stop` field's doc comment for why this exists.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Decides whether to drop this chunk
    /// - either because the input channel is building up backlog (if streaming
    ///   is stalled), never buffer more than `MAX_BACKLOG` chunks
    /// - or as part of the latency-reduction scheme: drops samples at a
    ///   rate of `DROPS_PER_CYCLE` until the desired latency reduction is
    ///   achieved, passing through every other chunk so silence detection at
    ///   both ends doesn't spuriously kick in. Dropping only starts once
    ///   `drop_delay` has elapsed after connecting (to let the renderer's
    ///   buffer fill) and stops once the desired reduction is achieved.
    #[inline(always)]
    fn should_drop_chunk(&self, chunk_len: usize) -> bool {
        if self.s.len() >= ChannelStream::MAX_BACKLOG {
            #[cfg(debug_assertions)]
            {
                debug!("Samples buffer overflow, dropping chunks!");
            }
            return true;
        }
        let remaining = self.drop_samples_remaining.load(Ordering::Relaxed);
        if remaining == 0 || self.connected_at.elapsed() < self.drop_delay {
            return false;
        }
        let (should_drop, new_remaining, new_drop_streak) = next_drop_decision(
            remaining,
            chunk_len,
            self.drop_streak.load(Ordering::Relaxed),
        );
        self.drop_samples_remaining
            .store(new_remaining, Ordering::Relaxed);
        self.drop_streak.store(new_drop_streak, Ordering::Relaxed);
        should_drop
    }

    /// called by the `run_sample_distributor` thread to write the
    /// f32 samples to our input channel, first discarding the chunk if
    /// `should_drop_chunk` says to (buffer overflow or latency-reduction).
    pub fn write(&self, samples: AudioSamples) {
        if self.should_drop_chunk(samples.len()) {
            return;
        }
        let _ = self.s.send(samples);
    }

    /// fill the HTTP read buffer with encoded flac data from the FlacChannel
    /// the f32 samples have already been encoded to FLAC and written to the
    /// `flac_out` channel of the `FlacChannel` encoder thread.
    /// the `flac_in` channel of the `FlacChannel` is read here, and the received
    /// chunks are copied straight into the read buffer via `flac_cursor`
    fn fill_flac_buffer(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        debug_assert!(self.flac_channel.is_some());
        // borrow instead of clone: `self.flac_channel` and `self.flac_cursor` are
        // disjoint fields, so this doesn't need its own `Receiver` handle
        let flac_in = &self.flac_channel.as_ref().unwrap().flac_in;
        self.flac_cursor.fill(buf, || {
            flac_in
                .recv()
                .map_err(|_| Error::other("FLAC channel receive error."))
        })
    }

    /// Fill the HTTP read buffer with LPCM/WAV/RF64 data.
    /// The f32 sample chunks are read from the f32 input channel (or replaced by
    /// silence after `capture_timeout`) and converted straight from the received
    /// chunks into the HTTP transmission buffer via `sample_cursor`, which only
    /// ever hands out whole groups of 4 samples (SSE2 f32x4).
    fn fill_lpcm_buffer(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        debug_assert!(self.flac_channel.is_none());
        if self.use_wave_format && !self.wav_hdr.is_empty() {
            let i = self.wav_hdr.len();
            debug_assert!(
                buf.len() >= i,
                "HTTP read buffer smaller than WAV/RF64 header!"
            );
            buf[..i].copy_from_slice(&self.wav_hdr);
            self.wav_hdr.clear();
            return Ok(i);
        }
        // borrow only the fields the closures need (not all of `self`), so they
        // can coexist with the `&mut self.sample_cursor` borrow below
        let fmt = self.lpcm_format;
        let (r, timeout, silence) = (&self.r, self.capture_timeout, &self.silence);
        Ok(self.sample_cursor.fill(
            buf,
            fmt.bytes_per_group(),
            || recv_or_silence(r, timeout, silence),
            |out, groups| fmt.convert(out, groups),
        ))
    }
}

/// Read position within the most recently received chunk of f32 samples, so the
/// samples can be converted straight from the chunk into the HTTP read buffer
/// without first being copied into an intermediate fifo.
///
/// The converters work on groups of 4 samples (f32x4), but chunk lengths are
/// arbitrary (e.g. 882 samples for 441-frame WASAPI periods at 44.1 kHz). So
/// `push` splits each chunk into:
/// - a stitched `head` group: the 0..=3 samples left over from the previous
///   chunk, completed with the first samples of this one (copied, max 4 samples)
/// - the whole groups `chunk[pos..end]`, handed out without copying
/// - the 0..=3 leftover samples `chunk[end..]`, saved in `carry` for the next push
#[derive(Clone, Default)]
struct SampleCursor {
    /// the chunk currently being consumed
    chunk: AudioSamples,
    /// next unread sample in `chunk`
    pos: usize,
    /// end of the whole groups in `chunk`, `(end - pos) % 4 == 0`
    end: usize,
    /// stitched group straddling the previous chunk boundary
    head: [f32; 4],
    /// `head` still has to be handed out (before `chunk[pos..end]`)
    has_head: bool,
    /// 0..=3 samples left over from previous chunks, waiting to be completed
    carry: Vec<f32>,
}

impl SampleCursor {
    /// true when all whole groups are used up and a new chunk must be pushed
    fn is_empty(&self) -> bool {
        !self.has_head && self.pos == self.end
    }

    /// Make `chunk` the current chunk; only call this when `is_empty()`.
    /// The result can still be empty (a chunk too short to complete a group
    /// only grows `carry`), so callers must check `is_empty()` again.
    fn push(&mut self, chunk: AudioSamples) {
        debug_assert!(self.is_empty());
        let mut start = 0;
        if !self.carry.is_empty() {
            // complete the pending group from the start of the new chunk
            start = (4 - self.carry.len()).min(chunk.len());
            self.carry.extend_from_slice(&chunk[..start]);
            if self.carry.len() == 4 {
                self.head.copy_from_slice(&self.carry);
                self.has_head = true;
                self.carry.clear();
            }
        }
        // whole groups after `start`, the 0..=3 samples after them go to `carry`
        let end = start + (chunk.len() - start) / 4 * 4;
        self.carry.extend_from_slice(&chunk[end..]);
        self.chunk = chunk;
        self.pos = start;
        self.end = end;
    }

    /// Take up to `max_groups` (at least 1) groups of 4 samples: the stitched
    /// head group if pending, else whole groups from the current chunk
    fn next_groups(&mut self, max_groups: usize) -> &[[f32; 4]] {
        if self.has_head {
            self.has_head = false;
            return std::slice::from_ref(&self.head);
        }
        let (groups, _) = self.chunk[self.pos..self.end].as_chunks::<4>();
        let groups = &groups[..max_groups.min(groups.len())];
        self.pos += groups.len() * 4;
        groups
    }

    /// Fill `buf` with as many whole groups of 4 samples as fit, at
    /// `bytes_per_group` bytes each, pushing new chunks from `next_chunk` as
    /// needed. `convert(out, groups)` writes `groups` into `out`, which is
    /// exactly `groups.len() * bytes_per_group` bytes long. It is called once
    /// per contiguous run of groups, not once per group.
    /// Any tail of `buf` shorter than a group is left untouched.
    /// Returns the number of bytes written.
    fn fill(
        &mut self,
        buf: &mut [u8],
        bytes_per_group: usize,
        mut next_chunk: impl FnMut() -> AudioSamples,
        mut convert: impl FnMut(&mut [u8], &[[f32; 4]]),
    ) -> usize {
        let groups_needed = buf.len() / bytes_per_group;
        // number of groups of 4 samples converted so far
        let mut done = 0;
        while done < groups_needed {
            if self.is_empty() {
                self.push(next_chunk());
                continue; // a tiny chunk may leave the cursor still empty
            }
            let groups = self.next_groups(groups_needed - done);
            let out = &mut buf[done * bytes_per_group..][..groups.len() * bytes_per_group];
            convert(out, groups);
            done += groups.len();
        }
        groups_needed * bytes_per_group
    }
}

/// Read position within the most recently received chunk of encoded bytes
/// (the FLAC encoder output), so chunks can be copied straight into the HTTP
/// read buffer without first being appended to an intermediate fifo.
#[derive(Clone, Default)]
struct ChunkCursor {
    /// the chunk currently being consumed
    chunk: Vec<u8>,
    /// number of bytes of `chunk` already copied out
    pos: usize,
}

impl ChunkCursor {
    /// Fill `buf` completely, first from the unconsumed remainder of the
    /// current chunk, then from new chunks obtained via `next_chunk`.
    /// Bytes left over from the last chunk are kept for the next call.
    /// Returns `buf.len()`, or the first error `next_chunk` returns.
    fn fill<E>(
        &mut self,
        buf: &mut [u8],
        mut next_chunk: impl FnMut() -> Result<Vec<u8>, E>,
    ) -> Result<usize, E> {
        let mut written = 0;
        while written < buf.len() {
            if self.pos == self.chunk.len() {
                // current chunk used up: take the next one
                self.chunk = next_chunk()?;
                self.pos = 0;
                continue;
            }
            let n = (buf.len() - written).min(self.chunk.len() - self.pos);
            buf[written..written + n].copy_from_slice(&self.chunk[self.pos..self.pos + n]);
            written += n;
            self.pos += n;
        }
        Ok(written)
    }
}

/// implement the Read trait for the HTTP writer
/// filling the read buffer with FLAC or LPCM/WAV/RF64 data
impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.stop.load(Ordering::Relaxed) {
            return Ok(0); // EOF: ends the HTTP response on this read
        }
        if self.flac_channel.is_some() {
            self.fill_flac_buffer(buf)
        } else {
            self.fill_lpcm_buffer(buf)
        }
    }
}

/// create an "infinite size" wav hdr for the PCM Data (s16le or s24le)
/// note: this may not work when streaming to an older "libsndfile" based renderer
/// as it insists on a seekable WAV file depending on the open mode used
/*
Field	        Length	Contents
ckID	        4	    Chunk ID: 'RIFF'
cksize	        4	    Chunk size: 4 + 24 + (8 + M*Nc*Ns + (0 or 1)
WAVEID	        4	    WAVE ID: 'WAVE'
ckID	        4	    Chunk ID: 'fmt '
cksize	        4	    Chunk size: 16
wFormatTag	    2	    WAVE_FORMAT_PCM (0001)
nChannels	    2	    Nc
nSamplesPerSec	4	    F
nAvgBytesPerSec	4	    F*M*Nc
nBlockAlign	    2	    M*Nc
wBitsPerSample	2	    rounds up to 8*M
ckID	        4	    Chunk ID: 'data'
cksize	        4	    Chunk size: M*Nc*Ns
sampled data	M*Nc*Ns	Nc*Ns channel-interleaved M-byte samples
pad byte	    0 or 1	Padding byte if M*Nc*Ns is odd
*/
fn create_wav_hdr(sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
    let mut hdr = [0u8; 44];
    hdr[0..4].copy_from_slice(b"RIFF"); //ChunkId, little endian WAV
    let riffchunksize: u32 = u32::MAX; // RIFF chunksize
    let datachunksize: u32 = riffchunksize - 36; // data chunksize
    hdr[4..8].copy_from_slice(&riffchunksize.to_le_bytes()); // RIFF ChunkSize
    hdr[8..12].copy_from_slice(b"WAVE"); // File Format
    write_fmt_chunk(&mut hdr[12..36], sample_rate, bits_per_sample); // SubChunk = Format
    hdr[36..40].copy_from_slice(b"data"); // SubChunk = "data"
    hdr[40..44].copy_from_slice(&datachunksize.to_le_bytes()); // data SubChunkSize
    debug!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
    hdr.to_vec()
}

/// create an "infinite size" RF64 header for the PCM Data (s16le or s24le)
/*
Field           Len offset   Meaning
ckID            4   0        chunk ID 'RF64'
ckSize          4   4        dummy chunksize -1 (0xffffffff)
WAVEID          4   8        compatibility 'WAVE' ID
ckID            4   12       chunk ID 'ds64'
ckSize          4   16       chunk size (28)
RIFFSize        8   20       size of RIFF chunk (data chunk size - 8)
dataSize        8   28       size of data chunk
sampleCount     8   36       number of samples
tableLength     4   44       number of valid table array entries 0
tableArray      0            not used
ckID            4   48       chunk ID 'fmt '
cksize	        4	52       Chunk size: 16
wFormatTag	    2	56       WAVE_FORMAT_PCM (0001)
nChannels	    2	58       Nc
nSamplesPerSec	4	60       F
nAvgBytesPerSec	4	64       F*M*Nc
nBlockAlign	    2	68       M*Nc
wBitsPerSample	2	70       rounds up to 8*M
ckID	        4	72       Chunk ID: 'data'
cksize	        4	76       dummy Chunk size -1 (0xffffffff)
sampled data    ... 80
*/
fn create_rf64_hdr(sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
    let mut hdr = [0u8; 80];
    hdr[0..4].copy_from_slice(b"RF64"); //ChunkId, little endian WAV
    let rf64chunksize: u32 = 0xffff_ffff; // dummy RIFF chunksize
    let datachunksize: u32 = 0xffff_ffff; // dummy data chunksize
    hdr[4..8].copy_from_slice(&rf64chunksize.to_le_bytes()); // RIFF ChunkSize
    hdr[8..12].copy_from_slice(b"WAVE"); // File Format
    write_ds64_chunk(&mut hdr[12..48], bits_per_sample); // SubChunk = ds64
    write_fmt_chunk(&mut hdr[48..72], sample_rate, bits_per_sample); // SubChunk = Format
    hdr[72..76].copy_from_slice(b"data"); // SubChunk = "data"
    hdr[76..80].copy_from_slice(&datachunksize.to_le_bytes()); // data SubChunkSize
    debug!("RF64 Header (l={}): \r\n{:02x?}", hdr.len(), hdr);

    hdr.to_vec()
}

/// Write the 36-byte 'ds64' subchunk (ckID + cksize + the 28-byte RIFF64
/// size fields) used by the RF64 header to carry 64-bit sizes.
fn write_ds64_chunk(buf: &mut [u8], bits_per_sample: u16) {
    debug_assert_eq!(buf.len(), 36);
    let channels: u16 = 2;
    let bytes_per_sample: u16 = bits_per_sample / 8;
    let ds64chunksize: u32 = 28;
    let frame_size: u64 = u64::from(bytes_per_sample) * u64::from(channels);
    let ds64nsamples: u64 = ((i64::MAX / 8) as u64 - 72) / frame_size;
    let ds64datasize: u64 = ds64nsamples * frame_size; // exact multiple of frame_size
    let ds64riffsize: u64 = ds64datasize + 72; // header overhead: WAVE(4)+ds64(36)+fmt(24)+data_hdr(8)
    let ds64tablelength = 0u32;
    buf[0..4].copy_from_slice(b"ds64"); // SubChunk = ds64
    buf[4..8].copy_from_slice(&ds64chunksize.to_le_bytes());
    buf[8..16].copy_from_slice(&ds64riffsize.to_le_bytes());
    buf[16..24].copy_from_slice(&ds64datasize.to_le_bytes());
    buf[24..32].copy_from_slice(&ds64nsamples.to_le_bytes());
    buf[32..36].copy_from_slice(&ds64tablelength.to_le_bytes());
}

/// Write the 24-byte PCM "fmt " subchunk (ckID + cksize + the 16-byte PCM format
/// fields) shared bbetween the WAV and RF64 headers, at different offsets
fn write_fmt_chunk(buf: &mut [u8], sample_rate: u32, bits_per_sample: u16) {
    debug_assert_eq!(buf.len(), 24);
    let channels: u16 = 2;
    let bytes_per_sample: u16 = bits_per_sample / 8;
    let block_align: u16 = channels * bytes_per_sample;
    let byte_rate: u32 = sample_rate * u32::from(block_align);
    buf[0..4].copy_from_slice(b"fmt "); // SubChunk = Format
    buf[4..8].copy_from_slice(&16u32.to_le_bytes()); // fmt chunksize for PCM
    buf[8..10].copy_from_slice(&1u16.to_le_bytes()); // AudioFormat: uncompressed PCM
    buf[10..12].copy_from_slice(&channels.to_le_bytes()); // numchannels 2
    buf[12..16].copy_from_slice(&sample_rate.to_le_bytes()); // SampleRate
    buf[16..20].copy_from_slice(&byte_rate.to_le_bytes()); // ByteRate (Bps)
    buf[20..22].copy_from_slice(&block_align.to_le_bytes()); // BlockAlign
    buf[22..24].copy_from_slice(&bits_per_sample.to_le_bytes()); // BitsPerSample
}

/// Number of interleaved stereo f32 samples corresponding to `ms` milliseconds
/// at `sample_rate` for 2 channels
fn stereo_sample_count(sample_rate: u32, ms: u64) -> usize {
    ((u64::from(sample_rate) * 2 * ms) / 1000) as usize
}

/// Compute how many samples still need to be dropped to achieve the desired
/// latency reduction
fn next_drop_remaining(remaining: usize, chunk_len: usize) -> usize {
    remaining.saturating_sub(chunk_len)
}

/// When latency-reduction is active one sample chunk gets dropped for
/// every DROPS_PER_CYCLE chunks arriving (until the desired latency
/// reduction is achieved)
///
/// Returns `(should_drop_this_chunk, new_remaining, new_drop_streak)`.
fn next_drop_decision(
    remaining: usize,
    chunk_len: usize,
    drop_streak: usize,
) -> (bool, usize, usize) {
    if drop_streak >= ChannelStream::DROPS_PER_CYCLE {
        (false, remaining, 0)
    } else {
        (
            true,
            next_drop_remaining(remaining, chunk_len),
            drop_streak + 1,
        )
    }
}

fn get_silence_buffer(sample_rate: u32, silence_period: u64) -> Vec<f32> {
    // silence_period is in msecs (capture_timeout / 4)
    let size = stereo_sample_count(sample_rate, silence_period);
    let mut silence = Vec::with_capacity(size);
    silence.resize(size, 0f32);
    silence
}

///
/// fill the pre-allocated noise buffer with a very faint white noise (-60db)
///
#[cfg(all(test, debug_assertions))]
fn get_noise_buffer(sample_rate: u32, silence_period: u64) -> Vec<f32> {
    // create the random generator for the white noise
    let mut rng = Rng::with_seed(79);
    let size = ((sample_rate as u64 * 2 * silence_period) / 1000) as usize;
    let mut noise = Vec::with_capacity(size);
    noise.resize(size, 0.0);
    let amplitude: f32 = 0.001;
    for sample in &mut noise {
        *sample = ((rng.f32() * 2.0) - 1.0) * amplitude;
    }
    noise
}

#[cfg(test)]
mod tests {
    use crate::audio::rwstream::*;
    fn u16le(buf: &[u8], off: usize) -> u16 {
        u16::from_le_bytes(buf[off..off + 2].try_into().unwrap())
    }
    fn u32le(buf: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
    }
    fn u64le(buf: &[u8], off: usize) -> u64 {
        u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
    }

    /// Assert the 36-byte 'ds64' subchunk at `off` satisfies the RF64 spec
    /// invariants: datasize is an exact multiple of the frame size, riffsize
    /// equals datasize plus the fixed header overhead, and the (unused) table
    /// length is 0.
    fn assert_ds64_chunk(hdr: &[u8], off: usize, bits_per_sample: u16) {
        let channels: u64 = 2;
        let bytes_per_sample = u64::from(bits_per_sample / 8);
        let frame_size = bytes_per_sample * channels;

        assert_eq!(&hdr[off..off + 4], b"ds64");
        assert_eq!(u32le(hdr, off + 4), 28, "ds64 chunksize");
        let ds64riffsize = u64le(hdr, off + 8);
        let ds64datasize = u64le(hdr, off + 16);
        let ds64nsamples = u64le(hdr, off + 24);
        assert_eq!(u32le(hdr, off + 32), 0, "table length unused");

        // datasize must be an exact multiple of the frame size, and
        // riffsize must equal datasize plus the fixed header overhead
        // (WAVE(4) + ds64(36) + fmt(24) + data-subchunk-header(8) = 72)
        assert_eq!(ds64datasize % frame_size, 0);
        assert_eq!(ds64datasize, ds64nsamples * frame_size);
        assert_eq!(ds64riffsize, ds64datasize + 72);
    }

    /// Assert the 24-byte PCM "fmt " subchunk at `off` matches values derived
    /// independently from the WAV spec, not by re-calling production code.
    fn assert_fmt_chunk(hdr: &[u8], off: usize, sample_rate: u32, bits_per_sample: u16) {
        let channels: u16 = 2;
        let bytes_per_sample = bits_per_sample / 8;
        let expected_block_align = channels * bytes_per_sample;
        let expected_byte_rate = sample_rate * u32::from(expected_block_align);

        assert_eq!(&hdr[off..off + 4], b"fmt ");
        assert_eq!(u32le(hdr, off + 4), 16, "fmt chunksize must be 16 for PCM");
        assert_eq!(u16le(hdr, off + 8), 1, "AudioFormat must be PCM (1)");
        assert_eq!(u16le(hdr, off + 10), channels);
        assert_eq!(u32le(hdr, off + 12), sample_rate);
        assert_eq!(u32le(hdr, off + 16), expected_byte_rate);
        assert_eq!(u16le(hdr, off + 20), expected_block_align);
        assert_eq!(u16le(hdr, off + 22), bits_per_sample);
    }

    #[test]
    fn test_fmt_chunk() {
        for &sample_rate in crate::globals::statics::SAMPLE_RATES {
            for &bits_per_sample in &[16u16, 24] {
                let mut buf = [0u8; 24];
                write_fmt_chunk(&mut buf, sample_rate, bits_per_sample);
                assert_fmt_chunk(&buf, 0, sample_rate, bits_per_sample);
            }
        }
    }

    #[test]
    fn test_wav_hdr() {
        for &sample_rate in crate::globals::statics::SAMPLE_RATES {
            for &bits_per_sample in &[16u16, 24] {
                let hdr = create_wav_hdr(sample_rate, bits_per_sample);
                assert_eq!(hdr.len(), 44);

                assert_eq!(&hdr[0..4], b"RIFF");
                let riffchunksize = u32le(&hdr, 4);
                assert_eq!(riffchunksize, u32::MAX, "streamed WAV declares max size");
                assert_eq!(&hdr[8..12], b"WAVE");

                assert_fmt_chunk(&hdr, 12, sample_rate, bits_per_sample);

                assert_eq!(&hdr[36..40], b"data");
                let datachunksize = u32le(&hdr, 40);
                assert_eq!(datachunksize, riffchunksize - 36);
            }
        }
    }

    #[test]
    fn test_rf64_hdr() {
        for &sample_rate in crate::globals::statics::SAMPLE_RATES {
            for &bits_per_sample in &[16u16, 24] {
                let hdr = create_rf64_hdr(sample_rate, bits_per_sample);
                assert_eq!(hdr.len(), 80);

                assert_eq!(&hdr[0..4], b"RF64");
                assert_eq!(u32le(&hdr, 4), 0xffff_ffff, "dummy RIFF chunksize");
                assert_eq!(&hdr[8..12], b"WAVE");

                assert_ds64_chunk(&hdr, 12, bits_per_sample);

                assert_fmt_chunk(&hdr, 48, sample_rate, bits_per_sample);

                assert_eq!(&hdr[72..76], b"data");
                assert_eq!(u32le(&hdr, 76), 0xffff_ffff, "dummy data chunksize");
            }
        }
    }

    #[test]
    fn test_ds64_chunk() {
        for &bits_per_sample in &[16u16, 24] {
            let mut buf = [0u8; 36];
            write_ds64_chunk(&mut buf, bits_per_sample);
            assert_ds64_chunk(&buf, 0, bits_per_sample);
        }
    }

    #[test]
    fn test_silence() {
        const SAMPLE_RATE: u32 = 44100;
        let sb = get_silence_buffer(SAMPLE_RATE, 250);
        assert_eq!(sb.len(), ((SAMPLE_RATE * 2) as u64 / (1000 / 250)) as usize);
    }

    #[test]
    fn test_stereo_sample_count() {
        assert_eq!(stereo_sample_count(44_100, 1000), 88_200);
        assert_eq!(stereo_sample_count(48_000, 500), 48_000);
        assert_eq!(stereo_sample_count(44_100, 0), 0);
        assert_eq!(stereo_sample_count(0, 1000), 0);
        // non-exact-ms remainders truncate (matches get_silence_buffer's existing behavior)
        assert_eq!(stereo_sample_count(44_100, 1), 88);
    }

    #[test]
    fn test_next_drop_remaining() {
        // chunk smaller than remaining: decreases normally
        assert_eq!(next_drop_remaining(100, 40), 60);
        // chunk exactly exhausts remaining
        assert_eq!(next_drop_remaining(40, 40), 0);
        // chunk larger than remaining: saturates at 0, never underflows
        assert_eq!(next_drop_remaining(30, 40), 0);
        // already zero stays zero
        assert_eq!(next_drop_remaining(0, 40), 0);
    }

    #[test]
    fn test_next_drop_decision_drops_two_then_passes_one() {
        // first eligible chunk (drop_streak starts at 0): dropped
        let (drop1, rem1, streak1) = next_drop_decision(1000, 100, 0);
        assert!(drop1);
        assert_eq!(rem1, 900);
        assert_eq!(streak1, 1);

        // second consecutive chunk: still dropped
        let (drop2, rem2, streak2) = next_drop_decision(rem1, 100, streak1);
        assert!(drop2);
        assert_eq!(rem2, 800);
        assert_eq!(streak2, 2);

        // third chunk: this cycle's pass-through turn — forwarded, remaining untouched
        let (drop3, rem3, streak3) = next_drop_decision(rem2, 100, streak2);
        assert!(!drop3);
        assert_eq!(rem3, 800);
        assert_eq!(streak3, 0);

        // back to a drop turn for the next cycle
        let (drop4, rem4, streak4) = next_drop_decision(rem3, 100, streak3);
        assert!(drop4);
        assert_eq!(rem4, 700);
        assert_eq!(streak4, 1);
    }

    #[test]
    fn test_next_drop_decision_drop_turn_saturates_at_zero() {
        let (should_drop, remaining, drop_streak) = next_drop_decision(30, 100, 0);
        assert!(should_drop);
        assert_eq!(remaining, 0);
        assert_eq!(drop_streak, 1);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_noise() {
        let noise = get_noise_buffer(44100, 250);
        eprintln!("{noise:?}");
    }

    /// Build chunks of the given lengths holding consecutive sample values
    /// 0.0, 1.0, 2.0, ... (exact in f32), so any loss, duplication or
    /// reordering by the cursor shows up as a value mismatch.
    fn numbered_chunks(lens: &[usize]) -> Vec<AudioSamples> {
        let mut next = 0.0f32;
        lens.iter()
            .map(|&len| {
                let chunk: Vec<f32> = (0..len).map(|i| next + i as f32).collect();
                next += len as f32;
                AudioSamples::from(chunk)
            })
            .collect()
    }

    /// bytes per group in the test conversion: 4 samples as u16 LE
    const TEST_GROUP_BYTES: usize = 8;

    /// Test stand-in for the real converters: write each sample as its u16 LE
    /// value, so the bytes in the buffer can be decoded back to the samples.
    fn convert_u16le(out: &mut [u8], groups: &[[f32; 4]]) {
        assert!(!groups.is_empty());
        assert_eq!(out.len(), groups.len() * TEST_GROUP_BYTES);
        let (pairs, _) = out.as_chunks_mut::<2>();
        for (bytes, s) in pairs.iter_mut().zip(groups.as_flattened()) {
            *bytes = (*s as u16).to_le_bytes();
        }
    }

    fn decode_u16le(buf: &[u8]) -> Vec<f32> {
        let (pairs, _) = buf.as_chunks::<2>();
        pairs
            .iter()
            .map(|&b| f32::from(u16::from_le_bytes(b)))
            .collect()
    }

    /// Result of one `SampleCursor::fill` call in `take_sequence`
    #[derive(Debug, PartialEq)]
    struct Taken {
        /// samples decoded from the buffer
        samples: Vec<f32>,
        /// number of `convert` calls (contiguous slices handed out)
        slices: usize,
        /// number of chunks pushed during this call
        pushes: usize,
    }

    /// Feed `chunks` (in order) to a fresh `SampleCursor` through successive
    /// `fill` calls, one per request of `wanted` groups. Panics if more chunks
    /// are needed than provided.
    fn take_sequence(chunks: &[AudioSamples], requests: &[usize]) -> Vec<Taken> {
        let mut cursor = SampleCursor::default();
        let mut source = chunks.iter().cloned();
        requests
            .iter()
            .map(|&wanted| {
                let mut buf = vec![0xAA; wanted * TEST_GROUP_BYTES];
                let mut slices = 0;
                let mut pushes = 0;
                let n = cursor.fill(
                    &mut buf,
                    TEST_GROUP_BYTES,
                    || {
                        pushes += 1;
                        source.next().expect("ran out of chunks")
                    },
                    |out, groups| {
                        slices += 1;
                        convert_u16le(out, groups);
                    },
                );
                assert_eq!(n, buf.len());
                Taken {
                    samples: decode_u16le(&buf),
                    slices,
                    pushes,
                }
            })
            .collect()
    }

    /// concatenate the samples of all `take_sequence` results
    fn all_samples(taken: Vec<Taken>) -> Vec<f32> {
        taken.into_iter().flat_map(|t| t.samples).collect()
    }

    fn seq(range: std::ops::Range<u16>) -> Vec<f32> {
        range.map(f32::from).collect()
    }

    #[test]
    fn test_sample_cursor_aligned_chunks_zero_copy() {
        // aligned chunks are handed out as one slice per chunk, no stitching
        let chunks = numbered_chunks(&[8, 8]);
        let out = take_sequence(&chunks, &[4]);
        assert_eq!(
            out,
            vec![Taken {
                samples: seq(0..16),
                slices: 2,
                pushes: 2
            }]
        );
    }

    #[test]
    fn test_sample_cursor_stitches_group_across_boundary() {
        // 6 = one whole group + 2 leftovers, completed by 2 samples of the next chunk
        let chunks = numbered_chunks(&[6, 6]);
        let out = take_sequence(&chunks, &[3]);
        // slices: [0..4] from chunk 1, stitched head [4..8], [8..12] from chunk 2
        assert_eq!(
            out,
            vec![Taken {
                samples: seq(0..12),
                slices: 3,
                pushes: 2
            }]
        );
    }

    #[test]
    fn test_sample_cursor_stitch_plus_groups_plus_new_carry() {
        // chunk 2 completes the carried group (2 + 2), has 1 whole group of
        // its own, and leaves 1 sample as new carry for chunk 3
        let chunks = numbered_chunks(&[6, 7, 3]);
        let out = take_sequence(&chunks, &[4]);
        // slices: [0..4], head [4..8], [8..12], head [12..16]
        assert_eq!(
            out,
            vec![Taken {
                samples: seq(0..16),
                slices: 4,
                pushes: 3
            }]
        );
    }

    #[test]
    fn test_sample_cursor_stitch_without_groups_leaves_new_carry() {
        // chunk 2 completes the carried group (3 + 1) but its remaining
        // sample is too few for a whole group and becomes the new carry
        let chunks = numbered_chunks(&[3, 2, 3]);
        let out = take_sequence(&chunks, &[2]);
        // slices: head [0..4], head [4..8]
        assert_eq!(
            out,
            vec![Taken {
                samples: seq(0..8),
                slices: 2,
                pushes: 3
            }]
        );
    }

    #[test]
    fn test_sample_cursor_keeps_leftover_across_calls() {
        let chunks = numbered_chunks(&[10, 10]);
        let out = take_sequence(&chunks, &[1, 2, 2]);
        assert_eq!(out[0].samples, seq(0..4));
        assert_eq!(out[1].samples, seq(4..12));
        assert_eq!(out[2].samples, seq(12..20));
        // the 3rd request is served from leftovers only, no new chunk
        assert_eq!(out[2].pushes, 0);
    }

    #[test]
    fn test_sample_cursor_tiny_and_empty_chunks() {
        // groups completed from several chunks shorter than 4, incl. empty ones
        let chunks = numbered_chunks(&[3, 0, 1, 0, 2, 1, 1]);
        let out = take_sequence(&chunks, &[2]);
        // [3] -> carry, [] -> nothing, [1] -> group 0..4, [] -> nothing,
        // [2] -> carry, [1] -> carry, [1] -> group 4..8
        assert_eq!(
            out,
            vec![Taken {
                samples: seq(0..8),
                slices: 2,
                pushes: 7
            }]
        );
    }

    #[test]
    fn test_sample_cursor_zero_groups_needs_no_chunk() {
        // an empty read must not block waiting for a chunk
        let out = take_sequence(&[], &[0]);
        assert_eq!(
            out,
            vec![Taken {
                samples: Vec::new(),
                slices: 0,
                pushes: 0
            }]
        );
    }

    #[test]
    fn test_sample_cursor_fill_leaves_partial_group_tail_untouched() {
        // a buffer that is not a multiple of the group size: only whole
        // groups are written, the tail keeps its contents
        let chunks = numbered_chunks(&[16]);
        let mut cursor = SampleCursor::default();
        let mut source = chunks.into_iter();
        let mut buf = vec![0xAA; 3 * TEST_GROUP_BYTES + 5];
        let n = cursor.fill(
            &mut buf,
            TEST_GROUP_BYTES,
            || source.next().expect("ran out of chunks"),
            convert_u16le,
        );
        assert_eq!(n, 3 * TEST_GROUP_BYTES);
        assert_eq!(decode_u16le(&buf[..n]), seq(0..12));
        assert!(buf[n..].iter().all(|&b| b == 0xAA));
        // the unread 4th group is still there for the next fill
        let mut buf = vec![0; TEST_GROUP_BYTES];
        let n = cursor.fill(&mut buf, TEST_GROUP_BYTES, || unreachable!(), convert_u16le);
        assert_eq!(n, TEST_GROUP_BYTES);
        assert_eq!(decode_u16le(&buf), seq(12..16));
    }

    #[test]
    fn test_sample_cursor_cpal_like_chunk_sizes() {
        // 441 stereo frames = 882 samples, not a multiple of 4
        let chunks = numbered_chunks(&[882; 5]);
        let out = take_sequence(&chunks, &[300, 300, 502]);
        assert_eq!(all_samples(out), seq(0..4408));
    }

    /// Randomized invariant: whatever the chunk lengths and request sizes, the
    /// concatenated output is exactly the concatenated input, in order, and
    /// the groups of a chunk are handed out as one slice (no copy) unless the
    /// request ends inside it.
    #[test]
    fn test_sample_cursor_random_invariant() {
        let mut rng = Rng::with_seed(0x5a17_u64);
        for _ in 0..200 {
            // mostly tiny chunks (lots of stitching), some bigger ones (lots
            // of whole groups per chunk, to catch a regression to one slice
            // per group)
            let lens: Vec<usize> = (0..50)
                .map(|_| {
                    if rng.bool() {
                        rng.usize(0..13)
                    } else {
                        rng.usize(13..41)
                    }
                })
                .collect();
            let total: usize = lens.iter().sum();
            let chunks = numbered_chunks(&lens);
            let mut requests = Vec::new();
            let mut left = total / 4;
            while left > 0 {
                let n = rng.usize(0..=left.min(10));
                requests.push(n);
                left -= n;
            }
            let out = take_sequence(&chunks, &requests);
            for t in &out {
                // each push yields at most a stitched head plus one slice of
                // whole groups; state left over from the previous request can
                // add one more of each
                assert!(
                    t.slices <= 2 * t.pushes + 2,
                    "{} slices for {} pushes",
                    t.slices,
                    t.pushes
                );
            }
            assert_eq!(all_samples(out), seq(0..(total / 4 * 4) as u16));
        }
    }

    /// 2 groups of exactly representable samples, including both clip limits
    const TEST_GROUPS: [[f32; 4]; 2] = [[0.0, 0.5, -0.5, 0.25], [-0.25, 1.0, -1.0, 0.125]];

    fn convert_groups(fmt: LpcmFormat, groups: &[[f32; 4]]) -> Vec<u8> {
        let mut out = vec![0u8; groups.len() * fmt.bytes_per_group()];
        fmt.convert(&mut out, groups);
        out
    }

    /// decode `bytes_per_sample`-wide signed samples in the given byte order
    fn decode_signed(buf: &[u8], bytes_per_sample: usize, endian: Endian) -> Vec<i32> {
        buf.chunks_exact(bytes_per_sample)
            .map(|s| {
                let mut be = s.to_vec();
                if endian == Little {
                    be.reverse();
                }
                // sign-extend from the most significant byte
                be.iter()
                    .skip(1)
                    .fold(i32::from(be[0] as i8), |acc, &b| (acc << 8) | i32::from(b))
            })
            .collect()
    }

    #[test]
    fn test_lpcm_format_bytes_per_group() {
        for endian in [Little, Big] {
            for dither in [Dither, NoDither] {
                let f16 = LpcmFormat {
                    endian,
                    bd: Bits16,
                    dither,
                };
                let f24 = LpcmFormat {
                    endian,
                    bd: Bits24,
                    dither,
                };
                assert_eq!(f16.bytes_per_group(), 8);
                assert_eq!(f24.bytes_per_group(), 12);
            }
        }
    }

    /// every undithered arm must match its reference quantizer/packer pair
    #[test]
    fn test_lpcm_format_convert_matches_reference() {
        type Quantize = fn(f32x4) -> [i32; 4];
        type Pack = fn(&[i32; 4], &mut [u8]);
        type Case = (Endian, BitDepth, Dither, Quantize, Pack);
        let cases: [Case; 4] = [
            (Little, Bits16, NoDither, f32x4_to_i32x4_16, i32_to_i16le),
            (Big, Bits16, NoDither, f32x4_to_i32x4_16, i32_to_i16be),
            (Little, Bits24, NoDither, f32x4_to_i32x4_24, i32_to_i24le),
            (Big, Bits24, NoDither, f32x4_to_i32x4_24, i32_to_i24be),
        ];
        for (endian, bd, dither, quantize, pack) in cases {
            let fmt = LpcmFormat { endian, bd, dither };
            let mut expected = Vec::new();
            for g in &TEST_GROUPS {
                let mut b = vec![0u8; fmt.bytes_per_group()];
                pack(&quantize(f32x4::new(*g)), &mut b);
                expected.extend(b);
            }
            assert_eq!(convert_groups(fmt, &TEST_GROUPS), expected, "{fmt:?}");
        }
    }

    /// 24 bit ignores the dither setting
    #[test]
    fn test_lpcm_format_24bit_never_dithered() {
        for endian in [Little, Big] {
            let d = LpcmFormat {
                endian,
                bd: Bits24,
                dither: Dither,
            };
            let nd = LpcmFormat {
                endian,
                bd: Bits24,
                dither: NoDither,
            };
            assert_eq!(
                convert_groups(d, &TEST_GROUPS),
                convert_groups(nd, &TEST_GROUPS)
            );
        }
    }

    /// little and big endian decode to the same, correctly scaled values
    #[test]
    fn test_lpcm_format_endianness_and_scale() {
        let input = TEST_GROUPS.as_flattened();
        for (bd, bytes, full_scale) in [(Bits16, 2, 32767.0f32), (Bits24, 3, 8_388_607.0)] {
            let le = LpcmFormat {
                endian: Little,
                bd,
                dither: NoDither,
            };
            let be = LpcmFormat {
                endian: Big,
                bd,
                dither: NoDither,
            };
            let le_vals = decode_signed(&convert_groups(le, &TEST_GROUPS), bytes, Little);
            let be_vals = decode_signed(&convert_groups(be, &TEST_GROUPS), bytes, Big);
            assert_eq!(le_vals, be_vals, "{bd:?}");
            for (&got, &s) in le_vals.iter().zip(input) {
                let want = s * full_scale;
                assert!((got as f32 - want).abs() <= 1.0, "{bd:?}: {s} -> {got}");
            }
        }
    }

    /// dithered 16 bit stays within a couple of LSBs of the undithered value
    #[test]
    fn test_lpcm_format_16bit_dither_close_to_undithered() {
        for endian in [Little, Big] {
            let d = LpcmFormat {
                endian,
                bd: Bits16,
                dither: Dither,
            };
            let nd = LpcmFormat {
                endian,
                bd: Bits16,
                dither: NoDither,
            };
            let dv = decode_signed(&convert_groups(d, &TEST_GROUPS), 2, endian);
            let ndv = decode_signed(&convert_groups(nd, &TEST_GROUPS), 2, endian);
            for (a, b) in dv.iter().zip(&ndv) {
                assert!((a - b).abs() <= 2, "{endian:?}: {a} vs {b}");
            }
        }
    }

    #[test]
    fn test_recv_or_silence_returns_chunk() {
        let (tx, rx) = crossbeam_channel::unbounded::<AudioSamples>();
        let silence: AudioSamples = vec![0.0f32; 8].into();
        let chunk: AudioSamples = vec![0.5f32; 4].into();
        tx.send(chunk.clone()).unwrap();
        let got = recv_or_silence(&rx, Duration::from_millis(100), &silence);
        assert!(Arc::ptr_eq(&got, &chunk));
    }

    #[test]
    fn test_recv_or_silence_times_out_to_silence() {
        let (_tx, rx) = crossbeam_channel::unbounded::<AudioSamples>();
        let silence: AudioSamples = vec![0.0f32; 8].into();
        let got = recv_or_silence(&rx, Duration::from_millis(1), &silence);
        assert!(Arc::ptr_eq(&got, &silence));
    }

    #[test]
    fn test_recv_or_silence_disconnected_gives_silence() {
        let (tx, rx) = crossbeam_channel::unbounded::<AudioSamples>();
        drop(tx);
        let silence: AudioSamples = vec![0.0f32; 8].into();
        let got = recv_or_silence(&rx, Duration::from_millis(100), &silence);
        assert!(Arc::ptr_eq(&got, &silence));
    }

    use dasp_sample::{I24, Sample};
    // just to prove that ((i32 >> 8) & 0xffffff) is indeed I24
    #[test]
    fn test_i24() {
        let sample = i32::from_sample(0x12345678i32);
        let i24_sample = I24::from_sample(sample);
        println!("i24: {i24_sample:X?}");
        let f32_sample: f32 = 0.123456;
        let a1 = {
            let i24sample = i32::from_sample(f32_sample) >> 8;
            let b = i24sample.to_le_bytes();
            [b[0], b[1], b[2]]
        };
        let a2 = { &((i32::from_sample(f32_sample) >> 8).to_le_bytes())[..=2] };
        assert_eq!(a1, a2);
        let b1 = {
            let i24sample = i32::from_sample(f32_sample) >> 8;
            let b = i24sample.to_be_bytes();
            [b[1], b[2], b[3]]
        };
        let b2 = { &((i32::from_sample(f32_sample) >> 8).to_be_bytes())[1..] };
        assert_eq!(b1, b2);
    }

    /// Feed `chunks` (in order) to a fresh `ChunkCursor` through successive
    /// `fill` calls with the given buffer sizes, returning what each call
    /// produced. Panics if the cursor asks for more chunks than provided.
    fn fill_sequence(chunks: &[&[u8]], buf_sizes: &[usize]) -> Vec<Vec<u8>> {
        let mut cursor = ChunkCursor::default();
        let mut source = chunks.iter().map(|c| c.to_vec());
        buf_sizes
            .iter()
            .map(|&size| {
                let mut buf = vec![0u8; size];
                let n = cursor
                    .fill(&mut buf, || source.next().ok_or("out of chunks"))
                    .unwrap();
                assert_eq!(n, size);
                buf
            })
            .collect()
    }

    #[test]
    fn test_chunk_cursor_buf_spans_several_chunks() {
        let out = fill_sequence(&[&[1, 2], &[3, 4, 5], &[6]], &[6]);
        assert_eq!(out, vec![vec![1, 2, 3, 4, 5, 6]]);
    }

    #[test]
    fn test_chunk_cursor_chunk_spans_several_bufs() {
        let out = fill_sequence(&[&[1, 2, 3, 4, 5, 6, 7]], &[3, 2, 2]);
        assert_eq!(out, vec![vec![1, 2, 3], vec![4, 5], vec![6, 7]]);
    }

    #[test]
    fn test_chunk_cursor_keeps_leftover_across_calls() {
        // leftover [4, 5] of the first chunk must come out before the next chunk
        let out = fill_sequence(&[&[1, 2, 3, 4, 5], &[6, 7, 8]], &[3, 4, 1]);
        assert_eq!(out, vec![vec![1, 2, 3], vec![4, 5, 6, 7], vec![8]]);
    }

    #[test]
    fn test_chunk_cursor_exact_chunk_boundaries() {
        let out = fill_sequence(&[&[1, 2], &[3, 4]], &[2, 2]);
        assert_eq!(out, vec![vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn test_chunk_cursor_skips_empty_chunks() {
        let out = fill_sequence(&[&[], &[1], &[], &[], &[2, 3]], &[3]);
        assert_eq!(out, vec![vec![1, 2, 3]]);
    }

    #[test]
    fn test_chunk_cursor_empty_buf_needs_no_chunk() {
        // an empty read must not block waiting for a chunk
        let out = fill_sequence(&[], &[0]);
        assert_eq!(out, vec![Vec::<u8>::new()]);
    }

    #[test]
    fn test_chunk_cursor_propagates_source_error() {
        let mut cursor = ChunkCursor::default();
        let mut source = vec![vec![1u8, 2]].into_iter();
        let mut buf = [0u8; 4];
        let result = cursor.fill(&mut buf, || source.next().ok_or("closed"));
        assert_eq!(result, Err("closed"));
    }
}
