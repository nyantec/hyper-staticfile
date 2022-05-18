use futures_util::stream::Stream;
use hyper::body::{Body, Bytes};
use std::io::Error as IoError;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, ReadBuf};

const BUF_SIZE: usize = 8 * 1024;

/// Wraps a `tokio::fs::File`, and implements a stream of `Bytes`s.
pub struct FileBytesStream {
    file: File,
    buf: Box<[MaybeUninit<u8>; BUF_SIZE]>,
    content_length: Option<usize>,
    written: usize,
}

impl FileBytesStream {
    /// Create a new stream from the given file.
    pub fn new(file: File, content_length: Option<usize>) -> FileBytesStream {
        let buf = Box::new([MaybeUninit::uninit(); BUF_SIZE]);
        FileBytesStream { file, buf, content_length, written: 0 }
    }
}

impl Stream for FileBytesStream {
    type Item = Result<Bytes, IoError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Self {
            ref mut file,
            ref mut buf,
            ref content_length,
            ref mut written,
        } = *self;
        let needed: Option<usize> = content_length.map(|cl| cl - *written);
        let mut read_buf = match needed {
            Some(0) => return Poll::Ready(None),
            Some(n) if n < BUF_SIZE => ReadBuf::uninit(&mut buf[..n]),
            _ => ReadBuf::uninit(&mut buf[..]),
        };

        match Pin::new(file).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled();
                if filled.is_empty() {
                    Poll::Ready(None)
                } else {
                    *written += filled.len();
                    Poll::Ready(Some(Ok(Bytes::copy_from_slice(filled))))
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl FileBytesStream {
    /// Create a Hyper `Body` from this stream.
    pub fn into_body(self) -> Body {
        Body::wrap_stream(self)
    }
}
