use core::ops::{Index, IndexMut, Range, RangeBounds, Bound};
use core::marker::PhantomData;
use core::fmt;
use serde::{ser::{self, Serialize, Serializer}, de::{Deserialize, Deserializer, Visitor, SeqAccess}};

/// Allocator optimised for serialisation and references of overlapping slices
/// 
/// Currently backed by a vec, so serialisation / deserialisation / retreival should be fast / addition will be fast if a sufficient capacity can be specified at creation. Removal is not supported. Indexes are typed so the user just needs to make sure that the indexes are not used on another arena of the same type.
/// 
/// # TODO
/// * compile time checking of whether id is related to this arena - dont use usize
/// * link vecs instead of resizing to avoid copies
pub struct Arena<T> {
  vec: Vec<T>,
}

impl<T> Arena<T> {
  pub fn new() -> Arena<T> {
    Arena {
      vec: Vec::new()
    }
  }

  pub fn with_capacity(capacity: usize) -> Arena<T> {
    Arena {
      vec: Vec::with_capacity(capacity)
    }
  }

  fn alloc_internal(&mut self, el: T) -> usize {
    self.vec.push(el);
    self.len() - 1
  }

  pub fn alloc(&mut self, el: T) -> ArenaIndex<T> {
    ArenaIndex {
      marker: std::marker::PhantomData,
      idx: self.alloc_internal(el),
    }
  }

  pub fn alloc_extend_internal<I>(&mut self, iterable: I) -> Range<usize>
  where
    I: IntoIterator<Item = T>,
  {
    let start = self.vec.len();
    self.vec.extend(iterable);
    start..self.len()
  }

  pub fn alloc_extend<I>(&mut self, iterable: I) -> ArenaSliceIndex<T>
  where
    I: IntoIterator<Item = T>,
  {
    let range = self.alloc_extend_internal(iterable);
    ArenaSliceIndex {
      marker: std::marker::PhantomData,
      start: range.start,
      end: range.end,
    }
  }

  /// Extends the arena with the ok value of each of the items in the iterator, if any iteration fails, the error is returned and the allocation does not occur
  pub fn alloc_extend_result<I, E>(&mut self, iterable: I) -> Result<ArenaSliceIndex<T>, E>
  where
    I: IntoIterator<Item = Result<T, E>>,
  {
    let start = self.vec.len();
    let mut iter = iterable.into_iter();
    self.vec.reserve(iter.size_hint().0); // reserve enought for the minimum size if provided
    while let Some(element) = iter.next() {
      match element {
        Ok(element) => 
          self.vec.push(element),
        Err(err) => {
          self.vec.truncate(start);
          return Err(err)
        }
      }
    }
    Ok(ArenaSliceIndex {
      marker: std::marker::PhantomData,
      start,
      end: self.vec.len(),
    })
  }

  pub fn len(&self) -> usize {
    self.vec.len()
  }
}

impl<T> Index<ArenaIndex<T>> for Arena<T> {
    type Output = T;

    #[inline]
    fn index(&self, refi: ArenaIndex<T>) -> &Self::Output {
        Index::index(&*self.vec, refi.idx)
    }
}

impl<T> IndexMut<ArenaIndex<T>> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, refi: ArenaIndex<T>) -> &mut Self::Output {
        IndexMut::index_mut(&mut *self.vec, refi.idx)
    }
}

impl<T> Index<ArenaSliceIndex<T>> for Arena<T> {
    type Output = [T];

    #[inline]
    fn index(&self, refi: ArenaSliceIndex<T>) -> &Self::Output {
        Index::index(&*self.vec, refi.start..refi.end)
    }
}

impl<T> IndexMut<ArenaSliceIndex<T>> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, refi: ArenaSliceIndex<T>) -> &mut Self::Output {
        IndexMut::index_mut(&mut *self.vec, refi.start..refi.end)
    }
}

pub struct ArenaIndex<T> {
  marker: PhantomData<T>,
  idx: usize,
}

impl<T> Clone for ArenaIndex<T> {
  fn clone(&self) -> Self {
    ArenaIndex {
      marker: PhantomData,
      idx: self.idx,
    }
  }
}

impl<T> Copy for ArenaIndex<T> {}

impl<T> PartialEq for ArenaIndex<T> {
  fn eq(&self, rhs: &Self) -> bool {
    self.idx == rhs.idx
  }
}

impl<T> fmt::Debug for ArenaIndex<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    write!(formatter, "arena::ArenaIndex({})", self.idx)
  }
}

pub struct ArenaSliceIndex<T> {
  marker: PhantomData<T>,
  start: usize,
  end: usize,
}

impl<T> ArenaSliceIndex<T> {
  pub fn sub<I>(&self, range: I) -> Self
    where I: RangeBounds<usize>
  {
    ArenaSliceIndex {
      marker: PhantomData,
      start: match range.start_bound() {
        Bound::Unbounded => self.start,
        Bound::Included(offset) => self.start + offset,
        Bound::Excluded(offset) => self.start + offset + 1,
      },
      end: match range.end_bound() {
        Bound::Unbounded => self.end,
        Bound::Included(offset) => self.start + offset + 1,
        Bound::Excluded(offset) => self.start + offset,
      },
    }
  }

  pub fn len(&self) -> usize {
    self.end - self.start
  }

  pub fn iter(&self) -> impl Iterator<Item = ArenaIndex<T>> {
    (self.start..self.end).into_iter().map(|idx|
      ArenaIndex {
        marker: PhantomData,
        idx: idx,
      }
    )
  }
}

impl<T> Clone for ArenaSliceIndex<T> {
  fn clone(&self) -> Self {
    ArenaSliceIndex {
      marker: PhantomData,
      start: self.start,
      end: self.end,
    }
  }
}

impl<T> Copy for ArenaSliceIndex<T> {}

impl<T> PartialEq for ArenaSliceIndex<T> {
  fn eq(&self, rhs: &Self) -> bool {
    self.start == rhs.start && self.end == rhs.end
  }
}

impl<T> fmt::Debug for ArenaSliceIndex<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    write!(formatter, "arena::ArenaSliceIndex({}..{})", self.start, self.end)
  }
}

impl<T> Serialize for Arena<T>
where
  T: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    use ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(self.vec.len()))?;
    for e in self.vec.iter() {
      seq.serialize_element(&e)?;
    }
    seq.end()
  }
}

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

      fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
      where
        V: SeqAccess<'de>,
      {
        let length: usize = seq.size_hint().unwrap_or(0);
        let mut arena = Arena::with_capacity(length);
        while let Some(e) = seq.next_element()? {
          arena.alloc(e);
        }
        Ok(arena)
      }
    }

    deserializer.deserialize_seq(ArenaVisitor { marker: PhantomData })
  }
}

impl<T> Serialize for ArenaSliceIndex<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ArenaSliceIndex", 2)?;
        state.serialize_field("start", &self.start)?;
        state.serialize_field("end", &self.end)?;
        state.end()
    }
}

impl<'de, T> Deserialize<'de> for ArenaSliceIndex<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
      D: Deserializer<'de>,
  {
      let (start, end) = deserializer.deserialize_struct(
          "ArenaSliceIndex",
          &["start", "end"],
          sede::RangeVisitor {
              expecting: "struct ArenaSliceIndex",
              phantom: PhantomData,
          },
      )?;
      Ok(ArenaSliceIndex {
        marker: PhantomData,
        start,
        end,
      })
  }
}

mod sede {
  use serde::{self, Deserialize, de::{self, MapAccess, Visitor, SeqAccess}};
  use core::marker::PhantomData;
  use core::fmt;

  #[derive(Deserialize)]
  #[serde(field_identifier, rename_all = "lowercase")]
  enum Field {
    Start,
    End,
  }

  pub struct RangeVisitor<Idx> {
    pub expecting: &'static str,
    pub phantom: PhantomData<Idx>,
  }

  impl<'de, Idx> Visitor<'de> for RangeVisitor<Idx>
  where
    Idx: Deserialize<'de>,
  {
    type Value = (Idx, Idx);

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let start: Idx = match seq.next_element()? {
            Some(value) => value,
            None => {
                return Err(de::Error::invalid_length(0, &self));
            }
        };
        let end: Idx = match seq.next_element()? {
            Some(value) => value,
            None => {
                return Err(de::Error::invalid_length(1, &self));
            }
        };
        Ok((start, end))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut start: Option<Idx> = None;
        let mut end: Option<Idx> = None;
        while let Some(key) = map.next_key()? {
            match key {
                Field::Start => {
                    if start.is_some() {
                        return Err(<A::Error as de::Error>::duplicate_field("start"));
                    }
                    start = Some(map.next_value()?);
                }
                Field::End => {
                    if end.is_some() {
                        return Err(<A::Error as de::Error>::duplicate_field("end"));
                    }
                    end = Some(map.next_value()?);
                }
            }
        }
        let start = match start {
            Some(start) => start,
            None => return Err(<A::Error as de::Error>::missing_field("start")),
        };
        let end = match end {
            Some(end) => end,
            None => return Err(<A::Error as de::Error>::missing_field("end")),
        };
        Ok((start, end))
    }
  }
}


#[cfg(test)]
mod test {
  use super::{Arena, ArenaIndex};

  #[allow(dead_code)]
  struct Node {
    parent: Option<ArenaIndex<Node>>,
    left: Option<ArenaIndex<Node>>,
    right: Option<ArenaIndex<Node>>,
  }
  impl Node {
    fn new_child_of(parent: ArenaIndex<Node>) -> Node {
      Node { parent: Some(parent), right: None, left: None }
    }
  }
  struct Tree {
    arena: Arena<Node>,
    root: ArenaIndex<Node>,
  }

  #[test]
  fn test_tree() {
    let mut arena: Arena<Node> = Arena::new();
    let root = arena.alloc(Node { parent: None, left: None, right: None });
    let mut tree = Tree {
      arena,
      root,
    };
    let first_right = tree.arena.alloc(Node::new_child_of(tree.root));
    tree.arena[tree.root].right = Some(first_right);
    assert_eq!(tree.arena[tree.root].right, Some(first_right));
    assert_eq!(tree.arena.len(), 2);
  }

  #[derive(PartialEq, Debug, Clone)]
  struct Stop(u32);

  #[test]
  fn test_trip() {
    let mut arena: Arena<Stop> = Arena::new();
    let stops_vec = vec![Stop(1), Stop(2), Stop(3), Stop(4), Stop(5)];
    arena.alloc(Stop(0));
    let slice_ref = arena.alloc_extend(stops_vec.clone().into_iter());
    arena.alloc(Stop(0));
    assert_eq!(7, arena.len());
    assert_eq!([Stop(1), Stop(2), Stop(3), Stop(4), Stop(5)], arena[slice_ref]);
    let subref = slice_ref.sub(..);
    assert_eq!([Stop(1), Stop(2), Stop(3), Stop(4), Stop(5)], arena[subref]);
    let subref = slice_ref.sub(1..3);
    assert_eq!([Stop(2), Stop(3)], arena[subref]);
    let subref = slice_ref.sub(1..=3);
    assert_eq!([Stop(2), Stop(3), Stop(4)], arena[subref]);
    let subref = slice_ref.sub(2..);
    assert_eq!([Stop(3), Stop(4), Stop(5)], arena[subref]);
    let subref = slice_ref.sub(..2);
    assert_eq!([Stop(1), Stop(2)], arena[subref]);
  }

  #[test]
  fn test_alloc_extend_result_ok() {
    let mut arena: Arena<Stop> = Arena::new();
    let stops_vec: Vec<Result<Stop, i64>> = vec![Ok(Stop(1)), Ok(Stop(2)), Ok(Stop(3)), Ok(Stop(4)), Ok(Stop(5))];
    let slice_ref = arena.alloc_extend_result(stops_vec.clone().into_iter()).unwrap();
    assert_eq!([Stop(1), Stop(2), Stop(3), Stop(4), Stop(5)], arena[slice_ref]);
    assert_eq!(5, arena.len());
  }

  #[test]
  fn test_alloc_extend_result_err() {
    let mut arena: Arena<Stop> = Arena::new();
    let stops_vec = vec![Ok(Stop(1)), Ok(Stop(2)), Ok(Stop(3)), Ok(Stop(4)), Err("fail")];
    let err = arena.alloc_extend_result(stops_vec.clone().into_iter()).unwrap_err();
    assert_eq!(err, "fail");
    assert_eq!(0, arena.len());
  }
}
