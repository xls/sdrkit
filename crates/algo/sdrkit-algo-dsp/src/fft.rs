//! FFTs: the [`FftBackend`] family with its registry [`BACKENDS`], and [`Fft`], a planned transform made by the selected backend.
//!
//! The plain backend is `rustfft` (any size, NEON on ARM, AVX on x86). On Apple platforms the `accelerate` backend (vDSP,
//! powers of two up to 2^20) comes first. Every transform is unnormalised (a forward then inverse transform scales by `len`),
//! runs in place, keeps its scratch and allocates nothing per call.

use crate::Complex32;
use sdrkit_core::backend::{Backend, Registry};
use std::sync::Arc;

/// A transform of one size and direction, planned in advance, with its own scratch.
pub trait FftPlan: Send {
    /// The size.
    fn len(&self) -> usize;
    /// No points (never, for a planned transform).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Transforms `buf` (exactly `len()` samples) in place, unnormalised.
    fn process(&mut self, buf: &mut [Complex32]);
    /// A copy with its own scratch.
    fn box_clone(&self) -> Box<dyn FftPlan>;
}

/// A backend of the FFT family: it makes [`FftPlan`]s.
///
/// A minimal backend, a direct DFT:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_dsp::fft::{check_backend, FftBackend, FftPlan};
/// use sdrkit_algo_dsp::Complex32;
///
/// #[derive(Clone)]
/// struct Dft { sign: f32, out: Vec<Complex32> }
///
/// impl FftPlan for Dft {
///     fn len(&self) -> usize { self.out.len() }
///     fn process(&mut self, buf: &mut [Complex32]) {
///         let n = buf.len();
///         for (k, o) in self.out.iter_mut().enumerate() {
///             *o = buf.iter().enumerate().map(|(i, x)| {
///                 x * Complex32::from_polar(1.0, self.sign * std::f32::consts::TAU * ((i * k) % n) as f32 / n as f32)
///             }).sum();
///         }
///         buf.copy_from_slice(&self.out);
///     }
///     fn box_clone(&self) -> Box<dyn FftPlan> { Box::new(self.clone()) }
/// }
///
/// struct DftBackend;
/// impl Backend for DftBackend {
///     fn name(&self) -> &'static str { "dft" }
/// }
/// impl FftBackend for DftBackend {
///     fn plan(&self, n: usize, inverse: bool) -> Option<Box<dyn FftPlan>> {
///         let sign = if inverse { 1.0 } else { -1.0 };
///         (n > 0).then(|| Box::new(Dft { sign, out: vec![Complex32::default(); n] }) as Box<dyn FftPlan>)
///     }
/// }
///
/// check_backend(&DftBackend);
/// ```
pub trait FftBackend: Backend {
    /// A plan of `n` points, forward or inverse. `None` when the backend cannot do `n` points or fails to start.
    fn plan(&self, n: usize, inverse: bool) -> Option<Box<dyn FftPlan>>;
}

/// The FFT backends of this platform, the default first and `rustfft` last.
pub static BACKENDS: Registry<dyn FftBackend> = Registry::new(TABLE);

#[cfg(target_vendor = "apple")]
const TABLE: &[&dyn FftBackend] = &[&accelerate::Accelerate, &RustFft];
#[cfg(not(target_vendor = "apple"))]
const TABLE: &[&dyn FftBackend] = &[&RustFft];

/// Makes sure that `backend` agrees with the plain backend: forward and inverse transforms of 2 to 4096 points (powers of two,
/// and 3, 12, 100 and 1000 where the backend plans them) of a fixed random signal. The largest error of a bin must stay
/// below `1e-5 * n` times the largest input magnitude. A backend that plans none of the powers of two fails.
///
/// # Panics
/// When a transform differs by more than the tolerance, or when the backend plans no power of two.
pub fn check_backend(backend: &dyn FftBackend) {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut planned = 0;
    for n in [2usize, 4, 8, 16, 64, 256, 1024, 4096, 3, 12, 100, 1000] {
        let x: Vec<Complex32> = (0..n)
            .map(|_| {
                let mut u = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5
                };
                Complex32::new(u(), u())
            })
            .collect();
        for inverse in [false, true] {
            let Some(plan) = backend.plan(n, inverse) else {
                continue;
            };
            assert_eq!(plan.len(), n, "{} plans {n} points", backend.name());
            planned += usize::from(n.is_power_of_two());
            let mut want = x.clone();
            RustFft
                .plan(n, inverse)
                .expect("rustfft plans every size")
                .process(&mut want);
            let mut got = x.clone();
            plan.box_clone().process(&mut got);
            let err = got
                .iter()
                .zip(&want)
                .map(|(a, b)| (a - b).norm())
                .fold(0.0f32, f32::max);
            assert!(
                err <= 1e-5 * n as f32,
                "{} {n} points, inverse {inverse}: error {err}",
                backend.name()
            );
        }
    }
    assert!(planned > 0, "{} plans no power of two", backend.name());
}

/// A planned FFT of one size and direction. Unnormalised (a forward then inverse transform scales by `len`).
pub struct Fft {
    plan: Box<dyn FftPlan>,
    backend: &'static str,
}

impl Clone for Fft {
    fn clone(&self) -> Self {
        Fft {
            plan: self.plan.box_clone(),
            backend: self.backend,
        }
    }
}

impl std::fmt::Debug for Fft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fft")
            .field("len", &self.len())
            .field("backend", &self.backend)
            .finish()
    }
}

impl Fft {
    /// A forward transform of `n` points, made by the first backend of [`BACKENDS`] that plans it.
    ///
    /// # Panics
    /// If `n` is 0.
    pub fn forward(n: usize) -> Self {
        Self::planned(n, false)
    }

    /// An inverse transform of `n` points, made by the first backend of [`BACKENDS`] that plans it.
    ///
    /// # Panics
    /// If `n` is 0.
    pub fn inverse(n: usize) -> Self {
        Self::planned(n, true)
    }

    fn planned(n: usize, inverse: bool) -> Self {
        BACKENDS
            .build(|b| Self::with_backend(n, inverse, b))
            .unwrap_or_else(|| panic!("no FFT backend plans {n} points"))
    }

    /// A transform made by `backend`. `None` when the backend cannot do `n` points.
    pub fn with_backend(n: usize, inverse: bool, backend: &dyn FftBackend) -> Option<Self> {
        Some(Fft {
            plan: backend.plan(n, inverse)?,
            backend: backend.name(),
        })
    }

    /// The size.
    pub fn len(&self) -> usize {
        self.plan.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The name of the backend that made the transform.
    pub fn backend(&self) -> &'static str {
        self.backend
    }

    /// Transforms `buf` in place.
    ///
    /// # Panics
    /// If `buf.len()` is not `len()`.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        assert_eq!(buf.len(), self.len(), "FFT buffer length");
        self.plan.process(buf);
    }
}

/// The plain backend: the `rustfft` crate.
struct RustFft;

impl Backend for RustFft {
    fn name(&self) -> &'static str {
        "rustfft"
    }
}

impl FftBackend for RustFft {
    fn plan(&self, n: usize, inverse: bool) -> Option<Box<dyn FftPlan>> {
        if n == 0 {
            return None;
        }
        let mut planner = rustfft::FftPlanner::new();
        let plan = if inverse {
            planner.plan_fft_inverse(n)
        } else {
            planner.plan_fft_forward(n)
        };
        Some(Box::new(RustFftPlan::new(plan)))
    }
}

/// A `rustfft` plan and its scratch.
#[derive(Clone)]
struct RustFftPlan {
    plan: Arc<dyn rustfft::Fft<f32>>,
    scratch: Vec<Complex32>,
}

impl RustFftPlan {
    fn new(plan: Arc<dyn rustfft::Fft<f32>>) -> Self {
        let scratch = vec![Complex32::default(); plan.get_inplace_scratch_len()];
        RustFftPlan { plan, scratch }
    }
}

impl FftPlan for RustFftPlan {
    fn len(&self) -> usize {
        self.plan.len()
    }
    fn process(&mut self, buf: &mut [Complex32]) {
        self.plan.process_with_scratch(buf, &mut self.scratch);
    }
    fn box_clone(&self) -> Box<dyn FftPlan> {
        Box::new(self.clone())
    }
}

/// A planner that shares `rustfft` plans (twiddles) between transforms of the same size.
pub struct FftPlans {
    planner: rustfft::FftPlanner<f32>,
}

impl Default for FftPlans {
    fn default() -> Self {
        Self::new()
    }
}

impl FftPlans {
    /// An empty planner.
    pub fn new() -> Self {
        FftPlans {
            planner: rustfft::FftPlanner::new(),
        }
    }

    fn make(&mut self, n: usize, inverse: bool) -> Fft {
        BACKENDS
            .build(|b| {
                if !BACKENDS.is_plain(b) {
                    return Fft::with_backend(n, inverse, b);
                }
                if n == 0 {
                    return None;
                }
                let plan = if inverse {
                    self.planner.plan_fft_inverse(n)
                } else {
                    self.planner.plan_fft_forward(n)
                };
                Some(Fft {
                    plan: Box::new(RustFftPlan::new(plan)),
                    backend: b.name(),
                })
            })
            .unwrap_or_else(|| panic!("no FFT backend plans {n} points"))
    }

    /// A forward transform of `n` points, as [`Fft::forward`], with a shared plan when the plain backend makes it.
    ///
    /// # Panics
    /// If `n` is 0.
    pub fn forward(&mut self, n: usize) -> Fft {
        self.make(n, false)
    }

    /// An inverse transform of `n` points, as [`Fft::inverse`], with a shared plan when the plain backend makes it.
    ///
    /// # Panics
    /// If `n` is 0.
    pub fn inverse(&mut self, n: usize) -> Fft {
        self.make(n, true)
    }
}

/// Swaps the halves of a spectrum so DC moves from index 0 to the middle (index `n / 2`), ordering bins from lowest to highest frequency.
pub fn fft_shift<T>(buf: &mut [T]) {
    let h = buf.len() / 2;
    buf.rotate_left(buf.len() - h);
}

/// A forward FFT of `n` points of the newest `len` samples (`len <= n`), windowed and zero-padded, that keeps the complex bins. The
/// padding makes the bin spacing finer than that of the window length. The bins are in FFT order: 0 Hz first, the negative
/// frequencies in the upper half.
///
/// ```
/// use sdrkit_algo_dsp::fft::WindowedFft;
/// use sdrkit_algo_dsp::window::Window;
/// use sdrkit_algo_dsp::Complex32;
/// let mut f = WindowedFft::new(8, 4, Window::Rectangular).unwrap();
/// assert!((f.run(&[Complex32::new(1.0, 0.0); 4])[0].re - 4.0).abs() < 1e-6);
/// assert!((f.run_power(&[Complex32::new(1.0, 0.0); 4])[0] - 16.0).abs() < 1e-4);
/// ```
pub struct WindowedFft {
    fft: Fft,
    window: Vec<f32>,
    buf: Vec<Complex32>,
    power: Vec<f32>,
}

impl std::fmt::Debug for WindowedFft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WindowedFft({} of {})",
            self.window.len(),
            self.buf.len()
        )
    }
}

impl WindowedFft {
    /// An `n`-point transform with a periodic `window` of `len` points. Errors when `len` is 0 or more than `n`.
    pub fn new(
        n: usize,
        len: usize,
        window: crate::window::Window,
    ) -> Result<Self, crate::DspError> {
        if len == 0 || len > n {
            return crate::Param::Window.range(1.0, n as f64, len as f64);
        }
        Ok(WindowedFft {
            fft: Fft::forward(n),
            window: window.coefficients(len),
            buf: vec![Complex32::default(); n],
            power: vec![0.0; n],
        })
    }

    /// The transform size `n`.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether the transform size is 0 (never).
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The window.
    pub fn window(&self) -> &[f32] {
        &self.window
    }

    /// The window, to replace with another shape of the same length.
    pub fn window_mut(&mut self) -> &mut [f32] {
        &mut self.window
    }

    /// Transforms the newest window-length samples of `x` (its end), windowed and zero-padded to `n`, and returns the bins. If `x`
    /// is shorter than the window, the missing older samples count as zero.
    pub fn run(&mut self, x: &[Complex32]) -> &[Complex32] {
        let w = self.window.len();
        let take = x.len().min(w);
        let x = &x[x.len() - take..];
        let gap = w - take;
        self.buf.fill(Complex32::default());
        for ((b, &v), &g) in self.buf[gap..w].iter_mut().zip(x).zip(&self.window[gap..]) {
            *b = v * g;
        }
        self.fft.process(&mut self.buf);
        &self.buf
    }

    /// As [`WindowedFft::run`], and also fills [`WindowedFft::power`] with `|X[k]|^2`. Returns the power.
    pub fn run_power(&mut self, x: &[Complex32]) -> &[f32] {
        self.run(x);
        for (p, b) in self.power.iter_mut().zip(&self.buf) {
            *p = b.norm_sqr();
        }
        &self.power
    }

    /// The bins of the last transform.
    pub fn bins(&self) -> &[Complex32] {
        &self.buf
    }

    /// The power of each bin of the last [`WindowedFft::run_power`].
    pub fn power(&self) -> &[f32] {
        &self.power
    }
}

#[cfg(target_vendor = "apple")]
mod accelerate {
    //! The vDSP backend: `vDSP_fft_zip` on split-complex buffers, with `vDSP_ctoz` / `vDSP_ztoc` to and from interleaved samples.
    //! The functions are declared here and linked from the Accelerate framework; nothing else is needed.

    use super::{Backend, FftBackend, FftPlan};
    use crate::Complex32;
    use std::os::raw::{c_int, c_long, c_ulong, c_void};
    use std::sync::Arc;

    #[repr(C)]
    struct DspSplitComplex {
        realp: *mut f32,
        imagp: *mut f32,
    }

    const FFT_RADIX2: c_int = 0;
    const FFT_FORWARD: c_int = 1;
    const FFT_INVERSE: c_int = -1;
    const MAX_LOG2: u32 = 20;

    #[link(name = "Accelerate", kind = "framework")]
    extern "C" {
        fn vDSP_create_fftsetup(log2n: c_ulong, radix: c_int) -> *mut c_void;
        fn vDSP_destroy_fftsetup(setup: *mut c_void);
        fn vDSP_fft_zip(
            setup: *mut c_void,
            c: *const DspSplitComplex,
            stride: c_long,
            log2n: c_ulong,
            direction: c_int,
        );
        fn vDSP_ctoz(
            c: *const Complex32,
            ic: c_long,
            z: *const DspSplitComplex,
            iz: c_long,
            n: c_ulong,
        );
        fn vDSP_ztoc(
            z: *const DspSplitComplex,
            iz: c_long,
            c: *mut Complex32,
            ic: c_long,
            n: c_ulong,
        );
    }

    /// An FFT setup (twiddle tables), destroyed when the last user drops it.
    struct Setup(*mut c_void);

    // SAFETY: a vDSP FFT setup is read-only once created; Apple documents that one setup may be used by several FFT calls at the
    // same time from different threads. It is destroyed only in `Drop`, when no one else holds it.
    unsafe impl Send for Setup {}
    unsafe impl Sync for Setup {}

    impl Drop for Setup {
        fn drop(&mut self) {
            // SAFETY: the pointer came from `vDSP_create_fftsetup`, is not null, and this is its only release.
            unsafe { vDSP_destroy_fftsetup(self.0) }
        }
    }

    fn supports(n: usize) -> bool {
        n.is_power_of_two() && n >= 2 && n.trailing_zeros() <= MAX_LOG2
    }

    /// A split-complex half, 64-byte aligned: vDSP takes different (equally correct, but not bit-identical) paths for different
    /// alignments, and the output must not depend on where an allocation lands.
    struct Aligned {
        v: Vec<f32>,
        off: usize,
        n: usize,
    }

    impl Aligned {
        fn new(n: usize) -> Self {
            let v = vec![0.0f32; n + 16];
            let off = (16 - (v.as_ptr() as usize % 64) / 4) % 16;
            Aligned { v, off, n }
        }
        fn ptr(&mut self) -> *mut f32 {
            self.v[self.off..self.off + self.n].as_mut_ptr()
        }
    }

    struct Engine {
        setup: Arc<Setup>,
        log2n: u32,
        inverse: bool,
        re: Aligned,
        im: Aligned,
    }

    /// The vDSP backend.
    pub(super) struct Accelerate;

    impl Backend for Accelerate {
        fn name(&self) -> &'static str {
            "accelerate"
        }
    }

    impl FftBackend for Accelerate {
        fn plan(&self, n: usize, inverse: bool) -> Option<Box<dyn FftPlan>> {
            if !supports(n) {
                return None;
            }
            let log2n = n.trailing_zeros();
            // SAFETY: plain call with a valid size (checked by `supports`). A null result means no plan.
            let p = unsafe { vDSP_create_fftsetup(log2n as c_ulong, FFT_RADIX2) };
            if p.is_null() {
                return None;
            }
            Some(Box::new(Engine {
                setup: Arc::new(Setup(p)),
                log2n,
                inverse,
                re: Aligned::new(n),
                im: Aligned::new(n),
            }))
        }
    }

    impl FftPlan for Engine {
        fn len(&self) -> usize {
            1 << self.log2n
        }
        fn process(&mut self, buf: &mut [Complex32]) {
            let n = self.len();
            assert!(buf.len() == n && self.re.n == n && self.im.n == n);
            let z = DspSplitComplex {
                realp: self.re.ptr(),
                imagp: self.im.ptr(),
            };
            let dir = if self.inverse {
                FFT_INVERSE
            } else {
                FFT_FORWARD
            };
            // SAFETY: `buf`, `re` and `im` each hold exactly `n` values (asserted; `Aligned::ptr` points at `n` values), `Complex32` is `#[repr(C)]` {re, im} like vDSP's
            // DSPComplex, the setup was made for this `log2n` and lives as long as `self`, and the buffers do not overlap.
            unsafe {
                vDSP_ctoz(buf.as_ptr(), 2, &z, 1, n as c_ulong);
                vDSP_fft_zip(self.setup.0, &z, 1, self.log2n as c_ulong, dir);
                vDSP_ztoc(&z, 1, buf.as_mut_ptr(), 2, n as c_ulong);
            }
        }
        fn box_clone(&self) -> Box<dyn FftPlan> {
            Box::new(Engine {
                setup: self.setup.clone(),
                log2n: self.log2n,
                inverse: self.inverse,
                re: Aligned::new(self.re.n),
                im: Aligned::new(self.im.n),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{ctone, Rng};

    fn backends() -> impl Iterator<Item = &'static dyn FftBackend> {
        BACKENDS.all().iter().copied()
    }

    /// A direct DFT, for reference.
    fn dft(x: &[Complex32], inverse: bool) -> Vec<Complex32> {
        let n = x.len();
        let sign = if inverse { 1.0 } else { -1.0 };
        (0..n)
            .map(|k| {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (i, v) in x.iter().enumerate() {
                    let p = sign * std::f64::consts::TAU * ((i * k) % n) as f64 / n as f64;
                    let (s, c) = p.sin_cos();
                    re += v.re as f64 * c - v.im as f64 * s;
                    im += v.re as f64 * s + v.im as f64 * c;
                }
                Complex32::new(re as f32, im as f32)
            })
            .collect()
    }

    #[test]
    fn tone_lands_in_its_bin_and_round_trips() {
        for n in [256usize, 1024, 4096] {
            for b in backends() {
                let x = ctone(n, 37.0 / n as f64, 1.0);
                let mut y = x.clone();
                let mut f = Fft::with_backend(n, false, b).unwrap();
                f.process(&mut y);
                let name = b.name();
                assert!(
                    (y[37].norm() - n as f32).abs() < 0.01 * n as f32 / 1024.0 + 0.01,
                    "{name} {n}"
                );
                let mut i = Fft::with_backend(n, true, b).unwrap().clone();
                i.process(&mut y);
                for (a, c) in x.iter().zip(&y) {
                    assert!((a - c / n as f32).norm() < 1e-4, "{name} {n}");
                }
            }
        }
        let mut v = vec![0, 1, 2, 3, 4, 5, 6, 7];
        fft_shift(&mut v);
        assert_eq!(v, vec![4, 5, 6, 7, 0, 1, 2, 3]);
    }

    #[test]
    fn every_backend_matches_a_direct_dft() {
        let mut rng = Rng(77);
        for n in [2usize, 8, 64, 512] {
            let x: Vec<Complex32> = (0..n)
                .map(|_| Complex32::new(rng.uniform() as f32, rng.uniform() as f32))
                .collect();
            for inverse in [false, true] {
                let want = dft(&x, inverse);
                for b in backends() {
                    let mut y = x.clone();
                    let Some(mut f) = Fft::with_backend(n, inverse, b) else {
                        continue;
                    };
                    f.process(&mut y);
                    let err = y
                        .iter()
                        .zip(&want)
                        .map(|(a, w)| (a - w).norm())
                        .fold(0.0f32, f32::max);
                    assert!(
                        err < 1e-4 * n as f32,
                        "{} {n} inverse {inverse}: error {err}",
                        b.name()
                    );
                }
            }
        }
    }

    #[test]
    fn the_output_does_not_depend_on_the_engine_or_its_allocations() {
        let n = 1024;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::new((i as f32 * 0.37).sin(), (i as f32 * 0.11).cos()))
            .collect();
        for b in backends() {
            let mut first = x.clone();
            Fft::with_backend(n, false, b).unwrap().process(&mut first);
            let mut keep = Vec::new();
            for k in 0..200 {
                keep.push(vec![0u8; 1 + k * 13 % 97]);
                let mut y = x.clone();
                Fft::with_backend(n, false, b).unwrap().process(&mut y);
                assert_eq!(y, first, "{} plan {k}", b.name());
            }
        }
    }

    #[test]
    fn the_best_backend_is_chosen() {
        let f = Fft::forward(1024);
        assert_eq!(f.backend(), BACKENDS.active());
        assert_eq!(BACKENDS.plain().name(), "rustfft");
        if cfg!(target_vendor = "apple") {
            assert_eq!(f.backend(), "accelerate");
        }
        // A size vDSP's radix-2 transform does not do falls back to rustfft.
        assert_eq!(Fft::forward(1000).backend(), "rustfft");
        let p = FftPlans::new().inverse(1000);
        assert_eq!((p.len(), p.backend()), (1000, "rustfft"));
    }

    #[test]
    fn every_backend_agrees_with_the_plain_one() {
        for b in backends() {
            check_backend(b);
        }
    }

    #[test]
    fn the_windowed_fft_pads_with_zeros_and_keeps_the_bins() {
        use crate::window::Window;
        assert!(WindowedFft::new(8, 0, Window::Hann).is_err());
        assert!(WindowedFft::new(8, 9, Window::Hann).is_err());
        // Eight samples of a tone at 1/8 cycle per sample, zero-padded to 32: the tone falls on bin 4, with the window's sum.
        let x: Vec<Complex32> = (0..20)
            .map(|i| Complex32::from_polar(1.0, (std::f64::consts::TAU * i as f64 / 8.0) as f32))
            .collect();
        let mut f = WindowedFft::new(32, 8, Window::Rectangular).unwrap();
        let tail = &x[12..];
        let bins = f.run(&x).to_vec();
        // Against a direct DFT of the newest 8 samples padded to 32.
        let mut padded = tail.to_vec();
        padded.resize(32, Complex32::default());
        for (a, b) in bins.iter().zip(dft(&padded, false)) {
            assert!((a - b).norm() < 1e-4);
        }
        assert!((bins[4] - tail[0] * 8.0).norm() < 1e-4);
        let p = f.run_power(&x)[4];
        assert!((p - 64.0).abs() < 1e-3);
        assert_eq!(f.power()[4], p);
        // Fewer samples than the window: the missing older ones count as zero.
        let short = f.run(&x[..2]).to_vec();
        padded.fill(Complex32::default());
        padded[6..8].copy_from_slice(&x[..2]);
        for (a, b) in short.iter().zip(dft(&padded, false)) {
            assert!((a - b).norm() < 1e-4);
        }
        // A Hann window is the periodic one.
        let h = WindowedFft::new(16, 8, Window::Hann).unwrap();
        assert_eq!(h.window(), Window::Hann.coefficients(8).as_slice());
    }
}
