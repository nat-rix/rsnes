use super::*;

#[test]
pub fn test_serialize_i8_array() {
    let mut i = 0u32;
    let a = [0i8; 2050].map(|_| {
        i += 1;
        (i & 0xff) as i8
    });
    let mut s = SaveStateSerializer { data: vec![] };
    a.serialize(&mut s);
    for (i, v) in s.data.iter().enumerate() {
        assert_eq!(((i + 1) & 0xff) as i8, *v as i8)
    }
    let mut d = SaveStateDeserializer {
        data: s.data.iter(),
    };
    let mut res = [0i8; 2050];
    res.deserialize(&mut d);
    for (i, v) in res.iter().enumerate() {
        assert_eq!(((i + 1) & 0xff) as i8, *v)
    }
    assert!(d.data.as_slice().is_empty());
}

macro_rules! test_serialize_int {
    ($t:ty, $iter:expr) => {{
        let mut s = SaveStateSerializer {
            data: Vec::with_capacity(core::mem::size_of::<$t>()),
        };
        for i in $iter {
            i.serialize(&mut s);
            assert_eq!(s.data.as_slice(), i.to_le_bytes().as_slice());
            let mut d = SaveStateDeserializer {
                data: s.data.iter(),
            };
            let mut v: $t = 0;
            v.deserialize(&mut d);
            assert_eq!(i, v);
            assert!(d.data.as_slice().is_empty());
            s.data.clear();
        }
    }};
}

#[test]
pub fn test_serialize_u8() {
    test_serialize_int!(u8, 0..=0xffu8)
}

#[test]
pub fn test_serialize_i8() {
    test_serialize_int!(i8, -0x80..=0x7fi8)
}

#[test]
pub fn test_serialize_u16() {
    test_serialize_int!(u16, 0..=0xffffu16)
}

#[test]
pub fn test_serialize_i16() {
    test_serialize_int!(i16, -0x8000..=0x7fffi16)
}

#[test]
pub fn test_serialize_u32() {
    test_serialize_int!(
        u32,
        (0..=0x11111u32)
            .zip((0..=0x11111u32).map(|i| (i * 17) + (i ^ 20)))
            .map(|(a, b)| ((a & b) ^ b) * 7)
    )
}

#[test]
pub fn test_serialize_i32() {
    test_serialize_int!(
        i32,
        (0..=0x11111u32)
            .zip((0..=0x11111u32).map(|i| (i * 17) + (i ^ 20)))
            .map(|(a, b)| ((a & b) ^ b) * 7)
            .map(|v| v as i32 - 0x400000)
    )
}

fn generate_u64_random_seq() -> impl Iterator<Item = u64> {
    (0..=0x11111u64)
        .zip((0..=0x11111u64).map(|i| ((i * 170) + (i ^ 0x25e123)) * 0x4127))
        .map(|(a, b)| ((((a * 0x939) & (b * 0xf77)) ^ b) * 0x7f3b) * (a >> (8 + (b & 4))))
}

#[test]
pub fn test_serialize_u64() {
    test_serialize_int!(u64, generate_u64_random_seq())
}

#[test]
pub fn test_serialize_i64() {
    test_serialize_int!(i64, generate_u64_random_seq().map(|i| i as i64))
}

#[test]
pub fn test_serialize_u128() {
    test_serialize_int!(u128, generate_u64_random_seq().map(|i| u128::from(i)))
}

#[test]
pub fn test_serialize_i128() {
    test_serialize_int!(i128, generate_u64_random_seq().map(|i| i128::from(i)))
}
