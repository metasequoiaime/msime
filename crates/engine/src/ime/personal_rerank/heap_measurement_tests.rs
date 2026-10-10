use super::allocations::{count, measure};

#[test]
fn heap_measurement_tracks_overlap_release_reallocation_and_returned_storage() {
    let (bytes, measured) = measure(|| {
        let first = std::hint::black_box(vec![0_u8; 32]);
        let mut bytes = Vec::<u8>::with_capacity(64);
        bytes.resize(64, 1);
        bytes.reserve_exact(64);
        assert_eq!(bytes.capacity(), 128);
        drop(first);
        bytes.truncate(16);
        bytes.shrink_to_fit();
        assert_eq!(bytes.capacity(), 16);
        bytes
    });
    assert_eq!(measured.allocations, 4);
    assert_eq!(measured.peak_bytes, 160);
    assert_eq!(measured.remaining_bytes, 16);
    assert_eq!(measured.minimum_bytes, 0);
    assert_eq!(bytes, [1; 16]);
    let ((), measured) = measure(|| {
        let first = std::hint::black_box(vec![0_u8; 32]);
        drop(first);
        let second = std::hint::black_box(vec![0_u8; 64]);
        drop(second);
    });
    assert_eq!(measured.allocations, 2);
    assert_eq!(measured.peak_bytes, 64);
    assert_eq!(measured.remaining_bytes, 0);
}

#[test]
fn heap_measurement_is_thread_local_and_cleans_up_after_unwinding() {
    let worker = std::thread::spawn(|| {
        let bytes = std::hint::black_box(vec![0_u8; 1024]);
        assert_eq!(bytes.len(), 1024);
    });
    let ((), measured) = measure(|| worker.join().expect("合成工作线程"));
    // join 会释放区间前创建的线程存储，因此只核对峰值，没有把负差值当在用字节。
    assert_eq!(measured.peak_bytes, 0);
    assert!(measured.minimum_bytes < 0);
    let failure = std::panic::catch_unwind(|| {
        measure(|| {
            let _bytes = std::hint::black_box(vec![0_u8; 32]);
            panic!("合成 unwind");
        });
    });
    assert!(failure.is_err());
    let (bytes, measured) = measure(|| std::hint::black_box(vec![0_u8; 48]));
    assert_eq!(measured.allocations, 1);
    assert_eq!(measured.peak_bytes, 48);
    assert_eq!(measured.remaining_bytes, 48);
    assert_eq!(bytes.len(), 48);
    let (_, allocations) = count(|| std::hint::black_box(vec![0_u8; 16]));
    assert_eq!(allocations, 1);
}
