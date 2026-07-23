use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_metadata_mut<R>(owner: &DescriptorOwner, callback: impl FnOnce(&mut Metadata) -> R) -> R {
    owner.descriptor_table_mut().with_metadata_mut(callback)
}

fn forward_metadata_mut<R>(
    owner: &DescriptorOwner,
    callback: impl FnOnce(&mut Metadata) -> R,
) -> R {
    with_metadata_mut(owner, callback)
}

fn invoke_from_non_root(owner: &DescriptorOwner) {
    forward_metadata_mut(owner, |_metadata| owner.read());
}

fn main() {
    let owner = DescriptorOwner;
    invoke_from_non_root(&owner);
}
