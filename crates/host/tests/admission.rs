use kokoda_host::{Admission, AdmissionError};

#[test]
fn a_busy_host_and_late_disconnect_cannot_replace_the_active_client() {
    let mut host = Admission::default();
    let first = host.admit().unwrap();
    assert_eq!(host.admit(), Err(AdmissionError::Busy));
    assert!(host.is_active(first));
    assert!(host.release(first));
    let next = host.admit().unwrap();
    assert_ne!(first, next);
    assert!(!host.is_active(first));
    assert!(!host.release(first));
    assert!(host.is_active(next));
}
