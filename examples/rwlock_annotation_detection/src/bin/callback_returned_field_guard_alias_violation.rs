use rwlock_annotation_detection::{FieldDescriptorOwner, Metadata};

struct GlobalState {
    write_owner: FieldDescriptorOwner,
    read_owner: FieldDescriptorOwner,
}

impl GlobalState {
    fn with_metadata_mut<R>(&self, callback: impl FnOnce(&mut Metadata) -> R) -> R {
        self.write_owner
            .descriptor_table_mut()
            .with_metadata_mut(|metadata| callback(metadata))
    }
}

fn main() {
    let write_owner = FieldDescriptorOwner::new();
    let state = GlobalState {
        read_owner: write_owner.clone_handle(),
        write_owner,
    };
    state.read_owner.assume_alias(&state.write_owner);
    state.with_metadata_mut(|_metadata| state.read_owner.read());
}
