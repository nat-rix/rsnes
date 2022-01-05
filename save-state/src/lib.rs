#[cfg(test)]
mod tests;

pub struct SaveStateSerializer {
    pub data: Vec<u8>,
}

pub struct SaveStateDeserializer<'a> {
    pub data: core::slice::Iter<'a, u8>,
}

impl<'a> SaveStateDeserializer<'a> {
    pub fn consume(&mut self, n: usize) {
        if n > 0 {
            let _ = self.data.nth(n - 1);
        }
    }
}

pub trait InSaveState: Sized {
    fn serialize(&self, state: &mut SaveStateSerializer);
    fn deserialize(&mut self, state: &mut SaveStateDeserializer);
}

macro_rules! impl_for_int {
    ($t:ty) => {
        impl InSaveState for $t {
            fn serialize(&self, state: &mut SaveStateSerializer) {
                state.data.extend_from_slice(&self.to_le_bytes())
            }

            fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
                if state.data.as_slice().len() >= core::mem::size_of::<$t>() {
                    *self = Self::from_le_bytes(state.data.as_slice()[..core::mem::size_of::<$t>()].try_into().unwrap());
                    state.consume(core::mem::size_of::<$t>());
                } else {
                    panic!("not enough data to deserialize")
                }
            }
        }
    };
    () => {};
    ($t1:ty $(,$t:ty)*) => { impl_for_int!($t1); impl_for_int!($($t),*); };
}

impl_for_int! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128 }

const _ASSERT_MAX_USIZE: [(); !(core::mem::size_of::<usize>() <= 8
    && core::mem::size_of::<isize>() <= 8) as usize] = [];

macro_rules! impl_usize_isize {
    ($t:ty, $i:ty) => {
        impl InSaveState for $t {
            fn serialize(&self, state: &mut SaveStateSerializer) {
                (*self as $i).serialize(state)
            }

            fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
                let mut i: $i = 0;
                i.deserialize(state);
                *self = i as $t
            }
        }
    };
}

impl_usize_isize!(usize, u64);
impl_usize_isize!(isize, i64);

fn is_u8_or_i8(v: &(dyn std::any::Any + 'static)) -> bool {
    v.is::<u8>() || v.is::<i8>()
}

impl<const N: usize, T: InSaveState + 'static> InSaveState for [T; N] {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        if is_u8_or_i8(self) {
            let arr: &[u8; N] = unsafe { core::mem::transmute(self) };
            state.data.extend_from_slice(arr)
        } else {
            for i in self.iter() {
                T::serialize(i, state)
            }
        }
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        if is_u8_or_i8(self) {
            if state.data.as_slice().len() >= core::mem::size_of::<[T; N]>() {
                let res: Result<&[u8; N], _> =
                    state.data.as_slice()[..core::mem::size_of::<[T; N]>()].try_into();
                state.consume(N);
                // TODO: use normal transmute instead as soon as possible!!
                // see https://github.com/rust-lang/rust/issues/43408
                // see https://github.com/rust-lang/rust/issues/60471
                *self = unsafe { core::mem::transmute_copy(res.unwrap()) }
            } else {
                panic!("not enough data to deserialize")
            }
        } else {
            self.iter_mut().for_each(|i| i.deserialize(state))
        }
    }
}

impl<T: InSaveState + Copy> InSaveState for core::cell::Cell<T> {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.get().serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        self.get_mut().deserialize(state)
    }
}

// This uses 0 and 255 for false and true. That will make the memory
// representation more robust against memory corruption by random
// bit flips.
impl InSaveState for bool {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        let i: u8 = if *self { 0xff } else { 0 };
        i.serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = i.count_ones() >= 4
    }
}

impl<T: InSaveState + Default> InSaveState for Option<T> {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.is_some().serialize(state);
        match self {
            Some(v) => v.serialize(state),
            // TODO: use extend_from_slice when const generics get stabilized enough
            None => (),
        }
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i = false;
        i.deserialize(state);
        *self = if i {
            let mut i = T::default();
            i.deserialize(state);
            Some(i)
        } else {
            None
        }
    }
}

impl<T1: InSaveState, T2: InSaveState> InSaveState for (T1, T2) {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.0.serialize(state);
        self.1.serialize(state);
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        self.0.deserialize(state);
        self.1.deserialize(state);
    }
}

impl InSaveState for Vec<u8> {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.len().serialize(state);
        state.data.extend_from_slice(self)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut n: usize = 0;
        n.deserialize(state);
        if state.data.as_slice().len() >= n {
            *self = state.data.as_slice()[..n].to_vec();
            state.consume(n);
        } else {
            panic!("not enough data to deserialize")
        }
    }
}

impl InSaveState for String {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        self.len().serialize(state);
        state.data.extend_from_slice(self.as_bytes())
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut n: usize = 0;
        n.deserialize(state);
        if state.data.as_slice().len() >= n {
            *self = core::str::from_utf8(&state.data.as_slice()[..n])
                .unwrap()
                .to_string();
            state.consume(n);
        } else {
            panic!("not enough data to deserialize")
        }
    }
}
