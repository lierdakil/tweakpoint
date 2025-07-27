//! Missing utilities

pub enum EitherIter<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Iterator for EitherIter<L, R>
where
    L: Iterator,
    R: Iterator<Item = L::Item>,
{
    type Item = L::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EitherIter::Left(it) => it.next(),
            EitherIter::Right(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            EitherIter::Left(it) => it.size_hint(),
            EitherIter::Right(it) => it.size_hint(),
        }
    }
}

impl<L, R> From<L> for EitherIter<L::IntoIter, R>
where
    L: IntoIterator,
{
    fn from(value: L) -> Self {
        EitherIter::left(value)
    }
}

impl<L, R> EitherIter<L, R> {
    pub fn left(v: impl IntoIterator<IntoIter = L>) -> Self {
        Self::Left(v.into_iter())
    }

    pub fn right(v: impl IntoIterator<IntoIter = R>) -> Self {
        Self::Right(v.into_iter())
    }
}

pub trait IteratorExt: Sized + IntoIterator {
    fn left<T>(self) -> EitherIter<Self::IntoIter, T>;

    fn right<T>(self) -> EitherIter<T, Self::IntoIter>;
}

impl<T: IntoIterator> IteratorExt for T {
    fn left<U>(self) -> EitherIter<Self::IntoIter, U> {
        EitherIter::Left(self.into_iter())
    }

    fn right<U>(self) -> EitherIter<U, Self::IntoIter> {
        EitherIter::Right(self.into_iter())
    }
}
