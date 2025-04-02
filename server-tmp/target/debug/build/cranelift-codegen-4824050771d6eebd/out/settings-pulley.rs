#[derive(Clone, Hash)]
/// Flags group `pulley`.
pub struct Flags {
    bytes: [u8; 2],
}
impl Flags {
    /// Create flags pulley settings group.
    #[allow(unused_variables)]
    pub fn new(shared: &settings::Flags, builder: &Builder) -> Self {
        let bvec = builder.state_for("pulley");
        let mut pulley = Self { bytes: [0; 2] };
        debug_assert_eq!(bvec.len(), 2);
        pulley.bytes[0..2].copy_from_slice(&bvec);
        pulley
    }
}
impl Flags {
    /// Iterates the setting values.
    pub fn iter(&self) -> impl Iterator<Item = Value> + use<> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(&self.bytes[0..2]);
        DESCRIPTORS.iter().filter_map(move |d| {
            let values = match &d.detail {
                detail::Detail::Preset => return None,
                detail::Detail::Enum { last, enumerators } => Some(TEMPLATE.enums(*last, *enumerators)),
                _ => None
            };
            Some(Value{ name: d.name, detail: d.detail, values, value: bytes[d.offset as usize] })
        })
    }
}
/// Values for `pulley.pointer_width`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PointerWidth {
    /// `pointer32`.
    Pointer32,
    /// `pointer64`.
    Pointer64,
}
impl PointerWidth {
    /// Returns a slice with all possible [PointerWidth] values.
    pub fn all() -> &'static [PointerWidth] {
        &[
            Self::Pointer32,
            Self::Pointer64,
        ]
    }
}
impl fmt::Display for PointerWidth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Self::Pointer32 => "pointer32",
            Self::Pointer64 => "pointer64",
        })
    }
}
impl core::str::FromStr for PointerWidth {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pointer32" => Ok(Self::Pointer32),
            "pointer64" => Ok(Self::Pointer64),
            _ => Err(()),
        }
    }
}
/// User-defined settings.
#[allow(dead_code)]
impl Flags {
    /// Get a view of the boolean predicates.
    pub fn predicate_view(&self) -> crate::settings::PredicateView {
        crate::settings::PredicateView::new(&self.bytes[1..])
    }
    /// Dynamic numbered predicate getter.
    fn numbered_predicate(&self, p: usize) -> bool {
        self.bytes[1 + p / 8] & (1 << (p % 8)) != 0
    }
    /// The width of pointers for this Pulley target
    /// Supported values:
    /// * 'pointer32'
    /// * 'pointer64'
    pub fn pointer_width(&self) -> PointerWidth {
        match self.bytes[0] {
            0 => {
                PointerWidth::Pointer32
            }
            1 => {
                PointerWidth::Pointer64
            }
            _ => {
                panic!("Invalid enum value")
            }
        }
    }
    /// Whether this is a big-endian target
    /// Whether this is a big-endian target
    pub fn big_endian(&self) -> bool {
        self.numbered_predicate(0)
    }
}
static DESCRIPTORS: [detail::Descriptor; 2] = [
    detail::Descriptor {
        name: "pointer_width",
        description: "The width of pointers for this Pulley target",
        offset: 0,
        detail: detail::Detail::Enum { last: 1, enumerators: 0 },
    },
    detail::Descriptor {
        name: "big_endian",
        description: "Whether this is a big-endian target",
        offset: 1,
        detail: detail::Detail::Bool { bit: 0 },
    },
];
static ENUMERATORS: [&str; 2] = [
    "pointer32",
    "pointer64",
];
static HASH_TABLE: [u16; 4] = [
    0xffff,
    0,
    1,
    0xffff,
];
static PRESETS: [(u8, u8); 0] = [
];
static TEMPLATE: detail::Template = detail::Template {
    name: "pulley",
    descriptors: &DESCRIPTORS,
    enumerators: &ENUMERATORS,
    hash_table: &HASH_TABLE,
    defaults: &[0x00, 0x00],
    presets: &PRESETS,
};
/// Create a `settings::Builder` for the pulley settings group.
pub fn builder() -> Builder {
    Builder::new(&TEMPLATE)
}
impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[pulley]")?;
        for d in &DESCRIPTORS {
            if !d.detail.is_preset() {
                write!(f, "{} = ", d.name)?;
                TEMPLATE.format_toml_value(d.detail, self.bytes[d.offset as usize], f)?;
                writeln!(f)?;
            }
        }
        Ok(())
    }
}
