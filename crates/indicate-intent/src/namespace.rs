//! Intent namespaces, classes, and bounded extension payloads (INT-01).

use crate::wire::IntentWireError;

/// Maximum byte length of a custom extension intent payload.
pub const MAX_EXTENSION_PAYLOAD: usize = 64;

/// 64-bit domain namespace partitioning core avionics intents from extension intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct IntentNamespace(pub u64);

impl IntentNamespace {
    /// Reserved core namespace (`0`) owned by `indicate-intent`.
    pub const CORE: Self = Self(0);

    /// Whether this namespace is [`IntentNamespace::CORE`].
    #[must_use]
    pub const fn is_core(self) -> bool {
        self.0 == 0
    }
}

/// Computes a deterministic 64-bit FNV-1a [`IntentNamespace`] from `domain`.
///
/// A hash output of `0` is mapped to `1` so [`IntentNamespace::CORE`] (`0`)
/// remains exclusively reserved for built-in core intents.
#[must_use]
pub const fn namespace_id(domain: &str) -> IntentNamespace {
    let bytes = domain.as_bytes();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut idx = 0;
    while idx < bytes.len() {
        hash ^= bytes[idx] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        idx += 1;
    }
    if hash == 0 {
        IntentNamespace(1)
    } else {
        IntentNamespace(hash)
    }
}

/// Core opcode constants inside [`IntentNamespace::CORE`].
pub mod core_opcode {
    /// Core opcode for `Intent::SelectBaro`.
    pub const SELECT_BARO: u16 = 0x0001;
    /// Core opcode for `Intent::SetHeadingBug`.
    pub const SET_HEADING_BUG: u16 = 0x0002;
    /// Core opcode for `Intent::SelectAltitude`.
    pub const SELECT_ALTITUDE: u16 = 0x0003;
    /// Core opcode for `Intent::SelectNavSource`.
    pub const SELECT_NAV_SOURCE: u16 = 0x0004;
    /// Core opcode for `Intent::AcknowledgeAlert`.
    pub const ACKNOWLEDGE_ALERT: u16 = 0x0005;
    /// Core opcode for `Intent::AcknowledgeAllAlerts`.
    pub const ACKNOWLEDGE_ALL_ALERTS: u16 = 0x0006;
    /// Core opcode for `Intent::SelectPage`.
    pub const SELECT_PAGE: u16 = 0x0007;
    /// Core opcode for `Intent::Declutter`.
    pub const DECLUTTER: u16 = 0x0008;
    /// Core opcode for `Intent::PressSoftkey`.
    pub const PRESS_SOFTKEY: u16 = 0x0009;
}

/// Declarative descriptor key naming one intent opcode or wildcard in a namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntentClass {
    /// Owning namespace (`0` for core intents).
    pub namespace: IntentNamespace,
    /// Opcode within `namespace`, or [`IntentClass::ANY_OPCODE`].
    pub opcode: u16,
}

impl IntentClass {
    /// Wildcard opcode matching every opcode in `namespace`.
    pub const ANY_OPCODE: u16 = 0xFFFF;

    /// Core barometric-reference selection (`0x0001`).
    pub const SELECT_BARO: Self = Self::core(core_opcode::SELECT_BARO);
    /// Core heading-bug selection (`0x0002`).
    pub const SET_HEADING_BUG: Self = Self::core(core_opcode::SET_HEADING_BUG);
    /// Core altitude selection (`0x0003`).
    pub const SELECT_ALTITUDE: Self = Self::core(core_opcode::SELECT_ALTITUDE);
    /// Core lateral navigation-source selection (`0x0004`).
    pub const SELECT_NAV_SOURCE: Self = Self::core(core_opcode::SELECT_NAV_SOURCE);
    /// Core single-alert acknowledgement (`0x0005`).
    pub const ACKNOWLEDGE_ALERT: Self = Self::core(core_opcode::ACKNOWLEDGE_ALERT);
    /// Core master-alert acknowledgement (`0x0006`).
    pub const ACKNOWLEDGE_ALL_ALERTS: Self = Self::core(core_opcode::ACKNOWLEDGE_ALL_ALERTS);
    /// Core display-page selection (`0x0007`).
    pub const SELECT_PAGE: Self = Self::core(core_opcode::SELECT_PAGE);
    /// Core declutter toggle (`0x0008`).
    pub const DECLUTTER: Self = Self::core(core_opcode::DECLUTTER);
    /// Core softkey activation (`0x0009`).
    pub const PRESS_SOFTKEY: Self = Self::core(core_opcode::PRESS_SOFTKEY);

    /// Constructs a core [`IntentClass`] for `opcode`.
    #[must_use]
    pub const fn core(opcode: u16) -> Self {
        Self {
            namespace: IntentNamespace::CORE,
            opcode,
        }
    }

    /// Constructs a custom [`IntentClass`] for `(namespace, opcode)`.
    #[must_use]
    pub const fn custom(namespace: IntentNamespace, opcode: u16) -> Self {
        Self { namespace, opcode }
    }

    /// Whether this declared class accepts `actual`.
    #[must_use]
    pub const fn matches(self, actual: Self) -> bool {
        self.namespace.0 == actual.namespace.0
            && (self.opcode == Self::ANY_OPCODE || self.opcode == actual.opcode)
    }

    /// Human-readable label for core opcodes, if known.
    #[must_use]
    pub const fn core_name(self) -> Option<&'static str> {
        if !self.namespace.is_core() {
            return None;
        }
        match self.opcode {
            core_opcode::SELECT_BARO => Some("SelectBaro"),
            core_opcode::SET_HEADING_BUG => Some("SetHeadingBug"),
            core_opcode::SELECT_ALTITUDE => Some("SelectAltitude"),
            core_opcode::SELECT_NAV_SOURCE => Some("SelectNavSource"),
            core_opcode::ACKNOWLEDGE_ALERT => Some("AcknowledgeAlert"),
            core_opcode::ACKNOWLEDGE_ALL_ALERTS => Some("AcknowledgeAllAlerts"),
            core_opcode::SELECT_PAGE => Some("SelectPage"),
            core_opcode::DECLUTTER => Some("Declutter"),
            core_opcode::PRESS_SOFTKEY => Some("PressSoftkey"),
            Self::ANY_OPCODE => Some("Core(*)"),
            _ => None,
        }
    }
}

/// Fixed-capacity inline byte payload (`<= 64` bytes) for [`crate::Intent::Custom`].
#[derive(Debug, Clone, Copy)]
pub struct ExtensionPayload {
    /// Active byte count (`0..=64`).
    pub len: u8,
    /// Backing storage; bytes at `len..` are zeroed by [`ExtensionPayload::new`].
    pub bytes: [u8; MAX_EXTENSION_PAYLOAD],
}

impl ExtensionPayload {
    /// Empty zero-byte extension payload.
    pub const EMPTY: Self = Self {
        len: 0,
        bytes: [0u8; MAX_EXTENSION_PAYLOAD],
    };

    /// Copies `slice` into a bounded [`ExtensionPayload`], failing if longer than 64 bytes.
    pub fn new(slice: &[u8]) -> Result<Self, IntentWireError> {
        if slice.len() > MAX_EXTENSION_PAYLOAD {
            return Err(IntentWireError::PayloadTooLarge { len: slice.len() });
        }
        let mut bytes = [0u8; MAX_EXTENSION_PAYLOAD];
        bytes[..slice.len()].copy_from_slice(slice);
        let len = u8::try_from(slice.len())
            .map_err(|_| IntentWireError::PayloadTooLarge { len: slice.len() })?;
        Ok(Self { len, bytes })
    }

    /// Active payload slice, clamped to [`MAX_EXTENSION_PAYLOAD`] if `len` was mutated.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        let clamped = usize::from(self.len).min(MAX_EXTENSION_PAYLOAD);
        &self.bytes[..clamped]
    }
}

impl Default for ExtensionPayload {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl PartialEq for ExtensionPayload {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl Eq for ExtensionPayload {}
