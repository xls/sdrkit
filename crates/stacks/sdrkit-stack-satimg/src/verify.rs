//! What this crate registers beside its descriptors: a verification case for each stack, and
//! the component types.

use crate::components::instruments::{AvhrrComponent, MsuMrComponent, NoaaComponent};
use crate::components::link::{CodeComponent, DemodComponent, FrameComponent, FrameLink};
use crate::modes;
use crate::typed::{EodbFields, Fy2SvissrFields, Fy3Fields, MeteorLrptFields, RdasFields};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::{field, keys, Field};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{Results, Value};
use sdrkit_proto_ccsds::codesync::CodeConfig;

/// Adds the components that can be made without arguments to `r`, each under the name its `info()` returns.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("satimg.psk_demod", || Box::new(DemodComponent::default()));
    r.add("satimg.ccsds_decoder", || {
        Box::new(CodeComponent::new(CodeConfig::LEO))
    });
    r.add("satimg.frame_sync", || {
        Box::new(FrameComponent::new(FrameLink::LEO_DUAL))
    });
    r.add("satimg.lrpt_deinterleaver", || {
        Box::new(modes::meteor::DeinterleaverComponent::default())
    });
    r.add("satimg.msumr", || Box::new(MsuMrComponent::default()));
    r.add("satimg.avhrr", || Box::new(AvhrrComponent::default()));
    r.add("satimg.noaa_hrpt", || Box::new(NoaaComponent::default()));
    r.add("satimg.fy3_rails", || {
        Box::new(modes::fy3::RailComponent::default())
    });
    r.add("satimg.apt", || {
        Box::new(modes::apt::AptComponent::default())
    });
    r.add("satimg.fsk_demod", || {
        Box::new(crate::components::fsk::FskDemodComponent::default())
    });
    r.add("satimg.pm_demod", || {
        Box::new(crate::components::pm::PmDemodComponent::default())
    });
    r.add("satimg.tip_hirs", || {
        Box::new(modes::dsb::TipComponent::default())
    });
    r.add("satimg.gvar", || {
        Box::new(modes::gvar::GvarComponent::default())
    });
    r.add("satimg.aws_frames", || {
        Box::new(modes::aws::AwsComponent::default())
    });
    r.add("satimg.gac", || {
        Box::new(modes::gac::GacComponent::default())
    });
    r.add("satimg.msumr_hrpt", || {
        Box::new(modes::hrpt_meteor::MsuHrptComponent::default())
    });
    r.add("satimg.seviri", || {
        Box::new(modes::msg::MsgComponent::default())
    });
    r.add("satimg.frame_table", || {
        Box::new(crate::components::table::FrameTableComponent::new(
            modes::goes_cda::CdaLink::R40.table(),
        ))
    });
    r.add("satimg.svissr", || {
        Box::new(modes::fy2::SvissrComponent::default())
    });
}

/// A check that the count field `f` is at least `min`.
pub fn at_least(results: &Results, f: &Field<field::I64>, min: i64) -> CheckLine {
    count_line(f.key, results.fields().get(f), min)
}

/// A check that the count `c` of a result set of the description is at least `min`.
pub fn count_at_least(results: &Results, c: &Column<field::I64>, min: i64) -> CheckLine {
    count_line(c.key, results.fields().at(c), min)
}

fn count_line(key: &str, v: Option<i64>, min: i64) -> CheckLine {
    CheckLine::new(
        key,
        v.is_some_and(|v| v >= min),
        format!("{v:?}, want at least {min}"),
    )
}

/// Entry `key` of catalogue setting `s`, as a case setting.
pub fn entry(s: &CatalogueSetting, key: &str) -> CaseSetting {
    let i = (s.choices)()
        .iter()
        .position(|c| c.key == key)
        .unwrap_or_default();
    CaseSetting {
        key: s.key,
        value: Value::Choice(u16::try_from(i).unwrap_or_default()),
    }
}

/// One case: the test signal at the defaults of the stack with `settings`, run for `seconds`, then `check`.
pub fn case(
    o: &VerifyOptions,
    name: &str,
    settings: &[CaseSetting],
    seconds: f64,
    check: fn(&Results) -> Vec<CheckLine>,
) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(name, o, 0.0);
    c.seconds = c.seconds.max(seconds);
    c.expect_audio = Some(false);
    c.check = Some(check);
    c.settings.extend_from_slice(settings);
    vec![c]
}

pub(crate) static METEOR_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(
            o,
            "test signal, OQPSK 72k",
            &[CaseSetting::new(
                &modes::meteor::MODULATION_CHOICE,
                modes::meteor::ModulationChoice::Oqpsk72,
            )],
            8.0,
            |r| {
                vec![
                    at_least(r, &keys::FRAMES_OK, 10),
                    count_at_least(r, &MeteorLrptFields::PICTURE_ROWS, 2),
                ]
            },
        )
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static HRPT_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(
            o,
            "test signal, MetOp AHRPT",
            &[CaseSetting::new(
                &modes::hrpt::VARIANT_CHOICE,
                modes::hrpt::VariantChoice::Metop,
            )],
            1.0,
            |r| {
                vec![
                    at_least(r, &keys::FRAMES_OK, 20),
                    count_at_least(r, &crate::typed::HrptFields::AVHRR_LINES, 2),
                ]
            },
        )
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static FY3_AHRPT_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(
            o,
            "test signal, FY-3C VIRR",
            &[entry(&modes::fy3::AHRPT_PRESET, "fy3c")],
            1.5,
            |r| {
                vec![
                    at_least(r, &keys::FRAMES_OK, 20),
                    count_at_least(r, &Fy3Fields::PICTURE_ROWS, 2),
                ]
            },
        )
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static FY3_MPT_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(
            o,
            "test signal, MPT MERSI",
            &[entry(&modes::fy3::MPT_PRESET, "mpt_c")],
            1.0,
            |r| vec![at_least(r, &keys::FRAMES_OK, 20)],
        )
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static FY2_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(o, "test signal, S-VISSR", &[], 9.0, |r| {
            vec![count_at_least(r, &Fy2SvissrFields::LINES, 8)]
        })
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static GRB_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(o, "test signal, QPSK 9/10", &[], 1.0, |r| {
            vec![
                count_at_least(r, &crate::typed::CaduFields::SPACE_PACKETS, 50),
                count_at_least(r, &crate::typed::GrbFields::PICTURE_BLOCKS, 3),
                count_at_least(r, &crate::typed::GrbFields::FILES, 1),
            ]
        })
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static UHRIT_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        case(o, "test signal, 8PSK 2/3", &[], 0.6, |r| {
            vec![
                count_at_least(r, &crate::typed::CaduFields::SPACE_PACKETS, 3),
                count_at_least(r, &crate::typed::UhritFields::FILES, 2),
            ]
        })
    },
    vector_settings: None,
    check_vector: None,
};
/// Tunes the cases of a stack that holds no frequency to the lowest of its range: the test signal sits wherever the radio
/// is tuned, and a tuned session asks the radio for the stack's rate.
fn test_tuning(cases: &mut [VerifyCase], d: &sdrkit_core::StackDescriptor) {
    for c in cases {
        c.freq_hz.get_or_insert(d.tuning.min_hz);
    }
}

pub(crate) static FILECAST_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        let mut v = case(o, "test signal, HimawariCast", &[], 4.0, |r| {
            vec![
                count_at_least(r, &crate::typed::FilecastFields::FILES_COMPLETE, 11),
                count_at_least(r, &crate::typed::FilecastFields::PICTURES_FINISHED, 1),
            ]
        });
        v[0].submode = Some(modes::filecast::HIMAWARICAST.into());
        test_tuning(&mut v, &modes::filecast::DESCRIPTOR);
        v
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static RDAS_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        let mut v = case(o, "test signal, Elektro-L", &[], 0.5, |r| {
            vec![
                at_least(r, &keys::FRAMES_OK, 400),
                count_at_least(r, &RdasFields::PACKETS, 50),
                count_at_least(r, &RdasFields::LINES_IR, 1),
            ]
        });
        v[0].submode = Some(modes::rdas::ELEKTRO_L.into());
        test_tuning(&mut v, &modes::rdas::DESCRIPTOR);
        v
    },
    vector_settings: None,
    check_vector: None,
};
pub(crate) static EODB_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        let mut v = case(o, "test signal, SARAL L band", &[], 1.0, |r| {
            vec![at_least(r, &keys::FRAMES_OK, 5)]
        });
        v[0].submode = Some("saral_l".into());
        let mut w = case(o, "test signal, DMSP OLS", &[], 2.0, |r| {
            vec![count_at_least(r, &EodbFields::LINES, 10)]
        });
        w[0].submode = Some("dmsp_rtd".into());
        v.append(&mut w);
        test_tuning(&mut v, &modes::eodb::DESCRIPTOR);
        v
    },
    vector_settings: None,
    check_vector: None,
};
