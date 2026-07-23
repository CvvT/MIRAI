use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_metadata_mut<R>(owner: &DescriptorOwner, callback: impl FnOnce(&mut Metadata) -> R) -> R {
    owner.descriptor_table_mut().with_metadata_mut(callback)
}

fn main() {
    let owner = DescriptorOwner;
    let other = DescriptorOwner;
    with_metadata_mut(&owner, |_metadata| other.read());
}
