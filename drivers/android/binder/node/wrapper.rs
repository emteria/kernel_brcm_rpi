// SPDX-License-Identifier: GPL-2.0

// Copyright (C) 2024 Google LLC.

use kernel::{
    list::{ListArc, ListArcSafe},
    prelude::*,
    seq_file::SeqFile,
    seq_print,
    sync::UniqueArc,
    uaccess::UserSliceWriter,
};

use crate::{node::Node, thread::Thread, DArc, DLArc, DTRWrap, DeliverToRead};

use core::mem::MaybeUninit;

pub(crate) struct CritIncrWrapper {
    inner: UniqueArc<MaybeUninit<DTRWrap<NodeWrapper>>>,
}

impl CritIncrWrapper {
    pub(crate) fn new() -> Result<Self> {
        Ok(CritIncrWrapper {
            inner: UniqueArc::try_new_uninit()?,
        })
    }

    pub(super) fn init(self, node: DArc<Node>) -> DLArc<dyn DeliverToRead> {
        match self.inner.pin_init_with(DTRWrap::new(NodeWrapper { node })) {
            Ok(initialized) => ListArc::from_pin_unique(initialized) as _,
            Err(err) => match err {},
        }
    }
}

struct NodeWrapper {
    node: DArc<Node>,
}

kernel::list::impl_list_arc_safe! {
    impl ListArcSafe<0> for NodeWrapper {
        untracked;
    }
}

impl DeliverToRead for NodeWrapper {
    fn do_work(self: DArc<Self>, _thread: &Thread, writer: &mut UserSliceWriter) -> Result<bool> {
        let node = &self.node;
        let mut owner_inner = node.owner.inner.lock();
        let inner = node.inner.access_mut(&mut owner_inner);

        let ds = &mut inner.delivery_state;

        assert!(ds.has_pushed_wrapper);
        assert!(ds.has_strong_zero2one);
        ds.has_pushed_wrapper = false;
        ds.has_strong_zero2one = false;

        node.do_work_locked(writer, owner_inner)
    }

    fn cancel(self: DArc<Self>) {}
    fn on_thread_selected(&self, _thread: &Thread) {}

    fn should_sync_wakeup(&self) -> bool {
        false
    }

    #[inline(never)]
    fn debug_print(&self, m: &mut SeqFile, prefix: &str, _tprefix: &str) -> Result<()> {
        seq_print!(
            m,
            "{}node work {}: u{:016x} c{:016x}\n",
            prefix,
            self.node.debug_id,
            self.node.ptr,
            self.node.cookie,
        );
        Ok(())
    }
}
