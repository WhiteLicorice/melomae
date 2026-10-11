//! After a successful load, the same library written another way is a no-op.
//!
//! The runtime is process-wide, so this test lives in its own binary and loads the pinned
//! library once. A path with an extra `.` segment resolves to the same library, so a later
//! call must return `Ok`.

mod common;
mod setup;

use melomae_core::inference::init_runtime;

#[test]
fn the_same_inference_runtime_library_written_another_way_is_a_no_op() {
    init_runtime(setup::ort_library()).expect("the pinned library must load");

    let library = setup::ort_library();
    let same = library
        .parent()
        .expect("the library has a parent directory")
        .join(".")
        .join(library.file_name().expect("the library has a file name"));
    init_runtime(&same).expect("the same resolved library must be accepted");
}
