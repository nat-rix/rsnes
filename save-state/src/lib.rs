pub struct SaveStateSerializer {
    data: Vec<u8>,
}

pub struct SaveStateDeserializer<'a> {
    data: core::slice::Iter<'a, u8>,
}

pub trait InSaveState: Sized {
    fn serialize(&self, state: &mut SaveStateSerializer);
    fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self>;
}

macro_rules! impl_for_int {
    ($t:ty) => {
        impl InSaveState for $t {
            fn serialize(&self, state: &mut SaveStateSerializer) {
                state.data.extend_from_slice(&self.to_le_bytes())
            }

            fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
                if state.data.as_slice().len() >= core::mem::size_of::<$t>() {
                    Some(Self::from_le_bytes(state.data.as_slice()[..core::mem::size_of::<$t>()].try_into().unwrap()))
                } else {
                    None
                }
            }
        }
    };
    () => {};
    ($t1:ty $(,$t:ty)*) => { impl_for_int!($t1); impl_for_int!($($t),*); };
}

impl_for_int! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128 }

macro_rules! impl_for_u8_i8_array {
    ($t:ty) => {
        impl<const N: usize> InSaveState for [$t; N] {
            fn serialize(&self, state: &mut SaveStateSerializer) {
                let arr: &[u8; N] = unsafe { core::mem::transmute(self) };
                state.data.extend_from_slice(arr)
            }

            fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
                if state.data.as_slice().len() >= core::mem::size_of::<[$t; N]>() {
                    let res: Result<&[u8; N], _> =
                        state.data.as_slice()[..core::mem::size_of::<[$t; N]>()].try_into();
                    // TODO: use normal transmute instead as soon as possible!!
                    // see https://github.com/rust-lang/rust/issues/43408
                    // see https://github.com/rust-lang/rust/issues/60471
                    Some(unsafe { core::mem::transmute_copy(res.unwrap()) })
                } else {
                    None
                }
            }
        }
    };
}

impl_for_u8_i8_array!(u8);
impl_for_u8_i8_array!(i8);

impl<const N: usize> InSaveState for [u16; N] {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        for i in self.into_iter() {
            i.serialize(state)
        }
    }

    fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
        // TODO: simplify when array_try_map feature is stabilized
        let mut arr = [0; N];
        for i in 0..N {
            arr[i] = u16::deserialize(state)?
        }
        Some(arr)
    }
}

impl<T: InSaveState + Copy> InSaveState for core::cell::Cell<T> {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.get().serialize(state)
    }

    fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
        T::deserialize(state).map(core::cell::Cell::new)
    }
}

// This uses 0 and 255 for false and true. That will make the memory
// representation more robust against memory corruption by random
// bit flips.
impl InSaveState for bool {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (if *self { 0xff } else { 0u8 }).serialize(state)
    }

    fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
        Some(u8::deserialize(state)?.count_ones() >= 4)
    }
}

impl<T: InSaveState + Copy> InSaveState for Option<T> {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.is_some().serialize(state);
        match self {
            Some(v) => v.serialize(state),
            // TODO: use extend_from_slice when const generics get stabilized enough
            None => (),
        }
    }

    fn deserialize(state: &mut SaveStateDeserializer) -> Option<Self> {
        Some(if bool::deserialize(state)? {
            Some(T::deserialize(state)?)
        } else {
            None
        })
    }
}
