use futures::stream::{Fuse, Stream};
use futures::{Async, Poll};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from either of the underlying streams as
/// they become available, and the streams are polled in a round-robin fashion.
/// Errors, however, are not merged: you get at most one error at a time.
///
/// Unlike normal select, the stream will stop once one of the streams ends.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct SelectBreaker<S1, S2> {
    stream1: Fuse<S1>,
    stream2: Fuse<S2>,
    flag: bool,
}

fn new<S1, S2>(stream1: S1, stream2: S2) -> SelectBreaker<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item, Error = S1::Error>,
{
    SelectBreaker {
        stream1: stream1.fuse(),
        stream2: stream2.fuse(),
        flag: false,
    }
}

impl<S1, S2> Stream for SelectBreaker<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item, Error = S1::Error>,
{
    type Item = S1::Item;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Option<S1::Item>, S1::Error> {
        let (a, b) = if self.flag {
            (
                &mut self.stream2 as &mut Stream<Item = _, Error = _>,
                &mut self.stream1 as &mut Stream<Item = _, Error = _>,
            )
        } else {
            (
                &mut self.stream1 as &mut Stream<Item = _, Error = _>,
                &mut self.stream2 as &mut Stream<Item = _, Error = _>,
            )
        };

        self.flag = !self.flag;

        match a.poll()? {
            Async::Ready(Some(item)) => return Ok(Some(item).into()),
            Async::Ready(None) => return Ok(None.into()),
            Async::NotReady => {}
        };

        match b.poll()? {
            Async::Ready(Some(item)) => Ok(Some(item).into()),
            Async::Ready(None) => return Ok(None.into()),
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

pub trait StreamExt: Stream + Sized {
    fn select_breaker<S>(self, other: S) -> SelectBreaker<Self, S>
    where
        S: Stream<Item = Self::Item, Error = Self::Error>,
        Self: Sized,
    {
        new(self, other)
    }
}

impl<S: Stream> StreamExt for S {}
