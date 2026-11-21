//! Connections by handle and port index, and why a connection is refused ([`ConnectError`]).

use crate::adapt::Catalog;
use crate::component::{CentreRule, ChannelRule, FormatConstraint, KindRule, RateRule};
use crate::graph::Side;
use crate::media::MediaSpec;
use crate::stream::{BitOrder, Format, ItemType, Kind, SampleType};

/// A component of a [`GraphPlan`](crate::GraphPlan), by the order it was added in. [`ComponentHandle::INPUT`] is the graph's
/// own `Iq` input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentHandle(pub u32);

impl ComponentHandle {
    /// The graph's own `Iq` input. It has one output, port 0.
    pub const INPUT: ComponentHandle = ComponentHandle(u32::MAX);

    /// The index of the component in the plan. `None` for [`ComponentHandle::INPUT`].
    pub fn index(self) -> Option<usize> {
        (self != Self::INPUT).then_some(self.0 as usize)
    }
}

/// One port of one component: the handle and the index of the port on its side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortRef {
    /// The component.
    pub component: ComponentHandle,
    /// The index of the port among the component's inputs or outputs.
    pub port: usize,
}

impl PortRef {
    /// Port `port` of `component`.
    pub fn new(component: ComponentHandle, port: usize) -> Self {
        PortRef { component, port }
    }
    /// The output of the graph's own input.
    pub const INPUT: PortRef = PortRef {
        component: ComponentHandle::INPUT,
        port: 0,
    };
}

impl std::fmt::Display for PortRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.component.index() {
            Some(i) => write!(f, "component {i} port {}", self.port),
            None => f.write_str("the graph input"),
        }
    }
}

/// Why a connection is refused. Each variant names the output (`from`) and the input (`to`) and what each side has.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ConnectError {
    /// The rate of the output does not fit the input.
    #[error("{from} runs at {rate} Hz, {to} takes {accepts:?}")]
    RateMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The rate of the output, in Hz.
        rate: f64,
        /// The rates of the input.
        accepts: RateRule,
    },
    /// The channel count of the output does not fit the input.
    #[error("{from} has {channels} channels, {to} takes {accepts:?}")]
    ChannelMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The channels of the output.
        channels: u16,
        /// The channel counts of the input.
        accepts: ChannelRule,
    },
    /// The output has another radio frequency at 0 Hz than the input needs, or the frequency lies outside the capture.
    #[error("{from} is centred on {centre_hz} Hz, {to} takes {accepts:?}")]
    CentreMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The centre of the output, in Hz.
        centre_hz: f64,
        /// The centre the input needs.
        accepts: CentreRule,
    },
    /// The rate of the output does not fit the channel width of the input.
    #[error("{from} runs at {rate} Hz, {to} takes a channel {width} Hz wide")]
    WidthMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The rate of the output, in Hz.
        rate: f64,
        /// The width the input needs, in Hz.
        width: f64,
    },
    /// The soft bits of the output have another scale than the input needs.
    #[error("{from} gives soft bits of scale {scale:?}, {to} takes {accepts}")]
    LlrScaleMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The scale of the output.
        scale: Option<f32>,
        /// The scale of the input.
        accepts: f32,
    },
    /// The bytes of the output pack bits in another order than the input needs.
    #[error("{from} packs bits {order:?}, {to} takes {accepts:?}")]
    BitOrderMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The bit order of the output.
        order: Option<BitOrder>,
        /// The bit order of the input.
        accepts: BitOrder,
    },
    /// The sample type of the output does not fit the input.
    #[error("{from} gives samples of type {sample_type:?}, {to} takes {accepts:?}")]
    SampleTypeMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The sample type of the output.
        sample_type: Option<SampleType>,
        /// The sample type of the input.
        accepts: SampleType,
    },
    /// The codecs that the output produces are not codecs that the input takes.
    #[error("{from} gives media {produces:?}, {to} takes {accepts:?}")]
    CodecMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The codecs of the output.
        produces: Vec<MediaSpec>,
        /// The codecs of the input.
        accepts: Vec<MediaSpec>,
    },
    /// Two typed streams of different item types.
    #[error("{from} gives {} ({}), {to} takes {accepts}", .produces.key(), .produces.rust_name())]
    ItemTypeMismatch {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The item type of the output.
        produces: ItemType,
        /// The kinds of the input.
        accepts: KindRule,
    },
    /// No conversion can join the output to the input (audio into a video decoder).
    #[error("{from} gives {produces}, {to} takes {}, and no conversion joins them", .accepts.kind)]
    NoConversion {
        /// The output.
        from: PortRef,
        /// The input.
        to: PortRef,
        /// The kind of the output.
        produces: Kind,
        /// The format rule of the input.
        accepts: Box<FormatConstraint>,
    },
    /// The input is connected already.
    #[error("{to} is connected already")]
    PortTaken {
        /// The input.
        to: PortRef,
    },
    /// The component has no such port.
    #[error("component {component:?} has no {side} {port}")]
    NoSuchPort {
        /// The component.
        component: ComponentHandle,
        /// Inputs or outputs.
        side: Side,
        /// The requested port index.
        port: usize,
    },
    /// The plan has no such component.
    #[error("there is no component {component:?}")]
    NoSuchComponent {
        /// The requested handle.
        component: ComponentHandle,
    },
    /// The graph's own input cannot feed a feedback edge.
    #[error("the graph input cannot feed {to} as feedback")]
    InputAsFeedback {
        /// The input it was to feed.
        to: PortRef,
    },
}

/// Checks an output of kind `produces` against input rule `accepts`, as far as kinds and media go. A kind that `accepts`
/// does not name passes when a chain of `catalog` joins them, item types included. `port` is the declaration of the output
/// when it is known. A `Bytes` output joins another kind only through an entry that its declaration serves (raw samples
/// need a declared sample type, packed bits a declared bit order).
pub(crate) fn check_kind(
    catalog: &Catalog,
    from: PortRef,
    to: PortRef,
    produces: Kind,
    port: Option<&FormatConstraint>,
    accepts: &FormatConstraint,
) -> Result<(), ConnectError> {
    let produces_media = port.map_or(&[][..], |p| &p.media[..]);
    let kinds: &[Kind] = match &accepts.kind {
        KindRule::Any => return Ok(()),
        KindRule::Is(k) => std::slice::from_ref(k),
        KindRule::OneOf(ks) => ks,
        KindRule::SameAs(_) => &[],
    };
    if kinds.contains(&produces) {
        // Another codec or pixel layout passes when the catalog converts within the kind (a decoder, a pixel converter).
        let ok = produces_media.is_empty()
            || accepts.media.is_empty()
            || produces_media
                .iter()
                .any(|p| accepts.media.iter().any(|a| p.fits(a)))
            || catalog.converts_within(produces, accepts);
        return if ok {
            Ok(())
        } else {
            Err(ConnectError::CodecMismatch {
                from,
                to,
                produces: produces_media.to_vec(),
                accepts: accepts.media.clone(),
            })
        };
    }
    let declared = port.filter(|_| produces == Kind::Bytes).map(|p| Format {
        sample_type: p.sample_type,
        bit_order: p.bit_order,
        ..Format::new(Kind::Bytes, 0.0)
    });
    if catalog.joins(produces, declared.as_ref(), accepts) {
        return Ok(());
    }
    if let Kind::Item(t) = produces {
        if kinds.iter().any(|k| matches!(k, Kind::Item(_))) {
            return Err(ConnectError::ItemTypeMismatch {
                from,
                to,
                produces: t,
                accepts: accepts.kind.clone(),
            });
        }
    }
    Err(ConnectError::NoConversion {
        from,
        to,
        produces,
        accepts: Box::new(accepts.clone()),
    })
}

/// Checks the rate, the channels and the sample type of a known output format against input rule `accepts`.
pub(crate) fn check_format(
    from: PortRef,
    to: PortRef,
    f: &crate::stream::Format,
    accepts: &FormatConstraint,
) -> Result<(), ConnectError> {
    if !accepts.rate.accepts(f.rate) {
        return Err(ConnectError::RateMismatch {
            from,
            to,
            rate: f.rate,
            accepts: accepts.rate,
        });
    }
    if !accepts.channels.accepts(f.channels) {
        return Err(ConnectError::ChannelMismatch {
            from,
            to,
            channels: f.channels,
            accepts: accepts.channels,
        });
    }
    if !accepts.width_accepts(f.rate) {
        return Err(ConnectError::WidthMismatch {
            from,
            to,
            rate: f.rate,
            width: accepts.width.unwrap_or_default(),
        });
    }
    if let Some(centre) = accepts.centre.filter(|c| !c.fits(f)) {
        return Err(ConnectError::CentreMismatch {
            from,
            to,
            centre_hz: f.centre_hz,
            accepts: centre,
        });
    }
    if let Some(scale) = accepts
        .llr_scale
        .filter(|s| f.llr_scale.is_none_or(|g| (g - s).abs() > 1e-6 * s.abs()))
    {
        return Err(ConnectError::LlrScaleMismatch {
            from,
            to,
            scale: f.llr_scale,
            accepts: scale,
        });
    }
    if let Some(order) = accepts.bit_order.filter(|o| f.bit_order != Some(*o)) {
        return Err(ConnectError::BitOrderMismatch {
            from,
            to,
            order: f.bit_order,
            accepts: order,
        });
    }
    if let Some(t) = accepts.sample_type {
        if f.sample_type != Some(t) {
            return Err(ConnectError::SampleTypeMismatch {
                from,
                to,
                sample_type: f.sample_type,
                accepts: t,
            });
        }
    }
    Ok(())
}
