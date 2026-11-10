//! Immutable buffer ownership across asynchronous consumers and renderer resource lifetimes.

use super::assets::BufferSnapshot;
use super::{ApplicationAssetLease, ApplicationBufferKey, ApplicationBufferSlice};
use std::collections::BTreeMap;

enum Lifetime {
    Drawing,
    Resource {
        request_id: u32,
        resource_id: Option<u32>,
    },
}
struct Lease {
    scope: ApplicationAssetLease,
    lifetime: Lifetime,
    buffers: BTreeMap<ApplicationBufferKey, BufferSnapshot>,
}
#[derive(Default)]
pub(super) struct AssetLeases {
    leases: BTreeMap<u32, Lease>,
}
impl AssetLeases {
    pub(super) fn insert_drawing(
        &mut self,
        lease_id: u32,
        renderer_resource_id: u32,
        resource_epoch: u32,
        buffers: Vec<BufferSnapshot>,
    ) -> ApplicationAssetLease {
        self.insert(
            lease_id,
            renderer_resource_id,
            resource_epoch,
            Lifetime::Drawing,
            buffers,
        )
    }
    pub(super) fn insert_resource(
        &mut self,
        lease_id: u32,
        renderer_resource_id: u32,
        request_id: u32,
        buffers: Vec<BufferSnapshot>,
    ) -> ApplicationAssetLease {
        self.insert(
            lease_id,
            renderer_resource_id,
            request_id,
            Lifetime::Resource {
                request_id,
                resource_id: None,
            },
            buffers,
        )
    }
    fn insert(
        &mut self,
        lease_id: u32,
        renderer_resource_id: u32,
        resource_epoch: u32,
        lifetime: Lifetime,
        buffers: Vec<BufferSnapshot>,
    ) -> ApplicationAssetLease {
        let buffers: BTreeMap<_, _> = buffers
            .into_iter()
            .map(|buffer| {
                (
                    ApplicationBufferKey {
                        buffer_id: buffer.buffer_id,
                        revision: buffer.revision,
                    },
                    buffer,
                )
            })
            .collect();
        let scope = ApplicationAssetLease {
            lease_id,
            renderer_resource_id,
            resource_epoch,
            buffers: buffers.keys().copied().collect(),
        };
        self.leases.insert(
            lease_id,
            Lease {
                scope: scope.clone(),
                lifetime,
                buffers,
            },
        );
        scope
    }
    pub(super) fn pinned_bytes_with<'a>(
        &'a self,
        extra: impl IntoIterator<Item = &'a BufferSnapshot>,
    ) -> usize {
        let mut allocations = BTreeMap::new();
        for buffer in self
            .leases
            .values()
            .flat_map(|lease| lease.buffers.values())
            .chain(extra)
        {
            allocations.insert(buffer.bytes.as_ptr(), buffer.bytes.len());
        }
        allocations.values().sum()
    }

    pub(super) fn buffer(
        &self,
        lease_id: u32,
        key: ApplicationBufferKey,
    ) -> Option<ApplicationBufferSlice<'_>> {
        let buffer = self.leases.get(&lease_id)?.buffers.get(&key)?;
        Some(ApplicationBufferSlice::U8(&buffer.bytes))
    }
    pub(super) fn consumed(&mut self, lease_id: u32, renderer: u32, epoch: u32) {
        if self.leases.get(&lease_id).is_some_and(|lease| {
            matches!(lease.lifetime, Lifetime::Drawing)
                && lease.scope.renderer_resource_id == renderer
                && lease.scope.resource_epoch == epoch
        }) {
            self.leases.remove(&lease_id);
        }
    }
    pub(super) fn prepared(&mut self, request_id: u32, resource_id: u32) {
        for lease in self.leases.values_mut() {
            if let Lifetime::Resource {
                request_id: current,
                resource_id: resource,
            } = &mut lease.lifetime
                && *current == request_id
            {
                *resource = Some(resource_id);
            }
        }
    }
    pub(super) fn cancelled(&mut self, request_id: u32) -> Vec<u32> {
        let mut retired = Vec::new();
        self.leases.retain(|_, lease| {
            let remove = matches!(lease.lifetime, Lifetime::Resource{request_id:current,resource_id:None} if current==request_id);
            if remove { retired.push(lease.scope.resource_epoch); }
            !remove
        });
        retired
    }
    pub(super) fn released(&mut self, resource_id: u32) -> Vec<u32> {
        let mut retired = Vec::new();
        self.leases.retain(|_, lease| {
            let remove = matches!(lease.lifetime, Lifetime::Resource{resource_id:Some(current),..} if current==resource_id);
            if remove { retired.push(lease.scope.resource_epoch); }
            !remove
        });
        retired
    }
    pub(super) fn retire_renderer(&mut self, renderer: u32) -> Vec<u32> {
        let mut retired = Vec::new();
        self.leases.retain(|_, lease| {
            if lease.scope.renderer_resource_id != renderer {
                return true;
            }
            if matches!(lease.lifetime, Lifetime::Resource { .. }) {
                retired.push(lease.scope.resource_epoch);
            }
            false
        });
        retired
    }
}
