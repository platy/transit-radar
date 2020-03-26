use core::ops::{Index, IndexMut, Range};
use core::slice::{SliceIndex};

/// # TODO
/// compile time checking of whether id is related to this arena - dont use usize
/// link vecs instead of resizing to avoid copies

pub struct Arena<T> {
  vec: Vec<T>,
}

impl<T> Arena<T> {
  pub fn with_capacity(capacity: usize) -> Arena<T> {
    Arena {
      vec: Vec::with_capacity(capacity)
    }
  }

  pub fn alloc(&mut self, el: T) -> usize {
    self.vec.push(el);
    self.len() - 1
  }

  pub fn alloc_extend<I>(&mut self, iterable: I) -> Range<usize>
  where
    I: IntoIterator<Item = T>,
  {
    let start = self.vec.len();
    self.vec.extend(iterable);
    start..self.len()
  }

  pub fn len(&self) -> usize {
    self.vec.len()
  }
}

impl<T, I: SliceIndex<[T]>> Index<I> for Arena<T> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&*self.vec, index)
    }
}

impl<T, I: SliceIndex<[T]>> IndexMut<I> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut *self.vec, index)
    }
}

extern crate serde;
use serde::{Serialize, Serializer, ser::SerializeSeq};

impl<T> Serialize for Arena<T>
where
  T: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut seq = serializer.serialize_seq(Some(self.vec.len()))?;
    for e in self.vec.iter() {
      seq.serialize_element(&e)?;
    }
    seq.end()
  }
}
use serde::{Deserialize, Deserializer, de::{Visitor, SeqAccess}};
use core::marker::PhantomData;
use core::fmt;

impl<'de, T> Deserialize<'de> for Arena<T> 
    where T: Deserialize<'de> 
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ArenaVisitor<T> {
      marker: PhantomData<Arena<T>>,
    }

    impl<'de, T> Visitor<'de> for ArenaVisitor<T>
      where T: Deserialize<'de>
    {
      type Value = Arena<T>;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
          formatter.write_str("struct GTFSData")
      }

      /// serialisation is
      /// stop_time_count: u32
      /// [stop_time: StopTime; stop_time_count]
      fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
      where
        V: SeqAccess<'de>,
      {
        let stop_time_count: usize = seq.size_hint().unwrap_or(0);
        let mut arena = Arena::with_capacity(stop_time_count);
        while let Some(e) = seq.next_element()? {
          arena.alloc(e);
        }
        Ok(arena)
      }
    }

    deserializer.deserialize_seq(ArenaVisitor { marker: PhantomData })
  }
}
