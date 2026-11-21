//! What is written from the base header `sdrkit.h`: the initialisers of the structs that begin with `size`
//! (`sdrkit/init.h`), and the same structs for the Swift file that the family files share.

use std::fmt::Write;

/// A struct of the base header that begins with `uint32_t size`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizedStruct {
    /// The C name: `sdrkit_audio_info`.
    pub name: String,
    /// The C initialiser of each member after `size`: `0`, or `{0}` for a struct or an array.
    pub zeros: Vec<&'static str>,
}

/// The structs of the base header `header` that begin with `uint32_t size`, in the order of the header.
pub fn sized_structs(header: &str) -> Vec<SizedStruct> {
    let mut out = Vec::new();
    let mut lines = header.lines();
    while let Some(line) = lines.next() {
        let Some(name) = line
            .strip_prefix("typedef struct ")
            .and_then(|l| l.strip_suffix(" {"))
        else {
            continue;
        };
        let members: Vec<&str> = lines
            .by_ref()
            .take_while(|l| !l.starts_with('}'))
            .map(str::trim)
            .filter(|l| l.ends_with(';') && !l.starts_with("//"))
            .collect();
        if members.first() != Some(&"uint32_t size;") {
            continue;
        }
        let zeros = members[1..]
            .iter()
            .map(|m| {
                let aggregate = !m.contains('*') && (m.starts_with("struct ") || m.contains('['));
                if aggregate {
                    "{0}"
                } else {
                    "0"
                }
            })
            .collect();
        out.push(SizedStruct {
            name: name.to_owned(),
            zeros,
        });
    }
    out
}

/// The header `sdrkit/init.h`: one `*_INIT` initialiser for each struct of [`sized_structs`].
pub fn render_init(header: &str) -> String {
    let mut out = String::from(
        "/*\n\
         \x20* sdrkit: the initialisers of the structs of sdrkit.h that begin with `size`. MIT licence.\n\
         \x20*\n\
         \x20* Written from sdrkit.h. Do not edit. sdrkit.h includes this file.\n\
         \x20*\n\
         \x20* Each initialiser sets `size` to the sizeof of the struct and every other member to 0. Declare every such\n\
         \x20* struct with it, a struct that a call fills too:\n\
         \x20*\n\
         \x20*     sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;\n\
         \x20*\n\
         \x20* A call returns SDRKIT_E_BAD_ARGUMENT for a struct whose `size` is below 4.\n\
         \x20*/\n\n\
         #ifndef SDRKIT_INIT_H\n#define SDRKIT_INIT_H\n\n",
    );
    for s in sized_structs(header) {
        let zeros: String = s.zeros.iter().flat_map(|z| [", ", z]).collect();
        let _ = writeln!(
            out,
            "#define {}_INIT {{sizeof({}){zeros}}}",
            s.name.to_uppercase(),
            s.name
        );
    }
    out.push_str("\n#endif\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sized_struct_has_an_initialiser_with_one_value_for_each_member() {
        let header = "typedef struct sdrkit_string {\n  const char *ptr;\n  size_t len;\n} sdrkit_string;\n\
                      // A.\ntypedef struct sdrkit_a_info {\n  // The size.\n  uint32_t size;\n  struct sdrkit_string key;\n  \
                      const struct sdrkit_arg *settings;\n  uint8_t fourcc[4];\n  double x;\n} sdrkit_a_info;\n";
        let all = sized_structs(header);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].zeros, ["{0}", "0", "{0}", "0"]);
        assert!(render_init(header)
            .contains("#define SDRKIT_A_INFO_INIT {sizeof(sdrkit_a_info), {0}, 0, {0}, 0}\n"));
    }
}
