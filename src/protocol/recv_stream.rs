use std::{
    fmt::Display,
    io::Read,
    task::{Context, Poll},
};

use futures_core::Stream;
use tokio::sync::mpsc::{error::TryRecvError, Receiver};
use webparse::{Binary, BinaryMut, Buf, Serialize, Helper};

use crate::ProtResult;

#[derive(Debug)]
pub struct RecvStream {
    receiver: Option<Receiver<(bool, Binary)>>,
    binary: Option<Binary>,
    binary_mut: Option<BinaryMut>,
    is_end: bool,
}

impl RecvStream {
    pub fn empty() -> RecvStream {
        RecvStream {
            receiver: None,
            binary: None,
            binary_mut: None,
            is_end: true,
        }
    }

    pub fn only(binary: Binary) -> RecvStream {
        RecvStream {
            receiver: None,
            binary: Some(binary),
            binary_mut: None,
            is_end: true,
        }
    }

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: Some(receiver),
            binary: None,
            binary_mut: Some(binary),
            is_end,
        }
    }

    pub fn binary(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.binary.take() {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            buffer.put_slice(bin.chunk());
        }
        buffer.freeze()
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn set_end(&mut self, end: bool) {
        self.is_end = end
    }

    pub fn try_recv(&mut self) {
        if self.receiver.is_none() {
            return;
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Ok(v) = receiver.try_recv() {
            if self.binary_mut.is_none() {
                self.binary_mut = Some(BinaryMut::new());
            }
            self.binary_mut.as_mut().unwrap().put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
    }

    pub fn read_now(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.binary.take() {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn copy_now(&self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = &self.binary {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = &self.binary_mut {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn body_len(&self) -> usize {
        let mut len = 0;
        if let Some(bin) = &self.binary {
            len += bin.remaining();
        }
        if let Some(bin) = &self.binary_mut {
            len += bin.remaining();
        }
        return len;
    }

    pub async fn wait_all(&mut self) -> Option<usize> {
        let mut size = 0;
        if self.receiver.is_none() || self.is_end {
            return Some(size);
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Some(v) = receiver.recv().await {
            if self.binary_mut.is_none() {
                self.binary_mut = Some(BinaryMut::new());
            }
            size += self.binary_mut.as_mut().unwrap().put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(size)
    }

    pub async fn read_all(&mut self, buffer: &mut BinaryMut) -> Option<usize> {
        let mut size = 0;
        if let Some(binary) = &mut self.binary {
            size += buffer.put_slice(binary.chunk());
            binary.advance_all();
        }
        if let Some(binary) = &mut self.binary_mut {
            size += buffer.put_slice(binary.chunk());
            binary.advance_all();
        }
        if self.is_end {
            return Some(size);
        }
        if self.receiver.is_none() {
            return Some(size);
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Some(v) = receiver.recv().await {
            size += buffer.put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(size)
    }

    fn encode_data<B: webparse::Buf + webparse::BufMut>(
        buffer: &mut B,
        data: &[u8],
        is_chunked: bool,
    ) -> webparse::WebResult<usize> {
        if is_chunked {
            Helper::encode_chunk_data(buffer, data)
        } else {
            Ok(buffer.put_slice(data))
        }
    }

    pub fn poll_encode<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut B,
        is_chunked: bool,
    ) -> Poll<webparse::WebResult<usize>> {
        let mut size = 0;
        if let Some(bin) = self.binary.take() {
            if bin.chunk().len() > 0 {
                size += Self::encode_data(buffer, bin.chunk(), is_chunked)?;
            }
        }
        if let Some(bin) = self.binary_mut.take() {
            if bin.chunk().len() > 0 {
                size += Self::encode_data(buffer, bin.chunk(), is_chunked)?;
            }
        }
        let mut has_encode_end = false;
        if self.receiver.is_some() && !self.is_end {
            loop {
                match self.receiver.as_mut().unwrap().poll_recv(cx) {
                    Poll::Pending => {
                        break;
                    }
                    Poll::Ready(Some((is_end, bin))) => {
                        size += Self::encode_data(buffer, bin.chunk(), is_chunked)?;
                        self.is_end = is_end;
                        has_encode_end = is_end;
                    }
                    Poll::Ready(None) => {
                        break;
                    }
                }
            }
        }
        if !has_encode_end && is_chunked && self.is_end {
            Self::encode_data(buffer, &[], is_chunked)?;
        }
        Poll::Ready(Ok(size))
    }
}

impl Stream for RecvStream {
    type Item = ProtResult<Binary>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for RecvStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.try_recv();
        let mut read_bytes = 0;
        if let Some(bin) = &mut self.binary {
            if bin.remaining() > 0 {
                let len = std::cmp::min(buf.len() - read_bytes, bin.remaining());
                read_bytes += bin.copy_to_slice(&mut buf[read_bytes..len]);
            }
        }
        if let Some(bin) = &mut self.binary {
            if bin.remaining() > 0 {
                let len = std::cmp::min(buf.len() - read_bytes, bin.remaining());
                read_bytes += bin.copy_to_slice(&mut buf[read_bytes..len]);
            }
        }
        Ok(read_bytes)
    }
}

// impl AsyncRead for RecvStream {
//     fn poll_read(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut tokio::io::ReadBuf<'_>,
//     ) -> Poll<std::io::Result<()>> {
//         todo!()
//     }
// }

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        buffer: &mut B,
    ) -> webparse::WebResult<usize> {
        let mut size = 0;
        if let Some(bin) = self.binary.take() {
            size += buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            size += buffer.put_slice(bin.chunk());
        }
        if self.receiver.is_some() && !self.is_end {
            loop {
                match self.receiver.as_mut().unwrap().try_recv() {
                    Ok((is_end, mut bin)) => {
                        size += bin.serialize(buffer)?;
                        self.is_end = is_end;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.is_end = true;
                        return Ok(size);
                    }
                    Err(TryRecvError::Empty) => {
                        return Ok(size);
                    }
                }
            }
        }
        Ok(size)
    }
}

unsafe impl Sync for RecvStream {}

unsafe impl Send for RecvStream {}

impl From<()> for RecvStream {
    fn from(_: ()) -> Self {
        RecvStream::empty()
    }
}

impl From<&str> for RecvStream {
    fn from(value: &str) -> Self {
        let bin = Binary::from(value.as_bytes().to_vec());
        RecvStream::only(bin)
    }
}

impl From<Binary> for RecvStream {
    fn from(value: Binary) -> Self {
        RecvStream::only(value)
    }
}

impl From<String> for RecvStream {
    fn from(value: String) -> Self {
        let bin = Binary::from(value.into_bytes().to_vec());
        RecvStream::only(bin)
    }
}

impl From<Vec<u8>> for RecvStream {
    fn from(value: Vec<u8>) -> Self {
        let bin = Binary::from(value);
        RecvStream::only(bin)
    }
}

impl From<RecvStream> for Vec<u8> {
    fn from(mut value: RecvStream) -> Self {
        let bin = value.read_now();
        bin.into_slice_all()
    }
}

impl From<RecvStream> for String {
    fn from(mut value: RecvStream) -> Self {
        let bin = value.read_now();
        let v = bin.into_slice_all();
        String::from_utf8_lossy(&v).to_string()
    }
}

impl Display for RecvStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_end {
            let bin = self.copy_now();
            f.write_str(&String::from_utf8_lossy(bin.chunk()))
        } else {
            let mut f = f.debug_struct("RecvStream");
            f.field("状态", &self.is_end);
            if self.is_end {
                f.field("接收字节数", &self.body_len());
            }
            f.finish()
        }
    }
}
// impl<T> From<T> for RecvStream where T : Serialize {
//     fn from(value: T) -> Self {
//         todo!()
//     }
// }

// impl From<Option<dyn Serialize + ?Sized>> for RecvStream {
//     fn from(value: Option<dyn Serialize + Sized>) -> Self {
//         todo!()
//     }
// }
