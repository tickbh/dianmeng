use std::{sync::Arc, task::{Context, Poll}, collections::HashMap};

use rbtree::RBTree;
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::http2::frame::{Frame, Priority, PriorityFrame, StreamIdentifier},
    Binary, Serialize, Response,
};

use crate::ProtoResult;

use super::codec::Codec;

#[derive(Debug)]
pub struct PriorityQueue {
    pub send_queue: RBTree<PriorityFrame<Binary>, ()>,
    pub hash_weight: HashMap<StreamIdentifier, u8>,
    pub hash_depend: HashMap<StreamIdentifier, StreamIdentifier>,
}

impl PriorityQueue {
    pub fn new() -> Self {
        PriorityQueue {
            send_queue: RBTree::new(),
            hash_weight: HashMap::from([
                (StreamIdentifier::zero(), 255),
            ]),
            hash_depend: HashMap::new(),
        }
    }

    pub fn priority_recv(&mut self, p: Priority) {
        let (id, depend_id, weight) = p.into();
        self.hash_weight.insert(id, weight);
        if !depend_id.is_zero() {
            self.hash_depend.insert(id, depend_id);
            let next = std::cmp::max(weight.wrapping_add(1), 255);
            self.hash_weight.entry(depend_id).and_modify(|v| {
                *v = std::cmp::max(*v, next)
            }).or_insert( next);
        }
    }

    pub fn weight(&self, stream_id: &StreamIdentifier) -> u8 {
        if self.hash_weight.contains_key(stream_id) {
            self.hash_weight[stream_id]
        } else {
            0
        }
    }

    pub fn send_frames(&mut self, stream_id: StreamIdentifier, vec: Vec<Frame<Binary>>) -> ProtoResult<()> {
        for v in vec {
            self.send_queue.insert(PriorityFrame::new(v, self.weight(&stream_id)), ());
        }
        Ok(())
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<()>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            if !codec.poll_ready(cx)?.is_ready() || self.send_queue.is_empty() {
                return Poll::Ready(None);
            }
            let first = self.send_queue.pop_first().unwrap();
            codec.send_frame(first.0.frame)?;
        }
    }

}

unsafe impl Sync for PriorityQueue {

}

unsafe impl Send for PriorityQueue {

}