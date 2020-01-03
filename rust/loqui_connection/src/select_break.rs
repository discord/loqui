use futures::stream::{Fuse, Stream, StreamExt as FuturesStreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from either of the underlying streams as
/// they become available, and the streams are polled in a round-robin fashion.
///
/// Unlike normal select, the stream will stop once one of the streams ends.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct SelectBreak<S1, S2> {
    stream1: Fuse<S1>,
    stream2: Fuse<S2>,
    flag: bool,
}

fn new<S1, S2>(stream1: S1, stream2: S2) -> SelectBreak<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
{
    SelectBreak {
        stream1: stream1.fuse(),
        stream2: stream2.fuse(),
        flag: false,
    }
}

impl<S1, S2> Stream for SelectBreak<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
{
    type Item = S1::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<S1::Item>> {
        let SelectBreak {
            flag,
            stream1,
            stream2,
        } = unsafe { self.get_unchecked_mut() };
        let stream1 = unsafe { Pin::new_unchecked(stream1) };
        let stream2 = unsafe { Pin::new_unchecked(stream2) };
        if !*flag {
            poll_inner(flag, stream1, stream2, cx)
        } else {
            poll_inner(flag, stream2, stream1, cx)
        }
    }
}

fn poll_inner<S1, S2>(
    flag: &mut bool,
    a: Pin<&mut S1>,
    b: Pin<&mut S2>,
    cx: &mut Context<'_>,
) -> Poll<Option<S1::Item>>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
{
    *flag = !*flag;
    match a.poll_next(cx) {
        Poll::Pending => {}
        other => return other,
    };

    b.poll_next(cx)
}

pub trait StreamExt: Stream + Sized {
    /// An adapter for merging the output of two streams.
    ///
    /// The merged stream produces items from either of the underlying streams as
    /// they become available, and the streams are polled in a round-robin fashion.
    ///
    /// Unlike normal select, the stream will stop once one of the streams ends.
    fn select_break<S>(self, other: S) -> SelectBreak<Self, S>
    where
        S: Stream<Item = Self::Item>,
        Self: Sized,
    {
        new(self, other)
    }
}

impl<S: Stream> StreamExt for S {}
