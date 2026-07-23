use rwlock_annotation_detection::{FieldDescriptorOwner, Metadata};

struct GlobalState {
    owner: FieldDescriptorOwner,
}

impl GlobalState {
    fn with_metadata_mut<R>(&self, callback: impl FnOnce(&mut Metadata) -> R) -> R {
        self.owner
            .descriptor_table_mut()
            .with_metadata_mut(|metadata| callback(metadata))
    }

    fn read(&self) {
        self.owner.read();
    }
}

fn main() {
    let state = GlobalState {
        owner: FieldDescriptorOwner::new(),
    };
    state.with_metadata_mut(|_metadata| state.read());
}
