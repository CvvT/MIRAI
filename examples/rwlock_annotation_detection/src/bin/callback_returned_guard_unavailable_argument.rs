use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_ready_metadata<R>(
    owner: &DescriptorOwner,
    callback: impl FnOnce(&mut Metadata) -> R,
) -> R {
    owner
        .descriptor_table_mut()
        .with_ready_metadata_mut(callback)
}

fn invoke_from_non_root(owner: &DescriptorOwner) {
    with_ready_metadata(owner, |metadata| metadata.require_ready());
}

fn main() {
    let owner = DescriptorOwner;
    invoke_from_non_root(&owner);
}
