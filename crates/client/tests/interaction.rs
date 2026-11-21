use kokoda_client::{Action::*, Controller, Quality::*};
use kokoda_media::{negotiate, Format::*, Window};

#[test]
fn clients_share_open_quality_revocation_and_reconnect_semantics() {
    let mut ui = Controller::default();
    let a = Window {
        id: 1,
        generation: 10,
    };
    let b = Window { id: 2, ..a };
    let next = Window {
        generation: 11,
        ..a
    };
    let c = ui.connected().unwrap();
    assert!(ui.connected().is_none());
    ui.catalog(c, [a, b]);
    assert_eq!(ui.open(c, a), Some(Open(a, Clear)));
    assert_eq!(ui.open(c, a), Some(Focus(a)));
    ui.open(c, b);
    assert_eq!(
        ui.quality(Smooth),
        [ApplyQuality(a, Smooth), ApplyQuality(b, Smooth)]
    );
    assert_eq!(ui.catalog(c, [next, b]), [Close(a)]);
    assert_eq!(ui.open(c, a), None);
    assert_eq!(ui.disconnected(c), [Close(b)]);
    let fresh = ui.connected().unwrap();
    ui.catalog(c, [a]);
    assert_eq!(ui.open(fresh, a), None);
    ui.catalog(fresh, [next]);
    assert_eq!(ui.open(fresh, next), Some(Open(next, Smooth)));
    assert!(ui.disconnected(c).is_empty());
    assert_eq!(ui.close(fresh, next), Some(Close(next)));
}

#[test]
fn backend_selection_respects_both_endpoints() {
    assert_eq!(negotiate(&[H264, Png], &[Png]), Some(Png));
    assert_eq!(negotiate(&[H264], &[Png]), None);
}
