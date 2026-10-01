use core::mem::transmute;

#[derive(Debug, Clone, Copy)]
pub enum AnyAmlOp {
    AmlOp(AmlOp),
    Ext(ExtOp),
}

use crate::acpi::aml::AmlError;
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AmlOp {
    ZeroOp = 0x00,
    OneOp = 0x01,

    AliasOp = 0x06,
    NameOp = 0x08,
    ByteOp = 0x0A,
    WordOp = 0x0B,
    DWordOp = 0x0C,
    StringOp = 0x0D,
    QWordOp = 0x0E,

    ScopeOp = 0x10,
    BufferOp = 0x11,
    PackageOp = 0x12,
    VarPackageOp = 0x13,
    MethodOp = 0x14,
    ExternalOp = 0x15,
    DualNameOp = b'.',
    MultiNameOp = b'/',

    ExtOpPrefix = 0x5B,
    ParentPrefixChar = b'^',
    MethodInvoke = b'\\',

    Local0 = 0x60,
    Local1 = 0x61,
    Local2 = 0x62,
    Local3 = 0x63,
    Local4 = 0x64,
    Local5 = 0x65,
    Local6 = 0x66,
    Local7 = 0x67,

    Arg0 = 0x68,
    Arg1 = 0x69,
    Arg2 = 0x6A,
    Arg3 = 0x6B,
    Arg4 = 0x6C,
    Arg5 = 0x6D,
    Arg6 = 0x6E,

    StoreOp = 0x70,
    RefOfOp = 0x71,
    AddOp = 0x72,
    ConcatOp = 0x73,
    SubtractOp = 0x74,
    IncrementOp = 0x75,
    DecrementOp = 0x76,
    MultiplyOp = 0x77,
    DivideOp = 0x78,
    ShiftLeftOp = 0x79,
    ShiftRightOp = 0x7A,
    AndOp = 0x7B,
    NandOp = 0x7C,
    OrOp = 0x7D,
    NorOp = 0x7E,
    XorOp = 0x7F,
    NotOp = 0x80,
    FindSetLeftBitOp = 0x81,
    FindSetRightBitOp = 0x82,
    DerefOfOp = 0x83,
    ConcatResOp = 0x84,
    ModOp = 0x85,
    NotifyOp = 0x86,
    SizeOfOp = 0x87,
    IndexOp = 0x88,
    MatchOp = 0x89,
    CreateDwordFieldOp = 0x8A,
    CreateWordFieldOp = 0x8B,
    CreateByteFieldOp = 0x8C,
    CreateBitFieldOp = 0x8D,
    ObjectTypeOp = 0x8E,
    CreateQWordFieldOp = 0x8F,
    LandOp = 0x90,
    LorOp = 0x91,
    LnotOp = 0x92,
    LequalOp = 0x93,
    LgreaterOp = 0x94,
    LLlessOp = 0x95,
    ToBufferOp = 0x96,
    ToDecimalStringOp = 0x97,
    ToHexStringOp = 0x98,
    ToIntegerOp = 0x99,
    ToStringOp = 0x9C,
    CopyObjectOp = 0x9D,
    MidOp = 0x9E,
    ContinueOp = 0x9F,

    IfOp = 0xA0,
    ElseOp = 0xA1,
    WhileOp = 0xA2,
    NoopOp = 0xA3,
    ReturnOp = 0xA4,
    BreakOp = 0xA5,
    BreakPointOp = 0xCC,

    DigitChar,
    NameChar = b'_',

    OnesOp = 0xFF,
}

impl TryFrom<u8> for AmlOp {
    type Error = AmlError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            b'0'..=b'9' => Ok(Self::DigitChar),
            b'A'..=b'Z' => Ok(Self::NameChar),
            0x2..=0x05
            | 0x07
            | 0x09
            | 0x0F
            | 0x16..=0x2D
            | 0x30..=0x5A
            | 0x5D
            | 0x6F
            | 0x9A..=0x9B
            | 0xA6..=0xCB
            | 0xCD..=0xFE => Err(AmlError::InvalidOperand(value)),
            _ => Ok(unsafe { transmute::<u8, Self>(value) }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtOp {
    MutexOp = 0x01,
    EventOp = 0x02,

    CondRefOfOp = 0x12,
    CreateFieldOp = 0x13,

    LoadTableOp = 0x1F,
    LoadOp = 0x20,
    StallOp = 0x21,
    SleepOp = 0x22,
    AquireOp = 0x23,
    SignalOp = 0x24,
    WaitOp = 0x25,
    ResetOp = 0x26,
    ReleaseOp = 0x27,
    FromBCDOp = 0x28,
    ToBCDOp = 0x29,
    UnloadOp = 0x2A,

    RevisionOp = 0x30,
    DebugOp = 0x31,
    FatalOp = 0x32,
    TimeOp = 0x33,

    OpRegionOp = 0x80,
    FieldOp = 0x81,
    DeviceOpList = 0x82,
    ProcessorOp = 0x83,
    PowerResOp = 0x84,
    ThermalZoneOpList = 0x85,
    IndexFieldOp = 0x86,
    BankFieldOp = 0x87,
    DataRegionOp = 0x88,
}

impl TryFrom<u8> for ExtOp {
    type Error = AmlError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 | 0x03..=0x11 | 0x14..=0x1E | 0x2B..=0x2F | 0x34..=0x7F | 0x88.. => {
                Err(AmlError::InvalidExtOperand(value))
            }
            _ => Ok(unsafe { transmute::<u8, Self>(value) }),
        }
    }
}

#[test_case]
fn op_coverage() {
    for i in 0..=u8::MAX {
        ExtOp::try_from(i);
    }

    for i in 0..=u8::MAX {
        AmlOp::try_from(i);
    }
}
