use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use keybound::Action;
use keybound::BindingId;
use keybound::BindingOptions;
#[cfg(feature = "evdev")]
use keybound::DeviceFilter;
use keybound::Key;
use keybound::Modifier;
use keybound::Passthrough;

#[test]
fn action_from_closure_runs_callback() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_for_closure = Arc::clone(&call_count);

    let action = Action::from(move || {
        call_count_for_closure.fetch_add(1, Ordering::SeqCst);
    });

    match action {
        Action::Callback(callback) => {
            callback();
        }
        _ => panic!("expected callback action"),
    }

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn generated_binding_ids_are_unique() {
    let mut ids = HashSet::new();

    for _ in 0..128 {
        let id = BindingId::new();
        assert!(ids.insert(id));
    }
}

#[test]
fn binding_options_default_to_consuming_events() {
    let options = BindingOptions::default();
    assert_eq!(options.passthrough(), Passthrough::Consume);
}

#[cfg(feature = "evdev")]
#[test]
fn device_filter_supports_name_pattern_and_usb_id() {
    let by_name = DeviceFilter::NamePattern("kbd-*".into());
    let by_usb = DeviceFilter::Usb {
        vendor_id: 0x1209,
        product_id: 0x0001,
    };

    let options = BindingOptions::default().with_device_filter(by_name.clone());
    assert_eq!(options.device_filter(), Some(&by_name));

    match by_usb {
        DeviceFilter::Usb {
            vendor_id,
            product_id,
        } => {
            assert_eq!(vendor_id, 0x1209);
            assert_eq!(product_id, 0x0001);
        }
        DeviceFilter::NamePattern(_) => panic!("expected usb filter"),
    }
}

#[test]
fn action_variants_exist_for_future_features() {
    let _ = Action::EmitKey(Key::Escape, vec![Modifier::Ctrl]);
    let _ = Action::PushLayer("nav".into());
    let _ = Action::ToggleLayer("nav".into());
    let _ = Action::PopLayer;
    let _ = Action::Swallow;
}
